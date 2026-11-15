//! Land-pattern compatibility: whether a candidate fits the original's
//! footprint, from the footprint pads OpenParts generates for each
//! Package (openparts-mcad lands -> openparts-pcbcad pads, the same
//! pads the KiCad footprint gets).
//!
//! Package ids are vendor-specific (`SOT25-...-DIODES` vs TI `DBV0005A`),
//! so two packages are compared by their pads: the same number of pads,
//! each at the same place (within [`POSITION_TOLERANCE`]) and of a
//! similar size ([`SIZE_TOLERANCE`]), SMD against SMD and through-hole
//! against through-hole. Chip parts (0603, 0805...) are compared by
//! their standard size code: every vendor's 0603 goes on the same 0603
//! land. When a package's pads can't be generated (a missing
//! dimension), family, lead count, pitch and body size decide.

use openparts_core::family::chip_codes;
use openparts_core::{Dimension, Package, PackageFamily};
use serde::Serialize;

/// Largest distance between two pads' centres still counted as one pad
/// position (mm). Vendors' outlines of one JEDEC package differ by a
/// few hundredths; their IPC lands by up to about a tenth.
pub const POSITION_TOLERANCE: f64 = 0.25;
/// Largest difference of a pad's width or length (mm), or this share
/// of it if that is larger ([`SIZE_TOLERANCE_RATIO`]).
pub const SIZE_TOLERANCE: f64 = 0.3;
pub const SIZE_TOLERANCE_RATIO: f64 = 0.35;
/// Body sizes compared when pads can't be (mm).
pub const BODY_TOLERANCE: f64 = 0.2;

/// One pad of a generated footprint.
#[derive(Debug, Clone, PartialEq)]
pub struct Land {
    pub number: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub length: f64,
    pub through_hole: bool,
}

/// What [`compare`] needs of a Package, computed once per Package.
#[derive(Debug, Clone, PartialEq)]
pub struct LandPattern {
    pub family: PackageFamily,
    pub lead_count: u32,
    pub pitch: Option<f64>,
    pub body: (Option<f64>, Option<f64>),
    /// Imperial size code of a standard chip body (`0603`).
    pub size_code: Option<&'static str>,
    /// Metric code (`1608`).
    pub size_code_metric: Option<&'static str>,
    /// `None` when the footprint can't be generated.
    pub lands: Option<Vec<Land>>,
}

/// A dimension's value as the generators read it.
pub(crate) fn stated(d: &Dimension) -> Option<f64> {
    d.nominal.or(match (d.min, d.max) {
        (Some(a), Some(b)) => Some((a + b) / 2.0),
        (a, b) => a.or(b),
    })
}

impl LandPattern {
    pub fn of(pkg: &Package) -> LandPattern {
        let w = stated(&pkg.dimensions.body_width);
        let l = stated(&pkg.dimensions.body_length);
        let codes = match (pkg.family, w, l) {
            (PackageFamily::Chip, Some(w), Some(l)) => chip_codes(w.max(l), w.min(l)),
            _ => &[],
        };
        let lands = openparts_mcad::generate(pkg).ok().map(|g| {
            let fp = openparts_pcbcad::build_footprint(&pkg.id.0, &g);
            fp.pads
                .iter()
                .filter(|p| p.mounting != openparts_mcad::Mounting::NonPlated)
                .map(|p| Land {
                    number: p.number.clone(),
                    x: p.position.x,
                    y: p.position.y,
                    width: p.size.0,
                    length: p.size.1,
                    through_hole: p.mounting == openparts_mcad::Mounting::ThroughHole,
                })
                .collect()
        });
        LandPattern {
            family: pkg.family,
            lead_count: pkg.lead_count,
            pitch: stated(&pkg.pitch),
            body: (w, l),
            size_code: codes.first().copied(),
            size_code_metric: codes.get(1).map(|c| c.trim_end_matches('m')),
            lands,
        }
    }
}

/// How compatibility was decided.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FootprintMethod {
    /// One Package document.
    SamePackage,
    /// Standard chip size code.
    SizeCode,
    /// Generated footprint pads.
    Pads,
    /// Family, lead count, pitch and body size (no pads to compare).
    Dimensions,
}

/// Why two land patterns differ.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FootprintDifference {
    SizeCode,
    PadCount,
    PadPosition,
    PadSize,
    Mounting,
    Family,
    LeadCount,
    Pitch,
    BodySize,
}

/// The footprint side of an alternative.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FootprintCheck {
    /// The candidate goes on the original's footprint.
    pub same_land_pattern: bool,
    pub method: FootprintMethod,
    /// What differs, when not the same.
    pub difference: Option<FootprintDifference>,
    /// The largest pad-centre offset found (mm), when pads were compared.
    pub max_offset_mm: Option<f64>,
}

/// The result of [`compare`]: the check, and (when the same) which
/// candidate pad sits on each original pad.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintMatch {
    pub check: FootprintCheck,
    pub pad_map: Vec<(String, String)>,
}

fn identity_map(p: &LandPattern) -> Vec<(String, String)> {
    match &p.lands {
        Some(l) => l
            .iter()
            .map(|l| (l.number.clone(), l.number.clone()))
            .collect(),
        None => (1..=p.lead_count)
            .map(|n| (n.to_string(), n.to_string()))
            .collect(),
    }
}

fn result(
    same: bool,
    method: FootprintMethod,
    difference: Option<FootprintDifference>,
    max_offset_mm: Option<f64>,
    pad_map: Vec<(String, String)>,
) -> FootprintMatch {
    FootprintMatch {
        check: FootprintCheck {
            same_land_pattern: same,
            method,
            difference,
            max_offset_mm,
        },
        pad_map,
    }
}

/// Whether `cand` fits on `orig`'s footprint.
pub fn compare(
    orig_id: &str,
    orig: &LandPattern,
    cand_id: &str,
    cand: &LandPattern,
) -> FootprintMatch {
    use FootprintDifference as D;
    use FootprintMethod as M;
    if orig_id == cand_id {
        return result(true, M::SamePackage, None, Some(0.0), identity_map(orig));
    }
    if orig.family == PackageFamily::Chip && cand.family == PackageFamily::Chip {
        if let (Some(a), Some(b)) = (orig.size_code, cand.size_code) {
            return if a == b {
                result(true, M::SizeCode, None, None, identity_map(orig))
            } else {
                result(false, M::SizeCode, Some(D::SizeCode), None, vec![])
            };
        }
    }
    if let (Some(a), Some(b)) = (&orig.lands, &cand.lands) {
        if a.len() != b.len() {
            return result(false, M::Pads, Some(D::PadCount), None, vec![]);
        }
        let mut used = vec![false; b.len()];
        let mut map = Vec::with_capacity(a.len());
        let mut worst: f64 = 0.0;
        for pa in a {
            let best = b
                .iter()
                .enumerate()
                .filter(|(i, _)| !used[*i])
                .map(|(i, pb)| (i, (pa.x - pb.x).hypot(pa.y - pb.y)))
                .min_by(|x, y| x.1.total_cmp(&y.1));
            let Some((i, d)) = best else {
                return result(false, M::Pads, Some(D::PadCount), None, vec![]);
            };
            if d > POSITION_TOLERANCE {
                return result(false, M::Pads, Some(D::PadPosition), Some(d), vec![]);
            }
            let pb = &b[i];
            if pa.through_hole != pb.through_hole {
                return result(false, M::Pads, Some(D::Mounting), Some(d), vec![]);
            }
            let close = |x: f64, y: f64| {
                (x - y).abs() <= SIZE_TOLERANCE.max(SIZE_TOLERANCE_RATIO * x.max(y))
            };
            if !close(pa.width, pb.width) || !close(pa.length, pb.length) {
                return result(false, M::Pads, Some(D::PadSize), Some(d), vec![]);
            }
            used[i] = true;
            worst = worst.max(d);
            map.push((pa.number.clone(), pb.number.clone()));
        }
        return result(true, M::Pads, None, Some(worst), map);
    }
    // No pads to compare: the package facts.
    let diff = if orig.family != cand.family {
        Some(D::Family)
    } else if orig.lead_count != cand.lead_count {
        Some(D::LeadCount)
    } else if !near(orig.pitch, cand.pitch, 0.02) {
        Some(D::Pitch)
    } else if !near(orig.body.0, cand.body.0, BODY_TOLERANCE)
        || !near(orig.body.1, cand.body.1, BODY_TOLERANCE)
    {
        Some(D::BodySize)
    } else {
        None
    };
    match diff {
        None => result(true, M::Dimensions, None, None, identity_map(orig)),
        Some(d) => result(false, M::Dimensions, Some(d), None, vec![]),
    }
}

fn near(a: Option<f64>, b: Option<f64>, tol: f64) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() <= tol,
        (None, None) => true,
        _ => false,
    }
}
