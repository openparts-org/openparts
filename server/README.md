# openparts-server

OpenParts distribution API server (Architecture Specification section
20). Serves search, part/device/package lookup, provenance, and
CAD artifacts (KiCad symbol/footprint, STEP, STL, 3MF), prebuilt or
generated on demand, over HTTP. Canonical Data editing is out of scope here -- this crate is
read-only; all writes happen through `openparts-data` Pull Requests.

## Status

First slice: loads `openparts-data` into an in-memory index at startup
(`src/store.rs`) instead of a real PostgreSQL-backed Importer
(Architecture Specification section 22) -- the HTTP layer above it is
written so that swap doesn't require route changes later. No
authentication, rate limiting, caching, or object storage yet.

3D models (STEP, STL, 3MF) are served prebuilt from an artifact bundle
-- see "Prebuilt artifacts" below -- so the server itself needs no
SolidCraft. Without a bundle, or for an artifact the bundle lacks, they
are built per request by a locally installed SolidCraft (see
`openparts/README.md`, "3D models"; 0.4-2.6 s each), and without
SolidCraft that request is `503`. The container image has no SolidCraft.

## Prebuilt artifacts

Build the bundle once per dataset release, on a machine with SolidCraft
installed (about 5 s for the current ~90 parts on 20 cores):

```sh
cd ../openparts
cargo run --release -p openparts-cli -- bundle --data ../openparts-data --out /srv/openparts-bundle
```

Ship that directory next to the same `openparts-data` revision and
point the server at it:

```sh
OPENPARTS_DATA_DIR=../openparts-data OPENPARTS_ARTIFACT_DIR=/srv/openparts-bundle cargo run --release
```

`manifest.json` maps (manufacturer, mpn) to each artifact's SHA-256;
the files are `artifacts/sha256/<hex>` (layout and manifest schema:
`openparts/README.md`, "Prebuilt artifacts"). Per request:

- STEP/STL/3MF: from the bundle for every `silicon_revision` (a 3D model
  depends on the package only; the revision is still checked, so an
  unknown one stays `422`). Missing from the bundle: built live with
  SolidCraft if installed, else `503` saying so.
- KiCad footprint: from the bundle (package only), else built live.
- KiCad symbol: from the bundle for the base revision; with a
  `silicon_revision` it is built live, since pins can differ.
- Bundled files are re-hashed when read; one that doesn't match the
  manifest is not served (logged, then treated as missing).

`X-Content-Hash` is the same SHA-256 the manifest lists. KiCad
artifacts never need SolidCraft, so they are always available.

## Running

```sh
# from this directory, with a sibling ../openparts-data checkout
cargo run
# or point at a different data directory / port, and serve a bundle
OPENPARTS_DATA_DIR=/path/to/openparts-data OPENPARTS_ARTIFACT_DIR=/path/to/bundle PORT=8080 cargo run
```

## API

| Route | Description |
| --- | --- |
| `GET /health` | liveness check |
| `GET /v1/search?q=...` | substring search over manufacturer/MPN |
| `GET /v1/parts/{manufacturer}/{mpn}` | Part |
| `GET /v1/parts/{manufacturer}/{mpn}/provenance` | Part+Device+Package provenance |
| `GET /v1/parts/{manufacturer}/{mpn}/revisions` | Device's declared silicon revisions |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts` | links to available artifact kinds |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts/kicad-symbol?silicon_revision=...` | generated `.kicad_sym` text |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts/kicad-footprint?silicon_revision=...` | generated `.kicad_mod` text |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts/step?silicon_revision=...` | STEP text (coloured AP242) |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts/stl?silicon_revision=...` | binary STL |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts/3mf?silicon_revision=...` | 3MF with a colour per body |
| `GET /v1/devices/{id}` | Device (`id` may contain `/`, e.g. `raspberrypi/RP2040`) |
| `GET /v1/packages/{id}` | Package |
| `GET /v1/sources/{id}` | Source |
| `GET /v1/sources/{id}/document` | the Source's PDF, fetched and cached (opt-in, below) |
| `GET /v1/parts/{manufacturer}/{mpn}/reviews` | review status of every fact of the part, its device and package, with counts |
| `GET /v1/reviews/queue?category=&manufacturer=&mpn=&status=&offset=&limit=` | facts to review (`status`: `unreviewed` (default), `disputed`, `confirmed`, `all`), parts with electrical parameters first |
| `GET /v1/reviews/config` | `{writes_enabled, documents_enabled}` |
| `POST /v1/reviews` | submit one review (`Authorization: Bearer <token>`; opt-in, below) |

Artifact responses carry an `X-Content-Hash: sha256:<hex>` header
(Architecture Specification section 24, Content Addressing). Missing
required dimensions or an unknown `silicon_revision` return `422` with
a diagnostic body rather than guessing (Testing and Quality
Specification section 7). A 3D model that is neither bundled nor
buildable here (no SolidCraft) returns `503`.

## Reviews (opt-in writes)

The server never writes openparts-data. With `OPENPARTS_REVIEW_DIR`
(a directory) and `OPENPARTS_REVIEW_TOKEN` (a shared secret of at least
16 characters) set, `POST /v1/reviews` accepts one review of one fact
(Canonical Data Specification section 26.1):

```json
{"target_kind": "package", "target": "standards/QFN56-RP2040",
 "pointer": "/dimensions/body_width", "status": "confirmed",
 "reviewer": "alice", "value_hash": "sha256:..."}
```

(`disputed` needs a `comment`; `corrected_value` is optional.) It is
checked -- the token (`401`), the body (at most 16 KiB, no unknown
fields, the record limits; `400`/`413`), that the fact exists (`404`)
and that `value_hash` is the fact's current hash (`409`: the data
changed while the reviewer looked) -- and appended to
`OPENPARTS_REVIEW_DIR/submissions.jsonl`. The status endpoints show
such submissions as pending. Without the token the endpoint answers
`403` and the site is read-only. The reviewer name is what the
submitter typed: anyone holding the token is trusted, and the commit
is the real gate. To publish them:

```sh
openparts reviews export --data ../openparts-data --from "$OPENPARTS_REVIEW_DIR"
# review the diff of reviews/, validate, commit or open a PR
```

Export is idempotent; submissions of a value that changed since are
left out (reported as stale). The log may be rotated after an export.

## Source documents (opt-in)

With `OPENPARTS_SOURCE_CACHE` set (a directory), `GET
/v1/sources/{id}/document` downloads the PDF the Source's `url` names
and keeps it there, for the site's datasheet viewer (manufacturer
sites don't allow cross-origin reads). Only `url`s of loaded Sources
are fetched; downloads use the user agent `openparts-server/<version>
(datasheet cache; +https://github.com/openparts-org/openparts)`, a
timeout (`OPENPARTS_SOURCE_TIMEOUT_SECS`, 30) and a size limit
(`OPENPARTS_SOURCE_MAX_MB`, 50), must be a PDF, and must match the
Source's `sha256` when it records one. A refusal (403, an HTML
challenge page) is a `502` with the reason, and the site opens the
original link instead. The cache is for viewing on this server, not
for redistribution.

## Self-hosting on a Raspberry Pi

`deploy/raspberrypi/` serves the API, the prebuilt artifact bundle and
the web site from one Pi behind Cloudflare Tunnel (no open ports, no
SolidCraft on the Pi): see its README. Relevant settings: `HOST`
(listen address, `127.0.0.1` behind a tunnel), `OPENPARTS_WEB_DIR` (an
openparts-web build served at `/`; the API is then also at `/api`).

## Deployment

`container/Containerfile` (Podman-first, OCI-compatible) and
`deploy/quadlet/openparts-server.container` (systemd-managed Podman
Quadlet unit) describe the intended production shape: the image has no
SolidCraft, and the Quadlet unit mounts the dataset and a bundle built
at release (`OPENPARTS_ARTIFACT_DIR`) read-only. Neither has been
exercised in this development environment (no container runtime
available here) -- treat a first real build/deploy as unverified until
run once.
