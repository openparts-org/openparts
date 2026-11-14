use crate::ids::ManufacturerId;
use crate::kind::Kind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Website {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manufacturer {
    pub schema_version: String,
    pub kind: Kind,
    pub id: ManufacturerId,
    pub name: String,
    #[serde(default)]
    pub website: Option<Website>,
}
