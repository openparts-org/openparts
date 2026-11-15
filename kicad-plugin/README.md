# openparts-kicad-plugin

A small native GUI companion app for [OpenParts](https://github.com/openparts-org/openparts),
with three tabs:

- **Search**: the registry's faceted catalog (`GET /v1/catalog`): text, the category tree,
  manufacturer, package family, size code, pins, pitch, mounting, the package structure (rows,
  gender, orientation, shroud, exposed pad), review state, body size ranges and the electrical
  parameter facets (values and min/max ranges such as `10n`..`1u`). The results table's columns
  follow the results: a capacitor search shows size, capacitance, voltage, dielectric; a
  connector search rows, gender, pitch. Pick a part and **Install into project**.
- **BOM check**: the open project's BOM against the registry (`POST /v1/bom/check`), with each
  line's status, issues and match. Select a line to see its candidates (the match, the listed
  alternatives, or **Find alternatives**: `GET .../alternatives`), each with **Install** and
  **Use MPN**.
- **Footprints**: every OpenParts footprint of the project (on its `.kicad_pcb` boards and in its
  project footprint libraries) whose `OpenParts placement` hash differs from the registry's
  current Placement Model (`GET .../placement`, see `docs/spec/placement-model.md` section 13).
  **Update** reinstalls the library footprint; board copies are then updated in KiCad with
  Tools > Update Footprints from Library.

It does **not** place anything on the schematic or board -- once a part is installed, pick it up
from KiCad's normal Symbol/Footprint picker as usual.

### Install into project

Installing (the same path as `openparts kicad-install`):

- downloads the part's KiCad symbol, KiCad footprint, and STEP artifacts (via
  `openparts-client`, which verifies each download's SHA-256 content hash before trusting it),
- writes the STEP file under `<project>/openparts_3d/<MPN>.step`,
- writes/merges the footprint into `<project>/openparts.pretty/<MPN>.kicad_mod`, patching its
  `(model ...)` block to point at the downloaded STEP file so the 3D viewer works immediately,
- merges the symbol into `<project>/openparts.kicad_sym` (creating it if needed, replacing an
  existing entry with the same name if reinstalling),
- registers both libraries in the project's `sym-lib-table`/`fp-lib-table` if not already
  present.

All of this is idempotent: installing the same part twice updates the existing entries in place
rather than duplicating them.

### How the BOM is read

The root schematic is `<name>.kicad_sch` of the project's `.kicad_pro`. The app walks the sheet
hierarchy (`Sheetfile`), reads every placed symbol's Reference (per sheet instance), Value,
Footprint, MPN (`MPN`, `Manufacturer Part Number`, ...), Manufacturer and DNP fields, skips power
symbols and `in_bom no`, merges the units of multi-unit symbols, and groups identical lines into a
KiCad-style BOM CSV. With the source set to "kicad-cli / schematic" and `kicad-cli` on `PATH` (or
`KICAD_CLI`), kicad-cli's netlist is sent instead; the symbol locations still come from the
schematic.

**Use MPN** writes the MPN (and the manufacturer, where the symbol has that field) into the
symbols' existing MPN field, rewriting only that string in the `.kicad_sch`. It does so only when
every symbol of the line has an MPN field on every unit and sits on a sheet used once; otherwise
it copies the MPN to the clipboard and says why. Reload an open schematic afterwards
(File > Revert).

### Command line

`openparts kicad-check-stale <project>` is the Footprints tab's check (`--all`, `--json`,
`--strict`).

> **Verified end-to-end** on a real Windows machine with KiCad 10: `install.ps1` downloading and
> installing the binary + launcher, the "OpenParts" toolbar button in the PCB editor, searching,
> installing RP2040, the symbol/footprint libraries being correctly registered as project-specific
> libraries, and the footprint being placed on a real board.
>
> One thing to know: after installing a part for the first time in a KiCad session, **restart the
> Symbol Editor / Footprint Editor / 3D viewer windows** (close and reopen) before the new
> `openparts` library shows up in them. KiCad reads library tables once when each of those windows
> opens and doesn't hot-reload them if the underlying `sym-lib-table`/`fp-lib-table` changes while
> they're already open -- this is KiCad's own behavior, not something this tool can avoid.

## Quick install

Downloads a prebuilt binary and registers the [KiCad toolbar launcher](#launching-from-inside-kicad)
for you -- no Rust toolchain needed.

**Linux:**
```sh
curl -fsSL https://raw.githubusercontent.com/openparts-org/openparts/main/kicad-plugin/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/openparts-org/openparts/main/kicad-plugin/install.ps1 | iex
```
Both scripts try to auto-detect your KiCad plugin directory; if they can't, they'll tell you how to
find it via KiCad's own Tools > External Plugins > Open Plugin Directory and re-run with an explicit
path (`-PluginDir` on Windows, `--plugin-dir` on Linux). Afterwards, restart KiCad (or Tools >
External Plugins > Refresh Plugins) and look for the new "OpenParts" button in the PCB editor
toolbar -- **not** in the Plugin and Content Manager's own "Installed" list, which only tracks
packages it installed itself and never shows an unpackaged Action Plugin like this one.

macOS isn't built by CI yet -- see [Manual install / building from source](#manual-install--building-from-source).

## Manual install / building from source

```sh
cargo build --release
```

This repo depends on `openparts-client`/`openparts-core` from a sibling checkout of
[`openparts`](https://github.com/openparts-org/openparts) via relative path
(`../openparts/crates/...`) -- clone both repos next to each other:

```
OpenParts/
├── openparts/
└── openparts-kicad-plugin/
```

## Running

```sh
cargo run --release
```

Enter your `openparts-server` URL (defaults to `$OPENPARTS_REGISTRY_URL`, else
`http://localhost:8080`) and the project directory. The project directory can also be pre-filled
via `--project-dir <path>` on the command line, which is how the KiCad launcher below invokes it.

## Launching from inside KiCad

`kicad-integration/openparts_launcher.py` is a pcbnew Action Plugin: it adds an "OpenParts" button
to the PCB Editor's toolbar that runs this app as an external process with `--project-dir` already
set to the currently open project's directory. It does not use KiCad's IPC API (which has no
library-management support to begin with) -- it's the same "Python toolbar button that shells out
to an external tool" pattern used by plugins like InteractiveHtmlBom.

The [Quick install](#quick-install) scripts set this up automatically, including writing a config
file the launcher reads to find the binary. If you're building from source instead, install it
manually:

```sh
mkdir -p ~/.local/share/kicad/9.0/scripting/plugins
ln -s "$(pwd)/kicad-integration/openparts_launcher.py" \
    ~/.local/share/kicad/9.0/scripting/plugins/openparts_launcher.py
```

Adjust `9.0` to your installed KiCad version (Help > About KiCad; Tools > External Plugins > Open
Plugin Directory is the authoritative way to find this folder on any OS/version), then restart
KiCad or use Tools > External Plugins > Refresh Plugins. The launcher looks for the binary in this
order: the `OPENPARTS_KICAD_PLUGIN_BIN` environment variable, then the installer's config file
(`~/.config/openparts-kicad-plugin/binary_path.txt`, or `%APPDATA%\openparts-kicad-plugin\binary_path.txt`
on Windows), then the `DEFAULT_BINARY_PATH` constant at the top of the script -- edit that constant
if your manual checkout lives somewhere else and you don't want to set an environment variable or
config file.

## Project-path auto-detect (alternative to the launcher above)

Optional feature `kicad-ipc`, off by default:

```sh
cargo build --features kicad-ipc
```

When enabled, the "Detect via KiCad" button uses [`kicad-ipc-rs`](https://docs.rs/kicad-ipc-rs) to
ask a running KiCad instance (10.0.1+, with its IPC API enabled in Preferences) for its current
project path. This is best-effort and never required -- the project directory can always be typed
in by hand, or pre-filled via the launcher plugin above (which needs neither this feature nor
`cmake`). It's off by default because `kicad-ipc-rs` pulls in `nng-sys`, which compiles the `nng`
C library from source via `cmake`; that's a real system dependency not everyone building this app
needs.

## Testing

```sh
cargo test
```

Everything but drawing is tested offline: the search state and its columns (`src/search.rs`),
schematic reading, BOM grouping and MPN writing against a trimmed copy of a Raspberry Pi Pico
board (`tests/fixtures/pico`, `src/schematic.rs`, `src/bom.rs`), the stale scan
(`src/stale.rs`, `openparts-client`'s `kicad_stale`), and installing from a pre-filled client
cache (`src/install.rs`). `src/ui.rs` only draws. With a server running,
`OPENPARTS_TEST_REGISTRY=http://127.0.0.1:8080 cargo test -- --ignored --nocapture` checks the
fixture's BOM against it.

`src/kicad_detect.rs`'s real IPC path (behind the `kicad-ipc` feature) could not be exercised in
the environment this was developed in (no `cmake` available to build `nng-sys`, no running KiCad
instance). Its two API calls (`KiCadClientBlocking::connect()`, `.get_current_project_path()`) are
copied verbatim from `kicad-ipc-rs`'s own documented usage, not guessed -- but this path has only
been verified inside real KiCad by the OpenParts maintainer, not by an automated test here.
