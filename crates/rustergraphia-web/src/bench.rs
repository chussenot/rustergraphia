//! The restoration bench under a weathered plate (web/bench_helpers.rb):
//! what the page needs to let a reader press lost digits back in, and the
//! bench's first state, drawn by the server exactly as the script draws it.
//!
//! The page only ever holds what the file shows: where each lacuna is, its
//! lost stroke positions, the digits its surviving strokes allow, the digits
//! of its sign that are not lost, and the tally signs. Never a lost value.

use std::collections::BTreeMap;

use rustergraphia::layout::{self, Layout};
use rustergraphia::svg::Damage;
use rustergraphia::{glyphs, lacunae, weathering, Line, Value, EVIL_MARKER};

use crate::esc::pad4;

pub fn place_name(place: Value) -> &'static str {
    match place {
        1 => "units",
        10 => "tens",
        100 => "hundreds",
        1000 => "thousands",
        _ => unreachable!("a glyph place"),
    }
}

/// One lacuna entry as the page's JSON holds it (key order sign, x, y,
/// turned, place, lost, digits, known, open).
#[derive(Debug, Clone)]
pub struct Entry {
    pub sign: usize,
    pub x: i32,
    pub y: i32,
    pub turned: bool,
    pub place: Value,
    pub lost: Vec<String>,
    pub digits: Vec<Value>,
    pub known: Value,
    pub open: Vec<Value>,
}

impl Entry {
    pub fn json(&self) -> String {
        format!(
            "{{\"sign\":{},\"x\":{},\"y\":{},\"turned\":{},\"place\":{},\"lost\":[{}],\"digits\":[{}],\"known\":{},\"open\":[{}]}}",
            self.sign,
            self.x,
            self.y,
            self.turned,
            self.place,
            self.lost
                .iter()
                .map(|s| crate::esc::json_str(s))
                .collect::<Vec<_>>()
                .join(","),
            join_values(&self.digits),
            self.known,
            join_values(&self.open)
        )
    }
}

fn join_values(values: &[Value]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

pub fn entries_json(entries: &[Entry]) -> String {
    format!(
        "[{}]",
        entries
            .iter()
            .map(Entry::json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// One entry per damaged place of a sign (a sign wholly lost has four), in
/// reading order, from the plate's data (the scribe's page).
pub fn lacunae_entries(sequence: &[Value], layout: &Layout, damage: &Damage) -> Vec<Entry> {
    let path = layout::path(sequence.len().isqrt(), layout).expect("a plate's own path");
    damage
        .iter()
        .flat_map(|(&index, lost)| {
            let options = options(sequence[index], lost);
            sign_entries(&path, layout, index, lost, &options)
        })
        .collect()
}

/// The same entries from the plate's file alone (the reader's page).
pub fn lacunae_in(
    svg: &str,
    layout: &Layout,
    values: &[Option<Value>],
) -> rustergraphia::Result<Vec<Entry>> {
    let path = layout::path(values.len().isqrt(), layout).expect("a plate's own path");
    Ok(lacunae::survey::damaged(svg, layout)?
        .into_iter()
        .flat_map(|sign| sign_entries(&path, layout, sign.index, &sign.lost, &sign.candidates))
        .collect())
}

/// The same entries from a plate rejoined (Fragments::Whole).
pub fn lacunae_from(
    values_len: usize,
    layout: &Layout,
    damage: &Damage,
    candidates: &BTreeMap<usize, Vec<Value>>,
) -> Vec<Entry> {
    let path = layout::path(values_len.isqrt(), layout).expect("a plate's own path");
    damage
        .iter()
        .flat_map(|(&index, lost)| {
            sign_entries(
                &path,
                layout,
                index,
                lost,
                candidates.get(&index).expect("a worn sign's candidates"),
            )
        })
        .collect()
}

/// A worn cell's entries by place, or None where it reads as no sign.
pub type CellEntries = Option<Vec<PlaceEntry>>;

/// A pathless worn plate's worn cells: each in grid order with its lacunae
/// read upright and turned half round.
pub struct WornCell {
    pub row: i32,
    pub col: i32,
    pub upright: CellEntries,
    pub turned: CellEntries,
}

impl WornCell {
    fn json(&self) -> String {
        format!(
            "{{\"row\":{},\"col\":{},\"upright\":{},\"turned\":{}}}",
            self.row,
            self.col,
            entries_side(&self.upright),
            entries_side(&self.turned)
        )
    }
}

fn entries_side(side: &CellEntries) -> String {
    match side {
        None => "null".to_string(),
        Some(entries) => format!(
            "[{}]",
            entries
                .iter()
                .map(PlaceEntry::json)
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

pub fn worn_cells_json(cells: &[WornCell]) -> String {
    format!(
        "[{}]",
        cells
            .iter()
            .map(WornCell::json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub fn worn_cells(svg: &str) -> rustergraphia::Result<Vec<WornCell>> {
    Ok(lacunae::survey::worn_cells(svg)?
        .into_iter()
        .map(|(row, col, upright, turned)| WornCell {
            row,
            col,
            upright: upright.map(|reading| by_place(&reading.candidates, &reading.lost)),
            turned: turned.map(|reading| by_place(&reading.candidates, &reading.lost)),
        })
        .collect())
}

/// How many lacunae a pathless plate's bench will show along any way.
pub fn pathless_lacunae(worn: &[WornCell]) -> usize {
    worn.iter()
        .map(|cell| {
            cell.upright
                .as_ref()
                .or(cell.turned.as_ref())
                .map_or(0, Vec::len)
        })
        .sum()
}

fn sign_entries(
    path: &[(i32, i32, bool)],
    layout: &Layout,
    index: usize,
    lost: &[Line],
    options: &[Value],
) -> Vec<Entry> {
    let (row, col, turned) = path[index];
    let (x, y) = layout::origin(row, col, layout);
    by_place(options, lost)
        .into_iter()
        .map(|entry| Entry {
            sign: index,
            x,
            y,
            turned,
            place: entry.place,
            lost: entry.lost,
            digits: entry.digits,
            known: entry.known,
            open: entry.open,
        })
        .collect()
}

/// Each damaged place of a sign, before the sign's own coordinates join it.
#[derive(Debug, Clone)]
pub struct PlaceEntry {
    pub place: Value,
    pub lost: Vec<String>,
    pub digits: Vec<Value>,
    pub known: Value,
    pub open: Vec<Value>,
}

impl PlaceEntry {
    fn json(&self) -> String {
        format!(
            "{{\"place\":{},\"lost\":[{}],\"digits\":[{}],\"known\":{},\"open\":[{}]}}",
            self.place,
            self.lost
                .iter()
                .map(|s| crate::esc::json_str(s))
                .collect::<Vec<_>>()
                .join(","),
            join_values(&self.digits),
            self.known,
            join_values(&self.open)
        )
    }
}

/// Each damaged place of a sign: its lost strokes (grouped in first-seen
/// order, listed by place descending), the digits it can still hold, the
/// sign's digits that survive and the open places.
pub fn by_place(options: &[Value], lost: &[Line]) -> Vec<PlaceEntry> {
    let mut groups: Vec<(Value, Vec<Line>)> = Vec::new();
    for &stroke in lost {
        let place = glyphs::stroke_place(stroke).expect("a lost stroke is a stroke position");
        match groups.iter_mut().find(|(p, _)| *p == place) {
            Some((_, strokes)) => strokes.push(stroke),
            None => groups.push((place, vec![stroke])),
        }
    }
    let mut open: Vec<Value> = groups.iter().map(|(place, _)| *place).collect();
    open.sort_unstable();
    let known = without(options.first().copied().unwrap_or(0), &open);
    let mut sorted = groups;
    sorted.sort_by_key(|(place, _)| std::cmp::Reverse(*place)); // places descending
    sorted
        .into_iter()
        .map(|(place, strokes)| PlaceEntry {
            place,
            lost: strokes
                .iter()
                .map(|stroke| stroke.map(|n| n.to_string()).join(","))
                .collect(),
            digits: digits_at(options, place),
            known,
            open: open.clone(),
        })
        .collect()
}

/// The values a damaged sign can be, from its surviving strokes alone.
pub fn options(value: Value, lost: &[Line]) -> Vec<Value> {
    weathering::candidates(&weathering::surviving(value, lost), lost)
        .expect("a damaged sign still reads")
}

/// A value with these places' digits taken out.
fn without(value: Value, places: &[Value]) -> Value {
    value
        - places
            .iter()
            .map(|place| (value / place % 10) * place)
            .sum::<Value>()
}

fn digits_at(values: &[Value], place: Value) -> Vec<Value> {
    let mut digits: Vec<Value> = values.iter().map(|value| value / place % 10).collect();
    digits.sort_unstable();
    digits.dedup();
    digits
}

/// A lacuna's pair as the bench first shows it: "1?34 + 6234 = ?".
pub fn bench_sum(values: &[Option<Value>], entries: &[Entry], lacuna: &Entry) -> String {
    let sign = lacuna.sign;
    let first = sign - sign % 2;
    let shown: Vec<String> = [first, first + 1]
        .iter()
        .map(
            |&index| match entries.iter().find(|entry| entry.sign == index) {
                Some(worn) => open_digits(worn),
                None => pad4(values[index].expect("an intact sign has its value")),
            },
        )
        .collect();
    format!("{} = ?", shown.join(" + "))
}

/// A damaged sign's digits as far as they can be seen: '?' in its open places.
pub fn open_digits(lacuna: &Entry) -> String {
    let mut digits: Vec<char> = pad4(lacuna.known).chars().collect();
    for &place in &lacuna.open {
        let position = 3 - (f64::from(place).log10().round() as usize);
        digits[position] = '?';
    }
    digits.into_iter().collect()
}

/// The bench's first verdict: a null pair needs no digit.
pub fn bench_verdict(values: &[Option<Value>], lacuna: &Entry) -> &'static str {
    let null = lacuna.sign.is_multiple_of(2)
        && values.get(lacuna.sign + 1).copied().flatten() == Some(EVIL_MARKER);
    if null {
        "A null pair: any digit."
    } else {
        "Press a digit to balance it."
    }
}

/// The tallies of a lacuna's row and column, in its place, open.
pub fn bench_tallies(lacuna: &Entry) -> String {
    format!("Tallies, {}: row ?, column ?", place_name(lacuna.place))
}

/// The bench's edition before anything is restored: null pairs (an intact
/// 9999) left out, [.] for a pair a lacuna took part of, letters for the rest.
pub fn bench_edition(values: &[Option<Value>], damage: &Damage) -> String {
    values
        .chunks(2)
        .enumerate()
        .filter_map(|(pair, signs)| {
            let first = signs.first().copied().flatten();
            let second = signs.get(1).copied().flatten();
            if second == Some(EVIL_MARKER) && !damage.contains_key(&(2 * pair + 1)) {
                return None;
            }
            if damage.contains_key(&(2 * pair)) || damage.contains_key(&(2 * pair + 1)) {
                return Some("[.]".to_string());
            }
            let sum = first.unwrap_or(0) + second.unwrap_or(0);
            rustergraphia::character_of(sum).map(|c| c.to_string())
        })
        .collect()
}
