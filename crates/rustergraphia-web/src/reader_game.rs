//! The reader's game (web/reader_game.rb): a weathered or broken game plate
//! brought to Read opens as its puzzle, the bench to restore it or the board
//! to rejoin it, built from the file alone.

use rustergraphia::grille::Grille;
use rustergraphia::layout::{Layout, KEYLESS};
use rustergraphia::svg::Damage;
use rustergraphia::{cipher, fragments, grille, lacunae, notebook, pathless, svg, tablet, Value};

use crate::bench;
use crate::error::AppError;
use crate::plate_file;
use crate::views::play::PlayState;

/// What the file says of the plate: worn, broken, pathless, with its own
/// tablet.
#[derive(Debug, Clone, Copy)]
pub struct Marks {
    pub weathered: bool,
    pub broken: bool,
    pub pathless: bool,
    pub own: bool,
}

pub fn read_marks(svg_text: &str) -> Result<Marks, rustergraphia::Error> {
    Ok(Marks {
        weathered: lacunae::is_weathered(svg_text)?,
        broken: fragments::is_broken(svg_text),
        pathless: pathless::is_pathless(svg_text),
        own: tablet::seed_of(svg_text).is_some(),
    })
}

/// Whether the plate brought to Read opens as its puzzle rather than its
/// reading, and the laid page when it does.
pub fn playable(
    svg_text: &str,
    passphrase: Option<&str>,
    decipher: bool,
    rejoined: bool,
    submitted: Option<&Grille>,
) -> Result<Option<PlayState>, AppError> {
    if passphrase.is_some() || decipher {
        return Ok(None);
    }
    let marks = read_marks(svg_text)?;
    if marks.own && (marks.weathered || marks.broken) {
        return Ok(None); // two to find: the bench and the board speak in letters
    }
    let Some(cleaned) = plate_file::clean(svg_text) else {
        return Ok(None);
    };
    lay(svg_text, cleaned, marks, rejoined, submitted)
}

/// The puzzle the plate opens as: the board for a broken plate (the bench on
/// it rejoined, when asked), the bench for a worn one, the desk for an
/// intact one.
pub fn lay(
    svg_text: &str,
    cleaned: String,
    marks: Marks,
    rejoined: bool,
    submitted: Option<&Grille>,
) -> Result<Option<PlayState>, AppError> {
    let mut state = PlayState {
        broken: marks.broken,
        rejoined: false,
        weathered: marks.weathered,
        expert: false,
        grid: None,
        notebook: None,
        desk: false,
        chapter: None,
        entries: Vec::new(),
        values: Vec::new(),
        worn: Vec::new(),
        fragments: None,
        size: 0,
        tally: None,
        layout: Layout::Phaistos,
        svg_content: cleaned,
        plate_svg: None,
        damage: Damage::new(),
    };
    if marks.broken && marks.weathered && rejoined {
        lay_rejoined(svg_text, &mut state)?;
        return Ok(Some(state));
    }
    if marks.broken {
        lay_board(svg_text, &mut state)?;
        return Ok(Some(state));
    }
    let laid = if marks.weathered {
        lay_bench(svg_text, marks, submitted, &mut state)?
    } else {
        lay_desk(svg_text, marks, submitted, &mut state)?
    };
    Ok(laid.then_some(state))
}

/// The bench on a ruined plate rejoined: the plate drawn from its own
/// fragments moved into place, its lacunae from the file alone.
fn lay_rejoined(svg_text: &str, state: &mut PlayState) -> Result<(), AppError> {
    let whole = fragments::rejoined(svg_text)?;
    state.layout = whole.layout.clone();
    state.values = whole.signs.clone();
    state.damage = whole.damage.clone();
    state.entries = bench::lacunae_from(
        whole.signs.len(),
        &state.layout,
        &whole.damage,
        &whole.candidates,
    );
    state.plate_svg = plate_file::clean(&whole.svg);
    state.rejoined = true;
    state.broken = false;
    state.size = state.values.len().isqrt();
    Ok(())
}

/// The board: the fragments in the order the file lists them.
fn lay_board(svg_text: &str, state: &mut PlayState) -> Result<(), AppError> {
    let fragments = fragments::read(svg_text)?;
    state.size = fragments::join::side(&fragments)?;
    state.fragments = Some(fragments);
    Ok(())
}

/// The reading desk: an intact game plate, along the layout that reads it as
/// letters (or the grille given).
fn lay_desk(
    svg_text: &str,
    marks: Marks,
    submitted: Option<&Grille>,
    state: &mut PlayState,
) -> Result<bool, AppError> {
    let submitted_layout = submitted.map(|g| Layout::Grille(g.clone()));
    grille::check(svg_text, submitted_layout.as_ref())?;
    let (layout, values) = match &submitted_layout {
        Some(layout) => (Some(layout.clone()), letters(svg_text, layout)),
        None => {
            let mut found = None;
            for layout in &KEYLESS {
                if let Some(values) = letters(svg_text, layout) {
                    found = Some((layout.clone(), values));
                    break;
                }
            }
            match found {
                Some((layout, values)) => (Some(layout), Some(values)),
                None => (None, None),
            }
        }
    };
    let (Some(layout), Some(values)) = (layout, values) else {
        return Ok(false);
    };
    state.layout = layout;
    state.values = values.iter().map(|&v| Some(v)).collect();
    state.size = state.values.len().isqrt();
    // A pathless plate lays its ways itself; a plate with its own tablet
    // opens the notebook (shut on the expert plate)
    if marks.pathless && submitted.is_none() {
        lay_pathless(svg_text, state)?;
    }
    state.expert = marks.own && marks.pathless;
    if marks.own {
        state.notebook = Some(if state.expert {
            Vec::new()
        } else {
            let intact: Vec<Value> = state.values.iter().map(|v| v.unwrap_or(0)).collect();
            notebook::entries(&intact)
        });
    }
    state.desk = true;
    Ok(true)
}

/// A pathless plate: the page gets every cell's readings in grid order and
/// not the way, nor the values along it.
fn lay_pathless(svg_text: &str, state: &mut PlayState) -> Result<(), AppError> {
    let grid = pathless::grid(svg_text)?;
    state.layout = Layout::Phaistos;
    state.values = grid
        .iter()
        .map(|cell| Some(cell.upright.unwrap_or(0)))
        .collect();
    state.grid = Some(grid);
    Ok(())
}

/// The plate's values along a layout when every pair reads as a letter.
fn letters(svg_text: &str, layout: &Layout) -> Option<Vec<Value>> {
    let sequence = svg::read(svg_text, layout, None).ok()?;
    cipher::decipher(&sequence, None).ok()?;
    Some(sequence)
}

/// The bench: along the layout the survey finds, or the grille given, when
/// it reads the plate.
fn lay_bench(
    svg_text: &str,
    marks: Marks,
    submitted: Option<&Grille>,
    state: &mut PlayState,
) -> Result<bool, AppError> {
    let layout = match submitted {
        Some(grille) => Some(Layout::Grille(grille.clone())),
        None => lacunae::survey::layout(svg_text)?,
    };
    let Some(layout) = layout else {
        return Ok(false);
    };
    if submitted.is_some() && lacunae::survey::edition(svg_text, &layout).is_none() {
        return Ok(false);
    }
    state.layout = layout;
    state.values = lacunae::survey::values(svg_text, &state.layout)?;
    state.size = state.values.len().isqrt();
    state.tally = rustergraphia::tally::plate::read(svg_text, &state.layout)?;
    if marks.pathless {
        lay_pathless(svg_text, state)?;
        state.worn = bench::worn_cells(svg_text)?;
        state.entries = Vec::new();
        state.damage = Damage::new();
    } else {
        let damaged = lacunae::survey::damaged(svg_text, &state.layout)?;
        state.damage = damaged
            .iter()
            .map(|sign| (sign.index, sign.lost.clone()))
            .collect();
        state.entries = bench::lacunae_in(svg_text, &state.layout, &state.values)?;
    }
    Ok(true)
}

/// The scribe's answer to the notebook: the pencilled entries that hold.
pub fn scribe_answer(svg_text: &str, params: &crate::form::Params) -> Result<String, AppError> {
    let (pencil, inked) = notebook_sent(params)?;
    let held = notebook::held(svg_text, &pencil, &inked)?;
    // JSON keys in `open` order: Notebook.held keeps the open sums' order
    let plate = notebook::plate(svg_text)?;
    let mut ordered: Vec<(Value, char)> = Vec::new();
    for sum in &plate.1 {
        if inked.contains(sum) {
            continue;
        }
        if let Some(&c) = held.get(sum) {
            ordered.push((*sum, c));
        }
    }
    let pairs: Vec<String> = ordered
        .iter()
        .map(|(sum, c)| format!("\"{sum}\":{}", crate::esc::json_str(&c.to_string())))
        .collect();
    Ok(format!("{{{}}}", pairs.join(",")))
}

/// `[pencil, inked]` from the page's JSON, parsed as Ruby does
/// (notebook_sent.rs).
fn notebook_sent(
    params: &crate::form::Params,
) -> Result<(std::collections::BTreeMap<Value, char>, Vec<Value>), AppError> {
    crate::notebook_sent::read(params.get("pencil"), params.get("inked"))
        .ok_or_else(|| AppError::Domain("The notebook could not be read: ask again".to_string()))
}
