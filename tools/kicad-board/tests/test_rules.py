"""Unit tests for rules.py (pure Python; run with pytest from the repo root
or tools/kicad-board)."""

import math
import re
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import rules  # noqa: E402


def pad(number, x, y, sx, sy, kind="smd", **extra):
    return {"number": number, "position": {"x": x, "y": y}, "size": {"x": sx, "y": sy}, "rotation": 0.0,
            "kind": kind, "exposed": False, "drill": 1.0 if kind == "tht" else None, **extra}


def quad(pitch=0.4, width=0.2, length=0.8, per_side=4, row=3.0, excess=0.25, ep=None):
    """A QFN-like model: `per_side` pads on each side, pin 1 upper left,
    numbered counter-clockwise; courtyard = lands + excess."""
    pads = []
    n = 1
    offs = [(i - (per_side - 1) / 2) * pitch for i in range(per_side)]
    for o in offs:  # left, top to bottom
        pads.append(pad(str(n), -row, -o, length, width)); n += 1
    for o in offs:  # bottom, left to right
        pads.append(pad(str(n), o, -row, width, length)); n += 1
    for o in offs:  # right, bottom to top
        pads.append(pad(str(n), row, o, length, width)); n += 1
    for o in offs:  # top, right to left
        pads.append(pad(str(n), -o, row, width, length)); n += 1
    if ep:
        pads.append({**pad("EP", 0, 0, ep, ep), "exposed": True})
    e = row + length / 2 + excess
    return {"pitch": pitch, "min_pad_gap": pitch - width, "pads": pads,
            "courtyard": {"polygons": [[{"x": -e, "y": -e}, {"x": e, "y": -e}, {"x": e, "y": e}, {"x": -e, "y": e}]]}}


FAB = {
    "preset": "test", "min_track": 0.1, "min_clearance": 0.1, "min_via_drill": 0.15, "min_via_diameter": 0.25,
    "min_annular_ring": 0.05, "min_hole_clearance": 0.2, "min_edge_clearance": 0.2,
    "copper": {"outer_oz": 1.0, "inner_oz": None},
    "stackup": {"thickness": 1.6, "dielectric": 1.53, "er": 4.5},
    "design": {"track": 0.15, "clearance": 0.15, "via_drill": 0.25, "via_diameter": 0.45},
}


# ---------------------------------------------------------------- fab


def test_presets_load_and_cite_their_source():
    for path in rules.FAB_DIR.glob("*.yaml"):
        fab = rules.load_fab({"fab": {"preset": path.stem}})
        assert fab["source"]["url"].startswith("https://")
        assert re.fullmatch(r"\d{4}-\d{2}-\d{2}", fab["source"]["retrieved"])
        assert fab["design"]["track"] >= fab["min_track"]


def test_fab_override_and_check():
    fab = rules.load_fab({"fab": {"preset": "jlcpcb-2layer-standard", "design": {"track": 0.2}}})
    assert fab["design"]["track"] == 0.2 and fab["design"]["clearance"] == 0.15
    with pytest.raises(ValueError, match="design track"):
        rules.load_fab({"fab": {"preset": "jlcpcb-2layer-standard", "design": {"track": 0.05}}})
    with pytest.raises(ValueError, match="not in"):
        rules.load_fab({"fab": {"preset": "no-such-fab"}})


# ------------------------------------------------------------ IPC-2221


def test_ipc2221_outer_1a_1oz():
    # 1 A, 1 oz (1.37 mil), 10 °C rise, outer: A = (1 / (0.048 · 10^0.44))^(1/0.725)
    # = 16.3 mil², i.e. 11.9 mil = 0.30 mm (the IPC-2221 chart's value).
    w = rules.ipc2221_width(1.0, rules.OZ_MM, outer=True)
    assert w == pytest.approx(0.302, abs=0.002)


def test_ipc2221_inner_needs_more_and_inverts():
    outer = rules.ipc2221_width(0.5, rules.OZ_MM, True)
    inner = rules.ipc2221_width(0.5, rules.OZ_MM, False)
    # k halves: the area grows by 2^(1/0.725) = 2.6x.
    assert inner / outer == pytest.approx(2 ** (1 / 0.725), rel=1e-6)
    assert rules.ipc2221_current(outer, rules.OZ_MM, True) == pytest.approx(0.5, rel=1e-9)
    assert rules.ipc2221_width(0.0, rules.OZ_MM) == 0.0


def test_net_widths_never_below_design_track():
    widths = rules.net_widths(FAB, {"VBUS": (1.0, "x"), "3V3": (0.3, "x")})
    assert widths["3V3"]["track"] == FAB["design"]["track"]
    assert widths["VBUS"]["track"] >= widths["VBUS"]["outer"]
    assert widths["VBUS"]["inner"] is None


def test_current_from_parameter():
    assert rules.current_from_parameter([{"typ": 800, "unit": "mA"}]) == pytest.approx(0.8)
    assert rules.current_from_parameter({"nominal": 800, "unit": "mA"}) == pytest.approx(0.8)
    assert rules.current_from_parameter({"max": 1, "unit": "A"}) == 1.0
    assert rules.current_from_parameter({"typ": 1, "unit": "V"}) is None
    assert rules.current_from_parameter(None) is None


# ----------------------------------------------------------- microstrip


def test_microstrip_matches_ipc2141_formula():
    w, h, t, er = 0.3, 0.2, 0.035, 4.4
    z0 = 87 / math.sqrt(er + 1.41) * math.log(5.98 * h / (0.8 * w + t))
    assert rules.microstrip_z0(w, h, t, er) == pytest.approx(z0)
    zd = 2 * z0 * (1 - 0.48 * math.exp(-0.96 * 0.15 / h))
    assert rules.edge_coupled_zdiff(w, 0.15, h, t, er) == pytest.approx(zd)


def test_pair_width_solves_and_falls_back():
    w = rules.solve_pair_width(90.0, 0.15, 0.2104, 0.035, 4.4)
    assert rules.edge_coupled_zdiff(w, 0.15, 0.2104, 0.035, 4.4) == pytest.approx(90.0, abs=0.01)
    four = {**FAB, "stackup": {"dielectric": 0.2104, "er": 4.4}}
    g = rules.pair_geometry(four, "usb2", escape=0.2)
    assert g["matched"] and g["zdiff"] == pytest.approx(90, abs=3)
    # On a 1.5 mm two-layer board 90 Ω needs ~1.2 mm tracks: not between
    # 0.4 mm-pitch pins. The pair is coupled at the escape width instead.
    g = rules.pair_geometry(FAB, "usb2", escape=0.2, pitch=0.4)
    assert not g["matched"] and g["width"] == 0.2 and g["gap"] == pytest.approx(0.2)
    assert "not reachable" in g["note"]


# --------------------------------------------------------- escape width


def test_escape_qfn_0p4():
    e = rules.escape_widths(quad(pitch=0.4, width=0.2), 0.15, 0.1)
    assert len(e) == 16
    assert all(v["width"] == pytest.approx(0.2) and v["fits"] for v in e.values())
    # Left-side pads escape left, top-side ones up.
    assert e["1"]["direction"] == (-1.0, 0.0) and e["13"]["direction"] == (0.0, 1.0)


def test_escape_is_limited_by_neighbours():
    # 0.3 mm pads at 0.4 mm: 0.1 mm gaps. Centred on its pad, a track
    # keeps 0.15 mm to the neighbouring pads only up to
    # wp + 2(g - c) = 0.3 + 2(0.1 - 0.15) = 0.2 mm.
    e = rules.escape_widths(quad(pitch=0.4, width=0.3), 0.15, 0.1)
    assert e["2"]["width"] == pytest.approx(0.2)
    assert e["2"]["limit"].startswith("pad")
    # 2(pitch - clearance) - pad width, the textbook form, gives the same.
    assert 2 * (0.4 - 0.15) - 0.3 == pytest.approx(0.2)


def test_escape_parallel_stubs_keep_their_clearance():
    # 0.15 mm pads at 0.3 mm: pad-wise 0.15 + 2(0.15 - 0.1) = 0.25 > 0.15,
    # but two stubs 0.3 mm apart with 0.1 clearance: w/2 + w/2 <= 0.2.
    e = rules.escape_widths(quad(pitch=0.3, width=0.15), 0.1, 0.05)
    for v in e.values():
        assert v["width"] <= 0.15 + 1e-9
    m = quad(pitch=0.3, width=0.28)
    e = rules.escape_widths(m, 0.1, 0.05)
    w = e["2"]["width"]
    assert w + e["3"]["width"] <= 2 * (0.3 - 0.1) + 1e-9


def test_escape_too_tight_for_the_fab():
    e = rules.escape_widths(quad(pitch=0.3, width=0.2), 0.15, 0.127)
    assert not e["2"]["fits"] and e["2"]["width"] < 0.127


def test_escape_plan_reports_errors_and_current_share():
    m = quad(pitch=0.4, width=0.2)
    spec = {"components": {"U1": "x/qfn"}, "board": {"zones": []}}
    nets = {("U1", str(n)): "VDD" for n in (1, 5, 9, 13)}
    nets.update({("U1", "2"): "SIG"})
    widths = rules.net_widths(FAB, {"VDD": (0.4, "t")})
    esc, errors = rules.escape_plan(spec, {"x/qfn": m}, {"U1": {"x": 0, "y": 0}}, nets, FAB, widths)
    assert not errors
    need = rules.ipc2221_width(0.1, rules.OZ_MM)  # 0.4 A shared by 4 pins
    assert esc["U1"]["pads"]["1"]["need"] == pytest.approx(need)
    widths = rules.net_widths(FAB, {"VDD": (4.0, "t")})
    _, errors = rules.escape_plan(spec, {"x/qfn": m}, {"U1": {"x": 0, "y": 0}}, nets, FAB, widths)
    assert errors and "needs" in errors[0]


# ------------------------------------------------------------- DRU


def balanced(text):
    depth = 0
    for c in re.sub(r'"[^"]*"', "", text):
        depth += c == "("
        depth -= c == ")"
        assert depth >= 0
    return depth == 0


def test_dru_text():
    m = quad()
    spec = {"components": {"U1": "x/qfn"}, "board": {"zones": []}}
    nets = {("U1", "1"): "VDD", ("U1", "2"): "A", ("U1", "3"): "B"}
    widths = rules.net_widths(FAB, {"VDD": (1.0, "t")})
    esc, _ = rules.escape_plan(spec, {"x/qfn": m}, {"U1": {"x": 0, "y": 0}}, nets, FAB, widths)
    pairs = [{"p": "D+", "n": "D-", "standard": "usb2", "geometry": rules.pair_geometry(FAB, "usb2", 0.2, 0.4)}]
    text = rules.dru_text(FAB, widths, esc, pairs)
    assert text.startswith("(version 1)")
    assert balanced(text)
    assert "(rule \"current VDD 1 A\"" in text and "(min 0.302mm)" in text
    assert "A.intersectsCourtyard('U1')" in text and "(max 0.200mm)" in text
    assert "diff_pair_gap" in text
    # The escape rules come after the current rules (later rules win).
    assert text.index("U1 escape") > text.index("current VDD")


def test_kicad_pair_base():
    assert rules.kicad_pair_base("USB_D+", "USB_D-") == "USB_D"
    assert rules.kicad_pair_base("CLK_P", "CLK_N") == "CLK_"
    assert rules.kicad_pair_base("USB_DP", "USB_DM") is None


# ------------------------------------------------------------ fan-out


def board_spec():
    return {"components": {"U1": "x/qfn", "J1": "x/hdr", "C1": "x/cap"},
            "board": {"width": 30, "height": 20, "zones": ["GND"]}}


def board_models():
    hdr = {"pitch": 2.54, "pads": [pad(str(i + 1), -6.35 + 2.54 * i, 0, 1.7, 1.7, "tht") for i in range(6)],
           "courtyard": {"polygons": [[{"x": -8, "y": -1.5}, {"x": 8, "y": -1.5}, {"x": 8, "y": 1.5}, {"x": -8, "y": 1.5}]]}}
    cap = {"pitch": None, "min_pad_gap": 0.8, "pads": [pad("1", -0.8, 0, 0.9, 1.0), pad("2", 0.8, 0, 0.9, 1.0)],
           "courtyard": {"polygons": [[{"x": -1.5, "y": -0.8}, {"x": 1.5, "y": -0.8}, {"x": 1.5, "y": 0.8}, {"x": -1.5, "y": 0.8}]]}}
    return {"x/qfn": quad(ep=1.2), "x/hdr": hdr, "x/cap": cap}


def test_fanout_stubs_vias_and_clearances():
    spec, models = board_spec(), board_models()
    placement = {"U1": {"x": 0, "y": 0}, "J1": {"x": 0, "y": 7}, "C1": {"x": -6, "y": -5, "side": "bottom"}}
    nets = {("U1", "13"): "A", ("J1", "1"): "A", ("U1", "14"): "B", ("J1", "2"): "B",
            ("U1", "1"): "VDD", ("C1", "1"): "VDD", ("C1", "2"): "GND", ("U1", "EP"): "GND",
            ("U1", "5"): "GND", ("J1", "6"): "GND"}
    widths = rules.net_widths(FAB, {"VDD": (0.2, "t")})
    esc, errors = rules.escape_plan(spec, models, placement, nets, FAB, widths)
    assert not errors and set(esc) == {"U1"}
    items, report = rules.fanout(spec, models, placement, nets, FAB, widths, esc, [])
    stubs = [i for i in items if i.get("role") == "escape"]
    # A, B, VDD get stubs; the GND pin beside a GND exposed pad does not.
    assert {i["net"] for i in stubs} == {"A", "B", "VDD"}
    cy = rules.zone_boxes(esc)["U1"]
    for s in stubs:
        assert s["width"] == pytest.approx(0.2)
    ends = [s["b"] for s in stubs]
    assert all(not rules.in_box(e, (cy[0] + 0.01, cy[1] + 0.01, cy[2] - 0.01, cy[3] - 0.01)) for e in ends)
    # VDD's only other pad is on the bottom: a dog-bone via; A and B go
    # to a through-hole header: straight.
    vias = [i for i in items if i["kind"] == "via" and i.get("role") == "dog-bone"]
    assert [v["net"] for v in vias] == ["VDD"]
    assert any(i.get("role") == "thermal" for i in items)
    # Nothing of one net comes within the clearance of another's.
    obstacles = rules.board_obstacles(spec, models, placement, nets)
    for i in items:
        others = [o for o in items + obstacles if o is not i]
        assert rules.conflicts(i, others, FAB["design"]["clearance"] - 1e-6) is None, i
    assert report[0].startswith("fan-out:")


def test_fanout_diff_pair_is_coupled():
    spec, models = board_spec(), board_models()
    spec["components"].update({"R1": "x/cap", "R2": "x/cap"})
    placement = {"U1": {"x": 0, "y": 0}, "J1": {"x": 0, "y": 7}, "C1": {"x": -6, "y": -5, "side": "bottom"},
                 "R1": {"x": 9, "y": 1.0, "rot": 180}, "R2": {"x": 9, "y": -1.0, "rot": 180}}
    # Right side pads 9..12 bottom to top: 10 = N (lower), 11 = P (upper).
    models["x/qfn"]["pads"][10]["pin"] = {"diff_pair": {"partner": "10", "polarity": "p", "standard": "usb2"}}
    models["x/qfn"]["pads"][9]["pin"] = {"diff_pair": {"partner": "11", "polarity": "n", "standard": "usb2"}}
    nets = {("U1", "11"): "D+", ("U1", "10"): "D-", ("R1", "2"): "D+", ("R2", "2"): "D-"}
    pairs = rules.diff_pairs(spec, models, nets)
    assert pairs == [{"p": "D+", "n": "D-", "standard": "usb2", "impedance_ohm": None, "ends": [("U1", "11", "10")]}]
    esc, _ = rules.escape_plan(spec, models, placement, nets, FAB, {})
    pairs[0]["geometry"] = rules.pair_geometry(FAB, "usb2", 0.2, 0.4)
    items, report = rules.fanout(spec, models, placement, nets, FAB, {}, esc, pairs)
    dp = [i for i in items if i.get("role") == "diff-pair"]
    assert dp and {i["net"] for i in dp} == {"D+", "D-"}
    assert any("coupled" in line for line in report)
    lp = sum(math.dist(i["a"], i["b"]) for i in dp if i["net"] == "D+")
    ln = sum(math.dist(i["a"], i["b"]) for i in dp if i["net"] == "D-")
    assert abs(lp - ln) < 0.5


def test_offset_and_octilinear():
    pts = [(0, 0), (1, 0), (2, 1), (4, 1)]
    left = rules.offset_polyline(pts, 0.1)
    assert left[0] == pytest.approx((0, 0.1))
    assert left[-1] == pytest.approx((4, 1.1))
    for path in rules.octilinear((0, 0), (1, 0), (3, 1)):
        assert path[0] == (0, 0) and path[-1] == (3, 1)
        for a, b in zip(path, path[1:]):
            dx, dy = abs(b[0] - a[0]), abs(b[1] - a[1])
            assert dy < 1e-9 or dx < 1e-9 or abs(dx - dy) < 1e-9  # 0°/90°/45°
    assert rules.octilinear((0, 0), (1, 0), (1, 3)) == []


# ------------------------------------------------------------- checks


def test_check_widths():
    widths = rules.net_widths(FAB, {"VBUS": (1.0, "t")})
    zones = {"U1": (-1, -1, 1, 1)}
    tracks = [
        {"net": "VBUS", "layer": "outer", "width": 0.2, "a": [5, 5], "b": [8, 5]},
        {"net": "VBUS", "layer": "outer", "width": 0.35, "a": [5, 6], "b": [8, 6]},
        {"net": "VBUS", "layer": "outer", "width": 0.2, "a": [0, 0], "b": [2, 0]},
        {"net": "SIG", "layer": "outer", "width": 0.1, "a": [5, 7], "b": [8, 7]},
    ]
    bad = rules.check_widths(tracks, widths, zones)
    assert len(bad) == 1 and bad[0]["a"] == [5, 5]


def test_pair_lengths_and_skew():
    tracks = [{"net": "P", "a": [0, 0], "b": [3, 4], "width": 0.2},
              {"net": "N", "a": [0, 0], "b": [4, 0], "width": 0.2}]
    assert rules.pair_lengths(tracks, [{"p": "P", "n": "N"}]) == {("P", "N"): (5.0, 4.0)}
    # FR-4, ~3.3 effective: about 6 ps/mm.
    assert 5.0 < rules.skew_ps(1.0, FAB, 0.2) < 7.0
