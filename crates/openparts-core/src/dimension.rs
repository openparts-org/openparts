use serde::{Deserialize, Serialize};

/// Canonical Data Specification section 7 / 9: a physical quantity that
/// may have `min`/`typ`/`max`, none of which are invented when a
/// datasheet doesn't state them.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Dimension {
    #[serde(default)]
    pub min: Option<f64>,
    /// The typical (nominal) value (schema 0.1: `nominal`).
    #[serde(default, alias = "nominal")]
    pub typ: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    pub unit: String,
}
