use serde::{Deserialize, Serialize};

/// Canonical Data Specification section 14 (Device): a broad
/// component-type classification, used for catalog browsing rather than
/// engineering decisions. Initial vocabulary -- extend as new component
/// types enter the dataset, the same way `PinType`'s vocabulary grows
/// (section 15). `Other` covers a real component type not yet in this
/// list; it must still be set explicitly, never left absent (Unknown
/// Values, section 7 -- this field has no default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentCategory {
    Microcontroller,
    Capacitor,
    Resistor,
    Inductor,
    Diode,
    Transistor,
    Led,
    Crystal,
    Fuse,
    Connector,
    Other,
}
