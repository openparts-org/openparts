# OpenParts Placement Model

Version 1 (schema `frame.schema_version: "1"`). Implemented by
`crates/pcbcad/openparts-placement`, served by
`GET /v1/parts/{manufacturer}/{mpn}/placement`.

## 1. Purpose

The Placement Model is everything a board tool needs to put one part on a
board: the frame its coordinates are in, the pads, the courtyard, the body
above the board, pin 1, whether it must sit at the board edge, the areas it
needs kept clear, how far its leads go through the board, how its 3D model
lines up, and where each of these numbers came from.

It is built from the part's Mechanical Geometry (`openparts_mcad::generate`),
the same geometry its 3D model is built from, and it is what the KiCad
footprint, the LibrePCB footprint, the SVG preview, the BOM land-pattern
comparison and the board tool (`tools/kicad-board`) all read. Nothing
downstream recomputes pads, courtyards or transforms on its own.

References:

- IPC-7351B, *Generic Requirements for Surface Mount Design and Land Pattern
  Standard*: land calculation (toe/heel/side fillet goals with tolerance
  stacking), the three density levels L/N/M, courtyard excess, the land-pattern
  centroid origin and the "Level A" zero component orientation.
- IPC-2581C, *Generic Requirements for Printed Board Assembly Products
  Manufacturing Description Data*: a right-handed board coordinate system
  viewed from the top (X right, Y up), counter-clockwise rotation, and
  bottom-side components described as mirrored packages.
- IPC-7251 (through-hole land patterns) for the through-hole courtyard, which
  IPC-7351B does not cover.
- ISO 10303 (STEP): the 3D model frame, Z up.

## 2. Frame

### 2.1 Axes and units

- Units: millimetres.
- Right-handed. Viewed from the top: +X right, +Y up; +Z up from the board's
  top surface (the seating plane is z = 0).
- Rotations are counter-clockwise viewed from the top, in degrees.

These are the IPC-2581 and STEP conventions. Tool adapters convert: KiCad's
board and footprint frames are Y-down (section 2.4).

### 2.2 Origin

The origin is the land-pattern centroid (IPC-7351B): the centre of the extents
of the plated lands (non-plated holes do not count). For a symmetric package
this is the body centre. For an asymmetric one it is not: a TO-252 sits off
centre towards its tab; a USB-C receptacle's origin lies between its contact
row and its front shell tabs.

### 2.3 Placing a part: `board = T · R(θ) · M`

A part placed at (x, y), rotated θ, on side s maps a part-frame point p to

```text
board = T · R(θ) · M · p
T = translate(x, y)
R(θ) = [[cos θ, −sin θ], [sin θ, cos θ]]    (counter-clockwise, viewed from the top)
M = diag(−1, 1) on the bottom side, I on the top side
```

So a bottom-side part is first mirrored x → −x in its own frame (it is seen
through the board), then rotated and moved. θ is still measured
counter-clockwise viewed from the top. A pad's own rotation φ becomes
θ + φ on the top and θ − φ on the bottom. `openparts_placement::Placement`
implements this (`apply`, `invert`, `apply_angle`).

### 2.4 KiCad

KiCad's board frame is Y-down. A footprint's orientation is counter-clockwise
as seen on screen, i.e. viewed from the top. A `.kicad_mod` stores the model's
pads at (x, −y).

KiCad flips a footprint to the back (`FOOTPRINT::Flip`) by mirroring its pads
in its own Y-down frame (local (fx, fy) → (fx, −fy)) and keeping an orientation
φ. A back-side pad at local f then lands at `C + Rd(φ)·(fx, −fy)`. Here `Rd` is
the on-screen counter-clockwise rotation in Y-down coordinates. Written in the
model's Y-up terms, that is `R(φ)·diag(1, −1)·p`. Since

```text
R(θ)·M = R(θ)·diag(−1, 1) = R(θ + 180°)·diag(1, −1)
```

the model's placement corresponds to KiCad's as follows:

| Model | KiCad position | KiCad orientation | KiCad side |
| --- | --- | --- | --- |
| top, θ | (x, −y) | θ | F.Cu |
| bottom, θ | (x, −y) | θ + 180° | B.Cu (flipped) |

pcbnew's `Flip(pos, FLIP_DIRECTION_LEFT_RIGHT)` on a footprint at orientation
α flips top-bottom (negating α) and then turns it 180°, leaving φ = 180° − α,
i.e. θ = −α. Setting a bottom part's angle *before* flipping it therefore
turns it the other way. The board tool used to do exactly that, and nothing
said so (section 15.2). Flip first, then set φ.

`openparts_kicad::placement` has the conversion (`to_kicad`, `from_kicad`)
and tests that KiCad's pad transform reproduces `board = T·R(θ)·M` for every
pad on both sides, and that the round trip is exact. The board tool also checks
it on the real board: after placing every footprint with pcbnew, it compares
each pad's KiCad position with `T·R(θ)·M` applied to the model's pad (to
1 µm) and stops if one differs.

The 3D model reference in the footprint, `(model (offset) (rotate))`, takes
the model's `model3d` alignment (section 11). KiCad turns a model clockwise
for a positive `rotate` value, so the adapter writes the negated angle. The
offset is in KiCad's model frame, which is Y-up like this model's.

## 3. Zero orientation

At θ = 0 every part lies in its IPC-7351B Level A orientation: pin 1 upper
left. The rule is applied geometrically. The model frame is the generator's
frame turned by a multiple of 90° about the land-pattern centroid. The
generator's own turn is kept when pin 1 (pad `1`, else `A1`, else the first
plated pad) already lies upper left of the centroid (x ≤ 0, y ≥ 0, not at
it). Otherwise the turn is the first of −90°, 180°, +90° that puts it
there. When pin 1 is the centroid itself (an RF connector's centre contact
between symmetric ground pads), no turn places it and the generator's frame is
kept. A test checks every package of the dataset.

| Families | At θ = 0 |
| --- | --- |
| `chip`, `sod`, `sod_flat`, `do214`, `axial`, `radial`, `smd_electrolytic` | Body along X; pin 1 (cathode, +, pad 1) on the left. |
| `to92`, `to220`, `sip` | Leads in a row along X; pin 1 on the left. |
| `sot`, `sot223`, `dpak`, `d2pak` | Leads down the left side, pin 1 at the top; the opposite lead or the tab on the right. SOT-23: pins 1 and 2 on the left, 3 on the right. The generator draws SOT leads along the bottom, so SOT packages turn −90°. |
| `soic`, `tssop`, `ssop`, `msop`, `son`, `dfn`, `dip` | Rows along Y; pin 1 at the top of the left row; numbering counter-clockwise. |
| `lqfp`, `qfn`, `lga` | Pin 1 at the top of the left side, or at the upper-left corner; numbering counter-clockwise. A crystal whose source numbers pin 1 lower left (ABM8) turns −90°. |
| `bga` | Ball A1 upper left; rows A… downward, columns 1… to the right. |
| `module` | Pin 1 upper left (Raspberry Pi Pico: USB end up, antenna end down). |
| `header`, `socket`, `shrouded_header` | Rows along Y, pin 1 upper left (1×N: pin 1 at the top, the row running down). |
| `right_angle_header` | Pins in a column along Y, pin 1 at the top; mating side +X. |
| `tact_switch_smd`, `tact_switch_tht` | Pad 1 upper left. A vendor numbering that puts it lower right turns 180°. |
| `usb_c` | Contact A1 upper left: the opening (mating face) faces −Y. The family's source frame has it facing +Y, so receptacles turn 180°. |
| `card_socket`, `barrel_jack`, `fpc_connector`, `rf_coax`, `micro_usb_b` | Contact 1 upper left; the mating face (+Y in the family frame, Canonical Data Specification 20.6) turns with it. Where it ends up depends on the source's numbering: contacts numbered 1… from the left along the rear edge (pin 1 lower left of the land centroid) turn −90°, so the mating face faces +X. |
| `terminal_block` | Poles in a row along X, pole 1 on the left; the wire entry faces +Y (the family frame already has it so). |
| `smd_led` | Pad 1 upper left. A source that numbers it lower right (WS2812B-2020: DO is pad 1) turns 180°. |

The model reports the rule (`frame.zero_orientation`: rule, family row,
convention, pin-1 pad). The turn and the move from the generator's frame are
in `model3d` (section 11).

## 4. Pads

Each pad has:

| Field | Meaning |
| --- | --- |
| `number` | As the source numbers it ("" for a non-plated hole). |
| `shape` | `rect`, `roundrect` (with `corner_ratio`), `circle`, `oval` (obround), `chamfered` (with `chamfer`: `{size, corners}`, the 45° cut in mm and the corners cut, in this frame), `polygon` (with `polygon`, relative to `position`, counter-clockwise). |
| `position`, `rotation`, `size` | Centre in the frame; degrees CCW; size before the rotation. Every generated pad is axis-aligned (rotation 0); quarter turns swap `size`. |
| `kind` | `smd`, `tht` (plated hole), `npth`. |
| `layers` | `copper`: `top` / `bottom` / `all` / `none`; `mask`: `top` / `bottom` / `both`; `paste`: bool (an aperture on the pad's copper side). `bottom` is a land on the opposite outer layer (Canonical Data Specification 20.6, `side: bottom`: an edge-mount connector's lower legs); `min_pad_gap` does not compare lands on opposite sides. |
| `drill`, `slot`, `plated` | Hole diameter; a slot's overall length along the pad's longer side (`drill` is then its width). |
| `mask_expansion` | mm; `null` uses the board's rule. |
| `paste` | `{mode: full}` (the land), `{mode: windowed, windows: [...]}` (segmented exposed-pad paste), or `null` (holes). |
| `pin` | `{name, type, diff_pair?}` of the device pin (with a device). `diff_pair`: `{partner, partner_pad, polarity, standard, function?, impedance_ohm?}` from the Device pin's `diff_pair` (Canonical Data Specification section 14.2); `partner_pad` is the pad of this footprint carrying the partner pin, `null` if none. |
| `exposed` | The package's exposed (thermal) pad. |
| `castellated` | Cut by the part's own edge (a module's half-holes). |
| `shares_contact_with` | This pad repeats another pad's contact under a second number (USB-C "A1 B12"). Same copper as that pad. |

Pad shape: SMD lands are `rect`. A square plated through-hole pad numbered 1 is
`rect` (the pin-1 cue), other square ones `circle`, longer ones `oval`. These
are KiCad's library conventions. A shape the package states (Canonical Data
Specification 20.7: `circle`, `chamfered`, `polygon`, a `cutout` drawn as a
polygon) is kept; its corners and points turn with the frame. `size` stays the
land's extent, which the courtyard and `min_pad_gap` use.

Where lands come from (`basis.pads`):

1. `datasheet`: the package states a land pattern (`Package.pads`).
2. `ipc7351`: IPC-7351B lands computed from the stated terminal dimensions
   (`openparts-mcad`'s `ipc.rs`, density level N). For each terminal, the
   outer and inner land edges and the land width are
   `Z/2 = Lmin/2 + Jt + √(CL² + F² + P²)/2`,
   `G/2 = Smax/2 − Jh − √(CS² + F² + P²)/2` and
   `X = Wmin + 2Js + √(CW² + F² + P²)`.
3. `datasheet` or `generator_default`: the terminal outline itself
   (through-hole pads, LGA terminals). `defaults` lists the unstated inputs.

Top level, for routing:

- `pitch`: the package's lead pitch (mm).
- `min_pad_gap`: the smallest copper-to-copper distance between two pads of
  different numbers, from their axis-aligned extents (mm).

## 5. Courtyard

`courtyard`: `{density, excess, grid, polygons}`. It is the lands' and the
body's extents grown by the excess, corners snapped outward to a 0.05 mm grid
(KiCad's library rule), as counter-clockwise polygons. Silkscreen does not
count.

| Density | SMD excess (IPC-7351B) | Through-hole excess (generator default) |
| --- | --- | --- |
| L (least) | 0.10 | 0.25 |
| N (nominal, default) | 0.25 | 0.50 |
| M (most) | 0.50 | 1.00 |

`?density=L|N|M` selects the level. The default is N. Footprint artifacts are
always N. `basis.courtyard` is `ipc7351` for SMD parts and `generator_default`
for through-hole ones. Version 1 changes only the courtyard with the level.
The lands stay at level N (section 17).

## 6. Body

`body.regions[]`: solid regions above the board. Each region has `name`,
`shape` (`rect`, `circle`, `stadium`), `center`, `size`, `outline` (a polygon;
a circle has 32 segments), `z_min` and `z_max` (mm above the top surface).

- `body`: the package body (the molded body, a can, a housing, a module's PCB).
- `pins`: a pin header's mating posts above its housing. For a right-angle
  header, the bent posts beyond its front face (none for surface-mount
  headers, whose contacts stay inside the housing).
- `shield`: a module's shield can (`Package::shield`), from the board's top
  face to its stated top.

`body.standoff` is the board-to-body-underside distance: gull-wing A1, a
header's housing standoff, else the body's `z_min`. `body.height` is the
highest `z_max`.

## 7. Pin 1 and polarity

`pin1`: `{pad, position, marker}`. `pad` is the pad the zero-orientation rule
places (`1`, `A1`, ...). `marker` is the body's pin-1 dot, when the geometry
has one.

`polarity` (only when built with the device): `{pad, terminal, mark,
position}` for a polarised two-terminal part (Canonical Data Specification
20.10). `pad` is the marked pad, `terminal` `cathode` (a diode or LED) or
`positive` (a polarised capacitor's `+`), read from the device's pin
names, `position` that pad's centre. `mark` is `band` (the body is banded
at that end: diodes, LEDs, tantalum capacitors; the footprint brackets
that end on the silkscreen) or `plus` (an electrolytic can: a `+` sign
beside the pad). Absent for unpolarised parts, packages with other than
two plated pad numbers, and models built without a device.

## 8. Board edge

`edge` (from `Package.edge`, Canonical Data Specification 20.3; only when the
source states it):

| Field | Meaning |
| --- | --- |
| `side` | The part side facing the board edge, in this frame (`+x`, `-x`, `+y`, `-y`). |
| `requirement` | `required` (the source draws the edge) or `recommended`. |
| `pcb_edge` | Distance from the origin to the PCB edge line, along `side`'s outward normal. `null` when no line is drawn. |
| `overhang` | How far the body reaches past that line (negative: it stays short of it). |

A board tool checks a required edge by transforming the edge line with the
part's placement and comparing it with the board outline. Castellations are
pad facts (`castellated`), not edge statements.

## 9. Keep-outs

`keepouts[]` (from `Package.keepouts`; only datasheet-stated areas, never a
generic default):

| Field | Meaning |
| --- | --- |
| `purpose` | `antenna`, `mechanical`, `thermal`, `other`. |
| `restricts` | `components`, `copper`, `tracks`, `vias`. |
| `polygon` | The area in this frame, when the source locates it. |
| `size` | The two lengths as stated, when the source does not locate it (orientation not given). |
| `side` | The part side the area lies at, when stated. |

An area with `size` but no `polygon` cannot be checked automatically. A board
tool reports it for a human to check (the Raspberry Pi Pico W's 14 × 9 mm
antenna cutout at its −y end).

## 10. Through-hole leads

`tht`: `{tail_length}`. This is how far the leads reach below the board's top
surface. The protrusion below the bottom surface is `tail_length` less the
board thickness. `basis.tht` is `datasheet` with `/dimensions/tail_length`,
else `generator_default`.

## 11. 3D model alignment

The generated STEP, 3MF and STL files are built in the geometry generator's
frame. `model3d` says where that frame sits in this one:

```text
p_frame = R(rotation) · p_model + offset      (rotation about +Z, CCW; offset in mm)
```

`rotation` is the zero-orientation turn and `offset` is the centroid move. The
seating plane is z = 0 in both frames. The footprint's 3D reference carries
exactly this transform. The tests check that the model's pads are the
generator's leads under it. The board tool renders the board in KiCad with the
transform applied (section 15.2).

## 12. Basis

`basis.{pads, courtyard, body, edge, keepouts, tht}`. Each entry is
`{kind, sources, defaults}`:

- `kind`: `datasheet` (stated by the source), `ipc7351` (calculated per
  IPC-7351B from stated dimensions), `generator_default`.
- `sources`: provenance pointers, `{pointer, source, page}`. `pointer` is a
  JSON Pointer into the Package document. `GET .../provenance` has the full
  locators.
- `defaults`: inputs the source does not state, which a generator default
  filled.

## 13. Hash

`hash` is `sha256:` followed by the hex SHA-256 of the model's JSON. Before
hashing, `hash` is emptied, `basis` is cleared and every pad's `pin` is
removed. Provenance and pin names do not change the copper. Coordinates are
rounded to 0.1 µm when the model is built, so the JSON is stable.

The KiCad footprint carries the density-N model's hash as the footprint
property `OpenParts placement`. A footprint is stale when that value differs
from the current `GET .../placement` (density N) hash.

## 14. API

`GET /v1/parts/{manufacturer}/{mpn}/placement?density=N|L|M&silicon_revision=...`

- `200`: the model.
- `400`: a density other than L, N or M (any case).
- `404`: unknown part.
- `422`: the part does not validate.
- `500`: its geometry cannot be generated.

The model includes the device's pin names and shared contacts for the
requested silicon revision.

## 15. Consumers

### 15.1 Footprints

`openparts_pcbcad::footprint_from_placement` builds the CAD-independent
footprint (pads, courtyard, 3D reference with the `model3d` transform, hash)
from the model. Fabrication and silkscreen drawings are drawn around the
model's body and pin 1. The KiCad and LibrePCB adapters and the SVG preview
render that. The BOM land-pattern comparison compares model-frame pads. The
web viewer's footprint pane is the same footprint. Its 3D pane still shows
the model in the generator frame.

### 15.2 Board tool

`tools/kicad-board` fetches every part's model and does the placement geometry
from it:

- the KiCad transform of section 2.4, including bottom-side parts;
- courtyard overlaps, per side, as polygons;
- parts off the board;
- required board edges (the edge line on the board outline, with the overhang);
- keep-outs it can check, plus a note for those it cannot;
- the reference-designator search box.

`placement.yaml` angles follow section 2.3: a bottom part's `rot` is θ as
viewed from the top, applied after the mirror.

## 16. Notes for routing tools

- Pad geometry is in the part frame. Board copper is `Placement.apply` of each
  pad's corners, or of its centre with `apply_angle` for the rotation. On the
  bottom side the copper layer is the bottom one.
- `pitch` and `min_pad_gap` give the neck-down budget directly: the widest
  track between two adjacent pads is `min_pad_gap − 2 × clearance`. Per-pad
  gaps come from the pads' extents.
- An `exposed` pad and `shares_contact_with` pads need care. A shared contact
  is one piece of copper with two numbers: don't route between them.
- `pin.type` (`power`, `ground`, `io`, ...) identifies supply pins for IPC-2152
  width rules. The current itself is not in the model.
- `pin.diff_pair` marks the two pads of a differential pair (`partner_pad`
  is the other one) and the standard it carries. The pair's impedance is the
  standard's (USB 2.0: 90 ohm differential), which the board tool supplies;
  the model gives an `impedance_ohm` only when the part's own source states
  one.
- A `castellated` pad sits at the module's edge, and the host board's copper
  under it is the land.

## 17. Not in version 1

- Lands at density levels L and M. The level changes only the courtyard. The
  IPC-7351B fillet goals for L and M are not tabulated in `openparts-mcad`
  yet.
- Family-specific courtyard excess from IPC-7351B's tables (BGA, chips smaller
  than 1608). Version 1 uses the general 0.10/0.25/0.50 mm.
- Windowed exposed-pad paste and per-pad mask expansion. No generator states
  them, so every SMD pad has `paste: full` and `mask_expansion: null`.
- `roundrect` pads. No generator draws them yet.
- Body regions beyond the main body, header posts and module shield cans
  (a USB-C shell's tabs).
