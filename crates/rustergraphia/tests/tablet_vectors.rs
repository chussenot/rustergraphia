//! A plate's own tablet against the tablets Ruby 3.3 deals for the same
//! seeds (tests/fixtures/ruby_random_vectors.json): the Fisher-Yates from
//! the SHA-256-seeded Random must match bit for bit, or a plate written by
//! one implementation would read as gibberish in the other.

use rustergraphia::tablet;
use serde_json::Value;

fn vectors() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ruby_random_vectors.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn tablets_match_ruby() {
    let v = vectors();
    for seed in ["ABCDEFGH23", "K7Q2MZXB4P"] {
        let want = v[format!("tablet_{seed}")]
            .as_object()
            .unwrap_or_else(|| panic!("missing tablet_{seed}"));
        let got = tablet::draw(seed).unwrap();
        assert_eq!(got.len(), want.len(), "seed {seed}");
        for (c, value) in got {
            let key = c.to_string();
            assert_eq!(
                want[&key].as_u64().unwrap(),
                u64::from(value),
                "seed {seed}, character {c:?}"
            );
        }
    }
}

#[test]
fn the_tablet_still_reads_by_sums() {
    // The values are the shared tablet's, only permuted: every drawn value is
    // one of the 27 sums, and the inverse lookup reads them back
    let characters = tablet::characters("K7Q2MZXB4P").unwrap();
    for (c, value) in tablet::rows("K7Q2MZXB4P").unwrap() {
        assert_eq!(characters[&value], c);
    }
}
