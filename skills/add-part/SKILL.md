---
name: add-part
description: Add electronic parts (Part + Device + Package + Source) to openparts-data from manufacturer datasheets, with provenance, and verify they validate and generate KiCad footprints and 3D models. Use when asked to add, ingest or bulk-create parts in openparts-data.
---

# Adding parts to openparts-data

Every value you write is a **candidate change** for human review (Agent
Ingestion Specification): it must come from a primary source you actually
read, with a page and locator. Read once: `docs/spec/canonical-data.md`
(sections 3-27, 34-35) and `docs/spec/agent-ingestion.md` (sections 5, 14,
28, 30).

## 0. Rules that are never bent

- **Unknown is better than invented.** Leave out what the source doesn't
  state. Never estimate, average, or copy from a "similar" part. A range
  given only as min/max is recorded as min/max -- never add a `typ` (the
  generators draw such a dimension at its midpoint without storing it). A
  lone "1.80 max" is recorded as `max: 1.80 mm` only -- never copy it into
  `typ` (the generator draws at the max; a min-only height is treated
  as a floor).
- **Record the land pattern.** When the datasheet gives a recommended
  PCB land ("Recommended Land Pattern", "Example Board Layout", "PCB
  layout"), record it in `pads` (every pad, arithmetic in the `/pads`
  locator): the footprint then uses it. Without it the footprint lands
  are computed from the leads by IPC-7351B (density N) -- fine, but the
  vendor's land is better.
- **Cite what you read.** The provenance `source` is the exact document
  *and revision* you opened. A newer revision than an existing Source gets
  its own Source (`id: <mfr>/<doc>@rev<N>`, file
  `sources/<mfr>/<doc>-rev<N>.yaml`).
- **Never get around access controls.** On 403/"Access Denied" you may
  retry **once with another standard tool** (e.g. WebFetch after curl) --
  that is not a workaround. What is: spoofing a user agent or other
  headers, solving or evading a bot challenge (Cloudflare "Just a moment",
  an empty 202), disabling TLS certificate checks, login walls, unofficial
  mirrors. If the second try also fails, use another *first-party* URL
  (product page, alternate official PDF link) or record the part as
  unverified and move on. Distributor-hosted copies (DigiKey/Mouser) of a
  manufacturer PDF are acceptable only when the existing dataset already
  relies on them for that manufacturer -- say so in the Source comment.
- **Facts, not copies** (Canonical Data Specification 26.2): values,
  units, conditions, identifiers. No table layouts, prose, figures or
  graph-read values; locators are short ("Table 5, VGS(th)"); facts from
  manufacturer documents only, never scraped from parametric databases.
- **Never mark anything reviewed.** A `review:` block (Canonical Data
  Specification section 26.1) is written only by a person in an
  openparts-data pull request. When you edit a document that has one,
  delete the whole block in the same change: the changed data needs a
  new review.
- **`construction`** (molded, tantalum_molded, glass, ceramic, metal_can)
  only when the source text says it -- never from the name, size or a photo.
- **`edge`** / **`keepouts`** (Canonical Data Specification 20.3) only when
  the source states them: a layout's labelled "PCB Edge" line (`pcb_edge`),
  a datasheet's keep-out area. An unlabelled line is not a PCB edge; an
  area the source doesn't locate gets `size` (and `side`), never invented
  coordinates.
- Data only, in your assignment. Don't edit code, the skill, or another
  agent's files; create files exclusively (never overwrite).

## 1. Layout, ids and names

```
manufacturers/<mfr>.yaml                   one per manufacturer (shared)
sources/<mfr>/<doc>.yaml                   one per document revision (with
                                           `retrieved: "YYYY-MM-DD"`, the
                                           day you downloaded it)
devices/<mfr>/<DEVICE>.yaml                logical device: pins, category
packages/standards/<family>/<ID>.yaml      generic outline, reusable
packages/manufacturers/<mfr>/<ID>.yaml     a vendor's own drawing
parts/<mfr>/<series>/<MPN>.yaml            orderable MPN -> device + package
```

Copy the shape of an existing file (`parts/onsemi/2n7002et1g/2N7002ET1G.yaml`,
`devices/onsemi/2N7002E.yaml`, `sources/onsemi/2N7002E-D.yaml`,
`packages/standards/lqfp/LQFP48-7x7-P0.5.yaml`) and give every new file a
comment header saying what you verified where.

Files are schema 0.2 (canonical-data.md 3.3 and 25): `source:` names the
file's main document, every fact carries `src: p<page> "<locator>"`,
quantities are unit strings (`0.4 mm`, `±10 %`), conditions go in `at`.
Run `openparts data fmt <files>` before committing; CI runs
`openparts data fmt --check`.

- **Ids share one namespace across kinds**: a Source id equal to a Device
  id (`aos/AO3400A`) breaks resolution. Suffix datasheet sources
  (`aos/AO3400A-DS`) or use the document number.
- **Ids and file names**: no spaces, `/`, `(`, `,` in the id path or file
  name -- replace with `-`/`_`. The `mpn:` field keeps the exact string
  (`TLP785(GB-LF6,F`, `LS Q976.01-N1Q2-1`, `24LC256-I/SN`).
- **Package names** carry what distinguishes them: body x length, pitch,
  EP size for QFN (`QFN48-7x7-P0.5-EP5.15`), the vendor drawing code
  (`SOT96-1`, `C04-052C`, `PW0020A`), a thickness or tab code where it
  varies (`C0805-DH`, `SOT223-...-TabPin4`).
- `category`: the most specific node of the category tree
  (canonical-data.md 14.1) the device's own datasheet title supports:
  microcontroller, capacitor, resistor, inductor, diode, transistor, led,
  crystal, fuse, connector, optocoupler, or a subcategory such as
  `linear_regulator`, `eeprom`, `imu`, `op_amp`, `can_transceiver`,
  `logic`, `rtc`, `tact_switch`, `gate_driver`; `other` only when the tree
  has no fitting node (the validator warns). The passive/discrete values
  change how the 3D model is drawn. `document_type` also accepts
  `product_page`, `pcn`, `eol_notice`, `other`.

## 2. Parts: existence and lifecycle

- `existence: verified` only when the MPN is printed verbatim in an
  ordering table or a product page's orderable list (cite it). An MPN
  assembled from a part-numbering legend ("not all combinations are
  valid") or an ordering *example* is `probable`; say which in the header.
  By family: tinyAVR 0/1/2 and AVR-DA/DB/DD datasheets have orderable
  tables (verified); classic PIC10/12/16/18 only a "Product Identification
  System" legend with examples (probable); a package-marking table is not
  an orderable list. Würth prints the order code in each drawing's title
  block (verified).
- Value-specific passives need **two** citations under `/existence`: the
  ordering-code legend and the availability table (resistance range/E
  series, the MLCC voltage/value cell).
- `lifecycle` is required: `{status: unknown}` unless a first-party source
  states it. TI's "PACKAGE OPTION ADDENDUM" Status column (inside the
  datasheet, usually newer than the datasheet revision) and product pages
  ("Full production", "Mass Production", "Active") are first-party; cite
  the page.
- Several MPNs (reel, grade, temperature) share one Device and Package.
- Electrical parameters (`parameters:` capacitance, resistance, rated
  voltage...) follow the Parameter Ingestion Guide,
  docs/spec/agent-ingestion.md section 19.1: closed key list, a unit of
  the key's quantity, a `src` and the printed conditions (`at`) for each,
  and no values decoded from the MPN unless the ordering-code table maps
  the code (cite that table).

## 3. Devices and pins

- **Pinouts are per package.** A part in SOIC and DIP (TL431, ATmega328P,
  W25Q128JV, MCP2515) often numbers pins differently: one Device per pin
  map (`ti/TL431-D`, `microchip/ATmega328P-SPDIP28`), header says which
  packages it covers.
- Take pin names from the pin table, check numbers against the pinout
  figure (and the outline's "STYLE" list for discretes). Prose can be
  wrong ("numbered clockwise"); figures can carry typos -- trust the table
  and note the conflict.
- **Polarity is read from pin names**, never assumed from numbers: name an
  LED's/diode's terminals `Cathode`/`Anode`, a polarised capacitor's
  `+`/`-`. The 3D model puts the cathode band, the LED flat and the
  tantalum band, and the footprint its silkscreen polarity mark
  (canonical-data 20.10), from these names (Kingbright numbers the cathode 1,
  Everlight the anode 1). When a datasheet names terminals but doesn't
  number them (OSRAM, many SOT-23 pinouts), number them in the generator's
  position convention (section 4) or, for LEDs, cathode = 1, and say so
  in the device header.
- **Exposed pad**: add an `EP` pin only if the datasheet says what it
  connects to; "recommended to VSS" -> type `passive`, never `ground`.
  Otherwise omit the pin (the footprint still gets the pad).
- `pin.duplicate_name` is exempt only for power/ground/nc/reserved pins
  and for pins linked by `connected_to`: when the source says a terminal
  is internally connected to another pin (a SOT-223/TO-252 tab "connected
  to pin 2", a second collector), give it `connected_to: "<pin>"` and the
  same name. Separate dies' terminals are different nets: name them apart
  (`Cathode1`/`Cathode2`). A TO-252 trimmed lead can simply be left out.

## 4. Packages

### Reuse only on identical numbers
Search before creating (`grep -rl "family: soic" packages/`). Reuse only if
every dimension in your drawing matches -- the same "LQFP48" or "SOT-23"
differs by vendor (body 1.30 vs 1.60, pitch 1.90 vs 1.83, tolerances,
A2 1.0 TQFP vs 1.4 LQFP). Compare numbers, not drawing revision letters. A
reused package cites its own Source: pass it with `--source` when
validating a single part. Different vendors' packages that happen to
match number-for-number stay separate (each cites its own drawing).

### Family conventions (what the generators assume)

| family | convention | notes |
|---|---|---|
| `lqfp` (TQFP too) | 4 equal sides, pin 1 top of left edge, CCW | body_width = E1 (X), body_length = D1 |
| `qfn` | 4 equal sides, flush terminals, optional `exposed_pad` | unequal sides not supported |
| `dfn` / `son` (WSON, VSON, UDFN, TDFN...) | flush terminals on the left/right edges only, pin 1 top-left, down the left, up the right (SOIC order); optional `exposed_pad` | body_width = across the two rows (X); `lead_length` = terminal L, `lead_width` b, `lead_thickness` A3 |
| `soic`, `tssop`, `ssop`, `msop` (VSSOP) | left/right rows, pin 1 top-left | body_width = E1, overall_width = E; MO-012/153/150/187 defaults |
| `sot` | `lead_layout: [bottom, top]`; bottom left->right, top right->left (CCW) on the bottom grid | SOT-23-3 `[2, 1]`, SOT-23-5 `[3, 2]`, SOT-23-6 `[3, 3]`; leads exit across `body_length` |
| `sot223` | 3 leads left edge from the top, tab right | `tab: {number}` from the source, `tab_width` (b2) |
| `dpak` / `to252` | leads + trimmed centre lead + tab | `tab.trimmed_leads`, `tab_length` (L3) |
| `d2pak` / `to263` | same layout as `dpak`, TO-263 defaults | body_width = D (along the leads, X), body_length = E (across), `overall_width` = H (lead toe to tab end), `tab_width`, `tab_length` (L3); 5/7-lead variants: `lead_count` = all positions, trimmed ones in `tab.trimmed_leads` |
| `sod` | 2-terminal molded body, gull-wing leads bent down (SOD-123/323/523) | frame like `chip`: `body_length` = body along the leads (X), `body_width` across; `overall_length` = HE (tip to tip); `lead_width` b, `lead_length` L, `lead_thickness` c; pin 1 at -X -- the cathode is whichever pin the Device names `K`/`cathode` (band drawn there) |
| `sod_flat` | as `sod`, flat leads straight out of the body underside (SOD-123F/FL, SOD-323F, PowerDI-123 style) | same fields |
| `do214` | SMA/SMB/SMC: J-bend leads folded under the body | same frame; `overall_length` = E incl. leads, `lead_length` = the folded foot L (becomes the pad), `lead_width` b, `lead_thickness` c |
| `to220` | vertical TO-220 (and TO-220-5/7 at their pitch): leads in one row along X, pin 1 left (front view, leads down), tab behind (+Y) | `body_width` = E (across), `body_length` = A (thickness incl. tab), `body_height` = molding height above its underside; `overall_height` = tab top, `mounting_hole_diameter`, `mounting_hole_inset` (tab top to hole centre), `hole_diameter` (drill) / `pad_diameter` if the source gives a land; `body_offset` = body centre behind the lead row (default 0.95); fully isolated TO-220F not supported |
| `lga` | sensor LGA (Bosch, ST): terminals on the underside, per-side counts `lead_layout: [left, bottom, right, top]` (zeros allowed), numbered counter-clockwise from the top of the left edge (top view), each side centred at `pitch` | `lead_length`/`lead_width` = terminal size, `lead_inset` = body edge to the terminal's outer end (0/absent = flush); a centre pad goes in `exposed_pad`. Check the vendor's numbering in the top view (drawings are often bottom views): clockwise-numbered parts (Bosch BME280/BMP280/BME680/BME688, BMI088) list the source's numbers in `pad_order`, in the CCW position order. A part whose pin 1 sits on a 2-pad edge may be turned 90 deg (say so in the header). Skip and report: unequal pitch or terminal length per side, off-centre rows, interior pads |
| `tact_switch_smd` / `tact_switch_tht` | tactile switch: housing + round actuator, 2 or 4 terminals left/right; positions numbered 1 top-left, 2 bottom-left, 3 bottom-right, 4 top-right (2-terminal: 1 left, 2 right) | `pitch` = Y spacing of the two terminals on one side; `pad_order` = the source's numbers in that position order when they differ; `pads` = the recommended land pattern (the footprint then uses it instead of the terminal feet -- record it whenever the source gives one); `body_*` housing, `overall_height` actuator top above the board, `actuator_diameter`. SMD: `overall_width` toe to toe, `lead_width`, `lead_length` (foot). THT: `row_spacing` (X between pin columns), `hole_diameter`, `pad_diameter`, `standoff`. Devices: name pins as printed; internally connected pairs get `connected_to` |
| `usb_c` | USB Type-C receptacle from an explicit land pattern: `pads: [{number, x, y, width, length, drill?, drill_length?}]`, top view, X right / Y up, origin on the receptacle centre line, mating opening toward +Y | copy every pad from the recommended PCB layout (signal pads and shell tabs; `drill_length` = slot length for slotted holes; alignment-peg holes as `{number: '', ..., drill: d, plated: false}`); `lead_count` = number of pads; `body_*` = shell (width across the opening, length = depth, height); `body_offset` = shell centre Y in the pad frame (required); `standoff` if the shell sits raised; coordinates computed from the drawing need the arithmetic in the `/pads` locator. Mid-mount / board-cutout receptacles not supported |
| `card_socket`, `barrel_jack`, `terminal_block`, `fpc_connector`, `rf_coax`, `micro_usb_b` | connector families (canonical-data 20.6; a hole drawn without a land: leave width/length out, a slot adds `slot_axis`; `rf_coax` may omit an unprinted `body_height`): explicit `pads` from the recommended PCB layout, top view, X right / Y up, **origin at the body outline's centre**, mating face (card slot, plug opening, cable/wire entry, right-angle barrel) toward +Y | `lead_count` = plated pads (peg holes `plated: false` don't count; shield pads may share a number); `body_height` required; `connector: {gender?, orientation?, mechanism?, contact_side?, interface?, edge_mount?}` with `src` -- card sockets need `mechanism` + `interface`, FPC `contact_side`, RF `interface` + `orientation`; edge-mount RF needs `edge` with `pcb_edge`; record `edge` whenever the layout draws a PCB edge line; optional `mating_diameter`, `pin_diameter` (barrel jack centre pin); converting from a pin datum to the body centre: arithmetic in the locator |
| `smd_led` (alias `plcc_led`) | top-view SMD LEDs (5050/3535 PLCC, 2020), addressable RGB LEDs (canonical-data 20.8): explicit `pads` from the recommended pad layout, top view, origin at the body centre | `lead_count` = plated pads; `body_height` or `overall_height`; terminals for 3D from `overall_width` + `lead_width` + `lead_length` when the outline states them |
| `smd_electrolytic` (alias `v_chip`) | SMD aluminium electrolytic ("V-chip", canonical-data 20.9): explicit `pads` from the recommended land pattern, top view, origin at the base plate's centre, terminals along X, `+` (pin 1) at -X -- sources draw them along Y: turn and say so | `body_width` x `body_length` = plate (B along the terminals, A), `can_diameter` = øD (required), `overall_height` = L (required), `overall_width` = H (tip to tip, a lone max is fine), `lead_width` W, `lead_length` I (often a "( )" reference value: say so); terminal reach K and gap P have no field; name the Device pins `+`/`-` |
| any family: non-rectangular lands | canonical-data 20.7: `shape: circle` (width = length), `shape: chamfered` + `chamfer` (mm) + `corners`, `shape: polygon` + `points` (relative to x/y, no width/length), or a rectangle with `cutout: {x, y, diameter}` (a circle cleared from it, package frame; one notch) | record the shape the drawing shows instead of the bounding rectangle; arithmetic for derived points in a comment |
| any family | `pad_order` (source numbers in the family's position order) and `pads` (recommended land, used as the footprint instead of the lead outlines) apply to every family | |
| `chip` | 2 end terminals along X; `body_length` is the long axis | style by category; 4-pad parts not supported |
| `radial` | can of diameter `body_width`, 2 leads | electrolytic stripe on the `-` lead |
| `axial` | horizontal can, 2 leads | band at the cathode; axial resistors not supported |
| `to92` | 3 leads, middle staggered | |
| `dip` | `body_width` is the **row spacing** eA (7.62/15.24) | E (shoulder width) is not the row spacing |
| `sip`, `bga` | single row / `lead_layout: [rows, cols]`, A1 top-left | |
| `module` | castellated board (Pico): pins 1..N/2 down the left edge from the top, the rest up the right; optional `aux_pads` row (debug pads) | body = the PCB (`body_height` = board thickness); pads are the source's recommended land: `overall_width` outer span, `lead_length`/`lead_width` pad size; `row_spacing`, `hole_diameter`, `aux_*`, `mounting_hole_*` per canonical-data 20.1 |
| `header`, `socket` | straight THT pin header / socket strip; `lead_layout: [rows, pins_per_row]` (1 or 2 rows), rows along Y at `pitch`, `row_spacing` apart in X; pin 1 top-left, numbers zig-zag across the rows (pin 2 beside pin 1, pin 3 below it) -- check the drawing's "1"/"2" labels (Würth: first-angle projection, the hole pattern is the component side) | `body_*` = housing (`body_width` across the rows, `body_length` = L along them); `hole_diameter` (recommended PCB hole, required) is the drill, `pad_diameter` the land if given (else hole + ring default); `lead_width` square pin, `mating_length` post above the housing, `tail_length` below it; row-by-row numbering and 3+ rows not supported |
| `shrouded_header` | box-shaped wire-to-board header (JST XH/PH), top entry; same frame, numbering and footprint rules as `header` (turn the source's PC-board layout, viewed from the mounting side, so pin 1 is on top and numbers run down) | `body_*` = housing outline (`body_width` = depth across the row); `body_offset` = housing centre's signed X offset from the pin grid (printed row-to-face distance and depth, arithmetic in the locator: XH 5.75/2 - (2.35) towards the boss side = -0.525); walls/floor/post height are generator defaults; undimensioned lock slots aren't drawn; drawn natural nylon + tin |
| `right_angle_header` | 1-row right-angle pin header: housing lying on the board beside the holes, pins bent 90° | `body_offset` required (Würth: tail-to-housing "1,5" + depth 2.54/2 = 2.77, +X when the hole pattern's housing is below the row and pin 1 on the right); `body_width` = housing depth along the pins, `body_height` = its height, `mating_length` = housing front face to tip, `tail_length` = board to tail end |

Covered, not missing: SC-70-6/SOT-363 and SOT-563 are `sot` with
`lead_layout: [3, 3]` (TI DRL0006A); SOD-523/SOD-923 `sod_flat` (or `sod`
for gull-wing leads); 4-pad ceramic crystals `lga` (Abracon ABM8), 2-pad
ones `chip`; RGB LEDs `smd_led`.

Not supported (report, don't force): SOIC-EP, X2QFN/USON, multi-die LEDs, round drum
inductors, HC-49 ovals, 2-row right-angle headers, side-entry or SMT
wire-to-board headers, connectors with extra board holes (boss/peg
variants such as JST B?B-XH-AM: the footprint would lack the hole).

### Dimensions (canonical-data.md 20.1)
`body_width`, `body_length`, `body_height` (A2, else A when no A2 is
given -- then don't also add `overall_height`), `overall_width`/
`overall_length` (lead span E/D, HE), `overall_height` (A), `standoff`
(A1), `lead_width` (b), `lead_length` (L: the gull-wing **foot** -- NXP's
`Lp`, not its `L`; on QFN the terminal length; DIP lead length to the
seating plane is *not* `lead_length`), `lead_thickness` (c, A3 on QFN),
`exposed_pad`, `tab_width`, `tab_length`. Each has its own `src`
(`lead_layout: { value: [2, 1], src: ... }` and `tab` can be cited too).

- Parenthesised values ("(0.2)") are reference values: record only if the
  table marks them as such and say so; "BSC" values are nominal.
- Vendor letter codes differ from JEDEC: Diodes uses A=b, B=E1, C=E, H=D,
  J=A1, M=c and orders columns Min/Max/Typ; Renesas uses D/E for the body
  and HD/HE for the span; older Diodes DS swap L/L1. Read the drawing's
  arrows, not just the letters (a "standoff"-looking value may not be A1).
- Inch drawings: convert x25.4 and keep the inch text in the locator.
- MLCC thickness depends on value and voltage (KEMET waterfall thickness
  codes, Murata's 4th MPN character); footnotes can widen L/W for some
  values. The package must match that cell. Bourns series have height
  groups by value range (SRN4018 1.88 vs 1.80 max).

## 5. Reading sources

- **Check what you downloaded**: `file x.pdf` -- error pages come back as
  HTTP 200 (Espressif ESP32-C2 is `esp8684_datasheet_en.pdf`), and some
  URLs serve another part's document (Diodes `BAT54.pdf` -> B0540W). Check
  the title page.
- Tools available here: poppler (`pdftotext -layout`, `pdftotext -bbox`
  for multi-column pin tables, `pdftoppm -r 110..400 [-x -y -W -H]` to
  render and crop). pypdf/pypdfium2 may be missing. Render tables whose
  text extraction drops empty cells (MLCC waterfalls) or uses an encoded
  font (Atmel-era Microchip pages extract as gibberish).
- **Page numbers**: cite the PDF page index; when the printed number
  differs (RP2350, ESP8684, Winbond: printed = PDF - 1), say which in the
  Source comment.
- Package outlines on one page can show several variants (QFN48 5x5 vs
  6x6): confirm the drawing title. Old datasheets carry new outline
  drawings (TI): cite the drawing number and revision. Many TI datasheets
  omit some package drawings (PDIP N/P, D0016A): check the drawing exists
  before choosing an MPN; TI's old MECHANICAL DATA sheets
  (`ti.com/lit/pdf/mpdi001`) are image-only.
- Dates: Raspberry Pi colophon build dates are **DD/MM/YYYY** ("03/07/2026"
  = 3 July 2026; check the release history); Bourns footers carry
  "REV. MM/YY"; Toshiba datasheets have no document number (cite the
  footer date); Renesas document numbers embed the revision.
- Shell: use a quoted heredoc (`<<'EOF'`) for YAML -- an unquoted one runs
  backticks in comments. Work in your own scratchpad subdirectory (other
  agents share the scratchpad).
- **WebFetch on a PDF URL also saves the original file** (under the
  session's `tool-results/webfetch-*.pdf`) even when its text answer is
  useless: copy it and read it with poppler.
- Rejections can arrive as HTTP 200 HTML ("Request Rejected", onsemi's
  missing-datasheet page): always `file` the download.
- **Proving a reused image-only drawing** (onsemi outlines are images, so
  text diffs prove nothing): render the same crop of each outline page
  (`pdftoppm -r 100 -f P -l P -x .. -y .. -W .. -H .. -png`) and compare
  checksums; identical embedded drawings come out byte-identical. Say in
  the header that the check was pixel identity.
- **Proving a reused drawing**: extract the outline page with `pdftotext
  -layout`, strip header/footer lines and `diff` it against the page the
  package was read from -- no rendering needed. Same drawing number with
  different revisions can still differ in one row (Microchip C04-065 Rev B
  c 0.17-0.25 vs Rev D 0.10-0.25): compare every row.
- **First-angle projections** (Würth): the view above the front view is
  the *bottom* view, mirrored against the hole pattern. Number pins from
  the "Recommended Hole Pattern" (component side). Rotated dimension text
  is dropped by pdftotext: render it.
- Never derive an unstated dimension by subtracting printed ones (a
  header's housing height from total pin length - post - tail); sums or
  differences of *printed* dimensions that define a stated feature are
  fine when the locator shows the arithmetic.
- A vendor reuses one outline drawing revision across many datasheets
  (TI DBV0005A, DGK0008A): diff the outline pages' `pdftotext -layout`
  text, then create one `packages/manufacturers/<mfr>/<DRAWING>` package
  and reuse it. Datasheets embed outlines newer than themselves (onsemi
  369C Issue L in a 2021 datasheet): note both dates.
- Board modules (`module`): use the datasheet's "SMT footprint" figure as
  the land pattern and the "pin numbering" figure for pad numbers; spell
  out any sum/difference of printed dimensions in the locator.
- onsemi: the STYLE list is on the page after the case outline (STYLE 6
  BJTs, 21 MOSFETs, 8 zeners, 318E STYLE 1 SOT-223 BJTs) -- use it to check
  discrete pinouts. Standalone outlines need the case suffix
  (`.../package-drawing/pdf/318E-04.pdf`; bare `318E.pdf` is empty). Old
  Fairchild FDN/NDS parts carry CASE 527AG, not CASE 318. Memory
  datasheets' pin tables have no numbers: number from the pin figure.
- Zener and similar series print the full OPN only in the electrical
  table's Device column while the ordering table shows a pattern
  (`BZX84CxxxLT1G`): that is `probable`.
- Samsung CL: the 9th code (Q/R/U/Z) widens L/W/T tolerances; take L/W/T
  from the product page, not an N-code package. Product pages state NRND
  and replacements (first-party lifecycle).
- PDFs can be revised page by page (KOA RK73H p1 10/26/25, p2 01/27/26):
  check every page footer and `pdfinfo` CreationDate before reusing a
  Source. Ratings can split one size by resistance range: cite the row
  that contains the value.
- JST catalogue PDFs have no revision: cite the PDF creation date; "Model
  No." tables are orderable lists (verified). The housing side comes from
  the PC-board layout (No. 1 circuit, boss, "Connector outline", lock
  side) plus the side view's row-to-face distance; record that distance
  and the depth and compute `body_offset` in the locator.
- Diodes TSOT26 (DS41326) uses JEDEC letters in Min/Max/Typ columns,
  unlike older Diodes letter codes.
- Ordering tables can contain typos (Raspberry Pi "SCO918"): cross-check
  with the vendor's product-information page.
- Downloading a distributor-hosted copy (DigiKey) may be refused by the
  permission classifier; that needs the user's approval -- don't work
  around it, report it.

- Diodes Inc.: when `/assets/Datasheets/<part>.pdf` is a 404 page, try
  `diodes.com/datasheet/download/<part>.pdf` ("Datasheet not found" in
  plain text means none). SMA tables use old letters (A width, B length,
  C b, D c, E span, G standoff, H foot, J height); SMB tables may use
  JEDEC letters for the same numbers; SMA "J" differs between datasheets.
- onsemi SOD-523 (case 502) has flat leads: `sod_flat`, not `sod`.
- 2-row DFN axes: Microchip and Richtek put D along the terminal rows
  (D = body_length, E = body_width); Winbond WSON is the other way round.
  Microchip's encoded-font pages are a Caesar shift (+29 per character)
  -- enough to find the drawing pages before rendering them.
- TI TO-263 (KTT) drawings give no separate tab width (the tab spans body
  width E; record that and say so); KTT0003B has no trimmed centre lead.
  Old TI TO-220-5 drawings (NDH/NEB) have bent, staggered leads: not
  `to220`. The side-view "(0.x) TYP" on TI SON/QFN drawings is side-wall
  metal thickness, not A3 -- leave `lead_thickness` out.
- `to220`: give `body_height` (the molding); without it the default
  9.0 is drawn (`overall_height` is the tab top). AOS outlines are at
  aosmd.com `/sites/default/files/res/packaging_information/TO220.pdf`;
  product pages embed `product_status` ("Last Time Buy" = eol_announced)
  and product URLs are in `sitemap.xml?page=1..3`.
- PowerDI-123 has a large cathode pad under the body that `sod_flat`
  cannot draw: flag it in the package header.

### Manufacturer access notes (as of 2026-11)

| reachable directly | blocked with curl (retry once with WebFetch) / unavailable (don't work around) |
|---|---|
| ti.com/lit/ds/symlink/<part>.pdf; suddendocs.samtec.com/prints/<series>-mkt.pdf (curl); world-semi.com/downloads/datasheets/en/<model>-<ver>.pdf (curl; the list is in /datasheet-availability.js; pages are images: read renders); onsemi.com/pdf/datasheet/<x>-d.pdf and onsemi.com/download/package-drawing/pdf/<case>.pdf (curl, 2026-11); we-online.com/components/products/datasheet/<code>.pdf (WebFetch; one PDF per order code, a missing code is 404); jst-mfg.com (WebFetch; encrypted PDFs allow text extraction); ww1.microchip.com/downloads/en/DeviceDoc/<DSnumber><rev>.pdf (a non-existent file answers 301 to filehandler, not 404: accept only 200; the newest revision is often the file name *without* a revision letter); aosmd.com (outline PO-00009 at /sites/default/files/res/package/TO252.pdf); vishay.com/docs/<n>/; richtek.com/en/product-specifications/... (JSON-LD has the SKU list and datasheet URL: cite as product_page); datasheets.raspberrypi.com (redirects to pip-assets: take id/revision from the redirect) and pip.raspberrypi.com (order codes); nxp.com/docs/en/data-sheet/; renesas.com/en/document/dst/; gd32mcu.com/data/documents/datasheet/; ww1.microchip.com aemDocuments PDFs (MSLD/APID/MPD paths); winbond.com/resource-files/; ams-osram.com product pages -> look.ams-osram.com PDFs (check English vs Chinese); kingbrightusa.com/images/catalog/spec/<mpn lowercase, / -> ->.pdf; ecsxtal.com/<series> (orderable list: verified existence); wch-ic.com/download/file?id=<n>; Toshiba product page `info/docget.jsp` links; product.samsungsem.com/mlcc/<PN>.do | st.com (timeouts with curl and WebFetch), KEMET content site (expired TLS), TDK, Silicon Labs, Nexperia, Torex (403 to both), Lite-On ("Request Rejected" to both), Coilcraft (Cloudflare challenge: unavailable), Nordic (Cloudflare challenge), WCH CH32V003 page (JavaScript only), microchip.com product pages (403 to both), Infineon (empty 202), analog.com (timeouts), Sharp opto (redirect), aviaic.com (TLS), Mouser PDFs (HTTP/2 errors), murata.com (JS app, no static PDFs) |

## 6. Verify

From `openparts/`:

```sh
# one part (add --source for a reused package's own source)
./target/debug/openparts validate --part <part> --device <device> \
  --package <package> --source <source.yaml> [--source ...]
# whole dataset
../openparts-data/scripts/validate.sh
# every package must generate; SMD pads must not overlap; toes at the span
cargo test -q -p openparts --test vertical_slice
# 3D model + PNG: one model per *package* (the first part found); the
# filter matches "<family>_<Category>_<mpn>", so filter by family or a
# series prefix (e.g. "module", "15006"), not necessarily your MPN
cargo run -q -p openparts-solidcraft --example gallery -- ../openparts-data <out-dir> <filter>
```

Look at the PNG of every new **package**: leads on the right sides, pin-1
mark at pin 1, plausible proportions. Look at the footprint too:
`openparts generate` writes `<mpn>.footprint.svg` (pads with their
numbers, silkscreen, body outline, courtyard) -- open it in a browser. SOT and header models carry no
pin-1 mark: check pin 1 from the generated footprint pads instead. For `sot` layouts with 4+ leads,
also check the generated pad x-positions against the datasheet's top view.

What `openparts validate --data` (and CI) enforces, so you don't learn
it the hard way (the full list is `RULES` in openparts-validator):
- **Unknown keys are errors** (a typo like `lead_lenght:` no longer
  silently falls back to a default).
- **Every device pin has a pad** (`pin.no_pad`): a pin cut at the body
  goes in `tab.trimmed_leads`; a pin sharing another's contact (USB-C
  "A1 B12") gets `connected_to` and the footprint overlays the pad under
  both numbers. An `EP` pin needs `dimensions.exposed_pad`. Pads without
  a pin are warnings (`pad.no_pin`).
- **Provenance keys must name a field the document has**
  (`provenance.unknown_path`).
- **`existence: verified` needs a `src` on `existence`** (page
  + locator of the orderable list) -- a warning today, for older parts.
- Source `date` is `YYYY`, `YYYY-MM`, `YYYY-MM-DD` or `unknown`;
  `retrieved` is `YYYY-MM-DD`; `url` is http(s).
- **Ids match their files**: `parts/<mfr>/...`, `devices/<mfr>/<NAME>`,
  `packages/standards/.../<NAME>` = `standards/<NAME>`,
  `packages/manufacturers/<mfr>/<NAME>` = `manufacturers/<mfr>/<NAME>`,
  `sources/<mfr>/...`.

## 7. Working in parallel

- Each agent owns its manufacturers, its package families or folders
  (`packages/manufacturers/<mfr>/`), and the manufacturer files it may
  create. Check before creating anything; never overwrite. List the
  package folder again right before moving files into place: another
  agent may have just created the same outline -- switch to reuse when
  the numbers match.
- One malformed file from anyone breaks everyone's `validate.sh` and
  gallery run: write files in one go (generate, then move into place), and
  if a failure points at someone else's file, wait and re-run.
- Validate in a merged copy first: copy openparts-data plus your staged
  files into your scratchpad, run its `scripts/validate.sh` with
  `OPENPARTS_CLI=target/debug/openparts` and the prebuilt gallery on it,
  then move the files into the shared tree.
- Check pin positions in `<mpn>.footprint.svg` from `openparts generate
  ... --out <dir>` (or `grep '(pad' <dir>/*.kicad_mod`, KiCad Y down).
- If `cargo run --example gallery` doesn't compile because code is being
  edited, the prebuilt `target/debug/examples/gallery` still works.

## 8. Report

Per part: files added, values verified (source/page/locator), values left
out and why, anything unusual. End with Learnings: concrete, reusable
lessons for this skill (they get merged into the sections above).
