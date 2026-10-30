//! The hand that presses a plate. Every stroke becomes a wedge, as a stylus
//! leaves it in clay: a notched triangular head where it bites, then a tail
//! that thins as it lifts. The ductus is a right-handed scribe's (left to
//! right, verticals top-down), so every head sits on the left or on top, as on
//! a cuneiform tablet. A seeded unevenness (angle, length, head) makes it a
//! hand, and the same plate is always pressed the same way.
//!
//! Only the look: plates keep their strokes as lines for the decoder (see svg).
//!
//! A plate's hand is seeded by each sign's place in it. The page's own signs
//! (the key, the tables, the preview) share another hand, seeded by each
//! stroke (`press`): a stroke always looks the same there, so a sign that
//! changes keeps the strokes it shares.

use std::f64::consts::PI;

use crate::ruby_random::RubyRandom;
use crate::{glyphs, Line};

pub const HEAD: f64 = 12.0; // width of the head's back face, in glyph units (a sign is 100 x 150)
pub const HEAD_LENGTH: f64 = 14.0; // from the back of the head to its point
pub const TAIL: f64 = 6.0; // width of the tail where it leaves the head
pub const NOTCH: f64 = 0.18; // depth of the bite in the head's back, relative to its width
pub const VARIATION: f64 = 6.0; // the unevenness of the hand: up to ±6 degrees, a few percent

/// One stroke pressed by the page's hand, as SVG path data.
pub fn press(stroke: Line) -> String {
    let seed = glyphs::canonical(stroke)
        .iter()
        .fold(0i64, |acc, &value| acc * 1000 + i64::from(value));
    wedge(stroke, &mut RubyRandom::new(seed))
        .iter()
        .map(|polygon| outline(polygon, 0.0, 0.0))
        .collect()
}

/// A closed polygon as SVG path data, moved by (x, y), with one decimal.
pub fn outline(points: &[(f64, f64)], x: f64, y: f64) -> String {
    let joined = points
        .iter()
        .map(|&(px, py)| format!("{} {}", round1(px + x), round1(py + y)))
        .collect::<Vec<_>>()
        .join("L");
    format!("M{joined}Z")
}

/// The outlines of one stroke (x1, y1, x2, y2), pressed: the head and the
/// tail, each a closed polygon of (x, y) points. The order of the points is
/// read back by public/js/ink.js to draw the wedge: the head's back corners
/// come first and third (the stroke's start lies between them), the tail's
/// tip second in the tail (its end).
pub fn wedge(stroke: Line, random: &mut RubyRandom) -> [Vec<(f64, f64)>; 2] {
    let [x1, y1, x2, y2] = ductus(stroke);
    let (angle, length, width, head_length) = hand(f64::from(x2 - x1), f64::from(y2 - y1), random);
    let start = (f64::from(x1), f64::from(y1));
    let along = (angle.cos(), angle.sin());
    [
        head(start, along, width, head_length.min(length * 0.6)),
        tail(start, along, width, length),
    ]
}

/// Left to right; verticals top-down.
pub fn ductus([x1, y1, x2, y2]: Line) -> Line {
    if x1 > x2 || (x1 == x2 && y1 > y2) {
        [x2, y2, x1, y1]
    } else {
        [x1, y1, x2, y2]
    }
}

/// The stroke as this hand presses it: its angle and length, slightly
/// uneven, and the size of the stylus' head.
fn hand(delta_x: f64, delta_y: f64, random: &mut RubyRandom) -> (f64, f64, f64, f64) {
    (
        delta_y.atan2(delta_x) + wobble(random) * VARIATION * PI / 90.0,
        uneven(delta_x.hypot(delta_y), 0.012, random),
        uneven(HEAD, 0.05, random),
        uneven(HEAD_LENGTH, 0.05, random),
    )
}

/// A size, a few percent off (spread per unit of VARIATION).
fn uneven(value: f64, spread: f64, random: &mut RubyRandom) -> f64 {
    value * (1.0 + wobble(random) * VARIATION * spread)
}

fn wobble(random: &mut RubyRandom) -> f64 {
    random.rand_float() - 0.5
}

/// The head: its two back corners, its point along the stroke, the notch.
fn head(start: (f64, f64), along: (f64, f64), width: f64, length: f64) -> Vec<(f64, f64)> {
    let side = across(along);
    vec![
        at(start, side, width / 2.0),
        at(start, along, length),
        at(start, side, -width / 2.0),
        at(start, along, width * NOTCH),
    ]
}

/// The tail: from inside the head to the end of the stroke.
fn tail(start: (f64, f64), along: (f64, f64), width: f64, length: f64) -> Vec<(f64, f64)> {
    let root = at(start, along, width * NOTCH);
    vec![
        at(root, across(along), TAIL / 2.0),
        at(start, along, length),
        at(root, across(along), -TAIL / 2.0),
    ]
}

fn across((along_x, along_y): (f64, f64)) -> (f64, f64) {
    (-along_y, along_x)
}

fn at((x, y): (f64, f64), (dx, dy): (f64, f64), distance: f64) -> (f64, f64) {
    (x + dx * distance, y + dy * distance)
}

/// Ruby's `Float#round(1)` followed by string interpolation: one decimal at
/// most, integral values keeping their `.0` (and -0.0 its sign), halves
/// rounded away from zero with MRI's representation correction.
fn round1(x: f64) -> String {
    let rounded = round_half_up(x, 10.0) / 10.0;
    if rounded == rounded.trunc() {
        format!("{rounded:.1}")
    } else {
        format!("{rounded}")
    }
}

/// MRI's `round_half_up`: round to the nearest multiple of 1/s, halves away
/// from zero, nudged where the product `x * s` fell on the wrong side.
fn round_half_up(x: f64, s: f64) -> f64 {
    let xs = x * s;
    let mut f = xs.round();
    if x > 0.0 {
        if (f + 0.5) / s <= x {
            f += 1.0;
        }
    } else if (f - 0.5) / s >= x {
        f -= 1.0;
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strokes_are_pressed_left_to_right_and_verticals_top_down() {
        assert_eq!(ductus([50, 0, 0, 0]), [0, 0, 50, 0]); // right to left, turned round
        assert_eq!(ductus([100, 50, 100, 0]), [100, 0, 100, 50]); // bottom-up, turned round
        assert_eq!(ductus([50, 0, 0, 50]), [0, 50, 50, 0]); // diagonals too: left to right
        assert_eq!(ductus([50, 0, 100, 50]), [50, 0, 100, 50]); // already in hand
    }

    #[test]
    fn the_head_sits_at_the_start_and_the_tail_reaches_the_end() {
        // drawn right to left: pressed from the left
        let [head, tail] = wedge([100, 0, 50, 0], &mut RubyRandom::new(1));
        let back = ((head[0].0 + head[2].0) / 2.0, (head[0].1 + head[2].1) / 2.0);
        assert!((back.0 - 50.0).abs() < 1e-9); // the head's back is centred on the start
        assert!(back.1.abs() < 1e-9);
        let tip = tail[1];
        assert!((tip.0 - 100.0).abs() < 2.5); // the hand may fall a little short or long
        assert!(tip.1.abs() < 6.0); // or a little off the line (up to ±6 degrees over 50)
    }

    #[test]
    fn the_hand_is_repeatable() {
        assert_eq!(
            wedge([50, 0, 50, 150], &mut RubyRandom::new(7)),
            wedge([50, 0, 50, 150], &mut RubyRandom::new(7))
        );
        assert_ne!(
            wedge([50, 0, 50, 150], &mut RubyRandom::new(7)),
            wedge([50, 0, 50, 150], &mut RubyRandom::new(8))
        );
    }

    #[test]
    fn the_page_hand_presses_a_stroke_the_same_way_whichever_way_it_was_drawn() {
        assert_eq!(press([50, 0, 100, 50]), press([100, 50, 50, 0]));
        assert_ne!(press([50, 0, 100, 50]), press([50, 50, 100, 0])); // each stroke its own hand
        let pressed = press(glyphs::STEM);
        assert!(pressed.starts_with('M'));
        assert_eq!(pressed.matches('Z').count(), 2); // a head and a tail
    }

    #[test]
    fn rounding_matches_ruby_float_round() {
        assert_eq!(round1(62.0), "62.0");
        assert_eq!(round1(-0.04), "-0.0");
        assert_eq!(round1(61.84), "61.8");
        assert_eq!(round1(61.85), "61.9"); // IEEE 61.85 is just above the half
        assert_eq!(round1(0.25), "0.3");
        assert_eq!(round1(-0.25), "-0.3");
    }
}
