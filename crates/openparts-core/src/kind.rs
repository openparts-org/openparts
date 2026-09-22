use serde::{Deserialize, Serialize};
use std::fmt;

/// Canonical Data Specification section 5: every document's required
/// `kind` header field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Manufacturer,
    Part,
    Device,
    Package,
    Source,
    Erratum,
    Rejected,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Manufacturer => "manufacturer",
            Kind::Part => "part",
            Kind::Device => "device",
            Kind::Package => "package",
            Kind::Source => "source",
            Kind::Erratum => "erratum",
            Kind::Rejected => "rejected",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
