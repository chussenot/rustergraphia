//! Lays a sequence out along a reading path (a layout) in an SVG document,
//! and reads it back. SVG is written as plain strings (numbers only, nothing
//! to escape) and parsed with xmltree, so the documents match the Ruby
//! library's byte for byte and read back the same way.
//!
//! A plate shows its signs pressed in wedges (stylus), one path per stroke
//! in reading order, and keeps every stroke as a hidden `<line>`: what the
//! decoder reads. Plates drawn before the wedges (visible lines only) read
//! the same. Where each sign goes is the layout's business; a weathered
//! plate's lacunae, lacunae's.

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::glyphs;
use crate::grille::{Grille, Part};
use crate::lacunae;
use crate::layout::{self, Layout};
use crate::ruby_random::RubyRandom;
use crate::stylus;
use crate::tablet;
use crate::tally;
use crate::{Line, Value};

pub const HOLE_MARGIN: i32 = 8; // between a stencil's hole and the edge of its cell

/// Damage: sequence index => the stroke positions a lacuna took (canonical,
/// upright, relative to the sign).
pub type Damage = BTreeMap<usize, Vec<Line>>;

/// The named `<line>` layers of a plate's file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Lines in no named layer: the strokes the decoder reads.
    Strokes,
    /// The stroke positions a weathered plate has lost.
    Lacunae,
    /// The tally signs.
    Tally,
}

/// Renders the sequence as a standalone SVG document along a layout's path
/// (with the strokes lost to weathering taken away: damage is weathering's).
/// The root says what else the plate is: pathless (a class), with a tablet
/// of its own (its seed, data-tablet).
pub fn render(
    sequence: &[Value],
    layout: &Layout,
    damage: &Damage,
    tally_signs: bool,
    pathless: bool,
    tablet_seed: Option<&str>,
) -> Result<String> {
    let (width, height, cols, _rows) = layout::dimensions(sequence.len(), layout, tally_signs)?;
    let signs = layout::placed(sequence, cols, layout, pathless)?;

    let mut root = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="{width}" height="{height}" viewBox="0 0 {width} {height}""#
    );
    if pathless {
        root.push_str(&format!(r#" class="{}""#, crate::pathless::CLASS));
    }
    if let Some(seed) = tablet_seed {
        root.push_str(&format!(
            r#" {}="{}""#,
            tablet::ATTRIBUTE,
            tablet::normalize(seed)?
        ));
    }

    let patches = lacunae::patches(cols, layout, damage, pathless)?;
    let wedges = pressed(&signs, layout, damage)?
        .iter()
        .map(|outline| format!(r#"    <path d="{outline}"/>"#))
        .collect::<Vec<_>>()
        .join("\n");
    let hidden = lacunae::hidden(cols, layout, damage, pathless)?;
    let tally_layer = if tally_signs {
        tally::plate::layer(sequence, cols, layout)?
    } else {
        String::new()
    };

    Ok(format!(
        "{root}>\n  <rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n  {patches}\n  <g fill=\"black\">\n{wedges}\n  </g>\n{lines}\n{hidden}\n{tally_layer}\n</svg>\n",
        lines = lines_layer(&signs, layout, damage)
    ))
}

/// Every stroke pressed by the hand, as SVG path data (one wedge each), sign
/// by sign (`layout::placed`: in reading order, the hand seeded by the
/// sign's place in it, so the same plate is always pressed the same way). A
/// turned sign is pressed upright, then turned with its cell, as the scribe
/// wrote it with the tablet turned.
pub fn pressed(
    signs: &[layout::Placed],
    layout: &Layout,
    damage: &Damage,
) -> Result<Vec<String>> {
    let mut outlines = Vec::new();
    for sign in signs {
        let empty = Vec::new();
        let lost = damage.get(&sign.index).unwrap_or(&empty);
        let strokes = lacunae::strokes(sign.num, lost);
        let origin = layout::origin(sign.row, sign.col, layout);
        let mut random = RubyRandom::new(sign.seed);
        outlines.extend(press(&strokes, sign.turned, origin, &mut random));
    }
    Ok(outlines)
}

/// One sign's wedges as path data, in its cell (turned half round if need be).
pub fn press(
    strokes: &[Line],
    turned: bool,
    (x, y): (i32, i32),
    random: &mut RubyRandom,
) -> Vec<String> {
    strokes
        .iter()
        .map(|stroke| {
            stylus::wedge(*stroke, random)
                .iter()
                .map(|polygon| {
                    let placed: Vec<(f64, f64)> = if turned {
                        polygon
                            .iter()
                            .map(|&(px, py)| {
                                (
                                    f64::from(glyphs::WIDTH) - px,
                                    f64::from(glyphs::HEIGHT) - py,
                                )
                            })
                            .collect()
                    } else {
                        polygon.clone()
                    };
                    stylus::outline(&placed, f64::from(x), f64::from(y))
                })
                .collect::<String>()
        })
        .collect()
}

/// Absolute strokes of every symbol, turned where the path says so.
pub fn strokes(signs: &[layout::Placed], layout: &Layout, damage: &Damage) -> Vec<Line> {
    let empty = Vec::new();
    signs
        .iter()
        .flat_map(|sign| {
            let (x, y) = layout::origin(sign.row, sign.col, layout);
            let lost = damage.get(&sign.index).unwrap_or(&empty);
            lacunae::strokes(sign.num, lost)
                .into_iter()
                .map(move |stroke| {
                    let stroke = if sign.turned {
                        glyphs::turn(stroke)
                    } else {
                        stroke
                    };
                    layout::relative(stroke, -x, -y)
                })
        })
        .collect()
}

/// The strokes as hidden lines: what the decoder reads (lines, not wedges).
pub fn lines_layer(signs: &[layout::Placed], layout: &Layout, damage: &Damage) -> String {
    let lines = strokes(signs, layout, damage)
        .into_iter()
        .map(line_tag)
        .collect::<Vec<_>>()
        .join("\n");
    format!("  <g stroke=\"black\" stroke-width=\"2\" display=\"none\">\n{lines}\n  </g>")
}

pub fn line_tag([x1, y1, x2, y2]: Line) -> String {
    format!(r#"    <line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}"/>"#)
}

/// Recovers the sequence from an SVG rendered by [`render`] with that
/// layout. `strokes`: the document's lines already parsed ([`lines`]), when
/// the same plate is read along several candidate layouts.
pub fn read(svg: &str, layout: &Layout, strokes: Option<&[Line]>) -> Result<Vec<Value>> {
    let owned;
    let strokes = match strokes {
        Some(lines) => lines,
        None => {
            owned = lines(svg, Layer::Strokes)?;
            &owned
        }
    };
    let mut cells: BTreeMap<(i32, i32), Vec<Line>> = BTreeMap::new();
    for line in strokes {
        cells
            .entry(layout::cell_of(*line, layout))
            .or_default()
            .push(*line);
    }
    layout::path(grid_size(cells.len())?, layout)?
        .into_iter()
        .map(|(row, col, turned)| read_cell(&cells, (row, col), turned, layout))
        .collect()
}

/// The number in one cell, its strokes made relative to the sign and turned
/// back upright.
fn read_cell(
    cells: &BTreeMap<(i32, i32), Vec<Line>>,
    (row, col): (i32, i32),
    turned: bool,
    layout: &Layout,
) -> Result<Value> {
    let strokes = cells.get(&(row, col)).ok_or_else(|| {
        Error::decode(format!("Missing symbol at row {row}, column {col}"))
    })?;
    glyphs::read(&layout::upright(strokes, row, col, turned, layout))
}

/// True for a plate written under a grille: only those are square.
pub fn is_square(svg: &str) -> bool {
    with_document(svg, |root| {
        let width = root.attributes.get("width").cloned().unwrap_or_default();
        root.name == "svg"
            && !width.is_empty()
            && Some(&width) == root.attributes.get("height")
    })
    .unwrap_or(false)
}

/// The grille as a stencil to print and cut, at the scale of its n x n
/// plate: laid on the printed plate with N at the top, it shows the first
/// quarter of the signs; turned a quarter clockwise, the next.
pub fn stencil(size: usize, grille: &Grille) -> Result<String> {
    let side = layout::SQUARE_CELL * size as i32 + layout::PADDING * 2;
    let name = match grille.part() {
        Part::Whole => "Grille",
        Part::Abbot => "Abbot\u{2019}s grille",
        Part::Novice => "Novice\u{2019}s grille",
        Part::Pair => return Err(Error::other("A stencil is cut for one part at a time")),
    };
    Ok(format!(
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{side}\" height=\"{side}\" viewBox=\"0 0 {side} {side}\">\n",
            "  <path class=\"stencil-sheet\" d=\"M0 0H{side}V{side}H0Z{holes}\" fill=\"#efe6d4\" fill-rule=\"evenodd\" stroke=\"#2b241c\" stroke-width=\"2\"/>\n",
            "  <text class=\"stencil-north\" x=\"{half}\" y=\"38\" text-anchor=\"middle\" font-family=\"Georgia, serif\" font-size=\"32\" fill=\"#a3301c\">N</text>\n",
            "  <text class=\"stencil-key\" x=\"{half}\" y=\"{bottom}\" text-anchor=\"middle\" font-family=\"Georgia, serif\" font-size=\"16\" fill=\"#62574a\">{name} {key} · turn clockwise</text>\n",
            "</svg>\n"
        ),
        side = side,
        holes = stencil_holes(size, grille)?,
        half = side / 2,
        bottom = side - 17,
        name = name,
        key = grille,
    ))
}

fn stencil_holes(size: usize, grille: &Grille) -> Result<String> {
    let hole = layout::SQUARE_CELL - 2 * HOLE_MARGIN;
    let square = Layout::Grille(grille.clone());
    Ok(grille
        .holes(size, 0)?
        .into_iter()
        .map(|(row, col)| {
            let (x, y) = layout::cell_origin(row, col, &square);
            format!(
                "M{} {}h{hole}v{hole}h-{hole}Z",
                x + HOLE_MARGIN,
                y + HOLE_MARGIN
            )
        })
        .collect())
}

/// Side of the square grid the cells form.
pub fn grid_size(cells: usize) -> Result<usize> {
    if cells == 0 {
        return Err(Error::decode("No signs found on this plate"));
    }
    let size = cells.isqrt();
    if size * size != cells {
        return Err(Error::decode(format!(
            "{cells} symbols do not form a square grid"
        )));
    }
    Ok(size)
}

/// Every `<line>` of the document as `[x1, y1, x2, y2]` integers: the
/// strokes, or (`Layer::Lacunae`) the stroke positions a weathered plate has
/// lost, or (`Layer::Tally`) the tally signs. The layers are read once per
/// file and kept for the file last read on this thread: a page lays a
/// puzzle from one file a dozen times.
pub fn lines(svg: &str, layer: Layer) -> Result<Vec<Line>> {
    LAYERS.with(|kept| {
        let mut kept = kept.borrow_mut();
        if kept.as_ref().is_none_or(|(text, _)| text != svg) {
            *kept = Some((svg.to_string(), read_layers(svg)?));
        }
        let (_, layers) = kept.as_ref().expect("just filled");
        Ok(match layer {
            Layer::Strokes => layers.strokes.clone(),
            Layer::Lacunae => layers.lacunae.clone(),
            Layer::Tally => layers.tally.clone(),
        })
    })
}

#[derive(Debug, Default, Clone)]
struct Layers {
    strokes: Vec<Line>,
    lacunae: Vec<Line>,
    tally: Vec<Line>,
}

thread_local! {
    static DOCUMENT: RefCell<Option<(String, xmltree::Element)>> = const { RefCell::new(None) };
    static LAYERS: RefCell<Option<(String, Layers)>> = const { RefCell::new(None) };
}

/// What the thread keeps of the last file read (its document, its lines),
/// let go: after a request, nothing of a reader's plate stays in memory.
pub fn forget() {
    DOCUMENT.with(|kept| *kept.borrow_mut() = None);
    LAYERS.with(|kept| *kept.borrow_mut() = None);
}

/// Runs a reading over the file's parsed document, parsed once and kept for
/// the file last parsed on this thread (read, never changed).
pub fn with_document<T>(svg: &str, read: impl FnOnce(&xmltree::Element) -> T) -> Result<T> {
    DOCUMENT.with(|kept| {
        let mut kept = kept.borrow_mut();
        if kept.as_ref().is_none_or(|(text, _)| text != svg) {
            *kept = Some((svg.to_string(), parse(svg)?));
        }
        let (_, document) = kept.as_ref().expect("just filled");
        Ok(read(document))
    })
}

fn read_layers(svg: &str) -> Result<Layers> {
    with_document(svg, |root| {
        if root.name != "svg" {
            return Err(Error::decode("Not an SVG document"));
        }
        let mut layers = Layers::default();
        collect_lines(root, None, &mut layers)?;
        Ok(layers)
    })?
}

/// Strokes are lines in no named layer; lacunae and tally signs have their
/// own. The parent's class decides, as in the Ruby library.
fn collect_lines(
    element: &xmltree::Element,
    parent_class: Option<&str>,
    layers: &mut Layers,
) -> Result<()> {
    if element.name == "line" {
        let line = line_of(element)?;
        match parent_class {
            None => layers.strokes.push(line),
            Some(lacunae::CLASS) => layers.lacunae.push(line),
            Some(tally::CLASS) => layers.tally.push(line),
            Some(_) => {}
        }
        return Ok(());
    }
    let class = element.attributes.get("class").map(String::as_str);
    for child in &element.children {
        if let xmltree::XMLNode::Element(child) = child {
            collect_lines(child, class, layers)?;
        }
    }
    Ok(())
}

fn line_of(element: &xmltree::Element) -> Result<Line> {
    let mut line = [0i32; 4];
    for (slot, attr) in line.iter_mut().zip(["x1", "y1", "x2", "y2"]) {
        *slot = coordinate(element.attributes.get(attr).map(String::as_str))?;
    }
    Ok(line)
}

pub fn parse(svg: &str) -> Result<xmltree::Element> {
    xmltree::Element::parse(svg.as_bytes())
        .map_err(|_| Error::decode("Unreadable SVG: not well-formed XML"))
}

pub fn coordinate(value: Option<&str>) -> Result<i32> {
    let text = value.ok_or_else(|| Error::decode("Invalid line coordinate: nil"))?;
    let number: f64 = text
        .trim()
        .parse()
        .map_err(|_| Error::decode(format!("Invalid line coordinate: {text:?}")))?;
    if !number.is_finite() || number.abs() > 1e9 {
        return Err(Error::decode(format!("Invalid line coordinate: {text:?}")));
    }
    Ok(number.round() as i32)
}
