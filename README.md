Rustergraphia
=============

**Every script draws a circle around its readers.**
*Your secrets, in a monk's handwriting — in Rust.*

Rustergraphia is a Rust clone of [cistergraphia](https://github.com/chussenot/cistergraphia),
the cipher that writes your message in the numerals of the Cistercian monks:
each letter becomes a monk's number, each number a sign, and a line of text
becomes a page that looks eight hundred years old. Everything the Ruby
original offers is here — the library, the command line, and the web
scriptorium — and the two implementations are **file-compatible**: a plate
written by either reads in the other.

What it does
------------

- **Six reading paths.** Phaistos (the snail), Rongorongo (the turning
  tablet), the leap (the Zodiac's step), the furrows (boustrophedon by
  columns), the unwinding (from the centre outward), and a turning
  **Cardan grille** under a 10-character key. The decoder tries the five
  keyless ways and keeps the one that reads.
- **Two ways to send it.** The riddle: a keyless cipher meant to be cracked.
  The seal: your exact words under AES-256-GCM, keyed from a passphrase by
  scrypt — the same signs, nothing but the length given away.
- **The reader's ladder.** Plates can be *pathless* (the reader finds the
  way), carry *a tablet of their own* (the reader builds the alphabet in the
  notebook, sum by sum), be *weathered* (strokes lost; the pair checksum
  restores them into an edition in the Leiden conventions: `[r]` restored,
  `[b/l]` for the reader to settle), carry *tally signs* (row and column
  sums on a higher cord, as on an Inka khipu, so whole signs can go), or be
  *broken* (two to four fragments in a tray, rejoined by their sums alone).
- **Two readers, one plate.** A decoy message under the novice's grille,
  drawn from the abbot's key: the abbot reads both, the novice only its own.
- **The day's chapter.** One plate a day, the same for every reader,
  climbing the ladder through the week.

Interoperability
----------------

This is not a look-alike: it is the same cipher, bit for bit.

- Ruby's seeded `Random` (MT19937, MRI's seeding, `limited_rand`, floats,
  `shuffle`, `sample`) is replicated in [`ruby_random`](crates/rustergraphia/src/ruby_random.rs)
  and vector-tested against Ruby 3.3 itself — so a plate's own tablet, the
  hand's seeded tremor and the day's chapter come out identical.
- Seeded plates in **every** mode are byte-identical with the Ruby
  library's output, and each implementation decodes the other's plates,
  sealed ones included. `tools/interop-check.sh` proves it whenever Ruby
  and the original repository are at hand, and CI runs it on every push.
- The web scriptorium renders the same pages: [docs/ui-proofs.md](docs/ui-proofs.md)
  screenshots the original and the clone side by side, and 16 of 16
  deterministic pages come out pixel-identical.
- The few places where the clone differs on purpose, and why, are listed in
  [docs/differences.md](docs/differences.md).

Try it
------

```
cargo build --release
target/release/rustergraphia -m "Meet me at the old abbey after vespers"
target/release/rustergraphia -m "Vespers at six" -g K7Q2M-ZXB4P --stencil grille.svg
target/release/rustergraphia -m "Vespers at six" -l leap --pathless -o plate.svg
target/release/rustergraphia -m "Meet me at the old abbey after vespers" --own-tablet -o plate.svg
target/release/rustergraphia -m "Ora et labora" --weathered --tally
target/release/rustergraphia -m "Ora et labora" --broken --weathered
target/release/rustergraphia -m "The relics are under the third stone" --decoy "Vespers at six" -g K7Q2M-ZXB4P
target/release/rustergraphia -d plate.svg          # decode; -s for sealed plates
cargo run -p rustergraphia-web                     # the web scriptorium on :9292
docker build -t rustergraphia . && docker run -p 8080:8080 rustergraphia   # the container Cloud Run runs
```

The passphrase for sealed plates is prompted without echo, or taken from
`CISTERGRAPHIA_PASSPHRASE` for scripts. PNG export uses librsvg
(`rsvg-convert`) or ImageMagick when installed.

For builders
------------

```
cargo test --workspace                   # the ported suite + Ruby-generated vectors
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
tools/interop-check.sh                   # needs Ruby 3.3 and the cistergraphia repo
```

The cipher lives in [`crates/rustergraphia`](crates/rustergraphia), one
module per Ruby file; the command line in
[`crates/rustergraphia-cli`](crates/rustergraphia-cli); the web app in
[`crates/rustergraphia-web`](crates/rustergraphia-web), serving the same
HTML as the original so its stylesheet and scripts carry over unchanged.
[PORTING.md](PORTING.md) is the porting contract;
[docs/web-port-spec.md](docs/web-port-spec.md) the web layer's functional
specification, surveyed from the running Ruby app; [docs/going-live.md](docs/going-live.md)
how it is deployed to Cloud Run behind Cloudflare.

Why, and further reading
------------------------

The history (David A. King's *The Ciphers of the Monks*), the design, the
security study and the cryptanalysis experiments live in the original:
[chussenot/cistergraphia](https://github.com/chussenot/cistergraphia).
