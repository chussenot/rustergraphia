//! The turning grille, a third way to read a plate (after Cardano's stencil
//! of 1550 and Fleissner's turning grille of 1881, the one in Jules Verne's
//! Mathias Sandorf). The plate is laid out like a page; a square parchment
//! pierced with holes is laid on it; the signs under the holes are read row
//! by row; then the parchment is turned a quarter clockwise, four times:
//! N, E, S, W.
//!
//! An n x n plate (n even) splits into n²/4 orbits: a cell of its top-left
//! quarter and the three cells it visits as the plate turns by quarters. The
//! grille pierces exactly one cell of each orbit, so its four turns uncover
//! every cell once: the whole plate is the message, in the grille's order.
//!
//! The key is a code of 10 characters (50 bits, Crockford's base 32) that
//! sets the grille for a plate of any size: each orbit's hole is drawn from
//! SHA-256 of the code and the plate's side. Read without the grille, a
//! plate is an anagram of its own message. A turning grille is a
//! transposition cipher of the 19th century, not secrecy.

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

pub const ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const LENGTH: usize = 10;
pub const EXAMPLE: &str = "K7Q2M-ZXB4P";
pub const TURNS: [&str; 4] = ["N", "E", "S", "W"];
pub const WRONG_KEY: &str = "This grille does not read this plate: check its key";
pub const WRONG_KEY_OR_PASSPHRASE: &str =
    "Wrong grille key or passphrase, or the plate was altered";

/// Which orbits a grille pierces: all of them (`Whole`), or, on a plate with
/// two readers, the abbot's or the novice's; `Pair` is that plate, both read
/// in turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    Whole,
    Abbot,
    Novice,
    Pair,
}

/// A turning grille: its key code (normalized) and the part it reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Grille {
    code: String,
    part: Part,
}

impl std::fmt::Display for Grille {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", &self.code[0..5], &self.code[5..10])
    }
}

impl Grille {
    /// A new grille under a random key.
    pub fn generate() -> Grille {
        let alphabet = ALPHABET.as_bytes();
        let mut key = String::with_capacity(LENGTH);
        for _ in 0..LENGTH {
            let mut byte = [0u8; 4];
            getrandom::getrandom(&mut byte).expect("operating system randomness");
            let index = (u32::from_le_bytes(byte) as usize) % alphabet.len();
            key.push(alphabet[index] as char);
        }
        Grille::new(&key, Part::Whole).expect("a generated key is well-formed")
    }

    /// Any case, spaces and dashes are fine, and so are the look-alikes
    /// Crockford allows (O for 0, I or L for 1).
    pub fn new(key: &str, part: Part) -> Result<Grille> {
        let code = normalize_code(key);
        if code.len() != LENGTH || !code.chars().all(|c| ALPHABET.contains(c)) {
            return Err(Error::other(format!(
                "A grille key has {LENGTH} letters and digits, like {EXAMPLE}"
            )));
        }
        Ok(Grille { code, part })
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn part(&self) -> Part {
        self.part
    }

    /// The same key for another part of a plate with two readers.
    pub fn as_part(&self, part: Part) -> Grille {
        Grille {
            code: self.code.clone(),
            part,
        }
    }

    /// The cells of an n x n plate by orbit: each `[row, col]` of its
    /// top-left quarter, followed by where a quarter turn clockwise takes
    /// it, three times.
    pub fn orbits(size: usize) -> Result<Vec<[(i32, i32); 4]>> {
        if !size.is_multiple_of(2) {
            return Err(Error::other("A grille needs a plate with an even side"));
        }
        let half = size / 2;
        let mut orbits = Vec::with_capacity(half * half);
        for row in 0..half {
            for col in 0..half {
                let mut orbit = [(row as i32, col as i32); 4];
                for turn in 1..4 {
                    orbit[turn] = Self::quarter_turn(orbit[turn - 1], size);
                }
                orbits.push(orbit);
            }
        }
        Ok(orbits)
    }

    pub fn quarter_turn((row, col): (i32, i32), size: usize) -> (i32, i32) {
        (col, size as i32 - 1 - row)
    }

    /// The novice's grille, drawn from this one's key (the abbot's) by
    /// SHA-256: whoever has the abbot's key has both, the novice's key gives
    /// only its own.
    pub fn novice(&self) -> Grille {
        let digest = Sha256::digest(format!("cistergraphia-novice:{}", self.code));
        let alphabet = ALPHABET.as_bytes();
        // The first LENGTH * 5 bits of the digest, five at a time
        let mut bits = Vec::with_capacity(digest.len() * 8);
        for byte in digest {
            for shift in (0..8).rev() {
                bits.push((byte >> shift) & 1);
            }
        }
        let code: String = bits[..LENGTH * 5]
            .chunks(5)
            .map(|chunk| {
                let index = chunk
                    .iter()
                    .fold(0usize, |acc, bit| (acc << 1) | *bit as usize);
                alphabet[index] as char
            })
            .collect();
        Grille {
            code,
            part: Part::Novice,
        }
    }

    /// The orbits this grille pierces on an n x n plate. The novice's are
    /// about half of them, drawn from the novice's key; the abbot's are the
    /// others.
    pub fn pierced(&self, size: usize) -> Vec<usize> {
        let all = 0..(size * size / 4);
        match self.part {
            Part::Novice => {
                let share = self.novice_share(size);
                all.filter(|&orbit| share[orbit]).collect()
            }
            Part::Abbot => {
                let share = self.novice().novice_share(size);
                all.filter(|&orbit| !share[orbit]).collect()
            }
            _ => all.collect(),
        }
    }

    /// Which of its four cells is pierced (0..3), orbit by orbit, on an
    /// n x n plate.
    pub fn holes_by_orbit(&self, size: usize) -> Vec<u8> {
        stream(
            &format!("cistergraphia-grille:{}:{}", self.code, size),
            size * size / 4,
        )
    }

    /// `[row, col]` of the holes with the grille turned clockwise by `turn`
    /// quarters, row by row.
    pub fn holes(&self, size: usize, turn: usize) -> Result<Vec<(i32, i32)>> {
        let orbits = Self::orbits(size)?;
        let by_orbit = self.holes_by_orbit(size);
        let mut holes: Vec<(i32, i32)> = self
            .pierced(size)
            .into_iter()
            .map(|orbit| orbits[orbit][(by_orbit[orbit] as usize + turn) % 4])
            .collect();
        holes.sort();
        Ok(holes)
    }

    /// The cells of the plate this grille reads, in its order: the holes at
    /// N, then at E, S and W. A plate with two readers reads the abbot's,
    /// then the novice's.
    pub fn path(&self, size: usize) -> Result<Vec<(i32, i32)>> {
        if self.part == Part::Pair {
            let mut path = self.as_part(Part::Abbot).path(size)?;
            path.extend(self.novice().path(size)?);
            return Ok(path);
        }
        let mut path = Vec::with_capacity(size * size);
        for turn in 0..4 {
            path.extend(self.holes(size, turn)?);
        }
        Ok(path)
    }

    /// One bit per orbit, from this key: true for the novice's orbits.
    fn novice_share(&self, size: usize) -> Vec<bool> {
        stream(
            &format!("cistergraphia-share:{}:{}", self.code, size),
            size * size / 4,
        )
        .into_iter()
        .map(|n| n % 2 == 1)
        .collect()
    }
}

/// A key as written anywhere, upper-cased with spaces, tabs and dashes gone
/// and the look-alikes (O for 0, I or L for 1) read as Crockford does.
pub fn normalize_code(key: &str) -> String {
    key.to_uppercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '-'))
        .map(|c| match c {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect()
}

/// `count` numbers from 0 to 3, drawn from SHA-256 of the label.
fn stream(label: &str, count: usize) -> Vec<u8> {
    let blocks = count.div_ceil(4 * 32); // four per byte, 32 bytes per digest
    let mut out = Vec::with_capacity(blocks * 128);
    for block in 0..blocks {
        let digest = Sha256::digest(format!("{label}:{block}"));
        for byte in digest {
            out.push(byte >> 6);
            out.push((byte >> 4) & 3);
            out.push((byte >> 2) & 3);
            out.push(byte & 3);
        }
    }
    out.truncate(count);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_normalize_like_crockford() {
        let grille = Grille::new("k7q2m-zxb4p", Part::Whole).unwrap();
        assert_eq!(grille.code(), "K7Q2MZXB4P");
        assert_eq!(grille.to_string(), "K7Q2M-ZXB4P");
        let look_alikes = Grille::new("oO1Il llllo", Part::Whole).unwrap();
        assert_eq!(look_alikes.code(), "0011111110");
        assert!(Grille::new("SHORT", Part::Whole).is_err());
        assert!(Grille::new("UUUUUUUUUU", Part::Whole).is_err());
    }

    #[test]
    fn orbits_cover_the_plate_once() {
        let orbits = Grille::orbits(4).unwrap();
        assert_eq!(orbits.len(), 4);
        let mut cells: Vec<(i32, i32)> = orbits.iter().flatten().copied().collect();
        cells.sort();
        cells.dedup();
        assert_eq!(cells.len(), 16);
        assert!(Grille::orbits(3).is_err());
    }

    #[test]
    fn a_path_reads_every_cell_once() {
        let grille = Grille::new(EXAMPLE, Part::Whole).unwrap();
        let mut path = grille.path(6).unwrap();
        assert_eq!(path.len(), 36);
        path.sort();
        path.dedup();
        assert_eq!(path.len(), 36);
    }

    #[test]
    fn abbot_and_novice_split_the_orbits() {
        let abbot = Grille::new(EXAMPLE, Part::Abbot).unwrap();
        let novice = abbot.novice();
        assert_eq!(novice.part(), Part::Novice);
        let mut both = abbot.pierced(6);
        both.extend(novice.pierced(6));
        both.sort();
        assert_eq!(both, (0..9).collect::<Vec<_>>());
        let pair = abbot.as_part(Part::Pair).path(6).unwrap();
        assert_eq!(pair.len(), 36);
    }
}
