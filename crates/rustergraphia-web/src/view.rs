//! View helpers for the web UI (web/view_helpers.rb): signs drawn from the
//! real cipher geometry, so the illustrations on the page are always
//! truthful, and the plate's data for app.js. Every JSON blob is built by
//! hand, key by key, in the Ruby hashes' insertion order: app.js reads some
//! of it positionally, and the goldens compare bytes.

use std::sync::LazyLock;

use rustergraphia::grille::{Grille, Part};
use rustergraphia::layout::{self, Layout};
use rustergraphia::pathless::CellReading;
use rustergraphia::svg::Damage;
use rustergraphia::{glyphs, stylus, svg, Line, Value, TABLET};

use crate::esc::{h, json_str};

pub const GLYPH_VIEWBOX: &str = "-8 -8 116 166";
pub const DESCRIPTION: &str = "Write a message as Cistercian numerals coiled like a snail: a cipher game, or a passphrase-sealed secret.";
pub const ZOOM_FROM: usize = 7;

pub const LAYOUT_NAMES: [(&str, &str); 5] = [
    ("phaistos", "Phaistos"),
    ("rongorongo", "Rongorongo"),
    ("leap", "Leap"),
    ("furrows", "Furrows"),
    ("unwinding", "Unwinding"),
];

/// Every place value of the glyph tables, in Ruby `LINES`'s insertion order:
/// each place's digits 1..9.
fn line_values() -> impl Iterator<Item = Value> {
    glyphs::PLACES
        .iter()
        .flat_map(|place| (1..=9).map(move |digit| digit * place))
}

/// Every stroke a sign can have (the stem and the twenty of the four
/// corners), pressed as a wedge: "x1,y1,x2,y2" => SVG path data, in first-
/// seen order.
pub static WEDGES: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
    let mut seen: Vec<(String, String)> = Vec::new();
    let mut add = |stroke: Line| {
        let key = stroke.map(|n| n.to_string()).join(",");
        if !seen.iter().any(|(k, _)| *k == key) {
            seen.push((key, stylus::press(stroke)));
        }
    };
    add(glyphs::STEM);
    for value in line_values() {
        for stroke in glyphs::lines_of(value).expect("every place digit has strokes") {
            add(*stroke);
        }
    }
    seen
});

fn wedge_of(stroke: Line) -> &'static str {
    let key = stroke.map(|n| n.to_string()).join(",");
    WEDGES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, path)| path.as_str())
        .expect("every glyph stroke is a known wedge")
}

/// The glyph geometry, its wedges and the tablet as JSON, for the live
/// previews in app.js. Generated from the library so the browser never holds
/// a second copy of the cipher.
pub static GLYPH_TABLE_JSON: LazyLock<String> = LazyLock::new(|| {
    let mut out = String::new();
    out.push_str("{\"viewBox\":");
    out.push_str(&json_str(GLYPH_VIEWBOX));
    out.push_str(",\"stem\":");
    out.push_str(&json_line(glyphs::STEM));
    out.push_str(",\"lines\":{");
    let mut first = true;
    for value in line_values() {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&format!("\"{value}\":["));
        let strokes = glyphs::lines_of(value).expect("a place digit");
        out.push_str(
            &strokes
                .iter()
                .map(|line| json_line(*line))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push(']');
    }
    out.push_str("},\"wedges\":{");
    out.push_str(
        &WEDGES
            .iter()
            .map(|(key, path)| format!("{}:{}", json_str(key), json_str(path)))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push_str("},\"tablet\":{");
    out.push_str(
        &TABLET
            .iter()
            .map(|(c, value)| format!("{}:{}", json_str(&c.to_string()), value * 10))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push_str("}}");
    out
});

fn json_line([x1, y1, x2, y2]: Line) -> String {
    format!("[{x1},{y1},{x2},{y2}]")
}

/// One Cistercian numeral as an inline SVG, pressed: one wedge per stroke.
pub fn glyph_svg(num: Value, css: &str, title: Option<&str>) -> String {
    let wedges: String = glyphs::strokes(num, 0, 0)
        .into_iter()
        .map(|stroke| format!("<path d=\"{}\"/>", wedge_of(stroke)))
        .collect();
    let number = num.to_string();
    let label = title.unwrap_or(&number);
    format!(
        "<svg class=\"{css}\" viewBox=\"{GLYPH_VIEWBOX}\" role=\"img\" aria-label=\"{label}\"><title>{label}</title><g>{wedges}</g></svg>"
    )
}

/// Where each sign of a plate sits, in reading order, for walking the plate
/// in app.js: `[[row, col, value, turned], ...]`. A sign a lacuna took part
/// of has no value (null): the page must not hold the answer.
pub fn plate_cells_json(sequence: &[Value], layout: &Layout, damage: &Damage) -> String {
    let path = layout::path(sequence.len().isqrt(), layout).expect("a plate's own path");
    let cells: Vec<String> = path
        .iter()
        .zip(sequence)
        .enumerate()
        .map(|(index, (&(row, col, turned), &value))| {
            let value = if damage.contains_key(&index) {
                "null".to_string()
            } else {
                value.to_string()
            };
            format!("[{row},{col},{value},{turned}]")
        })
        .collect();
    format!("[{}]", cells.join(","))
}

/// Like [`plate_cells_json`] for a sequence that may already have holes (a
/// rejoined plate's signs).
pub fn plate_cells_json_opt(values: &[Option<Value>], layout: &Layout, damage: &Damage) -> String {
    let sequence: Vec<Value> = values.iter().map(|v| v.unwrap_or(0)).collect();
    let path = layout::path(sequence.len().isqrt(), layout).expect("a plate's own path");
    let cells: Vec<String> = path
        .iter()
        .zip(values)
        .enumerate()
        .map(|(index, (&(row, col, turned), value))| {
            let value = match value {
                Some(v) if !damage.contains_key(&index) => v.to_string(),
                _ => "null".to_string(),
            };
            format!("[{row},{col},{value},{turned}]")
        })
        .collect();
    format!("[{}]", cells.join(","))
}

/// A pathless plate's cells for the page, in grid order, read upright, none
/// turned: the walk before a way is chosen.
pub fn pathless_cells_json(grid: &[CellReading]) -> String {
    let cells: Vec<String> = grid
        .iter()
        .map(|cell| {
            let upright = cell.upright.map_or("null".to_string(), |v| v.to_string());
            format!("[{},{},{upright},false]", cell.row, cell.col)
        })
        .collect();
    format!("[{}]", cells.join(","))
}

/// The grid of a pathless plate itself, both readings of every cell, for the
/// desk to lay the ways over: `[[row, col, upright|null, turned|null], ...]`.
pub fn plate_grid_json(grid: &[CellReading]) -> String {
    let cells: Vec<String> = grid
        .iter()
        .map(|cell| {
            let upright = cell.upright.map_or("null".to_string(), |v| v.to_string());
            let turned = cell.turned.map_or("null".to_string(), |v| v.to_string());
            format!("[{},{},{upright},{turned}]", cell.row, cell.col)
        })
        .collect();
    format!("[{}]", cells.join(","))
}

/// A Leiden edition as HTML: [r] restored, [b/l] doubtful, the rest as read.
pub fn leiden_html(text: &str) -> String {
    let mut out = String::new();
    let mut plain = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '[' {
            plain.push(c);
            continue;
        }
        let mut bracket = String::from("[");
        let mut closed = false;
        for inner in chars.by_ref() {
            bracket.push(inner);
            if inner == ']' {
                closed = true;
                break;
            }
        }
        if !closed {
            // No closing bracket: Ruby's split leaves the tail as plain text
            plain.push_str(&bracket);
            break;
        }
        if !plain.is_empty() {
            out.push_str(&h(&plain));
            plain.clear();
        }
        let css = if bracket.contains('/') {
            "leiden-doubt"
        } else {
            "leiden-restored"
        };
        out.push_str(&format!("<span class=\"{css}\">{}</span>", h(&bracket)));
    }
    if !plain.is_empty() {
        out.push_str(&h(&plain));
    }
    out
}

/// The grid's geometry for walking it.
pub fn plate_layout_json(len: usize, layout: &Layout) -> String {
    let grille = layout.is_grille();
    let (w, h) = layout::cell(layout);
    let offset = if grille { 0 } else { -layout::SPACE / 2 };
    format!(
        "{{\"padding\":{},\"cell\":[{w},{h}],\"offset\":{offset},\"size\":{},\"grille\":{grille}}}",
        layout::PADDING,
        len.isqrt()
    )
}

/// The plate's size class, for optical weight on the page.
pub fn plate_size_class(size: usize) -> &'static str {
    if size >= 10 {
        " plate--large"
    } else if size >= 7 {
        " plate--medium"
    } else {
        ""
    }
}

pub fn layout_name(layout: &Layout) -> &'static str {
    match layout {
        Layout::Phaistos => "Phaistos",
        Layout::Rongorongo => "Rongorongo",
        Layout::Leap => "Leap",
        Layout::Furrows => "Furrows",
        Layout::Unwinding => "Unwinding",
        Layout::Grille(grille) if grille.part() == Part::Pair => "Grille, two readers",
        Layout::Grille(_) => "Grille",
    }
}

pub fn stencil_label(part: Part) -> &'static str {
    match part {
        Part::Whole => "the grille",
        Part::Abbot => "the abbot\u{2019}s grille",
        Part::Novice => "the novice\u{2019}s grille",
        Part::Pair => "the grille",
    }
}

pub struct PlateGrille {
    pub part: &'static str,
    pub label: &'static str,
    pub key: String,
    pub stencil: String,
    pub path: String,
}

pub fn part_name(part: Part) -> &'static str {
    match part {
        Part::Whole => "whole",
        Part::Abbot => "abbot",
        Part::Novice => "novice",
        Part::Pair => "pair",
    }
}

/// The grilles of a plate, each with its stencil and its reading path: the
/// grille, or the abbot's and the novice's.
pub fn plate_grilles(layout: &Grille, symbols: usize) -> Vec<PlateGrille> {
    let grilles: Vec<Grille> = if layout.part() == Part::Pair {
        vec![layout.as_part(Part::Abbot), layout.novice()]
    } else {
        vec![layout.clone()]
    };
    let size = symbols.isqrt();
    grilles
        .into_iter()
        .map(|grille| {
            let path: Vec<String> = grille
                .path(size)
                .expect("a plate's own grille path")
                .into_iter()
                .map(|(row, col)| format!("[{row},{col}]"))
                .collect();
            PlateGrille {
                part: part_name(grille.part()),
                label: stencil_label(grille.part()),
                key: grille.to_string(),
                stencil: svg::stencil(size, &grille).expect("a stencil for a plate's grille"),
                path: format!("[{}]", path.join(",")),
            }
        })
        .collect()
}

/// The rungs of the reader's record (chapter_week names them).
pub fn rung_name(rung: &str) -> &'static str {
    match rung {
        "intact" => "intact",
        "worn" => "worn",
        "pathless" => "pathless",
        "broken" => "broken",
        "tablet" => "with a tablet of its own",
        "ruined" => "ruined",
        _ => "",
    }
}
