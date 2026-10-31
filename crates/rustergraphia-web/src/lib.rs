//! Rustergraphia Web Application
//! =============================
//!
//! The Ruby app (app.rb and web/) served the Rust way: the same pages to the
//! byte, the same headers, the same chapter each day. The middleware runs in
//! the Rack stack's order; every request's library work happens inside one
//! blocking closure, whose end lets the per-thread parse cache go.
//!
//! The router is built by [`app`] from a [`Config`]: the binary (main.rs)
//! reads it from the environment, the suites pass their own, so the front
//! door's key and the chapter's date are set per test, never through a
//! process-wide variable.

pub mod bench;
pub mod chapter;
pub mod chapter_check;
mod error;
mod esc;
mod figures;
pub mod form;
mod handlers;
pub mod middleware;
mod notebook_sent;
mod plate_file;
mod reader_game;
mod statics;
mod tray;
mod view;
mod views;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header::HeaderValue, Method, Request, Response};
use chrono::NaiveDate;

/// What the Ruby app read from its environment, given to the router.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// FRONT_DOOR_KEY: only a request carrying it in `X-Front-Door-Key` is
    /// answered (`/up` stays open). None or empty: every door is open.
    pub front_door_key: Option<String>,
    /// CHAPTER_DATE: the day the chapter counts as today (a rehearsal of a
    /// day to come). None: the clock's, midnight UTC.
    pub chapter_date: Option<NaiveDate>,
    /// Draws every inscribed plate's splits, wear and breaks from
    /// `Random.new(seed)` instead of the system's randomness, so a result
    /// page is the very page the Ruby app lays from the same seed. For the
    /// parity suites only: never read from the environment.
    #[doc(hidden)]
    pub plate_seed: Option<i64>,
}

impl Config {
    /// The configuration the Ruby app reads from its environment.
    pub fn from_env() -> Config {
        Config {
            front_door_key: std::env::var("FRONT_DOOR_KEY").ok(),
            chapter_date: std::env::var("CHAPTER_DATE")
                .ok()
                .and_then(|text| chapter::parse(&text)),
            plate_seed: None,
        }
    }

    fn door_key(&self) -> Option<&str> {
        self.front_door_key.as_deref().filter(|key| !key.is_empty())
    }
}

/// The whole app, as one router: every request goes through [`handle`].
pub fn app(config: Config) -> axum::Router {
    let config = Arc::new(config);
    axum::Router::new().fallback(move |request: Request<Body>| {
        let config = config.clone();
        async move { handle(&config, request).await }
    })
}

/// One request, in the Rack stack's order.
async fn handle(config: &Config, request: Request<Body>) -> Response<Body> {
    let method = request.method().clone();
    let raw_path = request.uri().path().to_string();
    let query = request.uri().query().map(str::to_string);
    let headers = request.headers().clone();
    let path = middleware::clean_path(&raw_path);
    let accept_encoding = headers
        .get("accept-encoding")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let fullpath = match &query {
        Some(query) => format!("{path}?{query}"),
        None => path.clone(),
    };

    // BodyLimit: refused before anything is parsed, before FrontDoor
    let content_length = headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(0);
    if content_length > middleware::BODY_LIMIT as u64 {
        return finish_plain(
            413,
            "The request is larger than any plate.\n",
            accept_encoding.as_deref(),
            &fullpath,
            &method,
        );
    }

    // FrontDoor
    if let Some(door_key) = config.door_key().filter(|_| path != "/up") {
        let given = headers
            .get("x-front-door-key")
            .map(|v| v.as_bytes().to_vec())
            .unwrap_or_default();
        if !constant_time_eq(&given, door_key.as_bytes()) {
            return finish_plain(
                403,
                "Not through the front door.\n",
                accept_encoding.as_deref(),
                &fullpath,
                &method,
            );
        }
    }

    // Sinatra's dispatch! reads the params first, static files or not: the
    // query, then a form body (Rack's form_data?: a form media type, or a
    // POST with none). A `%` that is not an escape is a 400 before anything
    // else, the before filter's headers included.
    let body = axum::body::to_bytes(request.into_body(), middleware::BODY_LIMIT + 1024 * 1024)
        .await
        .unwrap_or_default();
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    const QUERY: &str = "Invalid query parameters";
    const MULTIPART: &str = "Invalid multipart/form-data";
    let mut params = form::Params::default();
    let mut parsed = query.as_ref().map_or(Ok(()), |query| {
        params
            .parse_urlencoded(query.as_bytes())
            .map_err(|problem| (QUERY, problem))
    });
    if parsed.is_ok() {
        parsed = match media_type.as_str() {
            "application/x-www-form-urlencoded" => params
                .parse_urlencoded(&body)
                .map_err(|problem| (QUERY, problem)),
            "" if method == Method::POST => params
                .parse_urlencoded(&body)
                .map_err(|problem| (QUERY, problem)),
            "multipart/form-data" | "multipart/related" | "multipart/mixed" => {
                parse_multipart(&content_type, body.clone(), &mut params)
                    .await
                    .map_err(|problem| (MULTIPART, problem))
            }
            _ => Ok(()),
        };
    }
    if let Err((what, problem)) = parsed {
        // Sinatra's BadRequest: the message escaped when raised, and again
        // when written
        let message = format!("{what}: {}", esc::h(&esc::h(&problem)));
        let mut response = Response::builder().status(400);
        let response_headers = response.headers_mut().expect("fresh builder");
        response_headers.insert(
            "content-type",
            HeaderValue::from_static("text/html;charset=utf-8"),
        );
        return finish(
            response,
            message.into_bytes(),
            accept_encoding.as_deref(),
            &fullpath,
            &method,
        );
    }

    // Static files: GET/HEAD, before routing, before the leaf headers
    if method == Method::GET || method == Method::HEAD {
        if let Some(file) = statics::serve(&path, &headers) {
            let mut response = Response::builder().status(file.status);
            let response_headers = response.headers_mut().expect("fresh builder");
            *response_headers = file.headers;
            return finish(
                response,
                file.body,
                accept_encoding.as_deref(),
                &fullpath,
                &method,
            );
        }
    }

    // The routed app, in one blocking closure
    let base_url = middleware::base_url(&headers);
    let ssl = middleware::is_ssl(&headers);
    let method_name = method.as_str().to_string();
    let route_path = path.clone();
    let config = config.clone();
    let (bug_base_url, bug_path) = (base_url.clone(), path.clone());
    let page = tokio::task::spawn_blocking(move || {
        handlers::route(&method_name, &route_path, &mut params, &base_url, &config)
    })
    .await
    .unwrap_or_else(|_| handlers::bug_page(&bug_base_url, &bug_path));

    let mut response = Response::builder().status(page.status);
    let response_headers = response.headers_mut().expect("fresh builder");
    response_headers.insert("content-type", HeaderValue::from_static(page.content_type));
    middleware::leaf_headers(response_headers, &method, &path, ssl);
    if let Some(cache) = &page.cache_control {
        if let Ok(value) = HeaderValue::from_str(cache) {
            response_headers.insert("cache-control", value);
        }
    }
    if page.x_cascade {
        response_headers.insert("x-cascade", HeaderValue::from_static("pass"));
    }
    finish(
        response,
        page.body,
        accept_encoding.as_deref(),
        &fullpath,
        &method,
    )
}

/// A multipart body read as Rack reads it: nothing for an empty body or a
/// content type without a boundary; a body that ends before its closing
/// boundary (cut short, or under another boundary) is Rack's
/// EmptyContentError, which Sinatra answers 400. A file part with no file
/// chosen (`filename=""`) is left out; a file under any name but `file` is
/// kept as Ruby's upload Hash would read as text (not a plate).
async fn parse_multipart(
    content_type: &str,
    body: axum::body::Bytes,
    params: &mut form::Params,
) -> Result<(), String> {
    const EMPTY_CONTENT: &str = "Rack::Multipart::EmptyContentError";
    if body.is_empty() {
        return Ok(());
    }
    let Some(boundary) = multipart_boundary(content_type) else {
        return Ok(());
    };
    let stream = futures_stream(body);
    let mut multipart = multer::Multipart::new(stream, boundary);
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => return Ok(()),
            Err(_) => return Err(EMPTY_CONTENT.to_string()),
        };
        let name = field.name().unwrap_or("").to_string();
        let filename = field.file_name().map(str::to_string);
        let bytes = field.bytes().await.map_err(|_| EMPTY_CONTENT.to_string())?;
        match filename.as_deref() {
            Some("") => {}
            Some(_) if name == "file" => {
                let take = bytes.len().min(form::MAX_SVG_BYTES + 1);
                params.upload = Some(bytes[..take].to_vec());
            }
            Some(filename) => params.set(
                name.clone(),
                format!("{{\"filename\" => {filename:?}, \"name\" => {name:?}}}"),
            ),
            None => params.set(name, String::from_utf8_lossy(&bytes).into_owned()),
        }
    }
}

/// Rack's `MULTIPART` match: `multipart/…boundary="?([^";,]+)"?`, any
/// multipart subtype, the parameter's name in any case.
fn multipart_boundary(content_type: &str) -> Option<String> {
    let lowered = content_type.to_ascii_lowercase();
    if !lowered.starts_with("multipart/") {
        return None;
    }
    let at = lowered.find("boundary")? + "boundary".len();
    let rest = content_type[at..].strip_prefix('=')?;
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let end = rest.find(['"', ';', ',']).unwrap_or(rest.len());
    (end > 0).then(|| rest[..end].to_string())
}

fn futures_stream(
    body: axum::body::Bytes,
) -> impl futures_util::Stream<Item = Result<axum::body::Bytes, std::io::Error>> {
    futures_util::stream::once(async move { Ok(body) })
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

fn finish_plain(
    status: u16,
    body: &str,
    accept_encoding: Option<&str>,
    fullpath: &str,
    method: &Method,
) -> Response<Body> {
    let mut response = Response::builder().status(status);
    let headers = response.headers_mut().expect("fresh builder");
    headers.insert(
        "content-type",
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert("cache-control", HeaderValue::from_static("no-store"));
    finish(
        response,
        body.as_bytes().to_vec(),
        accept_encoding,
        fullpath,
        method,
    )
}

/// The response's way out: Rack::Protection's headers, the deflater, HEAD.
fn finish(
    mut response: axum::http::response::Builder,
    body: Vec<u8>,
    accept_encoding: Option<&str>,
    fullpath: &str,
    method: &Method,
) -> Response<Body> {
    let headers = response.headers_mut().expect("builder");
    middleware::protection_headers(headers);
    let mut built = response.body(Body::empty()).expect("response");
    let status = built.status().as_u16();
    let mut body = body;
    if !middleware::skips_deflate(status, built.headers()) {
        middleware::add_vary(built.headers_mut());
        match middleware::negotiate(accept_encoding) {
            middleware::Encoding::Gzip => {
                body = middleware::gzip(&body);
                built
                    .headers_mut()
                    .insert("content-encoding", HeaderValue::from_static("gzip"));
                built.headers_mut().remove("content-length");
            }
            middleware::Encoding::Identity => {}
            middleware::Encoding::NotAcceptable => {
                let message = format!(
                    "An acceptable encoding for the requested resource {fullpath} could not be found."
                );
                let mut refusal = Response::builder()
                    .status(406)
                    .header("content-type", "text/plain")
                    .body(Body::from(message))
                    .expect("response");
                middleware::protection_headers(refusal.headers_mut());
                return refusal;
            }
        }
    }
    if method == Method::HEAD {
        body = Vec::new();
    }
    *built.body_mut() = Body::from(body);
    built
}
