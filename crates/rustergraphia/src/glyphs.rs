//! Geometry of a single Cistercian numeral (0-9999), both ways:
//! number => strokes to draw, and drawn strokes => number.
//!
//! Only the units glyphs are defined; tens, hundreds and thousands are mirror
//! images of them, exactly like the medieval numerals. The stem is taller than
//! two digit cells, so the top and bottom halves never share a stroke and every
//! value is unambiguous.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::error::{Error, Result};
use crate::{Line, Value};

pub const WIDTH: i32 = 100;
pub const HEIGHT: i32 = 150; // Two 50px digit cells with a 50px gap between them
pub const STEM: Line = [50, 0, 50, HEIGHT];

/// The four place values, in the Ruby `PLACES` order.
pub const PLACES: [Value; 4] = [1, 10, 100, 1000];

/// Strokes (x1, y1, x2, y2 relative to the symbol origin) of the units digits,
/// drawn in the top-right cell. Compound digits are made of several strokes.
const UNITS: [(Value, &[Line]); 9] = [
    (1, &[[50, 0, 100, 0]]),
    (2, &[[50, 50, 100, 50]]),
    (3, &[[50, 0, 100, 50]]),
    (4, &[[50, 50, 100, 0]]),
    (5, &[[50, 0, 100, 0], [50, 50, 100, 0]]),
    (6, &[[100, 0, 100, 50]]),
    (7, &[[50, 0, 100, 0], [100, 0, 100, 50]]),
    (8, &[[50, 50, 100, 50], [100, 0, 100, 50]]),
    (9, &[[50, 0, 100, 0], [50, 50, 100, 50], [100, 0, 100, 50]]),
];

/// Tens mirror the units horizontally, hundreds vertically, thousands both ways.
fn mirror(place: Value, (x, y): (i32, i32)) -> (i32, i32) {
    match place {
        1 => (x, y),
        10 => (WIDTH - x, y),
        100 => (x, HEIGHT - y),
        _ => (WIDTH - x, HEIGHT - y),
    }
}

/// Place value (e.g. 300) => list of strokes, built once at first use.
static LINES: LazyLock<HashMap<Value, Vec<Line>>> = LazyLock::new(|| {
    let mut lines = HashMap::new();
    for place in PLACES {
        for (digit, strokes) in UNITS {
            let mirrored: Vec<Line> = strokes
                .iter()
                .map(|&[x1, y1, x2, y2]| {
                    let (mx1, my1) = mirror(place, (x1, y1));
                    let (mx2, my2) = mirror(place, (x2, y2));
                    [mx1, my1, mx2, my2]
                })
                .collect();
            lines.insert(digit * place, mirrored);
        }
    }
    lines
});

/// Place => { sorted canonical strokes => digit }, for reading glyphs back.
static DIGITS: LazyLock<HashMap<Value, HashMap<Vec<Line>, Value>>> = LazyLock::new(|| {
    PLACES
        .into_iter()
        .map(|place| {
            let glyphs = UNITS
                .iter()
                .map(|&(digit, _)| {
                    let mut strokes: Vec<Line> = LINES[&(digit * place)]
                        .iter()
                        .map(|&line| canonical(line))
                        .collect();
                    strokes.sort();
                    (strokes, digit)
                })
                .collect();
            (place, glyphs)
        })
        .collect()
});

/// Canonical stroke => place it belongs to (the four quadrants never share a stroke).
static STROKE_PLACES: LazyLock<HashMap<Line, Value>> = LazyLock::new(|| {
    let mut places = HashMap::new();
    for (place, glyphs) in DIGITS.iter() {
        for strokes in glyphs.keys() {
            for &stroke in strokes {
                places.insert(stroke, *place);
            }
        }
    }
    places
});

/// Place => digit => its strokes (canonical, sorted); 0 has none.
static DIGIT_STROKES: LazyLock<HashMap<Value, HashMap<Value, Vec<Line>>>> = LazyLock::new(|| {
    DIGITS
        .iter()
        .map(|(&place, glyphs)| {
            let mut by_digit: HashMap<Value, Vec<Line>> = glyphs
                .iter()
                .map(|(strokes, &digit)| (digit, strokes.clone()))
                .collect();
            by_digit.insert(0, Vec::new());
            (place, by_digit)
        })
        .collect()
});

/// Strokes are compared regardless of the direction they were drawn in: the
/// two endpoints sorted as (x, y) pairs.
pub fn canonical([x1, y1, x2, y2]: Line) -> Line {
    if (x1, y1) <= (x2, y2) {
        [x1, y1, x2, y2]
    } else {
        [x2, y2, x1, y1]
    }
}

/// The strokes of one place value (Ruby `LINES.fetch`), as drawn (not canonical).
pub fn lines_of(place_value: Value) -> Option<&'static [Line]> {
    LINES.get(&place_value).map(|lines| lines.as_slice())
}

/// The place a canonical stroke belongs to (Ruby `STROKE_PLACES`).
pub fn stroke_place(canonical_line: Line) -> Option<Value> {
    STROKE_PLACES.get(&canonical_line).copied()
}

/// The strokes (canonical, sorted) of one digit in one place (Ruby
/// `DIGIT_STROKES`); digit 0 has none.
pub fn digit_strokes(place: Value, digit: Value) -> &'static [Line] {
    DIGIT_STROKES[&place]
        .get(&digit)
        .map(|strokes| strokes.as_slice())
        .unwrap_or(&[])
}

/// The digit a set of canonical, sorted strokes spells in one place (Ruby
/// `DIGITS.fetch(place).fetch(strokes)`).
pub fn digit_of(place: Value, sorted_canonical: &[Line]) -> Option<Value> {
    DIGITS[&place].get(sorted_canonical).copied()
}

/// The digits a place can hold, for iteration in Ruby `DIGIT_STROKES` order
/// (0 then the nine digits — order does not matter to callers, which filter).
pub fn place_digits() -> impl Iterator<Item = Value> {
    0..=9
}

/// Breaks a number into its individual digits, each multiplied by its place
/// value (most significant first, zeros kept: 1900 => [1000, 900, 0, 0]).
pub fn digits(num: Value) -> Vec<Value> {
    let text = num.to_string();
    let count = text.len();
    text.chars()
        .enumerate()
        .map(|(i, c)| c.to_digit(10).unwrap() * 10u32.pow((count - 1 - i) as u32))
        .collect()
}

/// Absolute strokes of one symbol (stem first) whose top-left corner is (x, y).
pub fn strokes(num: Value, x: i32, y: i32) -> Vec<Line> {
    let mut all = vec![STEM];
    for value in digits(num) {
        if let Some(lines) = LINES.get(&value) {
            all.extend_from_slice(lines);
        }
    }
    all.into_iter()
        .map(|[x1, y1, x2, y2]| [x1 + x, y1 + y, x2 + x, y2 + y])
        .collect()
}

/// A stroke (relative to the symbol origin) turned half round in its cell, as
/// a rongorongo reader sees the line above once the tablet is turned: units
/// trade places with thousands, tens with hundreds.
pub fn turn([x1, y1, x2, y2]: Line) -> Line {
    [WIDTH - x1, HEIGHT - y1, WIDTH - x2, HEIGHT - y2]
}

/// Turns the strokes of one symbol (relative to its origin) back into a number.
pub fn read(strokes: &[Line]) -> Result<Value> {
    let mut strokes: Vec<Line> = strokes.iter().map(|&line| canonical(line)).collect();
    let stem = canonical(STEM);
    let before = strokes.len();
    strokes.retain(|&line| line != stem); // Ruby's delete removes every copy
    if strokes.len() == before {
        return Err(Error::decode("A sign without its stem"));
    }

    let mut by_place: HashMap<Value, Vec<Line>> = HashMap::new();
    for stroke in strokes {
        let place = stroke_place(stroke).ok_or_else(|| {
            Error::decode(format!(
                "Unknown stroke [{}, {}, {}, {}]",
                stroke[0], stroke[1], stroke[2], stroke[3]
            ))
        })?;
        by_place.entry(place).or_default().push(stroke);
    }

    let mut sum = 0;
    for (place, mut place_strokes) in by_place {
        place_strokes.sort();
        let digit =
            digit_of(place, &place_strokes).ok_or_else(|| Error::decode("Unknown glyph"))?;
        sum += digit * place;
    }
    Ok(sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_keep_their_places() {
        assert_eq!(digits(1993), vec![1000, 900, 90, 3]);
        assert_eq!(digits(100), vec![100, 0, 0]);
        assert_eq!(digits(0), vec![0]);
    }

    #[test]
    fn every_number_reads_back() {
        for num in [0, 1, 9, 10, 99, 100, 1000, 1993, 7777, 9000, 9999] {
            let drawn = strokes(num, 0, 0);
            assert_eq!(read(&drawn).unwrap(), num, "{num}");
        }
    }

    #[test]
    fn reading_ignores_stroke_direction_and_order() {
        let mut drawn = strokes(1993, 0, 0);
        drawn.reverse();
        let flipped: Vec<Line> = drawn
            .into_iter()
            .map(|[x1, y1, x2, y2]| [x2, y2, x1, y1])
            .collect();
        assert_eq!(read(&flipped).unwrap(), 1993);
    }

    #[test]
    fn a_sign_needs_its_stem_and_known_strokes() {
        let missing_stem: Vec<Line> = strokes(5, 0, 0).into_iter().skip(1).collect();
        assert!(matches!(read(&missing_stem), Err(Error::Decode(_))));
        let unknown = vec![STEM, [0, 0, 10, 10]];
        let err = read(&unknown).unwrap_err();
        assert_eq!(err.to_string(), "Unknown stroke [0, 0, 10, 10]");
    }

    #[test]
    fn turned_strokes_swap_quadrants() {
        assert_eq!(turn([50, 0, 100, 0]), [50, 150, 0, 150]);
        assert_eq!(canonical([100, 50, 50, 0]), [50, 0, 100, 50]);
    }
}
