//! Weathered plates: game plates that have lost some strokes, and the
//! restoration of what is lost by the pair checksum.
//!
//! MINIMAL PORT for the fragments wave: the reading side (candidates,
//! readings, balance, surviving, the edition) is real and complete; the
//! writing side (`weather` and its fairness judge) lands with the
//! weathering wave, which replaces this file wholesale.

use crate::error::{Error, Result};
use crate::glyphs;
use crate::ruby_random::RubyRandom;
use crate::svg::Damage;
use crate::{character_of, character_sums, Line, Value, EVIL_MARKER};

/// What a null pair (noise, 9999) reads as.
pub const NULL: &str = "";

/// Lacunae for a game sequence (the weathering wave's port replaces this).
pub fn weather(
    _sequence: &[Value],
    _random: &mut RubyRandom,
    _lines: Option<&[[crate::tally::Cord; 2]]>,
) -> Damage {
    unimplemented!("weathering wave: weather() lands with the full module")
}

/// Every letter (or NULL) a pair can balance as, given each sign's candidate
/// values. Linear in the candidates (a sign wholly lost has 10,000), never
/// their product: each tablet value is looked up, not every pair of values.
pub fn readings(firsts: &[Value], seconds: &[Value]) -> Vec<String> {
    if firsts.is_empty() {
        return Vec::new();
    }
    let others: std::collections::HashSet<Value> = seconds.iter().copied().collect();
    let mut letters: Vec<String> = character_sums()
        .filter(|&sum| {
            firsts
                .iter()
                .any(|&a| a <= sum && others.contains(&(sum - a)))
        })
        .map(|sum| character_of(sum).expect("a character sum").to_string())
        .collect();
    if others.contains(&EVIL_MARKER) {
        letters.push(NULL.to_string());
    }
    letters.sort();
    letters
}

/// A tablet letter, NULL for a null pair, or None when the pair does not
/// balance.
pub fn balance(first: Value, second: Value) -> Option<String> {
    if second == EVIL_MARKER {
        return Some(NULL.to_string());
    }
    character_of(first + second).map(|c| c.to_string())
}

/// The strokes of a number that a lacuna leaves (the stem always survives),
/// canonical.
pub fn surviving(num: Value, lost: &[Line]) -> Vec<Line> {
    glyphs::strokes(num, 0, 0)
        .into_iter()
        .map(glyphs::canonical)
        .filter(|stroke| !lost.contains(stroke))
        .collect()
}

/// Every number a sign can be: its visible strokes, whatever the lost
/// positions held.
pub fn candidates(visible: &[Line], lost: &[Line]) -> Result<Vec<Value>> {
    let seen = by_place(&without_stem(visible)?)?;
    let gone = by_place(
        &lost
            .iter()
            .map(|&l| glyphs::canonical(l))
            .collect::<Vec<_>>(),
    )?;
    let mut sums: Vec<Value> = vec![0];
    for place in glyphs::PLACES {
        let mut seen_here = place_strokes(&seen, place);
        seen_here.sort();
        let values: Vec<Value> = digits(place, &seen_here, &place_strokes(&gone, place))?
            .into_iter()
            .map(|d| d * place)
            .collect();
        sums = sums
            .iter()
            .flat_map(|&sum| values.iter().map(move |&value| sum + value))
            .collect();
    }
    Ok(sums)
}

fn without_stem(strokes: &[Line]) -> Result<Vec<Line>> {
    let stem = glyphs::canonical(glyphs::STEM);
    let mut strokes: Vec<Line> = strokes.iter().map(|&l| glyphs::canonical(l)).collect();
    let position = strokes
        .iter()
        .position(|&stroke| stroke == stem)
        .ok_or_else(|| Error::decode("Symbol without a stem"))?;
    strokes.remove(position);
    Ok(strokes)
}

fn by_place(strokes: &[Line]) -> Result<Vec<(Value, Line)>> {
    strokes
        .iter()
        .map(|&stroke| {
            glyphs::stroke_place(stroke)
                .map(|place| (place, stroke))
                .ok_or_else(|| {
                    Error::decode(format!(
                        "Unknown stroke [{}, {}, {}, {}]",
                        stroke[0], stroke[1], stroke[2], stroke[3]
                    ))
                })
        })
        .collect()
}

fn place_strokes(grouped: &[(Value, Line)], place: Value) -> Vec<Line> {
    grouped
        .iter()
        .filter(|(p, _)| *p == place)
        .map(|(_, stroke)| *stroke)
        .collect()
}

/// The digits a quadrant can hold, given what is seen and what is lost of
/// it. In Ruby's DIGIT_STROKES order: the digits 1..9, then 0.
fn digits(place: Value, seen_sorted: &[Line], gone: &[Line]) -> Result<Vec<Value>> {
    let mut found = Vec::new();
    for digit in [1, 2, 3, 4, 5, 6, 7, 8, 9, 0] {
        let mut left: Vec<Line> = glyphs::digit_strokes(place, digit)
            .iter()
            .filter(|stroke| !gone.contains(stroke))
            .copied()
            .collect();
        left.sort();
        if left == seen_sorted {
            found.push(digit);
        }
    }
    if found.is_empty() {
        return Err(Error::decode("Unknown glyph"));
    }
    Ok(found)
}

/// The restored text: every pair weighed, in the Leiden conventions.
pub mod edition {
    use super::*;

    /// How a letter was read.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Kind {
        Read,
        Restored,
        Doubt,
    }

    /// One letter: what the balance allows, and how it was read. A restored
    /// space is printed plain: word division is the editor's in any edition.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Token {
        pub letters: Vec<String>,
        pub kind: Kind,
    }

    impl std::fmt::Display for Token {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self.kind {
                Kind::Read => write!(f, "{}", self.letters[0]),
                Kind::Restored => {
                    if self.letters == [" "] {
                        write!(f, " ")
                    } else {
                        write!(f, "[{}]", self.letters[0])
                    }
                }
                Kind::Doubt => write!(f, "[{}]", self.letters.join("/")),
            }
        }
    }

    /// The edition as text, from (candidate values, damaged?) of every sign
    /// in reading order.
    pub fn text(cells: &[(Vec<Value>, bool)]) -> Result<String> {
        let letters = tokens(cells)?;
        if letters.is_empty() {
            return Err(Error::decode("No enciphered characters found"));
        }
        Ok(letters.iter().map(Token::to_string).collect())
    }

    pub fn tokens(cells: &[(Vec<Value>, bool)]) -> Result<Vec<Token>> {
        if !cells.len().is_multiple_of(2) {
            return Err(Error::decode("Sequence has an odd number of values"));
        }
        let mut tokens = Vec::new();
        for pair in cells.chunks_exact(2) {
            let (firsts, hurt_a) = &pair[0];
            let (seconds, hurt_b) = &pair[1];
            let letters = readings(firsts, seconds);
            if letters.is_empty() {
                return Err(Error::decode("A pair does not balance"));
            }
            if letters == [NULL] {
                continue;
            }
            let kind = kind(&letters, *hurt_a || *hurt_b);
            tokens.push(Token { letters, kind });
        }
        Ok(tokens)
    }

    fn kind(letters: &[String], damaged: bool) -> Kind {
        if !damaged {
            Kind::Read
        } else if letters.len() == 1 {
            Kind::Restored
        } else {
            Kind::Doubt
        }
    }
}
