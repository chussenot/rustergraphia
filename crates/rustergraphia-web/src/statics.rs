//! Static files (public/), served as Sinatra serves them: before routing and
//! before the leaf headers, GET/HEAD only, with the content types, the
//! hour's cache, Last-Modified/304 and single ranges the suites observe.

use std::path::{Path, PathBuf};

use axum::http::{HeaderMap, HeaderValue};

pub struct StaticFile {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

/// The directory the assets are served from: RUSTERGRAPHIA_PUBLIC, or the
/// crate's own public/ (the files are copied verbatim from the Ruby app).
pub fn public_dir() -> PathBuf {
    std::env::var_os("RUSTERGRAPHIA_PUBLIC")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("public"))
}

fn content_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "css" => Some("text/css;charset=utf-8"),
        "js" => Some("text/javascript;charset=utf-8"),
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "woff2" => Some("font/woff2"),
        "txt" => Some("text/plain;charset=utf-8"),
        _ => Some("application/octet-stream"),
    }
}

/// The file a cleaned path names, or None to fall through to routing.
pub fn serve(path: &str, request_headers: &HeaderMap) -> Option<StaticFile> {
    let relative = path.strip_prefix('/')?;
    if relative.is_empty() || relative.contains("..") {
        return None;
    }
    let file = public_dir().join(relative);
    if !file.is_file() {
        return None;
    }
    let metadata = std::fs::metadata(&file).ok()?;
    let modified = metadata.modified().ok()?;
    let mut headers = HeaderMap::new();
    headers.insert(
        "cache-control",
        HeaderValue::from_static("public, max-age=3600"),
    );
    // If-Modified-Since at or after the mtime: 304 with only these headers
    if let Some(since) = request_headers
        .get("if-modified-since")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| httpdate::parse_http_date(v).ok())
    {
        let whole_seconds = std::time::UNIX_EPOCH
            + std::time::Duration::from_secs(
                modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            );
        if since >= whole_seconds {
            return Some(StaticFile {
                status: 304,
                headers,
                body: Vec::new(),
            });
        }
    }
    headers.insert(
        "last-modified",
        HeaderValue::from_str(&httpdate::fmt_http_date(modified)).ok()?,
    );
    headers.insert(
        "content-type",
        HeaderValue::from_static(content_type(&file)?),
    );
    let body = std::fs::read(&file).ok()?;
    // A single bytes range (the suites ask for one)
    if let Some(range) = request_headers
        .get("range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes="))
    {
        if let Some((from, to)) = parse_range(range, body.len()) {
            headers.insert(
                "content-range",
                HeaderValue::from_str(&format!("bytes {from}-{to}/{}", body.len())).ok()?,
            );
            return Some(StaticFile {
                status: 206,
                headers,
                body: body[from..=to].to_vec(),
            });
        }
    }
    Some(StaticFile {
        status: 200,
        headers,
        body,
    })
}

fn parse_range(range: &str, len: usize) -> Option<(usize, usize)> {
    let (from, to) = range.split_once('-')?;
    let from: usize = from.parse().ok()?;
    let to: usize = if to.is_empty() {
        len.checked_sub(1)?
    } else {
        to.parse::<usize>().ok()?.min(len.checked_sub(1)?)
    };
    (from <= to && from < len).then_some((from, to))
}
