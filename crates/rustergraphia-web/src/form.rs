//! What the forms ask for, read from the request (web/form_params.rb,
//! web/plate_input.rb, web/refusals.rb): the mode (by its passphrase), the
//! layout (a grille by its key), the rung, and the refusals in the library's
//! exact words.

use std::collections::BTreeMap;

use rustergraphia::grille::{Grille, Part};
use rustergraphia::layout::Layout;
use rustergraphia::{fragments, lacunae, pathless, tablet};

use crate::error::AppError;

pub const MAX_MESSAGE_LENGTH: usize = 500;
pub const MAX_PAIR_LENGTH: usize = 250;
pub const MAX_SVG_BYTES: usize = 2 * 1024 * 1024;

/// The request's params: query merged with body, body winning; the last
/// value of a repeated key wins; plus the multipart upload, if any.
#[derive(Debug, Default)]
pub struct Params {
    values: BTreeMap<String, String>,
    pub upload: Option<Vec<u8>>,
}

impl Params {
    pub fn get(&self, name: &str) -> &str {
        self.values.get(name).map_or("", String::as_str)
    }

    pub fn set(&mut self, name: String, value: String) {
        self.values.insert(name, value);
    }

    /// `params.select! { |name, _| name == 'rejoined' }` for the chapter.
    pub fn keep_only(&mut self, name: &str) {
        self.values.retain(|key, _| key == name);
        self.upload = None;
    }

    /// Parse an application/x-www-form-urlencoded body or query string into
    /// this set (later keys win).
    pub fn parse_urlencoded(&mut self, text: &[u8]) {
        for pair in text.split(|&b| b == b'&') {
            if pair.is_empty() {
                continue;
            }
            let mut sides = pair.splitn(2, |&b| b == b'=');
            let name = decode_component(sides.next().unwrap_or_default());
            let value = decode_component(sides.next().unwrap_or_default());
            if !name.is_empty() {
                self.values.insert(name, value);
            }
        }
    }
}

/// Percent-decoding with `+` as space, invalid sequences kept literally,
/// lossy UTF-8 (as Rack tolerates).
pub fn decode_component(text: &[u8]) -> String {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.iter().copied().peekable();
    while let Some(b) = bytes.next() {
        match b {
            b'+' => out.push(b' '),
            b'%' => {
                let hex: Vec<u8> = bytes.clone().take(2).collect();
                if hex.len() == 2 && hex.iter().all(u8::is_ascii_hexdigit) {
                    let value = u8::from_str_radix(std::str::from_utf8(&hex).unwrap_or("0"), 16)
                        .unwrap_or(b'%');
                    out.push(value);
                    bytes.next();
                    bytes.next();
                } else {
                    out.push(b'%');
                }
            }
            other => out.push(other),
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The mode switch wins when present; otherwise a non-blank passphrase means
/// secret mode. Secret mode with a blank one gives `Some("")`.
pub fn passphrase(params: &Params) -> Option<String> {
    if params.get("mode") == "game" {
        return None;
    }
    let value = params.get("passphrase").to_string();
    if value.is_empty() && params.get("mode") != "secret" {
        return None;
    }
    Some(value)
}

/// What `chosen_layout` found, with what the form must show.
pub struct ChosenLayout {
    pub layout: Layout,
    pub grille_key: String,
    pub grille_error: Option<String>,
}

/// Phaistos unless another layout is asked for. A grille takes the key given
/// or a new one; a key that is not one is kept for the form to explain.
pub fn chosen_layout(params: &Params) -> ChosenLayout {
    match params.get("layout") {
        name @ ("rongorongo" | "leap" | "furrows" | "unwinding") => ChosenLayout {
            layout: Layout::keyless(name).expect("a keyless name"),
            grille_key: String::new(),
            grille_error: None,
        },
        "grille" => {
            let key = params.get("grille");
            if key.trim().is_empty() {
                return ChosenLayout {
                    layout: Layout::Grille(Grille::generate()),
                    grille_key: String::new(),
                    grille_error: None,
                };
            }
            match Grille::new(key, Part::Whole) {
                Ok(grille) => ChosenLayout {
                    layout: Layout::Grille(grille),
                    grille_key: String::new(),
                    grille_error: None,
                },
                Err(error) => ChosenLayout {
                    layout: Layout::Phaistos,
                    grille_key: key.to_string(),
                    grille_error: Some(error.to_string()),
                },
            }
        }
        _ => ChosenLayout {
            layout: Layout::Phaistos,
            grille_key: String::new(),
            grille_error: None,
        },
    }
}

/// The grille to read with, when its key is given; an invalid key raises.
pub fn submitted_grille(params: &Params) -> Result<Option<Grille>, AppError> {
    let key = params.get("grille");
    if key.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(Grille::new(key, Part::Whole)?))
}

/// The plate's condition as `(weathered, broken)`.
pub fn conditions(condition: &str) -> (bool, bool) {
    match condition {
        "worn" => (true, false),
        "broken" => (false, true),
        "ruined" => (true, true),
        _ => (false, false),
    }
}

pub fn valid_condition(params: &Params) -> String {
    match params.get("condition") {
        rung @ ("intact" | "worn" | "broken" | "ruined") => rung.to_string(),
        _ => "intact".to_string(),
    }
}

/// The refusals (web/refusals.rb), in the library's words.
pub struct Refusals;

impl Refusals {
    pub fn tablet_problem(
        passphrase: Option<&str>,
        layout: &Layout,
        weathered: bool,
        broken: bool,
    ) -> Option<&'static str> {
        if passphrase.is_some() {
            return Some(tablet::NOT_SEALED);
        }
        if layout.is_grille() {
            return Some(tablet::NOT_UNDER_A_GRILLE);
        }
        if weathered || broken {
            return Some(tablet::NOT_DAMAGED);
        }
        None
    }

    pub fn path_problem(
        passphrase: Option<&str>,
        layout: &Layout,
        broken: bool,
    ) -> Option<&'static str> {
        if passphrase.is_some() {
            return Some(pathless::NOT_SEALED);
        }
        if layout.is_grille() {
            return Some(pathless::NOT_UNDER_A_GRILLE);
        }
        if broken {
            return Some(pathless::NOT_BROKEN);
        }
        None
    }

    pub fn condition_problem(
        condition: &str,
        passphrase: Option<&str>,
        layout: &Layout,
        decoy: bool,
    ) -> Option<&'static str> {
        if condition == "intact" {
            return None;
        }
        let (_, broken) = conditions(condition);
        if broken {
            if passphrase.is_some() {
                return Some(fragments::NOT_SEALED);
            }
            if layout.is_grille() {
                return Some(fragments::NOT_UNDER_A_GRILLE);
            }
            if !matches!(layout, Layout::Phaistos) {
                return Some(fragments::NOT_ALONG_THE_ROWS);
            }
            return None;
        }
        if passphrase.is_some() {
            return Some(lacunae::NOT_SEALED);
        }
        if decoy {
            return Some(lacunae::NOT_TWO_READERS);
        }
        None
    }
}

/// `submitted_svg` (web/plate_input.rb): the upload (if non-empty) else the
/// pasted text; too large a file is refused before anything reads it.
pub fn submitted_svg(params: &Params) -> Result<String, AppError> {
    let svg = match &params.upload {
        Some(upload) if !upload.is_empty() => String::from_utf8_lossy(upload).into_owned(),
        _ => params.get("svg").to_string(),
    };
    if svg.len() > MAX_SVG_BYTES {
        return Err(AppError::halt(
            413,
            format!("SVG is limited to {} KB", MAX_SVG_BYTES / 1024),
        ));
    }
    Ok(svg)
}
