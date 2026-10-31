//! The small diagrams of the reading orders (Inscribe's key, the About
//! page): each layout drawn from the real reading path, beside what it comes
//! from (web/figures.rb).

use std::sync::LazyLock;

use rustergraphia::grille::{Grille, Part};
use rustergraphia::layout::{self, Layout};

use crate::esc::round1_s;

pub fn order_label(layout: &str) -> &'static str {
    match layout {
        "phaistos" => "Reading order: a clockwise spiral from the top-left corner",
        "rongorongo" => {
            "Reading order: rows from the bottom left, every other row running back upside down"
        }
        "leap" => {
            "Reading order: one row down and two columns across at every sign, wrapping at the edge; when the orbit closes, a new start one column to the right"
        }
        "furrows" => "Reading order: down the first column, up the next, from the left",
        "unwinding" => {
            "Reading order: from the centre outward, counter-clockwise, to the top-right corner"
        }
        _ => {
            "Reading order: the signs under the grille\u{2019}s holes, row by row, then the grille turned a quarter clockwise, four times"
        }
    }
}

/// A layout's reading order over an n x n grid: dots and the path through
/// them. Hollow dots are signs written upside down.
pub fn snail_svg(size: usize, layout_name: &str) -> String {
    let layout = Layout::keyless(layout_name).expect("a keyless layout's figure");
    let cells: Vec<(i32, i32, bool)> = layout::path(size, &layout)
        .expect("a keyless path")
        .into_iter()
        .map(|(row, col, turned)| (col * 20 + 10, row * 20 + 10, turned))
        .collect();
    format!(
        "<svg class=\"snail\" viewBox=\"0 0 {side} {side}\" role=\"img\" aria-label=\"{label}\">{marks}</svg>",
        side = size * 20,
        label = order_label(layout_name),
        marks = snail_marks(&cells)
    )
}

fn snail_marks(cells: &[(i32, i32, bool)]) -> String {
    let dots: String = cells
        .iter()
        .map(|&(x, y, turned)| {
            let class = if turned { " class=\"turned\"" } else { "" };
            format!("<circle cx=\"{x}\" cy=\"{y}\" r=\"2.2\"{class}/>")
        })
        .collect();
    let (start_x, start_y, _) = cells[0];
    format!(
        "{}{dots}<circle class=\"snail-start\" cx=\"{start_x}\" cy=\"{start_y}\" r=\"4\"/>",
        snail_lines(cells)
    )
}

/// The line through the cells, broken where a step wraps across the grid.
fn snail_lines(cells: &[(i32, i32, bool)]) -> String {
    let mut runs: Vec<Vec<(i32, i32)>> = Vec::new();
    for &(x, y, _) in cells {
        let wraps = runs
            .last()
            .and_then(|run| run.last())
            .is_some_and(|&(px, py)| f64::from(x - px).hypot(f64::from(y - py)) > 50.0);
        if runs.is_empty() || wraps {
            runs.push(Vec::new());
        }
        runs.last_mut().expect("just pushed").push((x, y));
    }
    runs.into_iter()
        .map(|run| {
            format!(
                "<polyline points=\"{}\" pathLength=\"1\"/>",
                int_points(&run)
            )
        })
        .collect()
}

fn int_points(path: &[(i32, i32)]) -> String {
    path.iter()
        .map(|(x, y)| format!("{x},{y}"))
        .collect::<Vec<_>>()
        .join(" ")
}

const BOUSTROPHEDON_ROWS: [i32; 4] = [64, 48, 32, 16];

/// Cardano's grille: an ordinary letter, words as strokes ([x, length] by
/// line), and the openings ([line, word]).
const CARDANO_LETTER: [&[(i32, i32)]; 5] = [
    &[(12, 16), (32, 10), (46, 22), (72, 16)],
    &[(12, 24), (40, 14), (58, 30)],
    &[(12, 10), (26, 20), (50, 14), (68, 20)],
    &[(12, 30), (46, 18), (68, 20)],
    &[(12, 18), (34, 26), (64, 14)],
];
const CARDANO_OPENINGS: [(usize, usize); 4] = [(0, 2), (2, 1), (3, 2), (4, 0)];

pub fn cardano_svg() -> String {
    let label =
        "Cardano\u{2019}s grille: an ordinary letter, with openings over the words that carry the secret";
    let words: String = CARDANO_LETTER
        .iter()
        .enumerate()
        .flat_map(|(row, line)| {
            line.iter()
                .map(move |&(x, length)| format!("<path d=\"M{x} {}h{length}\"/>", 16 + row * 12))
        })
        .collect();
    let openings: String = CARDANO_OPENINGS
        .iter()
        .map(|&(row, word)| {
            let (x, length) = CARDANO_LETTER[row][word];
            format!(
                "<rect class=\"aperture\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"10\" rx=\"1.5\"/>",
                x - 2,
                11 + row * 12,
                length + 4
            )
        })
        .collect();
    format!(
        "<svg class=\"turning\" viewBox=\"0 0 100 80\" role=\"img\" aria-label=\"{label}\"><rect x=\"4\" y=\"4\" width=\"92\" height=\"72\" rx=\"6\"/><g class=\"turning-glyphs\">{words}</g>{openings}</svg>"
    )
}

pub fn boustrophedon_svg() -> String {
    let label =
        "Reverse boustrophedon: read left to right from the bottom, turning the tablet half round between lines";
    let ticks: String = BOUSTROPHEDON_ROWS
        .iter()
        .enumerate()
        .flat_map(|(i, &y)| {
            (20..=80).step_by(12).map(move |x| {
                if i % 2 == 0 {
                    format!("<path d=\"M{x} {}v-7h4\"/>", y - 2)
                } else {
                    format!("<path d=\"M{x} {}v7h-4\"/>", y - 9)
                }
            })
        })
        .collect();
    let path: Vec<(i32, i32)> = BOUSTROPHEDON_ROWS
        .iter()
        .enumerate()
        .flat_map(|(i, &y)| {
            if i % 2 == 0 {
                [(14, y), (86, y)]
            } else {
                [(86, y), (14, y)]
            }
        })
        .collect();
    format!(
        "<svg class=\"turning\" viewBox=\"0 0 100 80\" role=\"img\" aria-label=\"{label}\"><rect x=\"4\" y=\"4\" width=\"92\" height=\"72\" rx=\"6\"/><g class=\"turning-glyphs\">{ticks}</g><polyline points=\"{points}\" pathLength=\"1\"/><circle class=\"snail-start\" cx=\"14\" cy=\"64\" r=\"3\"/></svg>",
        points = int_points(&path)
    )
}

/// Phaistos Disc: a spiral read clockwise from the rim to the centre.
pub fn disc_spiral_svg() -> String {
    let label = "A spiral read clockwise from the rim to the centre";
    let steps = 3 * 36;
    let points: Vec<String> = (0..=steps)
        .map(|i| {
            let radius = 42.0 - (34.0 * f64::from(i) / f64::from(steps));
            let degrees = f64::from(-90 + i * 10);
            let angle = degrees * std::f64::consts::PI / 180.0;
            format!(
                "{},{}",
                round1_s(50.0 + radius * angle.cos()),
                round1_s(50.0 + radius * angle.sin())
            )
        })
        .collect();
    format!(
        "<svg class=\"turning\" viewBox=\"0 0 100 100\" role=\"img\" aria-label=\"{label}\"><circle cx=\"50\" cy=\"50\" r=\"47\"/><polyline points=\"{}\" pathLength=\"1\"/><circle class=\"snail-start\" cx=\"50\" cy=\"8\" r=\"3\"/></svg>",
        points.join(" ")
    )
}

static GRILLE_EXAMPLE: LazyLock<Grille> = LazyLock::new(|| {
    Grille::new(rustergraphia::grille::EXAMPLE, Part::Whole).expect("the example key")
});

/// A grille over a 4 x 4 grid: its holes at N, and the turn at which each
/// other cell comes under a hole.
pub fn grille_order_svg() -> String {
    let cells: String = (0..4)
        .flat_map(|turn| {
            GRILLE_EXAMPLE
                .holes(4, turn)
                .expect("the example grille's holes")
                .into_iter()
                .map(move |cell| grille_cell(cell, turn))
        })
        .collect();
    format!(
        "<svg class=\"grille-order\" viewBox=\"0 0 80 80\" role=\"img\" aria-label=\"{}\">{cells}</svg>",
        order_label("grille")
    )
}

fn grille_cell((row, col): (i32, i32), turn: usize) -> String {
    let x = col * 20;
    let y = row * 20;
    // Ruby interpolates x + 1.5 and y + 13.5 as Floats: 1.5, 41.5, 13.5
    format!(
        "<g class=\"grille-cell{hole}\"><rect x=\"{rx}\" y=\"{ry}\" width=\"17\" height=\"17\" rx=\"2\"/><text x=\"{tx}\" y=\"{ty}\">{letter}</text></g>",
        hole = if turn == 0 { " grille-hole" } else { "" },
        rx = ruby_half(x, 1.5),
        ry = ruby_half(y, 1.5),
        tx = x + 10,
        ty = ruby_half(y, 13.5),
        letter = rustergraphia::grille::TURNS[turn]
    )
}

/// `x + 1.5` printed as Ruby prints the Float: always with its decimals.
fn ruby_half(base: i32, offset: f64) -> String {
    format!("{}", f64::from(base) + offset)
}
