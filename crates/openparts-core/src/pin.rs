use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinType {
    /// A supply input.
    Power,
    /// A supply output (a regulator's VOUT, an MCU's core-regulator
    /// output).
    PowerOut,
    Ground,
    Input,
    Output,
    Io,
    Analog,
    /// A clock input.
    Clock,
    /// A clock the part drives (a flash clock, a crystal driver's output).
    ClockOut,
    Reset,
    Nc,
    Reserved,
    Passive,
    Unknown,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: PinType,
    #[serde(default)]
    pub alternate_functions: Vec<String>,
    /// The pin number this pin is internally connected to, when the
    /// source says so (a SOT-223/TO-252 tab tied to pin 2, a second
    /// collector terminal). Such a pin may repeat the other pin's name
    /// (Canonical Data Specification section 14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_to: Option<String>,
    /// The other half of a differential pair this pin belongs to, when
    /// the source names the pair (Canonical Data Specification
    /// section 14.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_pair: Option<DiffPair>,
}

/// A pin's membership in a differential pair (Canonical Data
/// Specification section 14.2). Recorded only where the source names
/// the pair (USB_DP/USB_DM, D+/D-). The impedance a standard asks for
/// (USB 2.0: 90 ohm differential) is that standard's, not a fact of the
/// part, and is not recorded here.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffPair {
    /// The pin number of the other half of the pair.
    pub partner: String,
    pub polarity: DiffPolarity,
    /// The interface standard the pair carries.
    pub standard: DiffStandard,
    /// The alternate function the pair belongs to, when the pin carries
    /// the pair only in that function (an ESP32 GPIO muxed to USB_D+).
    /// One of the pin's `alternate_functions`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// A differential impedance the part's own source states (ohm).
    /// Normally absent: the standard's impedance is not a part fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impedance_ohm: Option<f64>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffPolarity {
    /// The positive half (D+, DP, _P).
    P,
    /// The negative half (D-, DM, _N).
    N,
}

impl DiffPolarity {
    pub fn opposite(self) -> Self {
        match self {
            DiffPolarity::P => DiffPolarity::N,
            DiffPolarity::N => DiffPolarity::P,
        }
    }
}

/// The interface a differential pair carries. A closed vocabulary that
/// grows with the data, like `PinType`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffStandard {
    /// USB 2.0 (low/full/high speed D+/D-).
    Usb2,
}

/// A partial Pin used inside `DeviceRevision.overrides.pins` (Canonical
/// Data Specification section 19). Only the fields actually being
/// overridden are present; every other field of the base Pin is kept as-is.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinOverride {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "type")]
    pub pin_type: Option<PinType>,
    #[serde(default)]
    pub alternate_functions: Option<Vec<String>>,
    #[serde(default)]
    pub connected_to: Option<String>,
    #[serde(default)]
    pub diff_pair: Option<DiffPair>,
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
            connected_to: ov
                .connected_to
                .clone()
                .or_else(|| self.connected_to.clone()),
            diff_pair: ov.diff_pair.clone().or_else(|| self.diff_pair.clone()),
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
            connected_to: None,
            diff_pair: None,
        };
        let ov = PinOverride {
            name: Some("PDR_ON".to_string()),
            pin_type: None,
            alternate_functions: None,
            connected_to: None,
            diff_pair: None,
        };
        let merged = base.apply_override(&ov);
        assert_eq!(merged.name, "PDR_ON");
        // Not overridden -> unchanged from base.
        assert_eq!(merged.pin_type, PinType::Ground);
        assert_eq!(merged.alternate_functions, vec!["FOO".to_string()]);
    }
}
