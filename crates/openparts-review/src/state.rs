//! Every reviewable fact of a dataset, and each fact's status from the
//! committed reviews plus pending submissions.

use crate::Submission;
use openparts_core::{FactReview, ReviewStatus, ReviewTargetKind};
use openparts_data::Dataset;
use openparts_validator::{facts_of, Fact};
use serde_json::Value;
use std::collections::BTreeMap;

/// The facts (provenance keys) of every Part, Device and Package.
#[derive(Debug, Default)]
pub struct FactIndex {
    by_target: BTreeMap<(ReviewTargetKind, String), Vec<Fact>>,
}

impl FactIndex {
    pub fn build(ds: &Dataset) -> FactIndex {
        let mut by_target = BTreeMap::new();
        for p in &ds.parts {
            by_target.insert(
                (ReviewTargetKind::Part, p.id.0.clone()),
                facts_of(p, &p.provenance),
            );
        }
        for d in ds.devices.values() {
            by_target.insert(
                (ReviewTargetKind::Device, d.id.0.clone()),
                facts_of(d, &d.provenance),
            );
        }
        for p in ds.packages.values() {
            by_target.insert(
                (ReviewTargetKind::Package, p.id.0.clone()),
                facts_of(p, &p.provenance),
            );
        }
        FactIndex { by_target }
    }

    /// The facts of one document, `None` when there is no such document.
    pub fn facts(&self, kind: ReviewTargetKind, target: &str) -> Option<&[Fact]> {
        self.by_target
            .get(&(kind, target.to_string()))
            .map(Vec::as_slice)
    }

    pub fn fact(&self, kind: ReviewTargetKind, target: &str, pointer: &str) -> Option<&Fact> {
        self.facts(kind, target)?
            .iter()
            .find(|f| f.pointer == pointer)
    }

    /// How many facts there are in all.
    pub fn len(&self) -> usize {
        self.by_target.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A fact's review status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FactStatus {
    /// No current review.
    Unreviewed,
    /// Every current review confirms it.
    Confirmed,
    /// At least one current review disputes it.
    Disputed,
}

impl FactStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FactStatus::Unreviewed => "unreviewed",
            FactStatus::Confirmed => "confirmed",
            FactStatus::Disputed => "disputed",
        }
    }
}

/// One review that counts for a fact now.
#[derive(Debug, Clone, PartialEq)]
pub struct StateRecord {
    pub record: FactReview,
    /// A submission not yet committed to openparts-data.
    pub pending: bool,
}

/// A fact's status and the reviews behind it.
#[derive(Debug, Clone, PartialEq)]
pub struct FactState {
    pub status: FactStatus,
    /// Every review behind `status` is still only a pending submission.
    pub pending: bool,
    /// Reviews of an earlier value of the fact (they don't count).
    pub stale: usize,
    /// The current reviews: each reviewer's latest, committed first.
    pub current: Vec<StateRecord>,
}

/// Same verdict on the same value (the date aside).
fn same_verdict(a: &FactReview, b: &FactReview) -> bool {
    a.pointer == b.pointer
        && a.reviewer == b.reviewer
        && a.status == b.status
        && a.value_hash == b.value_hash
        && a.comment == b.comment
        && a.corrected_value == b.corrected_value
}

/// The status of `fact` from its committed reviews (the records of its
/// review document for its pointer) and pending submissions (in the
/// order received). A submission identical to a committed record is
/// that record, exported; a reviewer's later review replaces their
/// earlier one; reviews of another value of the fact are stale.
pub fn fact_state(fact: &Fact, committed: &[FactReview], pending: &[&Submission]) -> FactState {
    let committed: Vec<&FactReview> = committed
        .iter()
        .filter(|r| r.pointer == fact.pointer)
        .collect();
    let mut all: Vec<StateRecord> = committed
        .iter()
        .map(|r| StateRecord {
            record: (*r).clone(),
            pending: false,
        })
        .collect();
    for s in pending {
        let r = &s.record;
        if r.pointer != fact.pointer || committed.iter().any(|c| same_verdict(c, r)) {
            continue;
        }
        all.push(StateRecord {
            record: r.clone(),
            pending: true,
        });
    }
    let mut stale = 0;
    let mut current: Vec<StateRecord> = Vec::new();
    for rec in all {
        if rec.record.value_hash != fact.hash {
            stale += 1;
            continue;
        }
        current.retain(|c| c.record.reviewer != rec.record.reviewer);
        current.push(rec);
    }
    let status = if current
        .iter()
        .any(|c| c.record.status == ReviewStatus::Disputed)
    {
        FactStatus::Disputed
    } else if current.is_empty() {
        FactStatus::Unreviewed
    } else {
        FactStatus::Confirmed
    };
    let pending = !current.is_empty() && current.iter().all(|c| c.pending);
    current.sort_by_key(|c| c.pending);
    FactState {
        status,
        pending,
        stale,
        current,
    }
}

/// A fact's value as short text: `3.9 mm`, `0.9–1.1 V`, `X7R`,
/// `verified`, `64 pins`.
pub fn display_value(value: Option<&Value>) -> String {
    let Some(v) = value else {
        return String::new();
    };
    let out = match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number(n.as_f64().unwrap_or(0.0)),
        Value::Array(items) => items
            .iter()
            .map(|i| display_value(Some(i)))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(map) => {
            let num = |k: &str| map.get(k).and_then(Value::as_f64);
            let unit = map.get("unit").and_then(Value::as_str).unwrap_or("");
            let with_unit = |s: String| {
                if unit.is_empty() {
                    s
                } else if unit == "%" {
                    format!("{s}%")
                } else {
                    format!("{s} {unit}")
                }
            };
            if let Some(s) = map.get("value").and_then(Value::as_str) {
                s.to_string()
            } else if let Some(status) = map.get("status").and_then(Value::as_str) {
                let mut s = status.to_string();
                if let Some(Value::Array(r)) = map.get("replacement") {
                    if !r.is_empty() {
                        s.push_str(&format!(" → {}", display_value(map.get("replacement"))));
                    }
                }
                s
            } else if map.contains_key("nominal")
                || map.contains_key("min")
                || map.contains_key("max")
            {
                match (num("nominal"), num("min"), num("max")) {
                    (Some(n), None, None) => with_unit(number(n)),
                    (Some(n), Some(lo), Some(hi)) => {
                        format!("{} ({}–{})", with_unit(number(n)), number(lo), number(hi))
                    }
                    (Some(n), None, Some(hi)) => {
                        format!("{} (≤{})", with_unit(number(n)), number(hi))
                    }
                    (Some(n), Some(lo), None) => {
                        format!("{} (≥{})", with_unit(number(n)), number(lo))
                    }
                    (None, Some(lo), Some(hi)) if lo == -hi && hi > 0.0 => {
                        with_unit(format!("±{}", number(hi)))
                    }
                    (None, Some(lo), Some(hi)) => {
                        with_unit(format!("{}–{}", number(lo), number(hi)))
                    }
                    (None, Some(lo), None) => format!("≥{}", with_unit(number(lo))),
                    (None, None, Some(hi)) => format!("≤{}", with_unit(number(hi))),
                    _ => String::new(),
                }
            } else if let (Some(name), Some(t)) = (
                map.get("name").and_then(Value::as_str),
                map.get("type").and_then(Value::as_str),
            ) {
                format!("{name} ({t})")
            } else if !map.is_empty() && map.values().all(|v| v.get("name").is_some()) {
                // A pin map.
                let names: Vec<String> = map
                    .iter()
                    .map(|(n, p)| format!("{n} {}", p["name"].as_str().unwrap_or("")))
                    .collect();
                format!("{} pins: {}", names.len(), names.join(", "))
            } else if !map.is_empty() && map.values().all(Value::is_object) {
                // A map of records (silicon revisions): their keys.
                map.keys().cloned().collect::<Vec<_>>().join(", ")
            } else {
                v.to_string()
            }
        }
    };
    truncate(&out, 240)
}

fn number(v: f64) -> String {
    let r = (v * 1e6).round() / 1e6;
    if r == r.trunc() && r.abs() < 1e15 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max - 1).collect();
    t.push('…');
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::ProvenanceEntry;
    use serde_json::json;

    fn fact(value: Value) -> Fact {
        let evidence = vec![ProvenanceEntry {
            source: "ex/DS1".into(),
            page: Some(3),
            locator: Some("Table 1".into()),
        }];
        Fact {
            pointer: "/x".into(),
            hash: openparts_validator::fact_hash(Some(&value), &evidence),
            value: Some(value),
            evidence,
        }
    }

    fn rec(reviewer: &str, status: ReviewStatus, hash: &str) -> FactReview {
        FactReview {
            pointer: "/x".into(),
            status,
            reviewer: reviewer.into(),
            date: "2026-11-15".into(),
            sources: vec!["ex/DS1".into()],
            value_hash: hash.into(),
            comment: (status == ReviewStatus::Disputed).then(|| "no".into()),
            corrected_value: None,
        }
    }

    fn sub(r: FactReview) -> Submission {
        Submission::new(ReviewTargetKind::Part, "ex/EX1", r)
    }

    #[test]
    fn merges_committed_and_pending_reviews() {
        let f = fact(json!({"nominal": 1.0, "unit": "mm"}));
        let h = f.hash.clone();
        let none = fact_state(&f, &[], &[]);
        assert_eq!((none.status, none.pending), (FactStatus::Unreviewed, false));

        let committed = [rec("alice", ReviewStatus::Confirmed, &h)];
        let s = fact_state(&f, &committed, &[]);
        assert_eq!((s.status, s.pending), (FactStatus::Confirmed, false));

        // The exported copy of alice's review is not counted twice.
        let same = sub(rec("alice", ReviewStatus::Confirmed, &h));
        let s = fact_state(&f, &committed, &[&same]);
        assert_eq!(s.current.len(), 1);

        // Bob disputes, pending: the fact is disputed.
        let bob = sub(rec("bob", ReviewStatus::Disputed, &h));
        let s = fact_state(&f, &committed, &[&bob]);
        assert_eq!((s.status, s.pending), (FactStatus::Disputed, false));
        assert_eq!(s.current.len(), 2);

        // Bob changes his mind: his latest counts.
        let bob2 = sub(rec("bob", ReviewStatus::Confirmed, &h));
        let s = fact_state(&f, &committed, &[&bob, &bob2]);
        assert_eq!(s.status, FactStatus::Confirmed);

        // Only pending reviews: pending.
        let s = fact_state(&f, &[], &[&bob2]);
        assert_eq!((s.status, s.pending), (FactStatus::Confirmed, true));
    }

    #[test]
    fn reviews_of_another_value_are_stale() {
        let f = fact(json!({"nominal": 1.0, "unit": "mm"}));
        let old = fact(json!({"nominal": 1.1, "unit": "mm"})).hash;
        let s = fact_state(&f, &[rec("alice", ReviewStatus::Confirmed, &old)], &[]);
        assert_eq!((s.status, s.stale), (FactStatus::Unreviewed, 1));
    }

    #[test]
    fn values_display_briefly() {
        let d = |v: Value| display_value(Some(&v));
        assert_eq!(d(json!({"nominal": 3.9, "unit": "mm"})), "3.9 mm");
        assert_eq!(d(json!({"min": 0.9, "max": 1.1, "unit": "V"})), "0.9–1.1 V");
        assert_eq!(d(json!({"min": -10.0, "max": 10.0, "unit": "%"})), "±10%");
        assert_eq!(
            d(json!({"nominal": 0.08, "max": 0.1, "unit": "Ω"})),
            "0.08 Ω (≤0.1)"
        );
        assert_eq!(d(json!({"max": 2.0, "unit": "A"})), "≤2 A");
        assert_eq!(d(json!({"value": "X7R"})), "X7R");
        assert_eq!(d(json!({"status": "verified"})), "verified");
        assert_eq!(
            d(
                json!({"1": {"name": "VDD", "type": "power"}, "2": {"name": "GND", "type": "ground"}})
            ),
            "2 pins: 1 VDD, 2 GND"
        );
        assert_eq!(d(json!({"name": "GPIO0", "type": "io"})), "GPIO0 (io)");
        assert_eq!(d(json!(48)), "48");
        assert_eq!(display_value(None), "");
    }
}
