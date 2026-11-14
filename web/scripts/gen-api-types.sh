#!/usr/bin/env bash
# Regenerates api/types.mbt -- the MoonBit types of every JSON document
# openparts-server returns -- from the Rust types themselves (schemars
# JSON Schema -> gen_api_types.py), so the site and the API cannot drift
# apart. CI runs this and fails if the committed file differs.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCHEMA="$(mktemp)"
trap 'rm -f "$SCHEMA"' EXIT
OPENPARTS_SCHEMA_OUT="$SCHEMA" cargo test -q --manifest-path "$ROOT/../Cargo.toml" \
  -p openparts-server --features schema --lib export_schemas >/dev/null
python3 "$ROOT/scripts/gen_api_types.py" "$SCHEMA" "$ROOT/api/types.mbt"
(cd "$ROOT" && moon fmt api >/dev/null)
echo "wrote $ROOT/api/types.mbt"
