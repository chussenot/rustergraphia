//! Where a plate's signs go: the grid, and the order in which it is read.
//!
//! A layout is Phaistos (the snail, clockwise from the top-left corner to the
//! centre), Rongorongo (reverse boustrophedon: rows from the bottom, every
//! other one turned half round), the Leap (the Zodiac's step: one row down,
//! two columns across, wrapping), the Furrows (boustrophedon by columns), the
//! Unwinding (from the centre outward, counter-clockwise) or a Grille (rows
//! read through a turning grille, with its key). Under a grille the cells are
//! square, so that the parchment turned by a quarter lands on the grid again,
//! and each sign is centred in its cell: only a grille's plate is square.

use crate::error::{Error, Result};
use crate::grille::Grille;
use crate::{glyphs, Line, Value};

pub const SPACE: i32 = 20;
pub const PADDING: i32 = 50; // Padding around the content
pub const CELL_WIDTH: i32 = glyphs::WIDTH + SPACE;
pub const CELL_HEIGHT: i32 = glyphs::HEIGHT + SPACE;
pub const SQUARE_CELL: i32 = CELL_HEIGHT;
pub const SQUARE_INSET: (i32, i32) = (
    (SQUARE_CELL - glyphs::WIDTH) / 2,
    (SQUARE_CELL - glyphs::HEIGHT) / 2,
);

/// A reading path: the five keyless ways, or a turning grille with its key.
#[derive(Debug, Clone, PartialEq)]
pub enum Layout {
    Phaistos,
    Rongorongo,
    Leap,
    Furrows,
    Unwinding,
    Grille(Grille),
}

/// Read without a key: the decoder tries them all.
pub const KEYLESS: [Layout; 5] = [
    Layout::Phaistos,
    Layout::Rongorongo,
    Layout::Leap,
    Layout::Furrows,
    Layout::Unwinding,
];

impl Layout {
    /// The layout's name, as the Ruby symbols spell it.
    pub fn name(&self) -> &'static str {
        match self {
            Layout::Phaistos => "phaistos",
            Layout::Rongorongo => "rongorongo",
            Layout::Leap => "leap",
            Layout::Furrows => "furrows",
            Layout::Unwinding => "unwinding",
            Layout::Grille(_) => "grille",
        }
    }

    /// The keyless layout of this name, if there is one.
    pub fn keyless(name: &str) -> Option<Layout> {
        KEYLESS.into_iter().find(|layout| layout.name() == name)
    }

    pub fn is_grille(&self) -> bool {
        matches!(self, Layout::Grille(_))
    }

    pub fn as_grille(&self) -> Option<&Grille> {
        match self {
            Layout::Grille(grille) => Some(grille),
            _ => None,
        }
    }
}

/// A sign placed on the grid: its cell, its value, its place in the reading
/// order, and the seed of the hand that presses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub row: i32,
    pub col: i32,
    pub turned: bool,
    pub num: Value,
    pub index: usize,
    pub seed: i64,
}

/// `(width, height, cols, rows)` of the plate for a sequence: a square grid,
/// one cell wider and taller with tally signs.
pub fn dimensions(len: usize, layout: &Layout, tally: bool) -> Result<(i32, i32, usize, usize)> {
    let cols = len.isqrt();
    if cols == 0 {
        return Err(Error::EmptyMessage(
            "Cannot lay out an empty sequence".into(),
        ));
    }
    let rows = len.div_ceil(cols);
    let (width, height) = cell(layout);
    let extra = if tally { 1 } else { 0 };
    Ok((
        width * (cols as i32 + extra) + PADDING * 2,
        height * (rows as i32 + extra) + PADDING * 2,
        cols,
        rows,
    ))
}

/// `(width, height)` of a grid cell.
pub fn cell(layout: &Layout) -> (i32, i32) {
    if layout.is_grille() {
        (SQUARE_CELL, SQUARE_CELL)
    } else {
        (CELL_WIDTH, CELL_HEIGHT)
    }
}

/// Where the sign sits in its cell.
pub fn inset(layout: &Layout) -> (i32, i32) {
    if layout.is_grille() {
        SQUARE_INSET
    } else {
        (0, 0)
    }
}

/// `(row, col, turned)` of every sign, in reading order.
pub fn path(size: usize, layout: &Layout) -> Result<Vec<(i32, i32, bool)>> {
    match layout {
        Layout::Grille(grille) => Ok(grille
            .path(size)?
            .into_iter()
            .map(|(row, col)| (row, col, false))
            .collect()),
        Layout::Phaistos => Ok(phaistos(size)),
        Layout::Rongorongo => Ok(rongorongo(size)),
        Layout::Leap => Ok(leap(size)),
        Layout::Furrows => Ok(furrows(size)),
        Layout::Unwinding => Ok(unwinding(size)),
    }
}

/// The keyless layouts that can read a plate of this side as another message:
/// those that form other pairs of signs. Two layouts that pair the same cells
/// in the same order (within a pair, order does not matter: the values add)
/// read any plate alike, so one is not an alternative to the other. At a side
/// of 2 the leap, the furrows and the unwinding are one such walk.
pub fn others(layout: &Layout, size: usize) -> Vec<Layout> {
    // Only ever asked of a keyless layout (a grille plate is never ambiguous);
    // a grille's own pairs would need its even-sided path.
    let own = match pairs(size, layout) {
        Ok(own) => own,
        Err(_) => return KEYLESS.to_vec(),
    };
    KEYLESS
        .into_iter()
        .filter(|other| other != layout)
        .filter(|other| {
            pairs(size, other)
                .map(|theirs| theirs != own)
                .unwrap_or(true)
        })
        .collect()
}

/// Every sign of a sequence placed along a layout, in reading order, the seed
/// its place in it (SVG's hand); on a pathless plate, in grid order and
/// seeded by the cell (Pathless.by_cell).
pub fn placed(
    sequence: &[Value],
    cols: usize,
    layout: &Layout,
    pathless: bool,
) -> Result<Vec<Placed>> {
    let mut signs: Vec<Placed> = path(cols, layout)?
        .into_iter()
        .zip(sequence.iter())
        .enumerate()
        .map(|(index, ((row, col, turned), &num))| Placed {
            row,
            col,
            turned,
            num,
            index,
            seed: index as i64,
        })
        .collect();
    if pathless {
        // The signs in grid order, row by row, each seeded by its cell: the
        // file's order and the hand's tremor tell nothing of the way
        signs.sort_by_key(|sign| (sign.row, sign.col));
        for sign in &mut signs {
            sign.seed = i64::from(sign.row) * cols as i64 + i64::from(sign.col);
        }
    }
    Ok(signs)
}

/// The pairs a layout forms, each as its two `(row, col, turned)` sorted
/// (two cells of a pair never coincide, so (row, col) decides the order).
pub fn pairs(size: usize, layout: &Layout) -> Result<Vec<Vec<(i32, i32, bool)>>> {
    Ok(path(size, layout)?
        .chunks(2)
        .map(|pair| {
            let mut pair = pair.to_vec();
            pair.sort_by_key(|&(row, col, _)| (row, col));
            pair
        })
        .collect())
}

/// The snail: clockwise from the top-left corner to the centre.
pub fn phaistos(size: usize) -> Vec<(i32, i32, bool)> {
    spiral_coordinates(size)
        .into_iter()
        .map(|(row, col)| (row, col, false))
        .collect()
}

/// The turning tablet: rows from the bottom, every other one turned half round.
pub fn rongorongo(size: usize) -> Vec<(i32, i32, bool)> {
    boustrophedon_coordinates(size)
}

/// The leap, the Zodiac's step: one row down and two columns across at every
/// sign, wrapping at the edge; when the orbit closes (after `size` signs) the
/// next starts one column to the right.
pub fn leap(size: usize) -> Vec<(i32, i32, bool)> {
    let n = size as i32;
    (0..n)
        .flat_map(|start| (0..n).map(move |i| (i, (start + 2 * i) % n, false)))
        .collect()
}

/// The furrows: boustrophedon by columns, as the ox turns at the end of the
/// field. Down the first column, up the second, from the left; no sign turned.
pub fn furrows(size: usize) -> Vec<(i32, i32, bool)> {
    let n = size as i32;
    (0..n)
        .flat_map(|col| {
            let rows: Vec<i32> = if col % 2 == 0 {
                (0..n).collect()
            } else {
                (0..n).rev().collect()
            };
            rows.into_iter().map(move |row| (row, col, false))
        })
        .collect()
}

/// The unwinding: from the centre outward, counter-clockwise, to the top-right
/// corner. The snail turned a quarter and read backwards: the snail itself
/// read backwards would pair the same signs, and read the whole message backwards.
pub fn unwinding(size: usize) -> Vec<(i32, i32, bool)> {
    let mut path: Vec<(i32, i32, bool)> = spiral_coordinates(size)
        .into_iter()
        .map(|(row, col)| (col, size as i32 - 1 - row, false))
        .collect();
    path.reverse();
    path
}

/// Reads an array of rows in a snail pattern (clockwise spiral from the
/// top-left): the first row, then the rest transposed and reversed, again.
fn snail(matrix: Vec<Vec<(i32, i32)>>) -> Vec<(i32, i32)> {
    if matrix.is_empty() {
        return Vec::new();
    }
    let mut rows = matrix.into_iter();
    let mut out = rows.next().unwrap();
    let rest: Vec<Vec<(i32, i32)>> = rows.collect();
    if rest.is_empty() || rest[0].is_empty() {
        return out;
    }
    // transpose then reverse the remaining rows
    let cols = rest[0].len();
    let mut turned: Vec<Vec<(i32, i32)>> = (0..cols)
        .map(|c| rest.iter().map(|row| row[c]).collect())
        .collect();
    turned.reverse();
    out.extend(snail(turned));
    out
}

/// `(row, col)` of each grid cell, in snail order.
pub fn spiral_coordinates(size: usize) -> Vec<(i32, i32)> {
    let n = size as i32;
    let matrix: Vec<Vec<(i32, i32)>> = (0..n)
        .map(|row| (0..n).map(|col| (row, col)).collect())
        .collect();
    snail(matrix)
}

/// `(row, col, turned)` of each grid cell in reverse boustrophedon order, the
/// rongorongo way: rows from the bottom, left to right, then the tablet turns
/// half round, so every other row runs back with its signs upside down.
pub fn boustrophedon_coordinates(size: usize) -> Vec<(i32, i32, bool)> {
    let n = size as i32;
    (0..n)
        .rev()
        .enumerate()
        .flat_map(|(k, row)| {
            let cols: Vec<i32> = if k % 2 == 0 {
                (0..n).collect()
            } else {
                (0..n).rev().collect()
            };
            cols.into_iter().map(move |col| (row, col, k % 2 == 1))
        })
        .collect()
}

/// Top-left corner of a grid cell.
pub fn cell_origin(row: i32, col: i32, layout: &Layout) -> (i32, i32) {
    let (width, height) = cell(layout);
    (PADDING + width * col, PADDING + height * row)
}

/// Top-left corner of the sign in a grid cell.
pub fn origin(row: i32, col: i32, layout: &Layout) -> (i32, i32) {
    let (x, y) = cell_origin(row, col, layout);
    let (dx, dy) = inset(layout);
    (x + dx, y + dy)
}

/// A line moved by (x, y): the plate's strokes made relative to a cell, or
/// absolute again.
pub fn relative([x1, y1, x2, y2]: Line, x: i32, y: i32) -> Line {
    [x1 - x, y1 - y, x2 - x, y2 - y]
}

/// A cell's lines made relative to its sign and turned back upright.
pub fn upright(lines: &[Line], row: i32, col: i32, turned: bool, layout: &Layout) -> Vec<Line> {
    let (x, y) = origin(row, col, layout);
    lines
        .iter()
        .map(|&line| {
            let local = relative(line, x, y);
            if turned {
                glyphs::turn(local)
            } else {
                local
            }
        })
        .collect()
}

/// Grid `(row, col)` a line belongs to, from its top-left-most point
/// (Ruby's `div` floors, so a crafted line left of the padding goes negative).
pub fn cell_of(line: Line, layout: &Layout) -> (i32, i32) {
    let [x1, y1, x2, y2] = line;
    let (width, height) = cell(layout);
    (
        (y1.min(y2) - PADDING).div_euclid(height),
        (x1.min(x2) - PADDING).div_euclid(width),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_keyless_path_visits_every_cell_once() {
        for layout in KEYLESS {
            for size in 1..=8 {
                let mut path = path(size, &layout).unwrap();
                assert_eq!(path.len(), size * size, "{} at {size}", layout.name());
                path.sort_by_key(|&(row, col, _)| (row, col));
                path.dedup_by_key(|&mut (row, col, _)| (row, col));
                assert_eq!(path.len(), size * size, "{} at {size}", layout.name());
            }
        }
    }

    #[test]
    fn the_snail_starts_at_the_corner_and_ends_at_the_centre() {
        let path = phaistos(3);
        assert_eq!(path.first(), Some(&(0, 0, false)));
        assert_eq!(path.last(), Some(&(1, 1, false)));
        assert_eq!(
            path.iter().map(|&(r, c, _)| (r, c)).collect::<Vec<_>>(),
            [
                (0, 0),
                (0, 1),
                (0, 2),
                (1, 2),
                (2, 2),
                (2, 1),
                (2, 0),
                (1, 0),
                (1, 1)
            ]
        );
    }

    #[test]
    fn rongorongo_turns_every_other_row() {
        let path = rongorongo(2);
        assert_eq!(
            path,
            [(1, 0, false), (1, 1, false), (0, 1, true), (0, 0, true)]
        );
    }

    #[test]
    fn the_unwinding_is_the_snail_turned_and_reversed() {
        let path = unwinding(2);
        assert_eq!(
            path,
            [(0, 0, false), (1, 0, false), (1, 1, false), (0, 1, false)]
        );
    }

    #[test]
    fn at_a_side_of_two_three_ways_are_one_walk() {
        let alternatives = others(&Layout::Leap, 2);
        assert!(!alternatives.contains(&Layout::Furrows));
        assert!(!alternatives.contains(&Layout::Unwinding));
        assert!(alternatives.contains(&Layout::Phaistos));
        let alternatives = others(&Layout::Phaistos, 4);
        assert_eq!(alternatives.len(), 4);
    }

    #[test]
    fn dimensions_refuse_an_empty_sequence() {
        assert!(matches!(
            dimensions(0, &Layout::Phaistos, false),
            Err(Error::EmptyMessage(_))
        ));
        assert_eq!(
            dimensions(4, &Layout::Phaistos, false).unwrap(),
            (340, 440, 2, 2)
        );
        assert_eq!(
            dimensions(4, &Layout::Phaistos, true).unwrap(),
            (460, 610, 2, 2)
        );
    }

    #[test]
    fn cells_floor_toward_the_origin() {
        assert_eq!(cell_of([50, 50, 150, 50], &Layout::Phaistos), (0, 0));
        assert_eq!(cell_of([10, 10, 20, 20], &Layout::Phaistos), (-1, -1));
    }
}
