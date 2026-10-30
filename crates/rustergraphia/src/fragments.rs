//! Broken plates: a game plate in two to four fragments, shuffled into a
//! tray below an empty board the size of the plate, and the rejoining of
//! them by the pair checksum.
//!
//! INTERIM STUBS: the full port lands with the fragments wave; only the
//! broken-file test is real.

use crate::error::{Error, Result};
use crate::layout::Layout;

pub const CLASS: &str = "fragment";

/// Whether a plate is broken (a string test: intact plates are not parsed
/// twice).
pub fn is_broken(svg_text: &str) -> bool {
    svg_text.contains(r#"class="fragment""#)
}

/// The message of a broken plate, rejoined.
pub fn decode(
    _svg_text: &str,
    _passphrase: Option<&str>,
    _layout: Option<&Layout>,
) -> Result<String> {
    Err(Error::other("Broken plates: port pending"))
}
