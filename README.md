# openparts

Core libraries, validators, generators, client, and CLI for OpenParts --
an open, provenance-tracked data platform for electronic components that
generates reproducible EDA/MCAD assets from Canonical Data instead of
distributing pre-made CAD files.

See [`docs/spec/`](docs/spec/) for the full specification set. Component
data lives in the sibling `openparts-data` repository; the distribution
API/database lives in `openparts-server`.

## Status

First Vertical Slice (Architecture Specification section 32):
Canonical YAML -> Parser -> Validator -> Effective Model -> KiCad + STEP.
Package support covers the LQFP/TQFP, QFN, SOIC, SOT, chip (2-terminal),
radial, axial, TO-92, DIP, SIP and BGA families.

3D models (STEP, STL, coloured 3MF, and an editable SolidCraft design)
are built by `openparts-solidcraft` and exported by a locally installed
[SolidCraft](https://solidcraft.io) -- see "3D models" below.

The CLI has `generate`, `validate`, `search`, `show` and `bundle` --
see "Prebuilt artifacts" below for `bundle`.

## Layout

```text
crates/
├── openparts-core        Canonical Rust data model
├── openparts-data         YAML <-> core model
├── openparts-validator     cross-entity validation
├── openparts-client        API client for openparts-server
├── openparts-cli           command-line interface
├── pcbcad/
│   ├── openparts-pcbcad    PCB CAD-independent Symbol/Footprint IR
│   ├── openparts-kicad      KiCad adapter
│   └── openparts-librepcb   LibrePCB adapter
└── mcad/
    ├── openparts-mcad       Mechanical Geometry generators
    ├── openparts-solidcraft Mechanical Geometry -> detailed parametric
    │                        SolidCraft model, run through SolidCraft
    ├── openparts-step       STEP exporter (via openparts-solidcraft)
    └── openparts-stl        binary STL exporter (via openparts-solidcraft)
```

## Usage

```sh
cargo build --workspace
cargo test --workspace

cargo run -p openparts-cli -- validate \
  --part   ../openparts-data/parts/exemplar/ex48/EX48F100Q6.yaml \
  --device ../openparts-data/devices/exemplar/EX48F100.yaml \
  --package ../openparts-data/packages/standards/lqfp/LQFP48-7x7-P0.5.yaml \
  --source ../openparts-data/sources/exemplar/EX-DS-0001.yaml

cargo run -p openparts-cli -- generate \
  --part   ../openparts-data/parts/exemplar/ex48/EX48F100Q6.yaml \
  --device ../openparts-data/devices/exemplar/EX48F100.yaml \
  --package ../openparts-data/packages/standards/lqfp/LQFP48-7x7-P0.5.yaml \
  --source ../openparts-data/sources/exemplar/EX-DS-0001.yaml \
  --out /tmp/openparts-out
```

`generate` writes the KiCad symbol/footprint, STEP, STL, 3MF, the
SolidCraft design (`.sc`), a LibrePCB library and a generation report
for one part.

## Prebuilt artifacts

`bundle` builds every part of an `openparts-data` checkout at once, for
shipping to `openparts-server` (which then needs no SolidCraft):

```sh
cargo run --release -p openparts-cli -- bundle \
  --data ../openparts-data --out /tmp/openparts-bundle [--jobs N] [--strict]
```

Each part is validated, its Effective Model built for the base silicon
revision, and its artifacts written content-addressed (Architecture
Specification section 24) -- SolidCraft runs once per part for all 3D
formats, parts in parallel (`--jobs`, default: CPU count):

```text
<out>/
├── manifest.json
└── artifacts/sha256/<hex>     STEP, binary STL, 3MF, .sc design,
                               .kicad_sym, .kicad_mod
```

`manifest.json` (`"format": "openparts-bundle"`, `"format_version": 1`):

- `artifact_path`: `"artifacts/sha256/{sha256}"`.
- `reproducibility`: dataset git commit (and whether it was dirty),
  `schema_versions`, generator crate versions and the openparts source
  commit, `toolchain` (SolidCraft and rustc versions),
  `silicon_revision` (`null`: base), and `not_byte_reproducible`
  (`["solidcraft"]`: SolidCraft's saved design keeps build timings;
  every other kind is byte-identical across rebuilds).
- `summary`: part counts (`ok`/`partial`/`failed`), distinct artifacts,
  total bytes.
- `parts`: sorted by (manufacturer, mpn), each with `part_id`,
  `device`, `package`, `status` (`ok`, `partial`, `failed`),
  `artifacts` (kind -> `{sha256, size, content_type, filename}`; kinds
  `step`, `stl`, `3mf`, `solidcraft`, `kicad-symbol`, `kicad-footprint`)
  and `errors` (`{stage, message}`, stage one of `resolve`,
  `validation`, `effective-model`, `geometry`, `3d-model`). A part that
  fails is listed with its reason, never dropped.
- `load_errors`: dataset files that could not be parsed at all.

The manifest has no time stamp, so rebuilding from the same dataset and
toolchain reproduces it. `--strict` exits non-zero unless every part was
bundled.

## 3D models

`openparts-mcad` stays the canonical geometry (where the body and each
lead are; the footprints read the same data). `openparts-solidcraft`
turns it -- per package family and device category -- into a SolidCraft
command script with what the real part looks like: drafted mold bodies,
formed gull-wing and through-hole leads, plated end caps, inlaid QFN
terminals, polarity bands and stripes, can sleeves, LED lenses, ...
Shared dimensions are SolidCraft design parameters (`body_w`, `pitch`,
`leads_per_side`, `lead_t`, ...) referenced by expressions, so the `.sc`
design `generate` writes can be edited in SolidCraft and rebuilds
consistently.

The shapes are YAML templates in
`crates/mcad/openparts-solidcraft/templates/` -- SolidCraft command
lists with `{{var}}` placeholders, one per building block (a gull-wing
lead row, a molded body, an MLCC, an electrolytic can, ...). The Rust
side only decides which templates to use and fills their variables from
the geometry; runs use SolidCraft's `--param-check strict`, so a
misspelt command parameter fails loudly.

SolidCraft must be installed: the Flatpak `io.tangencylabs.solidcraft`,
or `solidcraft-cli` on `PATH`; `OPENPARTS_SOLIDCRAFT_CLI` (a binary
path) or `OPENPARTS_SOLIDCRAFT_FLATPAK` (an app id) override the choice.
Tests that need it skip themselves when it is missing.

To review every model in the dataset (writes a script, STEP and PNG per
package):

```sh
cargo run -p openparts-solidcraft --example gallery -- ../openparts-data /tmp/gallery [filter]
```

`search`/`show` talk to an `openparts-server` instance (optional --
every other subcommand works entirely offline against local
`openparts-data` files). Point them at one with `--registry` or the
`OPENPARTS_REGISTRY_URL` env var (default `http://127.0.0.1:8080`):

```sh
# in a separate terminal, from openparts-server/ (OPENPARTS_ARTIFACT_DIR
# is optional: a `bundle` output to serve 3D models from):
OPENPARTS_DATA_DIR=../openparts-data OPENPARTS_ARTIFACT_DIR=/tmp/openparts-bundle \
  cargo run --release

# then:
cargo run -p openparts-cli -- search RP2040
cargo run -p openparts-cli -- show raspberrypi RP2040
```
