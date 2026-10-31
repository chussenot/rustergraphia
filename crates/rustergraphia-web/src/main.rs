//! Rustergraphia Web Application
//! =============================
//!
//! The Ruby app (app.rb and web/) served the Rust way: the same pages to the
//! byte, the same headers, the same chapter each day. The middleware runs in
//! the Rack stack's order; every request's library work happens inside one
//! blocking closure, whose end lets the per-thread parse cache go.
//!
//! Subcommands mirror the chapter rake tasks: `chapter-check [days]`,
//! `chapter-runway`.

mod bench;
mod chapter;
mod chapter_check;
mod error;
mod esc;
mod figures;
mod form;
mod handlers;
mod middleware;
mod plate_file;
mod reader_game;
mod statics;
mod tray;
mod view;
mod views;

use axum::body::Body;
use axum::http::{header::HeaderValue, Method, Request, Response};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("chapter-check") => {
            let days = args.get(2).and_then(|d| d.parse().ok()).unwrap_or(90);
            std::process::exit(chapter_check::check(days));
        }
        Some("chapter-runway") => {
            std::process::exit(chapter_check::runway());
        }
        _ => {}
    }
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9292);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("bind the server port");
    let bound = listener.local_addr().expect("a bound address");
    println!("rustergraphia-web listening on {bound}");
    let app = axum::Router::new().fallback(handle);
    axum::serve(listener, app).await.expect("serve");
}

/// One request, in the Rack stack's order.
async fn handle(request: Request<Body>) -> Response<Body> {
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
    let door_key = std::env::var("FRONT_DOOR_KEY").unwrap_or_default();
    if !door_key.is_empty() && path != "/up" {
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

    // The routed app: body read, params parsed, one blocking closure
    let body = axum::body::to_bytes(request.into_body(), middleware::BODY_LIMIT + 1024 * 1024)
        .await
        .unwrap_or_default();
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let mut params = form::Params::default();
    if let Some(query) = &query {
        params.parse_urlencoded(query.as_bytes());
    }
    if content_type.starts_with("application/x-www-form-urlencoded") {
        params.parse_urlencoded(&body);
    } else if content_type.starts_with("multipart/form-data") {
        parse_multipart(&content_type, body.clone(), &mut params).await;
    }
    let base_url = middleware::base_url(&headers);
    let ssl = middleware::is_ssl(&headers);
    let method_name = method.as_str().to_string();
    let route_path = path.clone();
    let page = tokio::task::spawn_blocking(move || {
        handlers::route(&method_name, &route_path, &mut params, &base_url)
    })
    .await;
    let page = match page {
        Ok(page) => page,
        Err(_) => handlers::Page {
            status: 500,
            content_type: "text/html;charset=utf-8",
            body: b"Something unexpected happened, please try again.".to_vec(),
            cache_control: None,
            x_cascade: false,
        },
    };

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

async fn parse_multipart(content_type: &str, body: axum::body::Bytes, params: &mut form::Params) {
    let Some(boundary) = multer::parse_boundary(content_type).ok() else {
        return;
    };
    let stream = futures_stream(body);
    let mut multipart = multer::Multipart::new(stream, boundary);
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        let has_filename = field.file_name().is_some();
        let bytes = field.bytes().await.unwrap_or_default();
        if has_filename {
            if name == "file" {
                let take = bytes.len().min(form::MAX_SVG_BYTES + 1);
                params.upload = Some(bytes[..take].to_vec());
            }
        } else {
            params.set(name, String::from_utf8_lossy(&bytes).into_owned());
        }
    }
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
