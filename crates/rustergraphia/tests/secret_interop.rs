//! Sealed plates across implementations: sequences sealed by the Ruby
//! original (tests/fixtures/secret_interop.json, written by
//! Cistergraphia::Secret.encrypt under Ruby 3.3 / OpenSSL) must open here
//! with the exact message, and tampering or the wrong passphrase must be
//! refused. The reverse direction (Rust seals, Ruby opens) was checked when
//! the fixtures were made; the formats are symmetric.

use rustergraphia::error::Error;
use rustergraphia::{secret, Value};
use serde_json::Value as Json;

fn cases() -> Vec<(String, String, bool, Vec<Value>)> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/secret_interop.json"
    );
    let json: Json = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    json.as_array()
        .unwrap()
        .iter()
        .map(|case| {
            (
                case["message"].as_str().unwrap().to_string(),
                case["passphrase"].as_str().unwrap().to_string(),
                case["even"].as_bool().unwrap(),
                case["sequence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_u64().unwrap() as Value)
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn ruby_sealed_plates_open_here() {
    for (message, passphrase, even, sequence) in cases() {
        assert_eq!(
            secret::decrypt(&sequence, &passphrase).unwrap(),
            message,
            "message {message:?}"
        );
        let side = sequence.len().isqrt();
        assert_eq!(side * side, sequence.len());
        if even {
            assert_eq!(side % 2, 0);
        }
    }
}

#[test]
fn the_wrong_passphrase_is_refused_on_ruby_plates() {
    for (_, _, _, sequence) in cases() {
        assert!(matches!(
            secret::decrypt(&sequence, "wrong passphrase entirely"),
            Err(Error::WrongPassphrase(_))
        ));
    }
}

#[test]
fn tampering_with_a_ruby_plate_is_detected() {
    let (_, passphrase, _, mut sequence) = cases().remove(0);
    sequence[3] = (sequence[3] + 1) % 10_000;
    assert!(secret::decrypt(&sequence, &passphrase).is_err());
}

#[test]
fn max_sealed_size_matches_ruby() {
    // Reference values from the Ruby original (tmp/gen_secret.rb)
    assert_eq!(secret::max_sealed_size("a"), 30);
    assert_eq!(secret::max_sealed_size("Les reliques sont à Cîteaux"), 47);
    assert_eq!(secret::max_sealed_size(&"x".repeat(500)), 331);
}
