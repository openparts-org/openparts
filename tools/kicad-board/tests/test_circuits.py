"""Unit tests for circuits.py (reference circuit expansion) against an
in-memory registry."""

import copy
import sys
from pathlib import Path

import pytest
import yaml

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import circuits  # noqa: E402

MCU = {"id": "t/MCU", "category": "microcontroller",
       "pins": {"1": {"name": "VDD"}, "2": {"name": "VDD"}, "3": {"name": "XIN"}, "4": {"name": "GND"}}}
CAP = {"id": "t/CAP", "category": "capacitor", "pins": {"1": {"name": "1"}, "2": {"name": "2"}}}


def cap(m, mpn, farads, label, volts, review="unreviewed", size="0603", dielectric="X7R"):
    return {"manufacturer": m, "mpn": mpn, "size_code": size, "lifecycle": "active", "existence": "verified",
            "review": {"state": review},
            "parameters": {"capacitance": {"value": farads, "label": label},
                           "rated_voltage": {"value": volts, "label": f"{volts:g} V"},
                           "dielectric": {"value": None, "label": dielectric}}}


CIRCUIT = {
    "id": "t/mcu-power", "title": "T", "host": "t/MCU",
    "nets": {"GND": {"kind": "ground"}, "VDD": {"kind": "power", "voltage": {"typ": 3.3, "unit": "V"}}},
    "connect": {"VDD": {"net": "VDD"}, "GND": {"net": "GND"}},
    "roles": {
        "C_VDD": {"per_pin": "VDD",
                  "requires": {"category": "capacitor", "capacitance": {"typ": 100, "unit": "nF"}},
                  "pins": {"1": "$pin", "2": "GND"}, "level": {"value": "required", "word": "must"},
                  "example": {"dielectric": {"value": "X5R"}}},
        "C_BULK": {"requires": {"category": "capacitor", "capacitance": {"min": 4.7, "unit": "uF"}},
                   "pins": {"1": "VDD", "2": "GND"}, "level": {"value": "recommended", "word": "should"}},
    },
    "provenance": {"/roles/C_VDD/level": [{"source": "t/DS", "page": 3, "locator": "2.1"}]},
}


class FakeRegistry:
    def __init__(self, items):
        self.items = items
        self.queries = []

    def circuit(self, cid):
        assert cid == "t/mcu-power"
        return copy.deepcopy(CIRCUIT)

    def part(self, pid):
        return {"device": "t/MCU" if pid == "t/MCU1" else "t/CAP"}

    def device(self, did):
        return MCU if did == "t/MCU" else CAP

    def catalog(self, q):
        self.queries.append(q)
        q = dict(q)
        if "q" in q:
            return {"items": [i for i in self.items if i["mpn"] == q["q"]]}
        out = self.items
        if "size" in q:
            out = [i for i in out if i["size_code"] == q["size"]]
        return {"items": out}


ITEMS = [
    cap("b", "B100N", 1e-7, "100 nF", 50),
    cap("a", "A100N", 1e-7, "100 nF", 50),
    cap("z", "Z100N", 1e-7, "100 nF", 4.0),             # 4 V < 1.5 x 3.3 V (ours)
    cap("a", "A10U", 1e-5, "10 µF", 10),
    cap("a", "A4U7", 4.7e-6, "4.7 µF", 10),
    cap("a", "A4U7-0402", 4.7e-6, "4.7 µF", 10, size="0402"),
]


def spec(**use):
    return {"components": {"U1": "t/MCU1"}, "nets": {"GND": ["U1.4"]}, "board": {},
            "select": {"filter": {"capacitor": {"size": "0603"}}},
            "use": [{"circuit": "t/mcu-power", "host": "U1", **use}]}


def run(tmp_path, s, items=ITEMS, update=False):
    board = tmp_path / "board.yaml"
    board.touch()
    return circuits.expand(s, board, FakeRegistry(items), out=tmp_path / "out", update=update)


def test_expands_roles_per_pin_into_parts_and_nets(tmp_path):
    s = spec()
    run(tmp_path, s)
    assert s["components"]["C1"] == s["components"]["C2"]
    assert s["nets"]["VDD"] == ["C1.1", "C2.1", "C3.1", "U1.1", "U1.2"]
    assert "C3.2" in s["nets"]["GND"] and "U1.4" in s["nets"]["GND"]
    basis = (tmp_path / "out" / "basis.md").read_text()
    assert 'required (must) (t/DS p3 "2.1")' in basis
    assert "1.5 x 3.3 V working voltage (ours)" in basis


def test_choice_is_deterministic_and_follows_the_tie_break(tmp_path):
    a = spec()
    run(tmp_path, a)
    b = spec()
    run(tmp_path, b, items=list(reversed(ITEMS)), update=True)
    assert a["components"] == b["components"]
    # Same score: manufacturer, then MPN; the 4 V part fails the rule (ours).
    assert a["components"]["C1"] == "t/A100N".replace("t/", "a/")
    # Closest to the `min` bound wins among the bulk candidates, 0603 only.
    assert a["components"]["C3"] == "a/A4U7"


def test_example_facts_and_preference_order(tmp_path):
    items = ITEMS + [cap("c", "C100N", 1e-7, "100 nF", 16, dielectric="X5R")]
    s = spec()
    run(tmp_path, s, items=items, update=True)
    assert s["components"]["C1"] == "c/C100N"           # matches the example X5R
    s = spec()
    s["select"]["prefer"] = {"manufacturers": ["b"]}
    run(tmp_path, s, update=True)
    assert s["components"]["C1"] == "b/B100N"           # board preference
    items = ITEMS + [cap("y", "Y100N", 1e-7, "100 nF", 25, review="reviewed")]
    s = spec()
    run(tmp_path, s, items=items, update=True)
    assert s["components"]["C1"] == "y/Y100N"           # reviewed first


def test_required_roles_need_a_waiver(tmp_path):
    with pytest.raises(circuits.ExpandError, match="waive"):
        run(tmp_path, spec(omit=["C_VDD"]))
    s = spec(omit=["C_VDD"], waive={"C_VDD": "external supply filter"})
    rep = run(tmp_path, s)
    assert "C1" in s["components"] and "C2" not in s["components"]
    assert any("waived" in (r.get("note") or "") for r in rep["rows"])
    # A recommended role only warns.
    rep = run(tmp_path, spec(omit=["C_BULK"]))
    assert any("C_BULK is recommended" in w for w in rep["warnings"])
    # An override that doesn't meet `requires` of a required role, too.
    with pytest.raises(circuits.ExpandError, match="waive"):
        run(tmp_path, spec(override={"C_VDD": {"part": "a/A10U"}}))


def test_unknown_roles_in_board_settings_are_errors(tmp_path):
    with pytest.raises(circuits.ExpandError, match="no role"):
        run(tmp_path, spec(refs={"C_NOPE": "C9"}))


def test_refs_pin_designators(tmp_path):
    s = spec(refs={"C_VDD": ["C10", "C11"], "C_BULK": "C20"})
    run(tmp_path, s)
    assert set(s["components"]) == {"U1", "C10", "C11", "C20"}


def test_the_lock_file_pins_choices_until_update(tmp_path):
    s = spec()
    run(tmp_path, s)
    lock = yaml.safe_load((tmp_path / "board.lock.yaml").read_text())
    assert lock["roles"]["U1 t/mcu-power C_VDD"] == "a/A100N"
    assert lock["circuits"]["t/mcu-power"]
    # A better candidate appears: the lock keeps the old choice...
    items = ITEMS + [cap("c", "C100N", 1e-7, "100 nF", 16, dielectric="X5R")]
    s = spec()
    run(tmp_path, s, items=items)
    assert s["components"]["C1"] == "a/A100N"
    # ...until --update-lock.
    s = spec()
    run(tmp_path, s, items=items, update=True)
    assert s["components"]["C1"] == "c/C100N"
    # A locked part that no longer meets the role is an error.
    (tmp_path / "board.lock.yaml").write_text(yaml.safe_dump(
        {"circuits": {}, "roles": {"U1 t/mcu-power C_VDD": "z/Z100N"}}))
    with pytest.raises(circuits.ExpandError, match="update-lock"):
        run(tmp_path, spec())


def test_formulas_evaluate_in_si():
    assert circuits.evaluate("C1 * C2 / (C1 + C2) + Cs", {"C1": 15e-12, "C2": 15e-12, "Cs": 3e-12}) == \
        pytest.approx(10.5e-12)
    assert circuits.evaluate("-sqrt(4) * 2", {}) == -4.0
    assert circuits.si(5.1, "kΩ") == pytest.approx(5100)
    assert circuits.si(128, "Mbit") == 128 * 2**20
    assert circuits.si(20, "%") == pytest.approx(0.2)
