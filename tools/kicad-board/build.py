#!/usr/bin/env python3
"""Builds a KiCad project from a board description (YAML) whose parts all
come from an OpenParts registry.

  build.py BOARD.yaml --out DIR [--stage fetch|sch|pcb|route|check|all|preview]
           [--data OPENPARTS_DATA_DIR] [--layers 2|4] [--sweep N] [--evaluate-all]

Stages (each needs the ones before it):
  fetch  install every part into DIR's own libraries (`openparts
         kicad-install`: symbol, footprint and STEP from the registry,
         or built from a local openparts-data checkout with --data)
  sch    write DIR/<name>.kicad_sch: each symbol placed on a grid, a
         global net label on every connected pin (so the schematic's net
         names equal the board's, without a sheet prefix), no-connect
         flags on open ones
  pcb    write DIR/<name>.kicad_pcb (KiCad's pcbnew, in its Flatpak):
         footprints at their place in DIR/placement.yaml (or a grid),
         nets, board outline, design rules, zones; footprints whose
         silkscreen had to be clipped to the outline go to the
         project's own `board` library and the schematic's footprint
         fields follow (DRC's library and parity checks then agree);
         every part's Placement Model (GET .../placement) gives its
         KiCad transform and the placement check: courtyard overlaps,
         parts off the board, board-edge statements and keep-outs.
         placement.yaml: {REF: {x, y, rot, side}} in mm from the board
         centre, Y up; a part lands at T · R(rot) · M (M mirrors x -> -x
         in the part's frame on the bottom side; rot is counter-clockwise
         viewed from the top on both sides) -- docs/spec/placement-model.md
  route  route it with Freerouting (Specctra DSN/SES), in parallel over
         ROUTER_VARIANTS cost settings x two plane modes (pours as
         planes on every layer, or only on the top / inner ones), and
         keep the result with the fewest unrouted connections plus pour
         pads cut off; with none complete, further sweeps with other
         settings follow (ROUTER_ROUNDS); --evaluate-all scores every
         result and reports how many are complete, --sweep N tries only
         the first N settings per sweep
  check  ERC, DRC with schematic parity, and renders under DIR/out/
  preview  `pcb` plus an unrouted render and DRC's placement findings

Routing rules (rules.py, README.md): `board.fab` names a fab profile
(fab/*.yaml: the fab's published limits and the design values used),
`board.nets: {NET: {current: A}}` the supply currents. The pcb stage
derives the IPC-2221 width per net, the escape width per fine-pitch pad,
the diff pairs' width and gap, writes them as net classes and as
<name>.kicad_dru (neck-down inside each fine-pitch courtyard), and places
the fan-out (escape stubs, dog-bone and thermal vias, coupled diff
pairs) before routing; Freerouting's own neck-down stays off. `check`
also compares the tracks with the current widths and reports the diff
pairs' skew.

The netlist is the board description, so the schematic and the board
are built from the same data and agree by construction (DRC's parity
check confirms it).
"""

import argparse
import json
import math
import os
import re
import shutil
import subprocess
import sys
import uuid
from pathlib import Path

import yaml

import rules

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
KICAD = ["flatpak", "run", "--command=sh", "org.kicad.KiCad", "-c"]
FREEROUTING_VERSION = "2.5.0"
GRID = 1.27


# ---------------------------------------------------------------- spec


def load_spec(path):
    spec = yaml.safe_load(Path(path).read_text())
    spec["components"] = {k: str(v) for k, v in spec["components"].items()}
    return spec


def mpn_of(part):
    return part.split("/", 1)[1]


def safe(name):
    return re.sub(r"[/\\ (),@]", "-", name)


def pin_refs(spec):
    """{(ref, pin): net} from the net list."""
    out = {}
    for net, members in spec["nets"].items():
        for m in members:
            ref, pin = m.split(".", 1)
            if (ref, pin) in out:
                sys.exit(f"{m} is in both {out[(ref, pin)]} and {net}")
            out[(ref, pin)] = net
    return out


# --------------------------------------------------------------- fetch


def openparts_cli():
    if os.environ.get("OPENPARTS_CLI"):
        return [os.environ["OPENPARTS_CLI"]]
    built = REPO / "target" / "debug" / "openparts"
    return [str(built)] if built.exists() else ["cargo", "run", "-q", "-p", "openparts-cli", "--"]


def stage_fetch(spec, out, data=None):
    parts = sorted(set(spec["components"].values()))
    cmd = openparts_cli() + ["--registry", spec["registry"], "kicad-install", "--project", str(out)]
    if data:
        cmd += ["--data", str(Path(data).resolve())]
    subprocess.run(cmd + parts, check=True, cwd=REPO)


# ----------------------------------------------------- placement model
#
# Every part's Placement Model (docs/spec/placement-model.md) from the
# registry: its pads, courtyard, body, board-edge and keep-out facts in
# its own frame (mm, Y up, origin at the land-pattern centroid, pin 1
# upper left). placement.yaml places a part as the model defines it:
# board = T · R(rot) · M, M mirroring x -> -x on the bottom side, rot
# counter-clockwise viewed from the top on either side.


def placement_models(spec, out):
    """{part: model} at the board's density (board.density, default N),
    fetched once per part; also written to DIR/placement-models.json."""
    from urllib.parse import quote
    from urllib.request import urlopen

    density = spec["board"].get("density", "N")
    models = {}
    for part in sorted(set(spec["components"].values())):
        mfr, mpn = part.split("/", 1)
        url = f"{spec['registry']}/v1/parts/{quote(mfr, safe='')}/{quote(mpn, safe='')}/placement?density={density}"
        with urlopen(url) as r:
            models[part] = json.loads(r.read())
    (out / "placement-models.json").write_text(json.dumps(models, indent=1))
    return models


place = rules.place


def normalize(deg):
    d = math.fmod(deg, 360.0)
    if d <= -180.0:
        d += 360.0
    elif d > 180.0:
        d -= 360.0
    return d + 0.0


def to_kicad(p):
    """KiCad's terms for a placement (spec section 2.4): orientation θ on
    the top, θ + 180° flipped on the bottom. Position stays in board mm,
    Y up (pcb_stage turns it into KiCad's Y-down page)."""
    bottom = p.get("side", "top") == "bottom"
    return {"x": p.get("x", 0.0), "y": p.get("y", 0.0),
            "orientation": normalize(float(p.get("rot", 0.0)) + (180.0 if bottom else 0.0)),
            "flipped": bottom}


def courtyards(model, p):
    """The model's courtyard polygons on the board."""
    return [[place(p, (q["x"], q["y"])) for q in poly] for poly in model["courtyard"]["polygons"]]


def box_of(polys):
    pts = [q for poly in polys for q in poly]
    return (min(q[0] for q in pts), min(q[1] for q in pts), max(q[0] for q in pts), max(q[1] for q in pts))


def area(poly):
    return abs(sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(poly, poly[1:] + poly[:1]))) / 2.0


def clip(subject, clipper):
    """Sutherland-Hodgman: subject ∩ clipper (clipper convex)."""
    sign = 1.0 if sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(clipper, clipper[1:] + clipper[:1])) > 0 else -1.0
    out = subject
    for a, b in zip(clipper, clipper[1:] + clipper[:1]):
        if not out:
            break

        def inside(q):
            return sign * ((b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0])) > 1e-9

        def cross(p, q):
            dx1, dy1 = q[0] - p[0], q[1] - p[1]
            dx2, dy2 = b[0] - a[0], b[1] - a[1]
            t = ((a[0] - p[0]) * dy2 - (a[1] - p[1]) * dx2) / (dx1 * dy2 - dy1 * dx2)
            return (p[0] + t * dx1, p[1] + t * dy1)

        pts, out = out, []
        for p, q in zip(pts, pts[1:] + pts[:1]):
            if inside(q):
                if not inside(p):
                    out.append(cross(p, q))
                out.append(q)
            elif inside(p):
                out.append(cross(p, q))
    return out


def overlap(a, b):
    """Shared area of two convex polygons (courtyards are)."""
    common = clip(a, b)
    return area(common) if len(common) >= 3 else 0.0


def placement_plan(spec, models, placement):
    """Per reference: the placement (placement.yaml, or a spot in a row
    beside the board), its KiCad transform, its pads and courtyard on the
    board, and the model hash."""
    w, h = spec["board"]["width"], spec["board"]["height"]
    spare_x = w / 2 + 5.0
    plan = {}
    for ref, part in spec["components"].items():
        m = models[part]
        p = placement.get(ref)
        if not p:
            x0, y0, x1, y1 = box_of(m["courtyard"]["polygons"] and
                                    [[(q["x"], q["y"]) for q in poly] for poly in m["courtyard"]["polygons"]])
            p = {"x": spare_x - x0, "y": h / 2 + 10 - (y0 + y1) / 2, "rot": 0.0, "side": "top"}
            spare_x += x1 - x0 + 2.0
        cy = courtyards(m, p)
        plan[ref] = {
            "placement": p,
            "kicad": to_kicad(p),
            "pads": [[pad["number"], *place(p, (pad["position"]["x"], pad["position"]["y"]))] for pad in m["pads"]],
            "courtyard_box": box_of(cy),
            "hash": m["hash"],
        }
    return plan


def check_placement(spec, models, plan):
    """The placement report, from the Placement Models: courtyard
    overlaps (per side), parts off the board, board-edge statements and
    keep-outs. Returns the number of problems."""
    w, h = spec["board"]["width"], spec["board"]["height"]
    problems = 0
    shapes = {ref: courtyards(models[spec["components"][ref]], e["placement"]) for ref, e in plan.items()}
    refs = sorted(plan)
    for i, a in enumerate(refs):
        for b in refs[i + 1:]:
            if plan[a]["placement"].get("side", "top") != plan[b]["placement"].get("side", "top"):
                continue
            shared = sum(overlap(pa, pb) for pa in shapes[a] for pb in shapes[b])
            if shared > 1e-6:
                print(f"courtyards overlap: {a} {b} ({shared:.3f} mm²)")
                problems += 1
    for ref in refs:
        m = models[spec["components"][ref]]
        p = plan[ref]["placement"]
        edge = m.get("edge")
        # The board edge the part's edge side faces (by its outward normal).
        facing = None
        if edge:
            n = {"+x": (1, 0), "-x": (-1, 0), "+y": (0, 1), "-y": (0, -1)}[edge["side"]]
            o = place(p, (0, 0))
            q = place(p, n)
            nb = (q[0] - o[0], q[1] - o[1])
            facing = max([("+x", nb[0]), ("-x", -nb[0]), ("+y", nb[1]), ("-y", -nb[1])], key=lambda t: t[1])
            board_at = {"+x": w / 2, "-x": w / 2, "+y": h / 2, "-y": h / 2}[facing[0]]
            if facing[1] < 0.999:
                print(f"edge: {ref} {edge['side']} side does not face a board edge squarely")
                problems += 1
            elif edge.get("pcb_edge") is not None:
                line = place(p, (n[0] * edge["pcb_edge"], n[1] * edge["pcb_edge"]))
                at = {"+x": line[0], "-x": -line[0], "+y": line[1], "-y": -line[1]}[facing[0]]
                off = at - board_at
                state = "on the board edge" if abs(off) < 0.01 else (
                    f"{off:.2f} mm beyond the board edge" if off > 0 else f"{-off:.2f} mm inside the board edge")
                print(f"edge: {ref} datasheet PCB edge line {state} (body overhang past it {edge['overhang']:.2f} mm)")
                if abs(off) >= 0.01 and edge["requirement"] == "required":
                    problems += 1
            else:
                reach = max(({"+x": x, "-x": -x, "+y": y, "-y": -y}[facing[0]]
                             for poly in shapes[ref] for x, y in poly))
                print(f"edge: {ref} should sit at the board edge ({edge['requirement']}, {facing[0]} side); "
                      f"its courtyard reaches {board_at - reach:.2f} mm short of it")
        # Off the board: copper and body inside the outline (a problem),
        # the courtyard too (a note: a header as long as the board), except
        # on the side an edge part faces out of.
        def past(points):
            sides = set()
            for x, y in points:
                over = {"+x": x - w / 2, "-x": -w / 2 - x, "+y": y - h / 2, "-y": -h / 2 - y}
                sides |= {s for s, d in over.items() if d > 1e-6 and not (facing and s == facing[0])}
            return sorted(sides)

        solid = [place(p, (q["x"], q["y"])) for r in m["body"]["regions"] for q in r["outline"]]
        for pad in m["pads"]:
            hx, hy = pad["size"]["x"] / 2, pad["size"]["y"] / 2
            cx, cy_ = pad["position"]["x"], pad["position"]["y"]
            solid += [place(p, (cx + sx * hx, cy_ + sy * hy)) for sx in (-1, 1) for sy in (-1, 1)]
        if past(solid):
            print(f"off-board: {ref} (pads or body past the {'/'.join(past(solid))} edge)")
            problems += 1
        elif past(q for poly in shapes[ref] for q in poly):
            print(f"note: {ref}'s courtyard reaches past the {'/'.join(past(q for poly in shapes[ref] for q in poly))}"
                  " edge (pads and body are on the board)")
        for k in m.get("keepouts", []):
            if k.get("polygon"):
                area_poly = [place(p, (q["x"], q["y"])) for q in k["polygon"]]
                if "components" in k["restricts"]:
                    for other in refs:
                        if other != ref and any(overlap(area_poly, pb) > 1e-6 for pb in shapes[other]):
                            print(f"keep-out: {other} inside {ref}'s {k['purpose']} keep-out")
                            problems += 1
            else:
                size = " x ".join(f"{v:g}" for v in k.get("size") or [])
                print(f"keep-out: {ref} {k['purpose']} keep-out {size} mm at its {k.get('side') or '?'} side, "
                      f"restricting {', '.join(k['restricts'])}: not located by the datasheet, check by hand")
    print(f"placement check: {problems} problem(s)")
    return problems


def apply_fab(spec, preset=None):
    """The fab profile (board.fab, `--fab` overriding its preset) checked
    and put in spec["fab"]; board.rules becomes its design values (the
    track, clearance and via the pcb stage uses)."""
    board = spec["board"]
    if preset:
        board["fab"] = {**(board.get("fab") or {}), "preset": preset}
    if not board.get("fab"):
        sys.exit("board.fab is required: a fab profile (tools/kicad-board/fab/*.yaml), e.g. "
                 "`fab: {preset: jlcpcb-2layer-standard}`")
    try:
        fab = rules.load_fab(board)
    except ValueError as e:
        sys.exit(str(e))
    if fab.get("layers") and fab["layers"] != board.get("layers", 2):
        sys.exit(f"fab profile {fab.get('preset')} is for {fab['layers']} layers, the board has "
                 f"{board.get('layers', 2)} (pick another with --fab)")
    d = fab["design"]
    board["rules"] = {"track": d["track"], "clearance": d["clearance"],
                      "via_diameter": d["via_diameter"], "via_drill": d["via_drill"]}
    spec["fab"] = fab
    try:
        spec["stackup"] = rules.load_stackup(board, set(spec["nets"]))
    except ValueError as e:
        sys.exit(str(e))


def apply_layers(spec, layers=None):
    """`--layers` over board.layers, then board.by_layers.<n> (the fab
    preset and stackup for that layer count, say) merged over board."""
    board = spec["board"]
    if layers:
        board["layers"] = layers
    extra = (board.pop("by_layers", None) or {}).get(board.get("layers", 2)) or {}
    board.update(extra)


def routing_of(spec, out):
    """The routing rules the pcb stage derived (out/routing.json), or
    those from the declared currents alone (before it ran)."""
    f = out / "routing.json"
    if f.exists():
        return json.loads(f.read_text())
    widths = rules.net_widths(spec["fab"], rules.net_currents(spec["board"]), spec["stackup"])
    return {"widths": widths, "pairs": []}


def net_classes(spec, routing):
    """{class name: {track, nets, [diff_pair_width, diff_pair_gap]}}:
    one class per net with a current (its IPC-2221 width) and one per
    diff pair (its width and gap)."""
    classes = {}
    for net, w in sorted(routing["widths"].items()):
        classes[f"I_{net}"] = {"track": w["track"], "nets": [net]}
    for p in routing.get("pairs", []):
        g = p.get("geometry")
        if g:
            classes[f"DP_{p['p']}"] = {"track": g["width"], "nets": [p["p"], p["n"]],
                                       "diff_pair_width": g["width"], "diff_pair_gap": g["gap"]}
    return classes


# --------------------------------------------------------- s-expressions


def sexp_parse(text):
    """A minimal S-expression reader: lists, atoms, quoted strings."""
    tokens = re.findall(r'\(|\)|"(?:[^"\\]|\\.)*"|[^\s()"]+', text)
    stack = [[]]
    for t in tokens:
        if t == "(":
            stack.append([])
        elif t == ")":
            done = stack.pop()
            stack[-1].append(done)
        else:
            stack[-1].append(t)
    return stack[0]


def unquote(s):
    return s[1:-1].replace('\\"', '"') if s.startswith('"') else s


def find_block(text, marker):
    """(start, end) of the balanced block starting with `marker`."""
    start = text.find(marker)
    if start < 0:
        return None
    depth, i = 0, start
    in_str = False
    while i < len(text):
        c = text[i]
        if in_str:
            if c == "\\":
                i += 1
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return start, i + 1
        i += 1
    return None


def symbol_pins(lib_text, name):
    """{unit: [(number, x, y, angle)]} for symbol `name`, and its block."""
    span = find_block(lib_text, f'(symbol "{name}"')
    if not span:
        sys.exit(f"symbol {name} not in openparts.kicad_sym")
    block = lib_text[span[0]:span[1]]
    tree = sexp_parse(block)[0]
    units = {}
    for sub in tree:
        if isinstance(sub, list) and sub and sub[0] == "symbol":
            unit = int(unquote(sub[1]).rsplit("_", 2)[1])
            for item in sub:
                if isinstance(item, list) and item and item[0] == "pin":
                    at = next(x for x in item if isinstance(x, list) and x[0] == "at")
                    number = next(x for x in item if isinstance(x, list) and x[0] == "number")
                    units.setdefault(unit, []).append(
                        (unquote(number[1]), float(at[1]), float(at[2]), float(at[3]))
                    )
    return units, block


# ------------------------------------------------------------------ sch


def uid():
    return str(uuid.uuid4())


def snap(v):
    return round(round(v / GRID) * GRID, 4)


def stage_sch(spec, out):
    name = spec["name"]
    lib = (out / "openparts.kicad_sym").read_text()
    nets = pin_refs(spec)
    no_connect = {tuple(p.split(".", 1)) for p in spec.get("no_connect", [])}
    root = uid()

    lib_symbols = {}
    placed = []  # (ref, part, unit, x, y, pins)
    # Lay the symbols out in rows, largest first.
    infos = []
    for ref, part in spec["components"].items():
        mpn = mpn_of(part)
        units, block = symbol_pins(lib, mpn)
        lib_symbols[mpn] = block
        for unit, pins in sorted(units.items()):
            if unit == 0 and len(units) > 1:
                continue
            all_pins = pins + (units.get(0, []) if unit == 1 and len(units) > 1 else [])
            xs = [p[1] for p in all_pins] or [0]
            ys = [p[2] for p in all_pins] or [0]
            w = max(xs) - min(xs) + 25.4
            h = max(ys) - min(ys) + 12.7
            infos.append((ref, part, unit, all_pins, w, h, min(xs), max(ys)))
    infos.sort(key=lambda i: (-i[5], i[0]))
    x, y, row_h = 25.4, 25.4, 0.0
    page_w = 800.0
    for ref, part, unit, pins, w, h, minx, maxy in infos:
        if x + w > page_w:
            x, y, row_h = 25.4, y + row_h, 0.0
        ox, oy = snap(x - minx + 12.7), snap(y + maxy + 6.35)
        placed.append((ref, part, unit, ox, oy, pins))
        x += w
        row_h = max(row_h, h)

    o = []
    o.append('(kicad_sch (version 20231120) (generator "openparts") (generator_version "0.1")')
    o.append(f'  (uuid "{root}")')
    o.append('  (paper "A0")')
    o.append("  (lib_symbols")
    for mpn, block in lib_symbols.items():
        o.append("    " + block.replace(f'(symbol "{mpn}"', f'(symbol "openparts:{mpn}"', 1))
    if spec.get("power_flags"):
        o.append(pwr_flag_lib())
    o.append("  )")
    used = set()
    flag_n = 0
    for ref, part, unit, ox, oy, pins in placed:
        mpn = mpn_of(part)
        su = uid()
        o.append(
            f'  (symbol (lib_id "openparts:{mpn}") (at {ox:.2f} {oy:.2f} 0) (unit {unit})'
            f" (in_bom yes) (on_board yes) (dnp no) (uuid \"{su}\")"
        )
        o.append(f'    (property "Reference" "{ref}" (at {ox:.2f} {oy - 3:.2f} 0) (effects (font (size 1.27 1.27))))')
        o.append(f'    (property "Value" "{mpn}" (at {ox:.2f} {oy + 3:.2f} 0) (effects (font (size 1.27 1.27))))')
        o.append(
            f'    (property "Footprint" "{footprint_ids(spec, out)[ref]}" (at {ox:.2f} {oy:.2f} 0)'
            " (effects (font (size 1.27 1.27)) hide))"
        )
        for number, *_ in pins:
            o.append(f'    (pin "{number}" (uuid "{uid()}"))')
        o.append(f'    (instances (project "{name}" (path "/{root}" (reference "{ref}") (unit {unit}))))')
        o.append("  )")
        # A label (or no-connect) on every pin, once per position.
        for number, px, py, angle in pins:
            ax, ay = snap(ox + px), snap(oy - py)
            if (ax, ay) in used:
                continue
            used.add((ax, ay))
            net = nets.get((ref, number))
            if net is None:
                if (ref, number) in no_connect or True:
                    o.append(f'  (no_connect (at {ax:.2f} {ay:.2f}) (uuid "{uid()}"))')
                continue
            # The label reads away from the symbol.
            label_angle = {0: 180, 180: 0, 90: 270, 270: 90}[int(angle) % 360]
            o.append(global_label(net, ax, ay, label_angle))
    # Power flags: these nets are fed from outside / by passive pins.
    fx, fy = 25.4, snap(y + row_h + 25.4)
    for net in spec.get("power_flags", []):
        flag_n += 1
        o.append(
            f'  (symbol (lib_id "power:PWR_FLAG") (at {fx:.2f} {fy:.2f} 0) (unit 1) (in_bom no) (on_board no)'
            f' (dnp no) (uuid "{uid()}")'
        )
        o.append(f'    (property "Reference" "#FLG0{flag_n}" (at {fx:.2f} {fy - 2.54:.2f} 0) (effects (font (size 1.27 1.27)) hide))')
        o.append(f'    (property "Value" "PWR_FLAG" (at {fx:.2f} {fy - 3.81:.2f} 0) (effects (font (size 1.27 1.27))))')
        o.append(f'    (pin "1" (uuid "{uid()}"))')
        o.append(f'    (instances (project "{name}" (path "/{root}" (reference "#FLG0{flag_n}") (unit 1))))')
        o.append("  )")
        o.append(global_label(net, fx, fy, 270))
        fx += 15.24
    o.append('  (sheet_instances (path "/" (page "1")))')
    o.append(")")
    (out / f"{name}.kicad_sch").write_text("\n".join(o) + "\n")
    write_project(spec, out)


def footprint_ids(spec, out):
    """{ref: library id} of each part's footprint: the project library's
    `openparts:<mpn>`, or the board's own fitted copy (silkscreen clipped
    to the outline) that the pcb stage listed in footprints.json."""
    ids = {ref: f"openparts:{safe(mpn_of(p))}" for ref, p in spec["components"].items()}
    fitted = out / "footprints.json"
    if fitted.exists():
        ids.update({r: i for r, i in json.loads(fitted.read_text()).items() if r in ids})
    return ids


def sync_sch_footprints(spec, out):
    """The schematic's footprint fields set to footprint_ids (the pcb
    stage may have fitted some), so DRC's parity check agrees."""
    sch = out / f"{spec['name']}.kicad_sch"
    if not sch.exists():
        return
    ids, ref, lines = footprint_ids(spec, out), None, []
    for line in sch.read_text().splitlines():
        m = re.match(r'\s*\(property "Reference" "([^"]*)"', line)
        if m:
            ref = m.group(1)
        if ref in ids:
            line = re.sub(r'^(\s*\(property "Footprint" )"[^"]*"', lambda m: f'{m.group(1)}"{ids[ref]}"', line)
        lines.append(line)
    sch.write_text("\n".join(lines) + "\n")


def global_label(net, x, y, angle):
    """A global label: its net is named `net` on the board too (a local
    label would name it `/net`, and DRC's parity check would disagree)."""
    justify = "right" if angle in (180, 270) else "left"
    return (
        f'  (global_label "{net}" (shape passive) (at {x:.2f} {y:.2f} {angle}) (fields_autoplaced yes)'
        f' (effects (font (size 1.27 1.27)) (justify {justify})) (uuid "{uid()}")'
        f' (property "Intersheetrefs" "${{INTERSHEET_REFS}}" (at {x:.2f} {y:.2f} 0)'
        f" (effects (font (size 1.27 1.27)) (justify {justify}) hide)))"
    )


def pwr_flag_lib():
    """KiCad's own `power:PWR_FLAG`, from its standard library (so ERC
    finds the schematic's copy identical to the library's)."""
    text = subprocess.run(
        KICAD + ["cat /app/extensions/Library/symbols/power.kicad_sym"],
        check=True, capture_output=True, text=True,
    ).stdout
    span = find_block(text, '(symbol "PWR_FLAG"')
    return "    " + text[span[0]:span[1]].replace('(symbol "PWR_FLAG"', '(symbol "power:PWR_FLAG"', 1)


def write_project(spec, out):
    """The project file: the fab's minimums as the board constraints, the
    design values as the default net class, the current and diff-pair
    classes (the router's widths; DRC's are in <name>.kicad_dru)."""
    pro = out / f"{spec['name']}.kicad_pro"
    r = spec["board"]["rules"]
    fab = spec["fab"]
    classes = net_classes(spec, routing_of(spec, out))

    def netclass(name, c):
        return {
            "name": name,
            "clearance": r["clearance"],
            "track_width": c["track"],
            "via_diameter": r["via_diameter"],
            "via_drill": r["via_drill"],
            "microvia_diameter": 0.3,
            "microvia_drill": 0.1,
            "diff_pair_width": c.get("diff_pair_width", r["track"]),
            "diff_pair_gap": c.get("diff_pair_gap", r["clearance"]),
            "diff_pair_via_gap": 0.25,
            "wire_width": 6,
            "bus_width": 12,
            "line_style": 0,
            "schematic_color": "rgba(0, 0, 0, 0.000)",
            "pcb_color": "rgba(0, 0, 0, 0.000)",
        }

    project = {
        "board": {
            "design_settings": {
                "rules": {
                    # The fab's limits (a narrower escape inside a
                    # fine-pitch courtyard is the custom rules' business).
                    "min_clearance": fab["min_clearance"],
                    "min_track_width": fab["min_track"],
                    "min_via_diameter": fab["min_via_diameter"],
                    "min_via_annular_width": fab.get("min_annular_ring", 0.05),
                    "min_through_hole_diameter": fab["min_via_drill"],
                    # KiCad's silk-to-edge check uses the copper edge
                    # clearance: not below 0.25 mm.
                    "min_copper_edge_clearance": max(0.25, fab.get("min_edge_clearance", 0.25)),
                    "min_hole_clearance": max(0.25, fab.get("min_hole_clearance", 0.25)),
                }
            }
        },
        "meta": {"filename": pro.name, "version": 3},
        "net_settings": {
            "classes": [netclass("Default", {"track": r["track"]})]
            + [netclass(name, c) for name, c in classes.items()],
            "meta": {"version": 4},
            "netclass_patterns": [
                {"netclass": name, "pattern": n} for name, c in classes.items() for n in c["nets"]
            ],
        },
    }
    pro.write_text(json.dumps(project, indent=2) + "\n")


# ------------------------------------------------------------------ pcb


def kicad_python(script, *args, capture=False, quiet=False):
    """Runs `script` with KiCad's own Python (pcbnew), in its Flatpak;
    returns its output when `capture` (it is echoed either way)."""
    cmd = " ".join(["python3", sh_quote(str(script))] + [sh_quote(str(a)) for a in args])
    run = subprocess.run(
        ["flatpak", "run", f"--filesystem={REPO}", f"--filesystem={args[0] if args else REPO}",
         "--command=sh", "org.kicad.KiCad", "-c", cmd],
        check=True, capture_output=capture, text=True,
    )
    if capture:
        out = "\n".join(l for l in run.stdout.splitlines() if not l.startswith("swig/python detected"))
        if not quiet:
            print(out, flush=True)
        return out
    return None


def sh_quote(s):
    return "'" + s.replace("'", "'\\''") + "'"


def unconnected_nets(spec, out):
    """{"REF.PIN": net} for the pins left open: the single-pin net KiCad
    gives a pin with a no-connect flag (`unconnected-(U1-SWD-Pad25)`),
    so that DRC's schematic parity finds the same net on the pad."""
    lib = (out / "openparts.kicad_sym").read_text()
    used = pin_refs(spec)
    nets = {}
    for ref, part in spec["components"].items():
        span = find_block(lib, f'(symbol "{mpn_of(part)}"')
        for pin in re.finditer(r'\(name "((?:[^"\\]|\\.)*)".*?\(number "((?:[^"\\]|\\.)*)"',
                               lib[span[0]:span[1]], re.S):
            name, number = pin.group(1), pin.group(2)
            if (ref, number) in used:
                continue
            label = "" if name in ("", "~") or name == number else f"-{name}"
            nets[f"{ref}.{number}"] = f"unconnected-({ref}{label}-Pad{number})".replace("/", "{slash}")
    return nets


def derived_currents(spec, models, nets, declared):
    """Currents for nets with none declared, from the parts' data: a net
    driven by a `power_out` pin of a part stating `output_current` (a
    regulator's rated output) carries at most that. Only an existing
    parameter is used; nothing is assumed."""
    from urllib.parse import quote
    from urllib.request import urlopen

    out, cache = {}, {}
    for ref, part in spec["components"].items():
        for pad in models[part]["pads"]:
            net = nets.get((ref, pad["number"]))
            if not net or net in declared or net in out or (pad.get("pin") or {}).get("type") != "power_out":
                continue
            if part not in cache:
                mfr, mpn = part.split("/", 1)
                with urlopen(f"{spec['registry']}/v1/parts/{quote(mfr, safe='')}/{quote(mpn, safe='')}") as r:
                    cache[part] = json.loads(r.read()).get("parameters") or {}
            amps = rules.current_from_parameter(cache[part].get("output_current"))
            if amps:
                out[net] = (amps, f"{ref} output_current")
    return out


def routing_rules(spec, models, placement, out):
    """Derives the routing rules (rules.py) and writes them: routing.json,
    <name>.kicad_dru and the fan-out for plan.json. Exits on a pad no
    escape fits."""
    fab = spec["fab"]
    nets = pin_refs(spec)
    layers = spec["board"].get("layers", 2)
    declared = rules.net_currents(spec["board"])
    currents = rules.net_currents(spec["board"], derived_currents(spec, models, nets, declared))
    widths = rules.net_widths(fab, currents, spec["stackup"])
    escapes, errors = rules.escape_plan(spec, models, placement, nets, fab, widths)
    pairs = rules.diff_pairs(spec, models, nets)
    for p in pairs:
        pins = [(ref, pad) for ref, pp, nn in p["ends"] for pad in (pp, nn)]
        fit = [escapes[ref]["pads"][pad]["width"] for ref, pad in pins
               if ref in escapes and pad in escapes[ref]["pads"]]
        pitch = min((models[spec["components"][ref]].get("pitch") or 9.0) for ref, _ in pins)
        p["geometry"] = rules.pair_geometry(fab, p["standard"], min(fit) if fit else fab["design"]["track"],
                                            pitch if pitch < 9.0 else None)
    src = fab.get("source") or {}
    print(f"fab: {fab.get('preset', 'inline')} ({src.get('publisher')}, {src.get('url')}, "
          f"retrieved {src.get('retrieved')}): min track/clearance {fab['min_track']}/{fab['min_clearance']} mm, "
          f"via {fab['min_via_drill']}/{fab['min_via_diameter']} mm; design track {fab['design']['track']}, "
          f"clearance {fab['design']['clearance']}, via {fab['design']['via_drill']}/{fab['design']['via_diameter']}")
    print("stackup: " + ", ".join(f"{s['layer']} {s['role']}" + (f" {s['net']}" if s["net"] else "")
                                  for s in spec["stackup"]))
    for net, w in sorted(widths.items()):
        inner = ""
        if w["inner"] is not None:
            inner = f"; inner {w['inner']:.3f} mm -> {w['inner_track']:.3f} mm"
            if all(name in rules.OUTER for name in w["layers"]):
                inner += f" (wider than {rules.INNER_MAX_WIDTH} mm: outer layers only)"
        print(f"current: {net} {w['current']:g} A ({w['from']}): IPC-2221 outer {w['outer']:.3f} mm "
              f"-> class track {w['track']:.3f} mm{inner}; routed on {', '.join(w['layers'])}")
    for ref, e in sorted(escapes.items()):
        ws = {}
        for num, pad in e["pads"].items():
            ws.setdefault(round(pad["width"], 3), []).append(num)
        print(f"escape: {ref} (clearance {e['clearance']}): "
              + "; ".join(f"{w:.3f} mm x {len(n)}" for w, n in sorted(ws.items())))
    for p in pairs:
        g = p["geometry"]
        print(f"diff pair: {p['p']}/{p['n']} ({p['standard']}, "
              f"{', '.join(f'{r}.{a}/{b}' for r, a, b in p['ends'])}): {g['note']}")
    (out / f"{spec['name']}.kicad_dru").write_text(rules.dru_text(fab, widths, escapes, pairs, layers))
    items, report = rules.fanout(spec, models, placement, nets, fab, widths, escapes, pairs,
                                 placement.get("fanout") or {}, layers)
    if NO_FANOUT:
        items, report = [], ["fan-out: none (--no-fanout)"]
    for line in report:
        print(line)
    routing = {
        "fab": {k: v for k, v in fab.items() if k != "source"},
        "stackup": spec["stackup"],
        "widths": widths,
        "escapes": escapes,
        "pairs": pairs,
        "errors": errors,
    }
    (out / "routing.json").write_text(json.dumps(routing, indent=1, default=list))
    for e in errors:
        print("escape error:", e)
    if errors:
        sys.exit(f"{len(errors)} pad(s) cannot escape within the fab's limits")
    return [{**{k: v for k, v in i.items() if k not in ("layers", "hole")},
             "layer": next(iter(i["layers"])) if i["kind"] == "track" else None}
            for i in items]


def stage_pcb(spec, out):
    for old in (out / f"{spec['name']}.kicad_pcb", out / f"{spec['name']}.ses", out / "routing.json"):
        old.unlink(missing_ok=True)
    placement = {}
    pfile = out / "placement.yaml"
    if pfile.exists():
        placement = yaml.safe_load(pfile.read_text()) or {}
    models = placement_models(spec, out)
    fanout = routing_rules(spec, models, placement, out)
    write_project(spec, out)
    parts = placement_plan(spec, models, placement)
    plan = {
        "name": spec["name"],
        "board": spec["board"],
        # Per copper layer, top to bottom: signal, or a plane of a net.
        "stackup": spec["stackup"],
        # The nets carried by copper areas: the pours and the planes.
        "pours": rules.pour_nets(spec["board"], spec["stackup"]),
        "components": {ref: safe(mpn_of(p)) for ref, p in spec["components"].items()},
        "values": {ref: mpn_of(p) for ref, p in spec["components"].items()},
        "nets": {**unconnected_nets(spec, out), **{f"{r}.{p}": n for (r, p), n in pin_refs(spec).items()}},
        "placement": placement,
        # Per reference, from the Placement Models: the KiCad transform,
        # the pads on the board (checked against pcbnew's), the courtyard
        # box and the model hash (checked against the footprint's).
        "parts": parts,
        # Locked escape stubs, dog-bone and thermal vias, coupled diff
        # pairs (rules.fanout).
        "fanout": fanout,
    }
    (out / "plan.json").write_text(json.dumps(plan, indent=1))
    kicad_python(HERE / "pcb_stage.py", out, "pcb")
    sync_sch_footprints(spec, out)
    check_placement(spec, models, parts)


def freerouting_jar():
    cache = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "openparts"
    jar = cache / f"freerouting-{FREEROUTING_VERSION}.jar"
    if not jar.exists():
        cache.mkdir(parents=True, exist_ok=True)
        url = (
            "https://github.com/freerouting/freerouting/releases/download/"
            f"v{FREEROUTING_VERSION}/freerouting-{FREEROUTING_VERSION}.jar"
        )
        subprocess.run(["curl", "-fsSL", "-o", str(jar), url], check=True)
    return jar


# Freerouting is deterministic for a given board and cost setting, but
# chaotic: on a dense board one setting ends a connection or two short
# and the next one completes, with nothing to tell them apart before.
# `route` runs a sweep of these settings in parallel (each with the pours
# as planes on every layer, and only on the top / inner ones) and keeps
# the result with the fewest unrouted connections plus cut-off pour pads.
ROUTER_VARIANTS = [
    {},
    {"via_costs": 100, "start_ripup_costs": 200},
    {"via_costs": 80, "start_ripup_costs": 250},
    {"via_costs": 120, "start_ripup_costs": 150},
    {"via_costs": 70},
    {"via_costs": 60, "start_ripup_costs": 150},
    {"via_costs": 90, "start_ripup_costs": 120},
    {"via_costs": 70, "start_ripup_costs": 300},
    {"via_costs": 50, "start_ripup_costs": 100},
    {"via_costs": 60, "start_ripup_costs": 200},
    {"via_costs": 80, "start_ripup_costs": 150},
    {"via_costs": 100, "start_ripup_costs": 100},
]
# When no result of a sweep is complete, further sweeps follow, each
# with settings of its own (re-running a setting gives the same board):
# up to ROUTER_ROUNDS sweeps in all.
ROUTER_ROUNDS = 3
EXTRA_VARIANTS = [{"via_costs": v, "start_ripup_costs": r}
                  for r in (80, 175, 225) for v in (45, 55, 65, 75, 85, 95, 110, 130)]
# Parallel Freerouting runs (each takes up to ~1 GB of heap).
ROUTER_JOBS = 8
# --evaluate-all: import and score every routing result (for measuring
# how robust a placement routes), not only until a complete one.
EVALUATE_ALL = False
# --no-fanout: the pcb stage places no fan-out (to measure what it buys).
NO_FANOUT = False
# --sweep N: only the first N cost settings (a quicker, reduced sweep).
SWEEP = len(ROUTER_VARIANTS)


def freerouting(dsn, ses, costs):
    """Runs Freerouting; returns (unrouted connections, log)."""
    cmd = [
        "java", "-jar", str(freerouting_jar()), "-de", str(dsn), "-do", str(ses),
        "--gui.enabled=false",
        "--router.autorouter.max_passes=40",
        # Never narrower than the net class (KiCad's minimum track
        # width): no automatic neck-down, and no fan-out stage (its pin
        # escapes neck down below the class width regardless).
        "--router.automatic_neckdown=false",
        "--router.fanout.enabled=false",
        "--router.optimizer.max_threads=2",
    ] + [f"--router.scoring.{k}={v}" for k, v in costs.items()]
    run = subprocess.run(cmd, capture_output=True, text=True)
    log = run.stdout + run.stderr
    if run.returncode != 0 or not Path(ses).exists():
        return None, log
    # The routing stage's own final count (the list of failed
    # connections is not always printed).
    final = re.findall(r"Auto-routing stage completed.*?\((\d+) unrouted", log)
    if not final:
        return None, log
    return int(final[-1]), log


def split_wire_paths(dsn):
    """Rewrites a DSN's pre-routed wires (the locked bridges and ties) as
    one two-point wire per segment: KiCad writes connected segments as one
    path, and Freerouting does not take a path of more than two points for
    connecting its ends (a bridge over a USB-C pad pair stayed unrouted in
    every run)."""
    text = Path(dsn).read_text()

    def split(m):
        layer, width, coords, rest = m.groups()
        nums = coords.split()
        pts = list(zip(nums[0::2], nums[1::2]))
        if len(pts) <= 2:
            return m.group(0)
        return "\n    ".join(f"(wire (path {layer} {width}  {a[0]} {a[1]}  {b[0]} {b[1]}){rest})"
                              for a, b in zip(pts, pts[1:]))

    Path(dsn).write_text(re.sub(r"\(wire \(path (\S+) (\S+)\s+([-\d.\s]+?)\)((?:\([^()]*\))*)\)", split, text))


def unrouted_nets(log):
    """The nets Freerouting's last stage lists as not routed."""
    tail = log.rsplit("could not be routed", 1)
    return re.findall(r"Net '([^']+)'", tail[1].split("Auto-routing stage completed")[0]) if len(tail) > 1 else []


def stage_route(spec, out):
    from concurrent.futures import ThreadPoolExecutor

    kicad_python(HERE / "pcb_stage.py", out, "dsn")
    name = spec["name"]
    second = "inner" if spec["board"].get("layers", 2) == 4 else "top"
    kicad_python(HERE / "pcb_stage.py", out, "dsn-from", out / f"{name}.kicad_pcb",
                 out / f"{name}-{second}.dsn", second)
    widths = routing_of(spec, out)["widths"]
    for dsn in (out / f"{name}.dsn", out / f"{name}-{second}.dsn"):
        split_wire_paths(dsn)
        # Plane layers typed `power`, per-layer widths, outer-only nets.
        dsn.write_text(rules.dsn_layer_rules(dsn.read_text(), widths, spec["stackup"]))
    ses = out / f"{name}.ses"
    ses.unlink(missing_ok=True)
    freerouting_jar()
    modes = (("all planes", "all", out / f"{name}.dsn"), (f"{second} plane", second, out / f"{name}-{second}.dsn"))
    variants = ROUTER_VARIANTS[:SWEEP]
    rounds = [variants] + [EXTRA_VARIANTS[k:k + len(variants)]
                           for k in range(0, len(variants) * (ROUTER_ROUNDS - 1), len(variants))]

    # Scored by what stays unconnected: connections Freerouting left
    # unrouted plus pour-net pads the stitched pour cannot reach (walled
    # in by tracks on both sides).
    def score(i):
        report = kicad_python(HERE / "pcb_stage.py", out, "ses", out / f".route-{i}.ses",
                              out / f".route-{i}.kicad_pcb", capture=True)
        return int(re.search(r"pour pads cut off: (\d+)", report).group(1))

    tries, results, cut = [], [], {}
    for rnd, costs_list in enumerate(r for r in rounds if r):
        sweep = [(f"{label} {costs or 'defaults'}", mode, dsn, costs)
                 for label, mode, dsn in modes for costs in costs_list]
        base = len(tries)
        tries += sweep
        with ThreadPoolExecutor(max_workers=ROUTER_JOBS) as pool:
            results += list(pool.map(lambda it: freerouting(it[1][2], out / f".route-{base + it[0]}.ses", it[1][3]),
                                     enumerate(sweep)))
        for (what, *_), (unrouted, log) in zip(sweep, results[base:]):
            print(f"freerouting {what}: " + ("failed" if unrouted is None else f"{unrouted} unrouted")
                  + "".join(f" {n}" for n in unrouted_nets(log)), flush=True)
        done = [i for i in range(base, len(tries)) if results[i][0] is not None]
        if EVALUATE_ALL:
            with ThreadPoolExecutor(max_workers=4) as pool:
                cut.update(zip(done, pool.map(score, done)))
            complete = sum(results[i][0] + cut[i] == 0 for i in done)
            print(f"round {rnd + 1}: complete (0 unrouted, 0 cut off): {complete} of {len(sweep)}", flush=True)
        else:
            # In order of unrouted count, until none can beat the best.
            for i in sorted(done, key=lambda i: (results[i][0], i)):
                if cut and results[i][0] >= min(results[j][0] + cut[j] for j in cut):
                    break
                cut[i] = score(i)
                if results[i][0] + cut[i] == 0:
                    break
        if any(results[i][0] + cut[i] == 0 for i in cut):
            break
        print(f"round {rnd + 1}: none complete", flush=True)
    if not cut:
        sys.exit("Freerouting failed:\n" + results[0][1][-2000:])
    i = min(cut, key=lambda i: (results[i][0] + cut[i], i))
    print(f"kept freerouting {tries[i][0]}: {results[i][0]} unrouted, {cut[i]} pour pad(s) cut off")
    (out / f".route-{i}.kicad_pcb").replace(out / f"{name}.kicad_pcb")
    (out / f".route-{i}.ses").replace(ses)
    (out / "out").mkdir(exist_ok=True)
    (out / "out" / "freerouting.log").write_text(results[i][1])
    # (SaveBoard also writes a project file beside each candidate.)
    for other in out.glob(".route-*"):
        other.unlink()
    fix_starved_thermals(spec, out)


def fix_starved_thermals(spec, out, tries=3):
    """DRC's starved_thermal findings on the kept board (a relief whose
    spokes reach only slivers of the pour; KiCad's own judgement, which
    the ses stage's spoke count does not always match on four layers)
    joined solid, until DRC finds none."""
    name = spec["name"]
    for _ in range(tries):
        report = out / "out" / ".thermal-drc.json"
        subprocess.run(["flatpak", "run", f"--filesystem={out}", "--command=sh", "org.kicad.KiCad", "-c",
                        f"cd {sh_quote(str(out))} && kicad-cli pcb drc --format json -o out/.thermal-drc.json "
                        f"{name}.kicad_pcb"], capture_output=True)
        if not report.exists():
            return
        found = json.loads(report.read_text()).get("violations", [])
        report.unlink()
        # (Each finding names the zone and the pad; `solid` acts on pads.)
        pads = sorted({i["uuid"] for v in found if v["type"] == "starved_thermal" for i in v["items"] if "uuid" in i})
        if not pads:
            return
        kicad_python(HERE / "pcb_stage.py", out, "solid", *pads)


def stage_check(spec, out):
    name = spec["name"]
    res = out / "out"
    res.mkdir(exist_ok=True)
    cli = "kicad-cli"
    cmds = [
        f"cd {sh_quote(str(out))} && {cli} sch erc --format json -o out/erc.json {name}.kicad_sch",
        f"cd {sh_quote(str(out))} && {cli} pcb drc --schematic-parity --format json -o out/drc.json {name}.kicad_pcb",
        f"cd {sh_quote(str(out))} && {cli} sch export svg -o out {name}.kicad_sch",
        f"cd {sh_quote(str(out))} && {cli} pcb export svg --mode-single --layers F.Cu,B.Cu,F.SilkS,Edge.Cuts,F.CrtYd -o out/{name}-pcb.svg {name}.kicad_pcb",
        f"cd {sh_quote(str(out))} && {cli} pcb render --side top --width 1600 --height 900 -o out/{name}-top.png {name}.kicad_pcb",
        f"cd {sh_quote(str(out))} && {cli} pcb render --side bottom --width 1600 --height 900 -o out/{name}-bottom.png {name}.kicad_pcb",
    ]
    # Each copper layer on its own (the 3D renders show only the outer
    # ones): out/<name>-<layer>.svg, and .png where ImageMagick is.
    copper = [s["layer"] for s in spec["stackup"]]
    for layer in copper:
        cmds.append(f"cd {sh_quote(str(out))} && {cli} pcb export svg --mode-single --page-size-mode 2 "
                    f"--exclude-drawing-sheet --layers {layer},Edge.Cuts -o out/{name}-{layer}.svg {name}.kicad_pcb")
    for c in cmds:
        subprocess.run(["flatpak", "run", f"--filesystem={out}", "--command=sh", "org.kicad.KiCad", "-c", c])
    magick = next((m for m in ("magick", "convert") if shutil.which(m)), None)
    for layer in copper if magick else []:
        svg = res / f"{name}-{layer}.svg"
        if svg.exists():
            subprocess.run([magick, "-density", "600", str(svg), "-background", "white", "-flatten",
                            str(svg.with_suffix(".png"))], capture_output=True)
    for kind in ("erc", "drc"):
        f = res / f"{kind}.json"
        if not f.exists():
            continue
        data = json.loads(f.read_text())
        items = data.get("violations", [])
        for s in data.get("sheets", []):
            items += s.get("violations", [])
        counts = {}
        for v in items + data.get("unconnected_items", []) + data.get("schematic_parity", []):
            key = (v.get("severity"), v.get("type"))
            counts[key] = counts.get(key, 0) + 1
        print(f"{kind}:", ", ".join(f"{n} x {t} ({s})" for (s, t), n in sorted(counts.items())) or "clean")
    check_routing(spec, out)


def check_routing(spec, out):
    """After routing: every track at least its net's current width
    (outside the neck-down zones; inside, its pad share's), and each diff
    pair's lengths and skew. Writes out/routing-check.json."""
    routing_file = out / "routing.json"
    if not routing_file.exists():
        return
    routing = json.loads(routing_file.read_text())
    text = kicad_python(HERE / "pcb_stage.py", out, "tracks", capture=True, quiet=True)
    tracks = json.loads(text[text.index("["):])
    zones = rules.zone_boxes(routing["escapes"])
    bad = rules.check_widths(tracks, routing["widths"], zones, routing["escapes"])
    for v in bad:
        print(f"current width: {v['net']} track {v['width']:.3f} mm < {v['need']:.3f} mm ({v['where']}) "
              f"at ({v['a'][0]:.2f}, {v['a'][1]:.2f})-({v['b'][0]:.2f}, {v['b'][1]:.2f})")
    print(f"current-width violations: {len(bad)}")
    pairs = []
    lengths = rules.pair_lengths(tracks, routing["pairs"])
    for p in routing["pairs"]:
        lp, ln = lengths[(p["p"], p["n"])]
        skew = abs(lp - ln)
        ps = rules.skew_ps(skew, spec["fab"], p["geometry"]["width"])
        print(f"diff pair {p['p']}/{p['n']}: {lp:.2f} / {ln:.2f} mm of track, skew {skew:.3f} mm "
              f"(about {ps:.1f} ps on the outer layer)")
        pairs.append({"p": p["p"], "n": p["n"], "length_p": lp, "length_n": ln, "skew_mm": skew, "skew_ps": ps})
    (out / "out" / "routing-check.json").write_text(
        json.dumps({"width_violations": bad, "pairs": pairs}, indent=1))


def stage_preview(spec, out):
    """For placement: the board (unrouted) rendered top and bottom, plus
    DRC's courtyard and edge findings."""
    stage_pcb(spec, out)
    name = spec["name"]
    c = sh_quote(str(out))
    for cmd in (
        f"cd {c} && kicad-cli pcb render --side top --width 1600 --height 900 -o out/preview-top.png {name}.kicad_pcb",
        f"cd {c} && kicad-cli pcb export svg --mode-single --layers F.Cu,B.Cu,F.SilkS,F.CrtYd,B.CrtYd,Edge.Cuts,F.Fab -o out/preview.svg {name}.kicad_pcb",
        f"cd {c} && kicad-cli pcb drc --format json -o out/preview-drc.json {name}.kicad_pcb",
    ):
        (out / "out").mkdir(exist_ok=True)
        subprocess.run(["flatpak", "run", f"--filesystem={out}", "--command=sh", "org.kicad.KiCad", "-c", cmd],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    drc = json.loads((out / "out" / "preview-drc.json").read_text())
    counts = {}
    for v in drc.get("violations", []):
        if v["type"] in ("courtyards_overlap", "copper_edge_clearance", "malformed_courtyard",
                         "silk_overlap", "silk_over_copper", "lib_footprint_mismatch"):
            counts[v["type"]] = counts.get(v["type"], 0) + 1
    print("placement DRC:", counts or "clean", "| unconnected:", len(drc.get("unconnected_items", [])))


STAGES = ["fetch", "sch", "pcb", "route", "check"]


def main():
    global EVALUATE_ALL, SWEEP, NO_FANOUT, ROUTER_ROUNDS
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("board")
    ap.add_argument("--out", required=True)
    ap.add_argument("--stage", default="all", choices=STAGES + ["all", "preview"])
    ap.add_argument("--data", help="build the parts from this openparts-data checkout instead of the registry")
    ap.add_argument("--registry", help="the OpenParts registry URL, overriding the board file's")
    ap.add_argument("--layers", type=int, choices=(2, 4),
                    help="copper layers, overriding the board file (then board.by_layers.N applies)")
    ap.add_argument("--fab", help="fab profile preset (tools/kicad-board/fab/NAME.yaml), overriding the board file's")
    ap.add_argument("--stackup", metavar="L1,L2,...",
                    help="copper layers top to bottom, each `signal` or a plane's net (signal,GND,3V3,signal), "
                         "overriding board.stackup")
    ap.add_argument("--no-fanout", action="store_true",
                    help="pcb: no pre-placed escape stubs, vias or diff-pair paths (for comparing)")
    ap.add_argument("--evaluate-all", action="store_true",
                    help="route: score every router configuration and report how many complete")
    ap.add_argument("--rounds", type=int, metavar="N",
                    help="route: at most N sweeps (default %d)" % ROUTER_ROUNDS)
    ap.add_argument("--sweep", type=int, metavar="N",
                    help="route: try only the first N router cost settings (of %d)" % len(ROUTER_VARIANTS))
    a = ap.parse_args()
    NO_FANOUT = a.no_fanout
    ROUTER_ROUNDS = a.rounds or ROUTER_ROUNDS
    EVALUATE_ALL = a.evaluate_all
    SWEEP = a.sweep or SWEEP
    spec = load_spec(a.board)
    if a.registry:
        spec["registry"] = a.registry.rstrip("/")
    apply_layers(spec, a.layers)
    if a.stackup:
        spec["board"]["stackup"] = ["signal" if s == "signal" else {"plane": s} for s in a.stackup.split(",")]
    apply_fab(spec, a.fab)
    out = Path(a.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    stages = STAGES if a.stage == "all" else [a.stage]
    for s in stages:
        print(f"== {s}", flush=True)
        if s == "fetch":
            stage_fetch(spec, out, a.data)
            continue
        if s in ("route", "check") and (out / "routing.json").exists():
            # The board's own stackup, as the pcb stage built it.
            spec["stackup"] = json.loads((out / "routing.json").read_text()).get("stackup", spec["stackup"])
        globals()[f"stage_{s}"](spec, out)


if __name__ == "__main__":
    main()
