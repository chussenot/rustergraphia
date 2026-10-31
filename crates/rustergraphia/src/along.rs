//! A new plate read along the other layouts, from its sequence: what the
//! decoder would read in its file along each other way, found without
//! drawing the file (as `lacunae::along` does for a weathered plate).
//!
//! - The other way reads the same signs, turned half round where the two
//!   paths disagree on a cell: exactly what reading the file back finds
//!   (`cipher::unambiguous`, kept as the reference the tests hold this to)
//! - Drawing a plate takes nothing from the random, so a plate is drawn
//!   once, when it is kept: drawing every candidate to read it back was two
//!   thirds of inscribing it

use std::collections::HashMap;

use crate::cipher;
use crate::error::Result;
use crate::glyphs;
use crate::layout::{self, Layout};
use crate::Value;

/// True when the sequence reads as letters along its own layout only.
pub fn alone(sequence: &[Value], layout: &Layout) -> Result<bool> {
    for other in layout::others(layout, sequence.len().isqrt()) {
        let reads = reread(sequence, layout, &other).and_then(|read| cipher::decipher(&read, None));
        match reads {
            Ok(_) => return Ok(false),
            Err(error) if error.is_decode() => {}
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

/// A sequence laid along one layout, read along another.
pub fn reread(sequence: &[Value], layout: &Layout, other: &Layout) -> Result<Vec<Value>> {
    let size = sequence.len().isqrt();
    let drawn: HashMap<(i32, i32), (usize, bool)> = layout::path(size, layout)?
        .into_iter()
        .enumerate()
        .map(|(index, (row, col, turned))| ((row, col), (index, turned)))
        .collect();
    layout::path(size, other)?
        .into_iter()
        .map(|(row, col, turned)| {
            let (index, was) = drawn[&(row, col)];
            if was == turned {
                Ok(sequence[index])
            } else {
                glyphs::turned(sequence[index])
            }
        })
        .collect()
}
