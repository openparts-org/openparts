"""The board half of build.py, run by KiCad's own Python (pcbnew).

  python3 pcb_stage.py OUT_DIR pcb|dsn|dsn-from|ses ...

pcb  builds OUT_DIR/<name>.kicad_pcb from OUT_DIR/plan.json: footprints
     from the project's openparts.pretty at their placement (unplaced
     ones in a row beside the board), nets on every pad, locked ties
     between pour-net pads of one footprint, locked bridges for pad
     pairs a straight track cannot join, silkscreen clipped to the
     outline (such footprints saved to the project's `board` library,
     listed in footprints.json), reference designators placed clear of
     pads, silk and the edge, the outline and the zones; prints a
     placement report
dsn  exports <name>.dsn for Freerouting from the unrouted board (its
     unlocked tracks removed), the pours as planes on both sides
dsn-from BOARD DSN [top|inner]  the DSN of a board, its pours as planes
     (`top`: not on the bottom; `inner`: inner layers only)
ses  [SES OUT_PCB] imports Freerouting's session, drops vias left
     dangling, refills zones, stitches the pours (a via wherever a pour
     fragment on one side overlaps one on the other and they are not
     tied yet), ties pour pads the pour cannot reach, joins starved
     thermal reliefs solid, and saves
"""

import json
import math
import sys
from pathlib import Path

import pcbnew

OUT = Path(sys.argv[1])
MODE = sys.argv[2]
PLAN = json.loads((OUT / "plan.json").read_text())
NAME = PLAN["name"]
PCB = OUT / f"{NAME}.kicad_pcb"
# Board coordinates: the outline's centre at (ORIGIN, ORIGIN) in KiCad's
# page; placement.yaml uses mm from the board centre, X right, Y up.
ORIGIN = 100.0


def mm(v):
    return pcbnew.FromMM(v)


def to_board(x, y):
    return pcbnew.VECTOR2I(mm(ORIGIN + x), mm(ORIGIN - y))


def flip(fp):
    pos = fp.GetPosition()
    try:
        fp.Flip(pos, pcbnew.FLIP_DIRECTION_LEFT_RIGHT)
    except (AttributeError, TypeError):
        fp.Flip(pos, False)


def build():
    board = pcbnew.NewBoard(str(PCB))
    spec = PLAN["board"]
    board.SetCopperLayerCount(spec.get("layers", 2))
    w, h = spec["width"], spec["height"]

    nets = {}

    def net(name):
        if name not in nets:
            item = pcbnew.NETINFO_ITEM(board, name)
            board.Add(item)
            nets[name] = item
        return nets[name]

    # Outline.
    rect = pcbnew.PCB_SHAPE(board)
    rect.SetShape(pcbnew.SHAPE_T_RECT)
    rect.SetLayer(pcbnew.Edge_Cuts)
    rect.SetStart(to_board(-w / 2, h / 2))
    rect.SetEnd(to_board(w / 2, -h / 2))
    rect.SetWidth(mm(0.1))
    board.Add(rect)

    placement = PLAN.get("placement") or {}
    spare_x = w / 2 + 5.0
    for ref, fp_name in PLAN["components"].items():
        fp = pcbnew.FootprintLoad(str(OUT / "openparts.pretty"), fp_name)
        if fp is None:
            sys.exit(f"{ref}: footprint {fp_name} not in openparts.pretty")
        fp.SetFPID(pcbnew.LIB_ID("openparts", fp_name))
        fp.SetReference(ref)
        fp.SetValue(PLAN["values"][ref])
        board.Add(fp)
        p = placement.get(ref)
        if p:
            fp.SetPosition(to_board(p.get("x", 0.0), p.get("y", 0.0)))
            fp.SetOrientationDegrees(float(p.get("rot", 0.0)))
            if p.get("side", "top") == "bottom":
                flip(fp)
        else:
            bbox = fp.GetBoundingBox(False)
            fw = pcbnew.ToMM(bbox.GetWidth())
            fp.SetPosition(to_board(spare_x + fw / 2, h / 2 + 10))
            spare_x += fw + 2.0
        for pad in fp.Pads():
            name = PLAN["nets"].get(f"{ref}.{pad.GetNumber()}")
            if name:
                pad.SetNet(net(name))
            if pad.HasHole() and name in spec.get("zones", []):
                # Thermal spokes on the diagonals: a header's spokes along
                # and across its row run into the next pins and into the
                # tracks alongside the row (starved_thermal); the
                # diagonals find the gaps between them.
                pad.SetThermalSpokeAngleDegrees(45)

    prewire(board, placement.get("prewire") or [], net)
    tie_pads_in_footprints(board)
    bridge_pads_in_footprints(board)
    fitted = save_board_variants(board, clip_silk_to_board(board, w, h))
    (OUT / "footprints.json").write_text(json.dumps(fitted, indent=1, sort_keys=True))
    place_references(board, w, h)

    # Copper pours.
    inset = 0.3
    copper = list(board.GetEnabledLayers().CuStack())
    # On four layers the second inner layer is left to signals: the
    # router gets three routing layers and a ground plane under the top.
    poured = copper if len(copper) == 2 else [l for l in copper if l != copper[2]]
    for name in spec.get("zones", []):
        for layer_id in poured:
            zone = pcbnew.ZONE(board)
            zone.SetLayer(layer_id)
            zone.SetNet(net(name))
            outline = zone.Outline()
            outline.NewOutline()
            for x, y in ((-1, 1), (1, 1), (1, -1), (-1, -1)):
                p = to_board(x * (w / 2 - inset), y * (h / 2 - inset))
                outline.Append(p.x, p.y)
            # The board's own clearance: a wider one keeps the pour out
            # of the gaps between tracks and cuts it into islands.
            zone.SetLocalClearance(mm(spec["rules"]["clearance"]))
            zone.SetMinThickness(mm(0.15))
            # SMD pads joined solid (a thermal relief on a small pad
            # between tracks is easily starved); plated holes through
            # thermal reliefs, for hand soldering, except where the
            # routing starves them (see fix_thermals).
            zone.SetPadConnection(pcbnew.ZONE_CONNECTION_THT_THERMAL)
            board.Add(zone)

    pcbnew.SaveBoard(str(PCB), board)
    report(board, w, h)


def prewire(board, items, net):
    """Locked copper from placement.yaml's `prewire` list, placed before
    routing (the router keeps it and routes around it), in board mm:
    `{net: GND, via: [x, y]}` or `{net: GND, layer: B.Cu, width: 0.3,
    path: [[x, y], ...]}`. For what the router reliably gets wrong: an
    exposed pad's thermal vias and a ground spine out from under a QFN
    to the main pour, so the routing cannot wall the pad in."""
    rules = PLAN["board"]["rules"]
    layers = {"F.Cu": pcbnew.F_Cu, "B.Cu": pcbnew.B_Cu}
    for item in items:
        netinfo = net(item["net"])
        if "via" in item:
            via = pcbnew.PCB_VIA(board)
            via.SetPosition(to_board(*item["via"]))
            via.SetWidth(mm(item.get("diameter", rules["via_diameter"])))
            via.SetDrill(mm(item.get("drill", rules["via_drill"])))
            via.SetNet(netinfo)
            via.SetLocked(True)
            board.Add(via)
            continue
        pts = item["path"]
        for a, b in zip(pts, pts[1:]):
            t = pcbnew.PCB_TRACK(board)
            t.SetStart(to_board(*a))
            t.SetEnd(to_board(*b))
            t.SetWidth(mm(item.get("width", rules["track"])))
            t.SetLayer(layers[item["layer"]])
            t.SetNet(netinfo)
            t.SetLocked(True)
            board.Add(t)
    if items:
        print(f"prewired: {len(items)} item(s)")


SILK_EDGE = 0.25    # KiCad's silk-to-edge check uses the copper edge clearance
SILK_PAD_GAP = 0.15  # silkscreen kept off pads (solder mask openings) by this
REF_SIZES = (1.0, 0.8)  # reference text heights tried, mm


def silk_layers(fp):
    return (pcbnew.B_SilkS, pcbnew.B_Fab) if fp.IsFlipped() else (pcbnew.F_SilkS, pcbnew.F_Fab)


def clip_silk_to_board(board, w, h):
    """Footprint silkscreen beyond the board outline less its clearance
    (a header as long as the board, a USB-C receptacle overhanging the
    edge): lines are clipped to the inside, other shapes that cross it
    are dropped. Returns the footprints changed."""
    changed = {}
    lo_x, hi_x = mm(ORIGIN - w / 2 + SILK_EDGE), mm(ORIGIN + w / 2 - SILK_EDGE)
    lo_y, hi_y = mm(ORIGIN - h / 2 + SILK_EDGE), mm(ORIGIN + h / 2 - SILK_EDGE)
    for fp in board.GetFootprints():
        for item in list(fp.GraphicalItems()):
            if item.GetClass() != "PCB_SHAPE" or item.GetLayer() not in (pcbnew.F_SilkS, pcbnew.B_SilkS):
                continue
            half = item.GetWidth() // 2
            box = item.GetBoundingBox()
            if (box.GetLeft() >= lo_x and box.GetRight() <= hi_x
                    and box.GetTop() >= lo_y and box.GetBottom() <= hi_y):
                continue
            segments = []
            shape = item.GetShape()
            if shape == pcbnew.SHAPE_T_SEGMENT:
                segments = [(item.GetStart(), item.GetEnd())]
            elif shape == pcbnew.SHAPE_T_RECT:
                a, b = item.GetStart(), item.GetEnd()
                corners = [(a.x, a.y), (b.x, a.y), (b.x, b.y), (a.x, b.y)]
                segments = [(pcbnew.VECTOR2I(*corners[i]), pcbnew.VECTOR2I(*corners[(i + 1) % 4]))
                            for i in range(4)]
            for start, end in segments:
                kept = clip_segment(start, end, lo_x + half, hi_x - half, lo_y + half, hi_y - half)
                if kept:
                    line = pcbnew.PCB_SHAPE(fp)
                    line.SetShape(pcbnew.SHAPE_T_SEGMENT)
                    line.SetLayer(item.GetLayer())
                    line.SetWidth(item.GetWidth())
                    line.SetStart(kept[0])
                    line.SetEnd(kept[1])
                    fp.Add(line)
            fp.Remove(item)
            changed[fp.GetReference()] = fp
    return list(changed.values())


BOARD_LIB = "board"


def save_board_variants(board, changed):
    """A footprint whose silkscreen was clipped to this board no longer
    matches its library copy (DRC's lib_footprint_mismatch): each such
    footprint is saved, as placed here, into the project's own `board`
    library (BOARD_LIB.pretty, rebuilt on every run), and the board's
    copy points there. Identical results share one entry (the two
    headers of a symmetric board), others are numbered. Returns
    {reference: "board:<name>"} for the schematic's footprint fields."""
    lib = OUT / f"{BOARD_LIB}.pretty"
    lib.mkdir(exist_ok=True)
    for old in lib.glob("*.kicad_mod"):
        old.unlink()
    names, out = {}, {}
    for fp in changed:
        local = pcbnew.FOOTPRINT(fp)
        if local.IsFlipped():
            flip(local)
        local.SetOrientationDegrees(0)
        local.SetPosition(pcbnew.VECTOR2I(0, 0))
        base = str(fp.GetFPID().GetLibItemName())
        key = (base, tuple(sorted(
            (item.GetLayer(), int(item.GetShape()), round(item.GetStart().x, -3), round(item.GetStart().y, -3),
             round(item.GetEnd().x, -3), round(item.GetEnd().y, -3))
            for item in local.GraphicalItems() if item.GetClass() == "PCB_SHAPE")))
        if key not in names:
            n = sum(1 for k in names if k[0] == base)
            names[key] = f"{base}-{NAME}" + (f"-{n + 1}" if n else "")
            local.SetFPID(pcbnew.LIB_ID(BOARD_LIB, names[key]))
            local.SetReference("REF**")
            pcbnew.PCB_IO_KICAD_SEXPR().FootprintSave(str(lib), local)
        fp.SetFPID(pcbnew.LIB_ID(BOARD_LIB, names[key]))
        out[fp.GetReference()] = f"{BOARD_LIB}:{names[key]}"
    table = OUT / "fp-lib-table"
    text = table.read_text() if table.exists() else "(fp_lib_table\n)\n"
    if f'(name "{BOARD_LIB}")' not in text:
        row = (f'  (lib (name "{BOARD_LIB}")(type "KiCad")(uri "${{KIPRJMOD}}/{BOARD_LIB}.pretty")'
               '(options "")(descr "footprints fitted to this board"))\n')
        cut = text.rstrip().rfind(")")
        table.write_text(text[:cut] + row + text[cut:])
    return out


def clip_segment(a, b, x0, x1, y0, y1):
    """Liang-Barsky: the part of segment a-b inside the box, or None."""
    dx, dy = b.x - a.x, b.y - a.y
    t0, t1 = 0.0, 1.0
    for p, q in ((-dx, a.x - x0), (dx, x1 - a.x), (-dy, a.y - y0), (dy, y1 - a.y)):
        if p == 0:
            if q < 0:
                return None
            continue
        t = q / p
        if p < 0:
            t0 = max(t0, t)
        else:
            t1 = min(t1, t)
        if t0 > t1:
            return None
    if (t1 - t0) * math.hypot(dx, dy) < mm(0.2):
        return None
    return (pcbnew.VECTOR2I(int(a.x + t0 * dx), int(a.y + t0 * dy)),
            pcbnew.VECTOR2I(int(a.x + t1 * dx), int(a.y + t1 * dy)))


def place_references(board, w, h):
    """Each reference designator goes where its text stays clear of every
    pad on its side, of all silkscreen and other designators, and of the
    board edge: tried beside the footprint's courtyard (above, below,
    left, right, then further out), at 1.0 then 0.8 mm; with no room
    anywhere it moves to the fabrication layer."""
    inside = pcbnew.BOX2I(to_board(-w / 2 + SILK_EDGE, h / 2 - SILK_EDGE),
                          pcbnew.VECTOR2I(mm(w - 2 * SILK_EDGE), mm(h - 2 * SILK_EDGE)))
    taken = {pcbnew.F_SilkS: [], pcbnew.B_SilkS: []}
    gap = mm(SILK_PAD_GAP)
    for fp in board.GetFootprints():
        for pad in fp.Pads():
            box = pad.GetBoundingBox()
            box.Inflate(gap)
            for layer, copper in ((pcbnew.F_SilkS, pcbnew.F_Cu), (pcbnew.B_SilkS, pcbnew.B_Cu)):
                if pad.IsOnLayer(copper):
                    taken[layer].append(box)
        for item in fp.GraphicalItems():
            if item.GetLayer() in taken and item.GetClass() == "PCB_SHAPE":
                box = item.GetBoundingBox()
                box.Inflate(mm(0.1))
                taken[item.GetLayer()].append(box)
    # Largest footprints first: they have the fewest spots to choose from.
    order = sorted(board.GetFootprints(), key=lambda f: -f.GetBoundingBox(False).GetArea())
    for fp in order:
        ref = fp.Reference()
        silk, fab = silk_layers(fp)
        cy = fp.GetCourtyard(pcbnew.B_CrtYd if fp.IsFlipped() else pcbnew.F_CrtYd)
        body = cy.BBox() if cy.OutlineCount() else fp.GetBoundingBox(False)
        placed = False
        for size in REF_SIZES:
            ref.SetTextSize(pcbnew.VECTOR2I(mm(size), mm(size)))
            ref.SetTextThickness(mm(size * 0.15))
            for spot, angle in ref_spots(body, size):
                ref.SetLayer(silk)
                ref.SetTextAngleDegrees(angle)
                ref.SetPosition(spot)
                box = ref.GetBoundingBox()
                if not inside.Contains(box):
                    continue
                if any(box.Intersects(t) for t in taken[silk]):
                    continue
                grown = pcbnew.BOX2I(box.GetPosition(), box.GetSize())
                grown.Inflate(mm(0.1))
                taken[silk].append(grown)
                placed = True
                break
            if placed:
                break
        if not placed:
            ref.SetTextSize(pcbnew.VECTOR2I(mm(REF_SIZES[0]), mm(REF_SIZES[0])))
            ref.SetTextAngleDegrees(0)
            ref.SetPosition(body.GetCenter())
            ref.SetLayer(fab)
            print(f"reference {fp.GetReference()}: no room on the silkscreen, on the fab layer")


def ref_spots(body, size):
    """Candidate centres around a box, nearest first: (point, angle)."""
    c = body.GetCenter()
    half_h = mm(size) // 2 + mm(0.15)
    for extra in (0.0, 0.4, 0.8, 1.2, 1.8, 2.5):
        e = mm(extra)
        yield pcbnew.VECTOR2I(c.x, body.GetTop() - half_h - e), 0
        yield pcbnew.VECTOR2I(c.x, body.GetBottom() + half_h + e), 0
        yield pcbnew.VECTOR2I(body.GetLeft() - half_h - e, c.y), 90
        yield pcbnew.VECTOR2I(body.GetRight() + half_h + e, c.y), 90
        for dx in (-1, 1):
            for dy in (-1, 1):
                yield pcbnew.VECTOR2I(c.x + dx * (body.GetWidth() // 2 + half_h + e),
                                      c.y + dy * (body.GetHeight() // 2 + half_h + e)), 0


def tie_pads_in_footprints(board):
    """Before routing: a pour-net pad joined by a short straight track to
    a pad of the same net in the same footprint (a QFN's ground pin to
    its exposed pad, a crystal's two ground pads) when nothing else is in
    the way. Locked, so the router keeps it and routes around it: such a
    pin is otherwise easily walled in by the escape routing, out of
    reach of the pour."""
    rules = PLAN["board"]["rules"]
    width, gap = mm(rules["track"]), mm(rules["clearance"]) + mm(0.01)
    done = set()
    for fp in board.GetFootprints():
        for net in PLAN["board"].get("zones", []):
            pads = [p for p in fp.Pads() if p.GetNetname() == net]
            for a in pads:
                for layer in (pcbnew.F_Cu, pcbnew.B_Cu):
                    if not a.IsOnLayer(layer):
                        continue
                    for b in sorted(pads, key=lambda q: dist(q.GetPosition(), a.GetPosition())):
                        if b is a or not b.IsOnLayer(layer) or b.GetPosition() == a.GetPosition():
                            continue
                        # To the nearest point of b's box, a little inside.
                        box = b.GetBoundingBox()
                        inset = min(mm(0.15), box.GetWidth() // 2, box.GetHeight() // 2)
                        start = a.GetPosition()
                        end = pcbnew.VECTOR2I(min(max(start.x, box.GetLeft() + inset), box.GetRight() - inset),
                                              min(max(start.y, box.GetTop() + inset), box.GetBottom() - inset))
                        if dist(start, end) > mm(2.5) or a.HitTest(end) or (id(b), id(a), layer) in done:
                            continue
                        track = pcbnew.PCB_TRACK(board)
                        track.SetStart(start)
                        track.SetEnd(end)
                        track.SetWidth(width)
                        track.SetLayer(layer)
                        track.SetNet(a.GetNet())
                        track.SetLocked(True)
                        if not collides(board, track, layer, net, gap):
                            board.Add(track)
                            # (b's own tie to a would be the same join.)
                            done.add((id(a), id(b), layer))
                        break


def bridge_pads_in_footprints(board):
    """Before routing: two surface pads of one signal net in the same
    footprint that a straight track cannot join (a USB-C receptacle's
    A6/B6 and A7/B7, each pair straddling a pad of the other) get a
    locked bridge: a stub and a via beside each pad, outward from the
    footprint's centre, and a track between the vias on the other side.
    The router then has nothing left to cross there, which it often
    fails to do on its own. Several such nets in one footprint are
    fitted together (a small search over each net's bridges)."""
    pours = set(PLAN["board"].get("zones", []))
    for fp in board.GetFootprints():
        by_net = {}
        for pad in fp.Pads():
            if pad.GetNetname() and pad.GetNetname() not in pours and not pad.HasHole():
                by_net.setdefault(pad.GetNetname(), {})[(pad.GetPosition().x, pad.GetPosition().y)] = pad
        wanted = []
        for net, spots in sorted(by_net.items()):
            pads = list(spots.values())
            if len(pads) == 2 and dist(pads[0].GetPosition(), pads[1].GetPosition()) <= mm(3):
                if not collides(board, bridge_track(board, pads[0].GetNet(), pads[0].GetPosition(),
                                                    pads[1].GetPosition(), pad_layer(pads[0])),
                                pad_layer(pads[0]), net, bridge_gap()):
                    continue  # the router manages a straight join
                wanted.append((net, pads))
        chosen = fit_bridges(board, fp, wanted, [])
        if chosen is None:
            # Not all of them fit: as many as do, in turn (the router
            # gets the rest).
            chosen = []
            for net, pads in wanted:
                first = next(iter(bridges(board, fp, net, pads, chosen)), None)
                if first:
                    chosen += first
        for item in chosen:
            board.Add(item)
        bridged = {i.GetNetname() for i in chosen}
        for net, _ in wanted:
            print(f"{'bridged' if net in bridged else 'no room to bridge'} {fp.GetReference()} {net}")


def pad_layer(pad):
    return pcbnew.F_Cu if pad.IsOnLayer(pcbnew.F_Cu) else pcbnew.B_Cu


def bridge_gap():
    return mm(PLAN["board"]["rules"]["clearance"]) + mm(0.01)


def bridge_track(board, netinfo, a, b, layer):
    t = pcbnew.PCB_TRACK(board)
    t.SetStart(a)
    t.SetEnd(b)
    t.SetWidth(mm(PLAN["board"]["rules"]["track"]))
    t.SetLayer(layer)
    t.SetNet(netinfo)
    t.SetLocked(True)
    return t


def fit_bridges(board, fp, wanted, placed, tries=30):
    """Items bridging every net in `wanted` clear of the board and of
    each other (backtracking over the first `tries` bridges of each),
    or None."""
    if not wanted:
        return placed
    net, pads = wanted[0]
    for items in list(bridges(board, fp, net, pads, placed))[:tries]:
        rest = fit_bridges(board, fp, wanted[1:], placed + items, tries)
        if rest is not None:
            return rest
    return None


def bridges(board, fp, net, pads, extra):
    """Candidate bridges for one pad pair: first a join on the pads'
    own side running past their ends (out along both pads, across, and
    back: it passes the ends of the pads between them), then stubs and
    vias with a track on the other side, nearest first."""
    rules = PLAN["board"]["rules"]
    via_d, drill = mm(rules["via_diameter"]), mm(rules["via_drill"])
    gap = bridge_gap()
    layer = pad_layer(pads[0])
    other = pcbnew.B_Cu if layer == pcbnew.F_Cu else pcbnew.F_Cu
    netinfo = pads[0].GetNet()
    a, b = pads
    boxes = [a.GetBoundingBox(), b.GetBoundingBox()]
    if all(x.GetHeight() > x.GetWidth() for x in boxes) or all(x.GetWidth() > x.GetHeight() for x in boxes):
        vertical = boxes[0].GetHeight() > boxes[0].GetWidth()
        for e in (0.3, 0.5, 0.8):
            for sign in (1, -1):
                ends = []
                for pad, box in zip(pads, boxes):
                    c = pad.GetPosition()
                    if vertical:
                        ends.append(pcbnew.VECTOR2I(c.x, c.y + sign * (box.GetHeight() // 2 + mm(e))))
                    else:
                        ends.append(pcbnew.VECTOR2I(c.x + sign * (box.GetWidth() // 2 + mm(e)), c.y))
                # Both ends level with the farther pad end.
                if vertical:
                    level = max(p.y for p in ends) if sign > 0 else min(p.y for p in ends)
                    ends = [pcbnew.VECTOR2I(p.x, level) for p in ends]
                else:
                    level = max(p.x for p in ends) if sign > 0 else min(p.x for p in ends)
                    ends = [pcbnew.VECTOR2I(level, p.y) for p in ends]
                # The cross piece in two halves: the exit stub below
                # starts where they meet (Freerouting takes a track end
                # in the middle of another track for unconnected).
                mid = pcbnew.VECTOR2I((ends[0].x + ends[1].x) // 2, (ends[0].y + ends[1].y) // 2)
                items = [bridge_track(board, netinfo, a.GetPosition(), ends[0], layer),
                         bridge_track(board, netinfo, ends[0], mid, layer),
                         bridge_track(board, netinfo, mid, ends[1], layer),
                         bridge_track(board, netinfo, ends[1], b.GetPosition(), layer)]
                if any(collides(board, t, layer, net, gap, extra) for t in items):
                    continue
                # The join gets a via out, on a stub further from the
                # pads: one behind them (under a receptacle's body) is
                # walled in by them, and the router did not attach a
                # route to one in front (in the open) either.
                # An exit the router does not use is pruned on import.
                exit_ = None
                for d in (0.5, 0.7, 0.9, 1.2):
                    v = (pcbnew.VECTOR2I(mid.x, mid.y + sign * mm(d)) if vertical
                         else pcbnew.VECTOR2I(mid.x + sign * mm(d), mid.y))
                    via = pcbnew.PCB_VIA(board)
                    via.SetPosition(v)
                    via.SetWidth(via_d)
                    via.SetDrill(drill)
                    via.SetNet(netinfo)
                    via.SetLocked(True)
                    stub = bridge_track(board, netinfo, mid, v, layer)
                    if (hole_clear(board, v, drill, extra) and not collides(board, stub, layer, net, gap, extra)
                            and not any(collides(board, via, l, net, gap, extra + items) for l in (layer, other))):
                        exit_ = [stub, via]
                        break
                if exit_ is None:
                    continue
                items += exit_
                yield items
    options = []
    for pad in pads:
        c, o = pad.GetPosition(), fp.GetPosition()
        length = max(dist(c, o), 1)
        ux, uy = (c.x - o.x) / length, (c.y - o.y) / length
        box = pad.GetBoundingBox()
        edge = abs(ux) * box.GetWidth() / 2 + abs(uy) * box.GetHeight() / 2
        found = []
        for r in (0.6, 1.0, 1.4, 1.8, 2.2, 2.6, 3.0):
            for side in (0, 0.5, -0.5, 1.0, -1.0):
                d = edge + mm(r)
                v = pcbnew.VECTOR2I(int(c.x + ux * d - uy * mm(side)), int(c.y + uy * d + ux * mm(side)))
                via = pcbnew.PCB_VIA(board)
                via.SetPosition(v)
                via.SetWidth(via_d)
                via.SetDrill(drill)
                via.SetNet(netinfo)
                via.SetLocked(True)
                stub = bridge_track(board, netinfo, c, v, layer)
                if (hole_clear(board, v, drill, extra) and not collides(board, stub, layer, net, gap, extra)
                        and not any(collides(board, via, l, net, gap, extra) for l in (layer, other))):
                    found.append((r + abs(side), via, stub))
        options.append(found)
    pairs = sorted(((ra + rb, va, sa, vb, sb) for ra, va, sa in options[0] for rb, vb, sb in options[1]),
                   key=lambda t: t[0])
    for _, va, sa, vb, sb in pairs:
        if dist(va.GetPosition(), vb.GetPosition()) < drill + mm(0.25):
            continue
        bridge = bridge_track(board, netinfo, va.GetPosition(), vb.GetPosition(), other)
        if not collides(board, bridge, other, net, gap, extra):
            yield [sa, sb, va, vb, bridge]


def dist(p, q):
    return math.hypot(p.x - q.x, p.y - q.y)


def report(board, w, h):
    """Placement report: footprints off the board and courtyard overlaps."""
    edge = pcbnew.BOX2I(to_board(-w / 2, h / 2), pcbnew.VECTOR2I(mm(w), mm(h)))
    shapes = []
    for fp in board.GetFootprints():
        cy = fp.GetCourtyard(pcbnew.F_CrtYd if not fp.IsFlipped() else pcbnew.B_CrtYd)
        box = cy.BBox() if cy.OutlineCount() else fp.GetBoundingBox(False)
        shapes.append((fp.GetReference(), fp.IsFlipped(), cy if cy.OutlineCount() else None, box))
        if not edge.Contains(box):
            print(f"off-board: {fp.GetReference()}")
    # Overlap as DRC sees it: the courtyard polygons (the outline's
    # centre line, not a bounding box grown by the line width) share area.
    for i, (ra, fa, pa, a) in enumerate(shapes):
        for rb, fb, pb, b in shapes[i + 1:]:
            if fa != fb or not a.Intersects(b):
                continue
            if pa is None or pb is None:
                print(f"courtyards overlap (no courtyard, bounding boxes): {ra} {rb}")
                continue
            common = pcbnew.SHAPE_POLY_SET(pa)
            common.BooleanIntersection(pb)
            if common.OutlineCount() and common.Area() > 0:
                print(f"courtyards overlap: {ra} {rb}")
    total = 0.0
    for name in board.GetNetsByName().keys():
        pads = [p for p in board.GetPads() if p.GetNetname() == str(name)]
        if len(pads) > 1 and str(name) not in PLAN["board"].get("zones", []):
            pts = [p.GetPosition() for p in pads]
            # Length of a nearest-neighbour chain: a ratsnest estimate.
            left, cur = pts[1:], pts[0]
            while left:
                nxt = min(left, key=lambda q: (q.x - cur.x) ** 2 + (q.y - cur.y) ** 2)
                total += math.hypot(nxt.x - cur.x, nxt.y - cur.y)
                left.remove(nxt)
                cur = nxt
    print(f"ratsnest (non-pour nets): {pcbnew.ToMM(int(total)):.1f} mm")


def export_dsn():
    """Two DSNs: <name>.dsn with the pours on both sides as planes, and
    <name>-top.dsn with the top one only. With the second, a pour-net
    pad on the bottom gets its own via to the top plane (decoupling
    capacitors under a QFN are easily walled in by its escape routing,
    out of reach of the bottom pour), at the price of more vias to fit;
    build.py routes both and keeps the better board. The board itself
    keeps its pours on both sides. This writes the first; build.py gets
    the second from `dsn-from`."""
    board = pcbnew.LoadBoard(str(PCB))
    # Every route starts from the unrouted board (a re-run replaces the
    # last result instead of routing around it).
    for track in list(board.GetTracks()):
        if not track.IsLocked():
            board.Remove(track)
    pcbnew.SaveBoard(str(PCB), board)
    if not pcbnew.ExportSpecctraDSN(board, str(OUT / f"{NAME}.dsn")):
        sys.exit("DSN export failed")
    # The top-plane DSN comes from `dsn-from` in a second process: pcbnew
    # misbehaves when a board is reloaded or its zones are removed in
    # the session that removed its tracks.


def export_dsn_from():
    """dsn-from BOARD DSN [top|inner]: the DSN of a board as it is, with
    the pours as planes everywhere, not on the bottom (`top`), or on the
    inner layers only (`inner`, four layers: every surface pad of a pour
    net then gets its own via to the inner plane)."""
    board = pcbnew.LoadBoard(sys.argv[3])
    mode = sys.argv[5] if len(sys.argv) > 5 else "all"
    for zone in list(board.Zones()):
        outer = zone.GetLayer() in (pcbnew.F_Cu, pcbnew.B_Cu)
        if (mode == "top" and zone.GetLayer() == pcbnew.B_Cu) or (mode == "inner" and outer):
            board.Remove(zone)
    if not pcbnew.ExportSpecctraDSN(board, sys.argv[4]):
        sys.exit("DSN export failed")


def import_ses():
    """ses [SES OUT_PCB]: by default <name>.ses into <name>.kicad_pcb."""
    ses = Path(sys.argv[3]) if len(sys.argv) > 3 else OUT / f"{NAME}.ses"
    target = Path(sys.argv[4]) if len(sys.argv) > 4 else PCB
    board = pcbnew.LoadBoard(str(PCB))
    ok = pcbnew.ImportSpecctraSES(board, str(ses))
    if not ok:
        sys.exit("SES import failed")
    prune_dangling_vias(board)
    pcbnew.ZONE_FILLER(board).Fill(board.Zones())
    stitch(board)
    fix_thermals(board)
    cut_off = sum(len(pads) for net in PLAN["board"].get("zones", [])
                  for pads, _ in clusters(board, net)[1:])
    # Read by build.py to pick among routing results.
    print(f"pour pads cut off: {cut_off}")
    pcbnew.SaveBoard(str(target), board)


def prune_dangling_vias(board):
    """A via of a signal net that nothing reaches on one of its sides (a
    bridge's exit via the router did not use: DRC's via_dangling) goes,
    with the stub tracks that only led to it."""
    pours = set(PLAN["board"].get("zones", []))
    layers = (pcbnew.F_Cu, pcbnew.B_Cu)
    doomed, removed = {}, []

    def meets(point, reach, net, layer, skip):
        return [o for o in list(board.GetTracks()) + list(board.GetPads())
                if o.GetNetname() == net and o.IsOnLayer(layer) and o.GetClass() != "PCB_VIA"
                and o.m_Uuid.AsString() not in skip and o.HitTest(point, reach)]

    for via in [t for t in board.GetTracks() if t.GetClass() == "PCB_VIA" and t.GetNetname() not in pours]:
        net, at, reach = via.GetNetname(), via.GetPosition(), via.GetWidth(pcbnew.F_Cu) // 2
        touching = {l: meets(at, reach, net, l, ()) for l in layers}
        if all(touching[l] for l in layers):
            continue
        doomed[via.m_Uuid.AsString()] = via
        for l in layers:
            for t in touching[l]:
                if t.GetClass() != "PCB_TRACK":
                    continue
                # A stub that only led to the via: its end there meets
                # nothing else of the net.
                near = t.GetStart() if dist(t.GetStart(), at) < dist(t.GetEnd(), at) else t.GetEnd()
                if not meets(near, t.GetWidth() // 2, net, l, (t.m_Uuid.AsString(),)):
                    doomed[t.m_Uuid.AsString()] = t
        removed.append(f"{net}@({pcbnew.ToMM(at.x) - ORIGIN:.2f}, {ORIGIN - pcbnew.ToMM(at.y):.2f})")
    for item in doomed.values():
        board.Delete(item)
    if removed:
        board.BuildConnectivity()
        print("dangling vias removed:", " ".join(removed))


def fix_thermals(board):
    """A plated hole's thermal relief that ends up with a single spoke on
    a side (DRC's starved_thermal: a side with none is simply not joined
    there) is joined solid instead."""
    zones = list(board.Zones())
    gaps = {z.GetNetname(): z.GetThermalReliefGap() for z in zones}
    fixed = []
    for pad in board.GetPads():
        net = pad.GetNetname()
        if net not in gaps or not pad.HasHole() or pad.GetLocalZoneConnection() == pcbnew.ZONE_CONNECTION_FULL:
            continue
        gap = gaps[net]
        for layer in (pcbnew.F_Cu, pcbnew.B_Cu):
            ring = as_polygon(pad, layer, gap - mm(0.05))
            ring.BooleanSubtract(as_polygon(pad, layer, mm(0.05)))
            spokes = 0
            for f in fragments(board, net, layer):
                common = pcbnew.SHAPE_POLY_SET(ring)
                common.BooleanIntersection(f)
                spokes += common.OutlineCount()
            if spokes == 1:
                pad.SetLocalZoneConnection(pcbnew.ZONE_CONNECTION_FULL)
                fixed.append(f"{pad.GetParentFootprint().GetReference()}.{pad.GetNumber()}")
                break
    if fixed:
        pcbnew.ZONE_FILLER(board).Fill(board.Zones())
        print("thermal reliefs joined solid (starved):", " ".join(fixed))


def fragments(board, net, layer):
    """The filled pour of `net` on `layer`, one poly set per fragment."""
    out = []
    for zone in board.Zones():
        if zone.GetNetname() == net and zone.IsOnLayer(layer) and zone.HasFilledPolysForLayer(layer):
            polys = zone.GetFilledPolysList(layer)
            out += [polys.UnitSet(i) for i in range(polys.OutlineCount())]
    return out


def stitch(board):
    """Ties the two sides' pours together with vias: wherever a fragment
    on the top overlaps one on the bottom and no via of the net joins
    them yet, a via goes inside the overlap, far enough from its edge
    to sit entirely in copper of the net on both sides (so it keeps the
    pour's clearance to everything else); repeated after a refill, as
    new vias reshape the pour. Only holes need their own check."""
    rules = PLAN["board"]["rules"]
    via_d, drill = mm(rules["via_diameter"]), mm(rules["via_drill"])
    hole_gap = mm(0.25)
    gap = mm(rules["clearance"]) + mm(0.01)
    if board.GetCopperLayerCount() != 2:
        return
    added = 0
    for net in PLAN["board"].get("zones", []):
        netinfo = board.FindNet(net)
        for _ in range(4):
            holes = [(p.GetPosition(), max(p.GetDrillSize().x, p.GetDrillSize().y) // 2)
                     for p in board.GetPads() if p.HasHole()]
            holes += [(v.GetPosition(), max(v.GetDrillValue(), drill) // 2)
                      for v in board.GetTracks() if v.GetClass() == "PCB_VIA"]
            vias = [v.GetPosition() for v in board.GetTracks()
                    if v.GetClass() == "PCB_VIA" and v.GetNetname() == net]
            # Pour fragments, and the net's large pads (a via wholly
            # inside a QFN's exposed pad is a thermal via).
            top, bottom = (fragments(board, net, l) + [as_polygon(p, l) for p in board.GetPads()
                                                        if p.GetNetname() == net and p.IsOnLayer(l)
                                                        and not p.HasHole()]
                           for l in (pcbnew.F_Cu, pcbnew.B_Cu))
            new = 0
            for f in top:
                for b in bottom:
                    if any(f.Contains(v) and b.Contains(v) for v in vias):
                        continue
                    overlap_ = pcbnew.SHAPE_POLY_SET(f)
                    overlap_.BooleanIntersection(b)
                    if not overlap_.OutlineCount():
                        continue
                    common = pcbnew.SHAPE_POLY_SET(overlap_)
                    # Room for the via plus a little, inside both.
                    common.Deflate(via_d // 2 + mm(0.05), pcbnew.CORNER_STRATEGY_ROUND_ALL_CORNERS, mm(0.005))

                    def hole_ok(pt):
                        return all((pt.x - h.x) ** 2 + (pt.y - h.y) ** 2 >= (r + drill // 2 + hole_gap) ** 2
                                   for h, r in holes)

                    def make(pt):
                        via = pcbnew.PCB_VIA(board)
                        via.SetPosition(pt)
                        via.SetWidth(via_d)
                        via.SetDrill(drill)
                        via.SetNet(netinfo)
                        return via

                    spot = None
                    for i in range(common.OutlineCount()):
                        chain = common.Outline(i)
                        spot = next((chain.CPoint(k) for k in range(chain.PointCount())
                                     if hole_ok(chain.CPoint(k))), None)
                        if spot:
                            break
                    if spot is None and overlap_.Area() > 0:
                        # A thin overlap: a via centred in it that keeps
                        # its clearance to other nets on both sides.
                        box = overlap_.BBox()
                        step = mm(0.1)
                        pts = [pcbnew.VECTOR2I(x, y)
                               for x in range(box.GetLeft(), box.GetRight() + 1, step)
                               for y in range(box.GetTop(), box.GetBottom() + 1, step)]
                        centre = box.GetCenter()
                        pts.sort(key=lambda q: (q.x - centre.x) ** 2 + (q.y - centre.y) ** 2)
                        for pt in pts:
                            if not overlap_.Contains(pt) or not hole_ok(pt):
                                continue
                            via = make(pt)
                            if not any(collides(board, via, l, net, gap) for l in (pcbnew.F_Cu, pcbnew.B_Cu)):
                                spot = pt
                                break
                    if spot is None:
                        continue
                    # A copy: CPoint() refers into a polygon about to go.
                    spot = pcbnew.VECTOR2I(int(spot.x), int(spot.y))
                    board.Add(make(spot))
                    vias.append(spot)
                    holes.append((spot, drill // 2))
                    new += 1
            added += new
            if not new:
                break
            pcbnew.ZONE_FILLER(board).Fill(board.Zones())
    print(f"stitching vias: {added}")
    for net in PLAN["board"].get("zones", []):
        tie_isolated_pads(board, net)
        drop_dead_islands(board, net)


def drop_dead_islands(board, net):
    """Pour fragments joined to each other by stitching vias but to no
    pad (a corner cut off by the routing on both sides): the vias go, so
    the refill drops the fragments as islands (DRC would otherwise find
    them unconnected from the rest of the net)."""
    dropped = 0
    for items in clusters(board, net, padless=True):
        for item in items:
            if not item.IsLocked():
                board.Delete(item)
                dropped += 1
    if dropped:
        board.BuildConnectivity()
        pcbnew.ZONE_FILLER(board).Fill(board.Zones())
        print(f"dead {net} islands: {dropped} via(s)/track(s) removed")


def clusters(board, net, padless=False):
    """The net's copper grouped by what joins it (tracks, vias and each
    separate fragment of the filled pour), largest first: a list of
    (pads, {layer: [fragments]}), or with `padless` the groups without a
    pad as a list of their tracks and vias. KiCad's connectivity API
    treats a zone as one item whatever its fragments, so the grouping is
    done here from the shapes."""
    layers = list(board.GetEnabledLayers().CuStack())
    nodes = []  # (pad or None, {layer: polygon}, fragment?)
    tracks = {}  # node index: track or via
    for item in list(board.GetPads()) + list(board.GetTracks()):
        if item.GetNetname() == net:
            if item.GetClass() != "PAD":
                tracks[len(nodes)] = item
            nodes.append((item if item.GetClass() == "PAD" else None,
                          {l: as_polygon(item, l) for l in layers if item.IsOnLayer(l)}, False))
    for l in layers:
        for f in fragments(board, net, l):
            nodes.append((None, {l: f}, True))
    parent = list(range(len(nodes)))

    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i

    boxes = [{l: s.BBox() for l, s in shapes.items()} for _, shapes, _ in nodes]
    for i in range(len(nodes)):
        for j in range(i + 1, len(nodes)):
            for l, a in nodes[i][1].items():
                b = nodes[j][1].get(l)
                if b is None or not boxes[i][l].Intersects(boxes[j][l]):
                    continue
                if overlap(a, b):
                    parent[find(i)] = find(j)
                    break
    groups = {}
    for i, (pad, shapes, is_fragment) in enumerate(nodes):
        pads, frags = groups.setdefault(find(i), ([], {l: [] for l in layers}))
        if pad is not None:
            pads.append(pad)
        if is_fragment:
            for l, f in shapes.items():
                frags[l].append(f)
    if padless:
        return [[tracks[i] for i in tracks if find(i) == root] for root, g in groups.items() if not g[0]]
    found = [g for g in groups.values() if g[0]]
    return sorted(found, key=lambda g: len(g[0]), reverse=True)


def overlap(a, b):
    common = pcbnew.SHAPE_POLY_SET(a)
    common.BooleanIntersection(b)
    return common.OutlineCount() > 0 and common.Area() > 0


def as_polygon(item, layer, grow=0):
    poly = pcbnew.SHAPE_POLY_SET()
    item.TransformShapeToPolygon(poly, layer, grow, mm(0.005), pcbnew.ERROR_INSIDE)
    return poly


def collides(board, new, layer, net, gap, extra=()):
    """Whether `new` (a track or via) comes within `gap` of copper of
    another net on `layer` (on the board, or among `extra` items not
    added yet)."""
    mine = as_polygon(new, layer, gap)
    box = mine.BBox()
    for item in list(board.GetTracks()) + list(board.GetPads()) + list(extra):
        if item.GetNetname() == net or not item.IsOnLayer(layer):
            continue
        theirs = as_polygon(item, layer)
        if box.Intersects(theirs.BBox()) and overlap(mine, theirs):
            return True
    return False


def inside(frags, point, margin):
    for f in frags:
        room = pcbnew.SHAPE_POLY_SET(f)
        if margin:
            room.Deflate(margin, pcbnew.CORNER_STRATEGY_ROUND_ALL_CORNERS, mm(0.005))
        if room.OutlineCount() and room.Contains(point):
            return True
    return False


def hole_clear(board, point, drill, extra=()):
    for v in extra:
        if v.GetClass() == "PCB_VIA" and math.hypot(v.GetPosition().x - point.x, v.GetPosition().y - point.y) < drill + mm(0.25):
            return False
    for h in board.GetPads():
        if h.HasHole():
            r = max(h.GetDrillSize().x, h.GetDrillSize().y) // 2
            if math.hypot(h.GetPosition().x - point.x, h.GetPosition().y - point.y) < r + drill // 2 + mm(0.25):
                return False
    for v in board.GetTracks():
        if v.GetClass() == "PCB_VIA" and math.hypot(v.GetPosition().x - point.x, v.GetPosition().y - point.y) < drill + mm(0.25):
            return False
    return True


def tie_isolated_pads(board, net):
    """A pad of a pour net that the main pour does not reach (walled in
    by tracks, or a QFN ground pin beside its exposed pad) gets a short
    track: to the main group's pour on its own side, or to a new via
    inside the main group's pour on the other side (a via wholly inside
    that pour keeps the pour's clearance there)."""
    rules = PLAN["board"]["rules"]
    gap = mm(rules["clearance"]) + mm(0.01)
    width = mm(rules["track"])
    via_d, drill = mm(rules["via_diameter"]), mm(rules["via_drill"])
    netinfo = board.FindNet(net)
    tied, stuck = 0, set()
    while True:
        groups = clusters(board, net)
        if len(groups) < 2:
            break
        main_pads, main_frags = groups[0]
        # Copper the main group owns on each side: its pour and its pads
        # (a via inside a QFN's exposed pad is a thermal via).
        main_copper = {l: main_frags[l] + [as_polygon(q, l) for q in main_pads if q.IsOnLayer(l)]
                       for l in main_frags}
        todo = [p for pads, _ in groups[1:] for p in pads if id_of(p) not in stuck]
        if not todo:
            break
        pad = todo[0]
        layer = pcbnew.F_Cu if pad.IsOnLayer(pcbnew.F_Cu) else pcbnew.B_Cu
        other = pcbnew.B_Cu if layer == pcbnew.F_Cu else pcbnew.F_Cu
        start = pad.GetPosition()
        ends = [q.GetPosition() for q in main_pads if q.IsOnLayer(layer)
                and math.hypot(q.GetPosition().x - start.x, q.GetPosition().y - start.y) < mm(3)]
        for r in (0.6, 0.8, 1.0, 1.2, 1.5, 1.8, 2.2, 2.6, 3.0, 3.5, 4.0, 4.5):
            for a in range(0, 360, 10):
                ends.append(pcbnew.VECTOR2I(int(start.x + mm(r) * math.cos(math.radians(a))),
                                            int(start.y + mm(r) * math.sin(math.radians(a)))))
        ends.sort(key=lambda e: math.hypot(e.x - start.x, e.y - start.y))
        done = False
        # A via in the pad itself first (a QFN's exposed pad walled in by
        # its own pins: thermal vias), then tracks outwards.
        box = pad.GetBoundingBox()
        step = mm(0.4)
        in_pad = [pcbnew.VECTOR2I(x, y)
                  for x in range(box.GetLeft() + step, box.GetRight(), step)
                  for y in range(box.GetTop() + step, box.GetBottom(), step)]
        in_pad.sort(key=lambda e: math.hypot(e.x - start.x, e.y - start.y))
        ends = [e for e in in_pad if pad.HitTest(e)] + ends
        for end in ends:
            in_pad = pad.HitTest(end)
            # The end only has to lie in the main group's copper: the
            # collision checks below keep the clearance to other nets.
            own_side = inside(main_copper[layer], end, 0)
            via_side = not own_side and inside(main_copper[other], end, 0)
            if not (own_side or via_side) or (in_pad and not via_side):
                continue
            if in_pad and not inside([as_polygon(pad, layer)], end, via_d // 2):
                continue
            track = pcbnew.PCB_TRACK(board)
            track.SetStart(start)
            track.SetEnd(end)
            track.SetWidth(width)
            track.SetLayer(layer)
            track.SetNet(netinfo)
            if not in_pad and collides(board, track, layer, net, gap):
                continue
            via = None
            if via_side:
                via = pcbnew.PCB_VIA(board)
                via.SetPosition(end)
                via.SetWidth(via_d)
                via.SetDrill(drill)
                via.SetNet(netinfo)
                if (collides(board, via, layer, net, gap) or collides(board, via, other, net, gap)
                        or not hole_clear(board, end, drill)):
                    continue
            if not in_pad:
                board.Add(track)
            if via:
                board.Add(via)
            done = True
            break
        if done:
            tied += 1
            pcbnew.ZONE_FILLER(board).Fill(board.Zones())
        else:
            ref = pad.GetParentFootprint().GetReference()
            print(f"cannot tie {net} pad {ref}.{pad.GetNumber()} to the main pour")
            # Every pad of its group is then tried in turn.
            stuck.add(id_of(pad))
    print(f"isolated {net} pads tied: {tied}")


def id_of(pad):
    return (pad.GetParentFootprint().GetReference(), pad.GetNumber(), pad.GetPosition().x, pad.GetPosition().y)


{"pcb": build, "dsn": export_dsn, "dsn-from": export_dsn_from, "ses": import_ses}[MODE]()
