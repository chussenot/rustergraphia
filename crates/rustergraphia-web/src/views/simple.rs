//! The small leaves: Erratum (views/error.erb), Read (views/decode.erb) and
//! Transcription (views/decoded.erb).

use rustergraphia::grille::Part;

use crate::esc::h;
use crate::view::{glyph_svg, leiden_html};

pub fn render_error(error_message: &str) -> String {
    format!(
        r#"<section class="leaf leaf--narrow" aria-labelledby="erratum-title">
  <h1 id="erratum-title" class="rubric">Erratum</h1>
  <p class="erratum-text">{}</p>
  <div class="actions">
    <a class="btn btn--primary" href="/">Inscribe a message</a>
    <a class="btn" href="/decode">Read a plate</a>
  </div>
</section>
"#,
        h(error_message)
    )
}

pub fn render_decode() -> String {
    format!(
        r#"<section class="leaf leaf--narrow" aria-labelledby="read-title">
  <p class="folio-mark" aria-hidden="true">fol. 2<sup>r</sup></p>
  <h1 id="read-title" class="rubric">Read a plate</h1>
  <p class="lede">Bring an image made here. Its signs are read back along its path, whichever of the five ways it runs; sealed ones need their passphrase, and those written under a grille need its key. A game plate opens as its puzzle, the reading desk, the bench or the board, with the decoder’s answer one click below.</p>
  <p class="record" data-record="read" aria-live="polite"></p>
  <p class="chapter-invite">No plate to hand: <a href="/chapter">read the day’s chapter</a>, the plate the abbey sets each day, the same for every reader.</p>

  <form class="entry" method="post" action="/decode" enctype="multipart/form-data">
    <label class="dropzone" for="file" data-dropzone>
      {dropzone_glyph}
      <span class="dropzone-title">Drop an SVG here</span>
      <span class="dropzone-sub">or click to choose a file</span>
      <span class="dropzone-file" data-file-name aria-live="polite"></span>
      <input type="file" id="file" name="file" accept=".svg,image/svg+xml">
    </label>

    <details class="paste">
      <summary>…or paste its SVG code</summary>
      <div class="field">
        <label for="svg" class="visually-hidden">SVG code</label>
        <textarea id="svg" name="svg" rows="8" class="code" spellcheck="false"
                  placeholder="&lt;svg xmlns=&quot;http://www.w3.org/2000/svg&quot; …"></textarea>
      </div>
    </details>

    <div class="field">
      <label for="passphrase">Passphrase <span class="label-note">· only for sealed images</span></label>
      <div class="passphrase">
        <input type="password" id="passphrase" name="passphrase" autocomplete="current-password"
               autocapitalize="off" spellcheck="false">
        <button type="button" class="btn btn--small" data-reveal="passphrase" aria-pressed="false">Show</button>
      </div>
    </div>

    <div class="field">
      <label for="grille">Grille key <span class="label-note">· only for plates written under a grille</span></label>
      <input type="text" id="grille" name="grille" class="key-input" placeholder="{example}"
             autocomplete="off" autocapitalize="characters" spellcheck="false">
    </div>

    <button class="btn btn--primary" type="submit">Read</button>
  </form>
</section>
"#,
        dropzone_glyph = glyph_svg(9999, "glyph", Some("A Cistercian sign")),
        example = rustergraphia::grille::EXAMPLE,
    )
}

pub struct Decoded<'a> {
    pub broken: bool,
    pub weathered: bool,
    pub secret: bool,
    pub reader: Option<Part>,
    pub message: &'a str,
    pub decoy: Option<&'a str>,
}

pub fn render_decoded(page: &Decoded) -> String {
    let lede = if page.broken && page.weathered {
        "Rejoined from a ruined plate: of every way its fragments fit the board, turned or not, one lets every pair balance. Then restored, in the epigraphers’ notation: [r] the pair balances only one way; [b/l] it balances both ways, and the words decide.".to_string()
    } else if page.broken {
        "Rejoined from a broken plate: of every way its fragments fit the board, turned or not, one balances every pair.".to_string()
    } else if page.weathered {
        "Restored from a worn plate, in the epigraphers’ notation: [r] the pair balances only one way; [b/l] it balances both ways, and the words decide.".to_string()
    } else if page.secret {
        h("Unsealed with its passphrase: authentic and unaltered.")
    } else {
        h("Read from an open plate: letters and spaces, as enciphered.")
    };
    let reader = match page.reader {
        Some(Part::Abbot) => "Read with the abbot’s key: the novice’s message follows.",
        Some(Part::Novice) => "Read with the novice’s key.",
        _ => "",
    };
    let transcription = if page.weathered {
        leiden_html(page.message)
    } else {
        h(page.message)
    };
    let second = match page.decoy {
        Some(decoy) => format!(
            "    <h2 class=\"index-heading\">The novice’s grille reads</h2>\n    <p class=\"transcription transcription--second\">{}</p>\n",
            h(decoy)
        ),
        None => String::new(),
    };
    format!(
        r#"<section class="leaf leaf--narrow" aria-labelledby="transcription-title">
  <p class="folio-mark" aria-hidden="true">fol. 3<sup>r</sup></p>
  <h1 id="transcription-title" class="rubric">Transcription</h1>
  <p class="lede">{lede}
    {reader}</p>

  <p class="transcription" data-transcription>{transcription}</p>
{second}
  <div class="actions">
    <button type="button" class="btn" data-action="copy-text">Copy text</button>
    <a class="btn btn--primary" href="/decode">Read another</a>
    <a class="btn btn--quiet" href="/">Inscribe a message</a>
  </div>
</section>
"#
    )
}
