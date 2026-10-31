//! What the Ruby suites had from Rack::Test and the standard library, for
//! the ported suites: a session that drives the router in-process
//! (`tower::ServiceExt::oneshot`) and keeps its last response, the form and
//! multipart encodings rack-test sends, and `CGI.unescapeHTML`, `JSON.parse`
//! and the regex captures the assertions read pages with.

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{HeaderMap, Request};
use regex::Regex;
use tower::ServiceExt;

pub use rustergraphia_web::{app, Config};

/// A response as rack-test's `last_response` gives it: status, headers and
/// the body as sent (not decompressed).
pub struct Response {
    pub status: u16,
    pub headers: HeaderMap,
    pub raw: Vec<u8>,
    pub body: String,
}

impl Response {
    /// `last_response.ok?`
    pub fn is_ok(&self) -> bool {
        self.status == 200
    }

    pub fn header(&self, name: &str) -> Option<String> {
        self.headers
            .get(name)
            .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
    }

    /// `last_response.content_type`
    pub fn content_type(&self) -> String {
        self.header("content-type").unwrap_or_default()
    }

    /// `last_response.media_type`: the content type without its parameters.
    pub fn media_type(&self) -> String {
        self.content_type()
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

/// One part of a multipart form: a field, or a file (`Rack::Test::UploadedFile`).
pub enum FormPart<'a> {
    Field(&'a str, &'a str),
    File {
        name: &'a str,
        filename: &'a str,
        content_type: &'a str,
        content: &'a [u8],
    },
}

/// `Rack::Test::Session` over the router: requests go to `example.org`, as
/// rack-test's default host.
pub struct Session {
    app: axum::Router,
    runtime: tokio::runtime::Runtime,
    last: Option<Response>,
}

impl Default for Session {
    fn default() -> Self {
        Session::new()
    }
}

impl Session {
    /// The app as the suites run it: no front door key, the clock's day.
    pub fn new() -> Session {
        Session::with(Config::default())
    }

    pub fn with(config: Config) -> Session {
        Session {
            app: app(config),
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("a runtime"),
            last: None,
        }
    }

    /// `last_response`
    pub fn last(&self) -> &Response {
        self.last.as_ref().expect("a request was made")
    }

    /// `last_response.body`
    pub fn body(&self) -> &str {
        &self.last().body
    }

    pub fn status(&self) -> u16 {
        self.last().status
    }

    pub fn get(&mut self, path: &str) -> &Response {
        self.get_with(path, &[])
    }

    pub fn get_with(&mut self, path: &str, headers: &[(&str, &str)]) -> &Response {
        self.request("GET", path, headers, None)
    }

    /// `post path, params`: an application/x-www-form-urlencoded body.
    pub fn post(&mut self, path: &str, form: &[(&str, &str)]) -> &Response {
        self.post_with(path, form, &[])
    }

    pub fn post_with(
        &mut self,
        path: &str,
        form: &[(&str, &str)],
        headers: &[(&str, &str)],
    ) -> &Response {
        let body = form_encode(form);
        self.request(
            "POST",
            path,
            headers,
            Some((
                "application/x-www-form-urlencoded".to_string(),
                body.into_bytes(),
            )),
        )
    }

    /// `post path, file: UploadedFile, ...`: a multipart/form-data body.
    pub fn post_multipart(&mut self, path: &str, parts: &[FormPart]) -> &Response {
        let boundary = "----RackMultipartBoundary0123456789";
        let mut body = Vec::new();
        for part in parts {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            match part {
                FormPart::Field(name, value) => {
                    body.extend_from_slice(
                        format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n")
                            .as_bytes(),
                    );
                    body.extend_from_slice(value.as_bytes());
                }
                FormPart::File {
                    name,
                    filename,
                    content_type,
                    content,
                } => {
                    body.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
                        )
                        .as_bytes(),
                    );
                    body.extend_from_slice(content);
                }
            }
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        self.request(
            "POST",
            path,
            &[],
            Some((format!("multipart/form-data; boundary={boundary}"), body)),
        )
    }

    pub fn request(
        &mut self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<(String, Vec<u8>)>,
    ) -> &Response {
        let mut request = Request::builder().method(method).uri(path);
        let has_host = headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("host"));
        if !has_host {
            request = request.header("host", "example.org");
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let given_length = headers
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case("content-length"));
        let request = match body {
            Some((content_type, bytes)) => {
                // an empty content type: the request sends none
                let mut request = request;
                if !content_type.is_empty() {
                    request = request.header("content-type", content_type);
                }
                if !given_length {
                    request = request.header("content-length", bytes.len().to_string());
                }
                request.body(Body::from(bytes))
            }
            None => request.body(Body::empty()),
        }
        .expect("a request");
        let app = self.app.clone();
        let response = self.runtime.block_on(async move {
            let response = app.oneshot(request).await.expect("the router answers");
            let status = response.status().as_u16();
            let headers = response.headers().clone();
            let raw = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("a body")
                .to_vec();
            (status, headers, raw)
        });
        let (status, headers, raw) = response;
        let body = String::from_utf8_lossy(&raw).into_owned();
        self.last = Some(Response {
            status,
            headers,
            raw,
            body,
        });
        self.last()
    }
}

/// `URI.encode_www_form_component`, as rack-test builds a form body.
pub fn form_encode(form: &[(&str, &str)]) -> String {
    form.iter()
        .map(|(name, value)| format!("{}={}", urlencode(name), urlencode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// `CGI.unescapeHTML`: named and numeric character references.
pub fn unescape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';').filter(|&end| end <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// `CGI.escapeHTML`
pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// `text[regex, 1]`: the first group of the first match.
pub fn capture(text: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .expect("a pattern")
        .captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// `text[regex]`: the whole first match.
pub fn matched(text: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .expect("a pattern")
        .find(text)
        .map(|m| m.as_str().to_string())
}

/// `text.scan(regex)` of the groups.
pub fn scan_groups(text: &str, pattern: &str) -> Vec<Vec<String>> {
    Regex::new(pattern)
        .expect("a pattern")
        .captures_iter(text)
        .map(|caps| {
            caps.iter()
                .skip(1)
                .map(|m| m.map_or(String::new(), |m| m.as_str().to_string()))
                .collect()
        })
        .collect()
}

/// `text.scan(needle).size`
pub fn count(text: &str, needle: &str) -> usize {
    text.matches(needle).count()
}

pub fn is_match(text: &str, pattern: &str) -> bool {
    Regex::new(pattern).expect("a pattern").is_match(text)
}

/// `assert_match(pattern, text)`
#[track_caller]
pub fn assert_match(pattern: &str, text: &str, message: &str) {
    assert!(
        is_match(text, pattern),
        "{message}: expected /{pattern}/ to match the page"
    );
}

/// `refute_match(pattern, text)`
#[track_caller]
pub fn refute_match(pattern: &str, text: &str, message: &str) {
    let found = matched(text, pattern);
    assert!(
        found.is_none(),
        "{message}: expected /{pattern}/ not to match, found {found:?}"
    );
}

/// `assert_includes text, needle`
#[track_caller]
pub fn assert_includes(text: &str, needle: &str, message: &str) {
    assert!(
        text.contains(needle),
        "{message}: expected the page to include {needle:?}"
    );
}

/// `refute_includes text, needle`
#[track_caller]
pub fn refute_includes(text: &str, needle: &str, message: &str) {
    assert!(
        !text.contains(needle),
        "{message}: expected the page not to include {needle:?}"
    );
}

/// `JSON.parse(CGI.unescapeHTML(body[/attr="([^"]*)"/, 1]))`
#[track_caller]
pub fn json_attr(body: &str, attribute: &str) -> serde_json::Value {
    let raw = capture(body, &format!(r#"{attribute}="([^"]*)""#))
        .unwrap_or_else(|| panic!("the page carries {attribute}"));
    serde_json::from_str(&unescape_html(&raw))
        .unwrap_or_else(|error| panic!("{attribute} is JSON: {error}"))
}

/// `CGI.unescapeHTML(body[/<input type="hidden" name="svg" value="([^"]*)"/, 1])`
#[track_caller]
pub fn hidden_svg(body: &str) -> String {
    unescape_html(
        &capture(body, r#"<input type="hidden" name="svg" value="([^"]*)""#)
            .expect("the page carries its file"),
    )
}
