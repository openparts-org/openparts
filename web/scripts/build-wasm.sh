#!/usr/bin/env bash
# Builds the part viewer (crates/viewer/openparts-viewer, Rust + wgpu) to
# WebAssembly and generates its JS bindings into src/viewer-pkg/ (not
# committed: `bun run build` and `bun run dev` run this first).
#
# Needs the wasm32-unknown-unknown target and wasm-bindgen-cli at exactly
# the version of the crate's `wasm-bindgen` dependency (Cargo.lock):
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version "$(scripts/build-wasm.sh --bindgen-version)"
#
# The crate is built with the workspace's `wasm-release` profile (size:
# opt-level "z", LTO, one codegen unit), then shrunk with binaryen's
# wasm-opt ($WASM_OPT_FLAGS) when it is on the PATH -- CI installs the
# version `--binaryen-version` prints. Without wasm-opt the build still
# works, only bigger.
#
# The default flags are -Oz without its optimizing inliner: measured on
# the viewer (binaryen 133), plain -Oz and -O3 shrink the raw file but
# grow it gzipped (-Oz 2.58 MB / 1.058 MB gz, -O3 2.66 / 1.065, none
# 2.86 / 1.024); -Oz --skip-pass=inlining-optimizing gives 2.63 / 1.005.
set -euo pipefail
WEB="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="$WEB/.."
# The binaryen release CI installs (github.com/WebAssembly/binaryen).
BINARYEN_VERSION=133
VERSION="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$REPO/Cargo.lock")"
case "${1:-}" in
  --bindgen-version) echo "$VERSION"; exit 0 ;;
  --binaryen-version) echo "$BINARYEN_VERSION"; exit 0 ;;
esac
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
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  --manifest-path "$REPO/Cargo.toml" -p openparts-viewer
rm -rf "$WEB/src/viewer-pkg"
wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section \
  --out-dir "$WEB/src/viewer-pkg" \
  "$TARGET_DIR/wasm32-unknown-unknown/wasm-release/openparts_viewer.wasm"
WASM="$WEB/src/viewer-pkg/openparts_viewer_bg.wasm"
if command -v wasm-opt >/dev/null; then
  # The features rustc's wasm32 target emits (its defaults since 1.82).
  # shellcheck disable=SC2086
  wasm-opt ${WASM_OPT_FLAGS:--Oz --skip-pass=inlining-optimizing} \
    --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue \
    "$WASM" -o "$WASM.opt"
  mv "$WASM.opt" "$WASM"
else
  echo "build-wasm: warning: wasm-opt (binaryen) not found; the viewer is not size-optimised." >&2
  echo "  Install binaryen version_$BINARYEN_VERSION: https://github.com/WebAssembly/binaryen/releases" >&2
fi
echo "wrote $WEB/src/viewer-pkg ($(du -h "$WASM" | cut -f1) wasm)"
