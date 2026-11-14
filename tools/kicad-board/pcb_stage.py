"""The board half of build.py, run by KiCad's own Python (pcbnew).

  python3 pcb_stage.py OUT_DIR pcb|dsn|ses

pcb  builds OUT_DIR/<name>.kicad_pcb from OUT_DIR/plan.json: footprints
     from the project's openparts.pretty at their placement (unplaced
     ones in a row beside the board), nets on every pad, the outline and
     the zones; prints a placement report
dsn  exports the Specctra DSN for Freerouting
ses  imports Freerouting's session, refills zones and saves
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
            zone.SetLocalClearance(mm(0.25))
            zone.SetMinThickness(mm(0.2))
            zone.SetPadConnection(pcbnew.ZONE_CONNECTION_THERMAL)
            board.Add(zone)

    pcbnew.SaveBoard(str(PCB), board)
    report(board, w, h)


def report(board, w, h):
    """Placement report: footprints off the board and courtyard overlaps."""
    edge = pcbnew.BOX2I(to_board(-w / 2, h / 2), pcbnew.VECTOR2I(mm(w), mm(h)))
    boxes = []
    for fp in board.GetFootprints():
        cy = fp.GetCourtyard(pcbnew.F_CrtYd if not fp.IsFlipped() else pcbnew.B_CrtYd)
        box = cy.BBox() if cy.OutlineCount() else fp.GetBoundingBox(False)
        boxes.append((fp.GetReference(), fp.IsFlipped(), box))
        if not edge.Contains(box):
            print(f"off-board: {fp.GetReference()}")
    for i, (ra, fa, a) in enumerate(boxes):
        for rb, fb, b in boxes[i + 1:]:
            if fa == fb and a.Intersects(b):
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
    board = pcbnew.LoadBoard(str(PCB))
    ok = pcbnew.ExportSpecctraDSN(board, str(OUT / f"{NAME}.dsn"))
    if not ok:
        sys.exit("DSN export failed")


def import_ses():
    board = pcbnew.LoadBoard(str(PCB))
    ok = pcbnew.ImportSpecctraSES(board, str(OUT / f"{NAME}.ses"))
    if not ok:
        sys.exit("SES import failed")
    pcbnew.ZONE_FILLER(board).Fill(board.Zones())
    pcbnew.SaveBoard(str(PCB), board)


{"pcb": build, "dsn": export_dsn, "ses": import_ses}[MODE]()
