"""Reference circuits in a board description (`use:` in board.yaml).

Each `use` entry names a circuit of the OpenParts registry
(Canonical Data Specification section 41) and the board component it
serves:

    use:
      - circuit: raspberrypi/rp2040-minimal
        host: U1
        with: {3V3: 3V3, VBUS: VBUS}       # circuit net -> board net
        refs: {C_IOVDD: [C1, C2], X1: Y1}  # fixed reference designators
        override: {SW_BOOT: {part: omron/B3FS-1050, pins: {...}}}
        omit: [R_CS_PULLUP]
        waive: {C_VREG_VIN: "reason"}      # needed to drop/replace a required role
        nets_as: {QSPI_CSn: QSPI_CS}       # rename generated nets

`expand()` turns them into components, nets and no-connects added to
the spec, chooses a part for every role and writes:

- `board.lock.yaml` next to board.yaml: the chosen part per role and
  each circuit's hash. Later builds reuse it; `--update-lock` chooses
  again.
- `basis.md` / `basis.json` in the build directory: per generated part
  its role, the constraint and where the source states it, the level,
  formula results, the OpenParts rules applied (marked "ours") and the
  circuit's review state.

Part choice: the role's `requires` (a named part, or the catalog
filtered by category and parameters), the board's `select.filter`
(e.g. 0603 chip passives) and the OpenParts derivation rules (ours,
not the source's): a capacitor's rated voltage at least
`select.rules.capacitor_voltage_margin` (default 1.5) times the
highest working voltage of its nets (`board.nets.<NET>.voltage`, else
the circuit net's own). Ties are broken in this order: reviewed
first, more `example` facts matched, the board's `select.prefer`
manufacturer order, closest to a `min`/`max` bound, then manufacturer
and MPN. Same dataset, same choice.
"""

import hashlib
import json
import math
import re
from pathlib import Path
from urllib.parse import quote, urlencode
from urllib.request import urlopen

import yaml

DEFAULT_MARGIN = 1.5

# Reference designator prefix per device category.
PREFIX = {
    "capacitor": "C", "resistor": "R", "inductor": "L", "crystal": "Y",
    "diode": "D", "led": "D", "connector": "J", "switch": "SW", "fuse": "F",
    "transistor": "Q", "oscillator": "X",
}

LEVELS = ("required", "recommended", "example", "optional")


class ExpandError(Exception):
    pass


# ---------------------------------------------------------------- registry


class Registry:
    """GETs JSON from an OpenParts registry (`<url>/v1/...`), cached."""

    def __init__(self, base):
        self.base = base.rstrip("/")
        self.cache = {}

    def get(self, path):
        if path not in self.cache:
            with urlopen(self.base + path) as r:
                self.cache[path] = json.loads(r.read())
        return self.cache[path]

    def circuit(self, cid):
        pub, name = cid.split("/", 1)
        return self.get(f"/v1/circuits/{quote(pub, safe='')}/{quote(name, safe='')}")

    def part(self, pid):
        m, mpn = pid.split("/", 1)
        return self.get(f"/v1/parts/{quote(m, safe='')}/{quote(mpn, safe='')}")

    def device(self, did):
        return self.get(f"/v1/devices/{did}")

    def catalog(self, params):
        return self.get("/v1/catalog?" + urlencode(params))


# ------------------------------------------------------------------ units

_PREFIX = {"p": 1e-12, "n": 1e-9, "u": 1e-6, "µ": 1e-6, "μ": 1e-6, "m": 1e-3, "": 1.0,
           "k": 1e3, "K": 1e3, "M": 1e6, "G": 1e9}
_BASE = ("F", "Ω", "Ω", "ohm", "V", "A", "W", "Hz", "H", "N", "s")


def si(value, unit):
    """`value` in `unit` as SI (`%` and `ppm` as ratios, bits binary)."""
    if unit in ("%",):
        return value * 1e-2
    if unit == "ppm":
        return value * 1e-6
    if unit in ("bit", "Kbit", "kbit", "Mbit", "Gbit"):
        return value * {"bit": 1, "Kbit": 1024, "kbit": 1024, "Mbit": 2**20, "Gbit": 2**30}[unit]
    for base in sorted(_BASE, key=len, reverse=True):
        if unit.endswith(base):
            prefix = unit[: -len(base)]
            if prefix in _PREFIX:
                return value * _PREFIX[prefix]
    raise ExpandError(f"unknown unit {unit!r}")


def stated(v):
    """A circuit value's SI bounds: (typ, min, max), or the enumerated text."""
    if "value" in v and v.get("unit") is None:
        return v["value"]
    unit = v.get("unit")
    f = (lambda x: None if x is None else si(x, unit))
    return f(v.get("typ")), f(v.get("min")), f(v.get("max"))


def label(v):
    s = stated(v)
    if isinstance(s, str):
        return s
    u = v.get("unit", "")
    n = lambda x: f"{x:g} {u}".strip()  # noqa: E731
    if v.get("typ") is not None:
        return n(v["typ"])
    lo, hi = v.get("min"), v.get("max")
    if lo is not None and hi is not None:
        return f"±{n(hi)}" if lo == -hi else f"{n(lo)}..{n(hi)}"
    return f"≥ {n(lo)}" if lo is not None else f"≤ {n(hi)}"


def close(a, b):
    return abs(a - b) <= 1e-6 * max(abs(a), abs(b), 1e-30)


DEVIATION = {"tolerance", "frequency_tolerance", "capacitance_tolerance", "temperature_coefficient",
             "frequency_stability", "aging"}


def meets(key, want, have):
    """Whether catalog value `have` ({value, label}) meets circuit value
    `want`; None when the part doesn't state the key."""
    if have is None:
        return None
    s = stated(want)
    if isinstance(s, str):
        return have.get("label") == s
    v = have.get("value")
    if v is None:
        return None
    typ, lo, hi = s
    if key in DEVIATION:
        bound = max(abs(x) for x in (typ, lo, hi) if x is not None)
        return v <= bound or close(v, bound)
    if typ is not None and lo is None and hi is None:
        return close(v, typ)
    return (lo is None or v >= lo or close(v, lo)) and (hi is None or v <= hi or close(v, hi))


def matches_example(key, want, have):
    if have is None:
        return False
    s = stated(want)
    if isinstance(s, str):
        return have.get("label") == s
    v = have.get("value")
    if v is None:
        return False
    typ, lo, hi = s
    if key in DEVIATION:
        return close(v, max(abs(x) for x in (typ, lo, hi) if x is not None))
    return typ is not None and close(v, typ)


# ----------------------------------------------------------------- pins


def pin_key(n):
    m = re.match(r"^(.*?)(\d+)$", n)
    return (m.group(1), int(m.group(2)), n) if m else (n, -1, n)


def pins_named(device, name):
    return sorted((n for n, p in device["pins"].items() if p["name"] == name), key=pin_key)


def role_pins(device, key):
    """A role pin key -> the device's pin numbers (a number, or every pin
    with that name)."""
    if key in device["pins"]:
        return [key]
    named = pins_named(device, key)
    if not named:
        raise ExpandError(f"{device['id']} has no pin {key!r}")
    return named


# ------------------------------------------------------------------ nets


class Nets:
    """Union-find over pins (`REF.PIN`) and named nets (`#NAME`)."""

    def __init__(self):
        self.up = {}

    def find(self, x):
        self.up.setdefault(x, x)
        while self.up[x] != x:
            self.up[x] = self.up[self.up[x]]
            x = self.up[x]
        return x

    def join(self, a, b):
        ra, rb = self.find(a), self.find(b)
        if ra != rb:
            self.up[max(ra, rb)] = min(ra, rb)

    def groups(self):
        out = {}
        for x in list(self.up):
            out.setdefault(self.find(x), []).append(x)
        return list(out.values())


# ------------------------------------------------------------- expansion


def circuit_hash(c):
    return hashlib.sha256(json.dumps(c, sort_keys=True).encode()).hexdigest()[:16]


def cite_label(c, ptr):
    """`RP-008371-DS@rev1 p152 "2.9.1"` for the nearest citation of `ptr`."""
    prov = c.get("provenance", {})
    p = ptr
    while p:
        if p in prov and prov[p]:
            e = prov[p][0]
            s = e["source"]
            if e.get("page") is not None:
                s += f" p{e['page']}"
            if e.get("locator"):
                s += f' "{e["locator"]}"'
            return s
        p = p.rsplit("/", 1)[0]
    return ""


def esc(k):
    return k.replace("~", "~0").replace("/", "~1")


def level_of(lv):
    if lv is None:
        return "required", None
    if isinstance(lv, str):
        return lv, None
    return lv["value"], lv.get("word")


class Expander:
    def __init__(self, spec, registry, lock, update=False):
        self.spec = spec
        self.reg = registry
        self.lock = lock or {}
        self.update = update
        self.nets = Nets()
        self.no_connect = set()
        self.rows = []          # basis rows, one per generated component
        self.formulas = []
        self.hints = []
        self.connects = []
        self.derived = []
        self.warnings = []
        self.circuits = {}      # id -> hash
        self.new_lock = {}
        self.components = dict(spec["components"])
        self.voltages = {}      # net node -> volts (circuit nets)
        self.select = spec.get("select", {})
        self.margin = float(self.select.get("rules", {}).get("capacitor_voltage_margin", DEFAULT_MARGIN))
        self.rename = {}
        self.local_n = 0
        self.taken = set(self.components)

    # ---- board nets
    def board_nets(self):
        for name, members in self.spec.get("nets", {}).items():
            self.nets.join(f"#{name}", f"#{name}")
            for m in members:
                self.nets.join(f"#{name}", m)
        for m in self.spec.get("no_connect", []):
            self.no_connect.add(m)

    def board_voltage(self, net):
        info = self.spec.get("board", {}).get("nets", {}).get(net)
        if isinstance(info, dict) and info.get("voltage") is not None:
            return float(info["voltage"])
        return None

    # ---- parts
    def device_of_part(self, pid):
        part = self.reg.part(pid)
        return self.reg.device(part["device"])

    def device_of_ref(self, ref):
        if ref not in self.components:
            raise ExpandError(f"host {ref} is not a component")
        return self.device_of_part(self.components[ref])

    # ---- refs
    def next_ref(self, prefix):
        n = 1
        while f"{prefix}{n}" in self.taken:
            n += 1
        return f"{prefix}{n}"

    # ---- the use entries
    def run(self):
        self.board_nets()
        for i, use in enumerate(self.spec.get("use", [])):
            host = use["host"]
            for k in ("with", "refs", "override", "omit", "waive", "nets_as"):
                use.setdefault(k, {} if k != "omit" else [])
            unknown = set(use["omit"]) | set(use["waive"]) | set(use["override"]) | set(use["refs"])
            self.seen_roles = set()
            net_map = {k: ("#" + v) for k, v in use["with"].items()}
            self.instance(use, use["circuit"], host, self.device_of_ref(host), net_map, [use["circuit"]])
            missing = unknown - self.seen_roles
            if missing:
                raise ExpandError(f"use {use['circuit']} on {host}: no role(s) {sorted(missing)}")
            self.rename.update(use["nets_as"])
        return self.finish()

    def instance(self, use, cid, host, host_dev, net_map, path):
        c = self.reg.circuit(cid)
        self.circuits[cid] = circuit_hash(c)
        self.check_host(c, host_dev, host)
        self.local_n += 1
        inst = self.local_n

        def net(name):
            if name in net_map:
                return net_map[name]
            kind = c.get("nets", {}).get(name, {}).get("kind")
            node = f"%{inst}:{name}" if kind == "local" else f"#{name}"
            net_map[name] = node
            return node

        for name, n in c.get("nets", {}).items():
            v = n.get("voltage")
            if v:
                typ, lo, hi = stated(v)
                volts = hi if hi is not None else typ
                if volts is not None:
                    self.voltages.setdefault(self.nets.find(net(name)), volts)
                    self.voltages.setdefault(net(name), volts)

        for pin, conn in c.get("connect", {}).items():
            numbers = pins_named(host_dev, pin)
            if not numbers:
                raise ExpandError(f"{cid}: host {host} has no pin {pin!r}")
            lv, word = level_of(conn.get("level"))
            where = cite_label(c, f"/connect/{esc(pin)}")
            for n in numbers:
                if conn.get("no_connect"):
                    self.no_connect.add(f"{host}.{n}")
                else:
                    self.nets.join(net(conn["net"]), f"{host}.{n}")
            self.connects.append({"circuit": cid, "host": host, "pin": pin, "pins": numbers,
                                  "net": "NC" if conn.get("no_connect") else conn["net"],
                                  "node": None if conn.get("no_connect") else f"{host}.{numbers[0]}",
                                  "level": lv, "word": word, "source": where})

        role_refs = {}
        for rname, role in c.get("roles", {}).items():
            role_refs[rname] = self.role(use, c, cid, rname, role, host, host_dev, net)

        for f_name, f in c.get("formulas", {}).items():
            self.formulas.append(self.formula(c, cid, f_name, f, role_refs))
        for i, h in enumerate(c.get("placement", [])):
            self.hints.append({"circuit": cid, "hint": h, "source": cite_label(c, f"/placement/{i}")})
        for i, h in enumerate(c.get("routing", [])):
            self.hints.append({"circuit": cid, "hint": h, "source": cite_label(c, f"/routing/{i}")})

        for inc in c.get("includes", []):
            child_map = {k: net(v) for k, v in inc.get("with", {}).items()}
            if inc.get("host"):
                refs = role_refs.get(inc["host"]["role"]) or []
                if len(refs) != 1:
                    raise ExpandError(f"{cid}: include {inc['circuit']} needs one {inc['host']['role']}")
                ref, pid = refs[0]
                self.instance(use, inc["circuit"], ref, self.device_of_part(pid), child_map,
                              path + [inc["circuit"]])
            else:
                self.instance(use, inc["circuit"], host, host_dev, child_map, path + [inc["circuit"]])

    def check_host(self, c, dev, host):
        h = c["host"]
        if isinstance(h, str):
            ok = dev["id"] == h
        else:
            ok = (not h.get("devices") or dev["id"] in h["devices"]) and \
                 (not h.get("has_pins") or all(pins_named(dev, p) for p in h["has_pins"]))
        if not ok:
            raise ExpandError(f"{c['id']} is not for {host} ({dev['id']})")

    # ---- one role
    def role(self, use, c, cid, rname, role, host, host_dev, net):
        self.seen_roles.add(rname)
        lv, word = level_of(role["level"])
        key = f"{use['host']} {cid} {rname}"
        at = f"/roles/{esc(rname)}"
        base = {"circuit": cid, "role": rname, "level": lv, "word": word,
                "source": cite_label(c, f"{at}/level"), "requires_source": cite_label(c, f"{at}/requires"),
                "reviewed": "review" in c}
        if rname in use["omit"]:
            self.waivable(use, rname, lv, "omitted")
            self.rows.append({**base, "ref": None, "part": None, "note": "omitted" +
                              (f" (waived: {use['waive'][rname]})" if rname in use["waive"] else "")})
            return []
        if role.get("per_pin"):
            host_pins = pins_named(host_dev, role["per_pin"])
            count = len(host_pins)
        else:
            host_pins = [None] * int(role.get("count") or 1)
            count = len(host_pins)
        pins_map = role.get("pins", {})
        override = use["override"].get(rname, {})
        if override.get("pins"):
            pins_map = override["pins"]
        hosts_include = any((inc.get("host") or {}).get("role") == rname for inc in c.get("includes", []))
        if not pins_map and not hosts_include:
            raise ExpandError(f"{cid} role {rname} has no pins: it can't be placed on a board")

        # The nets each instance's pins land on, for the derived rules.
        def targets(hp):
            out = []
            for k, t in pins_map.items():
                if t == "$pin":
                    out.append(f"{host}.{hp}")
                elif t.startswith("host."):
                    out.extend(f"{host}.{n}" for n in pins_named(host_dev, t[5:]))
                else:
                    out.append(net(t))
            return out

        part, how = self.choose(use, c, cid, rname, role, key, override, [targets(hp) for hp in host_pins])
        dev = self.device_of_part(part)
        prefix = PREFIX.get(dev["category"], "U")
        fixed = use["refs"].get(rname)
        if isinstance(fixed, str):
            fixed = [fixed]
        if fixed and len(fixed) != count:
            raise ExpandError(f"refs.{rname}: {len(fixed)} given for {count} instance(s)")
        out = []
        for i, hp in enumerate(host_pins):
            ref = fixed[i] if fixed else self.next_ref(prefix)
            if ref in self.components and self.components[ref] != part:
                raise ExpandError(f"{ref} is already {self.components[ref]}")
            self.components[ref] = part
            self.taken.add(ref)
            for k, t in pins_map.items():
                for n in role_pins(dev, k):
                    node = f"{ref}.{n}"
                    if t == "$pin":
                        self.nets.join(node, f"{host}.{hp}")
                    elif t.startswith("host."):
                        hps = pins_named(host_dev, t[5:])
                        if not hps:
                            raise ExpandError(f"{cid}: host {host} has no pin {t[5:]!r}")
                        for h in hps:
                            self.nets.join(node, f"{host}.{h}")
                    else:
                        self.nets.join(node, net(t))
            out.append((ref, part))
            self.rows.append({**base, "ref": ref, "host_pin": f"{host}.{hp}" if hp else None,
                              "part": part, "constraint": requirement_label(role["requires"]),
                              "selection": how})
        return out

    def waivable(self, use, rname, lv, what):
        if lv == "required" and rname not in use["waive"]:
            raise ExpandError(f"{rname} is required by its source: {what} only with `waive: {{{rname}: <reason>}}`")
        if lv == "recommended" and rname not in use["waive"]:
            self.warnings.append(f"{rname} is recommended by its source and was {what}")

    # ---- part choice
    def choose(self, use, c, cid, rname, role, key, override, instance_nets):
        req = role["requires"]
        lv, _ = level_of(role["level"])
        derived = self.derived_rules(req, instance_nets)
        how = {"derived": derived}
        if override.get("part"):
            part = override["part"]
            ok, unchecked = self.check_part(part, req, derived)
            if not ok:
                self.waivable(use, rname, lv, f"replaced by {part}, which doesn't meet `requires`")
            how.update(rule="override", unchecked=unchecked,
                       waiver=use["waive"].get(rname))
            self.new_lock[key] = part
            return part, how
        if req.get("part"):
            self.new_lock[key] = req["part"]
            return req["part"], {**how, "rule": "named by the source"}
        locked = self.lock.get("roles", {}).get(key)
        if locked and not self.update:
            ok, unchecked = self.check_part(locked, req, derived)
            if not ok:
                raise ExpandError(f"{key}: locked {locked} no longer meets the role; run with --update-lock")
            self.new_lock[key] = locked
            return locked, {**how, "rule": "board.lock.yaml", "unchecked": unchecked}
        part, info = self.select_part(req, role.get("example") or {}, derived, key)
        self.new_lock[key] = part
        return part, {**how, **info}

    def derived_rules(self, req, instance_nets):
        """OpenParts' own rules (not the source's) for this role."""
        cats = req.get("category")
        cats = cats if isinstance(cats, list) else [cats]
        out = {}
        if "capacitor" in cats and "rated_voltage" not in req:
            volts = 0.0
            for nets in instance_nets:
                for n in nets:
                    v = self.net_voltage(n)
                    if v is not None:
                        volts = max(volts, v)
            if volts > 0:
                need = self.margin * volts
                out["rated_voltage"] = {"min": need, "unit": "V",
                                        "why": f"{self.margin:g} x {volts:g} V working voltage (ours)"}
        return out

    def net_voltage(self, node):
        root = self.nets.find(node)
        best = None
        for g in self.nets.groups():
            if self.nets.find(g[0]) != root:
                continue
            for x in g:
                if x.startswith("#"):
                    v = self.board_voltage(x[1:])
                    if v is not None:
                        best = max(best or 0, v)
                if x in self.voltages:
                    best = max(best or 0, self.voltages[x])
        if best is None and root in self.voltages:
            best = self.voltages[root]
        return best

    def candidates(self, req):
        cats = req.get("category")
        cats = cats if isinstance(cats, list) else [cats]
        out = []
        for cat in cats:
            q = [("category", cat), ("limit", "500")]
            for k, v in self.select.get("filter", {}).get(cat, {}).items():
                q.append((k, str(v)))
            for k, v in req.items():
                if k in ("category", "part") or not isinstance(v, dict):
                    continue
                if v.get("typ") is not None and v.get("min") is None and v.get("max") is None \
                        and k not in DEVIATION and v.get("unit"):
                    q.append((k, f"{v['typ']:g}{v['unit']}"))
            out.extend(self.reg.catalog(q)["items"])
        return out

    def check_part(self, pid, req, derived):
        """(meets, unchecked keys) of a named part against `requires`."""
        m, mpn = pid.split("/", 1)
        part = self.reg.part(pid)
        dev = self.reg.device(part["device"])
        cats = req.get("category")
        cats = cats if isinstance(cats, list) else [cats] if cats else []
        if cats and not any(dev["category"] == c or c in category_parents(dev["category"]) for c in cats):
            return False, []
        items = self.reg.catalog([("q", mpn), ("limit", "50")])["items"]
        item = next((i for i in items if i["manufacturer"] == m and i["mpn"] == mpn), None)
        params = item["parameters"] if item else {}
        ok, unchecked = True, []
        for k, v in {**params_of(req), **{k: d for k, d in derived.items()}}.items():
            r = meets(k, v, params.get(k))
            if r is None:
                unchecked.append(k)
            elif not r:
                ok = False
        return ok, unchecked

    def select_part(self, req, example, derived, key):
        wanted = {**params_of(req), **derived}
        pool = []
        for it in self.candidates(req):
            if it.get("lifecycle") in ("obsolete", "eol", "nrnd") or it.get("existence") == "rejected":
                continue
            fails, unchecked = False, []
            for k, v in wanted.items():
                r = meets(k, v, it["parameters"].get(k))
                if r is None:
                    unchecked.append(k)
                elif not r:
                    fails = True
            if not fails:
                pool.append((it, unchecked))
        if not pool:
            raise ExpandError(f"{key}: no part in the catalog meets {sorted(wanted)}")
        # Parts that state every constrained key beat those that don't.
        fewest = min(len(u) for _, u in pool)
        pool = [(it, u) for it, u in pool if len(u) == fewest]
        prefer = self.select.get("prefer", {}).get("manufacturers", [])

        def ex_matches(it):
            n = 0
            for k, v in params_of(example).items():
                n += matches_example(k, v, it["parameters"].get(k))
            if example.get("size") and it.get("size_code") == example["size"]:
                n += 1
            if example.get("part") and f"{it['manufacturer']}/{it['mpn']}" == example["part"]:
                n += 1
            return n

        def closeness(it):
            d = 0.0
            for k, v in wanted.items():
                have = it["parameters"].get(k, {}).get("value")
                s = stated(v)
                if have is None or isinstance(s, str) or k in DEVIATION:
                    continue
                typ, lo, hi = s
                if typ is None and (lo or hi) and have > 0:
                    d += abs(math.log(have / (lo or hi)))
            return d

        def rank(entry):
            it, _ = entry
            return (it["review"]["state"] != "reviewed", -ex_matches(it),
                    prefer.index(it["manufacturer"]) if it["manufacturer"] in prefer else len(prefer),
                    round(closeness(it), 9), it["manufacturer"], it["mpn"])

        pool.sort(key=rank)
        best, unchecked = pool[0]
        return f"{best['manufacturer']}/{best['mpn']}", {
            "rule": "catalog", "candidates": len(pool), "example_matches": ex_matches(best),
            "unchecked": unchecked}

    # ---- formulas
    def formula(self, c, cid, name, f, role_refs):
        def value(inp):
            if "role" in inp:
                refs = role_refs.get(inp["role"]) or []
                if not refs:
                    return None, f"{inp['role']} (omitted)"
                pid = refs[0][1]
                v = self.part_param(pid, inp["param"])
                if v is None:
                    req = c["roles"][inp["role"]]["requires"].get(inp["param"])
                    return (stated(req)[0], f"{label(req)} (required; {pid} doesn't state it)") if req else (None, "?")
                return v["value"], f"{v['label']} ({pid})"
            typ = stated(inp)[0]
            return typ, label(inp)
        env, shown = {}, []
        for k, inp in f.get("inputs", {}).items():
            env[k], text = value(inp)
            shown.append(f"{k} = {text}")
        result = None
        if all(v is not None for v in env.values()):
            result = evaluate(f["expr"], env)
        unit = f.get("unit") or (f.get("result") or {}).get("unit")
        target = None
        if f.get("target"):
            t = f["target"]
            refs = role_refs.get(t["role"]) or []
            tv = self.part_param(refs[0][1], t["param"]) if refs else None
            target = f"{t['role']} {t['param']} " + (tv["label"] if tv else "?")
        return {"circuit": cid, "name": name, "expr": f["expr"], "inputs": shown,
                "result": None if result is None else f"{result / si(1, unit):.4g} {unit}" if unit else f"{result:.4g}",
                "stated": label(f["result"]) if f.get("result") else None,
                "target": target, "source": cite_label(c, f"/formulas/{esc(name)}"),
                "judged": "shown, not judged (the source states no tolerance)"}

    def part_param(self, pid, key):
        m, mpn = pid.split("/", 1)
        items = self.reg.catalog([("q", mpn), ("limit", "50")])["items"]
        item = next((i for i in items if i["manufacturer"] == m and i["mpn"] == mpn), None)
        return item["parameters"].get(key) if item else None

    # ---- the result
    def finish(self):
        groups = self.nets.groups()
        names = {}
        used = set()
        for g in groups:
            named = sorted({x[1:] for x in g if x.startswith("#")})
            if len(named) > 1:
                raise ExpandError(f"nets {named} are joined by the circuits")
            if named:
                names[id(g)] = named[0]
                used.add(named[0])
        local_names = {}
        for g in groups:
            if id(g) in names:
                continue
            locals_ = sorted(x for x in g if x.startswith("%"))
            if locals_:
                base = locals_[0].split(":", 1)[1]
            else:
                pins = sorted((x for x in g if "." in x and not x.startswith(("#", "%"))), key=pin_sort)
                base = self.host_pin_name(pins) if pins else None
            if base is None:
                continue
            name = base
            n = 2
            while name in used:
                name = f"{base}_{n}"
                n += 1
            used.add(name)
            names[id(g)] = name
            local_names[name] = g
        nets = {}
        for g in groups:
            name = names.get(id(g))
            members = sorted((x for x in g if "." in x and not x.startswith(("#", "%"))), key=pin_sort)
            if name is None or not members:
                continue
            name = self.rename.get(name, name)
            nets[name] = members
        for k in self.connects:
            node = k.pop("node", None)
            if node is not None:
                name = names.get(id(next(g for g in groups if node in g)))
                if name:
                    k["net"] = self.rename.get(name, name)
        for m in sorted(self.no_connect):
            for n, members in nets.items():
                if m in members:
                    raise ExpandError(f"{m} is left open by a circuit but on net {n}")
        return nets

    def host_pin_name(self, pins):
        """The name of the first host-side pin of a net (its device's pin name)."""
        for p in pins:
            ref, n = p.split(".", 1)
            if ref in self.spec["components"]:
                return self.device_of_ref(ref)["pins"][n]["name"]
        ref, n = pins[0].split(".", 1)
        return self.device_of_part(self.components[ref])["pins"][n]["name"]


def pin_sort(m):
    ref, pin = m.split(".", 1)
    return (pin_key(ref), pin_key(pin))


def category_parents(cat):
    parents = {
        "linear_regulator": "power", "switching_regulator": "power", "flash": "memory",
        "eeprom": "memory", "sram": "memory",
    }
    return [parents[cat]] if cat in parents else []


def params_of(d):
    return {k: v for k, v in d.items() if isinstance(v, dict) and k not in ("category", "part", "size")}


def requirement_label(req):
    if req.get("part"):
        return req["part"]
    parts = [f"{k} {label(v)}" for k, v in params_of(req).items()]
    cats = req.get("category")
    cats = "/".join(cats) if isinstance(cats, list) else cats
    return ", ".join([cats] + parts) if parts else cats


# ---------------------------------------------------------- formula eval

_TOKEN = re.compile(r"\s*(?:(\d+\.?\d*(?:e-?\d+)?)|([A-Za-z_]\w*)|(.))")


def evaluate(expr, env):
    """`+ - * /`, parentheses, unary minus, sqrt() over `env` (SI)."""
    toks = [m.groups() for m in _TOKEN.finditer(expr) if any(m.groups())]
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else (None, None, None)

    def take():
        pos[0] += 1
        return toks[pos[0] - 1]

    def atom():
        num, name, op = take()
        if num:
            return float(num)
        if name == "sqrt":
            take()
            v = add()
            take()
            return math.sqrt(v)
        if name:
            return env[name]
        if op == "(":
            v = add()
            take()
            return v
        if op == "-":
            return -atom()
        raise ExpandError(f"bad expression {expr!r}")

    def mul():
        v = atom()
        while peek()[2] in ("*", "/"):
            op = take()[2]
            r = atom()
            v = v * r if op == "*" else v / r
        return v

    def add():
        v = mul()
        while peek()[2] in ("+", "-"):
            op = take()[2]
            r = mul()
            v = v + r if op == "+" else v - r
        return v

    return add()


# ------------------------------------------------------------ entry point


def lock_path(board_path):
    p = Path(board_path)
    return p.with_name(p.stem + ".lock.yaml") if p.stem != "board" else p.with_name("board.lock.yaml")


def expand(spec, board_path, registry, out=None, update=False):
    """Expands `spec["use"]` in place (components, nets, no_connect) and
    writes the lock file and the basis report. Returns the report."""
    if not spec.get("use"):
        return None
    reg = registry if isinstance(registry, Registry) or hasattr(registry, "circuit") else Registry(registry)
    lp = lock_path(board_path)
    lock = yaml.safe_load(lp.read_text()) if lp.exists() else {}
    ex = Expander(spec, reg, lock, update=update)
    nets = ex.run()
    spec["components"] = ex.components
    spec["nets"] = nets
    spec["no_connect"] = sorted(ex.no_connect, key=pin_sort)
    new_lock = {"circuits": dict(sorted(ex.circuits.items())), "roles": dict(sorted(ex.new_lock.items()))}
    if new_lock != {k: lock.get(k) for k in new_lock}:
        lp.write_text("# Written by tools/kicad-board from board.yaml `use:` (reference circuits):\n"
                      "# the part chosen per role and each circuit's hash. Rebuilds reuse it;\n"
                      "# `--update-lock` chooses again.\n" + yaml.safe_dump(new_lock, sort_keys=False, allow_unicode=True))
    changed = [cid for cid, h in ex.circuits.items() if lock.get("circuits", {}).get(cid) not in (None, h)]
    for cid in changed:
        ex.warnings.append(f"circuit {cid} changed since board.lock.yaml; locked parts were re-checked")
    report = {"circuits": [{"id": cid, "hash": h, "reviewed": any(r["reviewed"] for r in ex.rows if r["circuit"] == cid)}
                           for cid, h in sorted(ex.circuits.items())],
              "rows": ex.rows, "connections": ex.connects, "formulas": ex.formulas, "hints": ex.hints,
              "warnings": ex.warnings,
              "rules_ours": {"capacitor_voltage_margin": ex.margin}}
    if out is not None:
        Path(out).mkdir(parents=True, exist_ok=True)
        (Path(out) / "basis.json").write_text(json.dumps(report, indent=1, ensure_ascii=False))
        (Path(out) / "basis.md").write_text(basis_markdown(report))
    for w in ex.warnings:
        print("circuits: warning:", w)
    return report


def basis_markdown(r):
    o = ["# Basis of the reference-circuit parts", "",
         "Generated by tools/kicad-board from board.yaml `use:`. Every value is the cited "
         "source's (Canonical Data Specification section 41) unless marked *ours*. "
         "Circuits are AI candidates unless reviewed; the source's terms and the designer's "
         "own checks apply.", ""]
    o += ["| Circuit | Review |", "|---|---|"]
    for c in r["circuits"]:
        o.append(f"| {c['id']} | {'reviewed' if c['reviewed'] else 'unreviewed (AI candidate)'} |")
    o += ["", "## Parts", "",
          "| Ref | Role | Part | Constraint (source) | Level | Basis |", "|---|---|---|---|---|---|"]
    for x in r["rows"]:
        lv = x["level"] + (f" ({x['word']})" if x.get("word") else "")
        role = x["role"] + (f" ({x['host_pin']})" if x.get("host_pin") else "")
        if x.get("ref") is None:
            o.append(f"| - | {role} | - | - | {lv} | {x.get('note', '')} |")
            continue
        s = x.get("selection", {})
        basis = [s.get("rule", "")]
        if s.get("rule") == "catalog":
            basis.append(f"{s['candidates']} candidate(s), {s['example_matches']} example fact(s) matched")
        for k, d in (s.get("derived") or {}).items():
            basis.append(f"{k} ≥ {d['min']:g} {d['unit']}: {d['why']}")
        if s.get("unchecked"):
            basis.append("not in the part's data: " + ", ".join(s["unchecked"]))
        if s.get("waiver"):
            basis.append(f"waived: {s['waiver']}")
        o.append(f"| {x['ref']} | {role} | {x['part']} | {x['constraint']} ({x['requires_source']}) | "
                 f"{lv} ({x['source']}) | {'; '.join(b for b in basis if b)} |")
    if r["formulas"]:
        o += ["", "## Formulas", ""]
        for f in r["formulas"]:
            o.append(f"- {f['circuit']} `{f['name']} = {f['expr']}` with {', '.join(f['inputs'])}: "
                     f"**{f['result']}**; source states {f['stated']}; target {f['target']} "
                     f"({f['source']}). {f['judged']}.")
    o += ["", "## Connections", "", "| Host pin | Net | Level | Source |", "|---|---|---|---|"]
    for k in r["connections"]:
        lv = k["level"] + (f" ({k['word']})" if k.get("word") else "")
        o.append(f"| {k['host']}.{k['pin']} ({', '.join(k['pins'])}) | {k['net']} | {lv} | {k['source']} |")
    if r["hints"]:
        o += ["", "## Placement and routing hints (not enforced)", ""]
        for h in r["hints"]:
            o.append(f"- {h['circuit']}: `{json.dumps(h['hint'], ensure_ascii=False)}` ({h['source']})")
    o += ["", "## OpenParts rules (ours, not the sources')", "",
          f"- capacitor rated voltage ≥ {r['rules_ours']['capacitor_voltage_margin']:g} × the highest "
          "working voltage of its nets (`select.rules.capacitor_voltage_margin`)",
          "- tie-break: reviewed, example facts matched, `select.prefer` manufacturers, closest to a "
          "min/max bound, manufacturer, MPN"]
    if r["warnings"]:
        o += ["", "## Warnings", ""] + [f"- {w}" for w in r["warnings"]]
    return "\n".join(o) + "\n"
