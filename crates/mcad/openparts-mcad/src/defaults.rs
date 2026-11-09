//! The one place package-shape defaults live, and how they combine with
//! the package's own dimensions: a datasheet value always wins over a
//! family default. Footprint pads (through `Lead`) and the detailed 3D
//! model (through `LeadForm`) are both built from what this module
//! resolves, so they cannot disagree.
//!
//! Every default here is a generator implementation detail for a
//! geometric approximation -- JEDEC-typical where a JEDEC outline exists
//! -- and is never written back into Canonical Data as a fact (Canonical
//! Data Specification section 7: never invent a nominal).

use crate::{LeadForm, McadError};
use openparts_core::{Dimension, Package};

/// Defaults for one gull-wing family.
pub(crate) struct GullWingDefaults {
    /// Molded body thickness (A2).
    pub body_height: f64,
    /// Lead width (b).
    pub lead_width: f64,
    /// Body edge to lead toe, i.e. (E - E1) / 2.
    pub protrusion: f64,
    /// Lead-frame thickness (c).
    pub thickness: f64,
    /// Flat foot length (L).
    pub foot_length: f64,
    /// Standoff (A1).
    pub standoff: f64,
}

/// LQFP/TQFP (JEDEC MS-026): every MS-026 body size has E = E1 + 2.0 mm,
/// b 0.22 (0.5 mm pitch), c 0.09..0.20, L 0.45..0.75, A1 0.05..0.15.
pub(crate) const QFP: GullWingDefaults = GullWingDefaults {
    body_height: 1.4,
    lead_width: 0.22,
    protrusion: 1.0,
    thickness: 0.15,
    foot_length: 0.6,
    standoff: 0.1,
};

/// SOIC/SOP (JEDEC MS-012): E 6.0 over E1 3.9, b 0.31..0.51,
/// c 0.17..0.25, L 0.40..1.27, A1 0.10..0.25; height is MS-012's A max.
pub(crate) const SOIC: GullWingDefaults = GullWingDefaults {
    body_height: 1.75,
    lead_width: 0.4,
    protrusion: 1.05,
    thickness: 0.2,
    foot_length: 0.8,
    standoff: 0.15,
};

/// SOT-23 class (JEDEC TO-236): HE 2.4 over E 1.3, b 0.3..0.5,
/// c 0.08..0.20, L ~0.4, A1 0.01..0.10.
pub(crate) const SOT: GullWingDefaults = GullWingDefaults {
    body_height: 1.0,
    lead_width: 0.4,
    protrusion: 0.55,
    thickness: 0.12,
    foot_length: 0.4,
    standoff: 0.05,
};

/// QFN/DFN (JEDEC MO-220-typical): A 0.9, L 0.4, b 0.25, A3 0.2 REF.
pub(crate) struct FlushDefaults {
    pub body_height: f64,
    pub lead_length: f64,
    pub lead_width: f64,
    pub thickness: f64,
}

pub(crate) const QFN: FlushDefaults = FlushDefaults {
    body_height: 0.9,
    lead_length: 0.4,
    lead_width: 0.25,
    thickness: 0.2,
};

/// The value a generator uses for an optional dimension: the stated
/// nominal, else the middle of a stated min..max range (many outline
/// tables give only limits). The midpoint is an approximation inside
/// the datasheet's own range, used for drawing only -- never stored.
/// `None` when neither is stated (a lone min or max isn't enough).
pub(crate) fn value(dim: &Option<Dimension>) -> Option<f64> {
    let d = dim.as_ref()?;
    d.nominal.or(match (d.min, d.max) {
        (Some(min), Some(max)) => Some((min + max) / 2.0),
        _ => None,
    })
}

/// Which body axis a row of leads exits across.
#[derive(Clone, Copy)]
pub(crate) enum Axis {
    /// Leads on the left/right edges; span is `overall_width`.
    X,
    /// Leads on the bottom/top edges; span is `overall_length`.
    Y,
}

/// Resolved gull-wing dimensions for one package.
pub(crate) struct GullWing {
    pub body_height: f64,
    pub lead_width: f64,
    pub thickness: f64,
    pub foot_length: f64,
    pub standoff: f64,
    defaults: &'static GullWingDefaults,
}

impl GullWing {
    pub fn resolve(package: &Package, defaults: &'static GullWingDefaults) -> GullWing {
        let d = &package.dimensions;
        let standoff = value(&d.standoff).unwrap_or(defaults.standoff);
        GullWing {
            body_height: body_height(package, standoff, defaults.body_height),
            lead_width: value(&d.lead_width).unwrap_or(defaults.lead_width),
            thickness: value(&d.lead_thickness).unwrap_or(defaults.thickness),
            foot_length: value(&d.lead_length).unwrap_or(defaults.foot_length),
            standoff,
            defaults,
        }
    }

    /// Body edge to lead toe across `axis`: half the stated overall span
    /// minus half the body, else the family default.
    pub fn protrusion(&self, package: &Package, axis: Axis) -> Result<f64, McadError> {
        let d = &package.dimensions;
        let (span, body, name) = match axis {
            Axis::X => (&d.overall_width, &d.body_width, "dimensions.overall_width"),
            Axis::Y => (
                &d.overall_length,
                &d.body_length,
                "dimensions.overall_length",
            ),
        };
        let (Some(span), Some(body)) = (value(span), body.nominal) else {
            return Ok(self.defaults.protrusion);
        };
        let protrusion = (span - body) / 2.0;
        if protrusion <= 0.0 {
            return Err(McadError::InconsistentDimensions(format!(
                "{name} ({span}) must exceed the body size ({body})"
            )));
        }
        Ok(protrusion)
    }

    pub fn lead_form(&self) -> LeadForm {
        LeadForm::GullWing {
            thickness: self.thickness,
            foot_length: self.foot_length,
            standoff: self.standoff,
        }
    }
}

/// Resolved flush-terminal (QFN/DFN) dimensions for one package.
pub(crate) struct Flush {
    pub body_height: f64,
    pub lead_length: f64,
    pub lead_width: f64,
    pub thickness: f64,
}

impl Flush {
    pub fn resolve(package: &Package, defaults: &FlushDefaults) -> Flush {
        let d = &package.dimensions;
        Flush {
            body_height: body_height(package, 0.0, defaults.body_height),
            lead_length: value(&d.lead_length).unwrap_or(defaults.lead_length),
            lead_width: value(&d.lead_width).unwrap_or(defaults.lead_width),
            thickness: value(&d.lead_thickness).unwrap_or(defaults.thickness),
        }
    }

    pub fn lead_form(&self) -> LeadForm {
        LeadForm::Flush {
            thickness: self.thickness,
        }
    }
}

/// Body thickness: `body_height`, else the overall height less the
/// standoff, else `default`.
pub(crate) fn body_height(package: &Package, standoff: f64, default: f64) -> f64 {
    let d = &package.dimensions;
    value(&d.body_height)
        .or_else(|| value(&d.overall_height).map(|a| a - standoff))
        .filter(|h| *h > 0.0)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(nominal: Option<f64>, min: Option<f64>, max: Option<f64>) -> Option<Dimension> {
        Some(Dimension {
            nominal,
            min,
            max,
            unit: "mm".into(),
        })
    }

    #[test]
    fn value_prefers_nominal_then_midpoint_never_a_lone_limit() {
        assert_eq!(value(&dim(Some(0.2), Some(0.1), Some(0.5))), Some(0.2));
        assert_eq!(value(&dim(None, Some(0.1), Some(0.5))), Some(0.3));
        assert_eq!(value(&dim(None, None, Some(0.5))), None);
        assert_eq!(value(&None), None);
    }
}
