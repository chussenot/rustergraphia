//! A pathless plate says nothing of where it begins: the reader finds the
//! way. Its file carries one class on the root, and its signs lie in grid
//! order, each wedge seeded by its cell (and so do a worn plate's patches and
//! lost positions: `lacunae`), so that neither the order of the elements nor
//! the hand's tremor tells which of the five ways it runs. The decoder reads
//! it as any other plate: it tries every way.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{Error, Result};
use crate::glyphs;
use crate::layout::{self, Layout};
use crate::svg;
use crate::{Line, Value};

pub const CLASS: &str = "pathless";
pub const NOT_SEALED: &str =
    "A sealed plate cannot be pathless: it reads as no letters along any way";
pub const NOT_UNDER_A_GRILLE: &str = "A grille plate hides its way by its key already";
pub const NOT_BROKEN: &str =
    "A broken plate cannot be pathless: the board rejoins its fragments along the snail";

/// Whether a plate was written pathless (a string test: the class on the
/// root).
pub fn is_pathless(svg: &str) -> bool {
    svg.contains(r#"class="pathless""#)
}

/// The seed of the hand at a cell: its place in grid order, row by row.
pub fn seed(row: i32, col: i32, cols: usize) -> i64 {
    (row as i64 * cols as i64) + col as i64
}

// `by_cell` (the signs in grid order, each seeded by its cell) lives inside
// `layout::placed`, which reorders its own `Placed` signs.

/// What one cell of a pathless plate reads, upright and turned half round
/// (`None` where the strokes read as no sign that way, and both `None` where
/// a lacuna took part of the sign: a worn cell reads as nothing whole).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellReading {
    pub row: i32,
    pub col: i32,
    pub upright: Option<Value>,
    pub turned: Option<Value>,
    pub worn: bool,
}

/// What each cell of a plate reads, upright and turned half round, in grid
/// order. What a desk or a bench needs to lay any of the five ways over it.
pub fn grid(svg_text: &str) -> Result<Vec<CellReading>> {
    let mut cells: BTreeMap<(i32, i32), Vec<Line>> = BTreeMap::new();
    for line in svg::lines(svg_text, svg::Layer::Strokes)? {
        cells
            .entry(layout::cell_of(line, &Layout::Phaistos))
            .or_default()
            .push(line);
    }
    let worn: BTreeSet<(i32, i32)> = svg::lines(svg_text, svg::Layer::Lacunae)?
        .into_iter()
        .map(|line| layout::cell_of(line, &Layout::Phaistos))
        .collect();
    Ok(cells
        .into_iter()
        .map(|((row, col), strokes)| {
            let is_worn = worn.contains(&(row, col));
            let (upright, turned) = if is_worn {
                (None, None)
            } else {
                (
                    reading(&strokes, row, col, false),
                    reading(&strokes, row, col, true),
                )
            };
            CellReading {
                row,
                col,
                upright,
                turned,
                worn: is_worn,
            }
        })
        .collect())
}

fn reading(strokes: &[Line], row: i32, col: i32, turned: bool) -> Option<Value> {
    glyphs::read(&layout::upright(
        strokes,
        row,
        col,
        turned,
        &Layout::Phaistos,
    ))
    .ok()
}

/// The plates that cannot be pathless, in the library's words.
/// (`under_grille` stands in for Ruby's `layout.is_a?(Grille)`.)
pub fn check(passphrase: Option<&str>, under_grille: bool) -> Result<()> {
    if passphrase.is_some() {
        return Err(Error::other(NOT_SEALED));
    }
    if under_grille {
        return Err(Error::other(NOT_UNDER_A_GRILLE));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_class_is_a_string_test() {
        assert!(is_pathless(r#"<svg class="pathless"></svg>"#));
        assert!(!is_pathless("<svg></svg>"));
    }

    #[test]
    fn seeds_run_in_grid_order() {
        assert_eq!(seed(0, 0, 4), 0);
        assert_eq!(seed(1, 2, 4), 6);
        assert_eq!(seed(3, 3, 4), 15);
    }

    #[test]
    fn refusals_speak_the_library_words() {
        assert_eq!(
            check(Some("pass"), false).unwrap_err().to_string(),
            NOT_SEALED
        );
        assert_eq!(
            check(None, true).unwrap_err().to_string(),
            NOT_UNDER_A_GRILLE
        );
        assert!(check(None, false).is_ok());
    }
}
