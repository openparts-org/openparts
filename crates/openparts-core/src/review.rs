//! Human review records (Canonical Data Specification section 26.1):
//! a person checked one recorded fact -- one provenance pointer of a
//! Part, Device or Package -- against the location its provenance
//! cites, and confirmed or disputed it.
//!
//! Reviews live beside the data in `reviews/<target_kind>/<target>.yaml`,
//! one document per reviewed entity. They never change the fact itself:
//! a dispute's `corrected_value` is a note for whoever fixes the data.

use crate::ids::SourceId;
use crate::kind::Kind;
use serde::{Deserialize, Serialize};

/// The kind of document a review is about.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTargetKind {
    Part,
    Device,
    Package,
}

impl ReviewTargetKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReviewTargetKind::Part => "part",
            ReviewTargetKind::Device => "device",
            ReviewTargetKind::Package => "package",
        }
    }
}

/// A reviewer's verdict on one fact. "Unreviewed" is the absence of a
/// record, never a stored value.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// The value matches the cited location.
    Confirmed,
    /// It doesn't (the comment says why).
    Disputed,
}

/// One reviewer's verdict on one fact.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactReview {
    /// The provenance key reviewed (a JSON Pointer, e.g.
    /// `/dimensions/body_width`).
    pub pointer: String,
    pub status: ReviewStatus,
    /// Who reviewed (a name or handle, as the reviewer gave it).
    pub reviewer: String,
    /// When (`YYYY-MM-DD`).
    pub date: String,
    /// The Source revisions the fact was checked against (those its
    /// provenance cited at the time).
    pub sources: Vec<SourceId>,
    /// `sha256:<hex>` of the fact as reviewed: its value and its
    /// provenance entries (openparts-validator `fact_hash`). When the
    /// data changes the hash no longer matches and the review is stale:
    /// it stops counting until someone reviews the new value.
    pub value_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// For a dispute: the value the reviewer read at the cited location,
    /// as free text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrected_value: Option<String>,
}

/// Every review of one Part, Device or Package (`kind: review`).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDocument {
    pub schema_version: String,
    pub kind: Kind,
    /// `<target_kind>/<target>`, e.g. `part/raspberrypi/RP2040`.
    pub id: String,
    pub target_kind: ReviewTargetKind,
    /// The reviewed document's id.
    pub target: String,
    #[serde(default)]
    pub reviews: Vec<FactReview>,
}

impl ReviewDocument {
    /// The id a review document of `target` has.
    pub fn id_for(target_kind: ReviewTargetKind, target: &str) -> String {
        format!("{}/{target}", target_kind.as_str())
    }

    /// An empty review document of `target`.
    pub fn new(target_kind: ReviewTargetKind, target: &str) -> ReviewDocument {
        ReviewDocument {
            schema_version: "0.1".into(),
            kind: Kind::Review,
            id: ReviewDocument::id_for(target_kind, target),
            target_kind,
            target: target.to_string(),
            reviews: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_document_parses() {
        let doc: ReviewDocument = serde_yaml::from_str(
            r#"
schema_version: "0.1"
kind: review
id: part/ex/EX1
target_kind: part
target: ex/EX1
reviews:
  - pointer: /parameters/capacitance
    status: disputed
    reviewer: alice
    date: "2026-11-15"
    sources: [ex/DS1@rev2]
    value_hash: "sha256:00"
    comment: "Table 3 says 220 nF"
    corrected_value: "220 nF"
"#,
        )
        .unwrap();
        assert_eq!(doc.kind, Kind::Review);
        assert_eq!(doc.target_kind, ReviewTargetKind::Part);
        assert_eq!(doc.reviews[0].status, ReviewStatus::Disputed);
        assert_eq!(doc.reviews[0].corrected_value.as_deref(), Some("220 nF"));
        assert_eq!(ReviewDocument::id_for(doc.target_kind, &doc.target), doc.id);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err = serde_yaml::from_str::<FactReview>(
            "pointer: /x\nstatus: confirmed\nreviewer: a\ndate: '2026-01-01'\nsources: []\nvalue_hash: h\nconfidence: 0.9\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("confidence"), "{err}");
    }
}
