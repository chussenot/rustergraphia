//! A plate's tablet of values. Every plate reads by a tablet, the mapping of
//! the twenty-seven characters (the letters and the space) to their values;
//! the shared one is `TABLET`. A plate may have a tablet of its own: the same
//! values dealt out again, drawn from a seed the file carries on its root
//! (`data-tablet`). Its reader must build the tablet in the notebook, sum by
//! sum; the decoder reads the seed and needs nothing else.
//!
//! Design decisions, as in the Ruby original:
//! - The values are the shared tablet's, only permuted: the pairs still sum
//!   to a value on the tablet, so the way is still found by the pair test and
//!   the null marks still read, under any tablet
//! - The seed is written in the grille key's alphabet and length, and
//!   normalised the same way (case, dashes, the look-alikes O, I and L)
//! - The permutation is drawn by a Fisher-Yates shuffle from a `Random`
//!   seeded by the SHA-256 of the seed, so every implementation (Ruby or
//!   Rust) deals the same tablet
//! - A game's plate only, intact, under a keyless way: a sealed plate has no
//!   tablet to find, a grille plate has a key already, and the bench and the
//!   board speak in letters

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::ruby_random::RubyRandom;
use crate::{character_sums, grille, Value, TABLET};

pub use grille::{ALPHABET, LENGTH};
pub const ATTRIBUTE: &str = "data-tablet";
/// Words: under this, a plate with its own tablet is close to guessing.
pub const SHORT: usize = 6;
pub const NOT_SEALED: &str =
    "A sealed plate has no tablet to find: its passphrase is the whole key";
pub const NOT_UNDER_A_GRILLE: &str = "A grille plate has a key already: one key at a time";
pub const NOT_DAMAGED: &str =
    "A worn or broken plate keeps the shared tablet: the bench and the board speak in letters";
pub const NOT_A_SEED: &str = "A tablet's seed has 10 letters and digits, as a grille key";

/// A new seed, at random.
pub fn generate() -> String {
    let alphabet = ALPHABET.as_bytes();
    let mut seed = String::with_capacity(LENGTH);
    for _ in 0..LENGTH {
        let mut bytes = [0u8; 4];
        getrandom::getrandom(&mut bytes).expect("operating system randomness");
        let index = (u32::from_le_bytes(bytes) as usize) % alphabet.len();
        seed.push(alphabet[index] as char);
    }
    seed
}

/// A seed as written anywhere: any case, spaces and dashes, O for 0, I or L
/// for 1.
pub fn normalize(seed: &str) -> Result<String> {
    let code = grille::normalize_code(seed);
    if code.len() != LENGTH || !code.chars().all(|c| ALPHABET.contains(c)) {
        return Err(Error::other(NOT_A_SEED));
    }
    Ok(code)
}

/// A seed as shown: in two halves, as a grille key.
pub fn show(seed: &str) -> Result<String> {
    let code = normalize(seed)?;
    Ok(format!("{}-{}", &code[..LENGTH / 2], &code[LENGTH / 2..]))
}

/// The tablet a seed draws: character => value, the shared values dealt out
/// again, in the shared tablet's character order.
pub fn draw(seed: &str) -> Result<Vec<(char, Value)>> {
    let code = normalize(seed)?;
    let digest = Sha256::digest(format!("cistergraphia tablet {code}"));
    let mut random = RubyRandom::from_be_bytes(&digest);
    let mut values: Vec<Value> = character_sums().collect();
    shuffle(&mut values, &mut random);
    Ok(TABLET.iter().map(|(c, _)| *c).zip(values).collect())
}

/// Fisher-Yates, written out in the Ruby original so that every Ruby deals
/// the same; `RubyRandom::shuffle` makes the identical draws.
fn shuffle(items: &mut [Value], random: &mut RubyRandom) {
    random.shuffle(items);
}

/// value => character, for reading a plate written with this tablet.
pub fn characters(seed: &str) -> Result<HashMap<Value, char>> {
    Ok(draw(seed)?.into_iter().map(|(c, v)| (v, c)).collect())
}

/// The rows of a tablet for a page or a terminal: `(character, value)`, in
/// the shared tablet's order.
pub fn rows(seed: &str) -> Result<Vec<(char, Value)>> {
    draw(seed)
}

/// The seed a plate's file carries, or `None` for a plate on the shared
/// tablet (a string test on the root's attribute, as `pathless` reads its
/// class).
pub fn seed_of(svg: &str) -> Option<String> {
    let start = svg.find("<svg")?;
    let after = &svg[start + 4..];
    // \b: the tag name ends here, not a longer one
    if after.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        return None;
    }
    let tag = &after[..after.find('>').unwrap_or(after.len())];
    let needle = format!("{ATTRIBUTE}=\"");
    let at = tag.find(&needle)?;
    // The attribute follows whitespace, as the Ruby regex (\s) asks
    if !tag[..at].ends_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let value = &tag[at + needle.len()..];
    let end = value.find('"')?;
    if end == 0 {
        return None;
    }
    Some(value[..end].to_string())
}

/// The plates that cannot have a tablet of their own, in the library's words.
/// (`under_grille` stands in for Ruby's `layout.is_a?(Grille)`: the caller
/// passes `matches!(layout, Layout::Grille(_))`.)
pub fn check(passphrase: Option<&str>, under_grille: bool, damaged: bool) -> Result<()> {
    if passphrase.is_some() {
        return Err(Error::other(NOT_SEALED));
    }
    if under_grille {
        return Err(Error::other(NOT_UNDER_A_GRILLE));
    }
    if damaged {
        return Err(Error::other(NOT_DAMAGED));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_normalize_like_grille_keys() {
        assert_eq!(normalize("k7q2m-zxb4p").unwrap(), "K7Q2MZXB4P");
        assert_eq!(show("k7q2m zxb4p").unwrap(), "K7Q2M-ZXB4P");
        assert_eq!(normalize("oO1Il llllo").unwrap(), "0011111110");
        assert!(normalize("SHORT").is_err());
        assert_eq!(normalize("UUUUUUUUUU").unwrap_err().to_string(), NOT_A_SEED);
    }

    #[test]
    fn a_tablet_permutes_the_shared_values() {
        let tablet = draw("ABCDEFGH23").unwrap();
        assert_eq!(tablet.len(), 27);
        let mut values: Vec<Value> = tablet.iter().map(|(_, v)| *v).collect();
        values.sort();
        let mut shared: Vec<Value> = character_sums().collect();
        shared.sort();
        assert_eq!(values, shared);
    }

    #[test]
    fn generated_seeds_are_well_formed() {
        let seed = generate();
        assert_eq!(normalize(&seed).unwrap(), seed);
    }

    #[test]
    fn the_seed_is_read_off_the_root() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="2" data-tablet="K7Q2MZXB4P"><rect/></svg>"#;
        assert_eq!(seed_of(svg).as_deref(), Some("K7Q2MZXB4P"));
        assert_eq!(
            seed_of(r#"<svg width="1"><g data-tablet="X"/></svg>"#),
            None
        );
        assert_eq!(seed_of("<svg><rect/></svg>"), None);
        assert_eq!(seed_of("plain text"), None);
    }

    #[test]
    fn refusals_speak_the_library_words() {
        assert_eq!(
            check(Some("pass"), false, false).unwrap_err().to_string(),
            NOT_SEALED
        );
        assert_eq!(
            check(None, true, false).unwrap_err().to_string(),
            NOT_UNDER_A_GRILLE
        );
        assert_eq!(
            check(None, false, true).unwrap_err().to_string(),
            NOT_DAMAGED
        );
        assert!(check(None, false, false).is_ok());
    }
}
