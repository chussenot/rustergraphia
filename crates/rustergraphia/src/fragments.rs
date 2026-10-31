//! Broken plates: a game plate in two to four fragments, shuffled into a
//! tray below an empty board the size of the plate, and the rejoining of
//! them by the pair checksum.
//!
//! - Breaks follow the grid, straight across a fragment (each break splits
//!   one fragment in two), and every fragment is at least MIN_SIDE signs
//!   each way. Straight breaks let several fragments fit the same hole, so
//!   the shapes alone do not rejoin a plate: the sums do, as the sums of a
//!   khipu rejoined two of its fragments
//! - Every edge is worn, each fragment on its own: they never fit like a
//!   jigsaw
//! - A fragment's signs are written in its own frame, already turned if it
//!   lies turned in the tray; its place in the tray (a translation and a
//!   slight tilt) is its only transform. The file holds neither where a
//!   fragment belongs nor whether it is turned
//! - A fragment turns half round only: a quarter turn lays the stems flat,
//!   which no reader would take for upright
//! - A plate is broken only if exactly one arrangement rejoins it: the
//!   decoder's own search is run on every new plate
//! - A weathered fragment's signs are read as their candidate values (what
//!   its strokes allow, the lost positions anything), upright and turned; an
//!   arrangement holds when every pair can balance
//! - A fragment's own frame is the plate's, as if it lay at the top-left
//!   corner: its signs sit where a plate's would, centred in cell boxes that
//!   start at BOARD. Turned half round, it lands on the same boxes; moved by
//!   whole cells onto the board, it shows the plate exactly as it was drawn
//!
//! Constraints: game plates only, and along the snail only (the snail
//! crosses every break, so pairs straddle every join); no tally signs; at
//! most MAX_PIECES fragments; the search is bounded for crafted files too
//! (MAX_CANDIDATES, MAX_WEIGHINGS).

use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::glyphs;
use crate::lacunae;
use crate::layout::{self, Layout};
use crate::ruby_random::RubyRandom;
use crate::svg::{self, Damage};
use crate::weathering;
use crate::{cipher, Line, Value};

pub const CLASS: &str = "fragment";
pub const EDGE: i32 = layout::SPACE / 2; // a fragment's margin around its signs: half the gap between two
pub const BOARD: i32 = layout::PADDING - EDGE; // where the board starts, and a fragment's box in its own frame
pub const MIN_SIDE: i32 = 2;
pub const MAX_PIECES: usize = 4;
pub const MAX_CANDIDATES: usize = 10; // a lacuna takes at most one corner of a sign: ten digits it may have held
pub const MAX_WEIGHINGS: i64 = 20_000; // pairs weighed in all; honest plates need under 3,000
pub const SEALED: &str = "A broken plate is a game plate: read it without a passphrase";
pub const NOT_SEALED: &str =
    "A sealed plate cannot be broken: its signs carry no sums to rejoin it by";
pub const NOT_UNDER_A_GRILLE: &str = "A plate under a grille cannot be broken yet";
pub const NOT_ALONG_THE_ROWS: &str =
    "Only a Phaistos plate can be broken: the board rejoins the fragments along the snail";
pub const NOT_TALLIED: &str = "A broken plate cannot carry tally signs yet";
pub const TOO_SMALL: &str =
    "Too short a message to break: a broken plate needs at least 4 × 4 signs";

/// A fragment where it belongs on the plate's grid, and whether it lies
/// turned in the tray.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub row: i32,
    pub col: i32,
    pub rows: i32,
    pub cols: i32,
    pub turned: bool,
}

impl Piece {
    /// `[row, col]` of every cell, row by row.
    pub fn cells(&self) -> Vec<(i32, i32)> {
        (0..self.rows)
            .flat_map(|row| (0..self.cols).map(move |col| (row, col)))
            .collect()
    }

    /// Where a cell lands once the fragment is turned half round.
    pub fn opposite(&self, row: i32, col: i32) -> (i32, i32) {
        (self.rows - 1 - row, self.cols - 1 - col)
    }

    /// The two pieces it breaks into, across its rows or its columns
    /// (`first`: the first piece's side).
    pub fn split(&self, axis: Axis, first: i32) -> [Piece; 2] {
        match axis {
            Axis::Rows => [
                Piece {
                    rows: first,
                    ..self.clone()
                },
                Piece {
                    row: self.row + first,
                    rows: self.rows - first,
                    ..self.clone()
                },
            ],
            Axis::Cols => [
                Piece {
                    cols: first,
                    ..self.clone()
                },
                Piece {
                    col: self.col + first,
                    cols: self.cols - first,
                    ..self.clone()
                },
            ],
        }
    }
}

/// An axis a piece can break across.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Rows,
    Cols,
}

/// A sign's candidate values read upright and turned half round (None where
/// its strokes read as no sign that way), and whether a lacuna took part of
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellReading {
    pub upright: Option<Vec<Value>>,
    pub turned: Option<Vec<Value>>,
    pub worn: bool,
}

/// A fragment as the file shows it: its size, each cell's sign read upright
/// and turned half round, and the stroke positions the lacunae took (for the
/// bench).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    pub rows: i32,
    pub cols: i32,
    pub readings: BTreeMap<(i32, i32), CellReading>,
    pub lost: BTreeMap<(i32, i32), Vec<Line>>,
}

impl Fragment {
    /// Its size from the cells it has, its last row and column.
    pub fn of(
        readings: BTreeMap<(i32, i32), CellReading>,
        lost: BTreeMap<(i32, i32), Vec<Line>>,
    ) -> Fragment {
        let rows = readings.keys().map(|(row, _)| row + 1).max().unwrap_or(0);
        let cols = readings.keys().map(|(_, col)| col + 1).max().unwrap_or(0);
        Fragment {
            rows,
            cols,
            readings,
            lost,
        }
    }

    /// `[row, col]` of every cell, row by row.
    pub fn cells(&self) -> Vec<(i32, i32)> {
        (0..self.rows)
            .flat_map(|row| (0..self.cols).map(move |col| (row, col)))
            .collect()
    }

    /// Where a cell lands once the fragment is turned half round.
    pub fn opposite(&self, row: i32, col: i32) -> (i32, i32) {
        (self.rows - 1 - row, self.cols - 1 - col)
    }
}

/// A way the fragments rejoin: the layout it reads along, the message (an
/// edition, if weathered), and where each fragment goes
/// (`(fragment index, row, col, turned)`, in file order).
#[derive(Debug, Clone, PartialEq)]
pub struct Rejoining {
    pub layout: Layout,
    pub message: String,
    pub places: Vec<(usize, i32, i32, bool)>,
}

/// A broken plate rejoined, for the bench: the layout it reads along, every
/// sign's value in reading order (None where a lacuna leaves it open), the
/// lacunae (canonical and upright), what each worn sign may hold, and the
/// plate drawn whole from the file's own fragments, each moved to its place.
#[derive(Debug, Clone, PartialEq)]
pub struct Whole {
    pub layout: Layout,
    pub signs: Vec<Option<Value>>,
    pub damage: Damage,
    pub candidates: BTreeMap<usize, Vec<Value>>,
    pub svg: String,
}

/// `(sequence, svg, pieces, damage)` for a broken game plate, along the
/// snail; weathered first if asked (damage is then weathering's, else
/// empty).
pub fn inscribe(
    message: &str,
    layout: &Layout,
    weathered: bool,
    random: &mut RubyRandom,
) -> Result<(Vec<Value>, String, Vec<Piece>, Damage)> {
    refuse(layout)?;

    for _ in 0..100 {
        let sequence = cipher::encode(message, None, false, None, random)?;
        let size = sequence.len().isqrt();
        if (size as i32) < 2 * MIN_SIDE {
            return Err(Error::other(TOO_SMALL));
        }
        let damage = if weathered {
            weathering::weather(&sequence, random, None)
        } else {
            Damage::new()
        };
        let pieces = breaking::cut(size, random);
        let svg_text = tray::render(&sequence, layout, &pieces, random, &damage)?;
        if join::all(&read(&svg_text)?, &layout::KEYLESS)?.len() == 1 {
            return Ok((sequence, svg_text, pieces, damage));
        }
    }
    Err(Error::other(
        "Could not break this plate so that it rejoins one way only",
    ))
}

fn refuse(layout: &Layout) -> Result<()> {
    if matches!(layout, Layout::Grille(_)) {
        return Err(Error::other(NOT_UNDER_A_GRILLE));
    }
    if *layout != Layout::Phaistos {
        return Err(Error::other(NOT_ALONG_THE_ROWS));
    }
    Ok(())
}

/// Whether a plate is broken (a string test: intact plates are not parsed
/// twice).
pub fn is_broken(svg_text: &str) -> bool {
    svg_text.contains(r#"class="fragment""#)
}

/// The message of a broken plate, rejoined.
pub fn decode(svg_text: &str, passphrase: Option<&str>, layout: Option<&Layout>) -> Result<String> {
    if passphrase.is_some() {
        return Err(Error::other(SEALED));
    }
    Ok(rejoin(svg_text, layout, None)?.message)
}

/// The one way a broken plate's fragments rejoin (along a layout, if given).
/// `fragments`: the fragments already read from this file, not read again.
pub fn rejoin(
    svg_text: &str,
    layout: Option<&Layout>,
    fragments: Option<&[Fragment]>,
) -> Result<Rejoining> {
    let owned;
    let fragments = match fragments {
        Some(fragments) => fragments,
        None => {
            owned = read(svg_text)?;
            &owned
        }
    };
    let layouts: Vec<Layout> = match layout {
        Some(layout) => vec![layout.clone()],
        None => layout::KEYLESS.to_vec(),
    };
    let found = join::all(fragments, &layouts)?;
    if found.is_empty() {
        return Err(Error::decode(
            "The fragments do not rejoin: no arrangement balances every pair",
        ));
    }
    let mut messages: Vec<&String> = found.iter().map(|r| &r.message).collect();
    messages.dedup();
    if messages.len() > 1 {
        return Err(Error::decode(
            "The fragments rejoin into more than one message",
        ));
    }
    Ok(found.into_iter().next().expect("found is not empty"))
}

/// A weathered broken plate rejoined (one way, or a decode error), for the
/// bench.
pub fn rejoined(svg_text: &str) -> Result<Whole> {
    let fragments = read(svg_text)?;
    let rejoining = rejoin(svg_text, None, Some(&fragments))?;
    rejoined::of(svg_text, &fragments, &rejoining)
}

/// The fragments of a broken plate, in the order the file lists them.
pub fn read(svg_text: &str) -> Result<Vec<Fragment>> {
    let groups = fragment_groups(svg_text)?;
    groups.iter().map(reading::fragment).collect()
}

/// The file's `<g class="fragment">` elements, in document order.
fn fragment_groups(svg_text: &str) -> Result<Vec<xmltree::Element>> {
    let groups = svg::with_document(svg_text, |root| {
        if root.name != "svg" {
            return Err(Error::decode("Not an SVG document"));
        }
        let mut groups = Vec::new();
        collect_groups(root, &mut groups);
        Ok(groups)
    })??;
    if groups.len() > MAX_PIECES {
        return Err(Error::decode(format!(
            "A broken plate has at most {MAX_PIECES} fragments"
        )));
    }
    Ok(groups)
}

fn collect_groups(element: &xmltree::Element, groups: &mut Vec<xmltree::Element>) {
    for child in &element.children {
        if let xmltree::XMLNode::Element(child) = child {
            if child.name == "g" && child.attributes.get("class").map(String::as_str) == Some(CLASS)
            {
                groups.push(child.clone());
            }
            collect_groups(child, groups);
        }
    }
}

/// A fragment read from its hidden lines, in its own frame: the strokes, and
/// the stroke positions lacunae took (weathered plates).
pub mod reading {
    use super::*;

    pub fn fragment(group: &xmltree::Element) -> Result<Fragment> {
        let cells = by_cell(group, svg::Layer::Strokes)?;
        if cells.is_empty() {
            return Err(Error::decode("A fragment has no signs"));
        }
        let lost = by_cell(group, svg::Layer::Lacunae)?;
        let empty = Vec::new();
        let mut cell_readings = BTreeMap::new();
        let mut cell_lost = BTreeMap::new();
        for (cell, strokes) in &cells {
            let gone_lines = lost.get(cell).unwrap_or(&empty);
            cell_readings.insert(*cell, readings(strokes, gone_lines, cell.0, cell.1)?);
            cell_lost.insert(*cell, gone(gone_lines, *cell));
        }
        let fragment = Fragment::of(cell_readings, cell_lost);
        if fragment.cells().len() != fragment.readings.len() {
            return Err(Error::decode("A fragment has lost a sign"));
        }
        Ok(fragment)
    }

    /// A cell's lost stroke positions, canonical and upright in the
    /// fragment's frame.
    fn gone(lost: &[Line], (row, col): (i32, i32)) -> Vec<Line> {
        layout::upright(lost, row, col, false, &Layout::Phaistos)
            .into_iter()
            .map(glyphs::canonical)
            .collect()
    }

    /// A fragment's lines in one layer, as `svg::lines` reads a plate's, by
    /// cell (in insertion order per cell; the map keys group them).
    fn by_cell(
        group: &xmltree::Element,
        layer: svg::Layer,
    ) -> Result<BTreeMap<(i32, i32), Vec<Line>>> {
        let mut lines = Vec::new();
        collect_layer(group, None, layer, &mut lines)?;
        let mut cells: BTreeMap<(i32, i32), Vec<Line>> = BTreeMap::new();
        for line in lines {
            cells
                .entry(layout::cell_of(line, &Layout::Phaistos))
                .or_default()
                .push(line);
        }
        Ok(cells)
    }

    /// Strokes are lines in no named layer; lacunae have their own (the
    /// parent's class decides, as in svg's layers).
    fn collect_layer(
        element: &xmltree::Element,
        parent_class: Option<&str>,
        layer: svg::Layer,
        lines: &mut Vec<Line>,
    ) -> Result<()> {
        if element.name == "line" {
            let of = match parent_class {
                None => Some(svg::Layer::Strokes),
                Some(lacunae::CLASS) => Some(svg::Layer::Lacunae),
                Some(crate::tally::CLASS) => Some(svg::Layer::Tally),
                Some(_) => None,
            };
            if of == Some(layer) {
                let mut line = [0i32; 4];
                for (slot, attr) in line.iter_mut().zip(["x1", "y1", "x2", "y2"]) {
                    *slot = svg::coordinate(element.attributes.get(attr).map(String::as_str))?;
                }
                lines.push(line);
            }
            return Ok(());
        }
        let class = element.attributes.get("class").map(String::as_str);
        for child in &element.children {
            if let xmltree::XMLNode::Element(child) = child {
                collect_layer(child, class, layer, lines)?;
            }
        }
        Ok(())
    }

    /// A sign's candidate values read upright and turned half round (None
    /// where it does not read), and whether a lacuna took part of it.
    pub fn readings(strokes: &[Line], lost: &[Line], row: i32, col: i32) -> Result<CellReading> {
        if row < 0 || col < 0 {
            return Err(Error::decode("A fragment sign lies outside its fragment"));
        }
        let seen = layout::upright(strokes, row, col, false, &Layout::Phaistos);
        let gone = layout::upright(lost, row, col, false, &Layout::Phaistos);
        Ok(CellReading {
            upright: candidates(&seen, &gone)?,
            turned: candidates(&turned(&seen), &turned(&gone))?,
            worn: !gone.is_empty(),
        })
    }

    fn candidates(seen: &[Line], gone: &[Line]) -> Result<Option<Vec<Value>>> {
        let values = legible(seen, gone)?;
        if let Some(values) = &values {
            if values.len() > MAX_CANDIDATES {
                return Err(Error::decode(
                    "A sign of a broken plate is worn past one corner",
                ));
            }
        }
        Ok(values)
    }

    /// What the strokes allow, or None where they are no sign (read the
    /// other way up, say).
    fn legible(seen: &[Line], gone: &[Line]) -> Result<Option<Vec<Value>>> {
        match weathering::candidates(seen, gone) {
            Ok(values) => Ok(Some(values)),
            Err(error) if error.is_decode() => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn turned(lines: &[Line]) -> Vec<Line> {
        lines.iter().map(|&stroke| glyphs::turn(stroke)).collect()
    }
}

/// Where a plate breaks: straight across, one fragment in two at a time.
pub mod breaking {
    use super::*;

    /// Two to MAX_PIECES fragments tiling the grid, one in two turned half
    /// round.
    pub fn cut(size: usize, random: &mut RubyRandom) -> Vec<Piece> {
        let breaks = 1 + random.rand_below(MAX_PIECES as u64 - 1) as usize;
        let mut pieces = vec![Piece {
            row: 0,
            col: 0,
            rows: size as i32,
            cols: size as i32,
            turned: false,
        }];
        for _ in 0..breaks {
            pieces = break_one(pieces, random);
        }
        for piece in &mut pieces {
            piece.turned = random.rand_below(2) == 0;
        }
        pieces
    }

    /// The pieces with one of them broken in two (as they were when none can
    /// break).
    fn break_one(pieces: Vec<Piece>, random: &mut RubyRandom) -> Vec<Piece> {
        let splittable: Vec<usize> = pieces
            .iter()
            .enumerate()
            .filter(|(_, piece)| piece.rows.max(piece.cols) >= 2 * MIN_SIDE)
            .map(|(i, _)| i)
            .collect();
        if splittable.is_empty() {
            return pieces;
        }
        let chosen = splittable[random.sample_index(splittable.len())];
        let halves = split(&pieces[chosen], random);
        pieces
            .into_iter()
            .enumerate()
            .flat_map(|(i, piece)| {
                if i == chosen {
                    halves.to_vec()
                } else {
                    vec![piece]
                }
            })
            .collect()
    }

    /// One piece broken in two, straight across its rows or its columns.
    fn split(piece: &Piece, random: &mut RubyRandom) -> [Piece; 2] {
        let sides: Vec<(Axis, i32)> = [(Axis::Rows, piece.rows), (Axis::Cols, piece.cols)]
            .into_iter()
            .filter(|(_, side)| *side >= 2 * MIN_SIDE)
            .collect();
        let (axis, side) = sides[random.sample_index(sides.len())];
        let first = MIN_SIDE + random.rand_below((side - 2 * MIN_SIDE + 1) as u64) as i32;
        piece.split(axis, first)
    }
}

/// Every way the fragments tile the board, turned or not, read along the
/// layouts.
pub mod join {
    use super::*;

    /// Each board cell's fragment, as `(index, its top row, its left
    /// column)`, by `[row][col]`.
    pub type Owners = Vec<Vec<Option<(usize, i32, i32)>>>;

    /// Fragments laid on the board along a tiling, some turned half round:
    /// what each cell shows.
    pub struct Laid<'a> {
        pub fragments: &'a [Fragment],
        pub tiling: &'a [(usize, i32, i32)],
        pub owner: &'a Owners,
        pub turns: &'a [bool],
    }

    impl Laid<'_> {
        /// The sign in a cell, read upright or turned as the path says:
        /// (candidate values if it reads, worn?).
        pub fn sign(&self, row: i32, col: i32, turned: bool) -> (Option<&Vec<Value>>, bool) {
            let (index, top, left) =
                self.owner[row as usize][col as usize].expect("a tiling covers the board");
            let reading = self.readings(index, row - top, col - left);
            let values = if self.turns[index] == turned {
                reading.upright.as_ref()
            } else {
                reading.turned.as_ref()
            };
            (values, reading.worn)
        }

        /// What the fragment shows at a cell of its own, wherever it lies
        /// turned.
        fn readings(&self, index: usize, row: i32, col: i32) -> &CellReading {
            let fragment = &self.fragments[index];
            let cell = if self.turns[index] {
                fragment.opposite(row, col)
            } else {
                (row, col)
            };
            fragment
                .readings
                .get(&cell)
                .expect("a fragment covers its own cells")
        }

        /// `(index, row, col, turned)` of every fragment, in file order.
        pub fn places(&self) -> Vec<(usize, i32, i32, bool)> {
            let mut places: Vec<(usize, i32, i32, bool)> = self
                .tiling
                .iter()
                .map(|&(index, row, col)| (index, row, col, self.turns[index]))
                .collect();
            places.sort();
            places
        }
    }

    /// Pairs left to weigh, in all the arrangements a search tries.
    struct Budget {
        left: i64,
    }

    impl Budget {
        fn spend(&mut self) -> Result<()> {
            self.left -= 1;
            if self.left < 0 {
                return Err(Error::decode(
                    "These fragments allow too many ways to rejoin them",
                ));
            }
            Ok(())
        }
    }

    /// The arrangements whose pairs all balance, as Rejoinings.
    pub fn all(fragments: &[Fragment], layouts: &[Layout]) -> Result<Vec<Rejoining>> {
        let size = side(fragments)?;
        type Paths = Vec<(Layout, Vec<(i32, i32, bool)>)>;
        let paths: Paths = layouts
            .iter()
            .map(|layout| Ok((layout.clone(), layout::path(size, layout)?)))
            .collect::<Result<_>>()?;
        let mut budget = Budget {
            left: MAX_WEIGHINGS,
        };
        let mut found = Vec::new();
        for tiling in tilings(fragments, size) {
            let owner = owners(fragments, &tiling, size);
            for turn_count in 0..(1usize << fragments.len()) {
                // Ruby's repeated_permutation: the first fragment's turn
                // varies slowest, the last one's fastest
                let turns: Vec<bool> = (0..fragments.len())
                    .map(|i| (turn_count >> (fragments.len() - 1 - i)) & 1 == 1)
                    .collect();
                let laid = Laid {
                    fragments,
                    tiling: &tiling,
                    owner: &owner,
                    turns: &turns,
                };
                for (layout, path) in &paths {
                    if let Some(rejoining) = rejoining(&laid, layout, path, &mut budget)? {
                        found.push(rejoining);
                    }
                }
            }
        }
        Ok(found)
    }

    /// The side of the board the fragments cover.
    pub fn side(fragments: &[Fragment]) -> Result<usize> {
        let cells: usize = fragments
            .iter()
            .map(|fragment| (fragment.rows * fragment.cols) as usize)
            .sum();
        let size = cells.isqrt();
        if size == 0 || size * size != cells {
            return Err(Error::decode("The fragments do not make a square plate"));
        }
        Ok(size)
    }

    /// Each cell's fragment, as `(index, its top row, its left column)`.
    pub fn owners(
        fragments: &[Fragment],
        tiling: &[(usize, i32, i32)],
        size: usize,
    ) -> Vec<Vec<Option<(usize, i32, i32)>>> {
        let mut owner = vec![vec![None; size]; size];
        for &(index, row, col) in tiling {
            let fragment = &fragments[index];
            for r in row..row + fragment.rows {
                for c in col..col + fragment.cols {
                    owner[r as usize][c as usize] = Some((index, row, col));
                }
            }
        }
        owner
    }

    /// The board read along a path: a Rejoining if every pair can balance,
    /// else None (on the first pair that cannot, which is where most
    /// arrangements stop).
    fn rejoining(
        laid: &Laid<'_>,
        layout: &Layout,
        path: &[(i32, i32, bool)],
        budget: &mut Budget,
    ) -> Result<Option<Rejoining>> {
        let mut cells: Vec<(Vec<Value>, bool)> = Vec::with_capacity(path.len());
        for pair in path.chunks(2) {
            budget.spend()?;
            let signs: Vec<(Option<&Vec<Value>>, bool)> = pair
                .iter()
                .map(|&(row, col, turned)| laid.sign(row, col, turned))
                .collect();
            let [first, second] = signs.as_slice() else {
                return Ok(None); // a crafted odd board never balances
            };
            if !balances(first.0, second.0) {
                return Ok(None);
            }
            for (values, worn) in signs {
                cells.push((values.expect("balanced signs read").clone(), worn));
            }
        }
        Ok(edition(&cells).map(|message| Rejoining {
            layout: layout.clone(),
            message,
            places: laid.places(),
        }))
    }

    /// The edition of a board whose pairs all balance; None if they are all
    /// null pairs (no letters).
    fn edition(cells: &[(Vec<Value>, bool)]) -> Option<String> {
        weathering::edition::text(cells).ok()
    }

    /// Whether a pair can balance: at once for two intact signs, else by
    /// what their candidates allow.
    fn balances(firsts: Option<&Vec<Value>>, seconds: Option<&Vec<Value>>) -> bool {
        let (Some(firsts), Some(seconds)) = (firsts, seconds) else {
            return false;
        };
        if firsts.len() == 1 && seconds.len() == 1 {
            return weathering::balance(firsts[0], seconds[0]).is_some();
        }
        !weathering::readings(firsts, seconds).is_empty()
    }

    /// Every way to tile the board with all the fragments.
    pub fn tilings(fragments: &[Fragment], size: usize) -> Vec<Vec<(usize, i32, i32)>> {
        let mut found = Vec::new();
        let mut board = vec![vec![false; size]; size];
        let mut placed = Vec::new();
        lay(fragments, &mut board, &mut placed, &mut found);
        found
    }

    /// The first empty cell, row by row, is the top-left corner of some
    /// fragment.
    fn lay(
        fragments: &[Fragment],
        board: &mut Vec<Vec<bool>>,
        placed: &mut Vec<(usize, i32, i32)>,
        found: &mut Vec<Vec<(usize, i32, i32)>>,
    ) {
        let Some((row, col)) = first_empty(board) else {
            found.push(placed.clone());
            return;
        };
        for (index, fragment) in fragments.iter().enumerate() {
            if placed.iter().any(|&(done, _, _)| done == index) || !fits(board, fragment, row, col)
            {
                continue;
            }
            mark(board, fragment, row, col, true);
            placed.push((index, row, col));
            lay(fragments, board, placed, found);
            placed.pop();
            mark(board, fragment, row, col, false);
        }
    }

    fn first_empty(board: &[Vec<bool>]) -> Option<(i32, i32)> {
        for (row, cells) in board.iter().enumerate() {
            if let Some(col) = cells.iter().position(|&taken| !taken) {
                return Some((row as i32, col as i32));
            }
        }
        None
    }

    fn fits(board: &[Vec<bool>], fragment: &Fragment, row: i32, col: i32) -> bool {
        let size = board.len() as i32;
        row + fragment.rows <= size
            && col + fragment.cols <= size
            && (row..row + fragment.rows)
                .all(|r| (col..col + fragment.cols).all(|c| !board[r as usize][c as usize]))
    }

    fn mark(board: &mut [Vec<bool>], fragment: &Fragment, row: i32, col: i32, value: bool) {
        for r in row..row + fragment.rows {
            for c in col..col + fragment.cols {
                board[r as usize][c as usize] = value;
            }
        }
    }
}

/// The plate whole again: what the bench needs, from the fragments laid
/// where the one rejoining puts them, and the drawing.
pub mod rejoined {
    use super::*;

    pub fn of(svg_text: &str, fragments: &[Fragment], rejoining: &Rejoining) -> Result<Whole> {
        let tiling: Vec<(usize, i32, i32)> = rejoining
            .places
            .iter()
            .map(|&(index, row, col, _)| (index, row, col))
            .collect();
        let side = join::side(fragments)?;
        let owner = join::owners(fragments, &tiling, side);
        let turns: Vec<bool> = rejoining.places.iter().map(|&(_, _, _, t)| t).collect();
        let laid = join::Laid {
            fragments,
            tiling: &tiling,
            owner: &owner,
            turns: &turns,
        };

        let mut signs = Vec::new(); // (candidates, worn, lost upright on the plate)
        for &(row, col, turned) in &layout::path(side, &rejoining.layout)? {
            let (values, worn) = laid.sign(row, col, turned);
            let values = values
                .ok_or_else(|| Error::decode("A rejoined sign does not read"))?
                .clone();
            let (index, top, left) =
                owner[row as usize][col as usize].expect("a tiling covers the board");
            let lost = lost(&fragments[index], (row - top, col - left), turns[index]);
            signs.push((values, worn, lost));
        }

        let mut damage = Damage::new();
        let mut candidates = BTreeMap::new();
        for (index, (values, worn, lost)) in signs.iter().enumerate() {
            if *worn {
                damage.insert(index, lost.clone());
                candidates.insert(index, values.clone());
            }
        }
        Ok(Whole {
            layout: rejoining.layout.clone(),
            signs: signs
                .iter()
                .map(|(values, worn, _)| if *worn { None } else { Some(values[0]) })
                .collect(),
            damage,
            candidates,
            svg: drawing(svg_text, fragments, &rejoining.places)?,
        })
    }

    /// A fragment cell's lost strokes as they lie on the plate: turned with
    /// the fragment.
    fn lost(fragment: &Fragment, cell: (i32, i32), turned: bool) -> Vec<Line> {
        let key = if turned {
            fragment.opposite(cell.0, cell.1)
        } else {
            cell
        };
        let lost = fragment.lost.get(&key).cloned().unwrap_or_default();
        if turned {
            lost.into_iter()
                .map(|stroke| glyphs::canonical(glyphs::turn(stroke)))
                .collect()
        } else {
            lost
        }
    }

    /// The file's fragment groups moved onto the plate's grid: translated to
    /// their cells, turned half round about their own middle where they lie
    /// turned. Spliced textually: the groups were machine-written, so the
    /// text is the serialization.
    fn drawing(
        svg_text: &str,
        fragments: &[Fragment],
        places: &[(usize, i32, i32, bool)],
    ) -> Result<String> {
        let groups = group_texts(svg_text);
        if groups.len() != fragments.len() {
            return Err(Error::decode("A fragment has no signs"));
        }
        let moved: Vec<String> = places
            .iter()
            .map(|&(index, row, col, turned)| {
                let replaced = with_transform(
                    &groups[index],
                    &transform(&fragments[index], (row, col), turned),
                )?;
                rexml_text(&replaced)
            })
            .collect::<Result<_>>()?;
        let (width, height) = canvas(join::side(fragments)?);
        Ok(tray::document(
            width,
            height,
            &format!("  {}", lacunae::PATTERN),
            &moved,
        ))
    }

    /// `(width, height)` of a whole plate of this side.
    fn canvas(side: usize) -> (i32, i32) {
        let (width, height) = layout::cell(&Layout::Phaistos);
        (
            width * side as i32 + 2 * layout::PADDING,
            height * side as i32 + 2 * layout::PADDING,
        )
    }

    /// The fragment groups as raw text regions, balanced `<g>` ... `</g>`.
    fn group_texts(svg_text: &str) -> Vec<String> {
        let mut groups = Vec::new();
        let mut from = 0;
        while let Some(found) = svg_text[from..].find("<g class=\"fragment\"") {
            let start = from + found;
            let mut depth = 0usize;
            let mut at = start;
            let mut end = None;
            while at < svg_text.len() {
                let open = svg_text[at..].find("<g");
                let close = svg_text[at..].find("</g>");
                match (open, close) {
                    (Some(o), Some(c)) if o < c => {
                        depth += 1;
                        at += o + 2;
                    }
                    (_, Some(c)) => {
                        depth -= 1;
                        at += c + 4;
                        if depth == 0 {
                            end = Some(at);
                            break;
                        }
                    }
                    _ => break,
                }
            }
            match end {
                Some(end) => {
                    groups.push(svg_text[start..end].to_string());
                    from = end;
                }
                None => break,
            }
        }
        groups
    }

    /// The group with its transform attribute's value replaced.
    fn with_transform(group: &str, transform: &str) -> Result<String> {
        let open_end = group.find('>').unwrap_or(group.len());
        let opening = &group[..open_end];
        let marker = " transform=\"";
        let Some(at) = opening.find(marker) else {
            // a fragment the library wrote always carries one; a crafted one
            // may not: give it the attribute before the tag closes
            return Ok(format!(
                "{} transform=\"{}\"{}",
                group[..open_end].trim_end_matches('>'),
                transform,
                &group[open_end..]
            ));
        };
        let value_from = at + marker.len();
        let value_to = value_from
            + opening[value_from..]
                .find('"')
                .ok_or_else(|| Error::decode("Unreadable SVG: not well-formed XML"))?;
        Ok(format!(
            "{}{}{}",
            &group[..value_from],
            transform,
            &group[value_to..]
        ))
    }

    /// The group as Ruby's REXML would re-serialize it after the move:
    /// every tag rewritten with its attributes sorted and single-quoted,
    /// the text between tags kept as it stands. The groups are
    /// machine-written, so a tag-level rewrite is a faithful serializer.
    fn rexml_text(group: &str) -> Result<String> {
        let mut out = String::with_capacity(group.len());
        let mut rest = group;
        let mut first = true;
        while let Some(open) = rest.find('<') {
            out.push_str(&rest[..open]);
            let close = rest[open..]
                .find('>')
                .ok_or_else(|| Error::decode("Unreadable SVG: not well-formed XML"))?;
            // The moved group itself carries the holder document's context
            // (double quotes); everything inside keeps the file's own
            out.push_str(&rexml_tag(
                &rest[open..=open + close],
                if first { '"' } else { '\'' },
            )?);
            first = false;
            rest = &rest[open + close + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    /// One tag rewritten as REXML writes it.
    fn rexml_tag(tag: &str, quote: char) -> Result<String> {
        if tag.starts_with("</") {
            return Ok(tag.to_string());
        }
        let inner = tag
            .trim_start_matches('<')
            .trim_end_matches('>')
            .trim_end_matches('/');
        let self_closing = tag.ends_with("/>");
        let name_end = inner.find(' ').unwrap_or(inner.len());
        let name = &inner[..name_end];
        let mut attributes: Vec<(String, String)> = Vec::new();
        let mut rest = inner[name_end..].trim_start();
        while !rest.is_empty() {
            let eq = rest
                .find("=\"")
                .ok_or_else(|| Error::decode("Unreadable SVG: not well-formed XML"))?;
            let key = rest[..eq].trim().to_string();
            let value_from = eq + 2;
            let value_to = value_from
                + rest[value_from..]
                    .find('"')
                    .ok_or_else(|| Error::decode("Unreadable SVG: not well-formed XML"))?;
            attributes.push((key, rest[value_from..value_to].to_string()));
            rest = rest[value_to + 1..].trim_start();
        }
        attributes.sort();
        let listed: String = attributes
            .iter()
            .map(|(key, value)| format!(" {key}={quote}{value}{quote}"))
            .collect();
        Ok(format!(
            "<{name}{listed}{}>",
            if self_closing { "/" } else { "" }
        ))
    }

    fn transform(fragment: &Fragment, (row, col): (i32, i32), turned: bool) -> String {
        let (width, height) = layout::cell(&Layout::Phaistos);
        let turn = if turned {
            let middle_x = f64::from(BOARD) + f64::from(fragment.cols * width) / 2.0;
            let middle_y = f64::from(BOARD) + f64::from(fragment.rows * height) / 2.0;
            format!(" rotate(180 {middle_x:?} {middle_y:?})")
        } else {
            String::new()
        };
        format!("translate({} {}){}", col * width, row * height, turn)
    }
}

/// The broken plate as drawn: the empty board, and the fragments in the tray
/// below it.
pub mod tray {
    use super::*;

    pub const GAP: i32 = 40; // between the board and the tray, and between fragments
    pub const TILT: f64 = 4.0; // degrees a fragment lies askew in the tray, at most
    pub const JITTER: i64 = 3; // how worn a fragment's edge is
    pub const STEP: i32 = 30; // how often along an edge it is worn
    pub const CLAY: &str = "#ece6da";
    pub const RIM: &str = "#b3a993";

    /// A sign as a fragment shows it, in its own frame: where, its value,
    /// whether it is turned, its place in the reading order, and the strokes
    /// a lacuna took.
    struct Sign {
        row: i32,
        col: i32,
        num: Value,
        turned: bool,
        index: usize,
        lost: Vec<Line>,
    }

    impl Sign {
        fn origin(&self) -> (i32, i32) {
            layout::origin(self.row, self.col, &Layout::Phaistos)
        }

        /// The strokes that survive, turned as the sign lies.
        fn strokes(&self) -> Vec<Line> {
            lacunae::strokes(self.num, &self.lost)
                .into_iter()
                .map(|stroke| {
                    if self.turned {
                        glyphs::turn(stroke)
                    } else {
                        stroke
                    }
                })
                .collect()
        }
    }

    /// Where the fragments go in the tray: each where it lies highest, along
    /// a skyline of what is already laid.
    pub struct Skyline {
        spans: Vec<(i32, i32, i32)>, // (from x, to x, first free y)
        pub room: i32,
        pub bottom: i32,
    }

    impl Skyline {
        pub fn new(top: i32, room: i32) -> Skyline {
            Skyline {
                spans: vec![(0, room, top)],
                room,
                bottom: top,
            }
        }

        /// Where a fragment's box goes: its top-left corner.
        pub fn place(&mut self, width: i32, height: i32) -> (i32, i32) {
            let (x, y) = self
                .spans
                .iter()
                .filter(|(from, _, _)| from + width <= self.room)
                .map(|&(from, _, _)| (from, self.top_over(from, width)))
                .min_by_key(|&(from, top)| (top, from))
                .expect("a fragment is never wider than the tray");
            self.raise_to(x, x + width + GAP, y + height + GAP);
            self.bottom = self.bottom.max(y + height);
            (BOARD + x, y)
        }

        fn top_over(&self, from: i32, width: i32) -> i32 {
            self.spans
                .iter()
                .filter(|&&(left, right, _)| left < from + width && right > from)
                .map(|&(_, _, free)| free)
                .max()
                .expect("the skyline always covers the tray")
        }

        /// The skyline from one x to another raised to a new top: what lay
        /// there before is cut away.
        fn raise_to(&mut self, from: i32, upto: i32, top: i32) {
            let mut kept: Vec<(i32, i32, i32)> = self
                .spans
                .iter()
                .flat_map(|&(left, right, free)| {
                    [(left, right.min(from), free), (left.max(upto), right, free)]
                })
                .filter(|&(left, right, _)| left < right)
                .collect();
            kept.push((from, upto, top));
            kept.sort();
            self.spans = kept;
        }
    }

    /// The board, and below it a tray a little wider: fragments as wide as
    /// the board fit side by side. Damage is weathering's, by reading index.
    pub fn render(
        sequence: &[Value],
        layout: &Layout,
        pieces: &[Piece],
        random: &mut RubyRandom,
        damage: &Damage,
    ) -> Result<String> {
        let size = sequence.len().isqrt();
        let mut tray = tray_below(size, layout);
        let fragments = lay(pieces, sequence, damage, layout, &mut tray, random)?;
        let defs = if damage.is_empty() {
            board(size, layout)
        } else {
            format!("  {}\n{}", lacunae::PATTERN, board(size, layout))
        };
        Ok(document(
            tray.room + 2 * BOARD,
            tray.bottom + BOARD,
            &defs,
            &fragments,
        ))
    }

    fn tray_below(size: usize, layout: &Layout) -> Skyline {
        let (width, height) = layout::cell(layout);
        Skyline::new(
            BOARD + height * size as i32 + GAP,
            width * size as i32 + (MAX_PIECES as i32 - 1) * GAP,
        )
    }

    /// The fragments drawn in the tray, the tallest first (in shuffled order
    /// among equals).
    fn lay(
        pieces: &[Piece],
        sequence: &[Value],
        damage: &Damage,
        layout: &Layout,
        tray: &mut Skyline,
        random: &mut RubyRandom,
    ) -> Result<Vec<String>> {
        let where_ = reading_order(sequence.len().isqrt(), layout)?;
        let mut shuffled = pieces.to_vec();
        random.shuffle(&mut shuffled);
        shuffled.sort_by_key(|piece| -piece.rows); // stable: shuffled order among equals
        shuffled
            .iter()
            .map(|piece| {
                Ok(fragment(
                    boxed(piece, layout),
                    &signs(piece, sequence, damage, &where_),
                    tray,
                    random,
                ))
            })
            .collect()
    }

    /// `(row, col)` => `(reading index, turned)`.
    fn reading_order(size: usize, layout: &Layout) -> Result<BTreeMap<(i32, i32), (usize, bool)>> {
        Ok(layout::path(size, layout)?
            .into_iter()
            .enumerate()
            .map(|(index, (row, col, turned))| ((row, col), (index, turned)))
            .collect())
    }

    /// `(width, height)` of a piece's box.
    fn boxed(piece: &Piece, layout: &Layout) -> (i32, i32) {
        let (width, height) = layout::cell(layout);
        (width * piece.cols, height * piece.rows)
    }

    pub fn document(width: i32, height: i32, board: &str, fragments: &[String]) -> String {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n  <rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n{board}\n{}\n</svg>\n",
            fragments.join("\n")
        )
    }

    /// The empty board, ruled in cells, where the plate was.
    pub fn board(size: usize, layout: &Layout) -> String {
        let (width, height) = layout::cell(layout);
        let cell = format!(
            "<rect width=\"{width}\" height=\"{height}\" fill=\"#faf7f1\" stroke=\"#e4dccb\" stroke-width=\"2\"/>"
        );
        format!(
            "<defs><pattern id=\"board\" x=\"{BOARD}\" y=\"{BOARD}\" width=\"{width}\" height=\"{height}\" patternUnits=\"userSpaceOnUse\">{cell}</pattern></defs>\n<rect class=\"board\" x=\"{BOARD}\" y=\"{BOARD}\" width=\"{}\" height=\"{}\" fill=\"url(#board)\" stroke=\"{RIM}\" stroke-width=\"2\" stroke-dasharray=\"10 8\"/>",
            width * size as i32,
            height * size as i32
        )
    }

    /// A fragment's signs, in its own frame.
    fn signs(
        piece: &Piece,
        sequence: &[Value],
        damage: &Damage,
        where_: &BTreeMap<(i32, i32), (usize, bool)>,
    ) -> Vec<Sign> {
        piece
            .cells()
            .into_iter()
            .map(|(row, col)| {
                let (index, turned) = where_[&(piece.row + row, piece.col + col)];
                let (row, col) = if piece.turned {
                    piece.opposite(row, col)
                } else {
                    (row, col)
                };
                Sign {
                    row,
                    col,
                    num: sequence[index],
                    turned: turned ^ piece.turned,
                    index,
                    lost: damage.get(&index).cloned().unwrap_or_default(),
                }
            })
            .collect()
    }

    /// One fragment in the tray: its clay, its signs pressed, and its
    /// strokes as hidden lines. In its own frame its box starts at BOARD, as
    /// the board does.
    fn fragment(
        (width, height): (i32, i32),
        signs: &[Sign],
        tray: &mut Skyline,
        random: &mut RubyRandom,
    ) -> String {
        let (x, y) = tray.place(width, height);
        let tilt = format!(
            "rotate({} {} {})",
            tilt(width, height, random),
            BOARD + width / 2,
            BOARD + height / 2
        );
        format!(
            "  <g class=\"{CLASS}\" transform=\"translate({} {}) {tilt}\">\n{}{}{}{}{}  </g>",
            x - BOARD,
            y - BOARD,
            sherd(width, height, random),
            lacunae_part(signs, Part::Worn),
            wedges(signs),
            hidden(signs),
            lacunae_part(signs, Part::LostLines),
        )
    }

    /// A slight tilt, never so much that a corner reaches the next fragment.
    fn tilt(width: i32, height: i32, random: &mut RubyRandom) -> String {
        // `* 180 / PI` as Ruby computes it, not `to_degrees` (180 / PI first)
        let limit = (f64::from(GAP) / 3.0).atan2(f64::from(width.max(height))) * 180.0
            / std::f64::consts::PI;
        crate::stylus::round1_as_ruby(random.rand_float_incl(-1.0, 1.0) * TILT.min(limit))
    }

    /// A fragment's clay: its box, every edge worn.
    fn sherd(width: i32, height: i32, random: &mut RubyRandom) -> String {
        let worn: Vec<String> = edges(width, height)
            .into_iter()
            .map(|(x, y)| {
                let jx = x as i64 + random.rand_int_incl(-JITTER, JITTER);
                let jy = y as i64 + random.rand_int_incl(-JITTER, JITTER);
                format!("{jx},{jy}")
            })
            .collect();
        format!(
            "    <polygon class=\"sherd\" points=\"{}\" fill=\"{CLAY}\" stroke=\"{RIM}\" stroke-width=\"1.5\"/>\n",
            worn.join(" ")
        )
    }

    /// Points every STEP or so around a fragment's box.
    fn edges(width: i32, height: i32) -> Vec<(i32, i32)> {
        let corners = [
            (BOARD, BOARD),
            (BOARD + width, BOARD),
            (BOARD + width, BOARD + height),
            (BOARD, BOARD + height),
        ];
        (0..4)
            .flat_map(|i| along(corners[i], corners[(i + 1) % 4]))
            .collect()
    }

    /// Points every STEP or so along an edge, from its first corner
    /// (integer arithmetic, as Ruby divides integers).
    fn along((ax, ay): (i32, i32), (bx, by): (i32, i32)) -> Vec<(i32, i32)> {
        let steps = (((bx - ax).abs() + (by - ay).abs()) / STEP).max(1);
        (0..steps)
            .map(|k| {
                (
                    ax + ((bx - ax) * k).div_euclid(steps),
                    ay + ((by - ay) * k).div_euclid(steps),
                )
            })
            .collect()
    }

    enum Part {
        Worn,
        LostLines,
    }

    /// A fragment's worn patches (under the wedges) or its lost stroke
    /// positions, drawn as lacunae draws a plate's ('' if it has none).
    fn lacunae_part(signs: &[Sign], part: Part) -> String {
        let worn: Vec<lacunae::WornSign> = signs
            .iter()
            .filter(|sign| !sign.lost.is_empty())
            .map(|sign| lacunae::WornSign {
                seed: sign.index as i64,
                lost: sign.lost.clone(),
                turned: sign.turned,
                origin: sign.origin(),
            })
            .collect();
        if worn.is_empty() {
            return String::new();
        }
        let drawn = match part {
            Part::Worn => lacunae::worn(&worn),
            Part::LostLines => lacunae::lost_lines(&worn),
        };
        let indented: String = drawn
            .lines()
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{indented}\n")
    }

    /// The signs pressed in wedges, seeded by their place in the reading
    /// order as on an intact plate (without the strokes a lacuna took).
    fn wedges(signs: &[Sign]) -> String {
        let paths: Vec<String> = signs
            .iter()
            .flat_map(|sign| {
                svg::press(
                    &lacunae::strokes(sign.num, &sign.lost),
                    sign.turned,
                    sign.origin(),
                    &mut RubyRandom::new(sign.index as i64),
                )
            })
            .map(|d| format!("      <path d=\"{d}\"/>"))
            .collect();
        format!("    <g fill=\"black\">\n{}\n    </g>\n", paths.join("\n"))
    }

    /// The strokes that survive, as hidden lines: what the decoder reads.
    fn hidden(signs: &[Sign]) -> String {
        let lines: Vec<String> = signs
            .iter()
            .flat_map(|sign| {
                let (x, y) = sign.origin();
                sign.strokes()
                    .into_iter()
                    .map(move |stroke| svg::line_tag(layout::relative(stroke, -x, -y)))
            })
            .collect();
        format!(
            "    <g stroke=\"black\" stroke-width=\"2\" display=\"none\">\n{}\n    </g>\n",
            lines.join("\n")
        )
    }
}
