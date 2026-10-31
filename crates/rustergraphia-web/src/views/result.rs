//! Cistergraphia: the plate (fol. 1v) — views/result.erb.

use rustergraphia::fragments::Fragment;
use rustergraphia::layout::Layout;
use rustergraphia::svg::Damage;
use rustergraphia::{tablet, Value};

use crate::esc::{h, pad4};
use crate::tray::{fragments_count, lacunae_count};
use crate::view::{layout_name, plate_cells_json, plate_grilles, plate_layout_json};
use crate::views::partials;

/// The plate page's ivars.
pub struct ResultState {
    pub secret: bool,
    pub message: String,
    pub decoy: Option<String>,
    pub pieces: Option<usize>,
    pub damage: Option<Damage>,
    pub tablet: Option<String>,
    pub pathless: bool,
    pub normalized: String,
    pub symbols: usize,
    pub layout: Layout,
    pub svg_content: String,
    pub fragments: Option<Vec<Fragment>>,
    pub sequence: Vec<Value>,
}

pub fn render_result(state: &ResultState) -> String {
    let size = state.symbols.isqrt();
    let lede = lede(state);
    let specs = specs(state, size);
    let turn = if matches!(state.layout, Layout::Rongorongo) {
        "      <button type=\"button\" class=\"btn awaits-script\" data-action=\"turn-plate\" aria-pressed=\"false\">Turn the plate</button>\n"
    } else {
        ""
    };
    let figure = match (&state.pieces, &state.fragments) {
        (Some(_), Some(fragments)) => format!(
            "    {}\n",
            partials::board(
                &state.svg_content,
                fragments,
                size,
                state.damage.is_some(),
                None
            )
        ),
        _ => {
            let empty = Damage::new();
            let damage = state.damage.as_ref().unwrap_or(&empty);
            format!(
                "    {}\n",
                partials::plate(
                    &state.svg_content,
                    &plate_cells_json(&state.sequence, &state.layout, damage),
                    &plate_layout_json(state.sequence.len(), &state.layout),
                    &state.layout,
                    state.decoy.is_some(),
                    size,
                    None,
                )
            )
        }
    };
    let bench = match (&state.damage, state.pieces) {
        (Some(damage), None) => {
            let entries = crate::bench::lacunae_entries(&state.sequence, &state.layout, damage);
            let values: Vec<Option<Value>> = state.sequence.iter().map(|&v| Some(v)).collect();
            format!(
                "    {}\n",
                partials::bench(&entries, &values, damage, None, false, &[])
            )
        }
        _ => String::new(),
    };
    let own_tablet = match &state.tablet {
        Some(seed) => {
            let rows: String = tablet::rows(seed)
                .expect("the plate's own tablet")
                .into_iter()
                .map(|(c, value)| {
                    format!(
                        "          <li><span class=\"own-tablet-value\">{}</span> <span class=\"own-tablet-char\">{}</span></li>\n",
                        pad4(value),
                        if c == ' ' { '␣' } else { c }
                    )
                })
                .collect();
            format!(
                "    <section class=\"own-tablet\" aria-labelledby=\"tablet-title\">\n      <h2 id=\"tablet-title\" class=\"index-heading\">Its tablet</h2>\n      <p class=\"field-meta\">What each sum reads as on this plate, for the scribe’s eyes. The reader has only the sums; the file carries the seed, so the decoder and your confirmations need nothing from you.</p>\n      <ol class=\"own-tablet-rows\" data-tablet-rows>\n{rows}      </ol>\n    </section>\n"
            )
        }
        None => String::new(),
    };
    let grille_block = grille_block(state);
    let readback_grille = match state.layout.as_grille() {
        Some(grille) => format!(
            "    <input type=\"hidden\" name=\"grille\" value=\"{}\">",
            h(&grille.to_string())
        ),
        None => "    ".to_string(),
    };
    let passphrase_field = if state.secret {
        "        <div class=\"field\">\n          <label for=\"readback-passphrase\">Passphrase</label>\n          <div class=\"passphrase\">\n            <input type=\"password\" id=\"readback-passphrase\" name=\"passphrase\" autocomplete=\"current-password\"\n                   autocapitalize=\"off\" spellcheck=\"false\" required>\n          </div>\n        </div>\n"
    } else {
        ""
    };
    let decoy_echo = match &state.decoy {
        Some(decoy) => format!(
            "    <p class=\"decoy-echo\"><span class=\"label\">For the novice</span> <q>{}</q></p>\n",
            h(decoy)
        ),
        None => String::new(),
    };
    format!(
        "<section class=\"leaf\" aria-labelledby=\"plate-title\">\n  <p class=\"folio-mark\" aria-hidden=\"true\">fol. 1<sup>v</sup></p>\n  <h1 id=\"plate-title\" class=\"rubric\">{title}</h1>\n\n  <p class=\"message-echo\"><span class=\"visually-hidden\">Message: </span><q>{message}</q></p>\n{decoy_echo}{lede}\n  <dl class=\"specs\">\n{specs}  </dl>\n\n  <div class=\"actions\">\n    <button type=\"button\" class=\"btn btn--primary\" data-action=\"download-svg\">Download SVG</button>\n    <button type=\"button\" class=\"btn btn--quiet\" data-action=\"download-png\">Download PNG</button>\n    <button type=\"button\" class=\"btn btn--quiet\" data-action=\"copy-svg\">Copy the file</button>\n{turn}  </div>\n\n{figure}\n{bench}\n{own_tablet}\n{grille_block}\n\n  <form class=\"readback\" method=\"post\" action=\"/decode\">\n    <h2 class=\"index-heading\">Ask the decoder</h2>\n    <input type=\"hidden\" name=\"svg\" value=\"{svg_value}\">\n    <input type=\"hidden\" name=\"decipher\" value=\"1\">\n{readback_grille}\n    <div class=\"readback-row\">\n{passphrase_field}      <button type=\"submit\" class=\"btn\">Decipher it</button>\n      <a class=\"btn btn--quiet\" href=\"/\">Inscribe another</a>\n    </div>\n  </form>\n</section>\n",
        title = if state.secret { "Your sealed plate" } else { "Your plate" },
        message = h(&state.message),
        svg_value = h(&state.svg_content),
    )
}

fn lede(state: &ResultState) -> String {
    let text = match (state.pieces, &state.damage) {
        (Some(pieces), Some(damage)) => format!(
            "Ruined: {}, {}. Download it and send it: Read opens it for your reader as the board, to rejoin by what each worn sign may hold, then as the bench to restore it. Below, a preview of their puzzle.",
            fragments_count(pieces),
            lacunae_count(damage.len())
        ),
        (Some(pieces), None) => format!(
            "Broken: {}. Download it and send it: Read opens it for your reader as the board, where only the sums say where each fragment goes. Below, a preview of their puzzle.",
            fragments_count(pieces)
        ),
        (None, Some(damage)) => format!(
            "Worn: {}. Download it and send it: Read opens it for your reader as the bench, where each pair must balance again and the words settle the doubts. Below, a preview of their puzzle.",
            lacunae_count(damage.len())
        ),
        (None, None) => {
            if state.tablet.is_some() && state.pathless {
                "The expert plate: pathless, with a tablet of its own. Download it and send it: your reader finds the way first, on sums alone, and then builds the tablet in the notebook, sum by sum; you confirm three entries at a time. The tablet is below, for your eyes.".to_string()
            } else if state.tablet.is_some() {
                "A tablet of its own: the values dealt out again for this plate alone. Download it and send it: your reader builds the tablet in the notebook, sum by sum, and you confirm three entries at a time. The tablet is below, for your eyes.".to_string()
            } else if state.secret {
                "Secret mode: sealed with your passphrase. Without it, this plate reveals nothing but its length.".to_string()
            } else if state.normalized != state.message {
                return format!(
                    "    <p class=\"lede\">Enciphered as: <code>{}</code></p>\n",
                    h(&state.normalized)
                );
            } else {
                "A game: anyone patient enough can read it back.".to_string()
            }
        }
    };
    format!("    <p class=\"lede\">{text}</p>\n")
}

fn specs(state: &ResultState, size: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "    <div><dt>Mode</dt><dd>{}</dd></div>\n",
        if state.secret { "Secret" } else { "Game" }
    ));
    out.push_str(&format!(
        "    <div><dt>Signs</dt><dd>{}</dd></div>\n",
        state.symbols
    ));
    out.push_str(&format!(
        "    <div><dt>Grid</dt><dd>{size} × {size}</dd></div>\n"
    ));
    out.push_str(&format!(
        "    <div><dt>Layout</dt><dd>{}</dd></div>\n",
        layout_name(&state.layout)
    ));
    if state.pathless {
        out.push_str("      <div><dt>Path</dt><dd>Withheld: the reader finds it</dd></div>\n");
    }
    if let Some(seed) = &state.tablet {
        out.push_str(&format!(
            "      <div><dt>Tablet</dt><dd>Its own <code class=\"grille-key\">{}</code></dd></div>\n",
            h(&tablet::show(seed).expect("the plate's own seed"))
        ));
    }
    match (state.pieces, &state.damage) {
        (Some(pieces), Some(_)) => out.push_str(&format!(
            "      <div><dt>Condition</dt><dd>Ruined: {}, worn</dd></div>\n",
            fragments_count(pieces)
        )),
        (Some(pieces), None) => out.push_str(&format!(
            "      <div><dt>Condition</dt><dd>Broken, in {}</dd></div>\n",
            fragments_count(pieces)
        )),
        (None, Some(_)) => {
            out.push_str("      <div><dt>Condition</dt><dd>Worn</dd></div>\n");
        }
        (None, None) => {}
    }
    if let Some(grille) = state.layout.as_grille() {
        if state.decoy.is_some() {
            out.push_str(&format!(
                "      <div><dt>Abbot’s key</dt><dd><code class=\"grille-key\">{}</code></dd></div>\n      <div><dt>Novice’s key</dt><dd><code class=\"grille-key\">{}</code></dd></div>\n",
                h(&grille.to_string()),
                h(&grille.novice().to_string())
            ));
        } else {
            out.push_str(&format!(
                "      <div><dt>Grille key</dt><dd><code class=\"grille-key\">{}</code></dd></div>\n",
                h(&grille.to_string())
            ));
        }
    }
    out
}

fn grille_block(state: &ResultState) -> String {
    let Some(grille) = state.layout.as_grille() else {
        return String::new();
    };
    let grilles = plate_grilles(grille, state.symbols);
    let mut out = String::from("    <div class=\"actions grille-actions\">\n");
    if grilles.len() > 1 {
        out.push_str("        <div class=\"segments grille-choice awaits-script\" role=\"group\" aria-label=\"Which grille to lay\">\n");
        for (i, entry) in grilles.iter().enumerate() {
            out.push_str(&format!(
                "            <button type=\"button\" class=\"segment\" data-grille-choice=\"{part}\" aria-pressed=\"{pressed}\"><span class=\"segment-title\">{title}</span></button>\n",
                part = entry.part,
                pressed = i == 0,
                title = if entry.part == "abbot" { "Abbot" } else { "Novice" },
            ));
        }
        out.push_str("        </div>\n");
    }
    out.push_str("      <button type=\"button\" class=\"btn btn--primary awaits-script\" data-action=\"lay-grille\" aria-pressed=\"false\">Lay the grille</button>\n");
    out.push_str(&format!(
        "      <button type=\"button\" class=\"btn awaits-script\" data-action=\"download-stencil\">{}</button>\n",
        if grilles.len() > 1 {
            "Download the stencils"
        } else {
            "Download the stencil"
        }
    ));
    for entry in &grilles {
        let label = if grilles.len() > 1 {
            format!("Copy {}", entry.label.replace("grille", "key"))
        } else {
            "Copy the key".to_string()
        };
        out.push_str(&format!(
            "        <button type=\"button\" class=\"btn awaits-script\" data-action=\"copy-key\" data-key=\"{}\">{label}</button>\n",
            h(&entry.key)
        ));
    }
    out.push_str("    </div>\n");
    for entry in &grilles {
        out.push_str(&format!(
            "      <template data-stencil=\"{part}\" data-label=\"{label}\" data-path=\"{path}\">{stencil}</template>\n",
            part = entry.part,
            label = h(entry.label),
            path = h(&entry.path),
            stencil = entry.stencil,
        ));
    }
    out
}
