#!/usr/bin/env python3
"""Builds a KiCad project from a board description (YAML) whose parts all
come from an OpenParts registry.

  build.py BOARD.yaml --out DIR [--stage fetch|sch|pcb|route|check|all|preview]
           [--data OPENPARTS_DATA_DIR]

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
         nets, board outline, design rules, zones
  route  route it with Freerouting (Specctra DSN/SES): every net,
         the pour nets too (exported without their zones, so each pad
         gets a real connection rather than relying on a pour that may
         leave islands), then refill the zones
  check  ERC, DRC with schematic parity, and renders under DIR/out/
  preview  `pcb` plus an unrouted render and DRC's placement findings

Net classes: `board.net_classes: {Name: {track: mm, nets: [...]}}`
(the older `board.power_nets: {track, nets}` is the class "Power").
Keep nets that reach a fine-pitch part (0.4 mm QFN) at a track that
fits between its pads: Freerouting's automatic neck-down is turned off,
so no track is ever narrower than its class.

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


def net_classes(spec):
    """{class name: (track, [nets])} from the board spec."""
    board = spec["board"]
    classes = {n: (c["track"], list(c.get("nets", []))) for n, c in (board.get("net_classes") or {}).items()}
    if board.get("power_nets"):
        classes.setdefault("Power", (board["power_nets"]["track"], list(board["power_nets"].get("nets", []))))
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
    """The project file: design rules and net classes from the board spec."""
    pro = out / f"{spec['name']}.kicad_pro"
    r = spec["board"]["rules"]
    classes = net_classes(spec)
    widths = [r["track"]] + [t for t, _ in classes.values()]

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
                    # No track narrower than the narrowest class (and
                    # Freerouting is told not to neck down below it).
                    "min_track_width": min(widths),
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
            + [netclass(name, track) for name, (track, _) in classes.items()],
            "meta": {"version": 4},
            "netclass_patterns": [
                {"netclass": name, "pattern": n} for name, (_, nets) in classes.items() for n in nets
            ],
        },
    }
    pro.write_text(json.dumps(project, indent=2) + "\n")


# ------------------------------------------------------------------ pcb


def kicad_python(script, *args, capture=False):
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
        "nets": {**unconnected_nets(spec, out), **{f"{r}.{p}": n for (r, p), n in pin_refs(spec).items()}},
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


# Freerouting is deterministic for a given board and cost setting, and
# a dense board can end a few connections short with one setting and
# complete with another: `route` runs these in parallel and keeps the
# result with the fewest unrouted connections (the first, on a tie).
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
# Parallel Freerouting runs (each takes up to ~1 GB of heap).
ROUTER_JOBS = 6


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


def stage_route(spec, out):
    from concurrent.futures import ThreadPoolExecutor

    kicad_python(HERE / "pcb_stage.py", out, "dsn")
    name = spec["name"]
    ses = out / f"{name}.ses"
    ses.unlink(missing_ok=True)
    freerouting_jar()
    tries = [(f"{label} {costs or 'defaults'}", dsn, costs)
             for label, dsn in (("both planes", out / f"{name}.dsn"), ("top plane", out / f"{name}-top.dsn"))
             for costs in ROUTER_VARIANTS]
    tries = [(what, dsn, costs, out / f".route-{i}.ses") for i, (what, dsn, costs) in enumerate(tries)]
    with ThreadPoolExecutor(max_workers=ROUTER_JOBS) as pool:
        results = list(pool.map(lambda t: freerouting(t[1], t[3], t[2]), tries))
    for (what, *_), (unrouted, log) in zip(tries, results):
        print(f"freerouting {what}: " + ("failed" if unrouted is None else f"{unrouted} unrouted"), flush=True)
    done = [(unrouted, i) for i, (unrouted, _) in enumerate(results) if unrouted is not None]
    if not done:
        sys.exit("Freerouting failed:\n" + results[0][1][-2000:])
    # Scored by what stays unconnected: connections Freerouting left
    # unrouted plus pour-net pads the stitched pour cannot reach (walled
    # in by tracks on both sides). Candidates are imported in order of
    # unrouted count until none can beat the best so far.
    best = None
    for unrouted, i in sorted(done):
        if best and unrouted >= best[0] + best[1]:
            break
        board = out / f".route-{i}.kicad_pcb"
        report = kicad_python(HERE / "pcb_stage.py", out, "ses", tries[i][3], board, capture=True)
        cut_off = int(re.search(r"pour pads cut off: (\d+)", report).group(1))
        if best is None or unrouted + cut_off < best[0] + best[1]:
            best = (unrouted, cut_off, board, i)
        if best[0] + best[1] == 0:
            break
    unrouted, cut_off, board, i = best
    print(f"kept freerouting {tries[i][0]}: {unrouted} unrouted, {cut_off} pour pad(s) cut off")
    board.replace(out / f"{name}.kicad_pcb")
    tries[i][3].replace(ses)
    (out / "out").mkdir(exist_ok=True)
    (out / "out" / "freerouting.log").write_text(results[i][1])
    # (SaveBoard also writes a project file beside each candidate.)
    for other in out.glob(".route-*"):
        other.unlink()


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
    ap.add_argument("--data", help="build the parts from this openparts-data checkout instead of the registry")
    a = ap.parse_args()
    spec = load_spec(a.board)
    out = Path(a.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    stages = STAGES if a.stage == "all" else [a.stage]
    for s in stages:
        print(f"== {s}", flush=True)
        if s == "fetch":
            stage_fetch(spec, out, a.data)
        else:
            globals()[f"stage_{s}"](spec, out)


if __name__ == "__main__":
    main()
