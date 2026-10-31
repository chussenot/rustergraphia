# Porting conventions (cistergraphia → rustergraphia)

This is the coordination contract for the port. The Ruby original lives at
`/home/user/cistergraphia` (read it; it is the specification). Each Rust
module mirrors one Ruby file under `lib/cistergraphia/`. Keep the Ruby
comments' voice: port the explanatory comments, adapted to Rust.

## Hard rules

- **File interoperability is the acceptance bar.** A plate (SVG) written by
  Ruby must decode here with the same result, and vice versa. Where output
  bytes can match, they must (same attribute order, same whitespace, same
  number formatting as Ruby string interpolation).
- **Determinism**: everything Ruby draws from a seeded `Random` goes through
  `crate::ruby_random::RubyRandom` (already ported and vector-tested — do
  not reimplement draws ad hoc). `SecureRandom` call sites use
  `getrandom::getrandom` instead (no compatibility needed).
  **Draw order matters**: keep the exact order and kind of RNG calls the
  Ruby code makes (e.g. Ruby evaluates `insert(rand(0..n), rand(9999), 9999)`
  arguments left to right).
- Do not edit `src/lib.rs`, `Cargo.toml`, or another module's file; your
  module may `use` siblings freely (one crate, cycles are fine).
- Errors: `crate::error::Error` variants mirror Ruby's exception classes.
  Where Ruby rescues `DecodeError`, match with `err.is_decode()`.
  Error **messages must match Ruby's text exactly** (tests and the web UI
  show them).
- No `unwrap()`/`panic!` on user input paths; `expect` only for genuine
  invariants. Clippy-clean (`cargo clippy --all-targets -- -D warnings`),
  rustfmt-formatted.
- Port the Ruby unit tests for your module (from `/home/user/cistergraphia/test/`)
  as `#[cfg(test)] mod tests` or `tests/` integration tests. Ruby 3.3 is
  installed: when in doubt about behavior, run the Ruby code and compare.

## Shared types (src/lib.rs — already in place)

```rust
pub type Value = u32;              // a sign's value, 0..=9999
pub type Line = [i32; 4];          // [x1, y1, x2, y2]
pub const EVIL_MARKER: Value = 9999;
pub const TABLET: [(char, Value); 27];       // Ruby TABLET (values NOT ×10)
pub fn character_of(sum: Value) -> Option<char>; // Ruby CHARACTERS[sum]
pub fn value_of(c: char) -> Option<Value>;       // Ruby VALUES[char]
pub fn character_sums() -> impl Iterator<Item = Value>; // CHARACTERS.keys
```

Sequences are `Vec<Value>`. Damage maps are `BTreeMap<usize, Vec<Line>>`
(index into the sequence → lost strokes, canonical and upright).
Grid cells are `(i32, i32)` (row, col) — they can be negative for crafted
files (Ruby's floor-division `cell_of`). A reading-path step is
`(i32, i32, bool)` = (row, col, turned).

## Module API contracts

Follow Ruby names snake_cased; same semantics, same constants. Key points:

- `glyphs`: `WIDTH=100`, `HEIGHT=150` (`i32`), `STEM: Line`;
  `digits(num) -> Vec<Value>`; `strokes(num, x, y) -> Vec<Line>` (stem
  first); `turn(line) -> Line`; `canonical(line) -> Line` (sort the two
  endpoints as (x,y) pairs); `read(&[Line]) -> Result<Value>`;
  plus whatever lookup tables weathering needs exposed:
  `lines_of(place_value) -> Option<&'static [Line]>` (Ruby `LINES`),
  `stroke_place(canonical_line) -> Option<Value>` (Ruby `STROKE_PLACES`),
  `digit_strokes(place, digit) -> &'static [Line]` canonical+sorted (Ruby
  `DIGIT_STROKES`, digit 0 → empty), `digit_of(place, sorted_canonical:
  &[Line]) -> Option<Value>` (Ruby `DIGITS`). Build these with
  `std::sync::LazyLock`. `PLACES` order is 1, 10, 100, 1000.
- `layout`: `Layout` enum `{ Phaistos, Rongorongo, Leap, Furrows,
  Unwinding, Grille(crate::grille::Grille) }` (derive Clone/Debug/
  PartialEq). `KEYLESS: [Layout; 5]`. Constants `SPACE=20`, `PADDING=50`,
  `CELL_WIDTH`, `CELL_HEIGHT`, `SQUARE_CELL`, `SQUARE_INSET`. Functions:
  `dimensions(len, &Layout, tally) -> Result<(i32, i32, usize, usize)>`,
  `cell`, `inset`, `path(size, &Layout) -> Result<Vec<(i32,i32,bool)>>`,
  `others(&Layout, size) -> Vec<Layout>`, `pairs`,
  `placed(&[Value], cols, &Layout, pathless) -> Vec<Placed>` where
  `Placed { row, col, turned, num, index, seed: i64 }`,
  `cell_origin`, `origin`, `relative(line, x, y)`, `upright(lines, row,
  col, turned, &Layout)`, `cell_of(line, &Layout) -> (i32, i32)` (use
  `div_euclid` for Ruby's floor `div`).
- `stylus`: `press(stroke: Line) -> String`; `wedge(stroke, &mut
  RubyRandom) -> [Vec<(f64, f64)>; 2]`; `outline(&[(f64,f64)], x, y) ->
  String`. Number formatting must match Ruby `Float#round(1)` then `to_s`:
  one decimal, and integral values keep `.0` (62.0 → `"62.0"`); round half
  away from zero. Page-hand seed: fold the canonical stroke as
  `acc * 1000 + value` over its 4 coords (i64).
- `svg`: `render(&[Value], &Layout, damage, tally, pathless, tablet:
  Option<&str>) -> Result<String>` — byte-for-byte the Ruby heredoc
  (attribute order, indentation, blank lines from empty sections, trailing
  newline). `read(svg, &Layout, strokes: Option<&[Line]>) ->
  Result<Vec<Value>>`; `lines(svg, Layer) -> Result<Vec<Line>>` with
  `enum Layer { Strokes, Lacunae, Tally }`; `is_square(svg) -> bool`;
  `stencil(size, &Grille) -> String`; `grid_size(cell count) ->
  Result<usize>`; `press(strokes, turned, origin, &mut RubyRandom) ->
  Vec<String>`; `line_tag(Line) -> String`; `coordinate(&str) ->
  Result<i32>` (parse float, Ruby `Float#round`: half away from zero).
  XML parsing via `xmltree`; replicate the per-thread single-file cache
  (`thread_local!`) with a `forget()` for the web layer; reject non-SVG
  roots with Ruby's messages.
- `cipher` (Ruby's `lib/cistergraphia.rb` module functions): `normalize`,
  `encode(message, passphrase: Option<&str>, even: bool, tablet:
  Option<&str>, random: &mut RubyRandom) -> Result<Vec<Value>>`,
  `decode(svg, passphrase: Option<&str>, layout: Option<&Layout>) ->
  Result<String>`, `read_whole`, `inscribe(message, passphrase,
  &Layout, tally, pathless, tablet, random) -> Result<(Vec<Value>,
  String)>`, `unambiguous`, `to_svg`, `cister`, `decipher(&[Value],
  tablet: Option<&str>) -> Result<String>`, `split`, `inject_evil_symbols`,
  `strip_evil_symbols`, `perfect_square`.
- Callers that need "the process's own random" use `RubyRandom::new_os()`.

## Already ported (do not redo)

- `ruby_random` (vector-tested against Ruby 3.3)
- `grille` (verified byte-identical paths/novice keys vs Ruby). The svg/
  cipher wave adds `grille::check(svg, layout)` when `svg::is_square` lands.
- `error`, shared types in `lib.rs`.

## Testing against Ruby

`ruby -r/home/user/cistergraphia/lib/cistergraphia -e '...'` runs the
original. Compare Rust output against it in tests' dev-time (don't shell
out to Ruby from committed tests; bake expected values in as fixtures).
