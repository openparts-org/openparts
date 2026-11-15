//! Human review records (Canonical Data Specification section 26.1):
//! what a reviewable fact is, the hash that ties a review to the value
//! it reviewed, and the checks of a `kind: review` document.

use crate::Diagnostic;
use openparts_core::{FactReview, ProvenanceEntry, ProvenanceMap, ReviewDocument, SourceId};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Longest reviewer name, in characters.
pub const MAX_REVIEWER_LEN: usize = 64;
/// Longest comment, in characters.
pub const MAX_COMMENT_LEN: usize = 2000;
/// Longest corrected value, in characters.
pub const MAX_CORRECTED_VALUE_LEN: usize = 500;
/// Longest pointer, in characters.
pub const MAX_POINTER_LEN: usize = 256;

/// One reviewable fact of a document: a provenance key, the value it
/// points at and the evidence cited for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Fact {
    pub pointer: String,
    /// `None` when the pointer names nothing in the document (the
    /// validator reports that as `provenance.unknown_path`).
    pub value: Option<Value>,
    pub evidence: Vec<ProvenanceEntry>,
    /// [`fact_hash`] of `value` and `evidence`.
    pub hash: String,
}

/// Every fact of `doc`: one per key of its `provenance`, in key order.
pub fn facts_of<T: serde::Serialize>(doc: &T, provenance: &ProvenanceMap) -> Vec<Fact> {
    let json = serde_json::to_value(doc).unwrap_or(Value::Null);
    provenance
        .iter()
        .map(|(pointer, evidence)| {
            let value = json.pointer(pointer).cloned();
            Fact {
                pointer: pointer.clone(),
                hash: fact_hash(value.as_ref(), evidence),
                value,
                evidence: evidence.clone(),
            }
        })
        .collect()
}

/// `sha256:<hex>` of a fact: the canonical JSON (object keys sorted, no
/// whitespace) of `{"evidence": [...], "value": ...}`. Changing the
/// value, or what is cited for it (source, page, locator), changes the
/// hash -- and makes every review of the old one stale.
pub fn fact_hash(value: Option<&Value>, evidence: &[ProvenanceEntry]) -> String {
    let evidence = serde_json::to_value(evidence).unwrap_or(Value::Null);
    let doc =
        serde_json::json!({ "value": value.cloned().unwrap_or(Value::Null), "evidence": evidence });
    let mut text = String::new();
    canonical_json(&doc, &mut text);
    let digest = Sha256::digest(text.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

fn canonical_json(v: &Value, out: &mut String) {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                canonical_json(&map[k], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical_json(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// What is wrong with one review record's own fields (empty when
/// nothing): the same limits apply to a record in the dataset and to a
/// submission openparts-server accepts.
pub fn review_record_problems(r: &FactReview) -> Vec<String> {
    let mut out = Vec::new();
    let printable = |s: &str| !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t');
    if !r.pointer.starts_with('/') || r.pointer.chars().count() > MAX_POINTER_LEN {
        out.push(format!(
            "pointer must be a JSON Pointer of at most {MAX_POINTER_LEN} characters"
        ));
    }
    let reviewer = r.reviewer.trim();
    if reviewer.is_empty()
        || reviewer != r.reviewer
        || r.reviewer.chars().count() > MAX_REVIEWER_LEN
        || r.reviewer.chars().any(char::is_control)
    {
        out.push(format!(
            "reviewer must be 1-{MAX_REVIEWER_LEN} characters without leading/trailing spaces or control characters"
        ));
    }
    if !is_day(&r.date) {
        out.push(format!("date \"{}\" is not YYYY-MM-DD", r.date));
    }
    let hex = r.value_hash.strip_prefix("sha256:").unwrap_or("");
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        out.push("value_hash must be sha256:<64 lowercase hex digits>".into());
    }
    if r.sources.is_empty() {
        out.push("sources must name the Source revision(s) the fact was checked against".into());
    }
    if let Some(c) = &r.comment {
        if c.chars().count() > MAX_COMMENT_LEN || !printable(c) {
            out.push(format!(
                "comment must be at most {MAX_COMMENT_LEN} printable characters"
            ));
        }
    }
    if let Some(v) = &r.corrected_value {
        if v.chars().count() > MAX_CORRECTED_VALUE_LEN || !printable(v) {
            out.push(format!(
                "corrected_value must be at most {MAX_CORRECTED_VALUE_LEN} printable characters"
            ));
        }
    }
    if r.status == openparts_core::ReviewStatus::Disputed
        && r.comment.as_deref().is_none_or(|c| c.trim().is_empty())
    {
        out.push("a dispute needs a comment saying what the source says instead".into());
    }
    out
}

fn is_day(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && s.split('-').all(|p| p.bytes().all(|c| c.is_ascii_digit()))
        && ("01"..="12").contains(&&s[5..7])
        && ("01"..="31").contains(&&s[8..10])
}

/// Checks one review document against the facts of its target (`None`
/// when the target isn't in the dataset). Malformed records are errors;
/// a review of a value that has since changed, or of a fact or document
/// that no longer exists, is a `review.stale` warning -- the data may
/// change, and the review then simply stops counting.
pub fn validate_review(
    doc: &ReviewDocument,
    target_facts: Option<&[Fact]>,
    known_sources: &BTreeSet<SourceId>,
) -> Vec<Diagnostic> {
    let entity = format!("review:{}", doc.id);
    let mut out = Vec::new();
    if doc.id != ReviewDocument::id_for(doc.target_kind, &doc.target) {
        out.push(Diagnostic::error(
            "review.invalid",
            &entity,
            Some("/id"),
            format!(
                "id must be \"{}\" (target_kind/target)",
                ReviewDocument::id_for(doc.target_kind, &doc.target)
            ),
        ));
    }
    if target_facts.is_none() {
        out.push(Diagnostic::warning(
            "review.stale",
            &entity,
            Some("/target"),
            format!(
                "{} {} is not in the dataset: its reviews no longer count",
                doc.target_kind.as_str(),
                doc.target
            ),
        ));
    }
    let mut seen = BTreeSet::new();
    for (i, r) in doc.reviews.iter().enumerate() {
        let at = format!("/reviews/{i}");
        let problems = review_record_problems(r);
        let malformed = !problems.is_empty();
        for problem in problems {
            out.push(Diagnostic::error(
                "review.invalid",
                &entity,
                Some(&at),
                problem,
            ));
        }
        if !seen.insert((r.pointer.as_str(), r.reviewer.as_str())) {
            out.push(Diagnostic::error(
                "review.invalid",
                &entity,
                Some(&at),
                format!(
                    "{} reviewed {} twice: keep only the latest record",
                    r.reviewer, r.pointer
                ),
            ));
        }
        for source in &r.sources {
            if !known_sources.contains(source) {
                out.push(Diagnostic::error(
                    "ref.unknown_source",
                    &entity,
                    Some(&at),
                    format!("review cites source \"{source}\" which was not supplied"),
                ));
            }
        }
        // A malformed record is reported as such, not also as stale.
        let Some(facts) = target_facts.filter(|_| !malformed) else {
            continue;
        };
        match facts.iter().find(|f| f.pointer == r.pointer) {
            None => out.push(Diagnostic::warning(
                "review.stale",
                &entity,
                Some(&at),
                format!(
                    "{} has no provenance entry {} any more: this review no longer counts",
                    doc.target, r.pointer
                ),
            )),
            Some(f) if f.hash != r.value_hash => out.push(Diagnostic::warning(
                "review.stale",
                &entity,
                Some(&at),
                format!(
                    "{} {} changed since {} reviewed it on {}: review it again",
                    doc.target, r.pointer, r.reviewer, r.date
                ),
            )),
            Some(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Package, ReviewStatus};

    fn package() -> Package {
        serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/T\nfamily: soic\nlead_count: 8\n\
             pitch: {nominal: 1.27, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 3.9, unit: mm}\n\
             \x20 body_length: {nominal: 4.9, unit: mm}\n\
             provenance:\n\
             \x20 /dimensions/body_width:\n\
             \x20   - {source: ex/DS1@rev2, page: 12, locator: 'Package outline, E1'}\n\
             \x20 /pitch:\n\
             \x20   - {source: ex/DS1@rev2, page: 12, locator: 'Package outline, e'}\n",
        )
        .unwrap()
    }

    fn review(yaml_reviews: &str) -> ReviewDocument {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: review\nid: package/standards/T\ntarget_kind: package\n\
             target: standards/T\nreviews:\n{yaml_reviews}"
        ))
        .unwrap()
    }

    fn record(pointer: &str, hash: &str) -> String {
        format!(
            "  - {{pointer: '{pointer}', status: confirmed, reviewer: alice, date: '2026-11-15', \
             sources: [ex/DS1@rev2], value_hash: '{hash}'}}\n"
        )
    }

    fn sources() -> BTreeSet<SourceId> {
        [SourceId::from("ex/DS1@rev2")].into()
    }

    #[test]
    fn facts_are_the_provenance_keys_with_their_values() {
        let p = package();
        let facts = facts_of(&p, &p.provenance);
        let pointers: Vec<&str> = facts.iter().map(|f| f.pointer.as_str()).collect();
        assert_eq!(pointers, ["/dimensions/body_width", "/pitch"]);
        assert_eq!(facts[0].value.as_ref().unwrap()["nominal"], 3.9);
        assert!(facts[0].hash.starts_with("sha256:") && facts[0].hash.len() == 71);
        assert_ne!(facts[0].hash, facts[1].hash);
    }

    #[test]
    fn the_hash_follows_the_value_and_the_evidence_only() {
        let p = package();
        let before = facts_of(&p, &p.provenance)[0].hash.clone();
        // Another field changing leaves it alone.
        let mut other = p.clone();
        other.dimensions.body_length.nominal = Some(5.0);
        assert_eq!(facts_of(&other, &other.provenance)[0].hash, before);
        // The value changing changes it...
        let mut changed = p.clone();
        changed.dimensions.body_width.nominal = Some(4.0);
        assert_ne!(facts_of(&changed, &changed.provenance)[0].hash, before);
        // ...and so does what is cited for it.
        let mut recited = p.clone();
        recited
            .provenance
            .get_mut("/dimensions/body_width")
            .unwrap()[0]
            .page = Some(13);
        assert_ne!(facts_of(&recited, &recited.provenance)[0].hash, before);
    }

    #[test]
    fn a_current_review_is_clean() {
        let p = package();
        let facts = facts_of(&p, &p.provenance);
        let doc = review(&record("/dimensions/body_width", &facts[0].hash));
        assert_eq!(doc.reviews[0].status, ReviewStatus::Confirmed);
        let diags = validate_review(&doc, Some(&facts), &sources());
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn a_changed_value_makes_the_review_stale() {
        let p = package();
        let hash = facts_of(&p, &p.provenance)[0].hash.clone();
        let doc = review(&record("/dimensions/body_width", &hash));
        let mut changed = p.clone();
        changed.dimensions.body_width.nominal = Some(4.0);
        let facts = facts_of(&changed, &changed.provenance);
        let diags = validate_review(&doc, Some(&facts), &sources());
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].rule_id, "review.stale");
        assert!(!diags[0].is_error());
        // A pointer that is gone, and a target that is gone, are stale too.
        let doc = review(&record("/dimensions/lead_width", &hash));
        let diags = validate_review(&doc, Some(&facts), &sources());
        assert_eq!(diags[0].rule_id, "review.stale");
        let diags = validate_review(&doc, None, &sources());
        assert!(diags.iter().all(|d| d.rule_id == "review.stale"));
    }

    #[test]
    fn malformed_records_are_errors() {
        let p = package();
        let facts = facts_of(&p, &p.provenance);
        let h = &facts[0].hash;
        let doc = review(&format!(
            "{}{}  - {{pointer: /pitch, status: disputed, reviewer: ' bob', date: '15-11-2026', \
             sources: [ex/Unknown], value_hash: 'md5:1'}}\n",
            record("/dimensions/body_width", h),
            record("/dimensions/body_width", h),
        ));
        let diags = validate_review(&doc, Some(&facts), &sources());
        let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(diags.iter().all(|d| d.is_error()), "{diags:?}");
        for needle in [
            "twice",
            "reviewer must be",
            "is not YYYY-MM-DD",
            "value_hash must be",
            "a dispute needs a comment",
            "ex/Unknown",
        ] {
            assert!(
                messages.iter().any(|m| m.contains(needle)),
                "{needle}: {messages:?}"
            );
        }
        let mut wrong_id = review("");
        wrong_id.id = "part/standards/T".into();
        assert_eq!(
            validate_review(&wrong_id, Some(&facts), &sources())[0].rule_id,
            "review.invalid"
        );
    }
}
