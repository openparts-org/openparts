# Self-hosting on a Raspberry Pi behind Cloudflare Tunnel

One Pi serves the whole site at `https://openparts.jijinbei.jp`:
`/` the web site, `/api/v1/...` the API and the prebuilt artifacts
(STEP/STL/3MF/KiCad from `openparts bundle`). The Pi needs no Rust, no
SolidCraft and no open ports: everything is built on the PC, and
cloudflared connects outward to Cloudflare. Artifacts are cached at
Cloudflare's edge (they carry `Cache-Control` and a content-hash ETag),
so most requests never reach the home line.

```
browser ── Cloudflare (TLS, cache) ── tunnel ── cloudflared ── 127.0.0.1:8080 openparts-server
                                                                  /srv/openparts/{data,bundle,web}
```

Requirements: Raspberry Pi 4/5 with 64-bit Raspberry Pi OS (an SSD is
recommended over an SD card), the domain's DNS on Cloudflare, and on the
PC: rustup, bun, rsync, SolidCraft (for the bundle).

## 1. Once, on the Pi

```sh
scp -r deploy/raspberrypi pi@raspberrypi.local:
ssh pi@raspberrypi.local 'sudo ./raspberrypi/setup-pi.sh'
```

Creates `/srv/openparts`, installs the hardened systemd unit
(`openparts-server.service`: throwaway user, read-only, localhost only)
and installs `cloudflared` from Cloudflare's apt repository.

## 2. Publish from the PC (and after every change)

```sh
PI=pi@raspberrypi.local SITE=https://openparts.jijinbei.jp deploy/raspberrypi/publish.sh
```

Builds the static aarch64 server, the artifact bundle and the web site,
rsyncs them with the dataset, restarts the service and checks
`/api/health`.

## 3. Once, connect the tunnel (Cloudflare dashboard)

1. Cloudflare dashboard → Zero Trust → Networks → Tunnels → Create a
   tunnel → Cloudflared → name it `openparts`.
2. Copy the install command it shows and run it on the Pi:
   `sudo cloudflared service install <TOKEN>` (runs as a systemd service).
3. Public hostname: subdomain `openparts`, domain `jijinbei.jp`,
   service `HTTP` → `localhost:8080`. Save.
4. Recommended cache rule (Caching → Cache Rules): when the URI path
   matches `/api/v1/parts/*/*/artifacts/*`, set **Eligible for cache** and
   **Respect origin TTL**. Without it Cloudflare caches only files with
   static extensions, and every artifact download reaches the Pi.

Check: `https://openparts.jijinbei.jp/api/health` returns `ok`.

## Operating notes

- Logs: `journalctl -u openparts-server -f`, `journalctl -u cloudflared -f`.
- The server reads `/srv/openparts` at start; `publish.sh` restarts it.
- Rolling back: re-run `publish.sh` from an older checkout.
- The `.sc` SolidCraft design in the bundle is not byte-reproducible
  (SolidCraft stores build timings); STEP/STL/3MF/KiCad are.
