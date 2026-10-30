//! The lacunae of a weathered plate, in its SVG: drawn, hidden, read back.
//!
//! - A lacuna is a set of stroke positions that cannot be read, so it is
//!   written as hidden `<line>`s in their own layer (class "lacunae"): they
//!   go through the same cell finding and half turns as the strokes, and
//!   every layout, grilles included, works unchanged
//! - Lost strokes are gone from the wedges and from the hidden strokes alike:
//!   the file never carries its own answer
//! - The worn patches lie under the wedges (strokes that survive at the edge
//!   of a break stay visible), roughened with the sign's own seed so the same
//!   plate always looks the same. The hatch is made of rects: the decoder
//!   reads every `<line>` outside the lacunae layer
//! - On a pathless plate the patches and the lost lines lie in grid order and
//!   are seeded by the cell, as the signs are (pathless): the file tells
//!   nothing of the way, and the reader's bench finds it
//!
//! Game plates only (see weathering).

use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::glyphs;
use crate::layout::{self, Layout, KEYLESS};
use crate::pathless;
use crate::ruby_random::RubyRandom;
use crate::svg::{self, Damage};
use crate::tally;
use crate::weathering::{self, edition};
use crate::{cipher, Line, Value};

pub const CLASS: &str = "lacunae";
pub const HATCH: &str = "#b3a993"; // the edge of a break, as in the pattern below
/// The hatch of a break, made of rects (the decoder reads every `<line>`
/// outside the lacunae).
pub const PATTERN: &str = "<defs><pattern id=\"worn\" width=\"8\" height=\"8\" patternUnits=\"userSpaceOnUse\" patternTransform=\"rotate(45)\"><rect width=\"8\" height=\"8\" fill=\"#ece6da\"/><rect width=\"2\" height=\"8\" fill=\"#b3a993\"/></pattern></defs>";
pub const SEALED: &str = "A worn plate is a game plate: read it without a passphrase";
pub const NOT_SEALED: &str =
    "A sealed plate cannot wear: with one stroke lost, it would never open again";
pub const NOT_TWO_READERS: &str = "A plate for two readers cannot wear yet";

/// A damaged sign as the drawing lays it: the seed of its patches, its lost
/// strokes, whether it lies turned, and its sign's origin on the plate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WornSign {
    pub seed: i64,
    pub lost: Vec<Line>,
    pub turned: bool,
    pub origin: (i32, i32),
}

/// (sequence, svg, damage) for a weathered game plate along a layout. A
/// keyless plate that would also restore along the other layout is drawn
/// again, as `cipher::inscribe` does for intact plates. With tally signs, the
/// plate may lose whole signs, which their row or column gives back.
/// Pathless, the plate says nothing of its way: the bench finds it.
pub fn inscribe(
    message: &str,
    layout: &Layout,
    passphrase: Option<&str>,
    tally_signs: bool,
    pathless_plate: bool,
    random: &mut RubyRandom,
) -> Result<(Vec<Value>, String, Damage)> {
    refuse(passphrase, tally_signs, layout, pathless_plate)?;
    let grille = layout.is_grille();
    for _ in 0..100 {
        let (sequence, damage) = weathered(message, layout, tally_signs, random)?;
        if damage.is_empty() {
            continue; // a very short message may need another draw
        }
        if !grille && !unambiguous(&sequence, &damage, layout, tally_signs)? {
            continue;
        }
        let svg_text = svg::render(
            &sequence,
            layout,
            &damage,
            tally_signs,
            pathless_plate,
            None,
        )?;
        return Ok((sequence, svg_text, damage));
    }
    Err(Error::other(
        "Could not draw an unambiguous plate for this message",
    ))
}

fn refuse(
    passphrase: Option<&str>,
    tally_signs: bool,
    layout: &Layout,
    pathless_plate: bool,
) -> Result<()> {
    if passphrase.is_some() {
        return Err(Error::other(NOT_SEALED));
    }
    if tally_signs && layout.is_grille() {
        return Err(Error::other(tally::NOT_UNDER_A_GRILLE));
    }
    if pathless_plate {
        pathless::check(passphrase, layout.is_grille())?;
    }
    Ok(())
}

/// (sequence, damage) for a message: with tally signs, whole signs may be
/// lost too.
pub fn weathered(
    message: &str,
    layout: &Layout,
    tally_signs: bool,
    random: &mut RubyRandom,
) -> Result<(Vec<Value>, Damage)> {
    let sequence = cipher::encode(message, None, layout.is_grille(), None, random)?;
    let lines = if tally_signs {
        Some(tally::lines(sequence.len().isqrt(), layout)?)
    } else {
        None
    };
    let damage = weathering::weather(&sequence, random, lines.as_deref());
    Ok((sequence, damage))
}

/// Whether the plate restores along its own layout only, checked from the
/// data (parsing the file it would make costs more than all the rest).
fn unambiguous(
    sequence: &[Value],
    damage: &Damage,
    layout: &Layout,
    tally_signs: bool,
) -> Result<bool> {
    for other in layout::others(layout, sequence.len().isqrt()) {
        if along::edition(sequence, damage, layout, &other, tally_signs).is_some() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The restored text of a weathered plate, in the Leiden conventions.
pub fn decode(svg_text: &str, passphrase: Option<&str>, layout: Option<&Layout>) -> Result<String> {
    if passphrase.is_some() {
        return Err(Error::other(SEALED));
    }
    let candidates: Vec<Layout> = match layout {
        Some(layout) => vec![layout.clone()],
        None => KEYLESS.to_vec(),
    };
    let mut errors = Vec::new();
    for candidate in &candidates {
        match restored(svg_text, candidate).and_then(|cells| edition::text(&cells)) {
            Ok(text) => return Ok(text),
            Err(error) if error.is_decode() => errors.push(error),
            Err(error) => return Err(error),
        }
    }
    if matches!(layout, Some(Layout::Grille(_))) {
        return Err(Error::WrongGrille(crate::grille::WRONG_KEY.to_string()));
    }
    Err(errors.into_iter().next().expect("at least one candidate"))
}

/// Whether a plate has lacunae (a string test first: intact plates are not
/// parsed twice).
pub fn is_weathered(svg_text: &str) -> Result<bool> {
    if !svg_text.contains("class=\"lacunae\"") {
        return Ok(false);
    }
    Ok(!svg::lines(svg_text, svg::Layer::Lacunae)?.is_empty())
}

/// (candidate values, damaged?) of every sign, narrowed by the tally signs if
/// the plate has them.
pub fn restored(svg_text: &str, layout: &Layout) -> Result<Vec<(Vec<Value>, bool)>> {
    let cells = read(svg_text, layout)?;
    let Some(tally_signs) = tally::plate::read(svg_text, layout)? else {
        return Ok(cells);
    };
    let values: Vec<Vec<Value>> = cells
        .iter()
        .map(|(candidates, _)| candidates.clone())
        .collect();
    let restored = tally::restore(&values, &tally_signs, layout)?;
    Ok(restored
        .into_iter()
        .zip(cells.into_iter().map(|(_, worn)| worn))
        .collect())
}

/// (candidate values, damaged?) of every sign, in reading order.
pub fn read(svg_text: &str, layout: &Layout) -> Result<Vec<(Vec<Value>, bool)>> {
    let strokes = cells(svg_text, svg::Layer::Strokes, layout)?;
    let lost = cells(svg_text, svg::Layer::Lacunae, layout)?;
    let empty = Vec::new();
    layout::path(svg::grid_size(strokes.len())?, layout)?
        .into_iter()
        .map(|(row, col, turned)| {
            let visible = strokes.get(&(row, col)).ok_or_else(|| {
                Error::decode(format!("Missing symbol at row {row}, column {col}"))
            })?;
            let gone = layout::upright(
                lost.get(&(row, col)).unwrap_or(&empty),
                row,
                col,
                turned,
                layout,
            );
            let seen = layout::upright(visible, row, col, turned, layout);
            Ok((weathering::candidates(&seen, &gone)?, !gone.is_empty()))
        })
        .collect()
}

/// A layer's lines, by grid cell.
pub fn cells(
    svg_text: &str,
    layer: svg::Layer,
    layout: &Layout,
) -> Result<BTreeMap<(i32, i32), Vec<Line>>> {
    let mut by_cell: BTreeMap<(i32, i32), Vec<Line>> = BTreeMap::new();
    for line in svg::lines(svg_text, layer)? {
        by_cell
            .entry(layout::cell_of(line, layout))
            .or_default()
            .push(line);
    }
    Ok(by_cell)
}

/// A sign's strokes, without those a lacuna took.
pub fn strokes(num: Value, lost: &[Line]) -> Vec<Line> {
    glyphs::strokes(num, 0, 0)
        .into_iter()
        .filter(|stroke| !lost.contains(&glyphs::canonical(*stroke)))
        .collect()
}

/// The worn patches, drawn under the wedges ('' for an intact plate).
pub fn patches(cols: usize, layout: &Layout, damage: &Damage, pathless: bool) -> Result<String> {
    if damage.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(
        "{PATTERN}\n{}",
        worn(&signs(cols, layout, damage, pathless)?)
    ))
}

/// The worn patches of some signs, filled with the PATTERN the document
/// defines once.
pub fn worn(signs: &[WornSign]) -> String {
    let polygons: Vec<String> = signs
        .iter()
        .flat_map(|sign| {
            let mut random = RubyRandom::new(sign.seed);
            outline::shapes(&sign.lost)
                .into_iter()
                .map(|shape| {
                    let points: Vec<(f64, f64)> = outline::rough(&shape, &mut random)
                        .into_iter()
                        .map(|point| outline::place(point, sign.turned, sign.origin))
                        .collect();
                    outline::polygon(&points)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    format!(
        "  <g class=\"weathering\" fill=\"url(#worn)\" stroke=\"{HATCH}\" stroke-width=\"1.5\">\n{}\n  </g>",
        polygons.join("\n")
    )
}

/// The lost stroke positions as hidden lines: what the decoder reads.
pub fn hidden(cols: usize, layout: &Layout, damage: &Damage, pathless: bool) -> Result<String> {
    if damage.is_empty() {
        return Ok(String::new());
    }
    Ok(lost_lines(&signs(cols, layout, damage, pathless)?))
}

/// The lost stroke positions of some signs as hidden lines, in their own
/// layer.
pub fn lost_lines(signs: &[WornSign]) -> String {
    let lines: Vec<String> = signs
        .iter()
        .flat_map(|sign| {
            let (x, y) = sign.origin;
            sign.lost.iter().map(move |&stroke| {
                let stroke = if sign.turned {
                    glyphs::turn(stroke)
                } else {
                    stroke
                };
                svg::line_tag(layout::relative(stroke, -x, -y))
            })
        })
        .collect();
    format!(
        "  <g class=\"{CLASS}\" stroke=\"black\" stroke-width=\"2\" display=\"none\">\n{}\n  </g>",
        lines.join("\n")
    )
}

/// Every damaged sign, in reading order and seeded by the sign's place in it;
/// on a pathless plate in grid order, each seeded by its cell.
pub fn signs(
    cols: usize,
    layout: &Layout,
    damage: &Damage,
    pathless_plate: bool,
) -> Result<Vec<WornSign>> {
    let mut worn: Vec<WornSign> = layout::path(cols, layout)?
        .into_iter()
        .enumerate()
        .filter_map(|(index, (row, col, turned))| {
            damage.get(&index).map(|lost| WornSign {
                seed: if pathless_plate {
                    pathless::seed(row, col, cols)
                } else {
                    index as i64
                },
                lost: lost.clone(),
                turned,
                origin: layout::origin(row, col, layout),
            })
        })
        .collect();
    if pathless_plate {
        worn.sort_by_key(|sign| sign.seed);
    }
    Ok(worn)
}

/// A weathered plate read along another layout, from the data: what the
/// decoder would find reading its file that way (the same signs, turned
/// where the two paths disagree), without drawing or parsing the file.
pub mod along {
    use super::*;

    /// The edition the decoder would print, or None if the plate does not
    /// restore that way.
    pub fn edition(
        sequence: &[Value],
        damage: &Damage,
        layout: &Layout,
        other: &Layout,
        tally_signs: bool,
    ) -> Option<String> {
        let mut read = cells(sequence, damage, layout, other).ok()?;
        if tally_signs {
            let values: Vec<Vec<Value>> = read.iter().map(|(c, _)| c.clone()).collect();
            let signs = tally::signs(sequence, layout).ok()?;
            let restored = tally::restore(&values, &signs, other).ok()?;
            read = restored
                .into_iter()
                .zip(read.into_iter().map(|(_, worn)| worn))
                .collect();
        }
        edition::text(&read).ok()
    }

    /// (candidate values, damaged?) of every sign as the other layout reads
    /// them.
    fn cells(
        sequence: &[Value],
        damage: &Damage,
        layout: &Layout,
        other: &Layout,
    ) -> Result<Vec<(Vec<Value>, bool)>> {
        let size = sequence.len().isqrt();
        let drawn: BTreeMap<(i32, i32), (usize, bool)> = layout::path(size, layout)?
            .into_iter()
            .enumerate()
            .map(|(index, (row, col, turned))| ((row, col), (index, turned)))
            .collect();
        let empty = Vec::new();
        layout::path(size, other)?
            .into_iter()
            .map(|(row, col, turned)| {
                let (index, was) = drawn[&(row, col)];
                let lost = damage.get(&index).unwrap_or(&empty);
                sign(sequence[index], lost, was != turned)
            })
            .collect()
    }

    /// One sign's (candidates, damaged?), its strokes turned half round if
    /// the other path reads it so.
    fn sign(num: Value, lost: &[Line], turned: bool) -> Result<(Vec<Value>, bool)> {
        let turn = |strokes: Vec<Line>| -> Vec<Line> {
            if turned {
                strokes.into_iter().map(glyphs::turn).collect()
            } else {
                strokes
            }
        };
        let seen = turn(weathering::surviving(num, lost));
        let gone = turn(lost.to_vec());
        Ok((weathering::candidates(&seen, &gone)?, !gone.is_empty()))
    }
}

/// What a weathered plate's file shows of itself, for a page that must hold
/// no more than the file (the reader's): the layout it reads along, its
/// damaged signs with their lost positions and the values their surviving
/// strokes allow, and every sign's value where no lacuna leaves it open.
pub mod survey {
    use super::*;

    /// The keyless layout along which the plate restores, or None.
    pub fn layout(svg_text: &str) -> Result<Option<Layout>> {
        Ok(KEYLESS
            .into_iter()
            .find(|layout| edition(svg_text, layout).is_some()))
    }

    pub fn edition(svg_text: &str, layout: &Layout) -> Option<String> {
        restored(svg_text, layout)
            .and_then(|cells| crate::weathering::edition::text(&cells))
            .ok()
    }

    /// One damaged sign in reading order: its lost stroke positions
    /// (canonical, upright) and what it can still be.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Damaged {
        pub index: usize,
        pub lost: Vec<Line>,
        pub candidates: Vec<Value>,
    }

    /// Every damaged sign in reading order.
    pub fn damaged(svg_text: &str, layout: &Layout) -> Result<Vec<Damaged>> {
        let strokes = cells(svg_text, svg::Layer::Strokes, layout)?;
        let lost = cells(svg_text, svg::Layer::Lacunae, layout)?;
        layout::path(svg::grid_size(strokes.len())?, layout)?
            .into_iter()
            .enumerate()
            .filter_map(|(index, (row, col, turned))| {
                let gone = lost.get(&(row, col))?;
                let result = (|| {
                    let visible = strokes.get(&(row, col)).ok_or_else(|| {
                        Error::decode(format!("Missing symbol at row {row}, column {col}"))
                    })?;
                    let seen = layout::upright(visible, row, col, turned, layout);
                    let gone: Vec<Line> = layout::upright(gone, row, col, turned, layout)
                        .into_iter()
                        .map(glyphs::canonical)
                        .collect();
                    Ok(Damaged {
                        index,
                        candidates: weathering::candidates(&seen, &gone)?,
                        lost: gone,
                    })
                })();
                Some(result)
            })
            .collect()
    }

    /// Every sign's value as the file shows it: None for a sign a lacuna
    /// touched.
    pub fn values(svg_text: &str, layout: &Layout) -> Result<Vec<Option<Value>>> {
        Ok(read(svg_text, layout)?
            .into_iter()
            .map(|(candidates, worn)| {
                if worn {
                    None
                } else {
                    candidates.first().copied()
                }
            })
            .collect())
    }

    /// What the file shows of a worn sign one way up: its lost positions
    /// (canonical) and its candidate values.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WornReading {
        pub lost: Vec<Line>,
        pub candidates: Vec<Value>,
    }

    /// One worn cell: its grid cell, and what the file shows of it read
    /// upright and turned half round (None where its strokes read as no sign
    /// so turned).
    pub type WornCell = (i32, i32, Option<WornReading>, Option<WornReading>);

    /// Every worn cell of a pathless plate, in grid order, read upright and
    /// turned half round. What a bench needs to lay any of the five ways
    /// over the grid.
    pub fn worn_cells(svg_text: &str) -> Result<Vec<WornCell>> {
        let strokes = cells(svg_text, svg::Layer::Strokes, &Layout::Phaistos)?;
        cells(svg_text, svg::Layer::Lacunae, &Layout::Phaistos)?
            .into_iter()
            .map(|((row, col), gone)| {
                let seen = strokes.get(&(row, col)).ok_or_else(|| {
                    Error::decode(format!("Missing symbol at row {row}, column {col}"))
                })?;
                let readings: Vec<Option<WornReading>> = [false, true]
                    .into_iter()
                    .map(|turned| worn_reading(seen, &gone, row, col, turned))
                    .collect();
                let mut readings = readings.into_iter();
                Ok((
                    row,
                    col,
                    readings.next().flatten(),
                    readings.next().flatten(),
                ))
            })
            .collect()
    }

    fn worn_reading(
        seen: &[Line],
        gone: &[Line],
        row: i32,
        col: i32,
        turned: bool,
    ) -> Option<WornReading> {
        let seen = layout::upright(seen, row, col, turned, &Layout::Phaistos);
        let gone: Vec<Line> = layout::upright(gone, row, col, turned, &Layout::Phaistos)
            .into_iter()
            .map(glyphs::canonical)
            .collect();
        let candidates = weathering::candidates(&seen, &gone).ok()?;
        Some(WornReading {
            lost: gone,
            candidates,
        })
    }
}

/// The shapes of a break, drawn in a sign's upright frame and placed on the
/// plate.
pub mod outline {
    use super::*;

    pub const BAND: f64 = 7.0; // half width of a worn band along one stroke
    pub const JITTER: i64 = 3; // how ragged a break's edge is

    /// The outlines to wear: a whole quadrant as its box, otherwise a band
    /// along each lost stroke (grouped by quadrant, in the order the lost
    /// strokes first touch each).
    pub fn shapes(lost: &[Line]) -> Vec<Vec<(f64, f64)>> {
        let mut places: Vec<(Value, Vec<Line>)> = Vec::new();
        for &stroke in lost {
            let place = glyphs::stroke_place(stroke).expect("a lost stroke is a position");
            match places.iter_mut().find(|(p, _)| *p == place) {
                Some((_, strokes)) => strokes.push(stroke),
                None => places.push((place, vec![stroke])),
            }
        }
        places
            .into_iter()
            .flat_map(|(place, strokes)| {
                if strokes.len() == weathering::positions(place).len() {
                    return vec![quadrant_box(place)];
                }
                strokes.into_iter().map(band).collect()
            })
            .collect()
    }

    /// A whole quadrant as its box, a little wider than its strokes.
    fn quadrant_box(place: Value) -> Vec<(f64, f64)> {
        let strokes = weathering::positions(place);
        let xs: Vec<i32> = strokes
            .iter()
            .flat_map(|&[x1, _, x2, _]| [x1, x2])
            .collect();
        let ys: Vec<i32> = strokes
            .iter()
            .flat_map(|&[_, y1, _, y2]| [y1, y2])
            .collect();
        let (x1, x2) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
        let (y1, y2) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
        [
            (x1 - 5, y1 - 5),
            (x2 + 5, y1 - 5),
            (x2 + 5, y2 + 5),
            (x1 - 5, y2 + 5),
        ]
        .into_iter()
        .map(|(x, y)| (f64::from(x), f64::from(y)))
        .collect()
    }

    /// A rectangle around a stroke, a little longer than it.
    fn band([x1, y1, x2, y2]: Line) -> Vec<(f64, f64)> {
        let (dx, dy) = unit(f64::from(x2 - x1), f64::from(y2 - y1));
        let ends = [
            shift((f64::from(x1), f64::from(y1)), (dx, dy), -4.0),
            shift((f64::from(x2), f64::from(y2)), (dx, dy), 4.0),
        ];
        [(0, 1.0), (1, 1.0), (1, -1.0), (0, -1.0)]
            .into_iter()
            .map(|(end, side)| shift(ends[end], (-dy * BAND, dx * BAND), side))
            .collect()
    }

    /// The direction of a stroke, as a vector of length 1.
    fn unit(width: f64, height: f64) -> (f64, f64) {
        let length = width.hypot(height);
        (width / length, height / length)
    }

    fn shift((x, y): (f64, f64), (dx, dy): (f64, f64), amount: f64) -> (f64, f64) {
        (x + dx * amount, y + dy * amount)
    }

    /// A broken edge: each side cut in three, every point moved a little.
    pub fn rough(corners: &[(f64, f64)], random: &mut RubyRandom) -> Vec<(f64, f64)> {
        let mut points = Vec::new();
        for (i, &a) in corners.iter().enumerate() {
            let b = corners[(i + 1) % corners.len()];
            points.extend(thirds(a, b));
        }
        points
            .into_iter()
            .map(|(x, y)| {
                let jx = random.rand_int_incl(-JITTER, JITTER) as f64;
                let jy = random.rand_int_incl(-JITTER, JITTER) as f64;
                (x + jx, y + jy)
            })
            .collect()
    }

    fn thirds((ax, ay): (f64, f64), (bx, by): (f64, f64)) -> Vec<(f64, f64)> {
        (0..=2)
            .map(|k| {
                let k = f64::from(k);
                (ax + (bx - ax) * k / 3.0, ay + (by - ay) * k / 3.0)
            })
            .collect()
    }

    /// An upright point of a sign, on the plate (turned with its sign if need
    /// be).
    pub fn place((x, y): (f64, f64), turned: bool, (ox, oy): (i32, i32)) -> (f64, f64) {
        let (x, y) = if turned {
            (f64::from(glyphs::WIDTH) - x, f64::from(glyphs::HEIGHT) - y)
        } else {
            (x, y)
        };
        (x + f64::from(ox), y + f64::from(oy))
    }

    pub fn polygon(points: &[(f64, f64)]) -> String {
        let listed: Vec<String> = points
            .iter()
            .map(|(x, y)| format!("{x:.1},{y:.1}"))
            .collect();
        format!("    <polygon points=\"{}\"/>", listed.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grille::{Grille, Part};

    #[test]
    fn the_file_never_carries_what_was_lost() {
        let mut random = RubyRandom::new(31);
        let (sequence, svg_text, damage) = inscribe(
            "the brothers will gather",
            &Layout::Phaistos,
            None,
            false,
            false,
            &mut random,
        )
        .unwrap();
        let lost = svg::lines(&svg_text, svg::Layer::Lacunae).unwrap();
        let strokes = svg::lines(&svg_text, svg::Layer::Strokes).unwrap();
        assert_eq!(
            damage.values().map(Vec::len).sum::<usize>(),
            lost.len(),
            "every lost position is hidden in its own layer"
        );
        assert!(lost.iter().all(|line| !strokes.contains(line)));
        let taken: usize = damage
            .iter()
            .map(|(&index, gone)| {
                glyphs::strokes(sequence[index], 0, 0)
                    .into_iter()
                    .map(glyphs::canonical)
                    .filter(|stroke| gone.contains(stroke))
                    .count()
            })
            .sum();
        let inked: usize = sequence
            .iter()
            .map(|&num| glyphs::strokes(num, 0, 0).len())
            .sum();
        assert_eq!(inked - taken, strokes.len());
        assert!(svg_text.contains("class=\"weathering\""));
        let mut cells: Vec<(i32, i32)> = strokes
            .iter()
            .map(|&line| layout::cell_of(line, &Layout::Phaistos))
            .collect();
        cells.sort();
        cells.dedup();
        assert_eq!(cells.len(), sequence.len(), "every stem survives");
    }

    #[test]
    fn even_one_letter_weathers() {
        let mut random = RubyRandom::new(17);
        let (_, svg_text, damage) =
            inscribe("a", &Layout::Phaistos, None, false, false, &mut random).unwrap();
        assert!(!damage.is_empty());
        let edition = cipher::decode(&svg_text, None, None).unwrap();
        let first = edition.replace(['[', ']'], "");
        assert_eq!(first.split('/').next().unwrap(), "a");
    }

    #[test]
    fn an_intact_plate_reads_as_before() {
        let mut random = RubyRandom::new(23);
        let (_, svg_text) = cipher::inscribe(
            "ora et labora",
            None,
            &Layout::Phaistos,
            false,
            false,
            None,
            &mut random,
        )
        .unwrap();
        assert!(!is_weathered(&svg_text).unwrap());
        assert!(!svg_text.contains("weathering"));
        assert_eq!(
            cipher::decode(&svg_text, None, None).unwrap(),
            "ora et labora"
        );
    }

    #[test]
    fn sealed_plates_do_not_weather() {
        let mut random = RubyRandom::new_os();
        let error = inscribe(
            "relics",
            &Layout::Phaistos,
            Some("abbey of molesme"),
            false,
            false,
            &mut random,
        )
        .unwrap_err();
        assert_eq!(error.to_string(), NOT_SEALED);

        let mut random = RubyRandom::new(29);
        let (_, svg_text, _) =
            inscribe("relics", &Layout::Phaistos, None, false, false, &mut random).unwrap();
        let error = cipher::decode(&svg_text, Some("abbey of molesme"), None).unwrap_err();
        assert_eq!(error.to_string(), SEALED);
    }

    #[test]
    fn a_weathered_grille_plate_needs_its_key() {
        let grille = Grille::new("K7Q2M-ZXB4P", Part::Whole).unwrap();
        let mut random = RubyRandom::new(37);
        let (_, svg_text, _) = inscribe(
            "ora et labora",
            &Layout::Grille(grille),
            None,
            false,
            false,
            &mut random,
        )
        .unwrap();
        assert!(matches!(
            cipher::decode(&svg_text, None, None),
            Err(Error::GrilleKeyNeeded(_))
        ));
        let wrong = Layout::Grille(Grille::new("00000-00000", Part::Whole).unwrap());
        assert!(matches!(
            cipher::decode(&svg_text, None, Some(&wrong)),
            Err(Error::WrongGrille(_))
        ));
    }

    #[test]
    fn a_pathless_plate_cannot_be_sealed_or_under_a_grille_even_worn() {
        let grille = Layout::Grille(Grille::new("K7Q2M-ZXB4P", Part::Whole).unwrap());
        let mut random = RubyRandom::new_os();
        assert!(inscribe("relics", &grille, None, false, true, &mut random).is_err());
        let mut random = RubyRandom::new_os();
        let error = inscribe(
            "relics",
            &Layout::Phaistos,
            Some("abbey"),
            false,
            true,
            &mut random,
        )
        .unwrap_err();
        assert_eq!(error.to_string(), NOT_SEALED);
    }
}
