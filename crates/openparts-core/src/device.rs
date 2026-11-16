use crate::category::ComponentCategory;
use crate::ids::{DeviceId, ManufacturerId};
use crate::kind::Kind;
use crate::parameter::Parameters;
use crate::pin::{Pin, PinOverride};
use crate::provenance::ProvenanceMap;
use crate::serde_util::deserialize_no_dup_map;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical Data Specification section 18.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionDetection {
    pub method: String,
    #[serde(default)]
    pub register: Option<String>,
    #[serde(default)]
    pub mask: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

/// Canonical Data Specification section 19: a diff against the base
/// Device, not a full copy.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceOverrides {
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub pins: BTreeMap<String, PinOverride>,
}

/// Canonical Data Specification section 17.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceRevision {
    pub manufacturer_revision: String,
    #[serde(default)]
    pub detection: Vec<RevisionDetection>,
    #[serde(default)]
    pub overrides: DeviceOverrides,
}

/// Canonical Data Specification section 14.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    pub schema_version: String,
    pub kind: Kind,
    pub id: DeviceId,
    pub manufacturer: ManufacturerId,
    pub category: ComponentCategory,
    #[serde(default)]
    pub family: Option<String>,
    /// Keyed by pin-number string (e.g. "1", "42", "A1", "EP"). Never
    /// coerced to an integer — Pad grid position != Pin number.
    #[serde(deserialize_with = "deserialize_no_dup_map")]
    pub pins: BTreeMap<String, Pin>,
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub revisions: BTreeMap<String, DeviceRevision>,
    /// Electrical parameters every Part of this Device shares (section
    /// 11.1), e.g. a memory's capacity. A Part's own value for a key
    /// replaces this one.
    #[serde(
        default,
        skip_serializing_if = "Parameters::is_empty",
        deserialize_with = "deserialize_no_dup_map"
    )]
    pub parameters: Parameters,
    #[serde(default, deserialize_with = "deserialize_no_dup_map")]
    pub provenance: ProvenanceMap,
    /// Human review status (section 26.1); `None`: unreviewed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<crate::review::DocumentReview>,
}

/// The terminal a polarised two-terminal part marks (Canonical Data
/// Specification 20.10).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolarTerminal {
    /// A diode's or LED's cathode.
    Cathode,
    /// A polarised capacitor's `+` terminal.
    Positive,
}

impl Device {
    /// The pin a polarity mark belongs to, read from the source's pin
    /// names (never assumed from pin numbers: Kingbright LEDs number the
    /// cathode 1, Everlight the anode): an LED's or diode's cathode
    /// (`Cathode`, `K`), a polarised capacitor's positive terminal (`+`,
    /// `Anode`, `Positive`). `None` when the device has no single such
    /// pin (an MLCC, a bidirectional TVS, a dual diode).
    pub fn polarity_pin(&self) -> Option<(&str, PolarTerminal)> {
        let (wanted, terminal): (&[&str], _) = if self.category.is_within(ComponentCategory::Led)
            || self.category.is_within(ComponentCategory::Diode)
        {
            (&["cathode", "k", "kathode"], PolarTerminal::Cathode)
        } else if self.category.is_within(ComponentCategory::Capacitor) {
            (&["+", "anode", "positive"], PolarTerminal::Positive)
        } else {
            return None;
        };
        let mut hits = self
            .pins
            .iter()
            .filter(|(_, pin)| wanted.contains(&pin.name.trim().to_ascii_lowercase().as_str()));
        match (hits.next(), hits.next()) {
            (Some((number, _)), None) => Some((number.as_str(), terminal)),
            _ => None,
        }
    }
}
