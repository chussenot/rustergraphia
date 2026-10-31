//! The golden suite: every response the Ruby app gave (captured with
//! rack-test from the running original, tests/fixtures/golden/) replayed
//! against this server and compared byte for byte.
//!
//! Masked, by design: the `date` and `content-length` headers; the Lectio
//! sign the About page draws at random; `/chapter`'s live seconds-left
//! max-age; and `cache-control` on POST /decode and /ask responses — the
//! port's one sanctioned deviation sends `no-store` where the Ruby app lets
//! a shared cache keep a decoded message (docs/web-port-spec.md §15).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn gunzip(path: &PathBuf) -> Vec<u8> {
    let file = std::fs::File::open(path).unwrap_or_else(|_| panic!("missing {path:?}"));
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(file)
        .read_to_end(&mut out)
        .expect("a well-formed fixture");
    out
}

fn golden(name: &str, part: &str) -> Vec<u8> {
    gunzip(&fixtures().join(format!("golden/{name}.{part}.gz")))
}

fn plate(name: &str) -> Vec<u8> {
    gunzip(&fixtures().join(format!("plates/{name}.svg.gz")))
}

struct Server {
    child: Child,
    port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn boot() -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rustergraphia-web"))
        .env("PORT", "0")
        .env("CHAPTER_DATE", "2026-11-01")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the server binary boots");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut reader = std::io::BufReader::new(stdout);
    let mut line = String::new();
    std::io::BufRead::read_line(&mut reader, &mut line).expect("the listening line");
    let port: u16 = line
        .trim()
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .expect("the bound port");
    Server { child, port }
}

fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// One raw HTTP/1.1 request, one connection.
fn request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<(&str, &[u8])>,
) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let mut head =
        format!("{method} {path} HTTP/1.1\r\nHost: example.org\r\nConnection: close\r\n");
    let body_bytes = match body {
        Some((content_type, bytes)) => {
            head.push_str(&format!(
                "Content-Type: {content_type}\r\nContent-Length: {}\r\n",
                bytes.len()
            ));
            bytes.to_vec()
        }
        None => Vec::new(),
    };
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).expect("send head");
    stream.write_all(&body_bytes).expect("send body");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read response");
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a response head");
    let head_text = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head_text.lines();
    let status: u16 = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .expect("a status line");
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect();
    let mut body = raw[split + 4..].to_vec();
    if headers
        .iter()
        .any(|(n, v)| n == "transfer-encoding" && v.contains("chunked"))
    {
        body = dechunk(&body);
    }
    (status, headers, body)
}

fn dechunk(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < body.len() {
        let line_end = match body[at..].windows(2).position(|w| w == b"\r\n") {
            Some(position) => at + position,
            None => break,
        };
        let size = usize::from_str_radix(String::from_utf8_lossy(&body[at..line_end]).trim(), 16)
            .unwrap_or(0);
        if size == 0 {
            break;
        }
        let start = line_end + 2;
        out.extend_from_slice(&body[start..start + size]);
        at = start + size + 2;
    }
    out
}

/// A golden case: how the request is made, and what to mask.
struct Case {
    name: &'static str,
    method: &'static str,
    path: &'static str,
    form: &'static [(&'static str, &'static str)],
    plate: Option<&'static str>,
    /// cache-control differs on purpose (the POST /decode deviation).
    masked_cache: bool,
    /// the Lectio sign is drawn at random per request.
    lectio: bool,
    /// the max-age counts live seconds.
    live_max_age: bool,
}

const fn page(name: &'static str, path: &'static str) -> Case {
    Case {
        name,
        method: "GET",
        path,
        form: &[],
        plate: None,
        masked_cache: false,
        lectio: false,
        live_max_age: false,
    }
}

const fn post(
    name: &'static str,
    path: &'static str,
    form: &'static [(&'static str, &'static str)],
) -> Case {
    Case {
        name,
        method: "POST",
        path,
        form,
        plate: None,
        masked_cache: false,
        lectio: false,
        live_max_age: false,
    }
}

const fn decode(
    name: &'static str,
    plate_name: &'static str,
    form: &'static [(&'static str, &'static str)],
) -> Case {
    Case {
        name,
        method: "POST",
        path: "/decode",
        form,
        plate: Some(plate_name),
        masked_cache: true,
        lectio: false,
        live_max_age: false,
    }
}

fn cases() -> Vec<Case> {
    let mut cases = vec![
        page("root", "/"),
        page("decode_form", "/decode"),
        page("nope", "/nope"),
        page("up", "/up"),
        post("post_up", "/up", &[]),
        post(
            "gen_shortpass",
            "/generate",
            &[
                ("message", "Ora"),
                ("mode", "secret"),
                ("passphrase", "short"),
            ],
        ),
        post(
            "gen_nopass",
            "/generate",
            &[("message", "Ora"), ("mode", "secret"), ("passphrase", "")],
        ),
        post(
            "gen_badkey",
            "/generate",
            &[
                ("message", "Ora"),
                ("layout", "grille"),
                ("grille", "NOT A KEY"),
            ],
        ),
        post(
            "gen_secret_worn",
            "/generate",
            &[
                ("message", "Ora"),
                ("mode", "secret"),
                ("passphrase", "monastery gate"),
                ("condition", "worn"),
            ],
        ),
        post(
            "gen_broken_rongo",
            "/generate",
            &[
                ("message", "Ora"),
                ("layout", "rongorongo"),
                ("condition", "broken"),
            ],
        ),
        post(
            "gen_worn_decoy",
            "/generate",
            &[
                ("message", "Ora"),
                ("condition", "worn"),
                ("decoy", "hi"),
                ("layout", "grille"),
            ],
        ),
        post(
            "gen_secret_pathless",
            "/generate",
            &[
                ("message", "Ora"),
                ("mode", "secret"),
                ("passphrase", "monastery gate"),
                ("path", "withheld"),
            ],
        ),
        post(
            "gen_worn_own",
            "/generate",
            &[("message", "Ora"), ("condition", "worn"), ("tablet", "own")],
        ),
        post("gen_empty", "/generate", &[("message", "!!!")]),
        decode("dec_nosigns", "", &[("svg", "<svg/>")]),
        decode("dec_notxml", "", &[("svg", "not xml")]),
        decode("dec_worn5_0", "worn5", &[]),
        decode("dec_worn5_1", "worn5", &[("decipher", "1")]),
        decode("dec_worn_rongo6_0", "worn_rongo6", &[]),
        decode("dec_worn_rongo6_1", "worn_rongo6", &[("decipher", "1")]),
        decode("dec_tallied7_0", "tallied7", &[]),
        decode("dec_tallied7_1", "tallied7", &[("decipher", "1")]),
        decode("dec_wornpathless8_0", "wornpathless8", &[]),
        decode("dec_wornpathless8_1", "wornpathless8", &[("decipher", "1")]),
        decode("dec_broken3_0", "broken3", &[]),
        decode("dec_broken3_1", "broken3", &[("decipher", "1")]),
        decode("dec_ruined4_0", "ruined4", &[]),
        decode("dec_ruined4_1", "ruined4", &[("decipher", "1")]),
        decode("dec_ruined4_2", "ruined4", &[("rejoined", "1")]),
        decode("dec_pathless77_0", "pathless77", &[]),
        decode("dec_pathless77_1", "pathless77", &[("decipher", "1")]),
        decode("dec_tablet99_0", "tablet99", &[]),
        decode("dec_tablet99_1", "tablet99", &[("decipher", "1")]),
        decode("dec_expert12_0", "expert12", &[]),
        decode("dec_expert12_1", "expert12", &[("decipher", "1")]),
        decode("dec_grille55_0", "grille55", &[("grille", "K7Q2M-ZXB4P")]),
        decode(
            "dec_grille55_1",
            "grille55",
            &[("grille", "K7Q2M-ZXB4P"), ("decipher", "1")],
        ),
        decode("dec_grille55_2", "grille55", &[]),
        decode("dec_two13_0", "two13", &[("grille", "K7Q2M-ZXB4P")]),
        decode("dec_two13_1", "two13", &[("grille", "DYJ46-FPR8Z")]),
        decode(
            "dec_sealed_0",
            "sealed",
            &[("passphrase", "monastery gate")],
        ),
        decode("dec_sealed_1", "sealed", &[]),
        decode(
            "dec_worngrille9_0",
            "worngrille9",
            &[("grille", "K7Q2M-ZXB4P")],
        ),
        decode(
            "dec_worngrille9_1",
            "worngrille9",
            &[("grille", "K7Q2M-ZXB4P"), ("decipher", "1")],
        ),
        page("chapter_2026-10-26", "/chapter/2026-10-26"),
        page("chapter_2026-10-27", "/chapter/2026-10-27"),
        page("chapter_2026-10-28", "/chapter/2026-10-28"),
        page("chapter_2026-10-29", "/chapter/2026-10-29"),
        page("chapter_2026-10-30", "/chapter/2026-10-30"),
        page("chapter_2026-10-31", "/chapter/2026-10-31"),
        page("chapter_2026-11-01", "/chapter/2026-11-01"),
        post(
            "chapter_rejoined",
            "/chapter/2026-10-31",
            &[("rejoined", "1")],
        ),
        page("chapter_early", "/chapter/2026-10-25"),
        page("chapter_late", "/chapter/2026-11-02"),
        page("chapter_invalid", "/chapter/2026-02-30"),
        page("chapter_slash", "/chapter/"),
    ];
    cases.push(Case {
        lectio: true,
        ..page("about", "/about")
    });
    cases.push(Case {
        lectio: true,
        ..page("traversal_about", "/a/../about")
    });
    cases.push(Case {
        live_max_age: true,
        ..page("chapter_today", "/chapter")
    });
    cases.push(Case {
        masked_cache: true,
        ..post(
            "ask_bad",
            "/ask",
            &[("pencil", "not json"), ("inked", "[]")],
        )
    });
    cases
}

/// The golden `.head` fixture: `status: N` then sorted lowercased headers.
fn head_lines(status: u16, headers: &[(String, String)]) -> Vec<String> {
    let mut lines: Vec<String> = headers
        .iter()
        .filter(|(name, _)| {
            !matches!(
                name.as_str(),
                "date" | "content-length" | "transfer-encoding" | "connection"
            )
        })
        .map(|(name, value)| format!("{name}: {value}"))
        .collect();
    lines.sort();
    lines.insert(0, format!("status: {status}"));
    lines
}

fn mask_lectio(body: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(body).into_owned();
    let Some(start) = text.find("data-lectio-sign data-number=\"") else {
        return body.to_vec();
    };
    let Some(end) = text[start..].find("</div>") else {
        return body.to_vec();
    };
    format!("{}LECTIO{}", &text[..start], &text[start + end..]).into_bytes()
}

#[test]
fn every_page_matches_the_ruby_original() {
    let server = boot();
    let mut failures = Vec::new();
    for case in cases() {
        let body = if case.form.is_empty() && case.plate.is_none() {
            None
        } else {
            let mut pairs: Vec<String> = Vec::new();
            if let Some(name) = case.plate {
                if !name.is_empty() {
                    let svg = plate(name);
                    pairs.push(format!("svg={}", urlencode(&String::from_utf8_lossy(&svg))));
                }
            }
            for (name, value) in case.form {
                pairs.push(format!("{name}={}", urlencode(value)));
            }
            Some(pairs.join("&"))
        };
        let (status, headers, got_body) = request(
            server.port,
            case.method,
            case.path,
            body.as_deref()
                .map(|b| ("application/x-www-form-urlencoded", b.as_bytes())),
        );

        // Heads
        let golden_head = String::from_utf8(golden(case.name, "head")).expect("utf-8 head");
        let mut want: Vec<String> = golden_head.lines().map(str::to_string).collect();
        let mut got = head_lines(status, &headers);
        let mask = |lines: &mut Vec<String>| {
            lines.retain(|line| !(case.masked_cache && line.starts_with("cache-control:")));
            if case.live_max_age {
                for line in lines.iter_mut() {
                    if line.starts_with("cache-control:") {
                        *line = "cache-control: public, max-age=LIVE".to_string();
                    }
                }
            }
        };
        mask(&mut want);
        mask(&mut got);
        if want != got {
            failures.push(format!(
                "{}: head\n  want {want:?}\n  got {got:?}",
                case.name
            ));
        }

        // Bodies
        let mut want_body = golden(case.name, "body");
        let mut got_full = got_body;
        if case.lectio {
            want_body = mask_lectio(&want_body);
            got_full = mask_lectio(&got_full);
        }
        if want_body != got_full {
            let at = want_body
                .iter()
                .zip(&got_full)
                .position(|(a, b)| a != b)
                .unwrap_or(want_body.len().min(got_full.len()));
            failures.push(format!(
                "{}: body differs at byte {at} (want {} bytes, got {})",
                case.name,
                want_body.len(),
                got_full.len()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} golden case(s) diverged from the Ruby app:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// `/ask` with three right entries answers them, in the sums' order; with
/// two it answers nothing (the Ruby app's own behaviour, from its goldens).
#[test]
fn the_scribe_answers_three_at_a_time() {
    let server = boot();
    let svg = plate("tablet99");
    let pencil_golden = golden("ask_three", "body");
    let pencil_two = golden("ask_two", "body");
    // The right pencil for the fixture plate is baked beside the plates
    let pencil = gunzip(&fixtures().join("plates/tablet99_pencil.json.gz"));
    let body = format!(
        "svg={}&pencil={}&inked=%5B%5D",
        urlencode(&String::from_utf8_lossy(&svg)),
        urlencode(&String::from_utf8_lossy(&pencil))
    );
    let (status, headers, got) = request(
        server.port,
        "POST",
        "/ask",
        Some(("application/x-www-form-urlencoded", body.as_bytes())),
    );
    assert_eq!(status, 200);
    assert!(headers
        .iter()
        .any(|(n, v)| n == "content-type" && v == "application/json"));
    assert_eq!(got, pencil_golden);
    assert_ne!(pencil_golden, pencil_two);
}
