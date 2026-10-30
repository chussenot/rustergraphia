//! The reader's notebook for a plate with a tablet of its own: one entry per
//! sum of a pair, and the scribe's confirmations.
//!
//! - What the notebook lists: every sum the plate's pairs make (null pairs
//!   aside: their mark is public), how often, and how each was spelled
//! - What the scribe confirms: entries written against sums, three at a time
//! - The reader maps sums, never signs: the split has already grouped every
//!   pair that adds to one value, so an entry is a sum
//! - The scribe confirms only when at least [`AT_ONCE`] entries hold at
//!   once, and then says which; otherwise "nothing holds yet", never how
//!   many. When fewer than [`AT_ONCE`] sums are left out of ink, the scribe
//!   confirms them as soon as all of them hold
//! - The scribe reads the plate's tablet from its seed, as the decoder does:
//!   the reader's page never holds the tablet

use std::collections::BTreeMap;

use crate::cipher;
use crate::error::{Error, Result};
use crate::layout;
use crate::svg;
use crate::tablet;
use crate::{Value, EVIL_MARKER};

pub const AT_ONCE: usize = 3;
pub const SPELLINGS: usize = 3; // spellings shown per entry
pub const NO_TABLET: &str = "This plate has no tablet of its own: the shared one reads it";

/// One entry per sum: how often it was seen, and how it was spelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub sum: Value,
    pub seen: usize,
    pub spellings: Vec<(Value, Value)>,
}

/// The sequence of a plate along the keyless way whose pairs all land on a
/// value of the tablet (the same set under any tablet), or `None`.
pub fn read(svg_text: &str) -> Result<Option<Vec<Value>>> {
    let strokes = svg::lines(svg_text, svg::Layer::Strokes)?;
    for way in &layout::KEYLESS {
        let reading = svg::read(svg_text, way, Some(&strokes))
            .and_then(|sequence| cipher::decipher(&sequence, None).map(|_| sequence));
        match reading {
            Ok(sequence) => return Ok(Some(sequence)),
            Err(error) if error.is_decode() => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}

/// Every pair that reads, as (first, second, sum), in reading order: null
/// pairs (a second sign of [`EVIL_MARKER`]) are left out.
pub fn pairs(sequence: &[Value]) -> Vec<(Value, Value, Value)> {
    sequence
        .chunks_exact(2)
        .filter(|pair| pair[1] != EVIL_MARKER)
        .map(|pair| (pair[0], pair[1], pair[0] + pair[1]))
        .collect()
}

/// One entry per sum, the most seen first (the space, nearly always), then
/// by sum. Spellings are unique, in first-seen order.
pub fn entries(sequence: &[Value]) -> Vec<Entry> {
    let mut listed: Vec<Entry> = Vec::new();
    for (a, b, sum) in pairs(sequence) {
        match listed.iter_mut().find(|entry| entry.sum == sum) {
            Some(entry) => {
                entry.seen += 1;
                if !entry.spellings.contains(&(a, b)) {
                    entry.spellings.push((a, b));
                }
            }
            None => listed.push(Entry {
                sum,
                seen: 1,
                spellings: vec![(a, b)],
            }),
        }
    }
    listed.sort_by_key(|entry| (std::cmp::Reverse(entry.seen), entry.sum));
    listed
}

/// The entries that hold, sum => character, of those written in pencil, the
/// sums already in ink set aside: all that hold when at least [`AT_ONCE`]
/// do, or when the sums left out of ink are fewer than [`AT_ONCE`] and every
/// one of them holds; otherwise none.
pub fn held(
    svg_text: &str,
    pencil: &BTreeMap<Value, char>,
    inked: &[Value],
) -> Result<BTreeMap<Value, char>> {
    let (tablet, sums) = plate(svg_text)?;
    let open: Vec<Value> = sums
        .into_iter()
        .filter(|sum| !inked.contains(sum))
        .collect();
    let right: BTreeMap<Value, char> = pencil
        .iter()
        .filter(|(sum, c)| open.contains(sum) && tablet.get(sum) == Some(c))
        .map(|(sum, c)| (*sum, *c))
        .collect();
    if confirmed(&right, &open) {
        Ok(right)
    } else {
        Ok(BTreeMap::new())
    }
}

/// (value => character, every sum) of a plate with a tablet of its own.
pub fn plate(svg_text: &str) -> Result<(std::collections::HashMap<Value, char>, Vec<Value>)> {
    let seed = tablet::seed_of(svg_text).ok_or_else(|| Error::other(NO_TABLET))?;
    let sequence = read(svg_text)?.ok_or_else(|| Error::decode("No way reads this plate"))?;
    Ok((
        tablet::characters(&seed)?,
        entries(&sequence)
            .into_iter()
            .map(|entry| entry.sum)
            .collect(),
    ))
}

/// Three at a time, or the last ones together.
fn confirmed(right: &BTreeMap<Value, char>, open: &[Value]) -> bool {
    right.len() >= AT_ONCE
        || (open.len() < AT_ONCE && open.iter().all(|sum| right.contains_key(sum)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Layout;
    use crate::ruby_random::RubyRandom;

    const SEED: &str = "K7Q2M-ZXB4P";
    const MESSAGE: &str = "Meet me at the old abbey after vespers";

    fn plate_with_tablet() -> (Vec<Value>, String) {
        let mut random = RubyRandom::new_os();
        cipher::inscribe(
            MESSAGE,
            None,
            &Layout::Furrows,
            false,
            false,
            Some(SEED),
            &mut random,
        )
        .unwrap()
    }

    fn right(svg: &str, sums: &[Value]) -> BTreeMap<Value, char> {
        let (tablet, _) = plate(svg).unwrap();
        sums.iter().map(|sum| (*sum, tablet[sum])).collect()
    }

    #[test]
    fn the_notebook_lists_every_sum_once_the_most_seen_first_with_its_spellings() {
        let (sequence, svg) = plate_with_tablet();
        let listed = entries(&sequence);
        let letters: Vec<char> = cipher::normalize(MESSAGE).chars().collect();
        let mut unique = letters.clone();
        unique.sort_unstable();
        unique.dedup();

        assert_eq!(listed.len(), unique.len(), "one entry per letter");
        assert_eq!(
            listed.iter().map(|entry| entry.seen).sum::<usize>(),
            letters.len(),
            "every pair that reads, null pairs aside"
        );
        let seen: Vec<usize> = listed.iter().map(|entry| entry.seen).collect();
        let mut sorted = seen.clone();
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        assert_eq!(seen, sorted);
        for entry in &listed {
            assert!(entry.spellings.iter().all(|(a, b)| a + b == entry.sum));
        }
        assert_eq!(
            read(&svg).unwrap(),
            Some(sequence),
            "the way is found by the pair test, under any tablet"
        );
    }

    #[test]
    fn the_scribe_confirms_three_at_a_time_and_says_nothing_of_fewer() {
        let (sequence, svg) = plate_with_tablet();
        let sums: Vec<Value> = entries(&sequence).into_iter().map(|e| e.sum).collect();
        let (tablet, _) = plate(&svg).unwrap();

        assert!(held(&svg, &right(&svg, &sums[..2]), &[])
            .unwrap()
            .is_empty());
        let wrong_char = ('a'..='z').find(|c| *c != tablet[&sums[3]]).unwrap();
        let mut three_and_wrong = right(&svg, &sums[..3]);
        three_and_wrong.insert(sums[3], wrong_char);
        assert_eq!(
            held(&svg, &three_and_wrong, &[]).unwrap(),
            right(&svg, &sums[..3]),
            "three right hold, the wrong one stays in pencil"
        );
        let mut two_and_wrong = right(&svg, &sums[..2]);
        two_and_wrong.insert(sums[3], wrong_char);
        assert!(held(&svg, &two_and_wrong, &[]).unwrap().is_empty());
    }

    #[test]
    fn the_last_ones_are_confirmed_together_when_fewer_than_three_are_left() {
        let (sequence, svg) = plate_with_tablet();
        let sums: Vec<Value> = entries(&sequence).into_iter().map(|e| e.sum).collect();
        let inked = &sums[..sums.len() - 2];
        let last = &sums[sums.len() - 2..];

        assert_eq!(
            held(&svg, &right(&svg, last), inked).unwrap(),
            right(&svg, last)
        );
        assert!(held(&svg, &right(&svg, &last[..1]), inked)
            .unwrap()
            .is_empty());
        assert!(
            held(&svg, &right(&svg, &sums[..3]), &sums[..3])
                .unwrap()
                .is_empty(),
            "entries already in ink count for nothing"
        );
    }

    #[test]
    fn a_plate_on_the_shared_tablet_has_no_notebook() {
        let mut random = RubyRandom::new_os();
        let (_, shared) = cipher::inscribe(
            MESSAGE,
            None,
            &Layout::Phaistos,
            false,
            false,
            None,
            &mut random,
        )
        .unwrap();
        let error = held(&shared, &BTreeMap::new(), &[]).unwrap_err();
        assert_eq!(error.to_string(), NO_TABLET);
    }
}
