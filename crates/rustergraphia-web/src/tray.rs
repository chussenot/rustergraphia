//! The board's data and counts (web/tray_helpers.rb): what the page needs to
//! let a reader move fragments, and the words for how many there are.

use rustergraphia::fragments::Fragment;
use rustergraphia::layout::{self, Layout};

use crate::esc::json_str;
use crate::view::LAYOUT_NAMES;

/// The fragments as the page's JSON holds them: the grid's geometry, every
/// reading path, the shared tablet, and each fragment's cells read upright
/// and turned (or null), with whether a lacuna touched them.
pub fn fragments_json(fragments: &[Fragment], size: usize) -> String {
    let (w, h) = layout::cell(&Layout::Phaistos);
    let board = layout::PADDING - layout::SPACE / 2;
    let mut out =
        format!("{{\"cell\":[{w},{h}],\"board\":{board},\"gap\":40,\"size\":{size},\"paths\":{{");
    let paths: Vec<String> = LAYOUT_NAMES
        .iter()
        .map(|(name, _)| {
            let layout = Layout::keyless(name).expect("a keyless layout");
            let steps: Vec<String> = layout::path(size, &layout)
                .expect("a keyless path")
                .into_iter()
                .map(|(row, col, turned)| format!("[{row},{col},{turned}]"))
                .collect();
            format!("{}:[{}]", json_str(name), steps.join(","))
        })
        .collect();
    out.push_str(&paths.join(","));
    out.push_str("},\"tablet\":{");
    let tablet: Vec<String> = rustergraphia::character_sums()
        .map(|sum| {
            let c = rustergraphia::character_of(sum).expect("a tablet sum");
            format!("\"{sum}\":{}", json_str(&c.to_string()))
        })
        .collect();
    out.push_str(&tablet.join(","));
    out.push_str(&format!("}},\"null\":{}", rustergraphia::EVIL_MARKER));
    // Ruby: "tablet" then "null" then "fragments"; wait — null sits inside? No:
    // {…,"tablet":{…},"null":9999,"fragments":[…]}
    out.push_str(",\"fragments\":[");
    let listed: Vec<String> = fragments
        .iter()
        .map(|fragment| {
            let signs: Vec<String> = fragment
                .cells()
                .into_iter()
                .map(|cell| {
                    let reading = fragment.readings.get(&cell).expect("a fragment cell reads");
                    format!(
                        "[{},{},{}]",
                        side(&reading.upright),
                        side(&reading.turned),
                        reading.worn
                    )
                })
                .collect();
            format!(
                "{{\"rows\":{},\"cols\":{},\"signs\":[{}]}}",
                fragment.rows,
                fragment.cols,
                signs.join(",")
            )
        })
        .collect();
    out.push_str(&listed.join(","));
    out.push_str("]}");
    out
}

fn side(values: &Option<Vec<rustergraphia::Value>>) -> String {
    match values {
        None => "null".to_string(),
        Some(values) => format!(
            "[{}]",
            values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

pub fn lacunae_count(n: usize) -> String {
    if n == 1 {
        "1 lacuna".to_string()
    } else {
        format!("{n} lacunae")
    }
}

pub fn fragments_count(n: usize) -> String {
    if n == 1 {
        "1 fragment".to_string()
    } else {
        format!("{n} fragments")
    }
}
