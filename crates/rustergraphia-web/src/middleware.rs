//! The request's passage, in the Ruby stack's order (web/leaf_headers.rb,
//! web/body_limit.rb, web/front_door.rb, Rack::Protection's observed
//! behaviour, Rack::Deflater): path cleanup before routing, the body limit
//! and the front door before anything is parsed, the leaf headers on every
//! routed response, nosniff everywhere, gzip on the way out.

use std::io::Write;

use axum::http::{HeaderMap, HeaderValue, Method};

pub const MAX_SVG_BYTES: usize = 2 * 1024 * 1024;
pub const BODY_LIMIT: usize = MAX_SVG_BYTES + 64 * 1024; // 2_162_688
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; img-src 'self' data: blob:; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// Rack::Protection::PathTraversal's cleanup, applied before routing: the
/// routed path, `request.path` and the static lookup all see the result.
pub fn clean_path(path: &str) -> String {
    let unescaped = path
        .replace("%2e", ".")
        .replace("%2E", ".")
        .replace("%2f", "/")
        .replace("%2F", "/")
        .replace("%5c", "\\")
        .replace("%5C", "\\")
        .replace('\\', "/");
    let mut cleaned: Vec<&str> = Vec::new();
    for part in unescaped.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                cleaned.pop();
            }
            part => cleaned.push(part),
        }
    }
    let mut out = String::from("/");
    out.push_str(&cleaned.join("/"));
    if !cleaned.is_empty()
        && (unescaped.ends_with('/') || unescaped.ends_with("/.") || unescaped.ends_with("/.."))
        && !out.ends_with('/')
    {
        out.push('/');
    }
    out
}

/// Whether the request is spoken for by HTTPS (Rack's `request.ssl?`).
pub fn is_ssl(headers: &HeaderMap) -> bool {
    scheme(headers) == "https" || scheme(headers) == "wss"
}

fn scheme(headers: &HeaderMap) -> String {
    if headers
        .get("x-forwarded-ssl")
        .is_some_and(|v| v.as_bytes() == b"on")
    {
        return "https".to_string();
    }
    if let Some(forwarded) = header_str(headers, "forwarded") {
        let mut found = None;
        for part in forwarded.split(';').flat_map(|p| p.split(',')) {
            let part = part.trim();
            if let Some(proto) = part.strip_prefix("proto=") {
                found = Some(proto.trim().to_ascii_lowercase());
            }
        }
        if let Some(proto) = found {
            return proto;
        }
    }
    if let Some(protos) = header_str(headers, "x-forwarded-proto") {
        if let Some(last) = protos.split(',').next_back() {
            let last = last.trim();
            if !last.is_empty() {
                return last.to_ascii_lowercase();
            }
        }
    }
    if let Some(scheme) = header_str(headers, "x-forwarded-scheme") {
        return scheme.trim().to_ascii_lowercase();
    }
    "http".to_string()
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// `request.base_url`: scheme and authority as Rack reads them.
pub fn base_url(headers: &HeaderMap) -> String {
    let scheme = scheme(headers);
    let host = authority(headers);
    let host = match (scheme.as_str(), host) {
        ("https", host) => host
            .strip_suffix(":443")
            .map(str::to_string)
            .unwrap_or(host),
        ("http", host) => host.strip_suffix(":80").map(str::to_string).unwrap_or(host),
        (_, host) => host,
    };
    format!("{scheme}://{host}")
}

fn authority(headers: &HeaderMap) -> String {
    if let Some(forwarded) = header_str(headers, "forwarded") {
        let mut found = None;
        for part in forwarded.split(';').flat_map(|p| p.split(',')) {
            let part = part.trim();
            if let Some(host) = part.strip_prefix("host=") {
                found = Some(host.trim().trim_matches('"').to_string());
            }
        }
        if let Some(host) = found {
            return host;
        }
    }
    if let Some(hosts) = header_str(headers, "x-forwarded-host") {
        if let Some(last) = hosts.split(',').next_back() {
            let last = last.trim();
            if !last.is_empty() {
                return last.to_string();
            }
        }
    }
    header_str(headers, "host")
        .unwrap_or("localhost")
        .to_string()
}

/// The before filter (web/leaf_headers.rb), on every routed request.
pub fn leaf_headers(headers: &mut HeaderMap, method: &Method, path: &str, ssl: bool) {
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    if ssl {
        headers.insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    if method == Method::POST {
        headers.insert("cache-control", HeaderValue::from_static("no-store"));
    }
    // DELIBERATE DEVIATION from the Ruby app: it marks `/decode` cacheable for
    // any method, so even `POST /decode` (whose body holds the decoded
    // message) goes out `public, max-age=300` and a shared cache may keep it.
    // Here a POST keeps `no-store`; GET/HEAD stay cacheable as in Ruby.
    // Flagged in docs/web-port-spec.md §15 and the PR.
    if matches!(path, "/" | "/decode" | "/about") && method != Method::POST {
        headers.insert(
            "cache-control",
            HeaderValue::from_static("public, max-age=300"),
        );
    }
}

/// Rack::Protection's surviving headers: nosniff on everything, the XSS and
/// frame headers on HTML alone.
pub fn protection_headers(headers: &mut HeaderMap) {
    let html = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| t.starts_with("text/html"));
    if !headers.contains_key("x-content-type-options") {
        headers.insert(
            "x-content-type-options",
            HeaderValue::from_static("nosniff"),
        );
    }
    if html {
        headers.insert(
            "x-xss-protection",
            HeaderValue::from_static("1; mode=block"),
        );
        headers.insert("x-frame-options", HeaderValue::from_static("SAMEORIGIN"));
    }
}

/// What Rack::Deflater decides for a response.
pub enum Encoding {
    Gzip,
    Identity,
    NotAcceptable,
}

/// `select_best_encoding(%w[gzip identity], accept)`, Rack's semantics: the
/// q-weighted best of gzip and identity; identity is acceptable unless
/// explicitly forbidden.
pub fn negotiate(accept: Option<&str>) -> Encoding {
    let Some(accept) = accept else {
        return Encoding::Identity;
    };
    let mut gzip_q: Option<f32> = None;
    let mut identity_q: Option<f32> = None;
    let mut star_q: Option<f32> = None;
    for part in accept.split(',') {
        let mut sides = part.trim().split(';');
        let name = sides.next().unwrap_or("").trim().to_ascii_lowercase();
        let q = sides
            .find_map(|p| p.trim().strip_prefix("q=").map(str::to_string))
            .and_then(|q| q.parse::<f32>().ok())
            .unwrap_or(1.0);
        match name.as_str() {
            "gzip" => gzip_q = Some(q),
            "identity" => identity_q = Some(q),
            "*" => star_q = Some(q),
            _ => {}
        }
    }
    let gzip = gzip_q.or(star_q).unwrap_or(0.0);
    let identity = identity_q.or(star_q).unwrap_or(1.0); // identity is assumed acceptable
    if gzip > 0.0 && gzip >= identity {
        return Encoding::Gzip;
    }
    if identity > 0.0 {
        return Encoding::Identity;
    }
    Encoding::NotAcceptable
}

/// Whether the deflater leaves a response alone (no Vary either).
pub fn skips_deflate(status: u16, headers: &HeaderMap) -> bool {
    if (100..200).contains(&status) || status == 204 || status == 304 {
        return true;
    }
    if headers
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.to_ascii_lowercase().contains("no-transform"))
    {
        return true;
    }
    if headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v != "identity")
    {
        return true;
    }
    if headers
        .get("content-length")
        .is_some_and(|v| v.as_bytes() == b"0")
    {
        return true;
    }
    false
}

pub fn add_vary(headers: &mut HeaderMap) {
    let existing = headers
        .get("vary")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let lowered = existing.to_ascii_lowercase();
    if lowered.split(',').any(|v| {
        let v = v.trim();
        v == "*" || v == "accept-encoding"
    }) {
        return;
    }
    let value = if existing.is_empty() {
        "Accept-Encoding".to_string()
    } else {
        format!("{existing},Accept-Encoding")
    };
    if let Ok(value) = HeaderValue::from_str(&value) {
        headers.insert("vary", value);
    }
}

pub fn gzip(body: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(body).expect("gzip to memory");
    encoder.finish().expect("gzip to memory")
}
