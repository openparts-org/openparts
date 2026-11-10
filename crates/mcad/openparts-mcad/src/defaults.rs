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

/// TSSOP (JEDEC MO-153, 4.4 mm body): E 6.4 BSC over E1 4.4, A2 0.80..1.05,
/// b 0.19..0.30, c 0.09..0.20, L 0.45..0.75, A1 0.05..0.15.
pub(crate) const TSSOP: GullWingDefaults = GullWingDefaults {
    body_height: 1.0,
    lead_width: 0.25,
    protrusion: 1.0,
    thickness: 0.15,
    foot_length: 0.6,
    standoff: 0.1,
};

/// SSOP (JEDEC MO-150, 5.3 mm body): E 7.4..8.2 over E1 5.3, A2 1.65..1.85,
/// b 0.22..0.38, c 0.09..0.25, L 0.55..0.95, A1 0.05 min.
pub(crate) const SSOP: GullWingDefaults = GullWingDefaults {
    body_height: 1.75,
    lead_width: 0.3,
    protrusion: 1.25,
    thickness: 0.15,
    foot_length: 0.75,
    standoff: 0.1,
};

/// MSOP (JEDEC MO-187, 3.0 mm body): E 4.9 BSC over E1 3.0, A2 0.75..0.95,
/// b 0.22..0.38 (0.65 pitch), c 0.08..0.23, L 0.40..0.80, A1 0.00..0.15.
pub(crate) const MSOP: GullWingDefaults = GullWingDefaults {
    body_height: 0.85,
    lead_width: 0.3,
    protrusion: 0.95,
    thickness: 0.15,
    foot_length: 0.6,
    standoff: 0.08,
};

/// SOT-223 (JEDEC TO-261AA): HE 6.7..7.3 over E 3.3..3.7, A2 ~1.6,
/// b 0.66..0.84, c 0.23..0.35, L 0.75 min, A1 0.02..0.10.
pub(crate) const SOT223: GullWingDefaults = GullWingDefaults {
    body_height: 1.6,
    lead_width: 0.75,
    protrusion: 1.75,
    thickness: 0.3,
    foot_length: 0.9,
    standoff: 0.06,
};

/// TO-252 / DPAK (JEDEC TO-252AA): H 9.40..10.42 over a 5.97..6.22 body
/// with the tab reaching L3 0.88..1.27 past it, A 2.18..2.39, b 0.64..0.89,
/// c 0.46..0.61, A1 0.00..0.13. `protrusion` is the lead side only
/// (H - D - L3 ~ 10.1 - 6.1 - 1.0).
pub(crate) const DPAK: GullWingDefaults = GullWingDefaults {
    body_height: 2.25,
    lead_width: 0.76,
    protrusion: 3.0,
    thickness: 0.5,
    foot_length: 1.0,
    standoff: 0.05,
};

/// TO-252 / DPAK tab defaults beyond `DPAK`.
pub(crate) struct TabDefaults {
    /// Body edge to tab end (L3).
    pub length: f64,
    /// Tab metal thickness (c2).
    pub thickness: f64,
    /// Soldered tab length under the body, as a fraction of the body
    /// size across the leads (TO-252 drawings give only a minimum, e.g.
    /// 4.32 min under a ~6.1 body).
    pub under_body_ratio: f64,
}

pub(crate) const DPAK_TAB: TabDefaults = TabDefaults {
    length: 1.0,
    thickness: 0.5,
    under_body_ratio: 0.7,
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

/// Castellated-module (`module`) presentation defaults.
pub(crate) struct ModuleDefaults {
    /// Board thickness when the package states no `body_height`: the
    /// common 1.6 mm FR-4 board.
    pub board_thickness: f64,
    /// Copper pad thickness (1 oz copper, 35 um) -- drawing only.
    pub pad_thickness: f64,
}

pub(crate) const MODULE: ModuleDefaults = ModuleDefaults {
    board_thickness: 1.6,
    pad_thickness: 0.035,
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

/// The value a generator draws a body/pitch dimension at -- the one
/// rule every generator uses: the stated nominal, else the middle of a
/// stated min..max range, else the single stated limit (outline tables
/// often give a height only as "1.80 max"). Drawing only: the derived
/// value is never written back into Canonical Data, so packages record
/// exactly what the datasheet prints (Canonical Data Specification 20.1).
pub(crate) fn required(dim: &Dimension) -> Option<f64> {
    dim.nominal.or(match (dim.min, dim.max) {
        (Some(min), Some(max)) => Some((min + max) / 2.0),
        (min, max) => min.or(max),
    })
}

/// [`required`], or `MissingDimension(what)` when nothing is stated.
pub(crate) fn dim(dim: &Dimension, what: &'static str) -> Result<f64, crate::McadError> {
    required(dim).ok_or(crate::McadError::MissingDimension(what))
}

/// [`required`] for an optional dimension (e.g. body height).
pub(crate) fn stated(dim: &Option<Dimension>) -> Option<f64> {
    dim.as_ref().and_then(required)
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
        let (Some(span), Some(body)) = (value(span), required(body)) else {
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
