//! The lacunae of a weathered plate, in its SVG: drawn, hidden, read back.
//!
//! INTERIM STUBS: the full port lands with the weathering wave. What is
//! already real here is what intact plates need: `strokes` (a sign's strokes
//! without those a lacuna took) and the weathered-file test.

use crate::error::{Error, Result};
use crate::glyphs;
use crate::layout::Layout;
use crate::svg::{self, Damage};
use crate::{Line, Value};

pub const CLASS: &str = "lacunae";
pub const HATCH: &str = "#b3a993"; // the edge of a break, as in the pattern below
/// The hatch of a break, made of rects (the decoder reads every `<line>`
/// outside the lacunae).
pub const PATTERN: &str =
    "<defs><pattern id=\"worn\" width=\"8\" height=\"8\" patternUnits=\"userSpaceOnUse\" \
patternTransform=\"rotate(45)\"><rect width=\"8\" height=\"8\" fill=\"#ece6da\"/>\
<rect width=\"2\" height=\"8\" fill=\"#b3a993\"/></pattern></defs>";
pub const SEALED: &str = "A worn plate is a game plate: read it without a passphrase";
pub const NOT_SEALED: &str =
    "A sealed plate cannot wear: with one stroke lost, it would never open again";
pub const NOT_TWO_READERS: &str = "A plate for two readers cannot wear yet";

/// Whether a plate has lacunae (a string test first: intact plates are not
/// parsed twice).
pub fn is_weathered(svg_text: &str) -> Result<bool> {
    if !svg_text.contains(r#"class="lacunae""#) {
        return Ok(false);
    }
    Ok(!svg::lines(svg_text, svg::Layer::Lacunae)?.is_empty())
}

/// A sign's strokes, without those a lacuna took.
pub fn strokes(num: Value, lost: &[Line]) -> Vec<Line> {
    glyphs::strokes(num, 0, 0)
        .into_iter()
        .filter(|stroke| !lost.contains(&glyphs::canonical(*stroke)))
        .collect()
}

/// The worn patches, drawn under the wedges ('' for an intact plate).
pub fn patches(_cols: usize, _layout: &Layout, damage: &Damage, _pathless: bool) -> Result<String> {
    if damage.is_empty() {
        return Ok(String::new());
    }
    Err(Error::other("Weathered plates: port pending"))
}

/// The lost stroke positions as hidden lines: what the decoder reads.
pub fn hidden(_cols: usize, _layout: &Layout, damage: &Damage, _pathless: bool) -> Result<String> {
    if damage.is_empty() {
        return Ok(String::new());
    }
    Err(Error::other("Weathered plates: port pending"))
}

/// The restored text of a weathered plate, in the Leiden conventions.
pub fn decode(
    _svg_text: &str,
    _passphrase: Option<&str>,
    _layout: Option<&Layout>,
) -> Result<String> {
    Err(Error::other("Weathered plates: port pending"))
}

/// A damaged sign as the drawing layers see it: the seed of its roughness,
/// the stroke positions it lost (canonical, upright), whether it lies turned,
/// and its sign's origin on the plate (or the fragment's frame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WornSign {
    pub seed: i64,
    pub lost: Vec<Line>,
    pub turned: bool,
    pub origin: (i32, i32),
}

/// The worn patches of some signs, filled with the PATTERN the document
/// defines once.
pub fn worn(signs: &[WornSign]) -> String {
    let polygons: Vec<String> = signs
        .iter()
        .flat_map(|sign| {
            let mut random = crate::ruby_random::RubyRandom::new(sign.seed);
            outline::shapes(&sign.lost)
                .into_iter()
                .map(|shape| {
                    outline::polygon(
                        &outline::rough(&shape, &mut random)
                            .into_iter()
                            .map(|point| outline::place(point, sign.turned, sign.origin))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    format!(
        "  <g class=\"weathering\" fill=\"url(#worn)\" stroke=\"{HATCH}\" stroke-width=\"1.5\">\n{}\n  </g>",
        polygons.join("\n")
    )
}

/// The lost stroke positions of some signs as hidden lines, in their own
/// layer.
pub fn lost_lines(signs: &[WornSign]) -> String {
    let lines: Vec<String> = signs
        .iter()
        .flat_map(|sign| {
            let (x, y) = sign.origin;
            sign.lost.iter().map(move |&stroke| {
                let stroke = if sign.turned {
                    glyphs::turn(stroke)
                } else {
                    stroke
                };
                svg::line_tag(crate::layout::relative(stroke, -x, -y))
            })
        })
        .collect();
    format!(
        "  <g class=\"{CLASS}\" stroke=\"black\" stroke-width=\"2\" display=\"none\">\n{}\n  </g>",
        lines.join("\n")
    )
}

/// The shapes of a break, drawn in a sign's upright frame and placed on the
/// plate.
pub mod outline {
    use crate::glyphs;
    use crate::ruby_random::RubyRandom;
    use crate::{Line, Value};

    pub const BAND: f64 = 7.0; // half width of a worn band along one stroke
    pub const JITTER: i64 = 3; // how ragged a break's edge is

    /// Each quadrant's five stroke positions: the strokes of 1, 2, 3, 4 and 6.
    fn positions(place: Value) -> Vec<Line> {
        [1, 2, 3, 4, 6]
            .iter()
            .map(|&digit| glyphs::canonical(glyphs::digit_strokes(place, digit)[0]))
            .collect()
    }

    /// The outlines to wear: a whole quadrant as its box, otherwise a band
    /// along each lost stroke. Grouped by place in the order the lost
    /// strokes name them, as Ruby's group_by does.
    pub fn shapes(lost: &[Line]) -> Vec<Vec<(f64, f64)>> {
        let mut places: Vec<(Value, Vec<Line>)> = Vec::new();
        for &stroke in lost {
            let place = glyphs::stroke_place(stroke).expect("a lost stroke is a stroke position");
            match places.iter_mut().find(|(p, _)| *p == place) {
                Some((_, strokes)) => strokes.push(stroke),
                None => places.push((place, vec![stroke])),
            }
        }
        places
            .into_iter()
            .flat_map(|(place, strokes)| {
                if strokes.len() == positions(place).len() {
                    vec![boxed(place)]
                } else {
                    strokes.iter().map(|&stroke| band(stroke)).collect()
                }
            })
            .collect()
    }

    /// A whole quadrant's box.
    fn boxed(place: Value) -> Vec<(f64, f64)> {
        let points: Vec<(i32, i32)> = positions(place)
            .iter()
            .flat_map(|&[x1, y1, x2, y2]| [(x1, y1), (x2, y2)])
            .collect();
        let x1 = points.iter().map(|p| p.0).min().expect("five positions");
        let x2 = points.iter().map(|p| p.0).max().expect("five positions");
        let y1 = points.iter().map(|p| p.1).min().expect("five positions");
        let y2 = points.iter().map(|p| p.1).max().expect("five positions");
        [
            (x1 - 5, y1 - 5),
            (x2 + 5, y1 - 5),
            (x2 + 5, y2 + 5),
            (x1 - 5, y2 + 5),
        ]
        .iter()
        .map(|&(x, y)| (f64::from(x), f64::from(y)))
        .collect()
    }

    /// A rectangle around a stroke, a little longer than it.
    fn band([x1, y1, x2, y2]: Line) -> Vec<(f64, f64)> {
        let (dx, dy) = unit(f64::from(x2 - x1), f64::from(y2 - y1));
        let ends = [
            shift((f64::from(x1), f64::from(y1)), (dx, dy), -4.0),
            shift((f64::from(x2), f64::from(y2)), (dx, dy), 4.0),
        ];
        [(0, 1.0), (1, 1.0), (1, -1.0), (0, -1.0)]
            .iter()
            .map(|&(end, side)| shift(ends[end], (-dy * BAND, dx * BAND), side))
            .collect()
    }

    /// The direction of a stroke, as a vector of length 1.
    fn unit(width: f64, height: f64) -> (f64, f64) {
        let length = width.hypot(height);
        (width / length, height / length)
    }

    fn shift((x, y): (f64, f64), (dx, dy): (f64, f64), amount: f64) -> (f64, f64) {
        (x + dx * amount, y + dy * amount)
    }

    /// A broken edge: each side cut in three, every point moved a little.
    pub fn rough(corners: &[(f64, f64)], random: &mut RubyRandom) -> Vec<(f64, f64)> {
        let mut points = Vec::with_capacity(corners.len() * 3);
        for (i, &a) in corners.iter().enumerate() {
            let b = corners[(i + 1) % corners.len()];
            points.extend(thirds(a, b));
        }
        points
            .into_iter()
            .map(|(x, y)| {
                let jx = random.rand_int_incl(-JITTER, JITTER) as f64;
                let jy = random.rand_int_incl(-JITTER, JITTER) as f64;
                (x + jx, y + jy)
            })
            .collect()
    }

    fn thirds((ax, ay): (f64, f64), (bx, by): (f64, f64)) -> Vec<(f64, f64)> {
        (0..=2)
            .map(|k| {
                let k = f64::from(k);
                (ax + (bx - ax) * k / 3.0, ay + (by - ay) * k / 3.0)
            })
            .collect()
    }

    /// An upright point of a sign, on the plate (turned with its sign if
    /// need be).
    pub fn place((x, y): (f64, f64), turned: bool, (ox, oy): (i32, i32)) -> (f64, f64) {
        let (x, y) = if turned {
            (f64::from(glyphs::WIDTH) - x, f64::from(glyphs::HEIGHT) - y)
        } else {
            (x, y)
        };
        (x + f64::from(ox), y + f64::from(oy))
    }

    pub fn polygon(points: &[(f64, f64)]) -> String {
        let listed: Vec<String> = points
            .iter()
            .map(|&(x, y)| format!("{x:.1},{y:.1}"))
            .collect();
        format!("    <polygon points=\"{}\"/>", listed.join(" "))
    }
}
