# PCBCraft Export

Version 0.1 Draft. Implemented by `crates/pcbcad/openparts-pcbcraft` (a
pure adapter over the PCB CAD IR, like openparts-kicad and
openparts-librepcb) and run as `openparts pcbcraft`.

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
pins per side around a 20.32 mm body, each pin keeping its side and
order (at most 32 units, 1,024 terminals). Pins the IR stacks at one
position (same name: RP2040's six IOVDD pins) become one terminal,
numbered by the visible pin.

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
base `--locator-base`, else `--registry`).

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
