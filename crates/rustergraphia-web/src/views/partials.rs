//! The shared furniture: _plate, _board, _bench, _desk, _notebook,
//! _notebook_row and _ways (views/_*.erb), byte for byte. Every partial's
//! string ends with the file's final newline, as `erb :_x` returns it.

use rustergraphia::layout::Layout;
use rustergraphia::notebook;
use rustergraphia::tally::Signs;
use rustergraphia::{svg::Damage, Value};

use crate::bench::{
    bench_edition, bench_sum, bench_tallies, bench_verdict, entries_json, pathless_lacunae,
    place_name, worn_cells_json, Entry, WornCell,
};
use crate::esc::{h, pad4};
use crate::figures::snail_svg;
use crate::tray::{fragments_count, fragments_json};
use crate::view::{glyph_svg, plate_size_class, GLYPH_TABLE_JSON, LAYOUT_NAMES, ZOOM_FROM};

/// _ways.erb. Local: at ('desk' or 'bench').
pub fn ways(at: &str) -> String {
    let buttons: String = LAYOUT_NAMES
        .iter()
        .map(|(way, title)| {
            format!(
                "      <button type=\"button\" class=\"segment segment--way\" data-{at}-way=\"{way}\" aria-pressed=\"false\">\n        <span class=\"segment-figure\" aria-hidden=\"true\">{figure}</span>\n        <span class=\"segment-title\">{title}</span>\n      </button>\n",
                figure = snail_svg(4, way),
            )
        })
        .collect();
    format!(
        "<div class=\"ways-ask\">\n  <p class=\"ways-question\" id=\"{at}-question\">Which way does it run?</p>\n  <div class=\"segments segments--ways ways\" role=\"group\" aria-labelledby=\"{at}-question\">\n{buttons}  </div>\n</div>\n"
    )
}

/// _notebook_row.erb. Local: entry.
pub fn notebook_row(entry: &notebook::Entry) -> String {
    let shown: String = entry
        .spellings
        .iter()
        .take(notebook::SPELLINGS)
        .map(|&(a, b)| format!("<span class=\"spelling\">{} + {}</span>", pad4(a), pad4(b)))
        .collect();
    let more = if entry.spellings.len() > 1 {
        let wide = if entry.spellings.len() > notebook::SPELLINGS {
            " spelling-more--wide"
        } else {
            ""
        };
        format!("<span class=\"spelling-more{wide}\">…</span>")
    } else {
        String::new()
    };
    let options: String = ('a'..='z')
        .chain([' '])
        .map(|c| {
            format!(
                "        <option value=\"{c}\">{}</option>\n",
                if c == ' ' { '␣' } else { c }
            )
        })
        .collect();
    format!(
        "<tr data-entry-row=\"{sum}\">\n  <td class=\"notebook-sum\">{padded}</td>\n  <td class=\"notebook-seen\">{seen}</td>\n  <td class=\"notebook-spellings\">{shown}{more}</td>\n  <td class=\"notebook-slot\">\n    <label class=\"visually-hidden\" for=\"entry-{sum}\">What {sum} reads as</label>\n    <select id=\"entry-{sum}\" class=\"input notebook-entry\" data-entry=\"{sum}\">\n      <option value=\"\">·</option>\n{options}    </select>\n  </td>\n</tr>\n",
        sum = entry.sum,
        padded = pad4(entry.sum),
        seen = entry.seen,
    )
}

/// _notebook.erb. Locals: entries, values, shut.
pub fn notebook_partial(
    entries: &[notebook::Entry],
    values: &[Option<Value>],
    shut: bool,
) -> String {
    let note = "Each sum is one letter, however its pair is spelled. Write a letter in pencil against a sum; the scribe confirms entries three at a time.";
    let sums = format!(
        "[{}]",
        entries
            .iter()
            .map(|entry| entry.sum.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let shut_attrs = if shut {
        format!(
            " data-notebook-shut data-notebook-open-note=\"{}\"",
            h(note)
        )
    } else {
        String::new()
    };
    let line = if shut {
        h(&"·".repeat(values.len() / 2))
    } else {
        let intact: Vec<Value> = values.iter().map(|v| v.unwrap_or(0)).collect();
        "<span>·</span>".repeat(notebook::pairs(&intact).len())
    };
    let rows: String = entries
        .iter()
        .map(|entry| format!("          {}\n", notebook_row(entry)))
        .collect();
    let template = if shut {
        let empty = notebook::Entry {
            sum: 0,
            seen: 0,
            spellings: Vec::new(),
        };
        format!(
            "<template data-notebook-row>{}</template>",
            notebook_row(&empty)
        )
    } else {
        String::new()
    };
    format!(
        "<section class=\"notebook awaits-script{is_shut}\" aria-labelledby=\"notebook-title\" data-notebook data-notebook-sums=\"{sums}\"{shut_attrs}>\n  <div class=\"notebook-head\">\n    <h2 id=\"notebook-title\" class=\"index-heading\">The notebook</h2>\n    <span class=\"notebook-count\" data-notebook-count aria-live=\"polite\">{count}</span>\n  </div>\n  <p class=\"desk-text notebook-line{line_class}\" data-desk-text aria-live=\"polite\">{line}</p>\n  <p class=\"notebook-note\" data-notebook-note>{note_text}</p>\n  <div class=\"notebook-scroll\">\n    <table class=\"notebook-table\">\n      <thead><tr><th scope=\"col\">Sum</th><th scope=\"col\" class=\"notebook-seen\">Seen</th><th scope=\"col\">Spelled</th><th scope=\"col\">Reads as</th></tr></thead>\n      <tbody data-notebook-rows>\n{rows}      </tbody>\n    </table>\n    {template}\n  </div>\n  <div class=\"notebook-ask\">\n    <button type=\"button\" class=\"btn btn--primary\" data-notebook-ask{disabled}>Ask the scribe</button>\n    <p class=\"notebook-verdict\" data-notebook-verdict aria-live=\"polite\">The scribe answers only when three entries hold at once.</p>\n  </div>\n</section>\n",
        is_shut = if shut { " is-shut" } else { "" },
        sums = h(&sums),
        count = if shut {
            "Shut until the way is found".to_string()
        } else {
            format!("0 of {} in ink · asked 0 times", entries.len())
        },
        line_class = if shut { "" } else { " desk-text--notebook" },
        note_text = if shut {
            "Find the way first: three pairs along it whose sums are values of the tablet open the notebook, on every sum the way makes."
        } else {
            note
        },
        disabled = if shut { " disabled" } else { "" },
    )
}

/// _desk.erb. Locals: values, tablet (the JSON of value => letter, `{}` for a
/// plate with its own tablet), pathless, own.
pub fn desk(values: &[Option<Value>], tablet_json: &str, pathless: bool, own: bool) -> String {
    let expert = pathless && own;
    let data_values = if own {
        let mut sums: Vec<Value> = rustergraphia::character_sums().collect();
        sums.sort_unstable();
        format!(
            " data-values=\"[{}]\"",
            sums.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
    } else {
        String::new()
    };
    let ways_line = if pathless {
        ways("desk")
    } else {
        String::new()
    };
    let sign = |index: usize| -> String {
        if pathless {
            String::new()
        } else {
            let titles = ["The first sign of the pair", "The second sign of the pair"];
            glyph_svg(
                values[index].unwrap_or(0),
                "glyph desk-glyph",
                Some(titles[index]),
            )
        }
    };
    let feedback = if expert {
        "Nothing says where it begins, nor what its sums read. Choose a way: its sums are the tablet’s values, or not."
    } else if pathless {
        "Nothing says where the plate begins. Choose a way: the pairs form along it, and read as letters or do not."
    } else if own {
        "Type what each sign reads: the page adds them. The sum is a letter only the notebook knows."
    } else {
        "Type what each sign reads: the page adds them and finds the letter on the tablet. Guided, a tap on a sign shows its value."
    };
    let text_line = if own {
        String::new()
    } else {
        format!(
            "<p class=\"desk-text\" data-desk-text aria-live=\"polite\">{}</p>",
            h(&"·".repeat(values.len() / 2))
        )
    };
    let notebook_line = if pathless {
        format!(
            "    <p class=\"desk-notebook\" data-desk-notebook aria-live=\"polite\">{}</p>\n",
            if expert {
                "The ways tried, and where each broke."
            } else {
                "The notebook: what each way gave, and where it broke."
            }
        )
    } else {
        String::new()
    };
    format!(
        "<section class=\"desk awaits-script\" aria-labelledby=\"desk-title\" data-desk data-desk-mode=\"guided\"{pathless_attr}{own_attr}\n         data-tablet=\"{tablet}\"{data_values} data-null=\"{null}\" data-glyph-table=\"{glyph_table}\">\n  <div class=\"desk-head\">\n    <h2 id=\"desk-title\" class=\"index-heading\">Read it</h2>\n    <div class=\"segments desk-modes\" role=\"group\" aria-label=\"How to read\">\n      <button type=\"button\" class=\"segment\" data-desk-set-mode=\"guided\" aria-pressed=\"true\"><span class=\"segment-title\">Guided</span></button>\n      <button type=\"button\" class=\"segment\" data-desk-set-mode=\"unaided\" aria-pressed=\"false\"><span class=\"segment-title\">Unaided</span></button>\n    </div>\n  </div>\n  {ways_line}\n  <div class=\"desk-nav\">\n    <button type=\"button\" class=\"btn btn--small\" data-desk-step=\"-1\" aria-label=\"Previous pair\">‹</button>\n    <span class=\"desk-which\" data-desk-which aria-live=\"polite\">{which}</span>\n    <button type=\"button\" class=\"btn btn--small\" data-desk-step=\"1\" aria-label=\"Next pair\">›</button>\n  </div>\n  <form class=\"desk-pair\" data-desk-form>\n    <div class=\"desk-sign\" data-desk-sign=\"0\">{sign0}</div>\n    <label class=\"visually-hidden\" for=\"desk-first\">What the first sign reads</label>\n    <input type=\"number\" id=\"desk-first\" class=\"input desk-value\" min=\"0\" max=\"9999\" inputmode=\"numeric\" autocomplete=\"off\" placeholder=\"first\" data-desk-value=\"0\">\n    <span class=\"desk-op\" aria-hidden=\"true\">+</span>\n    <div class=\"desk-sign\" data-desk-sign=\"1\">{sign1}</div>\n    <label class=\"visually-hidden\" for=\"desk-second\">What the second sign reads</label>\n    <input type=\"number\" id=\"desk-second\" class=\"input desk-value\" min=\"0\" max=\"9999\" inputmode=\"numeric\" autocomplete=\"off\" placeholder=\"second\" data-desk-value=\"1\">\n    <span class=\"desk-op\" aria-hidden=\"true\">=</span>\n    <output class=\"desk-letter\" data-desk-letter aria-label=\"The letter the pair makes\">?</output>\n    <button type=\"submit\" class=\"btn btn--primary btn--small\" data-desk-read>Read</button>\n  </form>\n  <p class=\"desk-feedback\" data-desk-feedback aria-live=\"polite\">{feedback}</p>\n  {text_line}\n{notebook_line}  <p class=\"lectio-standing desk-standing\"><span data-desk-rank>Novice</span> · <span data-desk-streak>0</span> in a row · best <span data-desk-best>0</span></p>\n</section>\n",
        pathless_attr = if pathless { " data-desk-pathless" } else { "" },
        own_attr = if own { " data-desk-own" } else { "" },
        tablet = h(tablet_json),
        null = rustergraphia::EVIL_MARKER,
        glyph_table = h(&GLYPH_TABLE_JSON),
        which = if pathless {
            "Choose a way to read".to_string()
        } else {
            format!("Pair 1 of {} · signs 1 and 2", values.len() / 2)
        },
        sign0 = sign(0),
        sign1 = sign(1),
    )
}

/// _bench.erb. Locals: entries, values, damage, tally, pathless, worn.
#[allow(clippy::too_many_arguments)]
pub fn bench(
    entries: &[Entry],
    values: &[Option<Value>],
    damage: &Damage,
    tally: Option<&Signs>,
    pathless: bool,
    worn: &[WornCell],
) -> String {
    let first = entries.first();
    let count = if pathless {
        pathless_lacunae(worn)
    } else {
        entries.len()
    };
    let tally_attr = match tally {
        Some(signs) => format!("\n         data-tally=\"{}\"", h(&tally_json(signs))),
        None => String::new(),
    };
    let worn_attr = if pathless {
        format!(
            "\n         data-bench-worn=\"{}\"",
            h(&worn_cells_json(worn))
        )
    } else {
        String::new()
    };
    let ways_line = if pathless {
        ways("bench")
    } else {
        String::new()
    };
    let palette: String = (0..10u32)
        .map(|digit| {
            let enabled = first.is_some_and(|f| f.digits.contains(&digit));
            let place = first.map_or(1, |f| f.place);
            format!(
                "      <button type=\"button\" class=\"bench-digit\" data-digit=\"{digit}\" aria-pressed=\"false\"{disabled}>\n        {glyph}<span>{digit}</span>\n      </button>\n",
                disabled = if enabled { "" } else { " disabled" },
                glyph = glyph_svg(
                    digit * place,
                    "glyph bench-glyph",
                    Some(&format!("{digit} in the {}", place_name(place)))
                ),
            )
        })
        .collect();
    let tally_line = match tally {
        Some(_) => format!(
            "\n    <span class=\"bench-verdict\" data-bench-tally>{}</span>",
            match first {
                Some(first) => h(&bench_tallies(first)),
                None => "Tallies: row ?, column ?".to_string(),
            }
        ),
        None => String::new(),
    };
    let notebook_line = if pathless {
        "    <p class=\"bench-notebook\" data-bench-notebook aria-live=\"polite\">The notebook: what each way gave.</p>\n"
    } else {
        ""
    };
    format!(
        "<section class=\"bench awaits-script\" aria-labelledby=\"bench-title\" data-bench{pathless_attr}\n         data-lacunae=\"{lacunae}\" data-glyph-table=\"{glyph_table}\"{tally_attr}{worn_attr}>\n  <div class=\"bench-head\">\n    <h2 id=\"bench-title\" class=\"index-heading\">Restore it</h2>\n    <div class=\"bench-nav\">\n      <button type=\"button\" class=\"btn btn--small\" data-bench-step=\"-1\" aria-label=\"Previous lacuna\">‹</button>\n      <span class=\"bench-which\" data-bench-which aria-live=\"polite\">{which}</span>\n      <button type=\"button\" class=\"btn btn--small\" data-bench-step=\"1\" aria-label=\"Next lacuna\">›</button>\n    </div>\n  </div>\n  {ways_line}\n  <div class=\"bench-palette\" role=\"group\" aria-label=\"Digits to press into the lacuna\">\n{palette}  </div>\n  <p class=\"bench-balance\" aria-live=\"polite\"><span class=\"bench-sum\" data-bench-sum>{sum}</span>\n    <span class=\"bench-verdict\" data-bench-verdict>{verdict}</span>{tally_line}\n    <span class=\"bench-verdict\" data-bench-count>0 of {count} restored</span></p>\n  <p class=\"bench-edition\" data-bench-edition>{edition}</p>\n{notebook_line}</section>\n",
        pathless_attr = if pathless { " data-bench-pathless" } else { "" },
        lacunae = h(&entries_json(entries)),
        glyph_table = h(&GLYPH_TABLE_JSON),
        which = match first {
            Some(first) => h(&format!(
                "Lacuna 1 of {count}: the {}",
                place_name(first.place)
            )),
            None => "Choose a way to restore".to_string(),
        },
        sum = match first {
            Some(first) => h(&bench_sum(values, entries, first)),
            None => "? + ? = ?".to_string(),
        },
        verdict = match first {
            Some(first) => bench_verdict(values, first).to_string(),
            None => "Choose a way: the pairs form along it.".to_string(),
        },
        edition = if first.is_some() {
            h(&bench_edition(values, damage))
        } else {
            "[.]".repeat(values.len() / 2)
        },
    )
}

pub fn tally_json(signs: &Signs) -> String {
    format!(
        "{{\"rows\":[{}],\"cols\":[{}],\"corner\":{}}}",
        join(&signs.rows),
        join(&signs.cols),
        signs.corner
    )
}

fn join(values: &[Value]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// _plate.erb. Locals: svg, cells, geometry, layout, decoy, size, grid.
#[allow(clippy::too_many_arguments)]
pub fn plate(
    svg: &str,
    cells: &str,
    geometry: &str,
    layout: &Layout,
    decoy: bool,
    size: usize,
    grid: Option<&str>,
) -> String {
    let zoomable = size >= ZOOM_FROM;
    let grid_attr = match grid {
        Some(grid) => format!(" data-plate-grid=\"{}\"", h(grid)),
        None => String::new(),
    };
    let zoom = if zoomable {
        "      <div class=\"plate-zoom awaits-script\" role=\"group\" aria-label=\"Zoom on the plate\" data-plate-zoom>\n        <button type=\"button\" class=\"plate-zoom-button\" data-zoom-step=\"-1\" aria-label=\"Zoom out\" title=\"Zoom out (−)\" disabled>−</button>\n        <button type=\"button\" class=\"plate-zoom-button plate-zoom-level\" data-zoom-fit data-zoom-level aria-label=\"×1: the whole plate\" title=\"The whole plate (0)\" disabled>×1</button>\n        <button type=\"button\" class=\"plate-zoom-button\" data-zoom-step=\"1\" aria-label=\"Zoom in\" title=\"Zoom in (+)\">+</button>\n        <span class=\"visually-hidden\" data-zoom-said aria-live=\"polite\"></span>\n      </div>\n"
    } else {
        ""
    };
    let rest = if grid.is_some() {
        "The plate says nothing of where it begins. Choose a way below: the walk follows it."
    } else if matches!(layout, Layout::Rongorongo) {
        "Read from the bottom left, left to right; turn the plate half round for each next line."
    } else if layout.is_grille() && decoy {
        "Two grilles share this plate: the abbot’s reads the message, the novice’s the other one. The abbot’s key gives both; the novice’s, only its own."
    } else if layout.is_grille() {
        "Read through the grille: N at the top, the holes row by row, then a quarter turn clockwise. Without it, the plate is an anagram of itself."
    } else {
        "Read clockwise from the top-left corner, coiling inward."
    };
    format!(
        "<figure class=\"plate{size_class}\">\n  <div class=\"plate-view\">\n    <div class=\"plate-frame{zoom_class}\" id=\"svg-display\" data-plate tabindex=\"0\" role=\"group\"\n         aria-label=\"The plate. Use the arrow keys to walk its signs in reading order{zoom_hint}.\"\n         data-plate-cells=\"{cells}\" data-plate-layout=\"{geometry}\"{grid_attr}>\n      {svg}\n    </div>\n{zoom}  </div>\n  <figcaption>\n    <span class=\"plate-reading\">\n        <span class=\"plate-reading-rest\">{rest}</span>\n      <span class=\"plate-reading-live\" data-plate-reading aria-live=\"polite\"></span>\n    </span>\n    <span class=\"plate-hint awaits-script\" data-plate-hint>Point at a sign, or focus the plate and use the arrow keys, to walk it in reading order{hint_zoom}.</span>\n  </figcaption>\n</figure>\n",
        size_class = plate_size_class(size),
        zoom_class = if zoomable { " plate-frame--zoomable" } else { "" },
        zoom_hint = if zoomable {
            ", and plus, minus and zero to zoom"
        } else {
            ""
        },
        cells = h(cells),
        geometry = h(geometry),
        hint_zoom = if zoomable { "; +\u{a0}and\u{a0}− zoom in and out" } else { "" },
    )
}

/// _board.erb. Locals: svg, fragments, size, worn; uses the chapter's path.
pub fn board(
    svg: &str,
    fragments: &[rustergraphia::fragments::Fragment],
    size: usize,
    worn: bool,
    chapter_path: Option<&str>,
) -> String {
    let restore = if worn {
        format!(
            "        <form method=\"post\" action=\"{action}\" class=\"rejoin-restore\">\n          <input type=\"hidden\" name=\"svg\" value=\"{svg}\">\n          <input type=\"hidden\" name=\"rejoined\" value=\"1\">\n          <button type=\"submit\" class=\"btn btn--small btn--primary\" data-rejoin-restore disabled>Restore it</button>\n        </form>\n",
            action = h(chapter_path.unwrap_or("/decode")),
            svg = h(svg),
        )
    } else {
        String::new()
    };
    format!(
        "<figure class=\"plate plate--broken\">\n  <div class=\"plate-frame\" id=\"svg-display\" data-plate role=\"group\"\n       aria-label=\"The broken plate: an empty board, and below it the tray of fragments, some turned half round.\"\n       data-fragments=\"{fragments_attr}\">\n    {svg}\n  </div>\n  <figcaption>\n    <span class=\"plate-reading\"><span class=\"plate-reading-rest\">Lay the fragments on the board. Their edges are worn: only the sums show where each one goes.{worn_sentence}</span></span>\n    <span class=\"plate-hint awaits-script\" data-plate-hint>Drag a fragment onto the board, or pick it and then the cell for its top-left corner. Arrow keys move a picked fragment; T or a double click turns it. Turn and swap them until every pair balances.</span>\n  </figcaption>\n</figure>\n\n<section class=\"rejoin awaits-script\" aria-labelledby=\"rejoin-title\" data-rejoin>\n  <div class=\"rejoin-head\">\n    <h2 id=\"rejoin-title\" class=\"index-heading\">Rejoin it</h2>\n    <div class=\"rejoin-tools\">\n      <button type=\"button\" class=\"btn btn--small\" data-rejoin-turn disabled>Turn half round</button>\n      <button type=\"button\" class=\"btn btn--small\" data-rejoin-reset>Back to the tray</button>\n{restore}    </div>\n  </div>\n  <p class=\"rejoin-status\" data-rejoin-status aria-live=\"polite\">On the board: 0 of {count}.</p>\n  <p class=\"rejoin-reading\" data-rejoin-reading aria-live=\"polite\"></p>\n</section>\n",
        fragments_attr = h(&fragments_json(fragments, size)),
        worn_sentence = if worn {
            " Some signs are worn too: a join holds when every pair can balance, whatever its lacunae held. Whole again, the bench opens on it."
        } else {
            ""
        },
        count = fragments_count(fragments.len()),
    )
}
