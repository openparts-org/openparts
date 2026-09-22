use crate::ids::ManufacturerId;
use crate::kind::Kind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Website {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manufacturer {
    pub schema_version: String,
    pub kind: Kind,
    pub id: ManufacturerId,
    pub name: String,
    #[serde(default)]
    pub website: Option<Website>,
}
