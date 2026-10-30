//! The game cipher and the public entry points (the Ruby library's
//! `lib/cistergraphia.rb` module functions).
//!
//! - Each character value is split into two random parts (same message, a
//!   different picture every time)
//! - A plate reads by a tablet, the shared one or one of its own (tablet)
//! - 'Evil' pairs [noise, 9999] pad the sequence to a perfect square; 9999
//!   can never come from a character, so the decoder drops them reliably
//! - The sequence is laid out along one of six reading paths (layouts);
//!   without a key the decoder tries the five keyless paths, and `inscribe`
//!   redraws any game plate that would read along more than one

use crate::error::{Error, Result};
use crate::fragments;
use crate::grille;
use crate::lacunae;
use crate::layout::{self, Layout};
use crate::pathless;
use crate::ruby_random::RubyRandom;
use crate::secret;
use crate::svg;
use crate::tablet;
use crate::{character_of, value_of, Value, EVIL_MARKER};

/// Lower-cases the message and turns everything missing from TABLET into
/// single spaces.
pub fn normalize(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut last_space = true; // leading spaces are dropped
    for c in message.to_lowercase().chars() {
        let c = if c.is_ascii_lowercase() { c } else { ' ' };
        if c == ' ' {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Full pipeline: message => sequence padded to a perfect square. With a
/// passphrase the message is encrypted (secret mode), otherwise enciphered
/// (game). `even`: a square with an even side, as a grille needs. Game
/// sequences always have one: they are made of pairs, and an even square has
/// an even side. `random`: where the splits and the evil pairs are drawn
/// from.
pub fn encode(
    message: &str,
    passphrase: Option<&str>,
    even: bool,
    tablet_seed: Option<&str>,
    random: &mut RubyRandom,
) -> Result<Vec<Value>> {
    if let Some(passphrase) = passphrase {
        return secret::encrypt(message, passphrase, even);
    }
    let mut sequence = cister(message, tablet_seed, random)?;
    while !perfect_square(sequence.len()) {
        inject_evil_symbols(&mut sequence, random);
    }
    Ok(sequence)
}

/// Full pipeline: SVG rendered by [`to_svg`] => message (normalized in game
/// mode). Without a layout, the five keyless paths are tried: only the right
/// one yields letters (game) or passes authentication (secret). A grille's
/// plate (square) can only be read with its grille. A plate with a tablet of
/// its own carries its seed: the decoder reads by that tablet.
pub fn decode(svg_text: &str, passphrase: Option<&str>, layout: Option<&Layout>) -> Result<String> {
    grille::check(svg_text, layout)?;
    if fragments::is_broken(svg_text) {
        return fragments::decode(svg_text, passphrase, layout);
    }
    if lacunae::is_weathered(svg_text)? {
        return lacunae::decode(svg_text, passphrase, layout);
    }
    read_whole(svg_text, passphrase, layout, tablet::seed_of(svg_text))
}

/// An intact plate: along the layout given, else along every keyless path,
/// the file parsed once.
pub fn read_whole(
    svg_text: &str,
    passphrase: Option<&str>,
    layout: Option<&Layout>,
    tablet_seed: Option<String>,
) -> Result<String> {
    let strokes = svg::lines(svg_text, svg::Layer::Strokes)?;
    let candidates: Vec<Layout> = match layout {
        Some(layout) => vec![layout.clone()],
        None => layout::KEYLESS.to_vec(),
    };
    let mut errors = Vec::new();
    for candidate in &candidates {
        let attempt = svg::read(svg_text, candidate, Some(&strokes)).and_then(|sequence| {
            match passphrase {
                Some(passphrase) => secret::decrypt(&sequence, passphrase),
                None => decipher(&sequence, tablet_seed.as_deref()),
            }
        });
        match attempt {
            Ok(message) => return Ok(message),
            Err(error) if error.is_decode() => errors.push(error),
            Err(error) => return Err(error),
        }
    }
    Err(failure(errors, layout, passphrase))
}

/// The error worth showing. Under a grille the reader only learns that the
/// key (or the passphrase) does not open the plate: a wrong key scrambles it.
fn failure(errors: Vec<Error>, layout: Option<&Layout>, passphrase: Option<&str>) -> Error {
    if matches!(layout, Some(Layout::Grille(_))) {
        return Error::WrongGrille(
            if passphrase.is_some() {
                grille::WRONG_KEY_OR_PASSPHRASE
            } else {
                grille::WRONG_KEY
            }
            .to_string(),
        );
    }
    let wrong_passphrase = errors
        .iter()
        .find(|e| matches!(e, Error::WrongPassphrase(_)))
        .cloned();
    wrong_passphrase.unwrap_or_else(|| errors.into_iter().next().expect("at least one candidate"))
}

/// Message => (sequence, SVG plate) along a layout. In game mode, a plate
/// that would also read as letters along another layout (short messages,
/// about 1 in 100 two-letter ones) is drawn again with new random splits, so
/// the decoder's layout detection is never wrong. Secret plates need no
/// check: the wrong layout fails authentication instead of yielding a false
/// message. With a tablet (a seed), the plate is written by a tablet of its
/// own. A seeded random draws the same game plate every time (the day's
/// chapter).
pub fn inscribe(
    message: &str,
    passphrase: Option<&str>,
    layout: &Layout,
    tally: bool,
    pathless_plate: bool,
    tablet_seed: Option<&str>,
    random: &mut RubyRandom,
) -> Result<(Vec<Value>, String)> {
    let under_grille = matches!(layout, Layout::Grille(_));
    if pathless_plate {
        pathless::check(passphrase, under_grille)?;
    }
    if tablet_seed.is_some() {
        tablet::check(passphrase, under_grille, false)?;
    }
    for _ in 0..100 {
        let sequence = encode(message, passphrase, under_grille, tablet_seed, random)?;
        let svg_text = to_svg(&sequence, layout, tally, pathless_plate, tablet_seed)?;
        if passphrase.is_some() || under_grille || unambiguous(&svg_text, layout)? {
            return Ok((sequence, svg_text));
        }
    }
    Err(Error::other(
        "Could not draw an unambiguous plate for this message",
    ))
}

/// True when the plate reads as letters along its own layout only (a path
/// that is the same walk at this plate's side is not another layout). The
/// values are the same under any tablet, so the shared one tells letters
/// from none.
pub fn unambiguous(svg_text: &str, layout: &Layout) -> Result<bool> {
    let strokes = svg::lines(svg_text, svg::Layer::Strokes)?;
    let mut cells: std::collections::BTreeSet<(i32, i32)> = std::collections::BTreeSet::new();
    for line in &strokes {
        cells.insert(layout::cell_of(*line, layout));
    }
    let side = svg::grid_size(cells.len())?;
    for other in layout::others(layout, side) {
        let reads = svg::read(svg_text, &other, Some(&strokes))
            .and_then(|sequence| decipher(&sequence, None));
        match reads {
            Ok(_) => return Ok(false),
            Err(error) if error.is_decode() => {}
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

/// Renders a sequence as an SVG document along a layout's reading path.
pub fn to_svg(
    sequence: &[Value],
    layout: &Layout,
    tally: bool,
    pathless_plate: bool,
    tablet_seed: Option<&str>,
) -> Result<String> {
    svg::render(
        sequence,
        layout,
        &svg::Damage::new(),
        tally,
        pathless_plate,
        tablet_seed,
    )
}

/// Converts a message into a sequence of numbers by the tablet: the shared
/// one, or the one a seed draws.
pub fn cister(
    message: &str,
    tablet_seed: Option<&str>,
    random: &mut RubyRandom,
) -> Result<Vec<Value>> {
    let chars: Vec<char> = normalize(message).chars().collect();
    if chars.is_empty() {
        return Err(Error::EmptyMessage(
            "Message has no characters that can be enciphered".to_string(),
        ));
    }
    let values: Vec<Value> = match tablet_seed {
        Some(seed) => {
            let drawn = tablet::draw(seed)?;
            chars
                .iter()
                .map(|c| {
                    drawn
                        .iter()
                        .find(|(ch, _)| ch == c)
                        .map(|(_, v)| *v)
                        .expect("normalize keeps only tablet characters")
                })
                .collect()
        }
        None => chars
            .iter()
            .map(|c| value_of(*c).expect("normalize keeps only tablet characters"))
            .collect(),
    };
    Ok(values
        .into_iter()
        .flat_map(|n| split(n, random))
        .collect())
}

/// Inverse of [`encode`]: drops evil symbols and joins the pairs back, each
/// read by the tablet (the shared one, or the one a seed draws).
pub fn decipher(sequence: &[Value], tablet_seed: Option<&str>) -> Result<String> {
    let values = strip_evil_symbols(sequence)?;
    if values.is_empty() {
        return Err(Error::decode("No enciphered characters found"));
    }
    if values.len() % 2 != 0 {
        return Err(Error::decode("Sequence has an odd number of values"));
    }
    let own_tablet = match tablet_seed {
        Some(seed) => Some(tablet::draw(seed)?),
        None => None,
    };
    values
        .chunks_exact(2)
        .map(|pair| {
            let sum = pair[0] + pair[1];
            let letter = match &own_tablet {
                Some(drawn) => drawn.iter().find(|(_, v)| *v == sum).map(|(c, _)| *c),
                None => character_of(sum),
            };
            letter.ok_or_else(|| {
                Error::decode(format!("{} + {} is not a known character", pair[0], pair[1]))
            })
        })
        .collect()
}

/// Splits a number into two random parts that add up to it.
pub fn split(num: Value, random: &mut RubyRandom) -> [Value; 2] {
    let part = random.rand_below(u64::from(num)) as Value;
    let mut pair = [num - part, part];
    random.shuffle(&mut pair);
    pair
}

/// Inserts a pair of 'evil' symbols between two character pairs.
pub fn inject_evil_symbols(sequence: &mut Vec<Value>, random: &mut RubyRandom) {
    let at = 2 * random.rand_int_incl(0, (sequence.len() / 2) as i64) as usize;
    let noise = random.rand_below(u64::from(EVIL_MARKER)) as Value;
    sequence.splice(at..at, [noise, EVIL_MARKER]);
}

/// Removes every [noise, EVIL_MARKER] pair. Each marker removes the value
/// right before it, which also unwinds pairs nested inside other pairs.
pub fn strip_evil_symbols(sequence: &[Value]) -> Result<Vec<Value>> {
    let mut kept = Vec::with_capacity(sequence.len());
    for &value in sequence {
        if value != EVIL_MARKER {
            kept.push(value);
            continue;
        }
        if kept.pop().is_none() {
            return Err(Error::decode("Evil symbol without its noise value"));
        }
    }
    Ok(kept)
}

/// Checks if a number is a perfect square (exact, no floating point).
pub fn perfect_square(num: usize) -> bool {
    let root = num.isqrt();
    root * root == num
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_like_ruby() {
        assert_eq!(normalize("  Ora   et LABORA!  "), "ora et labora");
        assert_eq!(normalize("café ☕ 123"), "caf");
        assert_eq!(normalize("!!!"), "");
    }

    #[test]
    fn evil_symbols_strip_even_when_nested() {
        assert_eq!(
            strip_evil_symbols(&[10, 20, 5, 9999, 30, 40]).unwrap(),
            vec![10, 20, 30, 40]
        );
        // a pair nested inside another pair unwinds too
        assert_eq!(
            strip_evil_symbols(&[10, 7, 3, 9999, 9999, 20]).unwrap(),
            vec![10, 20]
        );
        assert!(strip_evil_symbols(&[9999]).is_err());
    }

    #[test]
    fn splits_add_up() {
        let mut random = RubyRandom::new(1);
        for _ in 0..50 {
            let [a, b] = split(900, &mut random);
            assert_eq!(a + b, 900);
        }
    }

    #[test]
    fn perfect_squares() {
        assert!(perfect_square(0));
        assert!(perfect_square(16));
        assert!(!perfect_square(15));
        assert!(perfect_square(1 << 40));
    }
}
