//! Human review status of one data document (Canonical Data
//! Specification section 26.1): the optional top-level `review` block
//! of a Part, Device, Package or Source.
//!
//! ```yaml
//! review:
//!   status: reviewed
//!   by: [alice]        # GitHub logins of the reviewers
//!   pr: 123            # the openparts-data pull request that reviewed it
//!   date: 2026-11-20   # optional
//! ```
//!
//! No block means the document is unreviewed (an AI-extracted
//! candidate). Review happens in pull requests to openparts-data; the
//! block records that one did. Agents never write it, and drop it when
//! they change a reviewed document.

use serde::{Deserialize, Serialize};

/// The only stored status: "unreviewed" is the absence of the block.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// A person checked every value of the document against the
    /// locations its provenance cites.
    Reviewed,
}

/// A document's `review` block.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentReview {
    pub status: ReviewStatus,
    /// GitHub logins of the reviewers (required, at least one).
    #[serde(default)]
    pub by: Vec<String>,
    /// The openparts-data pull request that reviewed the document
    /// (required).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    /// When (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_block_parses() {
        let r: DocumentReview =
            serde_yaml::from_str("status: reviewed\nby: [alice, bob]\npr: 12\ndate: 2026-11-20\n")
                .unwrap();
        assert_eq!(r.status, ReviewStatus::Reviewed);
        assert_eq!(r.by, ["alice", "bob"]);
        assert_eq!(r.pr, Some(12));
        assert_eq!(r.date.as_deref(), Some("2026-11-20"));
    }

    #[test]
    fn unknown_status_and_fields_are_rejected() {
        assert!(serde_yaml::from_str::<DocumentReview>("status: pending\n").is_err());
        let err = serde_yaml::from_str::<DocumentReview>("status: reviewed\nconfidence: 1\n")
            .unwrap_err();
        assert!(err.to_string().contains("confidence"), "{err}");
    }
}
