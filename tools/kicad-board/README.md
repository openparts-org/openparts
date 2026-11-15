# tools/kicad-board

Builds a KiCad project from a board description (`board.yaml` +
`placement.yaml`) whose parts all come from an OpenParts registry:
schematic, placed board, Freerouting, ERC/DRC and renders.

```sh
tools/kicad-board/build.py examples/pico-board/board.yaml --out DIR \
    [--stage fetch|sch|pcb|route|check|all|preview] [--registry URL] \
    [--layers 2|4] [--fab PRESET] [--stackup L1,L2,...] [--no-fanout] \
    [--evaluate-all] [--rounds N] [--sweep N]
```

`build.py` (stages, schematic, project, routing sweep) runs with any
Python 3 with PyYAML; `pcb_stage.py` runs inside KiCad's Flatpak (pcbnew);
`rules.py` is the routing-rule derivation, pure Python and unit-tested
(`pytest tools/kicad-board/tests`). The stages are described in
`build.py`'s docstring; the routing rules below.

## Routing rules

Nothing in `board.yaml` states a track width. Every width follows from the
fab profile, the nets' currents and the parts' Placement Models
(`docs/spec/placement-model.md`).

### Fab profile

```yaml
board:
  fab: {preset: jlcpcb-2layer-standard}       # overrides may follow
```

Presets are in `fab/`, each with the page it was taken from and the date
it was read:

| Preset | Source (retrieved 2026-11-15) | Min track / space | Min via drill / pad | Copper | Stackup (trace to plane) |
| --- | --- | --- | --- | --- | --- |
| `jlcpcb-2layer-standard` | <https://jlcpcb.com/capabilities/pcb-capabilities> | 0.10 / 0.10 mm | 0.15 / 0.25 mm | 1 oz outer | 1.53 mm, εr 4.5 |
| `jlcpcb-4layer-standard` | same, and <https://jlcpcb.com/impedance> (JLC04161H-7628) | 0.09 / 0.09 mm | 0.15 / 0.25 mm | 1 oz outer, 0.5 oz inner | 0.2104 mm (7628 prepreg), εr 4.4 |
| `generic-6mil` | <https://docs.oshpark.com/services/two-layer/> (OSH Park as the representative 6/6 mil service) | 0.1524 / 0.1524 mm | 0.254 / 0.508 mm | 1 oz | 1.524 mm, εr 4.5 |

`min_*` are the fab's limits and become KiCad's board constraints;
`design` holds the values used by default (JLCPCB: 0.15 mm track and
clearance, 0.25/0.45 mm vias, the smallest at the standard price), which
must keep to the limits (checked).

### Current-based width (IPC-2221)

```yaml
board:
  nets:
    VBUS: {current: 1.0}      # A
```

Width from IPC-2221B section 6.2's curve fit:
`I = k · ΔT^0.44 · A^0.725` (I in A, A the cross-section in mil², ΔT the
rise in °C, k = 0.048 outer / 0.024 inner), ΔT = 10 °C, copper thickness
from the fab's oz (1 oz = 1.37 mil = 34.8 µm). IPC-2152 replaced this
chart with measured data and finds it conservative for most cases
(especially inner layers), but its charts have no closed form, so the
IPC-2221 fit is used. A net's class width is its outer-layer width, never
below the design track; inner layers have their own (below).

A net without a declared current takes one from the parts only when a
part states it: a `power_out` pin of a part with an `output_current`
parameter (a regulator's rated output) bounds its net. Pour nets (GND)
are carried by the pours and need no current.

Several pins of one IC on the same net share its current equally (the
RP2040's six IOVDD pins each carry a sixth of 3V3's budget); this is an
assumption, not a measurement, and only sizes the escape inside the
courtyard.

### Stackup and per-layer widths

```yaml
board:
  layers: 2
  by_layers:                 # merged over board for that layer count (--layers 4)
    4:
      fab: {preset: jlcpcb-4layer-standard}
      stackup: [signal, {plane: GND}, {plane: 3V3}, signal]   # L1 .. L4
```

`stackup` lists the copper layers top to bottom, each `signal` or
`{plane: NET}`. A plane is a zone of its net over the whole layer, and
the layer is typed `power` (KiCad and the DSN), so the router puts no
track on it and reaches it with vias. The outer layers are signal layers.
Without `stackup`: two layers both signal; four layers signal, a plane of
the board's first pour net (`zones[0]`, the ground: right under the top,
the microstrip reference of the diff pairs), signal, signal.
`--stackup signal,GND,3V3,signal` overrides it from the command line.
The board's `zones` stay pours on the outer layers.

An inner layer has half the copper of an outer one (JLCPCB: 0.5 oz
against 1 oz) and IPC-2221 halves its k, so a current needs about five
times the outer width there (1 A: 0.302 mm outer, 1.572 mm inner).
Folding that into one class width (the old behaviour: VBUS 1.575 mm on
every layer) makes the outer routing impossible, and Specctra's class
`rule` has one width. So each current net keeps two widths:

- the class width (KiCad net class, DSN `rule`): the outer width;
- the inner width, for the inner *signal* layers only (there is none
  when the inner layers are all planes): DSN `layer_rule <layer> (rule
  (width …))` in the net's class, and a `(layer inner)` rule in
  `<name>.kicad_dru`;
- a net whose inner width exceeds 0.5 mm (`INNER_MAX_WIDTH`) keeps to
  the outer layers: DSN `circuit (use_layer F.Cu B.Cu)` and a `(layer
  inner) (constraint disallow track)` rule.

KiCad's DSN export writes neither `layer_rule` nor `use_layer`; build.py
adds them (`rules.dsn_layer_rules`). Freerouting 2.5 parses both (its
`NetClass` keeps a trace width and an active flag per layer); `use_layer`
is seen to keep a class off the other layers. That it routes a class at
its `layer_rule` width is not shown yet: in the runs below Freerouting
never put a current net on the inner signal layer. The DRC rules check
the width whatever the router does.

Measured on the Pico board, four layers, one sweep of 24 router
configurations (`route --evaluate-all --rounds 1`, 2026-11-16):

| Stackup | Complete | Notes |
| --- | --- | --- |
| signal, GND plane, 3V3 plane, signal (`board.yaml`) | 9 / 24 | 9 of the 12 "inner plane" runs; the outer GND pours as planes too: 0-1 / 12 |
| signal, GND plane, signal, signal (default) | 0 / 24 | 3V3 and VBUS left with 8-19 connections in every run |
| either, before plane vias | 0 / 24 | 40-odd GND and 3V3 pads never reach their plane |

### Escape width (neck-down)

A part with a lead pitch of 0.65 mm or less, or (no pitch stated) pads
closer than a design track with its clearance on both sides, gets an
escape zone: its courtyard. Each of its SMD pads escapes along its long
side, away from the land-pattern centroid. The widest escape track is

```text
w ≤ wp                                   (never wider than the pad)
w ≤ wp + 2·(g − c)                       (clear of each pad q beside the corridor;
                                          g the edge gap, c the clearance)
w_p/2 + w_q/2 ≤ d − c                    (clear of a neighbour's parallel stub,
                                          d the centre distance; split as
                                          w_p ≤ 2(d − c) − min(w_q⁰, d − c))
```

which for equal pads at pitch p is the textbook
`min(wp, 2·(p − c) − wp)`. The pads' actual extents come from the
Placement Model, so uneven rows (a USB-C receptacle's wide VBUS/GND pads
between 0.3 mm signal pads) are handled. Below the fab's minimum track
it is retried at the fab's minimum clearance (with a matching clearance
rule in the zone), and failing that it is an error. The escape must also
carry the pad's share of its net's current.

### `<name>.kicad_dru`

KiCad custom rules (KiCad 7 or later; checked with KiCad 10.0): the
current width per net (outer, and inner where the stackup has an inner
signal layer; `disallow track` inside for a net too wide there), the diff pairs'
width and gap, then per fine-pitch part and group of nets
`A.intersectsCourtyard('U1')` with `track_width (min need) (opt w) (max w)`.
Later rules take precedence, so the neck-down overrides the current width
inside the courtyard. Example (Pico board):

```text
(rule "current VBUS 1 A"
  (layer outer)
  (condition "A.NetName == 'VBUS'")
  (constraint track_width (min 0.302mm) (opt 0.305mm)))
(rule "U1 escape 0.200mm: 1V1, 3V3, GND, GP0, GP1, GP10 ..."
  (condition "A.intersectsCourtyard('U1') && (A.NetName == '1V1' || ...)")
  (constraint track_width (min 0.100mm) (opt 0.200mm) (max 0.200mm)))
```

The project's net classes (one per net with a current, one per diff pair)
carry the same widths for the router.

### Fan-out

Freerouting reads neither the custom rules nor diff pairs, so the pcb
stage places locked copper before routing (`rules.fanout`):

- every escaping pad gets a straight stub at its escape width to just
  outside the courtyard (then the net's own width);
- a dog-bone via beyond the stub (straight out, or after a 45° jog,
  nearest free spot first) when the net's other surface pads are all on
  the other side (decoupling capacitors and a flash under the QFN) or it
  is a pour net; through-hole targets are reachable from either side and
  stay straight. No dog-bone goes into the 0.8 mm kept clear ahead of a
  straight stub. `placement.yaml` `fanout: {U1: {via: [...], straight: [...]}}`
  (pad numbers or nets) overrides the choice;
- a grid of thermal vias in an exposed pad of a pour net;
- the diff pairs' coupled paths (below), before the dog-bones;
- plane vias: every other surface pad of a plane net (stackup) gets a
  via of its own just beside it (away from its part's centre first,
  never in the pad), with a track from the pad's centre. Freerouting does
  not connect pads to a plane on a `power` layer by itself: on the
  four-layer Pico board, without these, all 24 configurations left 40-odd
  GND and 3V3 connections unrouted.

`placement.yaml` `fanout:` also takes `dogbone: [pour, other_side]`
(which automatic reasons apply), `stubs: false` (stubs only where a diff
pair starts, or for a plane net's pad, which needs its via) and
`pairs: false` (pairs left to the router).
`--no-fanout` leaves all of it out, and `route --evaluate-all --rounds 1`
scores all 24 router configurations of one sweep, for comparing.

### Differential pairs

Pins carry `diff_pair` in the data (Canonical Data Specification 14.2),
and the Placement Model gives each pad's partner pad. Two nets on the
partner pads of one part form a pair. Name them `<base>+`/`<base>-` (or
`P`/`N`) so KiCad recognises the pair for its gap check.

The standard's impedance is the board tool's (USB 2.0: 90 Ω ±15 %
differential, Universal Serial Bus Specification rev 2.0, chapter 7), not
a part fact. Width and gap come from the edge-coupled surface microstrip
approximation of IPC-2141A:

```text
Z0    = 87 / √(εr + 1.41) · ln(5.98 h / (0.8 w + t))      (0.1 < w/h < 2, 1 < εr < 15)
Zdiff = 2 · Z0 · (1 − 0.48 · e^(−0.96 s/h))
```

solved for w at s = the design clearance. It is an approximation (no
solder mask, no coplanar ground). Inside the pins' courtyard the traces
neck down to the escape width; outside, a width up to 0.5 mm (or the
escape width, if wider) is taken as practical. When the solution is not
(out of the formula's range, or wider, as on a 1.6 mm two-layer board,
where 90 Ω needs ~1.2 mm tracks), the pair is coupled at the escape width
and pin-pitch gap, and the estimated impedance is reported. Full-speed
USB (12 Mbit/s, the RP2040's) over a few centimetres is not
impedance-critical.

The microstrip's h and εr are the fab profile's trace-to-plane
dielectric: on four layers L1 to L2 (JLC04161H-7628: one 7628 prepreg,
0.2104 mm, εr 4.4), so L2 must be the ground plane (the default
stackup's); it gives 0.265 mm at a 0.150 mm gap, about 89 Ω. That is
wider than 0.4 mm-pitch pins allow where the stubs end (0.4 − 0.265 =
0.135 mm < 0.15 mm), so the first segment of each trace, from its stub
to the coupled lanes (width + gap apart), keeps the escape width; on four
layers the coupled path failed for exactly this reason before.

Freerouting 2.5 has no differential-pair routing, so the pair is
pre-placed: both traces as offsets of one octilinear centreline from the
pins' stubs to the next part's pads, with a breakout at the end; when no
clear path exists it is left to the router and reported. A receptacle
pair bridged inside its footprint (USB-C A6/B6 and A7/B7,
`bridge_pads_in_footprints`) is not coupled: the bridge already decides
its copper. The check stage reports each pair's track lengths and skew
(mm, and ps with the microstrip's effective permittivity).

### Checks

`check` runs ERC and DRC (with the custom rules) and then compares every
track with its net's current width outside the escape zones (inside: its
pad share's), and writes `out/routing-check.json` with the violations and
the diff-pair lengths.

It renders the board top and bottom (`out/<name>-top.png`, `-bottom.png`)
and each copper layer on its own (`out/<name>-<layer>.svg`, and `.png`
where ImageMagick is installed), the inner planes included.
