use crate::ids::PartId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleStatus {
    PreRelease,
    Active,
    Nrnd,
    EolAnnounced,
    Obsolete,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lifecycle {
    pub status: LifecycleStatus,
    /// Canonical Data Specification section 13: replacement does not by
    /// itself imply full compatibility.
    #[serde(default)]
    pub replacement: Vec<PartId>,
}
