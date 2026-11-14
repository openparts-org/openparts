use crate::ids::PartId;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields = nullable))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lifecycle {
    pub status: LifecycleStatus,
    /// Canonical Data Specification section 13: replacement does not by
    /// itself imply full compatibility.
    #[serde(default)]
    pub replacement: Vec<PartId>,
}
