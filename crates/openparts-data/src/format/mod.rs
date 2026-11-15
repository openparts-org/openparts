//! The data file format (Canonical Data Specification section 3.3):
//! schema 0.2, and reading 0.1 during the transition.
//!
//! In memory (openparts-core, the API) a document keeps its evidence in
//! a `provenance` map keyed by JSON Pointer and its quantities as
//! `typ`/`min`/`max` + `unit`. The 0.2 file writes both where a reader
//! looks for them:
//!
//! ```yaml
//! source: aos/AO3400A-DS                  # the file's default source
//! parameters:
//!   rds_on: { typ: 18 mΩ, max: 26.5 mΩ, at: { VGS: 10 V }, src: p2 "RDS(ON)" }
//! pins:
//!   src: p1 "Pin table"                   # covers /pins
//!   "1": { name: { value: VBAT, src: p73 "Pin definitions" }, type: power }
//! ```
//!
//! - `src` inside a mapping cites that mapping (its JSON Pointer); a
//!   scalar or list fact is wrapped as `{ value: <fact>, src: ... }`.
//! - Quantities are unit strings (`26.5 mΩ`, `±10 %` for a symmetric
//!   tolerance); conditions are `at: { <symbol>: <quantity> }`.
//!
//! [`decode`] turns a file into the in-memory shape, [`encode`] the
//! in-memory shape into a file; [`format`] re-writes a file canonically.

pub mod cite;
pub mod comments;
pub mod emit;

use cite::{num_value, parse_quantity, parse_src, quantity, Cite};
pub use comments::Comments;
use openparts_core::condition::symbol_order;
use openparts_core::parameter_spec;
use serde_yaml::{Mapping, Value};
use std::collections::BTreeMap;

/// The schema version this module writes.
pub const SCHEMA_VERSION: &str = "0.2";

/// Keys whose empty value is kept (the type requires them).
const KEEP_EMPTY: &[&str] = &["pins", "restricts"];

fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

fn key_str(k: &Value) -> String {
    match k {
        Value::String(s) => s.clone(),
        other => emit::scalar(other, true),
    }
}

/// `/parameters/<key>` or `/parameters/<key>/<i>`: a parameter value,
/// whose own `value` field is not a wrapper.
fn parameter_entry(ptr: &str) -> bool {
    let parts: Vec<&str> = ptr.split('/').collect();
    parts.get(1) == Some(&"parameters")
        && (parts.len() == 3 || (parts.len() == 4 && parts[3].parse::<usize>().is_ok()))
}

// ---------------------------------------------------------------- decode

/// A file's YAML in the in-memory shape: a 0.2 document is converted,
/// any other version is returned as it is (0.1's `nominal` reads as
/// `typ` through the types' serde aliases).
pub fn decode(doc: Value) -> Result<Value, String> {
    if doc.get("schema_version").and_then(Value::as_str) != Some(SCHEMA_VERSION) {
        return Ok(doc);
    }
    let Value::Mapping(mut map) = doc else {
        return Err("a document must be a mapping".into());
    };
    let default = match map.remove("source") {
        None => None,
        Some(Value::String(s)) => Some(s),
        Some(_) => return Err("`source:` must be a source id".into()),
    };
    let residual = map.remove("provenance");
    let mut prov: BTreeMap<String, Vec<Cite>> = BTreeMap::new();
    let mut out = Mapping::new();
    for (k, mut v) in map {
        let ptr = format!("/{}", escape(&key_str(&k)));
        walk(&mut v, &ptr, default.as_deref(), &mut prov)?;
        out.insert(k, v);
    }
    if let Some(Value::Mapping(r)) = residual {
        for (k, v) in r {
            let k = key_str(&k);
            let list = parse_src(&v, default.as_deref())?;
            if prov.insert(k.clone(), list).is_some() {
                return Err(format!("provenance for {k} is given twice"));
            }
        }
    }
    if !prov.is_empty() {
        let mut m = Mapping::new();
        for (k, list) in prov {
            m.insert(
                s(&k),
                Value::Sequence(list.iter().map(Cite::to_value).collect()),
            );
        }
        out.insert(s("provenance"), Value::Mapping(m));
    }
    Ok(Value::Mapping(out))
}

fn walk(
    node: &mut Value,
    ptr: &str,
    default: Option<&str>,
    prov: &mut BTreeMap<String, Vec<Cite>>,
) -> Result<(), String> {
    match node {
        Value::Mapping(m) => {
            if let Some(src) = m.remove("src") {
                let list = parse_src(&src, default).map_err(|e| format!("{ptr}: {e}"))?;
                if prov.insert(ptr.to_string(), list).is_some() {
                    return Err(format!("{ptr}: cited twice"));
                }
                let wrapper = m.len() == 1
                    && m.contains_key("value")
                    && (!parameter_entry(ptr) || m.get("value").is_some_and(Value::is_sequence));
                if wrapper {
                    let inner = m.remove("value").unwrap_or(Value::Null);
                    *node = inner;
                    return walk_children(node, ptr, default, prov);
                }
            }
            if ["typ", "min", "max", "tol"]
                .iter()
                .any(|k| m.contains_key(*k))
            {
                read_quantity(m).map_err(|e| format!("{ptr}: {e}"))?;
            }
            if let Some(at) = m.get_mut("at") {
                read_conditions(at).map_err(|e| format!("{ptr}/at: {e}"))?;
            }
            walk_children(node, ptr, default, prov)
        }
        _ => walk_children(node, ptr, default, prov),
    }
}

fn walk_children(
    node: &mut Value,
    ptr: &str,
    default: Option<&str>,
    prov: &mut BTreeMap<String, Vec<Cite>>,
) -> Result<(), String> {
    match node {
        Value::Mapping(m) => {
            for (k, v) in m.iter_mut() {
                let k = key_str(k);
                if k == "at" {
                    continue;
                }
                walk(v, &format!("{ptr}/{}", escape(&k)), default, prov)?;
            }
        }
        Value::Sequence(items) => {
            for (i, v) in items.iter_mut().enumerate() {
                walk(v, &format!("{ptr}/{i}"), default, prov)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// `{ min: 0.65 V, typ: 1.05 V }` -> `{ min: 0.65, typ: 1.05, unit: V }`.
fn read_quantity(m: &mut Mapping) -> Result<(), String> {
    if m.contains_key("unit") {
        return Err("write the unit in the value (\"26.5 mΩ\"), not as `unit:`".into());
    }
    let mut unit: Option<String> = None;
    let mut same = |u: String| -> Result<(), String> {
        match &unit {
            Some(have) if *have != u => Err(format!(
                "min, typ and max must use one unit (\"{have}\" and \"{u}\")"
            )),
            _ => {
                unit = Some(u);
                Ok(())
            }
        }
    };
    let tol = m.remove("tol");
    if tol.is_some() && (m.contains_key("min") || m.contains_key("max")) {
        return Err("`tol` is min and max; don't give them too".into());
    }
    if let Some(tol) = tol {
        let t = tol.as_str().ok_or("tol must be \"±<number> <unit>\"")?;
        let t = t.trim();
        let body = t
            .strip_prefix('±')
            .or_else(|| t.strip_prefix("+/-"))
            .ok_or_else(|| format!("tol {t:?} must start with ±"))?;
        let (v, u) = parse_quantity(body)?;
        m.insert(s("min"), s(&quantity(-v, &u)));
        m.insert(s("max"), s(&quantity(v, &u)));
    }
    for k in ["min", "typ", "max"] {
        let Some(v) = m.get_mut(k) else { continue };
        let text = match v {
            Value::String(t) => t.clone(),
            Value::Number(_) => {
                return Err(format!(
                    "{k} needs a unit (\"{} V\")",
                    emit::scalar(v, false)
                ))
            }
            _ => return Err(format!("{k} must be a quantity like \"26.5 mΩ\"")),
        };
        let (n, u) = parse_quantity(&text)?;
        same(u)?;
        *v = num_value(n);
    }
    if let Some(u) = unit {
        m.insert(s("unit"), s(&u));
    }
    Ok(())
}

/// `at: { VGS: 10 V, TA: -40..85 °C }` -> `{ VGS: { value: 10, unit: V }, ... }`.
fn read_conditions(at: &mut Value) -> Result<(), String> {
    let Value::Mapping(m) = at else {
        return Err("`at` must map symbols to quantities".into());
    };
    for (k, v) in m.iter_mut() {
        let text = v
            .as_str()
            .ok_or_else(|| format!("{}: a quantity like \"10 V\"", key_str(k)))?
            .to_string();
        let mut c = Mapping::new();
        if let Some((a, b)) = text.split_once("..") {
            let (hi, u) = parse_quantity(b)?;
            let lo: f64 = a
                .trim()
                .replace('−', "-")
                .parse()
                .map_err(|_| format!("{text:?}: a range is \"<min>..<max> <unit>\""))?;
            c.insert(s("min"), num_value(lo));
            c.insert(s("max"), num_value(hi));
            c.insert(s("unit"), s(&u));
        } else {
            let (n, u) = parse_quantity(&text)?;
            c.insert(s("value"), num_value(n));
            c.insert(s("unit"), s(&u));
        }
        *v = Value::Mapping(c);
    }
    Ok(())
}

// ---------------------------------------------------------------- encode

/// The 0.2 file form of an in-memory document (as `serde_yaml` writes the
/// openparts-core type).
pub fn encode(doc: &Value) -> Result<Mapping, String> {
    let Value::Mapping(map) = doc else {
        return Err("a document must be a mapping".into());
    };
    let mut map = map.clone();
    let kind = map
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut prov: Vec<(String, Vec<Cite>)> = Vec::new();
    if let Some(Value::Mapping(p)) = map.remove("provenance") {
        for (k, v) in p {
            let list = v
                .as_sequence()
                .ok_or("a provenance value must be a list")?
                .iter()
                .map(Cite::from_value)
                .collect::<Result<Vec<_>, _>>()?;
            prov.push((key_str(&k), list));
        }
    }
    let mut root = Value::Mapping(map);
    clean(&mut root);
    let Value::Mapping(mut map) = root else {
        unreachable!()
    };
    order_pins(&mut map);
    let pointers: Vec<&str> = prov.iter().map(|(p, _)| p.as_str()).collect();
    order_parameters(&mut map, &pointers);
    let mut root = Value::Mapping(map);
    write_quantities(&mut root, "");

    // The file's default source: the most cited one.
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, list) in &prov {
        for c in list {
            *counts.entry(c.source.as_str()).or_default() += 1;
        }
    }
    let default = counts
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(k, _)| k.to_string());

    // Deepest pointers first, so a wrapped list keeps its items' `src`.
    prov.sort_by(|a, b| {
        let depth = |p: &str| p.matches('/').count();
        depth(&b.0).cmp(&depth(&a.0)).then(a.0.cmp(&b.0))
    });
    let mut residual = Mapping::new();
    for (ptr, list) in &prov {
        let src = src_value(list, default.as_deref());
        if !place(&mut root, ptr, src) {
            residual.insert(
                s(ptr),
                Value::Sequence(list.iter().map(Cite::to_value).collect()),
            );
        }
    }
    let Value::Mapping(map) = root else {
        unreachable!()
    };
    let mut out = top_level_order(map, &kind, default);
    out.insert(s("schema_version"), s(SCHEMA_VERSION));
    if !residual.is_empty() {
        out.insert(s("provenance"), Value::Mapping(residual));
    }
    Ok(out)
}

fn src_value(list: &[Cite], default: Option<&str>) -> Value {
    let one = |c: &Cite| match c.shorthand(default) {
        Some(t) => s(&t),
        None => c.to_value(),
    };
    if list.len() == 1 {
        one(&list[0])
    } else {
        Value::Sequence(list.iter().map(one).collect())
    }
}

/// Drops nulls and empty lists/maps (the types' defaults).
fn clean(v: &mut Value) {
    match v {
        Value::Mapping(m) => {
            let keys: Vec<Value> = m.keys().cloned().collect();
            for k in keys {
                let name = key_str(&k);
                let child = m.get_mut(&k).unwrap();
                clean(child);
                let empty = match child {
                    Value::Null => true,
                    Value::Sequence(x) => x.is_empty() && !KEEP_EMPTY.contains(&name.as_str()),
                    Value::Mapping(x) => x.is_empty() && !KEEP_EMPTY.contains(&name.as_str()),
                    _ => false,
                };
                if empty {
                    m.remove(&k);
                }
            }
        }
        Value::Sequence(items) => {
            for item in items {
                clean(item);
            }
        }
        _ => {}
    }
}

/// Pin numbers in natural order (2 before 10, A2 before A10).
pub fn pin_order(a: &str, b: &str) -> std::cmp::Ordering {
    fn split(s: &str) -> (&str, Option<u64>) {
        let i = s.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        (&s[..i], s[i..].parse().ok())
    }
    let (pa, na) = split(a);
    let (pb, nb) = split(b);
    pa.cmp(pb).then(na.cmp(&nb)).then(a.cmp(b))
}

fn sort_mapping(m: &mut Mapping, cmp: impl Fn(&str, &str) -> std::cmp::Ordering) {
    let mut entries: Vec<(Value, Value)> = std::mem::take(m).into_iter().collect();
    entries.sort_by(|a, b| cmp(&key_str(&a.0), &key_str(&b.0)));
    m.extend(entries);
}

fn order_pins(map: &mut Mapping) {
    if let Some(Value::Mapping(pins)) = map.get_mut("pins") {
        sort_mapping(pins, pin_order);
    }
    if let Some(Value::Mapping(revs)) = map.get_mut("revisions") {
        for (_, rev) in revs.iter_mut() {
            if let Some(Value::Mapping(pins)) =
                rev.get_mut("overrides").and_then(|o| o.get_mut("pins"))
            {
                sort_mapping(pins, pin_order);
            }
        }
    }
}

/// Parameters in vocabulary order; a key with one value is written as
/// that value (unless something cites `/parameters/<key>/0`).
fn order_parameters(map: &mut Mapping, pointers: &[&str]) {
    let Some(Value::Mapping(params)) = map.get_mut("parameters") else {
        return;
    };
    let rank = |k: &str| {
        openparts_core::PARAMETERS
            .iter()
            .position(|s| s.key == k)
            .unwrap_or(usize::MAX)
    };
    sort_mapping(params, |a, b| rank(a).cmp(&rank(b)).then(a.cmp(b)));
    for (k, v) in params.iter_mut() {
        let base = format!("/parameters/{}/", escape(&key_str(k)));
        let indexed = pointers.iter().any(|p| p.starts_with(&base));
        if let Value::Sequence(items) = v {
            if items.len() == 1 && !indexed {
                *v = items.remove(0);
            }
        }
    }
}

/// In-memory quantities and conditions as unit strings, in reading order
/// (`min`, `typ`, `max`, `tol`, `value`, `at`).
fn write_quantities(v: &mut Value, ptr: &str) {
    match v {
        Value::Mapping(m) => {
            let is_quantity = m.get("unit").is_some_and(Value::is_string)
                && ["typ", "min", "max"].iter().any(|k| m.contains_key(*k))
                && m.keys().all(|k| {
                    matches!(
                        k.as_str(),
                        Some("min" | "typ" | "max" | "unit" | "value" | "at")
                    )
                });
            let has_at = m.get("at").is_some_and(Value::is_mapping) && parameter_entry_like(ptr);
            if is_quantity || has_at {
                *m = quantity_fields(m, ptr);
                return;
            }
            for (k, child) in m.iter_mut() {
                let p = format!("{ptr}/{}", escape(&key_str(k)));
                write_quantities(child, &p);
            }
        }
        Value::Sequence(items) => {
            for (i, item) in items.iter_mut().enumerate() {
                write_quantities(item, &format!("{ptr}/{i}"));
            }
        }
        _ => {}
    }
}

fn parameter_entry_like(ptr: &str) -> bool {
    parameter_entry(ptr)
}

fn quantity_fields(m: &Mapping, ptr: &str) -> Mapping {
    let unit = m
        .get("unit")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let f = |k: &str| m.get(k).and_then(Value::as_f64);
    let key = ptr.split('/').nth(2).map(unescape).unwrap_or_default();
    let deviation =
        ptr.starts_with("/parameters/") && parameter_spec(&key).is_some_and(|s| s.deviation);
    let mut out = Mapping::new();
    match (f("min"), f("typ"), f("max")) {
        (Some(lo), None, Some(hi)) if deviation && lo == -hi && hi > 0.0 => {
            out.insert(s("tol"), s(&format!("±{}", quantity(hi, &unit))));
        }
        (lo, typ, hi) => {
            for (k, v) in [("min", lo), ("typ", typ), ("max", hi)] {
                if let Some(v) = v {
                    out.insert(s(k), s(&quantity(v, &unit)));
                }
            }
        }
    }
    if let Some(v) = m.get("value") {
        out.insert(s("value"), v.clone());
    }
    if let Some(Value::Mapping(at)) = m.get("at") {
        let mut conds: Vec<(String, String)> = at
            .iter()
            .map(|(k, c)| (key_str(k), condition_text(c)))
            .collect();
        conds.sort_by(|a, b| {
            symbol_order(&a.0)
                .cmp(&symbol_order(&b.0))
                .then(a.0.cmp(&b.0))
        });
        let mut a = Mapping::new();
        for (k, t) in conds {
            a.insert(s(&k), s(&t));
        }
        out.insert(s("at"), Value::Mapping(a));
    }
    out
}

fn condition_text(c: &Value) -> String {
    let unit = c.get("unit").and_then(Value::as_str).unwrap_or("");
    let f = |k: &str| c.get(k).and_then(Value::as_f64);
    match (f("value"), f("min"), f("max")) {
        (Some(v), _, _) => quantity(v, unit),
        (None, Some(a), Some(b)) => format!("{}..{}", cite::number(a), quantity(b, unit)),
        _ => String::new(),
    }
}

/// Puts `src` at `ptr`: inside a mapping, or wrapping a scalar or list.
fn place(root: &mut Value, ptr: &str, src: Value) -> bool {
    if ptr.is_empty() || ptr == "/" {
        return false;
    }
    let mut node = root;
    for token in ptr.split('/').skip(1) {
        let token = unescape(token);
        node = match node {
            Value::Mapping(m) => match m.get_mut(token.as_str()) {
                Some(v) => v,
                None => return false,
            },
            Value::Sequence(items) => {
                match token.parse::<usize>().ok().and_then(|i| items.get_mut(i)) {
                    Some(v) => v,
                    None => return false,
                }
            }
            _ => return false,
        };
    }
    match node {
        Value::Mapping(m) => {
            if m.contains_key("src") {
                return false;
            }
            // A one-key `{ value: ... }` would read back as a wrapper.
            if m.len() == 1 && m.contains_key("value") && !parameter_entry(ptr) {
                return false;
            }
            // A table (pins, dimensions) is cited at its top; a record
            // after its fields.
            let table = m.len() > 1 && m.values().all(|v| v.is_mapping() || v.is_sequence());
            if table {
                let rest = std::mem::take(m);
                m.insert(s("src"), src);
                m.extend(rest);
            } else {
                m.insert(s("src"), src);
            }
        }
        other => {
            let mut w = Mapping::new();
            w.insert(s("value"), std::mem::take(other));
            w.insert(s("src"), src);
            *other = Value::Mapping(w);
        }
    }
    true
}

/// The preferred top-level key order of a kind (other keys follow in
/// their own order); `source:` goes after the identity keys.
fn top_level_order(map: Mapping, kind: &str, default: Option<String>) -> Mapping {
    let preferred: &[&str] = match kind {
        "part" => &[
            "schema_version",
            "kind",
            "id",
            "manufacturer",
            "mpn",
            "device",
            "package",
            "sources",
            "existence",
            "lifecycle",
            "parameters",
        ],
        "device" => &[
            "schema_version",
            "kind",
            "id",
            "manufacturer",
            "category",
            "family",
            "parameters",
            "pins",
            "revisions",
        ],
        "package" => &[
            "schema_version",
            "kind",
            "id",
            "family",
            "lead_count",
            "lead_layout",
            "pitch",
            "construction",
            "connector",
            "tab",
            "aux_pads",
            "pad_order",
            "geometry",
            "dimensions",
            "pads",
            "smt",
            "pad_rows",
            "pad_grids",
            "shield",
            "edge",
            "keepouts",
        ],
        "circuit" => &[
            "schema_version",
            "kind",
            "id",
            "title",
            "host",
            "conditions",
            "nets",
            "connect",
            "roles",
            "formulas",
            "placement",
            "routing",
            "includes",
        ],
        _ => &["schema_version", "kind", "id"],
    };
    let identity: &[&str] = match kind {
        "package" => &["id", "family", "lead_count", "lead_layout"],
        "circuit" => &["id", "title", "host"],
        _ => &[
            "id",
            "manufacturer",
            "mpn",
            "device",
            "package",
            "sources",
            "category",
            "family",
        ],
    };
    let mut rest = map;
    let mut ordered: Vec<(Value, Value)> = Vec::new();
    for k in preferred {
        if let Some(v) = rest.remove(*k) {
            ordered.push((s(k), v));
        }
    }
    let review = rest.remove("review");
    ordered.extend(rest);
    if let Some(r) = review {
        ordered.push((s("review"), r));
    }
    let mut out = Mapping::new();
    let last_identity = ordered
        .iter()
        .rposition(|(k, _)| identity.contains(&k.as_str().unwrap_or("")));
    for (i, (k, v)) in ordered.into_iter().enumerate() {
        out.insert(k, v);
        if Some(i) == last_identity {
            if let Some(d) = &default {
                out.insert(s("source"), s(d));
            }
        }
    }
    if last_identity.is_none() {
        if let Some(d) = default {
            out.insert(s("source"), s(&d));
        }
    }
    out
}

// ---------------------------------------------------------------- text

/// Parses a data file (any schema version) into the in-memory shape.
pub fn read(text: &str) -> Result<Value, String> {
    let v: Value = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    decode(v)
}

/// Writes an in-memory document as a 0.2 file, with `comments`.
pub fn write(doc: &Value, comments: &Comments) -> Result<String, String> {
    let map = encode(doc)?;
    Ok(emit::document(&map, comments))
}

/// The document through its openparts-core type (by `kind`): checks it
/// and puts it in the type's canonical form (field order, defaults).
pub fn canonical(doc: Value) -> Result<Value, String> {
    use openparts_core as c;
    fn via<T: serde::de::DeserializeOwned + serde::Serialize>(v: Value) -> Result<Value, String> {
        let t: T = serde_yaml::from_value(v).map_err(|e| e.to_string())?;
        serde_yaml::to_value(&t).map_err(|e| e.to_string())
    }
    match doc.get("kind").and_then(Value::as_str) {
        Some("part") => via::<c::Part>(doc),
        Some("device") => via::<c::Device>(doc),
        Some("package") => via::<c::Package>(doc),
        Some("source") => via::<c::Source>(doc),
        Some("manufacturer") => via::<c::Manufacturer>(doc),
        Some("erratum") => via::<c::Erratum>(doc),
        Some("rejected") => via::<c::Rejected>(doc),
        Some("circuit") => via::<c::Circuit>(doc),
        Some(k) => Err(format!("unknown kind \"{k}\"")),
        None => Err("no `kind`".into()),
    }
}

/// `text` re-written in the canonical 0.2 form (what `openparts data
/// fmt` writes). A 0.1 file is refused: convert it with `openparts data
/// migrate --to 0.2`.
pub fn format(text: &str) -> Result<String, String> {
    let raw: Value = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    if raw.get("schema_version").and_then(Value::as_str) != Some(SCHEMA_VERSION) {
        return Err(format!(
            "schema_version is not \"{SCHEMA_VERSION}\": run `openparts data migrate --to {SCHEMA_VERSION}`"
        ));
    }
    let doc = canonical(decode(raw)?)?;
    write(&doc, &comments::extract(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PART_01: &str = r#"# A comment.
schema_version: "0.1"
kind: part
id: aos/AO3400A
manufacturer: aos
mpn: AO3400A
device: aos/AO3400A
package: standards/SOT23
existence:
  status: probable
lifecycle:
  status: unknown
sources:
  - aos/AO3400A-DS
  - aos/PO-00001
parameters:
  polarity:
    value: n_channel
  rds_on:
    nominal: 18
    max: 26.5
    unit: mΩ
  tolerance:
    min: -10
    max: 10
    unit: "%"
provenance:
  "/existence":
    - source: aos/AO3400A-DS
      page: 1
      locator: "Title"
  "/package":
    - source: aos/PO-00001
      page: 1
  "/parameters/polarity":
    - source: aos/AO3400A-DS
      page: 1
      locator: "Title: \"30V N-Channel MOSFET\""
  "/parameters/rds_on":
    - source: aos/AO3400A-DS
      page: 2
      locator: "RDS(ON)"
  "/parameters/tolerance":
    - source: aos/AO3400A-DS
      page: 2
    - source: aos/PO-00001
      locator: "x"
"#;

    fn typed(text: &str) -> openparts_core::Part {
        serde_yaml::from_value(read(text).unwrap()).unwrap()
    }

    #[test]
    fn a_01_document_writes_as_02_and_reads_back_the_same() {
        let doc = canonical(read(PART_01).unwrap()).unwrap();
        let text = write(&doc, &comments::extract(PART_01)).unwrap();
        assert_eq!(
            text,
            r#"# A comment.
schema_version: "0.2"
kind: part
id: aos/AO3400A
manufacturer: aos
mpn: AO3400A
device: aos/AO3400A
package: { value: standards/SOT23, src: aos/PO-00001 p1 }
sources: [aos/AO3400A-DS, aos/PO-00001]
source: aos/AO3400A-DS

existence: { status: probable, src: p1 "Title" }
lifecycle: { status: unknown }

parameters:
  tolerance: { tol: ±10 %, src: [p2, aos/PO-00001 "x"] }
  polarity:  { value: n_channel, src: 'p1 "Title: "30V N-Channel MOSFET""' }
  rds_on:    { typ: 18 mΩ, max: 26.5 mΩ, src: p2 "RDS(ON)" }
"#
        );
        let mut old = typed(PART_01);
        old.schema_version = SCHEMA_VERSION.into();
        assert_eq!(typed(&text), old);
        // fmt is idempotent.
        assert_eq!(format(&text).unwrap(), text);
    }

    #[test]
    fn rejects_mixed_units_and_bare_numbers() {
        let doc = |p: &str| {
            format!("schema_version: \"0.2\"\nkind: part\nid: a/B\nsource: a/DS\nparameters:\n  rds_on: {p}\n")
        };
        assert!(read(&doc("{ typ: 18, src: p1 }"))
            .unwrap_err()
            .contains("unit"));
        assert!(read(&doc("{ typ: 18 mΩ, max: 0.03 Ω, src: p1 }"))
            .unwrap_err()
            .contains("one unit"));
        let v = read(&doc("[{ typ: 18 mΩ, at: { VGS: 10 V }, src: p1 }, { typ: 30 mΩ, at: { VGS: 4.5 V, TA: -40..85 °C }, src: p2 }]")).unwrap();
        assert_eq!(
            v["parameters"]["rds_on"][1]["at"]["TA"]["min"].as_f64(),
            Some(-40.0)
        );
        assert_eq!(
            v["provenance"]["/parameters/rds_on/1"][0]["page"].as_u64(),
            Some(2)
        );
    }

    #[test]
    fn pins_keep_natural_order_and_pin_level_src() {
        let text = "schema_version: \"0.2\"\nkind: device\nid: a/B\nmanufacturer: a\ncategory: other\nsource: a/DS\npins:\n  src: p3 \"Pin table\"\n  \"10\": { name: X, type: io }\n  \"2\": { name: { value: Y, src: p4 }, type: io }\n";
        let doc = canonical(read(text).unwrap()).unwrap();
        let out = write(&doc, &Comments::default()).unwrap();
        assert!(out.contains("pins:\n  src:  p3 \"Pin table\"\n  \"2\":  { name: { value: Y, src: p4 }, type: io }\n  \"10\": { name: X, type: io }\n"), "{out}");
        assert_eq!(format(&out).unwrap(), out);
    }

    #[test]
    fn a_circuit_reads_its_quantities_levels_and_src_and_writes_back() {
        let text = r#"schema_version: "0.2"
kind: circuit
id: t/x
title: X
host: t/MCU
source: t/DS
nets:
  VDD: { kind: power, voltage: { typ: 3.3 V }, src: p2 "2.1" }
roles:
  C1:
    per_pin: VDD
    requires: { category: capacitor, capacitance: { typ: 100 nF }, src: p3 "2.2" }
    pins: { "1": $pin, "2": GND }
    level: { value: required, word: must, src: p3 "2.2" }
  R1:
    requires: { category: resistor, resistance: { typ: 1 kΩ }, src: p4 }
    level: { value: example, src: p4 }
"#;
        let v = read(text).unwrap();
        assert_eq!(
            v["roles"]["C1"]["requires"]["capacitance"]["unit"].as_str(),
            Some("nF")
        );
        assert_eq!(v["roles"]["R1"]["level"].as_str(), Some("example"));
        assert_eq!(
            v["provenance"]["/roles/C1/level"][0]["page"].as_u64(),
            Some(3)
        );
        let c: openparts_core::Circuit = serde_yaml::from_value(v.clone()).unwrap();
        assert_eq!(c.roles["C1"].level.word.as_deref(), Some("must"));
        let out = format(text).unwrap();
        assert_eq!(format(&out).unwrap(), out);
        assert_eq!(
            canonical(read(&out).unwrap()).unwrap(),
            canonical(v).unwrap()
        );
    }
}
