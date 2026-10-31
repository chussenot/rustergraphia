#!/usr/bin/env bash
# Ruby <-> Rust interop check: both implementations write one plate of every
# kind; each decodes the other's; seeded plates must match byte for byte.
#
# Needs Ruby 3.2+ and a checkout of the Ruby original (CISTERGRAPHIA_DIR,
# default /home/user/cistergraphia); skips gracefully without them.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CISTERGRAPHIA_DIR="${CISTERGRAPHIA_DIR:-/home/user/cistergraphia}"
OUT="${INTEROP_OUT:-$(mktemp -d)}"
KEY="K7Q2M-ZXB4P"
PASSPHRASE="monastery gate"

if ! command -v ruby >/dev/null 2>&1; then
    echo "SKIP: ruby is not installed"
    exit 0
fi
if [ ! -f "$CISTERGRAPHIA_DIR/lib/cistergraphia.rb" ]; then
    echo "SKIP: the Ruby original was not found at $CISTERGRAPHIA_DIR (set CISTERGRAPHIA_DIR)"
    exit 0
fi

echo "Plates land in $OUT"
failed=0
fail() {
    echo "FAIL: $1"
    failed=1
}

echo "— Rust writes its plates"
cargo run --quiet --manifest-path "$ROOT/Cargo.toml" -p rustergraphia --example xcheck "$OUT" >/dev/null || {
    echo "FAIL: the Rust writer did not run"
    exit 1
}

echo "— Ruby writes its plates and reads the Rust ones"
RUBY_LOG="$OUT/ruby.log"
ruby -r"$CISTERGRAPHIA_DIR/lib/cistergraphia" - "$OUT" <<'RUBY' >"$RUBY_LOG" 2>&1
out = ARGV[0]
KEY = 'K7Q2M-ZXB4P'
PASSPHRASE = 'monastery gate'

%i[phaistos rongorongo leap furrows unwinding].each do |way|
  _, svg = Cistergraphia.inscribe('meet me at the old abbey after vespers', layout: way, random: Random.new(1234))
  File.write("#{out}/ruby-#{way}.svg", svg)
end
_, svg = Cistergraphia.inscribe('vespers at six', layout: :phaistos, pathless: true, random: Random.new(77))
File.write("#{out}/ruby-pathless.svg", svg)
_, svg = Cistergraphia.inscribe('ora et labora', layout: :phaistos, tablet: 'ABCDEFGH23', random: Random.new(99))
File.write("#{out}/ruby-tablet.svg", svg)
_, svg = Cistergraphia.inscribe('the relics are under the third stone', layout: Cistergraphia::Grille.new(KEY), random: Random.new(55))
File.write("#{out}/ruby-grille.svg", svg)
_, svg, = Cistergraphia::Lacunae.inscribe('ora et labora et ora', layout: :rongorongo, random: Random.new(42))
File.write("#{out}/ruby-weathered.svg", svg)
_, svg, = Cistergraphia::Lacunae.inscribe('ora et labora et ora', layout: :phaistos, tally: true, random: Random.new(43))
File.write("#{out}/ruby-weathered-tally.svg", svg)
_, svg, = Cistergraphia::Lacunae.inscribe('ora et labora et ora', layout: :leap, pathless: true, random: Random.new(44))
File.write("#{out}/ruby-weathered-pathless.svg", svg)
_, svg, = Cistergraphia::Fragments.inscribe('meet me at the old abbey after vespers', random: Random.new(31))
File.write("#{out}/ruby-broken.svg", svg)
_, svg, = Cistergraphia::Fragments.inscribe('meet me at the old abbey after vespers', weathered: true, random: Random.new(3))
File.write("#{out}/ruby-ruined.svg", svg)
_, svg, = Cistergraphia::TwoReaders.inscribe('the relics are under the third stone', 'vespers at six',
                                             grille: Cistergraphia::Grille.new(KEY))
File.write("#{out}/ruby-two.svg", svg)
_, svg, = Cistergraphia::TwoReaders.inscribe('the relics are under the third stone', 'vespers at six',
                                             passphrase: PASSPHRASE, grille: Cistergraphia::Grille.new(KEY))
File.write("#{out}/ruby-two-sealed.svg", svg)
_, svg = Cistergraphia.inscribe('In girum imus nocte', passphrase: PASSPHRASE)
File.write("#{out}/ruby-sealed.svg", svg)

Dir["#{out}/rust-*.svg"].sort.each do |path|
  name = File.basename(path)
  svg = File.read(path)
  begin
    if name.include?('two')
      passphrase = name.include?('sealed') ? PASSPHRASE : nil
      reading = Cistergraphia::TwoReaders.read(svg, Cistergraphia::Grille.new(KEY), passphrase: passphrase)
      puts "ruby decodes #{name}: #{reading.message} / decoy: #{reading.decoy || '-'}"
    else
      layout = name.include?('grille') ? Cistergraphia::Grille.new(KEY) : nil
      passphrase = name.include?('sealed') ? PASSPHRASE : nil
      puts "ruby decodes #{name}: #{Cistergraphia.decode(svg, passphrase: passphrase, layout: layout)}"
    end
  rescue Cistergraphia::Error => e
    puts "ruby FAILS on #{name}: #{e.message}"
  end
end
RUBY
if [ $? -ne 0 ]; then
    cat "$RUBY_LOG"
    echo "FAIL: the Ruby side did not run"
    exit 1
fi

echo "— Rust reads the Ruby plates"
RUST_LOG="$OUT/rust.log"
cargo run --quiet --manifest-path "$ROOT/Cargo.toml" -p rustergraphia --example xcheck "$OUT" >"$RUST_LOG" || {
    echo "FAIL: the Rust reader did not run"
    exit 1
}

echo "— Each side read the other"
grep 'ruby decodes' "$RUBY_LOG"
grep 'rust decodes' "$RUST_LOG"
if grep -q 'FAILS' "$RUBY_LOG" "$RUST_LOG"; then
    grep 'FAILS' "$RUBY_LOG" "$RUST_LOG"
    fail "a plate did not decode on the other side"
fi
ruby_reads=$(grep -c 'ruby decodes' "$RUBY_LOG")
rust_reads=$(grep -c 'rust decodes' "$RUST_LOG")
[ "$ruby_reads" -eq 16 ] || fail "ruby read $ruby_reads of 16 Rust plates"
[ "$rust_reads" -eq 16 ] || fail "rust read $rust_reads of 16 Ruby plates"

echo "— Seeded plates, byte for byte"
for kind in phaistos rongorongo leap furrows unwinding pathless tablet grille \
            weathered weathered-tally weathered-pathless broken ruined; do
    if cmp -s "$OUT/rust-$kind.svg" "$OUT/ruby-$kind.svg"; then
        echo "identical: $kind"
    else
        fail "$kind differs between the implementations"
    fi
done

if [ "$failed" -eq 0 ]; then
    echo "PASS: the two implementations read each other, and every seeded plate matches"
else
    echo "FAIL: see above"
fi
exit "$failed"
