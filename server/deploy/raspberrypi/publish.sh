#!/usr/bin/env bash
# Build everything on the PC and publish it to the Pi.
#   PI=pi@raspberrypi.local SITE=https://openparts.jijinbei.jp ./publish.sh
# (DATA=<openparts-data checkout>, default: next to this repository)
#
# Builds: the aarch64 server binary (static, no toolchain on the Pi),
# the artifact bundle (needs SolidCraft on this PC), and the web site for
# the root of SITE; then rsyncs them with the dataset and restarts the
# service. Re-run after every dataset or code change.
set -euo pipefail
PI="${PI:?set PI=user@host}"
SITE="${SITE:-https://openparts.jijinbei.jp}"
repo="$(cd "$(dirname "$0")/../../.." && pwd)"   # the openparts repository
data="${DATA:-$repo/../openparts-data}"            # the dataset checkout
build="$repo/target/publish"
mkdir -p "$build"

echo "== server (aarch64-unknown-linux-musl)"
rustup target add aarch64-unknown-linux-musl >/dev/null
(cd "$repo" && CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
  cargo build --release -p openparts-server --target aarch64-unknown-linux-musl)

echo "== artifact bundle"
(cd "$repo" && cargo build --release -p openparts-cli)
rm -rf "$build/bundle"
"$repo/target/release/openparts" bundle --data "$data" --out "$build/bundle"

echo "== web site for $SITE"
(cd "$repo/web" && OPENPARTS_SITE="$SITE" OPENPARTS_BASE=/ PUBLIC_OPENPARTS_API_BASE=/api bun run build)

echo "== upload to $PI"
rsync -az --delete "$repo/target/aarch64-unknown-linux-musl/release/openparts-server" "$PI:/srv/openparts/bin/"
rsync -az --delete --exclude .git "$data/" "$PI:/srv/openparts/data/"
rsync -az --delete "$build/bundle/" "$PI:/srv/openparts/bundle/"
rsync -az --delete "$repo/web/dist/" "$PI:/srv/openparts/web/"
ssh "$PI" 'sudo systemctl restart openparts-server && sleep 2 && curl -fsS http://127.0.0.1:8080/api/health >/dev/null && echo "server up"'
