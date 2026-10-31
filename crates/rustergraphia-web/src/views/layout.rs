//! Cistergraphia Layout: "The Abbey Index" (views/layout.erb).
//! Shared frame of every page: masthead, the leaf (main), colophon.

use crate::esc::h;
use crate::view::{glyph_svg, DESCRIPTION};

/// What the frame needs of a page: the Ruby layout's ivars.
#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub title: Option<String>,
    pub secret: bool,
    /// `inscribe`, `read`, `chapter` or `about` — the nav's current page.
    pub page: Option<&'static str>,
    pub description: Option<String>,
    /// The request's base URL (or the chapter cache's placeholder host).
    pub base_url: String,
    /// The request's cleaned path.
    pub path: String,
}

impl Frame {
    fn description(&self) -> String {
        h(self.description.as_deref().unwrap_or(DESCRIPTION))
    }

    fn nav(&self, page: &str) -> String {
        let class = if self.page == Some(page) {
            "is-current"
        } else {
            ""
        };
        let current = if self.page == Some(page) {
            " aria-current=\"page\""
        } else {
            ""
        };
        format!("class=\"{class}\"{current}")
    }
}

pub fn render_layout(frame: &Frame, body: &str) -> String {
    let title = match &frame.title {
        Some(title) => format!("{} · ", h(title)),
        None => String::new(),
    };
    let og_title = h(frame.title.as_deref().unwrap_or("Cistergraphia"));
    let description = frame.description();
    let base = h(&frame.base_url);
    let mode = if frame.secret { "secret" } else { "game" };
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}Cistergraphia</title>
  <meta name="description" content="{description}">
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="Cistergraphia">
  <meta property="og:title" content="{og_title}">
  <meta property="og:description" content="{description}">
  <meta property="og:url" content="{base}{path}">
  <meta property="og:image" content="{base}/og.png">
  <meta property="og:image:width" content="1200">
  <meta property="og:image:height" content="630">
  <meta property="og:image:alt" content="Cistergraphia: your secrets, in a monk’s handwriting. Beside it, a plate of Cistercian numerals.">
  <meta name="twitter:card" content="summary_large_image">
  <meta name="color-scheme" content="light dark">
  <meta name="theme-color" content="#f3ede1" media="(prefers-color-scheme: light)">
  <meta name="theme-color" content="#191714" media="(prefers-color-scheme: dark)">
  <link rel="icon" href="/favicon.svg" type="image/svg+xml">
  <link rel="preload" href="/fonts/eb-garamond-latin-400-normal.woff2" as="font" type="font/woff2" crossorigin>
  <link rel="stylesheet" href="/css/app.css">
  <script src="/js/ink.js" defer></script>
  <script src="/js/ways.js" defer></script>
  <script src="/js/app.js" defer></script>
  <script src="/js/sound.js" defer></script>
</head>
<body data-mode="{mode}">
  <a class="skip-link" href="#main">Skip to content</a>

  <header class="masthead">
    <a class="brand" href="/">
      {brand}
      <span>
        <span class="brand-name">Cistergraphia</span>
        <span class="brand-tag">The ciphers of the monks</span>
      </span>
    </a>
    <nav class="nav" aria-label="Main">
      <a href="/" {nav_inscribe}>Inscribe</a>
      <a href="/decode" {nav_read}>Read</a>
      <a href="/chapter" {nav_chapter}>Chapter</a>
      <a href="/about" {nav_about}>About</a>
    </nav>
  </header>

  <main id="main" class="folio">
    {body}
  </main>

  <footer class="colophon">
    <p>After the ciphers of the Cistercian monks, Cîteaux, 1098.</p>
    <nav aria-label="About">
      <a href="https://github.com/chussenot/cistergraphia">Source</a>
      <a href="https://github.com/chussenot/cistergraphia/blob/master/docs/security-study.md">Security study</a>
      <button type="button" class="hear awaits-script" data-hear aria-pressed="false">Hear the hand</button>
    </nav>
  </footer>

  <div class="toast" id="toast" role="status" aria-live="polite"></div>
</body>
</html>
"##,
        path = h(&frame.path),
        brand = glyph_svg(
            1098,
            "glyph brand-mark",
            Some("1098, the founding of Cîteaux")
        ),
        nav_inscribe = frame.nav("inscribe"),
        nav_read = frame.nav("read"),
        nav_chapter = frame.nav("chapter"),
        nav_about = frame.nav("about"),
    )
}
