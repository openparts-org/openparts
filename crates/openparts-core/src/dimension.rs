use serde::{Deserialize, Serialize};

/// Canonical Data Specification section 7 / 9: a physical quantity that
/// may have `nominal`/`min`/`max`, none of which are invented when a
/// datasheet doesn't state them.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Dimension {
    #[serde(default)]
    pub nominal: Option<f64>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    pub unit: String,
}
