//! Tally signs: sums on a higher cord, as on an Inka khipu. One sign at the
//! end of every row, one under every column, and one in the corner.
//!
//! - Each place is summed on its own, mod 10, without carries: the units of a
//!   row check each other, as do the tens, hundreds and thousands. Two
//!   quadrants lost in one row in different places are then two problems with
//!   one unknown each. It is also a sum a monk could check by eye
//! - Rows and columns are the grid's, whatever the reading path; the tally
//!   signs themselves are never weathered (the higher cord survives)
//! - The corner sums the row tallies (and so the column tallies too)
//!
//! Game plates only, like weathering.

use std::collections::{HashMap, HashSet};

use crate::error::{Error, Result};
use crate::glyphs;
use crate::layout::{self, Layout};
use crate::{Value, EVIL_MARKER};

pub const CLASS: &str = "tally-lines";
pub const RUBRIC: &str = "#a3301c"; // the tally signs and their cords, as a scribe's rubric
pub const NOT_UNDER_A_GRILLE: &str = "Tally signs are not drawn under a grille yet";
pub const NOT_SEALED: &str = "Tally signs are for game plates: a sealed plate never loses a stroke";

/// A grid line a sign lies on: its row, or its column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CordKind {
    Row,
    Col,
}

/// One grid line: (kind, number).
pub type Cord = (CordKind, i32);

/// The tally signs of a plate: one per row, one per column, one in the corner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signs {
    pub rows: Vec<Value>,
    pub cols: Vec<Value>,
    pub corner: Value,
}

/// A carry-free sum: each place's digits added mod 10.
pub fn add(values: &[Value]) -> Value {
    glyphs::PLACES
        .into_iter()
        .map(|place| {
            (values
                .iter()
                .map(|value| (value / place) % 10)
                .sum::<Value>()
                % 10)
                * place
        })
        .sum()
}

/// The tally signs for a sequence laid along a layout.
pub fn signs(sequence: &[Value], layout: &Layout) -> Result<Signs> {
    let grid = grid(sequence, layout)?;
    let rows: Vec<Value> = grid.iter().map(|row| add(row)).collect();
    let size = grid.len();
    let cols: Vec<Value> = (0..size)
        .map(|col| add(&grid.iter().map(|row| row[col]).collect::<Vec<_>>()))
        .collect();
    let corner = add(&rows);
    Ok(Signs { rows, cols, corner })
}

/// The sequence's values on the grid, by [row][col].
pub fn grid(sequence: &[Value], layout: &Layout) -> Result<Vec<Vec<Value>>> {
    let size = sequence.len().isqrt();
    let mut grid = vec![vec![0; size]; size];
    for ((row, col, _), &value) in layout::path(size, layout)?.iter().zip(sequence) {
        grid[*row as usize][*col as usize] = value;
    }
    Ok(grid)
}

/// The lines a reading-order index belongs to: [(Row, r), (Col, c)].
pub fn lines(size: usize, layout: &Layout) -> Result<Vec<[Cord; 2]>> {
    Ok(layout::path(size, layout)?
        .into_iter()
        .map(|(row, col, _)| [(CordKind::Row, row), (CordKind::Col, col)])
        .collect())
}

/// Candidate values of every sign, narrowed by the tallies and the pair
/// balance until nothing changes. `candidates`: one array per sign, in
/// reading order.
pub fn restore(
    candidates: &[Vec<Value>],
    tally: &Signs,
    layout: &Layout,
) -> Result<Vec<Vec<Value>>> {
    let mut candidates: Vec<Vec<Value>> = candidates.to_vec();
    let members = members(candidates.len().isqrt(), layout)?;
    // the balance first: a sign wholly lost keeps at most 28 values if its
    // partner is known
    loop {
        let mut changed = balance_pairs(&mut candidates)?;
        for (line, indices) in &members {
            changed += fix_lone_digits(&mut candidates, indices, tally_of(tally, *line)?)?;
        }
        if changed == 0 {
            return Ok(candidates);
        }
    }
}

/// Line => the reading-order indices on it, in the order the lines first
/// appear along the path (the Ruby hash's insertion order).
fn members(size: usize, layout: &Layout) -> Result<Vec<(Cord, Vec<usize>)>> {
    let mut order: Vec<(Cord, Vec<usize>)> = Vec::new();
    let mut at: HashMap<Cord, usize> = HashMap::new();
    for (index, on) in lines(size, layout)?.into_iter().enumerate() {
        for line in on {
            let slot = *at.entry(line).or_insert_with(|| {
                order.push((line, Vec::new()));
                order.len() - 1
            });
            order[slot].1.push(index);
        }
    }
    Ok(order)
}

fn tally_of(tally: &Signs, (kind, number): Cord) -> Result<Value> {
    let signs = match kind {
        CordKind::Row => &tally.rows,
        CordKind::Col => &tally.cols,
    };
    signs
        .get(number as usize)
        .copied()
        .ok_or_else(|| Error::decode("A tally sign is missing"))
}

/// In each place, a line with one sign whose digit is still open gets it from
/// its tally; a line with none must add up. Returns how many signs were
/// narrowed.
fn fix_lone_digits(candidates: &mut [Vec<Value>], indices: &[usize], sum: Value) -> Result<usize> {
    let mut narrowed = 0;
    for place in glyphs::PLACES {
        let open: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&index| digits_at(&candidates[index], place).len() != 1)
            .collect();
        if open.is_empty() {
            check_line(candidates, indices, place, sum)?;
            continue;
        }
        if open.len() != 1 {
            continue;
        }
        let settled: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|index| !open.contains(index))
            .collect();
        let digit = missing_digit(candidates, &settled, place, sum);
        narrowed += narrow(candidates, open[0], |value| (value / place) % 10 == digit)?;
    }
    Ok(narrowed)
}

fn check_line(
    candidates: &[Vec<Value>],
    indices: &[usize],
    place: Value,
    sum: Value,
) -> Result<()> {
    if missing_digit(candidates, indices, place, sum) != 0 {
        return Err(Error::decode("The tallies do not add up"));
    }
    Ok(())
}

/// The digit a line's tally leaves for its one open sign, in a place.
fn missing_digit(candidates: &[Vec<Value>], settled: &[usize], place: Value, sum: Value) -> Value {
    let told = (sum / place % 10) as i64;
    let known: i64 = settled
        .iter()
        .map(|&index| digits_at(&candidates[index], place)[0] as i64)
        .sum();
    (told - known).rem_euclid(10) as Value
}

/// Only the values that can balance with some value of their partner.
/// Returns how many signs were narrowed.
fn balance_pairs(candidates: &mut [Vec<Value>]) -> Result<usize> {
    let mut narrowed = 0;
    for first in (0..candidates.len()).step_by(2) {
        if candidates[first].len() == 1 && candidates[first + 1].len() == 1 {
            continue;
        }
        narrowed += balance_against(candidates, first, first + 1, true)?;
        narrowed += balance_against(candidates, first + 1, first, false)?;
    }
    Ok(narrowed)
}

/// Narrow one sign of a pair to the values that balance with some value of
/// the other.
fn balance_against(
    candidates: &mut [Vec<Value>],
    index: usize,
    partner: usize,
    first: bool,
) -> Result<usize> {
    let partners = &candidates[partner];
    if first && partners.contains(&EVIL_MARKER) {
        return Ok(0); // a null pair: any first sign will do
    }
    let allowed = balancing(partners, first);
    narrow(candidates, index, |value| allowed.contains(&value))
}

/// Every value that balances with one of these partners: a few dozen when the
/// partner is known, so a sign wholly lost (10,000 values) is narrowed by
/// lookup.
fn balancing(partners: &[Value], first: bool) -> HashSet<Value> {
    let mut values: Vec<i64> = partners
        .iter()
        .flat_map(|&partner| crate::character_sums().map(move |sum| sum as i64 - partner as i64))
        .collect();
    if !first {
        values.push(EVIL_MARKER as i64); // the second sign of a null pair
    }
    values
        .into_iter()
        .filter(|&value| (0..=EVIL_MARKER as i64).contains(&value))
        .map(|value| value as Value)
        .collect()
}

/// The distinct digits of some candidate values in a place, first seen first.
fn digits_at(values: &[Value], place: Value) -> Vec<Value> {
    let mut seen = Vec::new();
    for &value in values {
        let digit = (value / place) % 10;
        if !seen.contains(&digit) {
            seen.push(digit);
        }
    }
    seen
}

/// Keep a sign's candidates that pass the test: 1 if that narrowed it, else 0.
fn narrow(
    candidates: &mut [Vec<Value>],
    index: usize,
    keep: impl Fn(Value) -> bool,
) -> Result<usize> {
    let kept: Vec<Value> = candidates[index]
        .iter()
        .copied()
        .filter(|&value| keep(value))
        .collect();
    if kept.is_empty() {
        return Err(Error::decode("The tallies do not add up"));
    }
    if kept.len() == candidates[index].len() {
        return Ok(0);
    }
    candidates[index] = kept;
    Ok(1)
}

/// The tally signs in a plate's SVG: drawn, hidden, read back.
pub mod plate {
    use super::*;
    use crate::ruby_random::RubyRandom;
    use crate::svg;

    /// The tally signs of a plate in its SVG, one row and one column past the
    /// grid, with the cords that set them apart in the gutters: pressed, and
    /// as hidden lines in their own layer (what the decoder reads; never
    /// strokes).
    pub fn layer(sequence: &[Value], cols: usize, layout: &Layout) -> Result<String> {
        if layout.is_grille() {
            return Err(Error::other(NOT_UNDER_A_GRILLE));
        }
        let placed = placed(sequence, cols, layout)?;
        let pressed: Vec<String> = placed
            .iter()
            .enumerate()
            .flat_map(|(i, &((row, col), num))| press(num, row, col, layout, i))
            .collect();
        let hidden: Vec<String> = placed
            .iter()
            .flat_map(|&((row, col), num)| hidden(num, row, col, layout))
            .collect();
        Ok(format!(
            "<g class=\"tally\" fill=\"{RUBRIC}\">\n{cords}\n{pressed}\n</g>\n<g class=\"{CLASS}\" stroke=\"black\" stroke-width=\"2\" display=\"none\">\n{hidden}\n</g>",
            cords = cords(cols, layout),
            pressed = pressed.join("\n"),
            hidden = hidden.join("\n"),
        ))
    }

    /// ((row, col), value) of every tally sign.
    fn placed(
        sequence: &[Value],
        cols: usize,
        layout: &Layout,
    ) -> Result<Vec<((i32, i32), Value)>> {
        let tally = signs(sequence, layout)?;
        let values: Vec<Value> = tally
            .rows
            .iter()
            .chain(tally.cols.iter())
            .copied()
            .chain([tally.corner])
            .collect();
        Ok(cells(cols).into_iter().zip(values).collect())
    }

    /// Where the tally signs go: the end of each row, under each column, the
    /// corner.
    fn cells(cols: usize) -> Vec<(i32, i32)> {
        let n = cols as i32;
        (0..n)
            .map(|row| (row, n))
            .chain((0..n).map(|col| (n, col)))
            .chain([(n, n)])
            .collect()
    }

    fn press(num: Value, row: i32, col: i32, layout: &Layout, seed: usize) -> Vec<String> {
        let mut random = RubyRandom::new(-1 - seed as i64);
        svg::press(
            &glyphs::strokes(num, 0, 0),
            false,
            layout::origin(row, col, layout),
            &mut random,
        )
        .into_iter()
        .map(|outline| format!("  <path d=\"{outline}\"/>"))
        .collect()
    }

    fn hidden(num: Value, row: i32, col: i32, layout: &Layout) -> Vec<String> {
        let (x, y) = layout::origin(row, col, layout);
        glyphs::strokes(num, 0, 0)
            .into_iter()
            .map(|stroke| svg::line_tag(layout::relative(stroke, -x, -y)))
            .collect()
    }

    /// Two thin rules in the gutters, between the grid and its tallies.
    fn cords(cols: usize, layout: &Layout) -> String {
        let (width, height) = layout::cell(layout);
        let (x, y) = layout::cell_origin(cols as i32, cols as i32, layout);
        let from = layout::PADDING - 10;
        format!(
            "  <rect class=\"cord\" x=\"{}\" y=\"{from}\" width=\"3\" height=\"{}\"/>\n  <rect class=\"cord\" x=\"{from}\" y=\"{}\" width=\"{}\" height=\"3\"/>",
            x - 12,
            y + height - from,
            y - 12,
            x + width - from,
        )
    }

    /// The tally signs a plate carries, or None without.
    pub fn read(svg_text: &str, layout: &Layout) -> Result<Option<Signs>> {
        let lines = svg::lines(svg_text, svg::Layer::Tally)?;
        if lines.is_empty() {
            return Ok(None);
        }
        let mut by_cell: std::collections::BTreeMap<(i32, i32), Vec<crate::Line>> =
            std::collections::BTreeMap::new();
        for line in lines {
            by_cell
                .entry(layout::cell_of(line, layout))
                .or_default()
                .push(line);
        }
        let cols = by_cell
            .keys()
            .map(|&(row, _)| row)
            .max()
            .expect("tally lines exist") as usize;
        let values: Vec<Value> = cells(cols)
            .into_iter()
            .map(|cell| read_sign(&by_cell, cell, layout))
            .collect::<Result<_>>()?;
        Ok(Some(Signs {
            rows: values[..cols].to_vec(),
            cols: values[cols..2 * cols].to_vec(),
            corner: *values.last().expect("a corner sign"),
        }))
    }

    fn read_sign(
        by_cell: &std::collections::BTreeMap<(i32, i32), Vec<crate::Line>>,
        (row, col): (i32, i32),
        layout: &Layout,
    ) -> Result<Value> {
        let strokes = by_cell
            .get(&(row, col))
            .ok_or_else(|| Error::decode("A tally sign is missing"))?;
        glyphs::read(&layout::upright(strokes, row, col, false, layout))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruby_random::RubyRandom;
    use crate::{cipher, weathering};

    const MESSAGE: &str = "the brothers will gather in the chapter house after compline";

    fn whole() -> Vec<crate::Line> {
        weathering::whole_positions()
    }

    #[test]
    fn tally_signs_are_drawn_apart_and_read_back() {
        for layout in [Layout::Phaistos, Layout::Rongorongo] {
            let mut random = RubyRandom::new(21);
            let sequence = cipher::encode("ora et labora", None, false, None, &mut random).unwrap();
            let svg =
                crate::svg::render(&sequence, &layout, &Default::default(), true, false, None)
                    .unwrap();
            assert_eq!(
                plate::read(&svg, &layout).unwrap(),
                Some(signs(&sequence, &layout).unwrap())
            );
            assert_eq!(cipher::decode(&svg, None, None).unwrap(), "ora et labora");
            assert!(svg.contains("class=\"cord\""));
        }
    }

    #[test]
    fn scattered_whole_signs_come_back_from_their_lines() {
        let mut encoding = RubyRandom::new(8);
        let sequence = cipher::encode(MESSAGE, None, false, None, &mut encoding).unwrap();
        let mut random = RubyRandom::new(4);
        let starts: Vec<usize> = (0..sequence.len()).step_by(2).collect();
        let lost: Vec<usize> = random
            .sample_indices(10, starts.len())
            .into_iter()
            .map(|i| starts[i] + 1)
            .collect();
        let candidates: Vec<Vec<Value>> = sequence
            .iter()
            .enumerate()
            .map(|(index, &value)| {
                if lost.contains(&index) {
                    weathering::candidates(&weathering::surviving(value, &whole()), &whole())
                        .unwrap()
                } else {
                    vec![value]
                }
            })
            .collect();
        let signs = signs(&sequence, &Layout::Phaistos).unwrap();
        let restored = restore(&candidates, &signs, &Layout::Phaistos).unwrap();
        let expected: Vec<Vec<Value>> = sequence.iter().map(|&value| vec![value]).collect();
        assert_eq!(restored, expected);
    }

    #[test]
    fn tallies_that_do_not_add_up_are_no_restoration() {
        let mut encoding = RubyRandom::new(9);
        let sequence = cipher::encode("ora et labora", None, false, None, &mut encoding).unwrap();
        let mut candidates: Vec<Vec<Value>> = sequence.iter().map(|&value| vec![value]).collect();
        candidates[0] =
            weathering::candidates(&weathering::surviving(sequence[0], &whole()), &whole())
                .unwrap();
        let size = sequence.len().isqrt();
        let mut wrong = signs(&sequence, &Layout::Phaistos).unwrap();
        wrong.rows = vec![1111; size];
        wrong.cols = vec![1111; size];
        assert!(restore(&candidates, &wrong, &Layout::Phaistos)
            .unwrap_err()
            .is_decode());
    }

    #[test]
    fn whole_signs_are_lost_only_where_a_tally_gives_them_back() {
        for seed in 0..10i64 {
            let mut encoding = RubyRandom::new(100 + seed);
            let sequence = cipher::encode(MESSAGE, None, false, None, &mut encoding).unwrap();
            let lines = lines(sequence.len().isqrt(), &Layout::Phaistos).unwrap();
            let mut random = RubyRandom::new(seed);
            let damage = weathering::weather(&sequence, &mut random, Some(&lines));
            let wholes: Vec<usize> = damage
                .iter()
                .filter(|(_, lost)| lost.len() == whole().len())
                .map(|(&index, _)| index)
                .collect();
            assert!(!wholes.is_empty(), "seed {seed} loses a whole sign");
            for index in wholes {
                let alone = lines[index].iter().any(|line| {
                    damage
                        .keys()
                        .filter(|&&other| lines[other].contains(line))
                        .count()
                        == 1
                });
                assert!(alone, "sign {index} is not alone on a line (seed {seed})");
            }
        }
    }

    #[test]
    fn no_tally_signs_under_a_grille() {
        let grille = crate::grille::Grille::new("K7Q2M-ZXB4P", crate::grille::Part::Whole).unwrap();
        let layout = Layout::Grille(grille);
        let mut random = RubyRandom::new_os();
        let error =
            cipher::inscribe("ora", None, &layout, true, false, None, &mut random).unwrap_err();
        assert_eq!(error.to_string(), NOT_UNDER_A_GRILLE);
        let mut random = RubyRandom::new_os();
        let error =
            crate::lacunae::inscribe("ora", &layout, None, true, false, &mut random).unwrap_err();
        assert_eq!(error.to_string(), NOT_UNDER_A_GRILLE);
    }
}
