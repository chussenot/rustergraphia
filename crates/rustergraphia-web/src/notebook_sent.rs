//! What a notebook sends the scribe (web/reader_game.rb, `notebook_sent`):
//!
//! ```ruby
//! pencil = JSON.parse(params[:pencil].to_s).to_h { |sum, char| [Integer(sum), char.to_s] }
//! [pencil, JSON.parse(params[:inked].to_s).map { |sum| Integer(sum) }]
//! rescue JSON::ParserError, ArgumentError, TypeError, NoMethodError
//! ```
//!
//! read with the Ruby semantics each step has: the json gem's strict parse
//! (a key given twice is an error), `to_h` over a Hash or an Array (whose
//! items destructure into `|sum, char|`), `Integer()` (whitespace, a sign,
//! `0x`/`0b`/`0o`/`0d` and leading-zero octal, underscores between digits;
//! a Float truncated) and `to_s`. A sum no plate can hold (negative, or past
//! a sign's range) is kept out of the maps: it is never an open sum, so the
//! scribe's answer is the same.

use std::collections::BTreeMap;
use std::fmt;

use rustergraphia::Value;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// A parsed JSON value, objects in their written order.
#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool,
    Int(i128),
    /// A number with a fraction or an exponent, as Ruby's Float.
    Float(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Json, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_bool<E>(self, _: bool) -> Result<Json, E> {
        Ok(Json::Bool)
    }

    fn visit_i64<E>(self, n: i64) -> Result<Json, E> {
        Ok(Json::Int(n.into()))
    }

    fn visit_u64<E>(self, n: u64) -> Result<Json, E> {
        Ok(Json::Int(n.into()))
    }

    fn visit_i128<E>(self, n: i128) -> Result<Json, E> {
        Ok(Json::Int(n))
    }

    fn visit_u128<E>(self, n: u128) -> Result<Json, E> {
        Ok(Json::Int(i128::try_from(n).unwrap_or(i128::MAX)))
    }

    fn visit_f64<E>(self, n: f64) -> Result<Json, E> {
        Ok(Json::Float(n))
    }

    fn visit_str<E>(self, s: &str) -> Result<Json, E> {
        Ok(Json::Str(s.to_string()))
    }

    fn visit_string<E>(self, s: String) -> Result<Json, E> {
        Ok(Json::Str(s))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Json::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut pairs: Vec<(String, Json)> = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Json>()? {
            // the json gem refuses a key given twice
            if pairs.iter().any(|(kept, _)| *kept == key) {
                return Err(de::Error::custom("duplicate key"));
            }
            pairs.push((key, value));
        }
        Ok(Json::Object(pairs))
    }
}

/// An integer as Ruby holds it: a sum a plate may hold, or one it never can.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sum {
    Held(Value),
    OutOfReach,
}

fn sum_of(n: i128) -> Sum {
    Value::try_from(n).map_or(Sum::OutOfReach, Sum::Held)
}

/// Ruby's `Integer(value)` for a parsed JSON value; None where Ruby raises
/// (a TypeError or an ArgumentError).
fn integer(value: &Json) -> Option<Sum> {
    match value {
        Json::Int(n) => Some(sum_of(*n)),
        Json::Float(f) if f.is_finite() => Some(if f.trunc().abs() < 1e30 {
            sum_of(f.trunc() as i128)
        } else {
            Sum::OutOfReach
        }),
        Json::Str(text) => integer_text(text),
        _ => None,
    }
}

/// Ruby's `Integer(string)`.
fn integer_text(text: &str) -> Option<Sum> {
    let text = text.trim_matches([' ', '\t', '\n', '\u{b}', '\u{c}', '\r']);
    let (negative, unsigned) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let lowered = unsigned.to_ascii_lowercase();
    let (base, digits) = match lowered.as_bytes() {
        [b'0', b'x', ..] => (16, &unsigned[2..]),
        [b'0', b'b', ..] => (2, &unsigned[2..]),
        [b'0', b'o', ..] => (8, &unsigned[2..]),
        [b'0', b'd', ..] => (10, &unsigned[2..]),
        [b'0', ..] => (8, unsigned),
        _ => (10, unsigned),
    };
    let bytes = digits.as_bytes();
    let well_formed = !bytes.is_empty()
        && bytes[0] != b'_'
        && bytes[bytes.len() - 1] != b'_'
        && !digits.contains("__");
    if !well_formed {
        return None;
    }
    let mut value: u128 = 0;
    let mut overflow = false;
    for c in digits.chars().filter(|&c| c != '_') {
        let digit = c.to_digit(base)?;
        match value
            .checked_mul(u128::from(base))
            .and_then(|v| v.checked_add(u128::from(digit)))
        {
            Some(next) => value = next,
            None => overflow = true,
        }
    }
    if overflow || (negative && value != 0) {
        return Some(Sum::OutOfReach);
    }
    Some(sum_of(value as i128))
}

/// `char.to_s` as the scribe compares it with the tablet's character: only
/// a one-character string can be one; anything else reads as the sentinel,
/// which no tablet holds.
fn character(value: Option<&Json>) -> char {
    const NONE: char = '\0';
    match value {
        Some(Json::Str(text)) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => c,
                _ => NONE,
            }
        }
        _ => NONE,
    }
}

/// `[pencil, inked]`, or None where the Ruby route would say "The notebook
/// could not be read: ask again".
#[allow(clippy::type_complexity)]
pub fn read(pencil: &str, inked: &str) -> Option<(BTreeMap<Value, char>, Vec<Value>)> {
    let pencil_value: Json = serde_json::from_str(pencil).ok()?;
    let mut pencil = BTreeMap::new();
    let mut put = |sum: Sum, c: char| {
        if let Sum::Held(sum) = sum {
            pencil.insert(sum, c); // to_h: the last one written wins
        }
    };
    match &pencil_value {
        Json::Null => {}
        Json::Object(pairs) => {
            for (key, value) in pairs {
                put(integer_text(key)?, character(Some(value)));
            }
        }
        Json::Array(items) => {
            for item in items {
                // a block's |sum, char| destructures an Array item
                let (sum, c) = match item {
                    Json::Array(parts) => (parts.first().unwrap_or(&Json::Null), parts.get(1)),
                    other => (other, None),
                };
                put(integer(sum)?, character(c));
            }
        }
        _ => return None,
    }
    let inked_value: Json = serde_json::from_str(inked).ok()?;
    let inked = match inked_value {
        Json::Array(items) => {
            let mut sums = Vec::new();
            for item in &items {
                if let Sum::Held(sum) = integer(item)? {
                    sums.push(sum);
                }
            }
            sums
        }
        // Hash#map yields [key, value] pairs, which Integer() refuses
        Json::Object(pairs) if pairs.is_empty() => Vec::new(),
        _ => return None,
    };
    Some((pencil, inked))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ruby 3.3's `Integer(string)` on each (`nil`: it raises).
    #[test]
    fn integer_reads_as_ruby_does() {
        let cases: &[(&str, Option<i64>)] = &[
            ("10", Some(10)),
            (" 10 ", Some(10)),
            ("\t10\n", Some(10)),
            ("+10", Some(10)),
            ("-10", Some(-10)),
            ("010", Some(8)),
            ("0x1A", Some(26)),
            ("0X1a", Some(26)),
            ("0b101", Some(5)),
            ("0o17", Some(15)),
            ("0O17", Some(15)),
            ("0d19", Some(19)),
            ("0_1", Some(1)),
            ("1_000", Some(1000)),
            ("1__0", None),
            ("_1", None),
            ("1_", None),
            ("0x", None),
            ("0x_1", None),
            ("08", None),
            ("0", Some(0)),
            ("00", Some(0)),
            ("", None),
            (" ", None),
            ("1e3", None),
            ("1.0", None),
            ("10a", None),
            ("abc", None),
            ("-0", Some(0)),
            ("0b", None),
            ("1 0", None),
            ("\u{a0}10", None),
            ("１０", None),
            ("0x1_f", Some(31)),
            ("0o", None),
            ("0_", None),
            ("+", None),
            ("- 1", None),
            ("+-1", None),
            ("0d1_0", Some(10)),
            ("0B", None),
            ("\u{b}10\u{c}", Some(10)),
            ("0_7", Some(7)),
            ("0__7", None),
            ("07_7", Some(63)),
            ("0o_7", None),
            (" +0x10 ", Some(16)),
            ("-0x10", Some(-16)),
            ("1\u{0}", None),
        ];
        for (text, ruby) in cases {
            let expected = ruby.map(|n| sum_of(n.into()));
            assert_eq!(expected, integer_text(text), "{text:?}");
        }
        assert_eq!(
            Some(Sum::OutOfReach),
            integer_text("99999999999999999999999")
        );
        assert_eq!(Some(Sum::Held(1)), integer(&Json::Float(1.5)));
        assert_eq!(Some(Sum::OutOfReach), integer(&Json::Float(-1.5)));
        assert_eq!(
            Some(Sum::Held(4294967295)),
            integer(&Json::Float(4294967295.9))
        );
        assert_eq!(None, integer(&Json::Null));
        assert_eq!(None, integer(&Json::Bool));
    }

    #[test]
    fn the_notebook_is_read_as_the_ruby_route_reads_it() {
        let read_pencil = |pencil: &str| read(pencil, "[]").map(|(pencil, _)| pencil);
        let one = |sum: Value, c: char| Some(BTreeMap::from([(sum, c)]));
        assert_eq!(one(10, 'a'), read_pencil(r#"{"10":"a"}"#));
        assert_eq!(
            one(10, '\0'),
            read_pencil(r#"{"10":"ab"}"#),
            "two letters are no letter"
        );
        assert_eq!(
            one(10, '\0'),
            read_pencil(r#"{"10":null}"#),
            "nil.to_s is empty"
        );
        assert_eq!(one(8, 'a'), read_pencil(r#"{"010":"a"}"#), "octal");
        assert_eq!(
            one(8, 'b'),
            read_pencil(r#"{"010":"a","8":"b"}"#),
            "the last wins"
        );
        assert_eq!(None, read_pencil(r#"{"10":"a","10":"b"}"#), "a key twice");
        assert_eq!(None, read_pencil(r#"{"x":"a"}"#));
        assert_eq!(Some(BTreeMap::new()), read_pencil(r#"{"-10":"a"}"#));
        assert_eq!(Some(BTreeMap::new()), read_pencil("null"));
        assert_eq!(Some(BTreeMap::new()), read_pencil("[]"));
        assert_eq!(one(10, 'a'), read_pencil(r#"[[10,"a"]]"#), "pairs");
        assert_eq!(one(10, '\0'), read_pencil("[10]"), "a lone sum");
        assert_eq!(one(10, '\0'), read_pencil(r#"["10"]"#));
        assert_eq!(one(1, 'a'), read_pencil(r#"[[1.5,"a",3]]"#));
        assert_eq!(None, read_pencil("[[]]"), "Integer(nil)");
        assert_eq!(None, read_pencil("[null]"));
        assert_eq!(None, read_pencil("[true]"));
        assert_eq!(None, read_pencil(r#""x""#));
        assert_eq!(None, read_pencil("5"));
        assert_eq!(None, read_pencil(""));
        assert_eq!(None, read_pencil("not json"));

        let read_inked = |inked: &str| read("{}", inked).map(|(_, inked)| inked);
        assert_eq!(Some(vec![10, 20]), read_inked(r#"[10,"20"]"#));
        assert_eq!(Some(vec![1]), read_inked("[1.9]"));
        assert_eq!(Some(vec![]), read_inked("[-1]"));
        assert_eq!(Some(vec![]), read_inked("{}"));
        assert_eq!(None, read_inked(r#"{"10":1}"#));
        assert_eq!(None, read_inked("null"));
        assert_eq!(None, read_inked("[null]"));
        assert_eq!(None, read_inked(r#"["x"]"#));
        assert_eq!(None, read_inked("10"));
    }
}
