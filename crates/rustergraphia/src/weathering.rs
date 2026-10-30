//! Weathered plates: game plates that have lost some strokes, and the
//! restoration of what is lost by the pair checksum.
//!
//! - Damage is placed on purpose, not at random: a lacuna is kept only if the
//!   balance restores it, or narrows it to at most MAX_DOUBT letters, never a
//!   space, and at most one doubtful letter per word. The reader's sense of
//!   the language settles the doubts; the server needs no word list
//! - One lacuna per pair at most, about one pair in SHARE (one in SMALL_SHARE
//!   under SMALL_PLATE pairs): the stem always survives, so every sign keeps
//!   its place on the grid
//! - The edition follows the Leiden conventions of epigraphy: [r] restored,
//!   [b/l] restored with a doubt the balance cannot settle
//! - Damage is { sequence index => lost strokes }, strokes canonical and
//!   upright, relative to the sign: the same for every layout
//!
//! Game plates only: a sealed plate that loses one stroke no longer opens.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::error::{Error, Result};
use crate::glyphs;
use crate::ruby_random::RubyRandom;
use crate::svg::Damage;
use crate::tally::{Cord, CordKind};
use crate::{Line, Value, EVIL_MARKER, TABLET};

pub const SHARE: usize = 3; // about one pair in three loses something
pub const SMALL_SHARE: usize = 2; // one pair in two on a small plate, which has few words to carry a doubt
pub const SMALL_PLATE: usize = 32; // pairs: under 8 × 8
pub const TALLIED_SHARE: usize = 2; // one pair in two, with tally signs to restore whole signs
pub const MAX_DOUBT: usize = 3; // letters a doubtful lacuna may leave
pub const NULL: &str = ""; // what a null pair (noise, 9999) reads as

/// Each quadrant's five stroke positions: the strokes of 1, 2, 3, 4 and 6.
static POSITIONS: LazyLock<Vec<(Value, Vec<Line>)>> = LazyLock::new(|| {
    glyphs::PLACES
        .into_iter()
        .map(|place| {
            let strokes = [1, 2, 3, 4, 6]
                .into_iter()
                .map(|digit| {
                    glyphs::canonical(
                        glyphs::lines_of(digit * place).expect("a digit stroke exists")[0],
                    )
                })
                .collect();
            (place, strokes)
        })
        .collect()
});

/// The five stroke positions of one quadrant, in the Ruby POSITIONS order.
pub fn positions(place: Value) -> &'static [Line] {
    POSITIONS
        .iter()
        .find(|(p, _)| *p == place)
        .map(|(_, strokes)| strokes.as_slice())
        .expect("a known place")
}

/// Every stroke position of every quadrant (Ruby `Cords::WHOLE`).
pub fn whole_positions() -> Vec<Line> {
    POSITIONS
        .iter()
        .flat_map(|(_, strokes)| strokes.iter().copied())
        .collect()
}

/// Lacunae for a game sequence: { index => lost strokes }. With lines (the
/// grid row and column of every sign, on a plate with tally signs), whole
/// signs may be lost too, and more pairs are damaged.
pub fn weather(sequence: &[Value], random: &mut RubyRandom, lines: Option<&[[Cord; 2]]>) -> Damage {
    let mut starts: Vec<usize> = (0..sequence.len()).step_by(2).collect();
    let target = target_for(starts.len(), lines.is_some());
    let words = words_of(sequence);
    let mut doubted: HashSet<usize> = HashSet::new();
    let mut cords = lines.map(Cords::new);
    random.shuffle(&mut starts);

    let mut damage = Damage::new();
    for start in starts {
        if damage.len() >= target {
            break;
        }
        let found = match cords.as_mut().and_then(|c| c.try_whole(start, random)) {
            Some(whole) => Some(whole),
            None => try_lacuna(
                sequence,
                start,
                &words,
                &mut doubted,
                random,
                cords.as_mut(),
            ),
        };
        if let Some((index, lost)) = found {
            damage.insert(index, lost);
        }
    }
    damage
}

/// How many pairs to damage.
pub fn target_for(pairs: usize, tallied: bool) -> usize {
    let share = if tallied {
        TALLIED_SHARE
    } else if pairs < SMALL_PLATE {
        SMALL_SHARE
    } else {
        SHARE
    };
    (pairs / share).max(1)
}

/// (index, lost strokes) on one sign of the pair at start, or None when that
/// lacuna would not be fair (doubts are counted in doubted, by word).
fn try_lacuna(
    sequence: &[Value],
    start: usize,
    words: &HashMap<usize, Option<usize>>,
    doubted: &mut HashSet<usize>,
    random: &mut RubyRandom,
    cords: Option<&mut Cords>,
) -> Option<(usize, Vec<Line>)> {
    let index = start + random.rand_below(2) as usize;
    let lost = lacuna(random);
    let mut damage = Damage::new();
    damage.insert(index, lost.clone());
    let letters = outcomes(sequence, start, &damage);
    let word = words.get(&start).copied().flatten();
    if !fair(&letters, word, doubted) {
        return None;
    }
    if let Some(cords) = cords {
        if !cords.free(index) {
            return None;
        }
        cords.count(index);
    }
    if letters.len() > 1 {
        doubted.insert(word.expect("a doubt belongs to a word"));
    }
    Some((index, lost))
}

/// A whole quadrant, or one to three of its stroke positions.
fn lacuna(random: &mut RubyRandom) -> Vec<Line> {
    let place = glyphs::PLACES[random.sample_index(glyphs::PLACES.len())];
    let positions = positions(place);
    if random.rand_below(2) == 0 {
        return positions.to_vec();
    }
    let count = 1 + random.rand_below(3) as usize;
    random
        .sample_indices(count, positions.len())
        .into_iter()
        .map(|i| positions[i])
        .collect()
}

/// Whether a lacuna leaving these letters is fair to the reader.
fn fair(letters: &[String], word: Option<usize>, doubted: &HashSet<usize>) -> bool {
    if letters.len() == 1 {
        return true;
    }
    if letters.len() > MAX_DOUBT || letters.iter().any(|l| l == " " || l == NULL) {
        return false;
    }
    match word {
        Some(word) => !doubted.contains(&word),
        None => false,
    }
}

/// Pair start => the number of the word its letter belongs to (None for
/// spaces and null pairs).
pub fn words_of(sequence: &[Value]) -> HashMap<usize, Option<usize>> {
    let mut word = 0usize;
    sequence
        .chunks(2)
        .enumerate()
        .map(|(pair, values)| {
            let letter = if values.len() == 2 {
                balance(values[0], values[1])
            } else {
                None
            };
            if letter.as_deref() == Some(" ") {
                word += 1;
            }
            let of = match &letter {
                Some(l) if l.len() == 1 && l.chars().all(|c| c.is_ascii_lowercase()) => Some(word),
                _ => None,
            };
            (pair * 2, of)
        })
        .collect()
}

/// What a pair can read as, with some of its strokes lost.
pub fn outcomes(sequence: &[Value], start: usize, damage: &Damage) -> Vec<String> {
    let empty = Vec::new();
    let mut sides = [Vec::new(), Vec::new()];
    for (slot, index) in sides.iter_mut().zip([start, start + 1]) {
        let lost = damage.get(&index).unwrap_or(&empty);
        *slot =
            candidates(&surviving(sequence[index], lost), lost).expect("a library sequence reads");
    }
    let [firsts, seconds] = sides;
    readings(&firsts, &seconds)
}

/// Every letter (or NULL) a pair can balance as, given each sign's candidate
/// values. Linear in the candidates (a sign wholly lost has 10,000), never
/// their product: each tablet value is looked up, not every pair of values.
pub fn readings(firsts: &[Value], seconds: &[Value]) -> Vec<String> {
    if firsts.is_empty() {
        return Vec::new();
    }
    let others: HashSet<Value> = seconds.iter().copied().collect();
    let mut letters: Vec<String> = TABLET
        .iter()
        .filter(|(_, value)| {
            let sum = value * 10;
            firsts
                .iter()
                .any(|&a| sum >= a && others.contains(&(sum - a)))
        })
        .map(|(c, _)| c.to_string())
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
    crate::character_of(first + second).map(|c| c.to_string())
}

/// Whether a value can balance with one of its partner's candidates (a set):
/// a null pair (the second is 9999), or a sum on the tablet.
pub fn balances(value: Value, partners: &HashSet<Value>, first: bool) -> bool {
    if first {
        if partners.contains(&EVIL_MARKER) {
            return true;
        }
    } else if value == EVIL_MARKER {
        return true;
    }
    crate::character_sums().any(|sum| sum >= value && partners.contains(&(sum - value)))
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
    let empty = Vec::new();
    let mut sums: Vec<Value> = vec![0];
    for place in glyphs::PLACES {
        let mut seen_place = seen.get(&place).unwrap_or(&empty).clone();
        seen_place.sort();
        let gone_place = gone.get(&place).unwrap_or(&empty);
        let values: Vec<Value> = digits(place, &seen_place, gone_place)?
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
    let mut canonical: Vec<Line> = strokes.iter().map(|&l| glyphs::canonical(l)).collect();
    let before = canonical.len();
    canonical.retain(|&line| line != stem);
    if canonical.len() == before {
        return Err(Error::decode("Symbol without a stem"));
    }
    Ok(canonical)
}

fn by_place(strokes: &[Line]) -> Result<HashMap<Value, Vec<Line>>> {
    let mut grouped: HashMap<Value, Vec<Line>> = HashMap::new();
    for &stroke in strokes {
        let place = glyphs::stroke_place(stroke).ok_or_else(|| {
            Error::decode(format!(
                "Unknown stroke [{}, {}, {}, {}]",
                stroke[0], stroke[1], stroke[2], stroke[3]
            ))
        })?;
        grouped.entry(place).or_default().push(stroke);
    }
    Ok(grouped)
}

/// The digits a quadrant can hold, given what is seen and what is lost of it
/// (in the Ruby DIGIT_STROKES order: 1 to 9, then 0).
fn digits(place: Value, seen: &[Line], gone: &[Line]) -> Result<Vec<Value>> {
    let mut found = Vec::new();
    for digit in (1..=9).chain([0]) {
        let strokes = glyphs::digit_strokes(place, digit);
        let mut left: Vec<Line> = strokes
            .iter()
            .copied()
            .filter(|stroke| !gone.contains(stroke))
            .collect();
        left.sort();
        if left == seen {
            found.push(digit);
        }
    }
    if found.is_empty() {
        return Err(Error::decode("Unknown glyph"));
    }
    Ok(found)
}

/// Where whole signs may be lost, on a plate with tally signs: a whole sign
/// is given back exactly by a row (or column) tally when it is that line's
/// only lacuna. So a whole sign needs a line with no lacuna yet, and that
/// line is then kept clear: no later lacuna may enter it. Other lacunae need
/// only be fair by their pair, as without tallies (tallies only ever narrow).
pub struct Cords {
    lines: Vec<[Cord; 2]>,
    count: HashMap<Cord, usize>,
    reserved: HashSet<Cord>,
}

impl Cords {
    pub fn new(lines: &[[Cord; 2]]) -> Cords {
        Cords {
            lines: lines.to_vec(),
            count: HashMap::new(),
            reserved: HashSet::new(),
        }
    }

    /// (index, every stroke position) when a whole sign of this pair can go,
    /// half the time.
    pub fn try_whole(
        &mut self,
        start: usize,
        random: &mut RubyRandom,
    ) -> Option<(usize, Vec<Line>)> {
        if random.rand_below(2) != 0 {
            return None;
        }
        let index = start + random.rand_below(2) as usize;
        let clear = self.lines[index]
            .iter()
            .find(|line| self.count.get(line).copied().unwrap_or(0) == 0)
            .copied();
        let clear = clear?;
        if !self.free(index) {
            return None;
        }
        self.count(index);
        self.reserved.insert(clear);
        Some((index, whole_positions()))
    }

    /// Whether a lacuna may go on this sign: none of its lines is kept clear.
    pub fn free(&self, index: usize) -> bool {
        self.lines[index]
            .iter()
            .all(|line| !self.reserved.contains(line))
    }

    pub fn count(&mut self, index: usize) {
        for line in self.lines[index] {
            *self.count.entry(line).or_insert(0) += 1;
        }
    }
}

// Keep the Cord types in scope for the public signature above.
const _: fn() -> CordKind = || CordKind::Row;

/// The restored text: every pair weighed, in the Leiden conventions.
pub mod edition {
    use super::*;

    /// How a letter was read: plain, restored by the balance, or a doubt.
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
        for pair in cells.as_chunks::<2>().0 {
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
            return Kind::Read;
        }
        if letters.len() == 1 {
            Kind::Restored
        } else {
            Kind::Doubt
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cipher;

    const MESSAGES: [&str; 3] = [
        "the brothers will gather in the chapter house after compline",
        "Meet me at the old abbey after vespers",
        "ora et labora",
    ];

    #[test]
    fn each_quadrant_has_five_stroke_positions_and_the_stem_none() {
        let all = whole_positions();
        let mut unique = all.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 20);
        assert!(!all.contains(&glyphs::canonical(glyphs::STEM)));
        for stroke in &all {
            assert!(glyphs::stroke_place(*stroke).is_some());
        }
    }

    #[test]
    fn an_intact_sign_has_one_reading() {
        assert_eq!(candidates(&surviving(1098, &[]), &[]).unwrap(), vec![1098]);
    }

    #[test]
    fn a_lost_quadrant_could_hold_any_digit() {
        let lost = positions(10).to_vec();
        let mut found = candidates(&surviving(1098, &lost), &lost).unwrap();
        found.sort();
        let expected: Vec<Value> = (0..10).map(|d| 1008 + d * 10).collect();
        assert_eq!(found, expected);
    }

    #[test]
    fn the_strokes_that_survive_still_constrain_a_quadrant() {
        let hundreds = positions(100);
        let lost = vec![hundreds[0], hundreds[2]];
        let seen = surviving(920, &lost);
        let mut found = candidates(&seen, &lost).unwrap();
        found.sort();
        assert_eq!(found, vec![820, 920]);
    }

    #[test]
    fn a_pair_balances_as_a_letter_a_null_or_not_at_all() {
        assert_eq!(balance(3, 7).as_deref(), Some("a"));
        assert_eq!(balance(1234, 9999).as_deref(), Some(NULL));
        assert_eq!(balance(3, 8), None);
    }

    #[test]
    fn lacunae_are_fair_to_the_reader() {
        for seed in 0..40i64 {
            let mut encoding = RubyRandom::new(1000 + seed);
            let sequence = cipher::encode(
                MESSAGES[seed as usize % MESSAGES.len()],
                None,
                false,
                None,
                &mut encoding,
            )
            .unwrap();
            let mut random = RubyRandom::new(seed);
            let damage = weather(&sequence, &mut random, None);
            assert!(!damage.is_empty());
            let mut pairs: Vec<usize> = damage.keys().map(|index| index / 2).collect();
            pairs.dedup();
            assert_eq!(pairs.len(), damage.len(), "one lacuna per pair");
            let words = words_of(&sequence);
            let mut doubts = Vec::new();
            for index in damage.keys() {
                let start = index - index % 2;
                let letters = outcomes(&sequence, start, &damage);
                let truth = balance(sequence[start], sequence[start + 1]).unwrap();
                assert!(letters.contains(&truth), "the truth is among the outcomes");
                if letters.len() > 1 {
                    assert!(letters.len() <= MAX_DOUBT);
                    assert!(!letters.iter().any(|l| l == " "));
                    doubts.push(words[&start].expect("a doubt belongs to a word"));
                }
            }
            let mut unique = doubts.clone();
            unique.sort();
            unique.dedup();
            doubts.sort();
            assert_eq!(doubts, unique, "one doubt per word at most");
        }
    }

    #[test]
    fn the_edition_follows_the_leiden_conventions() {
        let tokens = [
            edition::Token {
                letters: vec!["a".into()],
                kind: edition::Kind::Read,
            },
            edition::Token {
                letters: vec!["b".into()],
                kind: edition::Kind::Restored,
            },
            edition::Token {
                letters: vec!["c".into(), "e".into()],
                kind: edition::Kind::Doubt,
            },
        ];
        let text: String = tokens.iter().map(|t| t.to_string()).collect();
        assert_eq!(text, "a[b][c/e]");
        let space = edition::Token {
            letters: vec![" ".into()],
            kind: edition::Kind::Restored,
        };
        assert_eq!(space.to_string(), " ");
    }

    #[test]
    fn a_pair_wholly_lost_is_weighed_without_trying_every_pair_of_values() {
        let all = whole_positions();
        let lost = candidates(&surviving(1234, &all), &all).unwrap();
        assert_eq!(lost.len(), 10_000);
        let readings = readings(&lost, &lost);
        let mut expected: Vec<String> = crate::TABLET.iter().map(|(c, _)| c.to_string()).collect();
        expected.push(NULL.to_string());
        expected.sort();
        assert_eq!(readings, expected);
    }

    #[test]
    fn a_pair_that_cannot_balance_is_no_edition() {
        let cells = [(vec![3], false), (vec![8], true)];
        assert!(edition::text(&cells).unwrap_err().is_decode());
    }

    #[test]
    fn small_plates_lose_one_pair_in_two_and_large_ones_one_in_three() {
        assert_eq!(target_for(8, false), 4); // 4 × 4
        assert_eq!(target_for(18, false), 9); // 6 × 6
        assert_eq!(target_for(32, false), 10); // 8 × 8
        assert_eq!(target_for(50, false), 16); // 10 × 10
        assert_eq!(target_for(8, true), 4); // tally signs: one in two throughout
    }

    #[test]
    fn the_same_plate_wears_the_same_way() {
        let mut encoding = RubyRandom::new(99);
        let sequence = cipher::encode("ora et labora", None, false, None, &mut encoding).unwrap();
        let mut first = RubyRandom::new(7);
        let mut second = RubyRandom::new(7);
        assert_eq!(
            weather(&sequence, &mut first, None),
            weather(&sequence, &mut second, None)
        );
    }
}
