//! `openparts data equiv`: proves a migrated tree means what the old one
//! did. Every file of both trees is loaded into its openparts-core type
//! and the two models are compared: values, units, provenance (source,
//! page, locator), everything. The only differences allowed are the
//! conditions a migration added to a parameter (`at`), each of which the
//! old locator must print as `SYM = value unit`, and that locator minus
//! exactly those texts (and the separators around them).

use crate::{format, migrate};
use serde_yaml::Value;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Difference {
    pub file: String,
    pub what: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Report {
    pub files: usize,
    /// Conditions present in `after` and justified by `before`.
    pub conditions: usize,
    pub differences: Vec<Difference>,
}

fn load(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut v = format::canonical(format::read(&text)?)?;
    if let Value::Mapping(m) = &mut v {
        m.remove("schema_version");
    }
    Ok(v)
}

fn rel_files(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut files = Vec::new();
    crate::yaml_files(root, &mut files).map_err(|e| e.to_string())?;
    Ok(files
        .iter()
        .map(|p| {
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect())
}

/// `s` without separators, for comparing locators modulo the `, ( ) @`
/// a cut leaves or removes.
fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, ' ' | ',' | ';' | '(' | ')' | '@'))
        .collect()
}

/// Checks one value's conditions against its old locator; returns the
/// number of conditions.
fn justify(old: &str, new: &str, at: &Value) -> Result<usize, String> {
    let Value::Mapping(at) = at else {
        return Err("at is not a mapping".into());
    };
    let found = migrate::find_conditions(old);
    let mut spans = Vec::new();
    for (sym, c) in at {
        let sym = sym.as_str().unwrap_or("");
        let value = c.get("value").and_then(Value::as_f64);
        let unit = c.get("unit").and_then(Value::as_str);
        let hit = found
            .iter()
            .find(|f| f.symbol == sym && Some(f.value) == value && Some(f.unit) == unit)
            .ok_or_else(|| format!("condition {sym} is not printed in the old locator {old:?}"))?;
        spans.push((hit.start, hit.end));
    }
    spans.sort();
    let mut rest = String::new();
    let mut last = 0;
    for (a, b) in &spans {
        let mut start = *a;
        let before = old[last..start].trim_end_matches(' ');
        if let Some(x) = before.strip_suffix(" at") {
            start = last + x.len();
        }
        rest.push_str(&old[last..start]);
        last = *b;
    }
    rest.push_str(&old[last..]);
    if squash(&rest) != squash(new) {
        return Err(format!(
            "locator changed beyond the conditions: {old:?} -> {new:?}"
        ));
    }
    Ok(spans.len())
}

/// Removes from `after` what the migration may add, after checking it
/// against `before`; returns the conditions accepted.
fn reconcile(before: &Value, after: &mut Value) -> Result<usize, String> {
    let mut n = 0;
    let Some(Value::Mapping(params)) = after.get("parameters").cloned() else {
        return Ok(0);
    };
    for (k, list) in params {
        let key = k.as_str().unwrap_or("");
        let Value::Sequence(list) = list else {
            continue;
        };
        for (i, v) in list.iter().enumerate() {
            let Some(at) = v.get("at") else { continue };
            let ptr = if list.len() == 1 {
                format!("/parameters/{key}")
            } else {
                format!("/parameters/{key}/{i}")
            };
            let old_entries = before
                .get("provenance")
                .and_then(|p| p.get(ptr.as_str()))
                .and_then(Value::as_sequence)
                .ok_or_else(|| format!("{ptr}: conditions without a cited value before"))?;
            let new_entries = after
                .get("provenance")
                .and_then(|p| p.get(ptr.as_str()))
                .and_then(Value::as_sequence)
                .ok_or_else(|| format!("{ptr}: no provenance after"))?;
            if old_entries.len() != 1 || new_entries.len() != 1 {
                return Err(format!(
                    "{ptr}: conditions on a value with several citations"
                ));
            }
            let old_loc = old_entries[0]
                .get("locator")
                .and_then(Value::as_str)
                .unwrap_or("");
            let new_loc = new_entries[0]
                .get("locator")
                .and_then(Value::as_str)
                .unwrap_or("");
            n += justify(old_loc, new_loc, at).map_err(|e| format!("{ptr}: {e}"))?;
            // Accepted: undo it, so the rest must be equal.
            if let Some(Value::Sequence(l)) =
                after.get_mut("parameters").and_then(|p| p.get_mut(key))
            {
                if let Some(Value::Mapping(m)) = l.get_mut(i) {
                    m.remove("at");
                }
            }
            if let Some(Value::Sequence(e)) = after
                .get_mut("provenance")
                .and_then(|p| p.get_mut(ptr.as_str()))
            {
                if let Some(Value::Mapping(m)) = e.get_mut(0) {
                    m.insert("locator".into(), Value::String(old_loc.to_string()));
                }
            }
        }
    }
    Ok(n)
}

/// The first JSON Pointer at which `a` and `b` differ.
fn first_difference(a: &Value, b: &Value, ptr: &str) -> Option<String> {
    match (a, b) {
        (Value::Mapping(x), Value::Mapping(y)) => {
            let keys: BTreeSet<String> = x
                .keys()
                .chain(y.keys())
                .map(|k| k.as_str().map_or_else(|| format!("{k:?}"), String::from))
                .collect();
            for k in keys {
                let p = format!("{ptr}/{k}");
                match (x.get(k.as_str()), y.get(k.as_str())) {
                    (Some(u), Some(v)) => {
                        if let Some(d) = first_difference(u, v, &p) {
                            return Some(d);
                        }
                    }
                    (u, v) => return Some(format!("{p}: {u:?} vs {v:?}")),
                }
            }
            None
        }
        (Value::Sequence(x), Value::Sequence(y)) if x.len() == y.len() => x
            .iter()
            .zip(y)
            .enumerate()
            .find_map(|(i, (u, v))| first_difference(u, v, &format!("{ptr}/{i}"))),
        (Value::Number(x), Value::Number(y)) if x.as_f64() == y.as_f64() => None,
        _ if a == b => None,
        _ => Some(format!("{ptr}: {a:?} vs {b:?}")),
    }
}

/// Compares every file of `before` with the same file in `after`.
pub fn compare_trees(before: &Path, after: &Path) -> Result<Report, String> {
    let (a, b) = (rel_files(before)?, rel_files(after)?);
    let mut report = Report::default();
    let diff = |file: &str, what: String| Difference {
        file: file.to_string(),
        what,
    };
    for file in a.union(&b) {
        report.files += 1;
        let (old, new) = match (a.contains(file), b.contains(file)) {
            (true, true) => (load(&before.join(file)), load(&after.join(file))),
            (true, false) => {
                report.differences.push(diff(file, "missing after".into()));
                continue;
            }
            _ => {
                report.differences.push(diff(file, "new after".into()));
                continue;
            }
        };
        let (old, mut new) = match (old, new) {
            (Ok(o), Ok(n)) => (o, n),
            (Err(e), _) => {
                report.differences.push(diff(file, format!("before: {e}")));
                continue;
            }
            (_, Err(e)) => {
                report.differences.push(diff(file, format!("after: {e}")));
                continue;
            }
        };
        match reconcile(&old, &mut new) {
            Ok(n) => report.conditions += n,
            Err(e) => {
                report.differences.push(diff(file, e));
                continue;
            }
        }
        if let Some(d) = first_difference(&old, &new, "") {
            report.differences.push(diff(file, d));
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locator_may_lose_only_its_conditions() {
        let at: Value =
            serde_yaml::from_str("{ VGS: { value: 10, unit: V }, ID: { value: 5.7, unit: A } }")
                .unwrap();
        let old = "Electrical Characteristics, RDS(ON), VGS=10V, ID=5.7A: Typ 18";
        assert_eq!(
            justify(old, "Electrical Characteristics, RDS(ON): Typ 18", &at),
            Ok(2)
        );
        assert!(justify(old, "Electrical Characteristics: Typ 18", &at).is_err());
        let wrong: Value = serde_yaml::from_str("{ VGS: { value: 4.5, unit: V } }").unwrap();
        assert!(justify(
            old,
            "Electrical Characteristics, RDS(ON), ID=5.7A: Typ 18",
            &wrong
        )
        .is_err());
    }
}
