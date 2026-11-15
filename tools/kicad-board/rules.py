"""Routing rules derived from the fab, the parts and the board: the pure
half of build.py's routing work (no pcbnew; unit-tested in tests/).

  fab profile     board.yaml `board.fab`: a preset from fab/ (the fab's
                  published limits, with source and date) plus overrides
  current width   IPC-2221 conductor width for each net's declared current
  escape width    per fine-pitch pad, from the Placement Model's pad
                  extents: the widest track that leaves the pad clear of
                  its neighbours
  diff pairs      from the pins' `diff_pair` metadata, with a width/gap
                  for the standard's impedance (edge-coupled microstrip)
  .kicad_dru      KiCad custom rules: current widths, neck-down inside
                  each fine-pitch courtyard, diff-pair gaps
  fan-out         locked escape stubs, dog-bone vias and coupled diff-pair
                  paths placed before Freerouting (which reads neither the
                  custom rules nor diff pairs)
  checks          track widths against the current widths after routing,
                  diff-pair length and skew

Coordinates are board mm, Y up, from the board centre (placement.yaml's
frame); a part's pads land at T · R(rot) · M (docs/spec/placement-model.md
section 2.3).
"""

import math
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
FAB_DIR = HERE / "fab"

# ------------------------------------------------------------ fab profile

# 1 oz/ft² of copper is 1.37 mil (34.8 µm) thick (IPC-2221B, the
# conductor thickness its current chart is drawn for).
OZ_MM = 0.0348
MIL_MM = 0.0254
FINE_PITCH = 0.65  # mm: a part with a lead pitch at or below this gets an escape zone


def load_fab(board):
    """The fab profile: board["fab"] = {preset: NAME, ...overrides} (or a
    full profile inline). Checks that the design values keep to the
    fab's minimums. Returns the merged dict."""
    given = dict(board.get("fab") or {})
    name = given.pop("preset", None)
    fab = {}
    if name:
        path = FAB_DIR / f"{name}.yaml"
        if not path.exists():
            raise ValueError(f"fab preset {name!r} not in {FAB_DIR} "
                             f"({', '.join(sorted(p.stem for p in FAB_DIR.glob('*.yaml')))})")
        fab = yaml.safe_load(path.read_text())
        fab["preset"] = name
    for key, value in given.items():
        if isinstance(value, dict) and isinstance(fab.get(key), dict):
            fab[key] = {**fab[key], **value}
        else:
            fab[key] = value
    for key in ("min_track", "min_clearance", "min_via_drill", "min_via_diameter", "copper", "stackup", "design"):
        if key not in fab:
            raise ValueError(f"fab profile lacks {key!r}")
    d = fab["design"]
    problems = []
    if d["track"] < fab["min_track"] - 1e-9:
        problems.append(f"design track {d['track']} < fab minimum {fab['min_track']}")
    if d["clearance"] < fab["min_clearance"] - 1e-9:
        problems.append(f"design clearance {d['clearance']} < fab minimum {fab['min_clearance']}")
    if d["via_drill"] < fab["min_via_drill"] - 1e-9:
        problems.append(f"design via drill {d['via_drill']} < fab minimum {fab['min_via_drill']}")
    if d["via_diameter"] < fab["min_via_diameter"] - 1e-9:
        problems.append(f"design via diameter {d['via_diameter']} < fab minimum {fab['min_via_diameter']}")
    ring = (d["via_diameter"] - d["via_drill"]) / 2
    if ring < fab.get("min_annular_ring", 0) - 1e-9:
        problems.append(f"design via annular ring {ring:.3f} < fab minimum {fab['min_annular_ring']}")
    if problems:
        raise ValueError("fab profile: " + "; ".join(problems))
    return fab


def copper_mm(fab, outer=True):
    oz = fab["copper"]["outer_oz" if outer else "inner_oz"]
    if oz is None:
        raise ValueError("fab profile has no inner copper weight")
    return oz * OZ_MM


# ------------------------------------------------------- current (IPC-2221)


def ipc2221_width(current, thickness_mm, outer=True, rise=10.0):
    """Conductor width (mm) carrying `current` A at a temperature rise of
    `rise` °C, IPC-2221B section 6.2: I = k · ΔT^0.44 · A^0.725, A the
    cross-section in mil², k = 0.048 for outer and 0.024 for inner
    layers. IPC-2152 supersedes this chart with measured data (and finds
    it conservative for inner layers); its charts have no closed form, so
    the IPC-2221 fit is used."""
    if current <= 0:
        return 0.0
    k = 0.048 if outer else 0.024
    area_mil2 = (current / (k * rise ** 0.44)) ** (1 / 0.725)
    return area_mil2 / (thickness_mm / MIL_MM) * MIL_MM


def ipc2221_current(width_mm, thickness_mm, outer=True, rise=10.0):
    """The inverse: the current a conductor carries at that rise."""
    k = 0.048 if outer else 0.024
    area_mil2 = (width_mm / MIL_MM) * (thickness_mm / MIL_MM)
    return k * rise ** 0.44 * area_mil2 ** 0.725


def net_currents(board, derived=None):
    """{net: (amps, where from)}: board.nets.<net>.current, else a value
    `derived` from the parts (a regulator's output_current)."""
    out = {}
    for net, info in (board.get("nets") or {}).items():
        if isinstance(info, dict) and info.get("current") is not None:
            out[net] = (float(info["current"]), "board.yaml")
    for net, (amps, why) in (derived or {}).items():
        out.setdefault(net, (amps, why))
    return out


def current_from_parameter(param):
    """Amps from a numeric parameter ({nominal|max, unit}), or None."""
    if not isinstance(param, dict):
        return None
    value = param.get("max", param.get("nominal"))
    scale = {"A": 1.0, "mA": 1e-3, "uA": 1e-6, "µA": 1e-6, "nA": 1e-9}.get(param.get("unit"))
    return None if value is None or scale is None else float(value) * scale


def net_widths(fab, currents, rise=10.0):
    """{net: {"current", "outer", "inner", "track"}}: the IPC-2221 width
    per layer kind, and the net-class track (never below the design
    track)."""
    out = {}
    four = fab["copper"].get("inner_oz") is not None
    for net, (amps, why) in currents.items():
        outer = ipc2221_width(amps, copper_mm(fab, True), True, rise)
        inner = ipc2221_width(amps, copper_mm(fab, False), False, rise) if four else None
        need = max(outer, inner or 0.0)
        out[net] = {"current": amps, "from": why, "outer": outer, "inner": inner,
                    "track": round_up(max(fab["design"]["track"], need))}
    return out


def round_up(v, step=0.005):
    return math.ceil(v / step - 1e-9) * step


# ---------------------------------------------------- microstrip (IPC-2141)


def microstrip_z0(w, h, t, er):
    """Surface microstrip characteristic impedance (Ω), IPC-2141A eq. for
    surface microstrip: Z0 = 87 / √(εr + 1.41) · ln(5.98 h / (0.8 w + t)).
    An approximation, stated for 0.1 < w/h < 2.0 and 1 < εr < 15."""
    return 87.0 / math.sqrt(er + 1.41) * math.log(5.98 * h / (0.8 * w + t))


def edge_coupled_zdiff(w, s, h, t, er):
    """Edge-coupled surface microstrip differential impedance (Ω),
    IPC-2141A: Zdiff = 2 · Z0 · (1 − 0.48 · e^(−0.96 s/h))."""
    return 2.0 * microstrip_z0(w, h, t, er) * (1.0 - 0.48 * math.exp(-0.96 * s / h))


def effective_er(w, h, er):
    """Hammerstad's effective permittivity of a microstrip (w/h >= 1
    form, used throughout as an estimate): (εr+1)/2 + (εr−1)/2 ·
    (1 + 12 h/w)^(−1/2)."""
    return (er + 1) / 2 + (er - 1) / 2 / math.sqrt(1 + 12 * h / w)


def solve_pair_width(zdiff, s, h, t, er, lo=0.02, hi=10.0):
    """The trace width giving `zdiff` at gap `s` (bisection; Zdiff falls
    as w grows), or None when it is out of [lo, hi]."""
    f = lambda w: edge_coupled_zdiff(w, s, h, t, er) - zdiff  # noqa: E731
    if f(lo) < 0 or f(hi) > 0:
        return None
    for _ in range(80):
        mid = (lo + hi) / 2
        if f(mid) > 0:
            lo = mid
        else:
            hi = mid
    return (lo + hi) / 2


# Per-standard defaults. Impedance is the interface standard's, not a
# part fact: USB 2.0 specifies a 90 Ω ±15 % differential impedance for
# its data lines (Universal Serial Bus Specification Revision 2.0,
# chapter 7, Electrical).
DIFF_STANDARDS = {
    "usb2": {"zdiff": 90.0, "tolerance": 0.15, "cite": "USB 2.0 Specification rev 2.0, chapter 7: 90 Ω ±15 %"},
}


def pair_geometry(fab, standard, max_width, pitch=None):
    """Width and gap for a pair: the standard's impedance on the fab's
    stackup at the design clearance as gap, when that width is
    practical (no wider than `max_width`, the formula in its range);
    else the escape width at the pin pitch's gap, with the impedance it
    gives. Returns a dict with `width`, `gap`, `zdiff`, `target`,
    `matched` and a note."""
    st = fab["stackup"]
    h, er = st["dielectric"], st["er"]
    t = copper_mm(fab, True)
    std = DIFF_STANDARDS.get(standard, {"zdiff": None})
    gap = fab["design"]["clearance"]
    target = std.get("zdiff")
    w = solve_pair_width(target, gap, h, t, er) if target else None
    if w is not None and 0.1 <= w / h <= 2.0 and w <= max_width + 1e-9:
        w = max(round_up(w, 0.005), fab["design"]["track"])
        return {"width": w, "gap": gap, "zdiff": edge_coupled_zdiff(w, gap, h, t, er), "target": target,
                "matched": True, "note": f"IPC-2141A edge-coupled microstrip, h={h} mm, εr={er} (approximation)"}
    width = max_width
    gap = max(fab["design"]["clearance"], (pitch - width) if pitch else 0.0)
    z = edge_coupled_zdiff(width, gap, h, t, er)
    why = ("no width in the formula's range" if w is None or not 0.1 <= w / h <= 2.0
           else f"needs {w:.2f} mm, wider than the {max_width:.2f} mm the pins allow")
    return {"width": width, "gap": gap, "zdiff": z, "target": target, "matched": False,
            "note": f"{target} Ω not reachable on h={h} mm, εr={er}: {why}; "
                    f"coupled at {width:.3f}/{gap:.3f} mm, ≈{z:.0f} Ω (IPC-2141A approximation)"}


# ------------------------------------------------------------- geometry


def place(p, pt):
    """A part-frame point on the board: T · R(rot) · M."""
    x, y = pt
    if p.get("side", "top") == "bottom":
        x = -x
    t = math.radians(float(p.get("rot", 0.0)))
    c, s = math.cos(t), math.sin(t)
    return (p.get("x", 0.0) + c * x - s * y, p.get("y", 0.0) + s * x + c * y)


def place_dir(p, d):
    o = place(p, (0.0, 0.0))
    q = place(p, d)
    return (q[0] - o[0], q[1] - o[1])


def pad_box(pad):
    """A pad's axis-aligned extents in its part frame (x0, y0, x1, y1)."""
    cx, cy = pad["position"]["x"], pad["position"]["y"]
    sx, sy = pad["size"]["x"], pad["size"]["y"]
    if round(pad.get("rotation", 0.0)) % 180 == 90:
        sx, sy = sy, sx
    return (cx - sx / 2, cy - sy / 2, cx + sx / 2, cy + sy / 2)


def board_box(box, p):
    pts = [place(p, (x, y)) for x in (box[0], box[2]) for y in (box[1], box[3])]
    return (min(q[0] for q in pts), min(q[1] for q in pts), max(q[0] for q in pts), max(q[1] for q in pts))


def courtyard_box(model):
    pts = [(q["x"], q["y"]) for poly in model["courtyard"]["polygons"] for q in poly]
    return (min(q[0] for q in pts), min(q[1] for q in pts), max(q[0] for q in pts), max(q[1] for q in pts))


def seg_point_dist(a, b, p):
    dx, dy = b[0] - a[0], b[1] - a[1]
    n = dx * dx + dy * dy
    t = 0.0 if n == 0 else max(0.0, min(1.0, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / n))
    return math.hypot(a[0] + t * dx - p[0], a[1] + t * dy - p[1])


def _cross(o, a, b):
    return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])


def segs_intersect(a, b, c, d):
    d1, d2 = _cross(c, d, a), _cross(c, d, b)
    d3, d4 = _cross(a, b, c), _cross(a, b, d)
    return ((d1 > 0) != (d2 > 0)) and ((d3 > 0) != (d4 > 0)) and d1 * d2 < 0 and d3 * d4 < 0


def seg_seg_dist(a, b, c, d):
    if segs_intersect(a, b, c, d):
        return 0.0
    return min(seg_point_dist(a, b, c), seg_point_dist(a, b, d), seg_point_dist(c, d, a), seg_point_dist(c, d, b))


def in_box(p, box):
    return box[0] <= p[0] <= box[2] and box[1] <= p[1] <= box[3]


def seg_box_dist(a, b, box):
    if in_box(a, box) or in_box(b, box):
        return 0.0
    x0, y0, x1, y1 = box
    corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    return min(seg_seg_dist(a, b, corners[i], corners[(i + 1) % 4]) for i in range(4))


# An obstacle or an item: {"kind": "pad"|"track"|"via", "net", "layers":
# set of "F.Cu"/"B.Cu"/"In1.Cu"..., geometry}. Pads: "box"; tracks: "a",
# "b", "width"; vias: "at", "diameter", "drill".


def gap_between(x, y):
    """Copper-to-copper distance between two items (negative: overlap)."""
    def core(i):
        if i["kind"] == "pad":
            return ("box", i["box"], 0.0)
        if i["kind"] == "via":
            return ("seg", (i["at"], i["at"]), i["diameter"] / 2)
        return ("seg", (i["a"], i["b"]), i["width"] / 2)

    kx, gx, rx = core(x)
    ky, gy, ry = core(y)
    if kx == "box" and ky == "box":
        bx, by = gx, gy
        dx = max(bx[0] - by[2], by[0] - bx[2], 0.0)
        dy = max(bx[1] - by[3], by[1] - bx[3], 0.0)
        return math.hypot(dx, dy)
    if kx == "box":
        return seg_box_dist(gy[0], gy[1], gx) - ry
    if ky == "box":
        return seg_box_dist(gx[0], gx[1], gy) - rx
    return seg_seg_dist(gx[0], gx[1], gy[0], gy[1]) - rx - ry


def conflicts(item, others, clearance, hole_gap=None):
    """The first item of another net closer than `clearance` on a shared
    layer (or a hole closer than `hole_gap` to the item's hole)."""
    for o in others:
        if hole_gap is not None and item["kind"] == "via" and o.get("hole"):
            if math.dist(item["at"], o["hole"][0]) < item["drill"] / 2 + o["hole"][1] + hole_gap - 1e-9:
                return o
        if o["net"] == item["net"] and item["net"] is not None:
            continue
        if not (item["layers"] & o["layers"]):
            continue
        if gap_between(item, o) < clearance - 1e-9:
            return o
    return None


# ------------------------------------------------------------ escape width


def escape_axes(model):
    """{pad number: (unit direction in the part frame, across width)} for
    the plated SMD pads that escape outward along their long side, away
    from the land-pattern centroid (the origin)."""
    out = {}
    for pad in model["pads"]:
        if pad["kind"] != "smd" or pad.get("exposed") or not pad["number"]:
            continue
        x0, y0, x1, y1 = pad_box(pad)
        cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
        w, h = x1 - x0, y1 - y0
        if abs(w - h) < 1e-6:
            along_y = abs(cy) > abs(cx)
        else:
            along_y = h > w
        c = cy if along_y else cx
        if abs(c) < 1e-6:
            continue
        sign = 1.0 if c > 0 else -1.0
        out[pad["number"]] = ((0.0, sign) if along_y else (sign, 0.0), w if along_y else h)
    return out


def same_contact(a, b):
    return (a["number"] == b["number"] or a.get("shares_contact_with") == b["number"]
            or b.get("shares_contact_with") == a["number"])


def escape_widths(model, clearance, min_track):
    """Per escaping pad: the widest straight escape track, generalising
    w = min(pad width across the pitch, 2·(pitch − clearance) − pad
    width) to the pads' actual extents. For pad p (width wp across its
    escape axis) and each pad q beside its escape corridor (gap g edge to
    edge, centres d apart across the axis):

      track centred on p clear of q's copper:   w ≤ wp + 2·(g − c)
      p's stub clear of q's parallel stub:      w_p/2 + w_q/2 ≤ d − c,
          split so w_p ≤ 2·(d − c) − min(w_q⁰, d − c)
      never wider than the pad:                 w ≤ wp

    The corridor runs from the pad outward to the courtyard. Returns
    {number: {"width", "direction", "across", "limit", "fits"}}: `fits`
    is False when even `min_track` does not fit."""
    axes = escape_axes(model)
    pads = {p["number"]: p for p in model["pads"] if p["number"] and p["kind"] != "npth"}
    cy = courtyard_box(model)
    first = {}
    info = {}
    for num, (d, across) in axes.items():
        p = pads[num]
        b = pad_box(p)
        ax = 1 if d[1] != 0 else 0  # along-axis index (0: x, 1: y)
        cr = 1 - ax
        lo, hi = (b[ax], cy[2 + ax]) if (d[ax] > 0) else (cy[ax], b[2 + ax])
        w = across
        limit = "pad width"
        stubs = []
        for q in pads.values():
            if same_contact(p, q):
                continue
            qb = pad_box(q)
            if qb[2 + ax] <= lo or qb[ax] >= hi:
                continue  # not beside the corridor
            gap = max(qb[cr] - b[2 + cr], b[cr] - qb[2 + cr])
            if gap < 0:
                continue  # in line with p (behind or ahead), not beside it
            bound = across + 2 * (gap - clearance)
            if bound < w:
                w, limit = bound, f"pad {q['number']} at {gap:.3f} mm"
            if q["number"] in axes and axes[q["number"]][0] == d:
                dist = abs(((qb[cr] + qb[2 + cr]) - (b[cr] + b[2 + cr])) / 2)
                stubs.append((q["number"], dist))
        first[num] = w
        info[num] = {"direction": d, "across": across, "limit": limit, "stubs": stubs}
    out = {}
    for num, e in info.items():
        w = first[num]
        limit = e["limit"]
        for q, dist in e["stubs"]:
            bound = 2 * (dist - clearance) - min(first[q], dist - clearance)
            if bound < w - 1e-12:
                w, limit = bound, f"stub of pad {q} {dist:.3f} mm away"
        w = math.floor(max(w, 0.0) / 0.001 + 1e-6) * 0.001
        out[num] = {"width": w, "direction": e["direction"], "across": e["across"], "limit": limit,
                    "fits": w >= min_track - 1e-9}
    return out


# --------------------------------------------------------------- the plan


def fine_pitch(model, fab=None):
    """A part whose pads need planned escapes: a lead pitch of 0.65 mm or
    less, or (pitch not stated, a USB-C receptacle) pads closer than a
    design track with its clearance on both sides."""
    if model.get("pitch") is not None:
        return model["pitch"] <= FINE_PITCH + 1e-9
    if fab is None or model.get("min_pad_gap") is None:
        return False
    d = fab["design"]
    return model["min_pad_gap"] < d["track"] + 2 * d["clearance"] - 1e-9


def pin_counts(nets, ref):
    """{net: number of the part's pins on it}."""
    out = {}
    for (r, _), net in nets.items():
        if r == ref:
            out[net] = out.get(net, 0) + 1
    return out


def escape_plan(spec, models, placement, nets, fab, widths):
    """Per fine-pitch reference: escape widths per pad (at the design
    clearance, else the fab's minimum clearance inside the courtyard) and
    the per-pad current check. Returns ({ref: {...}}, [errors])."""
    out, errors = {}, []
    design, minimum = fab["design"]["clearance"], fab["min_clearance"]
    for ref, part in spec["components"].items():
        m = models[part]
        if not fine_pitch(m, fab):
            continue
        clearance = design
        esc = escape_widths(m, clearance, fab["min_track"])
        if any(not e["fits"] for num, e in esc.items() if (ref, num) in nets):
            clearance = minimum
            esc = escape_widths(m, clearance, fab["min_track"])
        counts = pin_counts(nets, ref)
        pads = {}
        for num, e in esc.items():
            net = nets.get((ref, num))
            if net is None:
                continue
            need = 0.0
            if net in widths:
                per_pad = widths[net]["current"] / counts[net]
                need = ipc2221_width(per_pad, copper_mm(fab, True), True)
            entry = {**e, "net": net, "need": need}
            if not e["fits"]:
                errors.append(f"{ref}.{num} ({net}): no escape track fits: {e['width']:.3f} mm "
                              f"< fab minimum {fab['min_track']} ({e['limit']}, clearance {clearance})")
            elif need > e["width"] + 1e-9:
                errors.append(f"{ref}.{num} ({net}): its share of the current ({widths[net]['current']:.3g} A "
                              f"/ {counts[net]} pins) needs {need:.3f} mm, the escape fits {e['width']:.3f} mm")
            pads[num] = entry
        out[ref] = {"clearance": clearance, "pads": pads, "courtyard": courtyard_box(m),
                    "placement": placement_of(placement, ref)}
    return out, errors


def placement_of(placement, ref):
    p = placement.get(ref) or {}
    return {"x": p.get("x", 0.0), "y": p.get("y", 0.0), "rot": p.get("rot", 0.0), "side": p.get("side", "top")}


# ------------------------------------------------------------ diff pairs


def diff_pairs(spec, models, nets):
    """The board's differential pairs from the pins' `diff_pair`
    metadata: for a pad whose pin names a partner pin on the same part,
    the two nets on them form a pair. Returns [{"p": net, "n": net,
    "standard", "impedance_ohm", "ends": [(ref, p_pad, n_pad)]}]."""
    found = {}
    for ref, part in spec["components"].items():
        m = models[part]
        by_number = {p["number"]: p for p in m["pads"] if p["number"]}
        for pad in by_number.values():
            dp = (pad.get("pin") or {}).get("diff_pair")
            if not dp or dp.get("polarity") != "p":
                continue
            partner = dp.get("partner_pad") or dp.get("partner")
            pn, nn = nets.get((ref, pad["number"])), nets.get((ref, partner))
            if pn is None or nn is None or pn == nn:
                continue
            key = (pn, nn)
            e = found.setdefault(key, {"p": pn, "n": nn, "standard": dp.get("standard"),
                                       "impedance_ohm": dp.get("impedance_ohm"), "ends": []})
            e["ends"].append((ref, pad["number"], partner))
    return list(found.values())


def kicad_pair_base(p, n):
    """KiCad pairs nets named <base>+/<base>- or <base>P/<base>N."""
    for a, b in (("+", "-"), ("P", "N")):
        if p.endswith(a) and n.endswith(b) and p[: -len(a)] == n[: -len(b)]:
            return p[: -len(a)]
    return None


# ------------------------------------------------------------- .kicad_dru


def dru_text(fab, widths, escapes, pairs, layers=2):
    """The KiCad custom rules (KiCad 7+ syntax; later rules take
    precedence): the current widths per net (outer, inner), then the
    neck-down inside each fine-pitch courtyard, per group of nets with
    the same escape limits, then the diff pairs."""
    o = ["(version 1)",
         f"# Generated by tools/kicad-board from the fab profile ({fab.get('preset', 'inline')}), the nets'",
         "# currents (IPC-2221) and the parts' Placement Models. Do not edit: rebuild."]
    for net, w in sorted(widths.items()):
        o.append(f'(rule "current {net} {w["current"]:g} A"')
        o.append("  (layer outer)")
        o.append(f"  (condition \"A.NetName == '{net}'\")")
        o.append(f"  (constraint track_width (min {mm(max(w['outer'], fab['min_track']))}) (opt {mm(w['track'])})))")
        if layers > 2 and w["inner"] is not None:
            o.append(f'(rule "current {net} {w["current"]:g} A inner"')
            o.append("  (layer inner)")
            o.append(f"  (condition \"A.NetName == '{net}'\")")
            o.append(f"  (constraint track_width (min {mm(max(w['inner'], fab['min_track']))}) (opt {mm(w['track'])})))")
    for p in pairs:
        if not p.get("geometry"):
            continue
        g = p["geometry"]
        cond = f"A.NetName == '{p['p']}' || A.NetName == '{p['n']}'"
        o.append(f'(rule "diff pair {p["p"]}/{p["n"]} ({p["standard"]}, {g["zdiff"]:.0f} Ω est.)"')
        o.append("  (layer outer)")
        o.append(f'  (condition "{cond}")')
        o.append(f"  (constraint track_width (min {mm(fab['min_track'])}) (opt {mm(g['width'])}))")
        base = kicad_pair_base(p["p"], p["n"])
        if base:
            o.append(f"  (constraint diff_pair_gap (min {mm(min(g['gap'], fab['design']['clearance']))}) "
                     f"(opt {mm(g['gap'])})))")
        else:
            o[-1] += ")"
    for ref, e in sorted(escapes.items()):
        groups = {}
        for num, pad in e["pads"].items():
            lo = max(fab["min_track"], round_up(pad["need"], 0.001))
            key = (lo, pad["width"])
            groups.setdefault(key, set()).add(pad["net"])
        for (lo, hi), group in sorted(groups.items(), key=lambda kv: (-kv[0][1], kv[0][0])):
            # A net with pads in several groups gets the narrowest group's
            # limit (rules later in the file win, so the narrower come last).
            names = sorted(group)
            cond = " || ".join(f"A.NetName == '{n}'" for n in names)
            o.append(f'(rule "{ref} escape {mm(hi)}: {", ".join(names[:6])}{" ..." if len(names) > 6 else ""}"')
            o.append(f"  (condition \"A.intersectsCourtyard('{ref}') && ({cond})\")")
            o.append(f"  (constraint track_width (min {mm(lo)}) (opt {mm(hi)}) (max {mm(hi)})))")
        if e["clearance"] < fab["design"]["clearance"] - 1e-9:
            o.append(f'(rule "{ref} escape clearance"')
            o.append(f"  (condition \"A.intersectsCourtyard('{ref}') && B.intersectsCourtyard('{ref}')\")")
            o.append(f"  (constraint clearance (min {mm(e['clearance'])})))")
    return "\n".join(o) + "\n"


def mm(v):
    return f"{v:.3f}mm"


# ---------------------------------------------------------------- fan-out


def layer_of(side):
    return "B.Cu" if side == "bottom" else "F.Cu"


def board_obstacles(spec, models, placement, nets, layers=2):
    """Every pad on the board as an obstacle item."""
    copper = ["F.Cu", "B.Cu"] + [f"In{i}.Cu" for i in range(1, layers - 1)]
    out = []
    for ref, part in spec["components"].items():
        p = placement_of(placement, ref)
        for pad in models[part]["pads"]:
            box = board_box(pad_box(pad), p)
            if pad["kind"] == "smd":
                lay = {layer_of(p["side"])}
            else:
                lay = set(copper) if pad["kind"] == "tht" else set()
            hole = None
            if pad.get("drill"):
                hole = (((box[0] + box[2]) / 2, (box[1] + box[3]) / 2), pad["drill"] / 2)
            out.append({"kind": "pad", "net": nets.get((ref, pad["number"])) if pad["number"] else None,
                        "layers": lay, "box": box, "ref": ref, "number": pad["number"], "hole": hole})
    return out


def bridged_nets(spec, models, nets, pours):
    """{(ref, net)}: signal nets with two or more SMD pads in one
    footprint (USB-C A6/B6): the pcb stage bridges those itself."""
    out = set()
    for ref, part in spec["components"].items():
        at = {}
        for pad in models[part]["pads"]:
            net = nets.get((ref, pad["number"]))
            if net and net not in pours and pad["kind"] == "smd" and not pad.get("shares_contact_with"):
                at.setdefault(net, []).append((pad["position"]["x"], pad["position"]["y"]))
        # As pcb_stage.bridge_pads_in_footprints: two pads within 3 mm.
        out |= {(ref, n) for n, pts in at.items() if len(pts) == 2 and math.dist(*pts) <= 3.0}
    return out


def fanout(spec, models, placement, nets, fab, widths, escapes, pairs, overrides=None, layers=2):
    """Locked copper placed before autorouting, in board mm:

    - every escaping pad of a fine-pitch part gets a straight stub at its
      escape width from its centre to just outside the courtyard (where
      the net's own width takes over);
    - a dog-bone via just outside the courtyard (a short stub, then the
      via) for pads whose net has nothing else on this side to reach:
      pour nets (into the pour), and nets whose other pads are all on
      the other side (decoupling capacitors and a flash under a QFN) or
      through-hole; `overrides` ({ref: {via: [...], straight: [...]}},
      by pad number or net) adds or removes pads;
    - thermal vias in an exposed pad of a pour net;
    - diff pairs: a coupled path between the pair's pins on a fine-pitch
      part and the pads of the next part on both nets.

    `overrides["dogbone"]` lists which of the two automatic reasons apply
    (`pour`, `other_side`; default both), `overrides["pairs"]: false`
    leaves diff pairs to the router.

    Returns (items, report lines)."""
    overrides = dict(overrides or {})
    reasons = set(overrides.pop("dogbone", ["pour", "other_side"]))
    # `stubs: false`: escape stubs only where a diff pair starts.
    all_stubs = overrides.pop("stubs", True)
    pair_pads = {(r, q) for p in pairs for r, a, b in p["ends"] for q in (a, b)}
    if not overrides.pop("pairs", True):
        pairs = []
    pours = set(spec["board"].get("zones", []))
    design = fab["design"]
    clear = design["clearance"]
    hole_gap = max(fab.get("min_hole_clearance", 0.2), 0.25)
    obstacles = board_obstacles(spec, models, placement, nets, layers)
    bridged = bridged_nets(spec, models, nets, pours)
    items, report = [], []
    w, h = spec["board"]["width"], spec["board"]["height"]
    edge = fab.get("min_edge_clearance", 0.25)

    def on_board(pt, r):
        return abs(pt[0]) <= w / 2 - edge - r and abs(pt[1]) <= h / 2 - edge - r

    def class_width(net):
        return widths[net]["track"] if net in widths else design["track"]

    stubs = {}  # (ref, number): (layer, end point, outward direction, pad centre, net, width)
    via_wanted, reserved = [], []
    for ref, e in sorted(escapes.items()):
        part = spec["components"][ref]
        m = models[part]
        p = e["placement"]
        layer = layer_of(p["side"])
        ov = overrides.get(ref) or {}
        force_via = set(map(str, ov.get("via", [])))
        force_straight = set(map(str, ov.get("straight", [])))
        exposed_nets = {nets.get((ref, q["number"])) for q in m["pads"] if q.get("exposed")}
        cy = e["courtyard"]
        for pad in m["pads"]:
            num = pad["number"]
            if num not in e["pads"]:
                continue
            esc = e["pads"][num]
            net = esc["net"]
            if (ref, net) in bridged or pad.get("shares_contact_with"):
                continue
            if not all_stubs and (ref, num) not in pair_pads:
                continue
            if net in pours and net in exposed_nets:
                continue  # tied to the exposed pad (pcb stage) and its thermal vias
            d = esc["direction"]
            b = pad_box(pad)
            cx, cy_ = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
            ax = 0 if d[0] != 0 else 1
            # Distance from the pad centre to the courtyard edge outward.
            to_edge = (cy[2 + ax] - (cx, cy_)[ax]) if d[ax] > 0 else ((cx, cy_)[ax] - cy[ax])
            cw = class_width(net)
            ew = esc["width"]
            # At the escape width until the net's own (wider) width clears
            # the courtyard; the stub ends just outside it.
            narrow = to_edge + (cw if cw > ew else ew) / 2 + 0.01
            reach = narrow + (0.05 if cw > ew else 0.0)
            start = place(p, (cx, cy_))
            bd = place_dir(p, d)
            knee = place(p, (cx + d[0] * narrow, cy_ + d[1] * narrow))
            end = place(p, (cx + d[0] * reach, cy_ + d[1] * reach))
            segs = [(start, knee, ew), (knee, end, cw)]
            new = [{"kind": "track", "net": net, "layers": {layer}, "a": a, "b": b_, "width": wd, "role": "escape"}
                   for a, b_, wd in segs if math.dist(a, b_) > 1e-6]
            items += new
            stubs[(ref, num)] = (layer, end, bd, start, net, ew)
            others = [(r, q) for (r, q), n in nets.items() if n == net and r != ref]
            # Surface pads of the net only on the other side (a flash or
            # decoupling capacitors under the part), through-hole ones
            # reachable from either.
            other_side = (any(on_other_side(spec, models, placement, r, q, layer) for r, q in others)
                          and not any(same_layer_smd(spec, models, placement, r, q, layer) for r, q in others))
            want = (net in pours and "pour" in reasons) or (other_side and "other_side" in reasons)
            if num in force_via or net in force_via:
                want = True
            if num in force_straight or net in force_straight:
                want = False
            if want:
                via_wanted.append((ref, num))
            else:
                # Room kept ahead of a straight stub for the router to
                # take it on: no other net's dog-bone goes there.
                ahead = (end[0] + bd[0] * RESERVE, end[1] + bd[1] * RESERVE)
                reserved.append({"kind": "track", "net": net, "layers": {layer}, "a": end, "b": ahead,
                                 "width": cw, "role": "reserve"})
        # Thermal vias in an exposed pad of a pour net.
        for pad in m["pads"]:
            net = nets.get((ref, pad["number"]))
            if pad.get("exposed") and net in pours:
                box = board_box(pad_box(pad), p)
                items += thermal_vias(box, net, design, hole_gap, layers)

    # Diff pairs first: they need the straightest room.
    for pair in pairs:
        report += route_pair(pair, stubs, spec, models, placement, nets, fab, obstacles, items, on_board)

    for ref, num in via_wanted:
        layer, end, bd, start, net, ew = stubs[(ref, num)]
        placed = dog_bone(end, bd, net, layer, class_width(net), design, clear, hole_gap,
                          obstacles + reserved, items, on_board, layers)
        if placed:
            items += placed
        else:
            report.append(f"fan-out: no room for a via at {ref}.{num} ({net}); straight stub only")
    n_stub = sum(1 for i in items if i.get("role") == "escape")
    n_via = sum(1 for i in items if i["kind"] == "via" and i.get("role") == "dog-bone")
    n_th = sum(1 for i in items if i.get("role") == "thermal")
    report.insert(0, f"fan-out: {n_stub} escape segment(s), {n_via} dog-bone via(s), {n_th} thermal via(s)")
    return items, report


RESERVE = 1.6  # mm kept clear ahead of a straight escape stub


def on_other_side(spec, models, placement, ref, number, layer):
    m = models[spec["components"][ref]]
    p = placement_of(placement, ref)
    for pad in m["pads"]:
        if pad["number"] == number:
            return pad["kind"] == "smd" and layer_of(p["side"]) != layer
    return False


def same_layer_smd(spec, models, placement, ref, number, layer):
    m = models[spec["components"][ref]]
    p = placement_of(placement, ref)
    for pad in m["pads"]:
        if pad["number"] == number:
            return pad["kind"] == "smd" and layer_of(p["side"]) == layer
    return False


def thermal_vias(box, net, design, hole_gap, layers=2):
    """A grid of vias inside an exposed pad, kept a via radius inside its
    edge, at a pitch of at least 1.0 mm (and the hole spacing)."""
    d = design["via_diameter"]
    pitch = max(1.0, design["via_drill"] + hole_gap + 0.05, d + 0.2)
    span_x, span_y = box[2] - box[0] - d, box[3] - box[1] - d
    if span_x < 0 or span_y < 0:
        return []
    nx, ny = int(span_x // pitch) + 1, int(span_y // pitch) + 1
    cx, cy = (box[0] + box[2]) / 2, (box[1] + box[3]) / 2
    out = []
    for i in range(nx):
        for j in range(ny):
            at = (cx + (i - (nx - 1) / 2) * pitch, cy + (j - (ny - 1) / 2) * pitch)
            out.append(via_item(at, net, design, layers, "thermal"))
    return out


def via_item(at, net, design, layers, role="dog-bone"):
    copper = {"F.Cu", "B.Cu"} | {f"In{i}.Cu" for i in range(1, layers - 1)}
    return {"kind": "via", "net": net, "layers": copper, "at": at, "diameter": design["via_diameter"],
            "drill": design["via_drill"], "role": role, "hole": (at, design["via_drill"] / 2)}


def dog_bone(end, bd, net, layer, width, design, clear, hole_gap, obstacles, items, on_board, layers):
    """A via beyond a stub's end, straight out or after a 45° jog to
    either side, nearest first; with the track from the stub's end."""
    r = design["via_diameter"] / 2
    nx, ny = -bd[1], bd[0]  # left of the outward direction
    for out in (r + 0.05, r + 0.25, r + 0.45, r + 0.7, r + 1.0, r + 1.4):
        for lat in (0.0, 0.3, -0.3, 0.5, -0.5, 0.7, -0.7, 0.9, -0.9, 1.2, -1.2):
            if abs(lat) > out:
                continue
            at = (end[0] + bd[0] * out + nx * lat, end[1] + bd[1] * out + ny * lat)
            if not on_board(at, r):
                continue
            via = via_item(at, net, design, layers)
            if conflicts(via, obstacles + items, clear, hole_gap):
                continue
            # Straight out to where the 45° jog starts, then to the via.
            knee = (end[0] + bd[0] * (out - abs(lat)), end[1] + bd[1] * (out - abs(lat)))
            segs = [(end, knee), (knee, at)]
            tracks = [{"kind": "track", "net": net, "layers": {layer}, "a": a, "b": b, "width": width,
                       "role": "dog-bone"} for a, b in segs if math.dist(a, b) > 1e-6]
            if any(conflicts(t, obstacles + items, clear) for t in tracks):
                continue
            return tracks + [via]
    return None


def octilinear(start, d, target):
    """Paths from `start` heading `d` (a unit axis vector) to `target`:
    straight, a 45° jog, straight; the jog at several points along."""
    nx, ny = -d[1], d[0]
    rx, ry = target[0] - start[0], target[1] - start[1]
    u = rx * d[0] + ry * d[1]
    v = rx * nx + ry * ny
    if u < abs(v) - 1e-9:
        return []
    out = []
    for f in (0.0, 0.5, 1.0, 0.25, 0.75):
        k = f * (u - abs(v))
        p1 = (start[0] + d[0] * k, start[1] + d[1] * k)
        p2 = (p1[0] + d[0] * abs(v) + nx * v, p1[1] + d[1] * abs(v) + ny * v)
        pts = [start, p1, p2, target]
        out.append(dedupe(pts))
    return out


def dedupe(pts):
    out = [pts[0]]
    for q in pts[1:]:
        if math.dist(q, out[-1]) > 1e-6:
            out.append(q)
    return out


def offset_polyline(pts, dist):
    """The polyline moved `dist` to its left, with mitred corners."""
    def normal(a, b):
        L = math.dist(a, b)
        return (-(b[1] - a[1]) / L, (b[0] - a[0]) / L)

    out = []
    for i, q in enumerate(pts):
        if i == 0:
            n = normal(pts[0], pts[1])
            out.append((q[0] + n[0] * dist, q[1] + n[1] * dist))
        elif i == len(pts) - 1:
            n = normal(pts[-2], pts[-1])
            out.append((q[0] + n[0] * dist, q[1] + n[1] * dist))
        else:
            n1, n2 = normal(pts[i - 1], q), normal(q, pts[i + 1])
            m = (n1[0] + n2[0], n1[1] + n2[1])
            c = 1 + n1[0] * n2[0] + n1[1] * n2[1]
            out.append((q[0] + m[0] * dist / c, q[1] + m[1] * dist / c))
    return out


def polyline_length(pts):
    return sum(math.dist(a, b) for a, b in zip(pts, pts[1:]))


def route_pair(pair, stubs, spec, models, placement, nets, fab, obstacles, items, on_board):
    """A coupled path for a diff pair from its pins' escape stubs on a
    fine-pitch part to the P and N pads of the next part: the two traces
    run as offsets of one centreline at the pair's width and gap, then
    break out to the pads. Added to `items` when it is clear of
    everything; otherwise the router routes the pair (reported)."""
    g = pair.get("geometry")
    out = []
    for ref, pp, nn in pair["ends"]:
        if (ref, pp) not in stubs or (ref, nn) not in stubs:
            continue
        lay_p, end_p, d, _, _, _ = stubs[(ref, pp)]
        lay_n, end_n, _, _, _, _ = stubs[(ref, nn)]
        if lay_p != lay_n:
            continue
        # The far pads: the nearest surface pad of each net on another
        # part on the same copper side (a series resistor per line).
        mid0 = ((end_p[0] + end_n[0]) / 2, (end_p[1] + end_n[1]) / 2)
        targets = {}
        for (r, q), net in nets.items():
            if r == ref or net not in (pair["p"], pair["n"]):
                continue
            if not same_layer_smd(spec, models, placement, r, q, lay_p):
                continue
            box = next(o["box"] for o in obstacles if o["ref"] == r and o["number"] == q)
            c = ((box[0] + box[2]) / 2, (box[1] + box[3]) / 2)
            if net not in targets or math.dist(mid0, c) < math.dist(mid0, targets[net][1]):
                targets[net] = (r, c)
        if len(targets) < 2:
            out.append(f"diff pair {pair['p']}/{pair['n']}: no surface pads of both nets on {lay_p} beyond {ref}")
            continue
        r_far = "/".join(sorted({targets[pair["p"]][0], targets[pair["n"]][0]}))
        tp, tn = targets[pair["p"]][1], targets[pair["n"]][1]
        width, gap = g["width"], g["gap"]
        half = (width + gap) / 2
        nx, ny = -d[1], d[0]
        # Which side of the centreline P lies on (left: +1).
        side = 1.0 if (end_p[0] - mid0[0]) * nx + (end_p[1] - mid0[1]) * ny > 0 else -1.0
        tmid = mid_of([tp, tn])
        far_side = (tp[0] - tn[0]) * nx + (tp[1] - tn[1]) * ny
        if far_side * side <= 0:
            out.append(f"diff pair {pair['p']}/{pair['n']}: P and N swap sides between {ref} and {r_far}; "
                       "left to the router")
            continue
        spread = abs(far_side) / 2
        lead = 0.3
        best = None
        for back in (spread + 0.3, spread + 0.6, spread + 1.0):
            # The centreline ends `back` short of the far pads' midpoint
            # along d, then each trace breaks out to its pad.
            c_end = (tmid[0] - d[0] * back, tmid[1] - d[1] * back)
            c_start = (mid0[0] + d[0] * lead, mid0[1] + d[1] * lead)
            for centre in octilinear(c_start, d, c_end):
                centre = [mid0] + centre
                lane_p = offset_polyline(centre, side * half)
                lane_n = offset_polyline(centre, -side * half)
                path_p = dedupe([end_p] + lane_p[1:])
                path_n = dedupe([end_n] + lane_n[1:])
                ok = True
                tracks = []
                for path, target, net in ((path_p, tp, pair["p"]), (path_n, tn, pair["n"])):
                    tail = octilinear(path[-1], d, target)
                    if not tail:
                        ok = False
                        break
                    full = dedupe(path + tail[0][1:])
                    tracks.append([{"kind": "track", "net": net, "layers": {lay_p}, "a": a, "b": b,
                                    "width": width, "role": "diff-pair"} for a, b in zip(full, full[1:])])
                if not ok:
                    continue
                every = tracks[0] + tracks[1]
                if any(not on_board(t_["a"], width / 2) or not on_board(t_["b"], width / 2) for t_ in every):
                    continue
                if any(conflicts(t_, obstacles + items + [x for x in every if x["net"] != t_["net"]],
                                 fab["design"]["clearance"]) for t_ in every):
                    continue
                lp = sum(math.dist(x["a"], x["b"]) for x in tracks[0])
                ln = sum(math.dist(x["a"], x["b"]) for x in tracks[1])
                best = (every, lp, ln)
                break
            if best:
                break
        if best is None:
            out.append(f"diff pair {pair['p']}/{pair['n']}: no clear coupled path {ref} -> {r_far}; "
                       "left to the router")
            continue
        items += best[0]
        out.append(f"diff pair {pair['p']}/{pair['n']}: coupled {ref} -> {r_far} at {width:.3f}/{gap:.3f} mm, "
                   f"{best[1]:.2f} / {best[2]:.2f} mm (skew {abs(best[1] - best[2]):.3f} mm)")
    return out


def mid_of(pts):
    pts = list(pts)
    return (sum(p[0] for p in pts) / len(pts), sum(p[1] for p in pts) / len(pts))


# ----------------------------------------------------------------- checks


def zone_boxes(escapes):
    """Board-frame courtyard boxes of the neck-down zones."""
    return {ref: board_box(e["courtyard"], e["placement"]) for ref, e in escapes.items()}


def check_widths(tracks, widths, zones, escapes=None, tol=0.0005):
    """Track widths against the current widths: outside the neck-down
    zones a track is at least its net's IPC-2221 width (outer or inner);
    inside a zone at least its pad share's. `tracks`: [{"net", "layer",
    "width", "a", "b"}] (layer "outer"/"inner"). Returns violations."""
    out = []
    for t in tracks:
        w = widths.get(t["net"])
        if not w:
            continue
        in_zone = [ref for ref, box in zones.items() if seg_box_dist(tuple(t["a"]), tuple(t["b"]), box) <= t["width"] / 2]
        if in_zone:
            need = 0.0
            for ref in in_zone:
                for pad in (escapes or {}).get(ref, {}).get("pads", {}).values():
                    if pad["net"] == t["net"]:
                        need = max(need, pad["need"])
            where = f"in {', '.join(in_zone)}'s escape zone"
        else:
            need = w["outer"] if t["layer"] == "outer" else (w["inner"] or w["outer"])
            where = t["layer"]
        if t["width"] + tol < need:
            out.append({"net": t["net"], "width": t["width"], "need": need, "where": where,
                        "a": t["a"], "b": t["b"]})
    return out


def pair_lengths(tracks, pairs):
    """{(p, n): (length p, length n)} of all tracks of each pair's nets."""
    total = {}
    for t in tracks:
        total[t["net"]] = total.get(t["net"], 0.0) + math.dist(t["a"], t["b"])
    return {(p["p"], p["n"]): (total.get(p["p"], 0.0), total.get(p["n"], 0.0)) for p in pairs}


def skew_ps(length_mm, fab, width):
    """Propagation delay of a length on the outer layer (ps), with the
    stackup's effective permittivity."""
    st = fab["stackup"]
    eeff = effective_er(width, st["dielectric"], st["er"])
    return length_mm * math.sqrt(eeff) / 299.792458 * 1000.0
