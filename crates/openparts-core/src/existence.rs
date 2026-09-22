use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExistenceStatus {
    Unverified,
    Probable,
    Verified,
    HistoricalVerified,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Existence {
    pub status: ExistenceStatus,
}
