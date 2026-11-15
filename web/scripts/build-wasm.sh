#!/usr/bin/env bash
# Builds the part viewer (crates/viewer/openparts-viewer, Rust + wgpu) to
# WebAssembly and generates its JS bindings into src/viewer-pkg/ (not
# committed: `bun run build` and `bun run dev` run this first).
#
# Needs the wasm32-unknown-unknown target and wasm-bindgen-cli at exactly
# the version of the crate's `wasm-bindgen` dependency (Cargo.lock):
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version "$(scripts/build-wasm.sh --bindgen-version)"
set -euo pipefail
WEB="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="$WEB/.."
VERSION="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$REPO/Cargo.lock")"
if [ "${1:-}" = "--bindgen-version" ]; then
  echo "$VERSION"
  exit 0
fi
HAVE="$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }' || true)"
if [ "$HAVE" != "$VERSION" ]; then
  echo "build-wasm: wasm-bindgen-cli $VERSION is needed (found: ${HAVE:-none}):" >&2
  echo "  cargo install wasm-bindgen-cli --version $VERSION" >&2
  exit 1
fi
TARGET_DIR="${CARGO_TARGET_DIR:-$REPO/target}"
# The default toolchain may lack wasm32 (a pinned or nightly default);
# use stable's when it has it and the caller didn't pick one.
if [ -z "${RUSTUP_TOOLCHAIN:-}" ] && command -v rustup >/dev/null \
  && ! rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown \
  && rustup +stable target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
  export RUSTUP_TOOLCHAIN=stable
fi
cargo build --release --target wasm32-unknown-unknown \
  --manifest-path "$REPO/Cargo.toml" -p openparts-viewer
rm -rf "$WEB/src/viewer-pkg"
wasm-bindgen --target web --no-typescript --out-dir "$WEB/src/viewer-pkg" \
  "$TARGET_DIR/wasm32-unknown-unknown/release/openparts_viewer.wasm"
echo "wrote $WEB/src/viewer-pkg ($(du -h "$WEB/src/viewer-pkg/openparts_viewer_bg.wasm" | cut -f1) wasm)"
