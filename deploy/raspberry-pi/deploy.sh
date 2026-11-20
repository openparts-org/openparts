#!/usr/bin/env bash
# Deploy openparts-server to a Raspberry Pi (64-bit Raspberry Pi OS /
# Debian) as a systemd user service listening on 127.0.0.1:8080, to be
# published through a Cloudflare Tunnel. See README.md.
#
# Environment:
#   HOST        ssh target of the Pi (default: raspberry.local)
#   DATA_DIR    openparts-data checkout (default: ../openparts-data next to the monorepo)
#   BUNDLE_DIR  prebuilt 3D artifact bundle from `openparts bundle` (optional)
#   PUBLIC_URL  public URL to check after the deploy (optional)
set -euo pipefail

REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HOST=${HOST:-raspberry.local}
DATA_DIR=${DATA_DIR:-$REPO/../openparts-data}
BUNDLE_DIR=${BUNDLE_DIR:-}
PUBLIC_URL=${PUBLIC_URL:-}
TARGET=aarch64-unknown-linux-musl
BIN=$REPO/target/aarch64/$TARGET/release/openparts-server
WEB=$REPO/web/dist

[ -x "$BIN" ] || { echo "missing $BIN: build it first (README.md, step 1)" >&2; exit 1; }
[ -f "$WEB/index.html" ] || { echo "missing $WEB: build the web site first" >&2; exit 1; }
[ -d "$DATA_DIR/parts" ] || { echo "DATA_DIR=$DATA_DIR is not an openparts-data checkout" >&2; exit 1; }

# ~/openparts/state holds the PCBCraft revision ledger the server
# maintains: created here, never written or deleted by this script.
ssh "$HOST" 'mkdir -p ~/openparts/{bin,data,web,bundle,state} ~/.config/systemd/user'
rsync -az "$BIN" "$HOST":openparts/bin/openparts-server.new
rsync -az --delete --exclude .git "$DATA_DIR/" "$HOST":openparts/data/
rsync -az --delete "$WEB/" "$HOST":openparts/web/
rsync -az "$REPO/deploy/raspberry-pi/openparts.service" "$HOST":.config/systemd/user/openparts.service
# The data commit, named in new PCBCraft records' source references
# (the rsync above leaves .git behind).
DATA_REVISION=$(git -C "$DATA_DIR" rev-parse HEAD 2>/dev/null || echo unknown)
if [ -n "$(git -C "$DATA_DIR" status --porcelain --untracked-files=no 2>/dev/null)" ]; then
  DATA_REVISION="$DATA_REVISION-dirty"
fi
ssh "$HOST" "echo OPENPARTS_DATA_REVISION=$DATA_REVISION > ~/openparts/data.env"

if [ -n "$BUNDLE_DIR" ]; then
  [ -f "$BUNDLE_DIR/manifest.json" ] || { echo "BUNDLE_DIR=$BUNDLE_DIR has no manifest.json" >&2; exit 1; }
  rsync -az --delete "$BUNDLE_DIR/" "$HOST":openparts/bundle/
  ssh "$HOST" 'echo OPENPARTS_ARTIFACT_DIR=$HOME/openparts/bundle > ~/openparts/bundle.env'
elif ssh "$HOST" 'test -f ~/openparts/bundle/manifest.json'; then
  echo "BUNDLE_DIR not set: keeping the bundle already on the Pi"
else
  ssh "$HOST" 'rm -f ~/openparts/bundle.env'
fi

ssh "$HOST" 'mv ~/openparts/bin/openparts-server.new ~/openparts/bin/openparts-server &&
  systemctl --user daemon-reload &&
  systemctl --user enable openparts.service >/dev/null 2>&1 &&
  systemctl --user restart openparts.service &&
  for i in $(seq 1 30); do curl -s -o /dev/null http://127.0.0.1:8080/health && break; sleep 2; done;
  curl -s -o /dev/null -w "pi: %{http_code}\n" http://127.0.0.1:8080/;
  for i in $(seq 1 60); do curl -s http://127.0.0.1:8080/v1/pcbcraft | grep -q "\"state\":\"building\"" || break; sleep 2; done;
  echo "pcbcraft: $(curl -s http://127.0.0.1:8080/v1/pcbcraft)"'
if [ -n "$PUBLIC_URL" ]; then
  curl -s -o /dev/null -w "public: %{http_code}\n" "$PUBLIC_URL"
fi
