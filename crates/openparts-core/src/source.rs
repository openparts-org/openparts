use crate::ids::SourceId;
use crate::kind::Kind;
use serde::{Deserialize, Serialize};

/// Canonical Data Specification section 23.
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub url: String,
}
