UI proofs: the clone, screenshot for screenshot
===============================================

This page shows that Rustergraphia's web scriptorium is a faithful clone of
the Ruby original. The same pages were taken from both servers, running side
by side, and compared pixel by pixel.

**Result: 16 of 16 deterministic pages are pixel-identical: 0 differing
pixels out of 31,943,680.** The one page that cannot match, a freshly
inscribed plate, differs only inside the plate. That plate is drawn at random
on every inscription, by design.

How the proof was taken
-----------------------

- **Two servers, side by side.** The Ruby original (Sinatra on Puma, from
  [chussenot/cistergraphia](https://github.com/chussenot/cistergraphia)) and
  this clone (`rustergraphia-web`, axum) both served the pages. They were
  started with the same `CHAPTER_DATE=2026-10-31`, so both set the same
  plate for the day's chapter.
- **The same input to both.** The Ruby library drew eleven seeded plates, one
  of each kind. Each plate file was posted to `/decode` on both servers, the
  way a reader brings a plate to read.
- **The same browser.** Chromium was driven by Playwright at a 1280 × 900
  viewport, taking full-page screenshots. Every page was loaded in a fresh
  context, with `prefers-reduced-motion` so the wedges are pressed at once
  rather than one by one.
- **An exact comparison.** Each pair was compared pixel by pixel with Pillow.
  `ui-proofs/report.json` holds the numbers. Both screenshots of every pair
  are committed, so the comparison can be checked by anyone.

Reproduce it with
`CISTERGRAPHIA_DIR=/path/to/cistergraphia tools/ui-proofs/run.sh`. It needs
the original's gems installed, Node with Playwright, and Python with Pillow.

The deterministic pages, two by two
-----------------------------------

Each row shows the Ruby original on the left and the Rust clone on the right.

### Inscribe

The form, with the ladder of conditions (intact, worn, broken, ruined), the
six ways, the tablet (shared or its own), and the path (shown or withheld).

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/inscribe.ruby.png) | ![](ui-proofs/inscribe.rust.png) |

**0 differing pixels** of 1,754,880.

The same page in dark mode: the iron-gall ink, inverted.

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/inscribe-dark.ruby.png) | ![](ui-proofs/inscribe-dark.rust.png) |

**0 differing pixels** of 1,754,880.

### The day's chapter

Saturday 31 October 2026 is Chapter 6, a ruined plate in three worn
fragments. Both servers draw the plate from the date alone. It matches
because the clone replicates Ruby's random number generator bit for bit.

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/chapter.ruby.png) | ![](ui-proofs/chapter.rust.png) |

**0 differing pixels** of 2,625,280.

### Read: the form

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/read-form.ruby.png) | ![](ui-proofs/read-form.rust.png) |

**0 differing pixels** of 1,629,440.

### The reading desk: an intact plate, along the snail (Phaistos)

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/desk-phaistos.ruby.png) | ![](ui-proofs/desk-phaistos.rust.png) |

**0 differing pixels** of 2,176,000.

### The reading desk: Rongorongo, every other row turned half round

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/desk-rongorongo.ruby.png) | ![](ui-proofs/desk-rongorongo.rust.png) |

**0 differing pixels** of 2,176,000.

### The reading desk: under a turning grille (key `K7Q2M-ZXB4P`)

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/desk-grille.ruby.png) | ![](ui-proofs/desk-grille.rust.png) |

**0 differing pixels** of 1,981,440.

### The reading desk: a pathless plate, its way withheld

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/desk-pathless.ruby.png) | ![](ui-proofs/desk-pathless.rust.png) |

**0 differing pixels** of 2,087,680.

### The notebook: a plate with a tablet of its own

The reader builds the tablet sum by sum, and the scribe confirms entries
three at a time.

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/notebook-own-tablet.ruby.png) | ![](ui-proofs/notebook-own-tablet.rust.png) |

**0 differing pixels** of 3,297,280.

### The bench: a weathered plate to restore

The hatched lacunae, the digit picker, and the edition taking shape.

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/bench-weathered.ruby.png) | ![](ui-proofs/bench-weathered.rust.png) |

**0 differing pixels** of 2,133,760.

### The bench: a weathered plate with tally signs

The row and column sums sit on a higher cord, so whole signs may be lost.

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/bench-tallied.ruby.png) | ![](ui-proofs/bench-tallied.rust.png) |

**0 differing pixels** of 2,051,840.

### The board: a broken plate, its fragments in the tray

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/board-broken.ruby.png) | ![](ui-proofs/board-broken.rust.png) |

**0 differing pixels** of 2,374,400.

### The board: a ruined plate, broken and worn

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/board-ruined.ruby.png) | ![](ui-proofs/board-ruined.rust.png) |

**0 differing pixels** of 2,444,800.

### Decoded: a weathered plate's edition

Printed in the Leiden conventions of epigraphy: `[r]` marks a restored
letter.

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/decoded-edition.ruby.png) | ![](ui-proofs/decoded-edition.rust.png) |

**0 differing pixels** of 1,152,000.

### Decoded: a sealed plate (AES-256-GCM), opened with its passphrase

The plate was sealed by the Ruby library and opened by both servers.

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/decoded-sealed.ruby.png) | ![](ui-proofs/decoded-sealed.rust.png) |

**0 differing pixels** of 1,152,000.

### Decoded: two readers, read with the abbot's key

The abbot's key reads both messages. The novice's key reads only the decoy.

| Ruby original | Rust clone |
|---|---|
| ![](ui-proofs/decoded-two-readers.ruby.png) | ![](ui-proofs/decoded-two-readers.rust.png) |

**0 differing pixels** of 1,152,000.

The page that differs on purpose
--------------------------------

Inscribing *"Meet me at the old abbey after vespers"* draws new random splits
for every plate. Each letter's value is split into two signs differently each
time, and that is what makes the riddle a riddle. The two servers therefore
press two different plates around the same message, and the page around the
plate must still match.

| Ruby original | Rust clone | Where they differ |
|---|---|---|
| ![](ui-proofs/inscribed-result.ruby.png) | ![](ui-proofs/inscribed-result.rust.png) | ![](ui-proofs/inscribed-result.diff.png) |

The differing pixels, shown in red on the right, all fall inside the plate's
box (x 403–870, y 559–1221). Everything else on the page is identical: the
title, the message and how it was enciphered, the mode, sign count, grid and
layout, the downloads, the zoom controls, the reading hint and the footer.
Determinism on this page is proven elsewhere: the test suite
(`crates/rustergraphia-web/tests/seeded.rs`) seeds both implementations with
the same random draw and checks 21 such result pages byte for byte against
the Ruby app's own.

What these screenshots add to the test suite
--------------------------------------------

The test suite already compares the clone with the original at the level of
bytes:

- 67 golden HTTP responses captured from the running Ruby app;
- 21 seeded result pages;
- all 139 of the original's HTTP tests, ported;
- `tools/interop-check.sh`, which round-trips 16 kinds of plate between the two
  implementations and byte-compares 13 seeded kinds.

These screenshots check the last step, the page as a browser draws it. Fonts,
styles and scripts are served by the clone, and the plates are pressed in the
reader's browser. The pages come out the same, pixel for pixel.
