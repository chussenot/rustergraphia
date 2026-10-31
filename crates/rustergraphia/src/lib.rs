//! Cistergraphia Core Library (Rust port of the Ruby original)
//! ===========================================================
//!
//! Purpose:
//! - Encrypt a message into Cistercian numerals laid out as a snail (spiral)
//! - Decrypt an SVG produced by this library back into the message
//! - Two modes: the game (a keyless cipher meant to be cracked) and secret
//!   mode (passphrase, AES-256-GCM)
//!
//! This crate mirrors `lib/cistergraphia` of the Ruby implementation module
//! by module, and its files are interoperable: a plate written by either
//! implementation reads in the other. Where the Ruby library draws from a
//! seeded `Random`, this crate replicates Ruby's Mersenne Twister
//! ([`ruby_random`]), so a plate with a tablet of its own, or the day's
//! chapter, is the same plate under both.

pub mod along;
pub mod cipher;
pub mod error;
pub mod fragments;
pub mod glyphs;
pub mod grille;
pub mod lacunae;
pub mod layout;
pub mod notebook;
pub mod pathless;
pub mod ruby_random;
pub mod secret;
pub mod stylus;
pub mod svg;
pub mod tablet;
pub mod tally;
pub mod two_readers;
pub mod weathering;

pub use error::{Error, Result};
pub use ruby_random::RubyRandom;

/// A sign's value: 0..=9999 (`EVIL_MARKER`), the number one glyph carries.
pub type Value = u32;

/// A stroke or hidden line as `[x1, y1, x2, y2]` integer coordinates.
pub type Line = [i32; 4];

/// Never produced by a character (max value is 9000).
pub const EVIL_MARKER: Value = 9999;

/// Mapping of characters to numerical values (the shared tablet), in the
/// Ruby TABLET's order: the letters a..z, then the space.
pub const TABLET: [(char, Value); 27] = [
    ('a', 1),
    ('b', 2),
    ('c', 3),
    ('d', 4),
    ('e', 5),
    ('f', 6),
    ('g', 7),
    ('h', 8),
    ('i', 9),
    ('j', 10),
    ('k', 11),
    ('l', 12),
    ('m', 13),
    ('n', 14),
    ('o', 15),
    ('p', 16),
    ('q', 18),
    ('r', 19),
    ('s', 100),
    ('t', 200),
    ('u', 300),
    ('v', 400),
    ('w', 500),
    ('x', 600),
    ('y', 700),
    ('z', 800),
    (' ', 900),
];

/// Value => character, for decoding: each TABLET value times ten.
pub fn character_of(sum: Value) -> Option<char> {
    TABLET
        .iter()
        .find(|(_, value)| value * 10 == sum)
        .map(|(c, _)| *c)
}

/// Character => value, for enciphering by the shared tablet.
pub fn value_of(c: char) -> Option<Value> {
    TABLET
        .iter()
        .find(|(ch, _)| *ch == c)
        .map(|(_, value)| value * 10)
}

/// Every decodable sum (the shared tablet's values times ten), in TABLET order.
pub fn character_sums() -> impl Iterator<Item = Value> {
    TABLET.iter().map(|(_, value)| value * 10)
}
