//! Secret mode: real encryption behind the Cistercian glyphs.
//!
//! The message (any UTF-8 text) is encrypted with AES-256-GCM under a key
//! derived from a passphrase with scrypt. The resulting bytes are written as
//! base-10000 digits, one glyph each, so the picture looks like the game mode
//! but carries no structure: no tablet, no pairs, no evil marker. Without the
//! passphrase only the approximate message length can be learned, and GCM
//! rejects any tampered glyph.
//!
//! Payload layout (before base-10000 encoding):
//!   version (1 byte) | salt (16) | nonce (12) | ciphertext | GCM tag (16)
//! The version byte selects the KDF and cipher parameters, so they can be
//! strengthened later while old images stay decodable. It is authenticated as
//! associated data.
//!
//! Glyph sequence layout:
//!   [length / 10000, length % 10000, *payload digits, *random padding]
//! Padding glyphs are uniformly random, like the payload, and fill the square.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use num_bigint::BigUint;

use crate::error::{Error, Result};
use crate::Value;

pub const VERSION: u8 = 1;
/// OWASP-listed equivalent of N=2^17, r=8, p=1 that needs 16 MiB instead of 128 MiB
pub const SCRYPT_LOG_N: u8 = 14;
pub const SCRYPT_R: u32 = 8;
pub const SCRYPT_P: u32 = 5;
pub const SALT_BYTES: usize = 16;
pub const NONCE_BYTES: usize = 12;
pub const TAG_BYTES: usize = 16;
pub const HEADER_BYTES: usize = 1 + SALT_BYTES + NONCE_BYTES;
pub const MIN_PASSPHRASE_LENGTH: usize = 8;
pub const WRONG_PASSPHRASE: &str =
    "Wrong passphrase, or the image was not made in secret mode or was altered";
pub const BASE: Value = 10_000;

/// Ruby's `String#strip`: ASCII whitespace and NUL taken off both ends.
fn strip(message: &str) -> &str {
    message.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r' | '\0'))
}

/// Why a passphrase cannot seal a message, or `None` when it can. The one
/// copy of the rule: the encoder enforces it, the web form and the CLI
/// explain it.
pub fn passphrase_problem(passphrase: &str) -> Option<String> {
    if passphrase.is_empty() {
        return Some("Secret mode needs a passphrase".to_string());
    }
    if passphrase.chars().count() < MIN_PASSPHRASE_LENGTH {
        return Some(format!(
            "A passphrase needs at least {MIN_PASSPHRASE_LENGTH} characters"
        ));
    }
    None
}

/// Message + passphrase => sequence of numbers (0..9999) padded to a perfect
/// square (with an even side if `even`, as a grille needs).
pub fn encrypt(message: &str, passphrase: &str, even: bool) -> Result<Vec<Value>> {
    Ok(pad_to_square(seal_to_digits(message, passphrase)?, even))
}

/// Message + passphrase => the framed signs (length, then payload), unpadded.
pub fn seal_to_digits(message: &str, passphrase: &str) -> Result<Vec<Value>> {
    if let Some(problem) = passphrase_problem(passphrase) {
        return Err(Error::Other(problem));
    }
    let plaintext = strip(message);
    if plaintext.is_empty() {
        return Err(Error::EmptyMessage("Message is empty".to_string()));
    }
    Ok(frame(to_digits(&seal(plaintext, passphrase)?)))
}

/// The most signs `seal_to_digits` can give for a message, without sealing it
/// (the count depends on the random salt and nonce, by one sign at most).
pub fn max_sealed_size(message: &str) -> usize {
    let bytes = HEADER_BYTES + strip(message).len() + TAG_BYTES;
    // ((2 * 256**bytes) - 1) has the most base-10000 digits any payload of
    // that many bytes (with the 0x01 guard in front) can have
    let largest = (BigUint::from(2u32) << (8 * bytes)) - 1u32;
    base_10000_digit_count(&largest) + 2
}

/// Inverse of `encrypt`; fails on a wrong passphrase or any tampering.
pub fn decrypt(sequence: &[Value], passphrase: &str) -> Result<String> {
    if sequence.len() < 2 {
        return Err(Error::decode("Too few symbols for a secret message"));
    }
    let length = (sequence[0] as usize * BASE as usize) + sequence[1] as usize;
    let digits = sequence
        .get(2..2 + length)
        .ok_or_else(|| Error::decode("Secret message is truncated"))?;
    unseal(&from_digits(digits)?, passphrase)
}

/// Two length glyphs, then the payload glyphs.
pub fn frame(digits: Vec<Value>) -> Vec<Value> {
    let mut framed = Vec::with_capacity(digits.len() + 2);
    framed.push(digits.len() as Value / BASE);
    framed.push(digits.len() as Value % BASE);
    framed.extend(digits);
    framed
}

/// Random glyphs, indistinguishable from the ciphertext, fill the square grid.
pub fn pad_to_square(mut sequence: Vec<Value>, even: bool) -> Vec<Value> {
    let mut size = sequence.len().isqrt();
    if size * size != sequence.len() {
        size += 1;
    }
    if even && size % 2 == 1 {
        size += 1;
    }
    while sequence.len() < size * size {
        sequence.push(secure_below(BASE));
    }
    sequence
}

fn seal(plaintext: &str, passphrase: &str) -> Result<Vec<u8>> {
    let mut header = vec![VERSION];
    let mut random = [0u8; SALT_BYTES + NONCE_BYTES];
    getrandom::getrandom(&mut random).expect("operating system randomness");
    header.extend_from_slice(&random);
    let cipher = aead(passphrase, &header)?;
    let nonce = Nonce::from_slice(&header[1 + SALT_BYTES..]);
    let sealed = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext.as_bytes(),
                aad: &header,
            },
        )
        .map_err(|_| Error::other("Could not seal the message"))?;
    header.extend(sealed);
    Ok(header)
}

fn unseal(payload: &[u8], passphrase: &str) -> Result<String> {
    let (header, body_and_tag) = split_payload(payload)?;
    // No plate was ever sealed with a passphrase this short: wrong, without
    // the scrypt work
    if passphrase.chars().count() < MIN_PASSPHRASE_LENGTH {
        return Err(Error::WrongPassphrase(WRONG_PASSPHRASE.to_string()));
    }
    let cipher = aead(passphrase, header)?;
    let nonce = Nonce::from_slice(&header[1 + SALT_BYTES..]);
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: body_and_tag,
                aad: header,
            },
        )
        .map_err(|_| Error::WrongPassphrase(WRONG_PASSPHRASE.to_string()))?;
    String::from_utf8(plaintext).map_err(|_| Error::decode("Corrupted secret message"))
}

/// The header, then the ciphertext with its tag (as AES-GCM reads them).
fn split_payload(payload: &[u8]) -> Result<(&[u8], &[u8])> {
    if payload.len() < HEADER_BYTES + TAG_BYTES {
        return Err(Error::decode("Secret message is too short"));
    }
    let version = payload[0];
    if version != VERSION {
        return Err(Error::decode(format!(
            "Unsupported secret format version {version}"
        )));
    }
    Ok((&payload[..HEADER_BYTES], &payload[HEADER_BYTES..]))
}

/// AES-256-GCM keyed from the passphrase and the salt in the header, with
/// the whole header (version, salt, nonce) authenticated as associated data.
fn aead(passphrase: &str, header: &[u8]) -> Result<Aes256Gcm> {
    let key = key(passphrase, &header[1..1 + SALT_BYTES])?;
    Ok(Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key)))
}

fn key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let params = scrypt::Params::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P, 32)
        .map_err(|_| Error::other("Invalid key derivation parameters"))?;
    let mut key = [0u8; 32];
    scrypt::scrypt(passphrase.as_bytes(), salt, &params, &mut key)
        .map_err(|_| Error::other("Key derivation failed"))?;
    Ok(key)
}

/// Bytes => base-10000 digits, most significant first. A leading 0x01 byte
/// keeps leading zero bytes.
pub fn to_digits(bytes: &[u8]) -> Vec<Value> {
    let mut guarded = Vec::with_capacity(bytes.len() + 1);
    guarded.push(1u8);
    guarded.extend_from_slice(bytes);
    let mut n = BigUint::from_bytes_be(&guarded);
    let base = BigUint::from(BASE);
    let mut digits = Vec::new();
    while n > BigUint::ZERO {
        let rest = &n % &base;
        digits.push(
            rest.iter_u32_digits()
                .next()
                .map(|d| d as Value)
                .unwrap_or(0),
        );
        n /= &base;
    }
    if digits.is_empty() {
        digits.push(0); // Ruby's Integer#digits of 0 is [0]; unreachable with the guard
    }
    digits.reverse();
    digits
}

pub fn from_digits(digits: &[Value]) -> Result<Vec<u8>> {
    if digits.iter().any(|&d| d >= BASE) {
        return Err(Error::decode("Invalid secret symbol"));
    }
    let base = BigUint::from(BASE);
    let mut acc = BigUint::ZERO;
    for &digit in digits {
        acc = acc * &base + digit;
    }
    let bytes = acc.to_bytes_be();
    if bytes.first() != Some(&1) {
        return Err(Error::decode("Corrupted secret message"));
    }
    Ok(bytes[1..].to_vec())
}

/// How many base-10000 digits a number has (Ruby's `digits(BASE).size`).
fn base_10000_digit_count(n: &BigUint) -> usize {
    let base = BigUint::from(BASE);
    let mut n = n.clone();
    let mut count = 0;
    loop {
        count += 1;
        n /= &base;
        if n == BigUint::ZERO {
            return count;
        }
    }
}

/// `SecureRandom.random_number(n)`: uniform in 0..n from the operating
/// system, by rejection.
fn secure_below(n: Value) -> Value {
    let zone = u32::MAX - (u32::MAX % n);
    loop {
        let mut bytes = [0u8; 4];
        getrandom::getrandom(&mut bytes).expect("operating system randomness");
        let drawn = u32::from_le_bytes(bytes);
        if drawn < zone {
            return drawn % n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSPHRASE: &str = "correct horse battery staple";

    #[test]
    fn round_trip_keeps_the_exact_message() {
        let message =
            "Rendez-vous à 23h00, porte nord — 7 moines 🙏\nNo normalisation: CAPS, digits & punctuation!";
        let sequence = encrypt(message, PASSPHRASE, false).unwrap();
        assert_eq!(decrypt(&sequence, PASSPHRASE).unwrap(), message);
    }

    #[test]
    fn sequence_is_a_perfect_square_of_valid_glyph_values() {
        let sequence = encrypt("meet me at dawn", PASSPHRASE, false).unwrap();
        let size = sequence.len().isqrt();
        assert_eq!(size * size, sequence.len());
        assert!(sequence.iter().all(|&n| n <= 9999));
    }

    #[test]
    fn an_even_side_when_asked() {
        let sequence = encrypt("meet me at dawn", PASSPHRASE, true).unwrap();
        let size = sequence.len().isqrt();
        assert_eq!(size * size, sequence.len());
        assert_eq!(size % 2, 0);
    }

    #[test]
    fn same_message_gives_a_different_image_every_time() {
        let a = encrypt("same message", PASSPHRASE, false).unwrap();
        let b = encrypt("same message", PASSPHRASE, false).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn wrong_passphrase_is_rejected() {
        let sequence = encrypt("top secret", PASSPHRASE, false).unwrap();
        let error = decrypt(&sequence, "correct horse battery stapler").unwrap_err();
        assert!(error.is_decode());
        assert!(error.to_string().starts_with("Wrong passphrase"));
    }

    #[test]
    fn any_altered_glyph_is_detected() {
        let sequence = encrypt("top secret", PASSPHRASE, false).unwrap();
        let length = (sequence[0] as usize * 10_000) + sequence[1] as usize;
        for index in [2, 2 + (length / 2), 1 + length] {
            let mut tampered = sequence.clone();
            tampered[index] = (tampered[index] + 1) % 10_000;
            assert!(decrypt(&tampered, PASSPHRASE).is_err(), "glyph {index}");
        }
    }

    #[test]
    fn padding_glyphs_are_not_authenticated_but_not_needed_either() {
        for n in 1..=20 {
            let message = "x".repeat(n);
            let mut sequence = encrypt(&message, PASSPHRASE, false).unwrap();
            let used = 2 + (sequence[0] as usize * 10_000) + sequence[1] as usize;
            if sequence.len() > used {
                let last = sequence.len() - 1;
                sequence[last] = (sequence[last] + 1) % 10_000;
                assert_eq!(decrypt(&sequence, PASSPHRASE).unwrap(), message);
                return;
            }
        }
        panic!("no padded length found");
    }

    #[test]
    fn short_passphrases_are_refused() {
        let error = encrypt("hello", "short", false).unwrap_err();
        assert!(error.to_string().contains("at least 8"));
    }

    #[test]
    fn passphrase_problem_explains_the_rule() {
        assert_eq!(
            passphrase_problem("").as_deref(),
            Some("Secret mode needs a passphrase")
        );
        assert_eq!(
            passphrase_problem("1234567").as_deref(),
            Some("A passphrase needs at least 8 characters")
        );
        assert_eq!(passphrase_problem("12345678"), None);
        assert_eq!(passphrase_problem(&"🙏".repeat(8)), None); // characters, not bytes
    }

    #[test]
    fn a_short_passphrase_never_opens_a_plate() {
        let sequence = encrypt("top secret", PASSPHRASE, false).unwrap();
        assert!(matches!(
            decrypt(&sequence, "short"),
            Err(Error::WrongPassphrase(_))
        ));
    }

    #[test]
    fn empty_message_is_refused() {
        assert!(matches!(
            encrypt("  \n ", PASSPHRASE, false),
            Err(Error::EmptyMessage(_))
        ));
    }

    #[test]
    fn unknown_format_version_is_rejected() {
        let mut payload = vec![2u8];
        payload.extend(std::iter::repeat_n(0u8, 60));
        let digits = to_digits(&payload);
        let sequence = frame(digits);
        let error = decrypt(&sequence, PASSPHRASE).unwrap_err();
        assert!(error.to_string().contains("version 2"));
    }

    #[test]
    fn truncated_sequences_are_rejected() {
        let sequence = encrypt("top secret", PASSPHRASE, false).unwrap();
        assert!(decrypt(&sequence[..5], PASSPHRASE).is_err());
        assert!(decrypt(&[9], PASSPHRASE).is_err());
    }

    #[test]
    fn digits_round_trip_keeps_leading_zero_bytes() {
        let bytes = [0u8, 0, 0xFF, 0];
        assert_eq!(from_digits(&to_digits(&bytes)).unwrap(), bytes);
    }

    #[test]
    fn the_sealed_size_has_an_upper_bound_without_sealing() {
        for message in ["a", "Les reliques sont à Cîteaux", &"x".repeat(500)] {
            let sizes: Vec<usize> = (0..5)
                .map(|_| seal_to_digits(message, PASSPHRASE).unwrap().len())
                .collect();
            let bound = max_sealed_size(message);
            assert!(sizes.iter().max().unwrap() <= &bound);
            assert!(sizes.iter().min().unwrap() >= &(bound - 1));
        }
    }
}
