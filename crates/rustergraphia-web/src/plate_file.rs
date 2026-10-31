//! A plate brought to Read, redrawn for the page from the file it came in:
//! the elements and attributes a plate is made of, nothing else
//! (web/plate_file.rb). The reader's page shows the file itself (the
//! fragments move, the lacunae are hatched), and a file posted from anywhere
//! must not bring a script, a link or a style with it.

use rustergraphia::svg;

const ELEMENTS: [&str; 8] = [
    "svg", "defs", "pattern", "g", "rect", "polygon", "path", "line",
];
const ATTRIBUTES: [&str; 25] = [
    "version",
    "width",
    "height",
    "viewBox",
    "class",
    "id",
    "fill",
    "stroke",
    "stroke-width",
    "stroke-dasharray",
    "stroke-linecap",
    "stroke-linejoin",
    "x",
    "y",
    "x1",
    "y1",
    "x2",
    "y2",
    "points",
    "d",
    "transform",
    "display",
    "opacity",
    "patternUnits",
    "patternTransform",
];
const XMLNS: &str = "http://www.w3.org/2000/svg";

/// The file as the page shows it; None when it is not a plate at all. Its
/// document is the one every reader of the request reads (the library's
/// per-thread parse).
pub fn clean(svg_text: &str) -> Option<String> {
    let written = svg::with_document(svg_text, |root| {
        if root.name != "svg" {
            return None;
        }
        let mut out = String::with_capacity(svg_text.len());
        write(root, &mut out, true);
        Some(out)
    });
    written.ok().flatten()
}

/// An element with its kept attributes and children, written out as REXML
/// would write a copy (attributes by name, double quotes, empty elements
/// closed). One not a plate's is left out with everything inside it; text is
/// never kept.
fn write(node: &xmltree::Element, out: &mut String, xmlns: bool) {
    out.push('<');
    out.push_str(&node.name);
    attributes(node, out, xmlns);
    let children: Vec<&xmltree::Element> = node
        .children
        .iter()
        .filter_map(|child| match child {
            xmltree::XMLNode::Element(element) if ELEMENTS.contains(&element.name.as_str()) => {
                Some(element)
            }
            _ => None,
        })
        .collect();
    if children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in children {
        write(child, out, false);
    }
    out.push_str("</");
    out.push_str(&node.name);
    out.push('>');
}

/// The kept attributes, by name as REXML writes them.
fn attributes(node: &xmltree::Element, out: &mut String, xmlns: bool) {
    let mut kept: Vec<(&str, &str)> = node
        .attributes
        .iter()
        .filter(|(name, value)| keep(name, value))
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    if xmlns {
        kept.push(("xmlns", XMLNS));
    }
    kept.sort_by(|a, b| a.0.cmp(b.0));
    for (name, value) in kept {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        out.push_str(&escape(value));
        out.push('"');
    }
}

/// A kept value holds no markup; escaped all the same, as REXML does.
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

fn keep(name: &str, value: &str) -> bool {
    if !(ATTRIBUTES.contains(&name) || name == "data-tablet") {
        return false;
    }
    if value.contains("url(") {
        return paint(value);
    }
    plain_value(value)
}

/// Ruby's `/\Aurl\(#[\w-]+\)\z/` (ASCII classes).
fn paint(value: &str) -> bool {
    let Some(inner) = value
        .strip_prefix("url(#")
        .and_then(|v| v.strip_suffix(')'))
    else {
        return false;
    };
    !inner.is_empty()
        && inner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Ruby's `/\A[\w\s#().,%-]*\z/` with ASCII `\w` and `\s`.
fn plain_value(value: &str) -> bool {
    value.bytes().all(|b| {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'_' | b' '
                    | b'\t'
                    | b'\n'
                    | 0x0b
                    | 0x0c
                    | b'\r'
                    | b'#'
                    | b'('
                    | b')'
                    | b'.'
                    | b','
                    | b'%'
                    | b'-'
            )
    })
}
