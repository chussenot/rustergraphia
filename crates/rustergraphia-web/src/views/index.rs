//! Cistergraphia: Inscribe (fol. 1r) — views/index.erb.

use crate::esc::h;
use crate::figures::{grille_order_svg, snail_svg};
use crate::view::{glyph_svg, GLYPH_TABLE_JSON};

/// The Inscribe form's state: the Ruby page's ivars.
#[derive(Debug, Clone, Default)]
pub struct IndexState {
    pub secret: bool,
    /// The keyless layout chosen, when it is not phaistos.
    pub layout_keyless: Option<String>,
    /// The grille is the layout on the form: chosen, or sent back with an
    /// error (`grille_chosen?`).
    pub grille_chosen: bool,
    pub message: String,
    pub grille_key: String,
    pub grille_error: Option<String>,
    pub decoy: Option<String>,
    pub decoy_error: Option<String>,
    pub passphrase_error: Option<String>,
    pub condition: Option<String>,
    pub condition_error: Option<String>,
    pub own_tablet: bool,
    pub tablet_error: Option<String>,
    pub pathless: bool,
    pub path_error: Option<String>,
}

impl IndexState {
    fn condition(&self) -> &str {
        self.condition.as_deref().unwrap_or("intact")
    }

    /// The reader's puzzle as its summary says it.
    fn puzzle_state(&self) -> String {
        let rung = match self.condition() {
            "worn" => "Worn",
            "broken" => "Broken",
            "ruined" => "Ruined",
            _ => "Intact",
        };
        format!(
            "{rung}, {}, {}",
            if self.own_tablet {
                "its own tablet"
            } else {
                "shared tablet"
            },
            if self.pathless {
                "path withheld"
            } else {
                "path shown"
            }
        )
    }

    /// The puzzle's disclosure is open when any of it was chosen or refused.
    fn puzzle_open(&self) -> bool {
        self.pathless
            || self.own_tablet
            || self.condition() != "intact"
            || self.condition_error.is_some()
            || self.path_error.is_some()
            || self.tablet_error.is_some()
    }

    /// `form_errors`' order: passphrase, grille, decoy, condition, path, tablet.
    fn first_error_is_condition(&self) -> bool {
        self.passphrase_error.is_none()
            && self.grille_error.is_none()
            && self.decoy_error.is_none()
            && self.condition_error.is_some()
    }

    /// The rung sent back with an error: invalid, and focused when its error
    /// is the form's first (raw output).
    fn condition_invalid(&self, rung: &str) -> &'static str {
        if self.condition_error.is_none() || rung != self.condition() {
            return "";
        }
        if self.first_error_is_condition() {
            " aria-invalid=\"true\" autofocus"
        } else {
            " aria-invalid=\"true\""
        }
    }
}

fn error_text(error: &Option<String>) -> String {
    error.as_deref().map(h).unwrap_or_default()
}

/// Raw ` aria-invalid="true" autofocus` when the field was refused.
fn invalid_focus(error: &Option<String>) -> &'static str {
    if error.is_some() {
        " aria-invalid=\"true\" autofocus"
    } else {
        ""
    }
}

/// The tablet/path radios' escaped quirk: Ruby escapes the attribute, so the
/// page literally shows ` aria-invalid=&quot;true&quot;` (kept for byte
/// parity with the original; fixing it belongs in both codebases at once).
fn invalid_escaped(error: &Option<String>) -> &'static str {
    if error.is_some() {
        " aria-invalid=&quot;true&quot;"
    } else {
        ""
    }
}

fn checked(on: bool) -> &'static str {
    if on {
        " checked"
    } else {
        ""
    }
}

const RUNGS: [(&str, &str, bool); 4] = [
    ("intact", "Intact", false),
    ("worn", "Worn", false),
    ("broken", "Broken", true),
    ("ruined", "Ruined", true),
];

pub fn render_index(state: &IndexState) -> String {
    let keyless = state.layout_keyless.as_deref();
    let phaistos_checked = checked(keyless.is_none() && !state.grille_chosen);
    let rungs: String = RUNGS
        .iter()
        .map(|(rung, title, snail)| {
            format!(
                "                  <label class=\"segment segment--rung segment--{rung}{snail_class}\">\n                    <input type=\"radio\" name=\"condition\" value=\"{rung}\" data-rung{checked}\n                           aria-describedby=\"note-{rung} note-unweathered note-snail condition-error\"{invalid}>\n                    <span class=\"segment-title\">{title}</span>\n                  </label>\n",
                snail_class = if *snail { " segment--snail" } else { "" },
                checked = checked(state.condition() == *rung),
                invalid = state.condition_invalid(rung),
            )
        })
        .collect();
    let tabula: String = (1..=9)
        .map(|digit| {
            format!(
                "        <li>{}<span aria-hidden=\"true\">{digit}</span></li>\n",
                glyph_svg(digit, "glyph", None)
            )
        })
        .collect();

    format!(
        r#"<div class="spread">
  <section class="leaf" aria-labelledby="inscribe-title">
    <p class="folio-mark" aria-hidden="true">fol. 1<sup>r</sup></p>
    <h1 id="inscribe-title" class="rubric">Inscribe a message</h1>
    <p class="lede">Each letter becomes a number, each number a monk’s sign, and the signs coil inward like a snail, turn like a tablet, or hide under a grille.</p>

    <form class="entry" method="post" action="/generate" data-entry>
      <div class="choices">
        <fieldset class="choice">
          <legend>Mode</legend>
          <div class="segments">
            <label class="segment segment--game">
              <input type="radio" name="mode" value="game"{game_checked} aria-describedby="note-game">
              <span class="segment-title">Game</span>
            </label>
            <label class="segment segment--secret">
              <input type="radio" name="mode" value="secret"{secret_checked} aria-describedby="note-secret">
              <span class="segment-title">Secret</span>
            </label>
          </div>
          <div class="choice-notes">
            <p class="choice-note" id="note-game" data-note="game">Open to any patient reader, and to machines: a puzzle in the old manner.</p>
            <p class="choice-note" id="note-secret" data-note="secret">Sealed with a passphrase (AES-256-GCM): without it, the plate shows only its length.</p>
          </div>
        </fieldset>


        <fieldset class="choice choice--ways">
          <legend>Layout</legend>
          <div class="segments segments--ways">
            <label class="segment segment--way">
              <input type="radio" name="layout" value="phaistos"{phaistos_checked} aria-describedby="note-phaistos">
              <span class="segment-figure" aria-hidden="true">{snail_phaistos}</span>
              <span class="segment-title">Phaistos</span>
            </label>
            <label class="segment segment--way">
              <input type="radio" name="layout" value="rongorongo"{rongorongo_checked} aria-describedby="note-rongorongo">
              <span class="segment-figure" aria-hidden="true">{snail_rongorongo}</span>
              <span class="segment-title">Rongorongo</span>
            </label>
            <label class="segment segment--way">
              <input type="radio" name="layout" value="leap"{leap_checked} aria-describedby="note-leap">
              <span class="segment-figure" aria-hidden="true">{snail_leap}</span>
              <span class="segment-title">Leap</span>
            </label>
            <label class="segment segment--way">
              <input type="radio" name="layout" value="furrows"{furrows_checked} aria-describedby="note-furrows">
              <span class="segment-figure" aria-hidden="true">{snail_furrows}</span>
              <span class="segment-title">Furrows</span>
            </label>
            <label class="segment segment--way">
              <input type="radio" name="layout" value="unwinding"{unwinding_checked} aria-describedby="note-unwinding">
              <span class="segment-figure" aria-hidden="true">{snail_unwinding}</span>
              <span class="segment-title">Unwinding</span>
            </label>
            <label class="segment segment--way">
              <input type="radio" name="layout" value="grille"{grille_checked} aria-describedby="note-grille">
              <span class="segment-figure" aria-hidden="true">{grille_order}</span>
              <span class="segment-title">Grille</span>
            </label>
          </div>
          <div class="choice-notes">
            <p class="choice-note" id="note-phaistos" data-note="phaistos">The snail: clockwise from the top-left corner, coiling to the centre.</p>
            <p class="choice-note" id="note-rongorongo" data-note="rongorongo">From the bottom up, turning the plate half round at every line.</p>
            <p class="choice-note" id="note-leap" data-note="leap">The Zodiac’s step: one down, two across, wrapping at the edge; a new start a column to the right when the orbit closes.</p>
            <p class="choice-note" id="note-furrows" data-note="furrows">Down the first column and up the next, from the left, as the ox turns at the end of the field.</p>
            <p class="choice-note" id="note-unwinding" data-note="unwinding">From the centre outward, counter-clockwise, to the top-right corner.</p>
            <p class="choice-note" id="note-grille" data-note="grille">Row by row, as on a page, through a pierced parchment turned four times.</p>
          </div>
        </fieldset>


      </div>

      <div class="field">
        <div class="field-head">
          <label for="message">Message</label>
          <span class="counter" data-counter><span data-count>0</span> / 500</span>
        </div>
        <textarea id="message" name="message" maxlength="500" rows="4" required
                  placeholder="Meet me at the old abbey after vespers" data-counted>{message}</textarea>
        <div class="preview is-empty" data-preview data-glyph-table="{glyph_table}">
          <p class="label">Preview</p>
          <div class="preview-stage">
            <ol class="preview-signs" data-preview-signs aria-hidden="true"></ol>
            <p class="preview-note preview-note--empty">Each letter becomes one sign. Anything else becomes a space.</p>
            <p class="preview-note preview-note--secret">No preview when sealed: the signs come from the encryption. Any text works.</p>
            <p class="preview-note preview-note--own">No preview with a tablet of its own: the values are dealt when the plate is inscribed.</p>
          </div>
        </div>
      </div>

      <div class="entry-end">
        <div class="slot grille-slot">
          <div class="slot-inner">
            <div class="field">
              <label for="grille">Grille key <span class="label-note">· optional: a grille you already share</span></label>
              <input type="text" id="grille" name="grille" value="{grille_key}" class="key-input"
                     placeholder="{example}" autocomplete="off" autocapitalize="characters" spellcheck="false"
                     aria-describedby="grille-error grille-hint"{grille_invalid}>
              <div class="field-notes">
                <p class="field-meta" id="grille-hint">Leave it empty and a new grille is drawn for this plate. Each plate written under the same key makes that key easier to break.</p>
                <p class="field-error" id="grille-error" aria-live="polite">{grille_error}</p>
              </div>
            </div>
            <div class="field">
              <label for="decoy">Novice’s message <span class="label-note">· optional: a second reader</span></label>
              <textarea id="decoy" name="decoy" rows="2" maxlength="250"
                        placeholder="Vespers at six in the choir" aria-describedby="decoy-error decoy-hint"{decoy_invalid}>{decoy}</textarea>
              <div class="field-notes">
                <p class="field-meta" id="decoy-hint">A novice’s grille reads it, drawn from your key: yours reads both messages, the novice’s only this one. Up to 250 characters each.</p>
                <p class="field-error" id="decoy-error" aria-live="polite">{decoy_error}</p>
              </div>
            </div>
          </div>
        </div>
        <div class="slot secret-slot">
          <div class="slot-inner">
            <div class="field">
              <label for="passphrase">Passphrase <span class="label-note">· secret mode only</span></label>
              <div class="passphrase">
                <input type="password" id="passphrase" name="passphrase" minlength="8"
                       autocomplete="new-password" autocapitalize="off" spellcheck="false"
                       aria-describedby="passphrase-error passphrase-hint"{passphrase_invalid}>
                <button type="button" class="btn btn--small" data-reveal="passphrase" aria-pressed="false">Show</button>
              </div>
              <div class="field-notes">
                <p class="field-meta" id="passphrase-hint">At least 8 characters; four or more random words is far better. Never stored.</p>
                <p class="field-error" id="passphrase-error" aria-live="polite" data-passphrase-error
                   data-missing="Secret mode needs a passphrase"
                   data-too-short="A passphrase needs at least 8 characters">{passphrase_error}</p>
              </div>
            </div>
          </div>
        </div>
        <details class="puzzle" data-puzzle{puzzle_open}>
          <summary class="puzzle-summary">
            <span class="puzzle-title">The reader’s puzzle</span>
            <span class="puzzle-state" data-puzzle-state>{puzzle_state}</span>
            <span class="puzzle-sealed">None: a sealed plate is not a puzzle</span>
          </summary>
          <div class="choices puzzle-choices">
            <fieldset class="choice choice--condition">
              <legend>Condition</legend>
              <div class="segments segments--ladder">
{rungs}              </div>
              <div class="choice-notes">
                <p class="choice-note" id="note-intact" data-note="intact">As copied: read the signs, add the pairs, look them up on the tablet.</p>
                <p class="choice-note" id="note-worn" data-note="worn">Some strokes are lost. Press the digits back in: each pair must balance, and the words settle the doubts.</p>
                <p class="choice-note" id="note-broken" data-note="broken">The plate in two to four pieces, some turned. Only the sums say where each one goes.</p>
                <p class="choice-note" id="note-ruined" data-note="ruined">Broken and worn. Rejoin it by what each worn sign may hold, then restore the whole.</p>
                <p class="choice-note" id="note-snail" data-note="snail">Broken plates follow the snail: the board rejoins the fragments along it. Choose Phaistos, or a plate in one piece.</p>
                <p class="choice-note" id="note-unweathered" data-note="unweathered">A sealed plate cannot weather or break: with one stroke lost, it would never open again.</p>
                <p class="choice-note field-error" id="condition-error" aria-live="polite">{condition_error}</p>
              </div>
            </fieldset>
            <fieldset class="choice choice--tablet">
              <legend>Tablet</legend>
              <div class="segments">
                <label class="segment segment--tablet-shared">
                  <input type="radio" name="tablet" value="shared"{shared_checked} aria-describedby="note-tablet-shared">
                  <span class="segment-title">Shared</span>
                </label>
                <label class="segment segment--tablet-own">
                  <input type="radio" name="tablet" value="own"{own_checked}
                         aria-describedby="note-tablet-own note-tablet-short note-tablet-sealed note-tablet-grille note-tablet-damaged note-tablet-expert tablet-error"{tablet_invalid}>
                  <span class="segment-title">Its own</span>
                </label>
              </div>
              <div class="choice-notes">
                <p class="choice-note" id="note-tablet-shared" data-note="shared">The tablet everyone has: add a pair, look its sum up.</p>
                <p class="choice-note" id="note-tablet-own" data-note="own">A tablet of this plate’s own. The reader builds it in the notebook, sum by sum; the scribe confirms three entries at a time.</p>
                <p class="choice-note" id="note-tablet-short" data-note="tablet-short">A tablet of its own, on a message this short, is close to guessing: under 6 words there is little for a reader to count. A longer message reads better.</p>
                <p class="choice-note" id="note-tablet-sealed" data-note="tablet-sealed">A sealed plate has no tablet to find: its passphrase is the whole key. Choose Game.</p>
                <p class="choice-note" id="note-tablet-grille" data-note="tablet-grille">A grille plate has a key already: one key at a time. Choose another layout.</p>
                <p class="choice-note" id="note-tablet-damaged" data-note="tablet-damaged">A worn or broken plate keeps the shared tablet: the bench and the board speak in letters. Choose Intact.</p>
                <p class="choice-note" id="note-tablet-expert" data-note="tablet-expert">The expert plate: pathless, with a tablet of its own. Two things to find, in order: the way, on sums alone, then the tablet in the notebook.</p>
                <p class="choice-note field-error" id="tablet-error" aria-live="polite">{tablet_error}</p>
              </div>
            </fieldset>
            <fieldset class="choice choice--path">
              <legend>Path</legend>
              <div class="segments">
                <label class="segment segment--path-shown">
                  <input type="radio" name="path" value="shown"{shown_checked} aria-describedby="note-shown">
                  <span class="segment-title">Shown</span>
                </label>
                <label class="segment segment--path-withheld">
                  <input type="radio" name="path" value="withheld"{withheld_checked}
                         aria-describedby="note-withheld note-path-sealed note-path-grille note-path-broken path-error"{path_invalid}>
                  <span class="segment-title">Withheld</span>
                </label>
              </div>
              <div class="choice-notes">
                <p class="choice-note" id="note-shown" data-note="shown">The reader’s desk opens on the way the plate runs.</p>
                <p class="choice-note" id="note-withheld" data-note="withheld">A pathless plate: nothing says where it begins. The reader finds the way among the five, intact or worn.</p>
                <p class="choice-note" id="note-path-sealed" data-note="path-sealed">A sealed plate reads as no letters along any way: nothing to find. Choose Game.</p>
                <p class="choice-note" id="note-path-grille" data-note="path-grille">A grille plate hides its way by its key already. Choose another layout.</p>
                <p class="choice-note" id="note-path-broken" data-note="path-broken">A broken plate follows the snail: the board rejoins it along it. Choose Intact or Worn.</p>
                <p class="choice-note field-error" id="path-error" aria-live="polite">{path_error}</p>
              </div>
            </fieldset>
          </div>
        </details>
        <button class="btn btn--primary" type="submit" data-submit>Inscribe</button>
      </div>
    </form>
  </section>

  <aside class="leaf" aria-labelledby="key-title">
    <h2 id="key-title" class="rubric rubric--small">How to read a sign</h2>

    <figure class="anatomy" data-anatomy data-number="1098">
      <button type="button" class="quadrant quadrant--tens" data-place="10" disabled><b data-digit>9</b>tens</button>
      {anatomy_glyph}
      <button type="button" class="quadrant quadrant--units" data-place="1" disabled><b data-digit>8</b>units</button>
      <button type="button" class="quadrant quadrant--thousands" data-place="1000" disabled><b data-digit>1</b>thousands</button>
      <button type="button" class="quadrant quadrant--hundreds" data-place="100" disabled><b data-digit>0</b>hundreds</button>
      <figcaption>One stem, four corners: <span data-anatomy-caption>1098, the founding of Cîteaux</span>.</figcaption>
      <p class="anatomy-hint awaits-script" data-anatomy-hint>Touch a corner to turn its digit.</p>
    </figure>

    <h3 class="index-heading">Tabula of units</h3>
    <ol class="tabula">
{tabula}    </ol>
    <p class="note">Tens mirror the units to the left, hundreds below, thousands both ways.</p>

    <h3 class="index-heading">Order of reading</h3>
    <div class="orders">
      <figure class="order" data-layout-note="phaistos">
        {snail_phaistos}
        <figcaption><b>Phaistos.</b> Clockwise from the top-left corner, coiling to the centre.</figcaption>
      </figure>
      <figure class="order" data-layout-note="rongorongo">
        {snail_rongorongo}
        <figcaption><b>Rongorongo.</b> From the bottom left, then turn the plate half round for each next line.
          Hollow points are signs written upside down.</figcaption>
      </figure>
      <figure class="order" data-layout-note="leap">
        {snail_leap6}
        <figcaption><b>Leap.</b> One down, two across, wrapping at the edge; when the orbit closes, a new start one column
          to the right. The Zodiac’s step.</figcaption>
      </figure>
      <figure class="order" data-layout-note="furrows">
        {snail_furrows}
        <figcaption><b>Furrows.</b> Down the first column, up the next, from the left, as the ox turns at the end of the field.</figcaption>
      </figure>
      <figure class="order" data-layout-note="unwinding">
        {snail_unwinding}
        <figcaption><b>Unwinding.</b> From the centre outward, counter-clockwise, to the top-right corner.</figcaption>
      </figure>
      <figure class="order" data-layout-note="grille">
        {grille_order}
        <figcaption><b>Grille.</b> Read the signs under the holes row by row, then turn the grille a quarter clockwise:
          N, E, S, W. Its key decides where the holes are.</figcaption>
      </figure>
    </div>
    <p class="note"><a href="/about#layouts">Where do they come from?</a></p>
    <p class="note" data-path-withheld>Path withheld: the file says nothing of it. The reader finds the way among the five.</p>
  </aside>
</div>
"#,
        game_checked = checked(!state.secret),
        secret_checked = checked(state.secret),
        phaistos_checked = phaistos_checked,
        rongorongo_checked = checked(keyless == Some("rongorongo")),
        leap_checked = checked(keyless == Some("leap")),
        furrows_checked = checked(keyless == Some("furrows")),
        unwinding_checked = checked(keyless == Some("unwinding")),
        grille_checked = checked(state.grille_chosen),
        snail_phaistos = snail_svg(4, "phaistos"),
        snail_rongorongo = snail_svg(4, "rongorongo"),
        snail_leap = snail_svg(4, "leap"),
        snail_leap6 = snail_svg(6, "leap"),
        snail_furrows = snail_svg(4, "furrows"),
        snail_unwinding = snail_svg(4, "unwinding"),
        grille_order = grille_order_svg(),
        message = h(&state.message),
        glyph_table = h(&GLYPH_TABLE_JSON),
        grille_key = h(&state.grille_key),
        example = rustergraphia::grille::EXAMPLE,
        grille_invalid = invalid_focus(&state.grille_error),
        grille_error = error_text(&state.grille_error),
        decoy_invalid = invalid_focus(&state.decoy_error),
        decoy = state.decoy.as_deref().map(h).unwrap_or_default(),
        decoy_error = error_text(&state.decoy_error),
        passphrase_invalid = invalid_focus(&state.passphrase_error),
        passphrase_error = error_text(&state.passphrase_error),
        puzzle_open = if state.puzzle_open() { " open" } else { "" },
        puzzle_state = h(&state.puzzle_state()),
        rungs = rungs,
        condition_error = error_text(&state.condition_error),
        shared_checked = checked(!state.own_tablet),
        own_checked = checked(state.own_tablet),
        tablet_invalid = invalid_escaped(&state.tablet_error),
        tablet_error = error_text(&state.tablet_error),
        shown_checked = checked(!state.pathless),
        withheld_checked = checked(state.pathless),
        path_invalid = invalid_escaped(&state.path_error),
        path_error = error_text(&state.path_error),
        anatomy_glyph = glyph_svg(
            1098,
            "glyph",
            Some("1098: one thousand, no hundreds, nine tens, eight units")
        ),
        tabula = tabula,
    )
}
