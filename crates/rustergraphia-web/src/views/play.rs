//! Cistergraphia: the scriptorium (fol. 2v) — views/play.erb, the reader's
//! page: desk, bench, board or notebook, built from the file alone.

use rustergraphia::fragments::Fragment;
use rustergraphia::layout::Layout;
use rustergraphia::notebook;
use rustergraphia::pathless::CellReading;
use rustergraphia::svg::Damage;
use rustergraphia::tally::Signs;
use rustergraphia::Value;

use crate::bench::{pathless_lacunae, Entry, WornCell};
use crate::chapter::{self, Day};
use crate::esc::h;
use crate::tray::fragments_count;
use crate::view::{
    layout_name, pathless_cells_json, plate_cells_json_opt, plate_grid_json, plate_layout_json,
};
use crate::views::partials;

/// The reader's page's ivars, as explicit state.
pub struct PlayState {
    pub broken: bool,
    pub rejoined: bool,
    pub weathered: bool,
    pub expert: bool,
    pub grid: Option<Vec<CellReading>>,
    pub notebook: Option<Vec<notebook::Entry>>,
    pub desk: bool,
    pub chapter: Option<Day>,
    pub entries: Vec<Entry>,
    pub values: Vec<Option<Value>>,
    pub worn: Vec<WornCell>,
    pub fragments: Option<Vec<Fragment>>,
    pub size: usize,
    pub tally: Option<Signs>,
    pub layout: Layout,
    pub svg_content: String,
    pub plate_svg: Option<String>,
    pub damage: Damage,
}

impl PlayState {
    fn rung(&self) -> &'static str {
        if self.broken || self.rejoined {
            if self.weathered {
                "ruined"
            } else {
                "broken"
            }
        } else if self.expert {
            "expert"
        } else if self.grid.is_some() {
            "pathless"
        } else if self.notebook.is_some() {
            "tablet"
        } else if self.desk {
            "intact"
        } else {
            "worn"
        }
    }

    fn stroke_positions(count: usize) -> &'static str {
        if count == 1 {
            "stroke position"
        } else {
            "stroke positions"
        }
    }

    fn lede(&self) -> String {
        if self.rejoined {
            return format!(
                "Whole again: the fragments are rejoined, and the plate has lost {} {}, hatched where the surface is worn. Now restore it: press the lost digits back in, each pair of signs must sum to a value on the tablet, and where two or three letters balance, the words decide.",
                self.entries.len(),
                Self::stroke_positions(self.entries.len())
            );
        }
        if self.grid.is_some() && !self.desk {
            let lacunae = pathless_lacunae(&self.worn);
            return format!(
                "The plate is worn and pathless: {} pairs of signs, {lacunae} {} lost where the surface is hatched, and nothing says which way it runs. Choose one of the five ways at the bench: along it the pairs form, and read as letters or do not. Along the way, press the lost digits back in: each pair must sum to a value on the tablet, and where two or three letters balance, the words decide.",
                self.values.len() / 2,
                Self::stroke_positions(lacunae)
            );
        }
        if self.expert {
            return format!(
                "The plate is whole, pathless, and has a tablet of its own: {} pairs of signs, and nothing says which way they run, nor what their sums read. First the way: choose one of the five at the desk and read along it. Every pair sums to a value of the tablet, whatever the tablet, and along a wrong way one soon does not. Three pairs that land open the notebook on every sum the way makes. Then the tablet: write letters in pencil, and the scribe confirms them three at a time.",
                self.values.len() / 2
            );
        }
        if self.grid.is_some() {
            return format!(
                "The plate is whole and pathless: {} pairs of signs, but nothing says which way it runs. Choose one of the five ways at the desk and read along it: the pairs form, and each sums to a letter on the tablet, or does not. Along the wrong way the letters stop; the notebook keeps what each way gave.",
                self.values.len() / 2
            );
        }
        if let Some(entries) = &self.notebook {
            return format!(
                "The plate is whole and has a tablet of its own: {} sums, each one letter, but not the letter the shared tablet gives it. Build its tablet in the notebook. Every pair that adds to one sum is the same letter, however it is spelled; the sum seen most is usually the space, or <i>e</i>. Write a letter in pencil against a sum and the text below takes it at once. Ask the scribe: entries are confirmed three at a time, never one.",
                entries.len()
            );
        }
        if self.desk {
            return format!(
                "The plate is whole: {} pairs of signs, each summing to a letter on the tablet. Read it pair by pair: type what each sign reads and the page adds them. Guided, a tap on a sign shows its value; unaided, a wrong reading is marked, never corrected, and three in a row earn the Lectio’s ranks.",
                self.values.len() / 2
            );
        }
        if self.broken && self.weathered {
            return format!(
                "The plate came in {}, worn where the surface is hatched. Lay them on the board, turned where need be: a join holds when every pair of signs can balance, whatever its lacunae held. Whole again, it reads as an edition, and the bench opens on it to restore what is lost.",
                fragments_count(self.fragments.as_ref().map_or(0, Vec::len))
            );
        }
        if self.broken {
            return format!(
                "The plate came in {}. Lay them on the board, turned where need be: only one arrangement makes every pair of signs balance, and the shapes alone will not tell you which.",
                fragments_count(self.fragments.as_ref().map_or(0, Vec::len))
            );
        }
        format!(
            "The plate has lost {} {}, hatched where the surface is worn. Press the lost digits back in: each pair of signs must sum to a value on the tablet. Where two or three letters balance, the words decide.",
            self.entries.len(),
            Self::stroke_positions(self.entries.len())
        )
    }

    fn title(&self) -> &'static str {
        if self.broken {
            if self.weathered {
                "A plate to rejoin and restore"
            } else {
                "A plate to rejoin"
            }
        } else if self.desk {
            "A plate to read"
        } else {
            "A plate to restore"
        }
    }

    fn condition_rows(&self) -> String {
        let mut out = String::new();
        if self.broken {
            out.push_str(&format!(
                "      <div><dt>Condition</dt><dd>{}</dd></div>\n",
                if self.weathered {
                    format!(
                        "Ruined: {}, worn",
                        fragments_count(self.fragments.as_ref().map_or(0, Vec::len))
                    )
                } else {
                    format!(
                        "Broken, in {}",
                        fragments_count(self.fragments.as_ref().map_or(0, Vec::len))
                    )
                }
            ));
        } else if self.rejoined {
            out.push_str("      <div><dt>Condition</dt><dd>Ruined, rejoined: worn</dd></div>\n");
        } else if self.grid.is_some() && !self.desk {
            out.push_str(&format!(
                "      <div><dt>Condition</dt><dd>{}</dd></div>\n",
                if self.tally.is_some() {
                    "Worn, pathless, with tally signs"
                } else {
                    "Worn, pathless"
                }
            ));
        } else if self.grid.is_some() {
            out.push_str("      <div><dt>Condition</dt><dd>Intact, pathless</dd></div>\n");
            // the expert row shares its line with the if-tags: 6 spaces stay
            out.push_str("      ");
            if self.expert {
                out.push_str("<div><dt>Tablet</dt><dd>Its own: to build</dd></div>");
            }
            out.push('\n');
        } else if self.notebook.is_some() {
            out.push_str("      <div><dt>Condition</dt><dd>Intact</dd></div>\n");
            out.push_str("      <div><dt>Tablet</dt><dd>Its own: to build</dd></div>\n");
        } else if self.desk {
            out.push_str("      <div><dt>Condition</dt><dd>Intact</dd></div>\n");
        } else {
            out.push_str(&format!(
                "      <div><dt>Condition</dt><dd>{}</dd></div>\n",
                if self.tally.is_some() {
                    "Worn, with tally signs"
                } else {
                    "Worn"
                }
            ));
        }
        out
    }

    fn body(&self) -> String {
        if self.broken {
            return format!(
                "    {}\n",
                partials::board(
                    &self.svg_content,
                    self.fragments.as_deref().unwrap_or(&[]),
                    self.size,
                    self.weathered,
                    self.chapter.as_ref().map(|day| day.path()).as_deref(),
                )
            );
        }
        let plate = if let Some(grid) = &self.grid {
            partials::plate(
                &self.svg_content,
                &pathless_cells_json(grid),
                &plate_layout_json(self.values.len(), &self.layout),
                &self.layout,
                false,
                self.size,
                Some(&plate_grid_json(grid)),
            )
        } else if self.desk {
            partials::plate(
                &self.svg_content,
                &plate_cells_json_opt(&self.values, &self.layout, &Damage::new()),
                &plate_layout_json(self.values.len(), &self.layout),
                &self.layout,
                false,
                self.size,
                None,
            )
        } else {
            partials::plate(
                self.plate_svg.as_deref().unwrap_or(&self.svg_content),
                &plate_cells_json_opt(&self.values, &self.layout, &self.damage),
                &plate_layout_json(self.values.len(), &self.layout),
                &self.layout,
                false,
                self.size,
                None,
            )
        };
        let work = if self.grid.is_some() && !self.desk {
            format!(
                "          {}\n",
                partials::bench(
                    &self.entries,
                    &self.values,
                    &self.damage,
                    self.tally.as_ref(),
                    true,
                    &self.worn,
                )
            )
        } else if self.grid.is_some() {
            let characters = characters_json();
            let desk = partials::desk(
                &self.values,
                if self.expert { "{}" } else { &characters },
                true,
                self.expert,
            );
            let notebook = if self.expert {
                partials::notebook_partial(&[], &self.values, true)
            } else {
                String::new()
            };
            format!("          {desk}\n          {notebook}\n")
        } else if let Some(entries) = &self.notebook {
            format!(
                "          {}\n          {}\n",
                partials::desk(&self.values, "{}", false, true),
                partials::notebook_partial(entries, &self.values, false)
            )
        } else if self.desk {
            format!(
                "          {}\n",
                partials::desk(&self.values, &characters_json(), false, false)
            )
        } else {
            format!(
                "          {}\n",
                partials::bench(
                    &self.entries,
                    &self.values,
                    &self.damage,
                    self.tally.as_ref(),
                    false,
                    &[],
                )
            )
        };
        let plate_line = format!("          {plate}\n");
        format!(
            "    <div class=\"room\">\n      <div class=\"room-plate\">\n{plate_line}      </div>\n      <div class=\"room-work\">\n{work}      </div>\n    </div>\n"
        )
    }
}

fn characters_json() -> String {
    let pairs: Vec<String> = rustergraphia::character_sums()
        .map(|sum| {
            let c = rustergraphia::character_of(sum).expect("a tablet sum");
            format!("\"{sum}\":{}", crate::esc::json_str(&c.to_string()))
        })
        .collect();
    format!("{{{}}}", pairs.join(","))
}

pub fn render_play(state: &PlayState) -> String {
    let rung = state.rung();
    let chapter_attr = match &state.chapter {
        Some(day) => format!(" data-chapter=\"{}\"", day.date.format("%Y-%m-%d")),
        None => String::new(),
    };
    let chapter_mark = match &state.chapter {
        Some(day) => {
            let feast = match &day.feast {
                Some(feast) => format!(" · <span class=\"chapter-feast\">{}</span>", h(feast)),
                None => String::new(),
            };
            format!(
                "    <p class=\"chapter-mark\"><span class=\"chapter-number\">Chapter {}</span> · {}{feast}</p>\n",
                day.number,
                h(&chapter::chapter_day(day))
            )
        }
        None => String::new(),
    };
    let chapter_week = match &state.chapter {
        Some(day) => format!(
            "    <p class=\"chapter-week\">{}</p>\n",
            h(&chapter::chapter_week(day))
        ),
        None => String::new(),
    };
    let layout_row = if state.broken {
        "Phaistos".to_string()
    } else if state.grid.is_some() {
        "Withheld: one of the five ways".to_string()
    } else {
        layout_name(&state.layout).to_string()
    };
    let grille_row = match state.layout.as_grille() {
        Some(grille) => format!(
            "      <div><dt>Grille key</dt><dd><code class=\"grille-key\">{}</code></dd></div>\n",
            h(&grille.to_string())
        ),
        None => String::new(),
    };
    let chapter_source = match &state.chapter {
        Some(day) => format!(
            "    <p class=\"chapter-source\" data-chapter-source hidden>From {}.</p>\n",
            h(&day.source)
        ),
        None => String::new(),
    };
    let readback_grille = match state.layout.as_grille() {
        Some(grille) => format!(
            "    <input type=\"hidden\" name=\"grille\" value=\"{}\">",
            h(&grille.to_string())
        ),
        None => "    ".to_string(),
    };
    let turn = if matches!(state.layout, Layout::Rongorongo) && state.layout.as_grille().is_none() {
        "        <button type=\"button\" class=\"btn awaits-script\" data-action=\"turn-plate\" aria-pressed=\"false\">Turn the plate</button>\n"
    } else {
        ""
    };
    let chapter_nav = match &state.chapter {
        Some(day) => {
            let before = chapter::of(day.date - chrono::Duration::days(1));
            let after = chapter::of(day.date + chrono::Duration::days(1));
            if before.is_none() && after.is_none() {
                String::new()
            } else {
                let mut nav =
                    String::from("      <nav class=\"chapter-nav\" aria-label=\"Chapters\">\n");
                if let Some(before) = &before {
                    nav.push_str(&format!(
                        "        <a href=\"{}\">Chapter {}, the day before</a>\n",
                        h(&before.path()),
                        before.number
                    ));
                } else {
                    nav.push_str("        \n");
                }
                if let Some(after) = &after {
                    nav.push_str(&format!(
                        "        <a class=\"chapter-nav-after\" href=\"{}\">Chapter {}, the day after</a>\n",
                        h(&after.path()),
                        after.number
                    ));
                } else {
                    nav.push_str("        \n");
                }
                nav.push_str("      </nav>\n");
                nav
            }
        }
        None => String::new(),
    };
    format!(
        "<section class=\"leaf\" aria-labelledby=\"play-title\" data-play=\"{rung}\"{chapter_attr}>\n  <p class=\"folio-mark\" aria-hidden=\"true\">fol. 2<sup>v</sup></p>\n{chapter_mark}  <h1 id=\"play-title\" class=\"rubric\">{title}</h1>\n    <p class=\"lede\">{lede}</p>\n\n{chapter_week}\n  <dl class=\"specs\">\n    <div><dt>Signs</dt><dd>{signs}</dd></div>\n    <div><dt>Grid</dt><dd>{size} × {size}</dd></div>\n    <div><dt>Layout</dt><dd>{layout_row}</dd></div>\n{condition_rows}{grille_row}  </dl>\n\n{body}\n  <p class=\"record\" data-record=\"play\" aria-live=\"polite\"></p>\n{chapter_source}\n  <form class=\"readback\" method=\"post\" action=\"/decode\">\n    <h2 class=\"index-heading\">Ask the decoder</h2>\n    <input type=\"hidden\" name=\"svg\" value=\"{svg_value}\">\n    <input type=\"hidden\" name=\"decipher\" value=\"1\">\n{readback_grille}\n    <div class=\"readback-row\">\n      <button type=\"submit\" class=\"btn\">Decipher it</button>\n{turn}      <a class=\"btn btn--quiet\" href=\"/decode\">Read another</a>\n    </div>\n  </form>\n\n{chapter_nav}</section>\n",
        body = state.body(),
        title = state.title(),
        lede = state.lede(),
        signs = state.size * state.size,
        size = state.size,
        condition_rows = state.condition_rows(),
        svg_value = h(&state.svg_content),
    )
}
