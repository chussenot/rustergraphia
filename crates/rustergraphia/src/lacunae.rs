//! The lacunae of a weathered plate, in its SVG: drawn, hidden, read back.
//!
//! INTERIM STUBS: the full port lands with the weathering wave. What is
//! already real here is what intact plates need: `strokes` (a sign's strokes
//! without those a lacuna took) and the weathered-file test.

use crate::error::{Error, Result};
use crate::glyphs;
use crate::layout::Layout;
use crate::svg::{self, Damage};
use crate::{Line, Value};

pub const CLASS: &str = "lacunae";
pub const SEALED: &str = "A worn plate is a game plate: read it without a passphrase";
pub const NOT_SEALED: &str =
    "A sealed plate cannot wear: with one stroke lost, it would never open again";
pub const NOT_TWO_READERS: &str = "A plate for two readers cannot wear yet";

/// Whether a plate has lacunae (a string test first: intact plates are not
/// parsed twice).
pub fn is_weathered(svg_text: &str) -> Result<bool> {
    if !svg_text.contains(r#"class="lacunae""#) {
        return Ok(false);
    }
    Ok(!svg::lines(svg_text, svg::Layer::Lacunae)?.is_empty())
}

/// A sign's strokes, without those a lacuna took.
pub fn strokes(num: Value, lost: &[Line]) -> Vec<Line> {
    glyphs::strokes(num, 0, 0)
        .into_iter()
        .filter(|stroke| !lost.contains(&glyphs::canonical(*stroke)))
        .collect()
}

/// The worn patches, drawn under the wedges ('' for an intact plate).
pub fn patches(_cols: usize, _layout: &Layout, damage: &Damage, _pathless: bool) -> Result<String> {
    if damage.is_empty() {
        return Ok(String::new());
    }
    Err(Error::other("Weathered plates: port pending"))
}

/// The lost stroke positions as hidden lines: what the decoder reads.
pub fn hidden(_cols: usize, _layout: &Layout, damage: &Damage, _pathless: bool) -> Result<String> {
    if damage.is_empty() {
        return Ok(String::new());
    }
    Err(Error::other("Weathered plates: port pending"))
}

/// The restored text of a weathered plate, in the Leiden conventions.
pub fn decode(
    _svg_text: &str,
    _passphrase: Option<&str>,
    _layout: Option<&Layout>,
) -> Result<String> {
    Err(Error::other("Weathered plates: port pending"))
}
