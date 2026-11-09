---
name: add-part
description: Add electronic parts (Part + Device + Package + Source) to openparts-data from manufacturer datasheets, with provenance, and verify they validate and generate KiCad footprints and 3D models. Use when asked to add, ingest or bulk-create parts in openparts-data.
---

# Adding parts to openparts-data

Every value you write is a **candidate change** for human review (Agent
Ingestion Specification): it must come from a primary source you actually
read, with a page and locator. Read these first, once:
`docs/spec/canonical-data.md` (sections 3-27, 34-35) and
`docs/spec/agent-ingestion.md`.

## 0. Rules that are never bent

- **Unknown is better than invented.** If the datasheet doesn't state a
  value, leave the field out. Never estimate, average, or copy from a
  "similar" part. Ranges given only as min/max are recorded as min/max --
  never invent a nominal.
- **Cite what you read.** The `source` in provenance is the exact
  document *and revision* you opened. If the manufacturer now serves a
  newer revision than an existing Source file, add a new Source
  (`id: <mfr>/<doc>@rev<N>`, file `sources/<mfr>/<doc>-rev<N>.yaml`)
  instead of citing the old one with new page numbers.
- **Never get around access controls.** If a site returns 403/"Access
  Denied", a bot check, a login wall or a TLS error, do not spoof user
  agents, use mirrors of dubious origin, disable certificate checks or
  retry tricks. Try the manufacturer's other official URL (product page,
  alternate PDF link, regional official site you are sure is
  first-party); otherwise record the part as unverified in your report
  and move on.
- **Package `construction`** (molded, tantalum_molded, glass, ceramic,
  metal_can) only when the source text says it -- not from the package
  name or a photo.
- Don't edit Rust code or other agents' files. Data only, in your
  assigned manufacturers/folders.

## 1. Repository layout (openparts-data)

```
manufacturers/<mfr>.yaml                 one per manufacturer (shared!)
sources/<mfr>/<doc>.yaml                 one per document revision
devices/<mfr>/<DEVICE>.yaml              logical device: pins, category
packages/standards/<family>/<ID>.yaml    physical package, reusable
parts/<mfr>/<series>/<MPN>.yaml          orderable MPN -> device + package
```

Copy the shape of an existing file of the same kind (e.g.
`parts/onsemi/2n7002et1g/2N7002ET1G.yaml`, `devices/onsemi/2N7002E.yaml`,
`sources/onsemi/2N7002E-D.yaml`, `packages/standards/lqfp/LQFP48-7x7-P0.5.yaml`).
Put a short comment header on every new file saying what you verified
where (existing files show the style). Files are UTF-8, LF, simple YAML.

- **Manufacturer files are shared.** Before creating one, check
  `manufacturers/`. Create only manufacturers assigned to you.
- **Device vs Part.** Several MPNs (reel size, grade, temperature) share
  one Device and one Package. Prefer adding the device once and one or
  more MPNs from the ordering table.
- `category` is one of: microcontroller, capacitor, resistor, inductor,
  diode, transistor, led, crystal, fuse, connector, other (it changes how
  the 3D model is drawn).
- `existence.status: verified` needs the ordering table (page/locator).
  `lifecycle` only if a first-party page states it; say where.

## 2. Packages: reuse first

Search before creating: `grep -rl "family: soic" packages/` and compare
`pitch`, `lead_count`, body size. Reuse a package only if its dimensions
match the new datasheet's outline drawing (same JEDEC outline / same
numbers). A package created for one part should still be named
generically (`standards/SOIC8-3.9x4.9-P1.27`) if it is a standard outline.

Supported `family` values and what the generators assume:

| family | geometry convention | notes |
|---|---|---|
| `lqfp` (also TQFP) | 4 sides, pin 1 top of the left edge, counter-clockwise | body_width = E1 (X), body_length = D1 (Y) |
| `qfn` | 4 sides flush terminals, optional `exposed_pad` | EP becomes a pad named "EP" |
| `soic` | left/right rows, pin 1 top-left | body_width = E1 (across the rows) |
| `sot` | needs `lead_layout: [bottom, top]` (SOT-23-3: `[2, 1]`, SOT-23-5: `[3, 2]`) | leads exit across `body_length` |
| `chip` | 2 terminals along X; `body_length` is the long axis | resistor/MLCC/LED/diode/crystal/fuse by category |
| `radial` | can of diameter `body_width`, pin 1 at -X | electrolytic: pin 1 = +; LED: pin 1 = cathode |
| `axial` | horizontal can, pin 1 = cathode at -X | `body_height` = body length |
| `to92` | 3 leads, middle staggered | |
| `dip` | `body_width` is the **row spacing** (7.62 for 300 mil) | the molding is drawn inside the rows |
| `sip` | single row | |
| `bga` | `lead_layout: [rows, cols]`, full grid, A1 top-left | |

Anything else (TSSOP, SOT-223, DPAK, 4-pad crystals, WS2812, axial
resistors ...) is **not supported yet** -- don't force it into another
family; report it.

Dimensions (`docs/spec/canonical-data.md` 20.1), all `{nominal?, min?,
max?, unit: mm}` with their own provenance pointer
(`/dimensions/lead_width` ...):
`body_width`, `body_length`, `body_height` (A2 body thickness),
`overall_width`/`overall_length` (lead span E/D, HE), `overall_height`
(A), `standoff` (A1), `lead_width` (b), `lead_length` (L),
`lead_thickness` (c, A3 on QFN), `exposed_pad`. The validator rejects a
span not larger than the body and a body/standoff not smaller than the
overall height -- if `body_height` already holds A, don't also add
`overall_height`.

## 3. Reading datasheets

- Open the PDF (WebFetch). For tables and drawings that text extraction
  garbles, render the page (e.g. `python3 -c` with pypdfium2/pypdf if
  available) and read the image; cross-check pinout diagrams against the
  package outline's "STYLE" list or pin table.
- Package outline pages often carry several variants (5x5 vs 6x6 QFN,
  different lead counts). Confirm the drawing's title matches *your*
  package before copying numbers.
- Page numbers are the PDF page index you read, not the printed page if
  they differ -- say which in the locator when ambiguous.

## 4. Verify

From `openparts/`:

```sh
# one part
cargo run -q -p openparts-cli -- validate --part <part> --device <device> \
  --package <package> --source <source.yaml> [--source ...]
# whole dataset (all parts, all references)
../openparts-data/scripts/validate.sh
# footprint/pad sanity across every package (no overlaps, toe at span/2)
cargo test -q -p openparts --test vertical_slice
# 3D model + PNG of your parts (filter = part of the MPN)
cargo run -q -p openparts-solidcraft --example gallery -- ../openparts-data <out-dir> <filter>
```

Look at the PNG of every new package (Read tool): leads on the right
sides, pin-1 mark where pin 1 is, body proportions plausible. A part is
done only when validate, the vertical_slice test and its gallery build
all pass.

## 5. Report

List per part: files added, values verified (source/page/locator),
values left out and why, anything unusual. Keep notes of what slowed you
down or surprised you -- they go into the Learnings below.

## Learnings

(Collected from past ingestion runs; newest last.)

- onsemi.com may refuse automated downloads; do not work around it.
  KEMET's content site has served an expired TLS certificate -- treat as
  unavailable, don't bypass.
- ST serves only the latest datasheet revision; table and page numbers
  move between revisions (DS5319 Rev 18 Table 61 p101 -> Rev 20 Table 62
  p99).
- Espressif datasheets show several QFN variants in one chapter (QFN48
  5x5 vs 6x6); the wrong figure is an easy mistake.
- A datasheet that never says "glass" (1N4148, Vishay 81857) does not
  justify `construction: glass`, however obvious it looks.
