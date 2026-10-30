//! Tally signs: sums on a higher cord, as on an Inka khipu.
//!
//! INTERIM STUBS: the full port lands with the weathering wave; only the
//! constants the SVG reader needs are real.

use crate::error::{Error, Result};
use crate::layout::Layout;
use crate::Value;

pub const CLASS: &str = "tally-lines";
pub const RUBRIC: &str = "#a3301c"; // the tally signs and their cords, as a scribe's rubric
pub const NOT_UNDER_A_GRILLE: &str = "Tally signs are not drawn under a grille yet";
pub const NOT_SEALED: &str = "Tally signs are for game plates: a sealed plate never loses a stroke";

/// The tally signs in a plate's SVG: drawn, hidden, read back.
pub mod plate {
    use super::*;

    /// The tally signs of a plate in its SVG.
    pub fn layer(_sequence: &[Value], _cols: usize, _layout: &Layout) -> Result<String> {
        Err(Error::other("Tally signs: port pending"))
    }
}
