use crate::ids::ManufacturerId;
use crate::kind::Kind;
use serde::{Deserialize, Serialize};

/// Canonical Data Specification section 33: a tombstone for a claimed
/// MPN that was investigated and found not to exist / not to be valid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rejected {
    pub schema_version: String,
    pub kind: Kind,
    pub id: String,
    pub claimed_mpn: String,
    pub manufacturer: ManufacturerId,
    pub reason: String,
    #[serde(default)]
    pub sources_checked: Vec<String>,
}
