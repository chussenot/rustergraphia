//! The result pages, to the byte: what the golden suite cannot replay, as a
//! result page is drawn at random. Here every plate is drawn from
//! `Random.new(seed)` on both sides (`Config::plate_seed`; the Ruby app
//! captured by tools/capture-seeded.rb), and the library being a
//! byte-identical port, the Rust page must be the Ruby page: every layout,
//! a grille under its key, pathless, worn (each way, under a grille,
//! pathless), broken and ruined, large and tiny, escaped.

mod common;

use std::io::Read;
use std::path::PathBuf;

use common::*;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/seeded")
}

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    seed: i64,
    form: serde_json::Map<String, serde_json::Value>,
}

fn ruby_page(name: &str) -> String {
    let file = std::fs::File::open(fixtures().join(format!("{name}.body.gz")))
        .unwrap_or_else(|_| panic!("missing the {name} fixture"));
    let mut page = String::new();
    flate2::read::GzDecoder::new(file)
        .read_to_string(&mut page)
        .expect("a page");
    page
}

/// Where two pages first part, with some context, for the failure message.
fn first_difference(ruby: &str, rust: &str) -> String {
    let at = ruby
        .bytes()
        .zip(rust.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(ruby.len().min(rust.len()));
    let around = |text: &str| {
        let from = text.floor_char_boundary(at.saturating_sub(120));
        let to = text.ceil_char_boundary((at + 120).min(text.len()));
        text[from..to].to_string()
    };
    format!(
        "at byte {at} (lengths {} and {}):\n--- ruby\n{}\n--- rust\n{}",
        ruby.len(),
        rust.len(),
        around(ruby),
        around(rust)
    )
}

#[test]
fn every_seeded_result_page_matches_the_ruby_original() {
    let cases: Vec<Case> = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("cases.json")).expect("the cases"),
    )
    .expect("cases.json");
    assert!(cases.len() >= 20);
    let mut failures = Vec::new();
    for case in &cases {
        let mut s = Session::with(Config {
            plate_seed: Some(case.seed),
            ..Config::default()
        });
        let form: Vec<(String, String)> = case
            .form
            .iter()
            .map(|(name, value)| (name.clone(), value.as_str().expect("a string").to_string()))
            .collect();
        let form: Vec<(&str, &str)> = form.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
        s.post("/generate", &form);
        let ruby = ruby_page(&case.name);
        if s.status() != 200 {
            failures.push(format!("{}: status {}", case.name, s.status()));
        } else if s.body() != ruby {
            failures.push(format!(
                "{}: {}",
                case.name,
                first_difference(&ruby, s.body())
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} pages differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n\n")
    );
}

/// The seed is the whole of a plate's chance: the same seed lays the same
/// page twice, and another seed another plate.
#[test]
fn a_seeded_plate_is_the_same_every_time() {
    let page = |seed| {
        let mut s = Session::with(Config {
            plate_seed: Some(seed),
            ..Config::default()
        });
        s.post(
            "/generate",
            &[("message", "ora et labora"), ("condition", "worn")],
        );
        s.body().to_string()
    };
    assert_eq!(page(3), page(3));
    assert_ne!(page(3), page(4));
}
