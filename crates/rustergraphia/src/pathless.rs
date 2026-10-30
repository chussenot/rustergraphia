//! A pathless plate says nothing of where it begins: the reader finds the
//! way. Its file carries one class on the root, and its signs lie in grid
//! order, each wedge seeded by its cell (and so do a worn plate's patches and
//! lost positions: `lacunae`), so that neither the order of the elements nor
//! the hand's tremor tells which of the five ways it runs. The decoder reads
//! it as any other plate: it tries every way.

use crate::error::{Error, Result};

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

// `by_cell` (the signs in grid order, each seeded by its cell) and `grid`
// (what each cell reads, upright and turned) land with the svg/layout wave:
// they reorder `layout::Placed` signs and read `svg::lines`, which are being
// ported in parallel.

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
