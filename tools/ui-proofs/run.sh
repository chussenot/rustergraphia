#!/usr/bin/env bash
# Regenerates docs/ui-proofs: boots the Ruby original and this clone on the
# same chapter date, draws seeded plates of every kind with the Ruby
# library, screenshots the same pages from both servers, and counts the
# pixels that differ.
#
# Needs: Ruby 3.3 with the original's gems installed (bundle install in its
# checkout), Node with Playwright and Chromium, Python 3 with Pillow.
#
# Usage: CISTERGRAPHIA_DIR=/path/to/cistergraphia tools/ui-proofs/run.sh
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
original=${CISTERGRAPHIA_DIR:?set CISTERGRAPHIA_DIR to the cistergraphia checkout}
date=${CHAPTER_DATE:-2026-10-31}
ruby_port=${RUBY_PORT:-9401}
rust_port=${RUST_PORT:-9402}
work=$(mktemp -d)
pids=()
trap 'kill "${pids[@]}" 2>/dev/null || true; rm -rf "$work"' EXIT

wait_up() {
  for _ in $(seq 1 50); do
    curl -fs "http://127.0.0.1:$1/up" >/dev/null && return 0
    sleep 0.2
  done
  echo "server on port $1 did not come up" >&2
  return 1
}

cargo build --release -q -p rustergraphia-web --manifest-path "$root/Cargo.toml"
(cd "$original" && CHAPTER_DATE=$date RACK_ENV=production \
  exec bundle exec rackup -p "$ruby_port" -o 127.0.0.1 >"$work/ruby.log" 2>&1) &
pids+=($!)
CHAPTER_DATE=$date PORT=$rust_port "$root/target/release/rustergraphia-web" >"$work/rust.log" 2>&1 &
pids+=($!)
wait_up "$ruby_port"
wait_up "$rust_port"

mkdir -p "$work/plates"
CISTERGRAPHIA_DIR=$original ruby "$here/make_plates.rb" "$work/plates" >/dev/null
node "$here/capture.js" "http://127.0.0.1:$ruby_port" "http://127.0.0.1:$rust_port" \
  "$work/plates" "$root/docs/ui-proofs"
python3 -I "$here/compare.py" "$root/docs/ui-proofs"
