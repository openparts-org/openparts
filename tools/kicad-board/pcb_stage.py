"""The board half of build.py, run by KiCad's own Python (pcbnew).

  python3 pcb_stage.py OUT_DIR pcb|dsn|ses

pcb  builds OUT_DIR/<name>.kicad_pcb from OUT_DIR/plan.json: footprints
     from the project's openparts.pretty at their placement (unplaced
     ones in a row beside the board), nets on every pad, locked ties
     between pour-net pads of one footprint, the outline and the zones;
     prints a placement report
dsn  exports the Specctra DSN for Freerouting, without the zones: a
     pour net exported with its zone is a plane to Freerouting, which
     then never routes it, and pads the pour cannot reach stay open
ses  [SES OUT_PCB] imports Freerouting's session, refills zones,
     stitches the pours
     (a via wherever a pour fragment on one side overlaps one on the
     other and they are not tied yet) and saves
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

    tie_pads_in_footprints(board)

    # Copper pours.
    inset = 0.3
    for name in spec.get("zones", []):
        for layer in range(spec.get("layers", 2)):
            layer_id = pcbnew.F_Cu if layer == 0 else (pcbnew.B_Cu if layer == spec.get("layers", 2) - 1 else pcbnew.In1_Cu + layer - 1)
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
            # Pads joined solid: a thermal relief's spokes on a pad
            # between tracks are easily starved or land on different
            # pour fragments (DRC's starved_thermal).
            zone.SetPadConnection(pcbnew.ZONE_CONNECTION_FULL)
            board.Add(zone)

    pcbnew.SaveBoard(str(PCB), board)
    report(board, w, h)


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
    keeps its pours on both sides."""
    board = pcbnew.LoadBoard(str(PCB))
    # Every route starts from the unrouted board (a re-run replaces the
    # last result instead of routing around it).
    for track in list(board.GetTracks()):
        if not track.IsLocked():
            board.Remove(track)
    pcbnew.SaveBoard(str(PCB), board)
    if not pcbnew.ExportSpecctraDSN(board, str(OUT / f"{NAME}.dsn")):
        sys.exit("DSN export failed")
    # A fresh copy: removing zones after removing tracks in one session
    # can hang pcbnew.
    board = pcbnew.LoadBoard(str(PCB))
    for zone in list(board.Zones()):
        if zone.GetLayer() != pcbnew.F_Cu:
            board.Remove(zone)
    if not pcbnew.ExportSpecctraDSN(board, str(OUT / f"{NAME}-top.dsn")):
        sys.exit("DSN export failed")


def import_ses():
    """ses [SES OUT_PCB]: by default <name>.ses into <name>.kicad_pcb."""
    ses = Path(sys.argv[3]) if len(sys.argv) > 3 else OUT / f"{NAME}.ses"
    target = Path(sys.argv[4]) if len(sys.argv) > 4 else PCB
    board = pcbnew.LoadBoard(str(PCB))
    ok = pcbnew.ImportSpecctraSES(board, str(ses))
    if not ok:
        sys.exit("SES import failed")
    pcbnew.ZONE_FILLER(board).Fill(board.Zones())
    stitch(board)
    cut_off = sum(len(pads) for net in PLAN["board"].get("zones", [])
                  for pads, _ in clusters(board, net)[1:])
    # Read by build.py to pick among routing results.
    print(f"pour pads cut off: {cut_off}")
    pcbnew.SaveBoard(str(target), board)


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


def clusters(board, net):
    """The net's copper grouped by what joins it (tracks, vias and each
    separate fragment of the filled pour), largest first: a list of
    (pads, {layer: [fragments]}). KiCad's connectivity API treats a zone
    as one item whatever its fragments, so the grouping is done here
    from the shapes."""
    layers = (pcbnew.F_Cu, pcbnew.B_Cu)
    nodes = []  # (pad or None, {layer: polygon}, fragment?)
    for item in list(board.GetPads()) + list(board.GetTracks()):
        if item.GetNetname() == net:
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


def collides(board, new, layer, net, gap):
    """Whether `new` (a track or via) comes within `gap` of copper of
    another net on `layer`."""
    mine = as_polygon(new, layer, gap)
    box = mine.BBox()
    for item in list(board.GetTracks()) + list(board.GetPads()):
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


def hole_clear(board, point, drill):
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


{"pcb": build, "dsn": export_dsn, "ses": import_ses}[MODE]()
