//! Two readers, one plate. A grille's key (the abbot's) splits the plate's
//! orbits with a second grille drawn from it (the novice's, see
//! [`Grille::novice`]): the abbot's grille reads one message, the novice's
//! another. The abbot's key gives the novice's, so the abbot reads both; the
//! novice's key reads only its own.
//!
//! Each message fills its share of the plate and is padded within it: evil
//! pairs in the game, random signs when sealed. Sealed, the novice's message
//! takes the passphrase, the abbot's the passphrase with the abbot's key:
//! the novice's key says which cells are the abbot's, and with the
//! passphrase alone a novice could try the abbot's holes cell by cell.
//!
//! Not deniability: the novice's grille visibly leaves about half the plate
//! unread, and a game plate's grilles fall to the known-scheme attack. Two
//! readers with different rights, no more.

use crate::cipher;
use crate::error::Result;
use crate::grille::{Grille, Part};
use crate::layout::Layout;
use crate::ruby_random::RubyRandom;
use crate::secret;
use crate::svg;
use crate::Value;

/// What a key reads: the plate's message (reader [`Part::Whole`] for an
/// ordinary grille plate), or the abbot's with the novice's
/// ([`Part::Abbot`]), or the novice's alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub message: String,
    pub decoy: Option<String>,
    pub reader: Part,
}

/// Keys tried per plate size before a larger plate.
pub const ATTEMPTS: usize = 20;

/// (sequence, svg, the abbot's grille) for a message and a decoy.
pub fn inscribe(
    message: &str,
    decoy: &str,
    passphrase: Option<&str>,
    grille: Option<&Grille>,
) -> Result<(Vec<Value>, String, Grille)> {
    let mut random = RubyRandom::new_os();
    let fake = unpadded(decoy, passphrase, &mut random)?;
    let mut real = match passphrase {
        // sealed once the abbot's key is known
        Some(_) => None,
        None => Some(cipher::cister(message, None, &mut random)?),
    };
    let needs = [
        real.as_ref()
            .map_or_else(|| secret::max_sealed_size(message), Vec::len),
        fake.len(),
    ];
    let (abbot, size) = place(&needs, grille)?;
    if real.is_none() {
        let sealed = abbots_passphrase(passphrase.unwrap_or_default(), &abbot);
        real = Some(unpadded(message, Some(&sealed), &mut random)?);
    }
    let sequence = fill(
        &abbot,
        size,
        real.expect("filled above"),
        fake,
        passphrase.is_some(),
        &mut random,
    )?;
    let svg_text = svg::render(
        &sequence,
        &Layout::Grille(abbot.as_part(Part::Pair)),
        &svg::Damage::new(),
        false,
        false,
        None,
    )?;
    Ok((sequence, svg_text, abbot))
}

/// The passphrase the abbot's message is sealed with.
pub fn abbots_passphrase(passphrase: &str, abbot: &Grille) -> String {
    format!("{passphrase} {}", abbot.code())
}

/// What a key reads on a plate.
pub fn read(svg_text: &str, grille: &Grille, passphrase: Option<&str>) -> Result<Reading> {
    let whole = cipher::decode(svg_text, passphrase, Some(&Layout::Grille(grille.clone())));
    let error = match whole {
        Ok(message) => {
            return Ok(Reading {
                message,
                decoy: None,
                reader: Part::Whole,
            })
        }
        Err(error) if error.is_decode() => error,
        Err(error) => return Err(error),
    };
    let sealed_abbot = passphrase.map(|p| abbots_passphrase(p, grille));
    if let Some(message) = attempt(
        svg_text,
        &grille.as_part(Part::Abbot),
        sealed_abbot.as_deref(),
    )? {
        let decoy = attempt(svg_text, &grille.novice(), passphrase)?;
        return Ok(Reading {
            message,
            decoy,
            reader: Part::Abbot,
        });
    }
    match attempt(svg_text, &grille.as_part(Part::Novice), passphrase)? {
        Some(message) => Ok(Reading {
            message,
            decoy: None,
            reader: Part::Novice,
        }),
        None => Err(error),
    }
}

/// One grille's share deciphered, or `None` where it does not read.
fn attempt(svg_text: &str, grille: &Grille, passphrase: Option<&str>) -> Result<Option<String>> {
    let reading = svg::read(svg_text, &Layout::Grille(grille.clone()), None).and_then(|sequence| {
        match passphrase {
            Some(passphrase) => secret::decrypt(&sequence, passphrase),
            None => cipher::decipher(&sequence, None),
        }
    });
    match reading {
        Ok(message) => Ok(Some(message)),
        Err(error) if error.is_decode() => Ok(None),
        Err(error) => Err(error),
    }
}

/// A message's signs before padding: pairs in the game, the sealed frame in
/// secret.
fn unpadded(
    message: &str,
    passphrase: Option<&str>,
    random: &mut RubyRandom,
) -> Result<Vec<Value>> {
    match passphrase {
        Some(passphrase) => secret::seal_to_digits(message, passphrase),
        None => cipher::cister(message, None, random),
    }
}

/// The smallest even side whose square holds `count` signs.
fn even_side(count: usize) -> usize {
    let mut side = count.isqrt().max(2);
    while side * side < count || side % 2 == 1 {
        side += 1;
    }
    side
}

/// (the abbot's grille, the plate's side) where both messages fit their
/// shares. A given grille keeps its key and the plate grows; otherwise new
/// keys are drawn first, then the plate grows.
fn place(needs: &[usize; 2], grille: Option<&Grille>) -> Result<(Grille, usize)> {
    let mut size = even_side(needs[0] + needs[1]);
    loop {
        let candidates: Vec<Grille> = match grille {
            Some(grille) => vec![grille.clone()],
            None => (0..ATTEMPTS).map(|_| Grille::generate()).collect(),
        };
        for abbot in candidates {
            let shares = shares(&abbot, size)?;
            if needs[0] <= shares.0 && needs[1] <= shares.1 {
                return Ok((abbot, size));
            }
        }
        size += 2;
    }
}

fn shares(abbot: &Grille, size: usize) -> Result<(usize, usize)> {
    Ok((
        abbot.as_part(Part::Abbot).path(size)?.len(),
        abbot.novice().path(size)?.len(),
    ))
}

/// Both messages along their grilles, each padded to its share.
fn fill(
    abbot: &Grille,
    size: usize,
    real: Vec<Value>,
    fake: Vec<Value>,
    sealed: bool,
    random: &mut RubyRandom,
) -> Result<Vec<Value>> {
    let (share, novices) = shares(abbot, size)?;
    let mut sequence = pad(real, share, sealed, random);
    sequence.extend(pad(fake, novices, sealed, random));
    Ok(sequence)
}

fn pad(mut sequence: Vec<Value>, size: usize, sealed: bool, random: &mut RubyRandom) -> Vec<Value> {
    if sealed {
        while sequence.len() < size {
            sequence.push(uniform_glyph());
        }
        return sequence;
    }
    while sequence.len() < size {
        cipher::inject_evil_symbols(&mut sequence, random);
    }
    sequence
}

/// A uniform glyph value, as `SecureRandom.random_number(BASE)` draws one:
/// operating-system randomness, rejection-sampled so no value is favoured.
fn uniform_glyph() -> Value {
    let bound = u64::from(secret::BASE);
    let cap = (u64::from(u32::MAX) + 1) / bound * bound;
    loop {
        let mut bytes = [0u8; 4];
        getrandom::getrandom(&mut bytes).expect("operating system randomness");
        let drawn = u64::from(u32::from_le_bytes(bytes));
        if drawn < cap {
            return (drawn % bound) as Value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    const KEY: &str = "K7Q2M-ZXB4P";

    fn key() -> Grille {
        Grille::new(KEY, Part::Whole).unwrap()
    }

    #[test]
    fn the_novice_key_is_drawn_from_the_abbot_key_and_not_the_other_way() {
        let abbot = key();
        assert_eq!(abbot.novice(), key().novice()); // always the same
        assert_ne!(abbot.code(), abbot.novice().code());
        // the novice's key leads elsewhere
        let from_novice = Grille::new(abbot.novice().code(), Part::Whole).unwrap();
        assert_ne!(abbot.code(), from_novice.novice().code());
    }

    #[test]
    fn the_two_grilles_share_the_orbits_without_overlap() {
        for size in [2usize, 6, 10, 32] {
            let abbot = key().as_part(Part::Abbot).pierced(size);
            let novice = key().novice().pierced(size);
            assert!(abbot.iter().all(|orbit| !novice.contains(orbit)));
            let mut both = abbot;
            both.extend(novice);
            both.sort_unstable();
            assert_eq!(both, (0..size * size / 4).collect::<Vec<_>>());
        }
    }

    #[test]
    fn a_plate_with_two_readers_is_read_once_through_both_grilles() {
        let mut path = key().as_part(Part::Pair).path(12).unwrap();
        path.sort_unstable();
        path.dedup();
        assert_eq!(path.len(), 144); // every cell once
    }

    #[test]
    fn the_abbot_reads_both_messages_and_the_novice_only_the_decoy() {
        let (_sequence, svg, abbot) = inscribe(
            "Relics lie under the third pillar",
            "Vespers at six in the choir",
            None,
            None,
        )
        .unwrap();
        let reading = read(&svg, &abbot, None).unwrap();
        assert_eq!(reading.message, "relics lie under the third pillar");
        assert_eq!(
            reading.decoy.as_deref(),
            Some("vespers at six in the choir")
        );
        assert_eq!(reading.reader, Part::Abbot);

        let novice = Grille::new(abbot.novice().code(), Part::Whole).unwrap();
        let reading = read(&svg, &novice, None).unwrap();
        assert_eq!(reading.message, "vespers at six in the choir");
        assert_eq!(reading.decoy, None);
        assert_eq!(reading.reader, Part::Novice);
    }

    #[test]
    fn sealed_plates_with_two_readers() {
        let passphrase = "abbey of molesme";
        let (_sequence, svg, abbot) = inscribe(
            "Les reliques sont à Cîteaux",
            "Vêpres à six heures",
            Some(passphrase),
            None,
        )
        .unwrap();
        let reading = read(&svg, &abbot, Some(passphrase)).unwrap();
        assert_eq!(reading.message, "Les reliques sont à Cîteaux");
        assert_eq!(reading.decoy.as_deref(), Some("Vêpres à six heures"));
        assert_eq!(reading.reader, Part::Abbot);
        assert!(matches!(
            read(&svg, &abbot, Some("not the passphrase")),
            Err(Error::WrongGrille(_))
        ));
    }

    #[test]
    fn the_passphrase_alone_does_not_open_the_abbots_share() {
        let passphrase = "abbey of molesme";
        let (_sequence, svg, abbot) = inscribe(
            "Les reliques sont à Cîteaux",
            "Vêpres",
            Some(passphrase),
            None,
        )
        .unwrap();
        // the cells a novice knows are the abbot's
        let share = svg::read(&svg, &Layout::Grille(abbot.as_part(Part::Abbot)), None).unwrap();
        assert!(secret::decrypt(&share, passphrase).is_err());
        assert_eq!(
            secret::decrypt(&share, &abbots_passphrase(passphrase, &abbot)).unwrap(),
            "Les reliques sont à Cîteaux"
        );
    }

    #[test]
    fn an_ordinary_grille_plate_reads_whole() {
        let grille = key();
        let mut random = RubyRandom::new_os();
        let (_sequence, svg) = cipher::inscribe(
            "Ora et labora",
            None,
            &Layout::Grille(grille.clone()),
            false,
            false,
            None,
            &mut random,
        )
        .unwrap();
        let reading = read(&svg, &grille, None).unwrap();
        assert_eq!(reading.message, "ora et labora");
        assert_eq!(reading.decoy, None);
        assert_eq!(reading.reader, Part::Whole);
    }

    #[test]
    fn a_wrong_key_or_no_key_reads_nothing() {
        let (_sequence, svg, _) = inscribe("Relics", "Vespers", None, Some(&key())).unwrap();
        assert!(matches!(
            cipher::decode(&svg, None, None),
            Err(Error::GrilleKeyNeeded(_))
        ));
        let wrong = Grille::new("00000-00000", Part::Whole).unwrap();
        assert!(matches!(
            read(&svg, &wrong, None),
            Err(Error::WrongGrille(_))
        ));
    }

    #[test]
    fn a_given_key_grows_the_plate_until_both_messages_fit() {
        let long = "the brothers will gather in the chapter house after compline ".repeat(2);
        let (sequence, svg, abbot) = inscribe(&long, "vespers", None, Some(&key())).unwrap();
        assert_eq!(abbot, key());
        assert_eq!(sequence.len().isqrt() % 2, 0);
        assert_eq!(
            read(&svg, &abbot, None).unwrap().message,
            cipher::normalize(&long)
        );
    }
}
