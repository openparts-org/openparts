#!/usr/bin/env bash
# One-time setup on the Raspberry Pi (64-bit Raspberry Pi OS / Debian):
# directories, the systemd unit and cloudflared. Run with sudo.
#   sudo ./setup-pi.sh
set -euo pipefail
[[ $(id -u) -eq 0 ]] || { echo "run with sudo" >&2; exit 1; }
[[ $(uname -m) == aarch64 ]] || { echo "expects 64-bit (aarch64) OS, got $(uname -m)" >&2; exit 1; }

here="$(cd "$(dirname "$0")" && pwd)"
install -d -m 755 /srv/openparts/{bin,data,bundle,web}
# Owned by the user who deploys (publish.sh rsyncs as that user).
owner="${SUDO_USER:-root}"
chown -R "$owner": /srv/openparts
[[ -f /srv/openparts/openparts.env ]] || install -m 644 -o "$owner" "$here/openparts.env" /srv/openparts/openparts.env
install -m 644 "$here/openparts-server.service" /etc/systemd/system/openparts-server.service
systemctl daemon-reload
systemctl enable openparts-server

# cloudflared from Cloudflare's own apt repository.
if ! command -v cloudflared >/dev/null; then
  install -d -m 755 /usr/share/keyrings
  curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg -o /usr/share/keyrings/cloudflare-main.gpg
  echo "deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared any main" \
    > /etc/apt/sources.list.d/cloudflared.list
  apt-get update -qq && apt-get install -y -qq cloudflared
fi
echo "done. Next: publish from the PC (publish.sh), then connect the tunnel (README)."
