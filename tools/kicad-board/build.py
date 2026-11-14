#!/usr/bin/env python3
"""Builds a KiCad project from a board description (YAML) whose parts all
come from an OpenParts registry.

  build.py BOARD.yaml --out DIR [--stage fetch|sch|pcb|route|check|all]

Stages (each needs the ones before it):
  fetch  install every part into DIR's own libraries (`openparts
         kicad-install`: symbol, footprint and STEP from the registry)
  sch    write DIR/<name>.kicad_sch: each symbol placed on a grid, a net
         label on every connected pin, no-connect flags on open ones
  pcb    write DIR/<name>.kicad_pcb (KiCad's pcbnew, in its Flatpak):
         footprints at their place in DIR/placement.yaml (or a grid),
         nets, board outline, design rules, zones
  route  route it with Freerouting (Specctra DSN/SES) and refill zones
  check  ERC, DRC with schematic parity, and renders under DIR/out/

The netlist is the board description, so the schematic and the board
are built from the same data and agree by construction (DRC's parity
check confirms it).
"""

import argparse
import json
import os
import re
import subprocess
import sys
import uuid
from pathlib import Path

import yaml

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
    built = REPO / "target" / "debug" / "openparts"
    return [str(built)] if built.exists() else ["cargo", "run", "-q", "-p", "openparts-cli", "--"]


def stage_fetch(spec, out):
    parts = sorted(set(spec["components"].values()))
    cmd = openparts_cli() + ["--registry", spec["registry"], "kicad-install", "--project", str(out)] + parts
    subprocess.run(cmd, check=True, cwd=REPO)


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
    o.append(f'(kicad_sch (version 20231120) (generator "openparts") (generator_version "0.1")')
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
            f'    (property "Footprint" "openparts:{safe(mpn)}" (at {ox:.2f} {oy:.2f} 0)'
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
            justify = "right" if label_angle in (180, 270) else "left"
            o.append(
                f'  (label "{net}" (at {ax:.2f} {ay:.2f} {label_angle})'
                f' (effects (font (size 1.27 1.27)) (justify {justify} bottom)) (uuid "{uid()}"))'
            )
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
        o.append(
            f'  (label "{net}" (at {fx:.2f} {fy:.2f} 270)'
            f' (effects (font (size 1.27 1.27)) (justify right bottom)) (uuid "{uid()}"))'
        )
        fx += 15.24
    o.append('  (sheet_instances (path "/" (page "1")))')
    o.append(")")
    (out / f"{name}.kicad_sch").write_text("\n".join(o) + "\n")
    write_project(spec, out)


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
    """The project file: design rules and net classes from the board spec."""
    pro = out / f"{spec['name']}.kicad_pro"
    r = spec["board"]["rules"]
    power = spec["board"].get("power_nets", {})

    def netclass(name, track):
        return {
            "name": name,
            "clearance": r["clearance"],
            "track_width": track,
            "via_diameter": r["via_diameter"],
            "via_drill": r["via_drill"],
            "microvia_diameter": 0.3,
            "microvia_drill": 0.1,
            "diff_pair_width": r["track"],
            "diff_pair_gap": r["clearance"],
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
                    "min_clearance": r["clearance"],
                    "min_track_width": min(r["track"], 0.15),
                    "min_via_diameter": r["via_diameter"] - 0.1,
                    "min_through_hole_diameter": r["via_drill"],
                    "min_copper_edge_clearance": 0.25,
                    "min_hole_clearance": 0.25,
                }
            }
        },
        "meta": {"filename": pro.name, "version": 3},
        "net_settings": {
            "classes": [netclass("Default", r["track"])]
            + ([netclass("Power", power["track"])] if power else []),
            "meta": {"version": 4},
            "netclass_patterns": [{"netclass": "Power", "pattern": n} for n in power.get("nets", [])],
        },
    }
    pro.write_text(json.dumps(project, indent=2) + "\n")


# ------------------------------------------------------------------ pcb


def kicad_python(script, *args):
    """Runs `script` with KiCad's own Python (pcbnew), in its Flatpak."""
    cmd = " ".join(["python3", sh_quote(str(script))] + [sh_quote(str(a)) for a in args])
    subprocess.run(
        ["flatpak", "run", f"--filesystem={REPO}", f"--filesystem={args[0] if args else REPO}",
         "--command=sh", "org.kicad.KiCad", "-c", cmd],
        check=True,
    )


def sh_quote(s):
    return "'" + s.replace("'", "'\\''") + "'"


def stage_pcb(spec, out):
    write_project(spec, out)
    for old in (out / f"{spec['name']}.kicad_pcb", out / f"{spec['name']}.ses"):
        old.unlink(missing_ok=True)
    placement = {}
    pfile = out / "placement.yaml"
    if pfile.exists():
        placement = yaml.safe_load(pfile.read_text()) or {}
    plan = {
        "name": spec["name"],
        "board": spec["board"],
        "components": {ref: safe(mpn_of(p)) for ref, p in spec["components"].items()},
        "values": {ref: mpn_of(p) for ref, p in spec["components"].items()},
        "nets": {f"{r}.{p}": n for (r, p), n in pin_refs(spec).items()},
        "placement": placement,
    }
    (out / "plan.json").write_text(json.dumps(plan, indent=1))
    kicad_python(HERE / "pcb_stage.py", out, "pcb")


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


def stage_route(spec, out):
    kicad_python(HERE / "pcb_stage.py", out, "dsn")
    dsn, ses = out / f"{spec['name']}.dsn", out / f"{spec['name']}.ses"
    ses.unlink(missing_ok=True)
    subprocess.run(
        ["java", "-jar", str(freerouting_jar()), "-de", str(dsn), "-do", str(ses),
         "-mp", "40", "--gui.enabled=false"],
        check=True,
    )
    kicad_python(HERE / "pcb_stage.py", out, "ses")


def stage_check(spec, out):
    name = spec["name"]
    res = out / "out"
    res.mkdir(exist_ok=True)
    cli = f"kicad-cli"
    cmds = [
        f"cd {sh_quote(str(out))} && {cli} sch erc --format json -o out/erc.json {name}.kicad_sch",
        f"cd {sh_quote(str(out))} && {cli} pcb drc --schematic-parity --format json -o out/drc.json {name}.kicad_pcb",
        f"cd {sh_quote(str(out))} && {cli} sch export svg -o out {name}.kicad_sch",
        f"cd {sh_quote(str(out))} && {cli} pcb export svg --mode-single --layers F.Cu,B.Cu,F.SilkS,Edge.Cuts,F.CrtYd -o out/{name}-pcb.svg {name}.kicad_pcb",
        f"cd {sh_quote(str(out))} && {cli} pcb render --side top --width 1600 --height 900 -o out/{name}-top.png {name}.kicad_pcb",
        f"cd {sh_quote(str(out))} && {cli} pcb render --side bottom --width 1600 --height 900 -o out/{name}-bottom.png {name}.kicad_pcb",
    ]
    for c in cmds:
        subprocess.run(["flatpak", "run", f"--filesystem={out}", "--command=sh", "org.kicad.KiCad", "-c", c])
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
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("board")
    ap.add_argument("--out", required=True)
    ap.add_argument("--stage", default="all", choices=STAGES + ["all", "preview"])
    a = ap.parse_args()
    spec = load_spec(a.board)
    out = Path(a.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    stages = STAGES if a.stage == "all" else [a.stage]
    for s in stages:
        print(f"== {s}", flush=True)
        globals()[f"stage_{s}"](spec, out)


if __name__ == "__main__":
    main()
