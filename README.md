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
Package support currently covers LQFP, QFN, chip (2-terminal), SOIC,
and SOT families; other families (TSSOP, BGA, ...) are not implemented
yet.

The CLI's `generate`/`validate`/`search`/`show` subcommands are
implemented; `provenance`, `diff`, `install`, `update`, and `bundle`
are still stubs (Architecture Specification section 7.5 defers their
exact semantics to implementation time, and none has been designed
yet).

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
│   ├── openparts-librepcb   LibrePCB adapter
│   └── openparts-altium     (not implemented -- proprietary binary
│                             format, no Altium install to verify
│                             against; see the crate's doc comment for
│                             the planned DelphiScript-emission approach)
└── mcad/
    ├── openparts-mcad      Mechanical Geometry generators
    ├── openparts-step       STEP exporter
    ├── openparts-stl        STL exporter
    └── openparts-gltf       (not implemented for the first vertical
                              slice -- see the crate's doc comment)
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

`search`/`show` talk to an `openparts-server` instance (optional --
every other subcommand works entirely offline against local
`openparts-data` files). Point them at one with `--registry` or the
`OPENPARTS_REGISTRY_URL` env var (default `http://127.0.0.1:8080`):

```sh
# in a separate terminal, from openparts-server/:
OPENPARTS_DATA_DIR=../openparts-data cargo run --release

# then:
cargo run -p openparts-cli -- search RP2040
cargo run -p openparts-cli -- show raspberrypi RP2040
```
