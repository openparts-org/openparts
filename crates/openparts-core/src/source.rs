use crate::ids::SourceId;
use crate::kind::Kind;
use serde::{Deserialize, Serialize};

/// Canonical Data Specification section 23.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Datasheet,
    ReferenceManual,
    Errata,
    ApplicationNote,
    PackageDrawing,
    ProductPage,
    Pcn,
    EolNotice,
    Other,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub schema_version: String,
    pub kind: Kind,
    pub id: SourceId,
    pub publisher: String,
    pub document_id: String,
    pub document_type: DocumentType,
    /// Kept as a String, not a number: revisions are labels ("8", "A",
    /// "rev-b2"), not an ordered numeric scale.
    pub revision: String,
    pub date: String,
    /// Where the document was read. Vendor "latest" links (TI's
    /// `.../symlink/<part>.pdf`) may serve a newer revision later than
    /// the one recorded in `revision`.
    pub url: String,
    /// When the document was downloaded and read (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieved: Option<String>,
    /// SHA-256 (lowercase hex) of the document as read, when recorded.
    /// openparts-server's document cache refuses a download that
    /// doesn't match it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Human review status (section 26.1); `None`: unreviewed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<crate::review::DocumentReview>,
}
