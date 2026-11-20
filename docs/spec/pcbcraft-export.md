# PCBCraft Export

Version 0.1 Draft. Implemented by `crates/pcbcad/openparts-pcbcraft` (a
pure adapter over the PCB CAD IR, like openparts-kicad and
openparts-librepcb) and run as `openparts pcbcraft`; openparts-server
serves the same records as a parts catalog (section 6).

[PCBCraft](https://github.com/tangency-labs/pcbcraft) reads a *native
library*: a folder with `library.json` and flat `symbols/`,
`footprints/`, `categories/`, `parts/` and `manufacturer-parts/`
directories of JSON records, or the same documents in one
`pcbcraft-library-bundle/1` file. Every record is addressed by (library,
kind, id, revision); a revision is immutable, and two copies of one
address with different content are both quarantined. The export follows
PCBCraft's published JSON Schemas (`docs/schemas/*.schema.json`) and the
rules of its part, symbol and footprint schema documents; its output is
checked against PCBCraft's own `scripts/check-library-schemas.py` and
`check-footprint-schemas.py`.

---

## 1. Mapping

| OpenParts | PCBCraft record |
| --- | --- |
| Device | symbol `pcbcraft-symbol/1` (one unit) or `/4` (several), and generic part `pcbcraft-part/2` |
| Package | footprint `pcbcraft-footprint/1` (`/4` with non-plated holes); one package variant of each generic part that uses it |
| Part (MPN) | manufacturer part `pcbcraft-manufacturer-part/1`: manufacturer, MPN, generic part, package variant, datasheet, typed specifications |
| ComponentCategory | category `pcbcraft-category/1` |

**Symbol.** The IR symbol (`openparts_pcbcad::build_symbol`), Y flipped
(PCBCraft points Y down), in nanometres. PCBCraft pins sit on the
1.27 mm grid at most 12.7 mm from the centre, so a symbol whose pins do
not fit (RP2040: 70 mm tall) is laid out again into units of at most 8
pins per side, each around the smallest body (at most 20.32 mm square)
that keeps its labels apart (at most 32 units, 1,024 terminals),
grouped as the KiCad symbol is: the same pin families
(`openparts_pcbcad::PinGroups`, shared with `build_symbol`: power,
ground, the bus and the other signal families by name prefix, each
family in natural name order, `GPIO2` before `GPIO10`).

- Everything goes in one unit when it fits (8 per side and the labels
  apart, power on top, ground at the bottom, signals on the left then
  the right).
- Otherwise the first unit holds power (top), ground (bottom) and the
  non-bus signal families (left, then right); families that do not fit
  move on to the next unit, related neighbours (same first two
  characters: `MTCK`/`MTDO`, `SPICS`/`SPIHD`) together.
- The buses -- the IR's bus, every family of 8 or more and the ports of
  the same series (`PC` next to `PA`/`PB`, `P5.` next to `P1.`), sorted
  by name -- follow in units of their own, never in the first one.
- A family stays on one side; one longer than a side fills an empty
  unit's left then right side; one longer than a unit (16) is cut in
  order into as few pieces as it takes. Families sharing a side are one
  empty slot apart when there is room.
- A symbol with no families (a connector: every pin named by its
  number) keeps pin number order, 8 per side.

RP2040 becomes power + GND + TESTEN/XIN/XOUT/SWCLK/SWD/RUN/USB, then
QSPI, then GPIO0-15, then GPIO16-29; STM32F103C8 power + ground +
OSC/NRST/BOOT0, then PA, PB and PC. Only drawing positions and units
change: the terminals and their pads are the same in any layout. Pins
the IR stacks at one position (same name: RP2040's six IOVDD pins)
become one terminal, numbered by the visible pin.

*Labels.* PCBCraft draws each pin's number and name itself
(`PinAppearance::graphics`), always upright, in its stroke font
(PCBcraft Technical, about 0.83 x the height per capital); names and
numbers are 1 mm high (PCBCraft's own name height, and the 1 mm per
character the KiCad layout reserves). An upright name is centred on a
top or bottom pin, so names 2.54 mm apart run into each other. Those
pins' names are hidden (`show_name: false`) and drawn instead as
`graphic` text in the unit's drawing (`rotation` 270: reading bottom to
top, as KiCad draws them), 0.4 mm inside the body edge: hanging from the
top edge (`align: end`), rising from the bottom one (`align: start`).
Graphic text holds 128 characters (the plain `text` shape: 16, no
rotation), more than any pin name (32), so no name is shortened.

Every unit is checked with PCBCraft's label geometry and the font's own
glyph metrics (derived from its `strokes.json`): no label may touch
another label, the body outline or another pin's lead, and names of
different pins keep 1 mm apart in a re-laid-out unit (0.25 mm in the
IR's drawing; numbers 0.25 mm). The IR's drawing is kept with turned
names if that is clean, else with upright ones (KiCad sized it for
upright names: LM1117's 5 mm body), else the symbol is laid out again.
There, rows and columns sit 2.54 mm apart on the odd 1.27 mm steps
(facing rows line up), as near the middle of their side as the labels
allow: the left and right names keep clear of the turned names in the
body's corners. A pin number centred over a lead longer than the lead
(a module's `SWCLK`, a connector's `SHELL`) gets the unit longer leads,
in 1.27 mm steps, and a smaller body. A name too long for any body
(onsemi MC74HC165A's `SERIAL_SHIFT/PARALLEL_LOAD`, 26 characters) is
lettered smaller, down to 0.5 mm. Across the dataset no exported unit
has an overlapping label.

**Footprint.** The package's Placement Model at density level N, built
without a device: pads (mm -> integer nm, Y flipped, rotation 0),
non-plated round holes, and the fabrication, silkscreen and courtyard
drawings as `artwork` (an outline over 64 points becomes consecutive
open polylines). When the device changes the footprint (shared contacts
add overlaid pads; polarity marks), that part gets a device-specific
footprint (`<package>-<device>`) instead. A pad number the IR uses for
several lands keeps it on the first; the others become `<n>_2`,
`<n>_3`... (PCBCraft pad numbers are distinct).

**Package map.** Each terminal maps to the pads of every pin it stands
for (a stacked GND terminal to all its GND pins and the exposed pad);
pads no pin names are `unconnected_pads`. A pin with no pad refuses the
part.

**Specifications and categories.** Each parameter key's primary value
(measurement conditions are not carried), one field per stated bound:
`openparts:<key>` for `typ`, `openparts:<key>-min` / `-max` for the
limits, and a deviation's symmetric magnitude (±1 % -> `0.01`). PCBCraft's
own field ids are used where the meaning is the same
(`pcbcraft:resistance`, `pcbcraft:tolerance`,
`pcbcraft:supply-voltage-min`/`-max`, `pcbcraft:rated-current`). A
value is written only when it is exactly one canonical base-unit decimal
(Ohm, F, H, V, A, W, Hz, s, m, K, ratio `1`; temperatures °C + 273.15
-> K), converted by decimal digit arithmetic, never by binary
floating-point scaling. Quantities PCBCraft has no unit for (1/K, dB,
bit, cd...), disjoint ranges, multi-valued keys and asymmetric
deviations are left out: unknown is never zero or a guess. Enumerated
keys are `choice` fields with the vocabulary's spellings. One category
per ComponentCategory, holding the fields its parts state (at most 64,
the most used first); PCBCraft categories are flat, so the category
tree's parents are not represented. Generic parts carry no
`fixed_parameters`: every value lives on the manufacturer part.

## 2. Refusals

What PCBCraft cannot represent is not approximated: the part is left out
and reported (`refused <part>: <reason>`):

- chamfered, polygonal and annular pads (PCBCraft pads are rectangle,
  rounded rectangle, circle and oval);
- slotted holes, and a drill not smaller than its land;
- bottom-side surface pads (surface copper is component side only);
- more than 1,024 pads or terminals, 128 non-plated holes, 256 drawing
  objects, or a symbol needing more than 32 units;
- pin and pad numbers that are not 1-8 letters, digits, `+`, `-`, `_`,
  and pin names longer than 32 characters;
- a pin with no pad in its package.

Against openparts-data at `b0b8016`: 3,850 parts written, 60 refused
(51 slotted holes, 2 annular pads, 1 polygon pad, 5 long pin names, 1
package without geometry).

## 3. Identity, revisions and the ledger

Record ids are slugs of the OpenParts ids (`raspberrypi/RP2040` ->
`raspberrypi-rp2040`); a slug that is too long or not one key's alone
gets a hash of the key (`ti-lm358-1a2b3c4d`). Without a ledger every
record is revision 1.

`--ledger <file>` (`openparts-pcbcraft-ledger/1`, read if present, then
updated) keeps, per library and `<kind>/<id>`, the OpenParts key and
every published revision with the SHA-256 of its canonical document
(revision and source reference blanked). An export reuses a revision
whose digest matches -- with that revision's own source reference, so
the bytes are identical -- and otherwise allocates the next one; ids
once assigned stay assigned. Records are written as fixed-field-order
structs, two-space JSON and a final newline, so digests are stable.

## 4. Licence and provenance

Every record declares the licence given by `--license` (default
`CC0-1.0`, Architecture Specification section 30). Records carry the
source triple `kind: "openparts"`, `reference: "openparts-data <git
revision>; openparts-pcbcraft <version>"` and `locator`: the registry URL
of the part, device or package (`<base>/v1/parts/<manufacturer>/<mpn>`;
base `--locator-base`, default `https://openparts.jijinbei.jp`, the
public OpenParts site).

## 5. Command

```sh
openparts pcbcraft --data ../openparts-data --out lib/ [--license CC0-1.0] \
    [--ledger pcbcraft-ledger.json] [--group-by category-group] [parts...]
openparts pcbcraft --data ../openparts-data --bundle openparts.pclib.json raspberrypi/RP2040
```

PCBCraft reads at most 8,192 files, 1 MiB per file and 32 MiB per
library source (a bundle at most 16 MiB). The whole dataset is about
9,300 files, so it is split with `--group-by category-group`: one
library per top-level category (`openparts-capacitor`, `openparts-power`... as
`<library-id>-<category>`), each in `<out>/<library-id>/` (or
`<bundle-dir>/<library-id>.pclib.json`). An export over the limits fails
and says so. Existing output is never overwritten.

3D models are not exported: PCBCraft footprints have no 3D model field.

## 6. Catalog API

openparts-server serves the same records as an external parts catalog
for PCBCraft (the provider boundary of
[tangency-labs/pcbcraft#94](https://github.com/tangency-labs/pcbcraft/issues/94):
search, details, and on-demand resolution into pinned native records).
Every route is read-only.

| Route | Answer |
| --- | --- |
| `GET /v1/pcbcraft` | status: `state` (`building`, `ready`, `failed` + `error`), `persistent_revisions`, `license`, library/part/refusal counts |
| `GET /v1/pcbcraft/search?q=&category=&manufacturer=&exportable=&offset=&limit=` | `{total, offset, limit, persistent_revisions, results}`; a page is complete only when `offset + results.length == total`. `limit` 1-200 (default 50); `exportable=true/false` keeps only exportable or only refused parts; `category` takes a group and its subcategories |
| `GET /v1/pcbcraft/parts/{manufacturer}/{mpn}` | `{summary, dependencies, license, persistent_revisions}`: `dependencies` is the address of every record the part's bundle holds |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts/pcbcraft` | the part as a `pcbcraft-library-bundle/1`; `422` `{error, reason}` when refused |
| `GET /v1/pcbcraft/libraries` | the libraries with their part, refusal and document counts, bundle size and SHA-256 |
| `GET /v1/pcbcraft/libraries/{id}` | one whole library as a `pcbcraft-library-bundle/1` |

A summary carries `manufacturer` (the OpenParts id used in the URLs),
`manufacturer_name`, `mpn`, `category`, a one-line `description`, the
`library`, the addresses of its `manufacturer_part` and generic `part`
(`{library, kind, id, revision, sha256}`, the SHA-256 of the document
text as served), `refused`/`refusal`, and the `bundle` URL.

**Libraries and addresses.** The server exports the whole dataset once,
in the background after startup, exactly as
`openparts pcbcraft --group-by category-group` does (both use
`openparts_artifacts::pcbcraft`): one library per top-level category,
`openparts-<group>`. A part's bundle is the subset of its library that
the part needs -- `library.json`, the manufacturer part, its generic part,
the generic part's symbol and category, and the footprint of every
package variant of the generic part (other MPNs of the same device may
add variants) -- with each document byte for byte as in the whole
library. A part fetched alone, a library fetched whole and a CLI export
with the same ledger therefore agree on every address and body. Until
the export is done (about 1.3 s for the whole dataset on a PC; several
seconds on a Raspberry Pi 4) the PCBCraft routes answer `503`.

**Revisions.** A served address never has two bodies:

- With `OPENPARTS_PCBCRAFT_LEDGER=<file>` the ledger (section 3) is read
  at startup; content whose digest it knows keeps that revision (with
  that revision's own source reference, so the bytes are identical),
  changed content gets the next revision, and the file is replaced
  atomically (temporary file, `fsync`, rename) before anything is
  served. If the ledger cannot be read or written, the PCBCraft routes
  answer `503` with the reason: an unreadable ledger is never replaced,
  and no revision is served that would not survive a restart.
- Without it the ledger lives in memory: every record is revision 1 on
  each start, `persistent_revisions` is `false`, and bundles carry
  `X-PCBCraft-Revisions: ephemeral`. Only for development.
- A change to a dependency changes its dependents too: a new symbol
  revision gives the generic part (whose reference names it) a new
  revision, and so its manufacturer parts. Content that returns to an
  earlier state gets that earlier revision back. One server per ledger
  file.

Bundles carry `Content-Disposition: attachment;
filename="<manufacturer>-<mpn>.pcbcraft.json"` (`<id>.pcbcraft.json`
for a library), `X-Content-Hash: sha256:<hex>` (also the `ETag`) and
`X-PCBCraft-Revisions: persistent|ephemeral`. They are built from the
catalog, never from the content-addressed artifact bundle of `openparts
bundle`: their revisions come from the server's ledger. Every library
is within PCBCraft's limits today (the largest, `openparts-connector`,
is 5.9 MB in 1,504 documents); one that is not is listed with
`limit_error` and no `bundle` URL, and is exported as a folder with the
CLI instead (section 5). Record locators use `OPENPARTS_PUBLIC_URL`
(default `https://openparts.jijinbei.jp`) and the source reference names
`OPENPARTS_DATA_REVISION` (default: the data checkout's git HEAD).

```sh
curl -s 'https://openparts.jijinbei.jp/api/v1/pcbcraft/search?q=RP2040&limit=1'
# {"total":12,"offset":0,"limit":1,"persistent_revisions":true,"results":[{
#   "manufacturer":"raspberrypi","manufacturer_name":"Raspberry Pi Ltd",
#   "mpn":"RP2040","category":"microcontroller",
#   "description":"Microcontroller raspberrypi/RP2040 in standards/QFN56-RP2040",
#   "library":"openparts-microcontroller",
#   "manufacturer_part":{"library":"openparts-microcontroller",
#     "kind":"manufacturer-part","id":"raspberrypi-rp2040","revision":1,"sha256":"53c8..."},
#   "part":{"library":"openparts-microcontroller","kind":"part",
#     "id":"raspberrypi-rp2040","revision":1,"sha256":"9bd8..."},
#   "refused":false,"refusal":null,
#   "bundle":"/v1/parts/raspberrypi/RP2040/artifacts/pcbcraft"}]}
curl -sOJ https://openparts.jijinbei.jp/api/v1/parts/raspberrypi/RP2040/artifacts/pcbcraft
# raspberrypi-RP2040.pcbcraft.json: library.json + 5 records
curl -s https://openparts.jijinbei.jp/api/v1/parts/bourns/PTV09A-2015F-B103/artifacts/pcbcraft
# 422 {"error":"bourns/PTV09A-2015F-B103 cannot be exported to PCBCraft: ...",
#      "reason":"footprint: pad MP has a slotted hole (PCBCraft has round holes only)"}
```

The site's part page offers the bundle as "Download PCBCraft library"
next to the KiCad and STL downloads, and shows the refusal reason
instead when the part cannot be exported.
