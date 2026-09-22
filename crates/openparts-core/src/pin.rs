use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinType {
    Power,
    Ground,
    Input,
    Output,
    Io,
    Analog,
    Clock,
    Reset,
    Nc,
    Reserved,
    Passive,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pin {
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: PinType,
    #[serde(default)]
    pub alternate_functions: Vec<String>,
}

/// A partial Pin used inside `DeviceRevision.overrides.pins` (Canonical
/// Data Specification section 19). Only the fields actually being
/// overridden are present; every other field of the base Pin is kept as-is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PinOverride {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "type")]
    pub pin_type: Option<PinType>,
    #[serde(default)]
    pub alternate_functions: Option<Vec<String>>,
}

impl Pin {
    /// Merges an override onto this base pin. Fields absent from `ov` keep
    /// this pin's existing value — an override must never blank out an
    /// unrelated field.
    pub fn apply_override(&self, ov: &PinOverride) -> Pin {
        Pin {
            name: ov.name.clone().unwrap_or_else(|| self.name.clone()),
            pin_type: ov.pin_type.unwrap_or(self.pin_type),
            alternate_functions: ov
                .alternate_functions
                .clone()
                .unwrap_or_else(|| self.alternate_functions.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_keeps_unspecified_fields() {
        let base = Pin {
            name: "VSS".to_string(),
            pin_type: PinType::Ground,
            alternate_functions: vec!["FOO".to_string()],
        };
        let ov = PinOverride {
            name: Some("PDR_ON".to_string()),
            pin_type: None,
            alternate_functions: None,
        };
        let merged = base.apply_override(&ov);
        assert_eq!(merged.name, "PDR_ON");
        // Not overridden -> unchanged from base.
        assert_eq!(merged.pin_type, PinType::Ground);
        assert_eq!(merged.alternate_functions, vec!["FOO".to_string()]);
    }
}
