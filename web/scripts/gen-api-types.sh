#!/usr/bin/env bash
# Regenerates src/lib/api/*.ts -- the TypeScript types of every JSON
# document openparts-server returns -- from the Rust types themselves
# (ts-rs), so the site and the API cannot drift apart. CI runs this and
# fails if the committed files differ.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/src/lib/api"
rm -rf "$OUT"
# The documents (openparts-core) and the server's own responses.
for crate in openparts-core openparts-server; do
  TS_RS_EXPORT_DIR="$OUT" cargo test -q --manifest-path "$ROOT/../Cargo.toml" \
    -p "$crate" --features ts export_bindings >/dev/null
done
echo "wrote $(ls "$OUT" | wc -l) files to $OUT"
