#!/usr/bin/env bash
# Regenerates src/lib/api/*.ts -- the TypeScript types of every JSON
# document openparts-server returns -- from the Rust types themselves
# (ts-rs), so the site and the API cannot drift apart. Needs the sibling
# openparts-server and openparts checkouts. CI runs this and fails if
# the committed files differ.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/src/lib/api"
rm -rf "$OUT"
# The documents (openparts-core) and the server's own responses.
TS_RS_EXPORT_DIR="$OUT" cargo test -q \
  --manifest-path "$ROOT/../openparts/Cargo.toml" -p openparts-core --features ts export_bindings >/dev/null
TS_RS_EXPORT_DIR="$OUT" cargo test -q \
  --manifest-path "$ROOT/../openparts-server/Cargo.toml" --features ts export_bindings >/dev/null
echo "wrote $(ls "$OUT" | wc -l) files to $OUT"
