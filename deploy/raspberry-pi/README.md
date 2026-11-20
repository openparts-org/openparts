# Raspberry Pi + Cloudflare Tunnel

Runs `openparts-server` on a Raspberry Pi and publishes it on the
internet through a Cloudflare Tunnel. No inbound port is opened on the
Pi or the router: the server listens on `127.0.0.1:8080` only, and
`cloudflared` on the Pi connects out to Cloudflare.

Tested with a Raspberry Pi 4 (4 GB), Raspberry Pi OS / Debian 12,
64-bit (`aarch64`). The whole dataset loads in about 2 s and uses
well under 1 GB of memory.

## What runs where

| | Where it is built | Where it runs |
|---|---|---|
| `openparts-server` | your PC, cross-compiled to a static `aarch64` binary | Pi, systemd **user** service `openparts` |
| web site (`web/dist`) | your PC | served by the server |
| openparts-data | git checkout on your PC | copied to the Pi |
| 3D models (STEP/STL/3MF) | your PC, `openparts bundle` (needs SolidCraft) | served from the bundle |
| `cloudflared` | Cloudflare's apt repository | Pi, system service |

SolidCraft is not installed on the Pi, so 3D models come only from the
prebuilt bundle. Without a bundle the site works but 3D downloads
return `503`.

## Do not serve datasheets from a public server

Leave `OPENPARTS_SOURCE_CACHE` unset on a public deployment. With it
set, the server downloads manufacturers' PDFs and serves them from its
own origin, which redistributes those documents (Architecture
Specification section 20, Canonical Data Specification section 23).
Without it the site links every value to the original PDF on the
manufacturer's site. `openparts.service` leaves it out on purpose.

## 1. Build on your PC

From the monorepo root:

```sh
# Server for the Pi (static musl binary; zig is the cross linker)
rustup target add aarch64-unknown-linux-musl
cargo install cargo-zigbuild          # or: nix shell nixpkgs#zig nixpkgs#cargo-zigbuild
CARGO_TARGET_DIR=target/aarch64 \
  cargo zigbuild --release --target aarch64-unknown-linux-musl -p openparts-server

# Web site
(cd web && bun install && bun run build)

# 3D models (optional; takes a while and several GB)
cargo build --release -p openparts-cli
./target/release/openparts bundle --data ../openparts-data --out ../bundle
```

## 2. Prepare the Pi

1. Enable SSH and install a key from your PC
   (`ssh-copy-id <user>@raspberry.local`).
2. Let the user service run without a login session:
   `loginctl enable-linger` (on the Pi).
3. `rsync` must be installed on both sides.

## 3. Deploy

```sh
HOST=raspberry.local BUNDLE_DIR=../bundle \
  PUBLIC_URL=https://openparts.example.org/ \
  deploy/raspberry-pi/deploy.sh
```

`deploy.sh` copies the server binary, the web site, the data checkout
(without `.git`) and, if `BUNDLE_DIR` is given, the bundle to
`~/openparts/` on the Pi, installs `openparts.service` as a user unit
and restarts it. It never touches `~/openparts/state/` (the PCBCraft
ledger, below). Run it again after every data or code update. Without
`BUNDLE_DIR`, a bundle already on the Pi is kept; rebuild and pass it
when parts or packages changed.

On the Pi: `systemctl --user status openparts`,
`journalctl --user -u openparts`.

## PCBCraft catalog and its revision ledger

The server also serves the dataset as PCBCraft native libraries
(`/v1/pcbcraft/...`, `.../artifacts/pcbcraft`;
docs/spec/pcbcraft-export.md, "Catalog API"). PCBCraft quarantines two
different documents under one (library, kind, id, revision), so the
revisions are kept in a ledger that must outlive every deploy:

- `openparts.service` sets
  `OPENPARTS_PCBCRAFT_LEDGER=~/openparts/state/pcbcraft-ledger.json`.
  At each start the server exports every part in the background (a
  second or two on a PC, expect several on the Pi; the API answers at
  once and the PCBCraft endpoints `503` until it is done), gives changed
  records the next revision, and replaces the ledger atomically before
  serving anything.
- `deploy.sh` creates `~/openparts/state/` but never writes, copies over
  or deletes anything in it. It writes the deployed openparts-data
  commit to `~/openparts/data.env` (`OPENPARTS_DATA_REVISION`), which
  new records name in their source reference.
- Back the ledger up now and then
  (`scp raspberry.local:openparts/state/pcbcraft-ledger.json .`). If it
  is lost, the next start numbers every record from 1 again, and
  PCBCraft users who pinned an older copy of a changed record will see
  it quarantined. If it is corrupted, the server refuses to serve the
  PCBCraft endpoints (`503`, see `GET /v1/pcbcraft`) instead of
  replacing it; restore the backup.
- Run one server per ledger file.

## 4. Cloudflare Tunnel

Needs a Cloudflare account and a domain whose nameservers point to
Cloudflare (Add a domain, Free plan, then change the nameservers at the
registrar and wait until the zone is Active).

1. Dashboard: **Networking → Tunnels → Create Tunnel**, type
   *Cloudflared*, environment *Debian*, architecture *arm64*.
2. On the Pi, run the two commands the dashboard shows: install
   `cloudflared` from Cloudflare's apt repository, then
   `sudo cloudflared service install <token>`. The token is a secret;
   don't commit or share it.
3. When the connector shows *Healthy*, open the tunnel's **Routes** and
   add a *Published application*:

   | Field | Value |
   |---|---|
   | Subdomain | e.g. `openparts` |
   | Domain | your zone |
   | Service URL | `http://localhost:8080` |

   The service URL is plain HTTP on purpose: it is the hop inside the
   Pi. Visitors get HTTPS from Cloudflare, and the Pi-to-Cloudflare leg
   runs inside the encrypted tunnel.

Cloudflare creates the DNS record and the certificate; the first HTTPS
request can fail for a few minutes until the certificate is issued.
