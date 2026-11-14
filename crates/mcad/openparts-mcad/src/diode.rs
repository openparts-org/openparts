//! Two-terminal molded packages with leads (as opposed to `chip`'s
//! wrap-around terminations): SOD-123/323/523 with bent or flat leads,
//! and DO-214 (SMA/SMB/SMC) with J-bend leads folded under the body.
//!
//! Frame (the same as `chip`): `body_length` is the body along X, the
//! terminal axis; `body_width` along Y; `overall_length` the lead-tip to
//! lead-tip span along X (JEDEC HE / E). Terminal "1" is at -X, "2" at
//! +X -- which one is the cathode is the Device's business (its pin
//! names), so no polarity marker is emitted here.

use crate::defaults::{self, value, Axis, GullWing, GullWingDefaults};
use crate::{
    Body, BodyShape, Lead, LeadForm, McadError, MechanicalGeometry, Mounting, Point3, Size3,
};
use openparts_core::{Package, PackageFamily as F};

/// Generates Mechanical Geometry for an SOD-class package: family `sod`
/// (gull-wing leads bent down to the board, SOD-123/323/523) or
/// `sod_flat` (flat leads straight out of the body underside,
/// SOD-123F/FL, SOD-323F). Lead toes land at `overall_length / 2`;
/// unstated lead dimensions fall back to `defaults::SOD` / `SOD_FLAT`.
pub fn generate_sod(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let flat = match package.family {
        F::Sod => false,
        F::SodFlat => true,
        other => return Err(McadError::UnsupportedFamily(other)),
    };
    let family_defaults = if flat {
        &defaults::SOD_FLAT
    } else {
        &defaults::SOD
    };
    let (body, shape, reach) = resolve(
        package,
        family_defaults,
        crate::defaults::diode::SOD_LEAD_RATIO,
    )?;
    let half = body.size.x / 2.0;
    let t = shape.thickness;
    let leads = terminals(|sign| Lead {
        number: String::new(),
        position: Point3 {
            x: sign * (half + reach / 2.0),
            y: 0.0,
            z: t / 2.0,
        },
        size: Size3 {
            x: reach,
            y: shape.lead_width,
            z: t,
        },
        mounting: Mounting::Smd,
        drill: None,
        slot: None,
    });
    let lead_form = if flat {
        LeadForm::Flat { thickness: t }
    } else {
        shape.lead_form()
    };
    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],
        pitch: crate::defaults::required(&package.pitch),
        details: crate::Details::None,
        body,
        leads,
        markers: vec![],
        lead_form: Some(lead_form),
    })
}

/// Generates Mechanical Geometry for a DO-214 package (SMA = DO-214AC,
/// SMB = AA, SMC = AB): J-bend leads out of each body end, down the end
/// face and folded under the body, so each pad is the folded foot --
/// `lead_length` (L) in from `overall_length / 2`. The body sits on the
/// folded leads (its underside at the lead thickness).
pub fn generate_do214(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let (mut body, shape, reach) = resolve(
        package,
        &defaults::DO214,
        crate::defaults::diode::DO214_LEAD_RATIO,
    )?;
    let t = shape.thickness;
    let half = body.size.x / 2.0;
    let end = half + reach;
    if reach <= t {
        return Err(McadError::InconsistentDimensions(format!(
            "do214: overall_length leaves {reach:.3} mm beyond the body for a {t:.3} mm lead"
        )));
    }
    // The foot runs from the overall end back under the body; never past
    // the body centre.
    let foot = shape.foot_length.min(end - 0.1);
    body.position.z += t;
    let leads = terminals(|sign| Lead {
        number: String::new(),
        position: Point3 {
            x: sign * (end - foot / 2.0),
            y: 0.0,
            z: t / 2.0,
        },
        size: Size3 {
            x: foot,
            y: shape.lead_width,
            z: t,
        },
        mounting: Mounting::Smd,
        drill: None,
        slot: None,
    });
    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],
        pitch: crate::defaults::required(&package.pitch),
        details: crate::Details::None,
        body,
        leads,
        markers: vec![],
        lead_form: Some(LeadForm::JBend {
            thickness: t,
            foot_length: foot,
        }),
    })
}

/// The body (X = body_length), the resolved lead shape and the reach
/// from the body end to the lead tip.
fn resolve(
    package: &Package,
    family_defaults: &'static GullWingDefaults,
    lead_ratio: f64,
) -> Result<(Body, GullWing, f64), McadError> {
    if package.lead_count != 2 {
        return Err(McadError::InvalidLeadCount(package.lead_count));
    }
    let d = &package.dimensions;
    let body_len = defaults::required(&d.body_length)
        .ok_or(McadError::MissingDimension("dimensions.body_length"))?;
    let body_w = defaults::required(&d.body_width)
        .ok_or(McadError::MissingDimension("dimensions.body_width"))?;
    let mut shape = GullWing::resolve(package, family_defaults);
    if value(&d.lead_width).is_none() {
        shape.lead_width =
            defaults::two_terminal_lead_width(shape.default_lead_width(), body_w, lead_ratio);
    }
    // `protrusion(Axis::Y)` reads overall_length against body_length --
    // this frame's terminal axis.
    let reach = shape.protrusion(package, Axis::Y)?;
    let h = shape.body_height;
    let body = Body {
        position: Point3 {
            x: 0.0,
            y: 0.0,
            z: shape.standoff + h / 2.0,
        },
        size: Size3 {
            x: body_len,
            y: body_w,
            z: h,
        },
        shape: BodyShape::Box,
    };
    Ok((body, shape, reach))
}

/// Terminal "1" at -X and "2" at +X, from `make(sign)`.
fn terminals(make: impl Fn(f64) -> Lead) -> Vec<Lead> {
    [("1", -1.0), ("2", 1.0)]
        .into_iter()
        .map(|(n, sign)| Lead {
            number: n.to_string(),
            ..make(sign)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(yaml: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nlead_count: 2\n\
             pitch: {{unit: mm}}\n{yaml}"
        ))
        .unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Diodes Inc. SOD123 (DS30086): E 2.55..2.85, D 1.40..1.70,
    /// He 3.55..3.85, b 0.55..0.65 (illustrative).
    fn sod123() -> Package {
        package(
            "family: sod\ndimensions:\n\
             \x20 body_length: {min: 2.55, max: 2.85, unit: mm}\n\
             \x20 body_width: {min: 1.40, max: 1.70, unit: mm}\n\
             \x20 overall_length: {min: 3.55, max: 3.85, unit: mm}\n\
             \x20 lead_width: {min: 0.55, max: 0.65, unit: mm}\n",
        )
    }

    fn sma() -> Package {
        package(
            "family: do214\ndimensions:\n\
             \x20 body_length: {min: 3.99, max: 4.50, unit: mm}\n\
             \x20 body_width: {min: 2.54, max: 2.79, unit: mm}\n\
             \x20 body_height: {min: 1.98, max: 2.29, unit: mm}\n\
             \x20 overall_length: {min: 4.93, max: 5.28, unit: mm}\n\
             \x20 lead_width: {min: 1.25, max: 1.65, unit: mm}\n\
             \x20 lead_length: {min: 0.76, max: 1.52, unit: mm}\n\
             \x20 lead_thickness: {min: 0.15, max: 0.31, unit: mm}\n",
        )
    }

    #[test]
    fn sod_leads_reach_the_overall_length_on_both_ends() {
        let g = generate_sod(&sod123()).unwrap();
        let numbers: Vec<&str> = g.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, ["1", "2"]);
        let (l1, l2) = (&g.leads[0], &g.leads[1]);
        assert!(close(l1.position.x - l1.size.x / 2.0, -1.85));
        assert!(close(l2.position.x + l2.size.x / 2.0, 1.85));
        assert!(
            close(l1.position.x + l1.size.x / 2.0, -1.35),
            "from the body end"
        );
        assert!(close(l1.size.y, 0.6));
        assert!(matches!(g.lead_form, Some(LeadForm::GullWing { .. })));
    }

    #[test]
    fn sod_flat_has_flat_leads_and_small_bodies_get_narrow_default_leads() {
        let mut p = sod123();
        p.family = openparts_core::PackageFamily::SodFlat;
        p.dimensions.lead_width = None;
        p.dimensions.body_width = openparts_core::Dimension {
            nominal: Some(0.8),
            min: None,
            max: None,
            unit: "mm".into(),
        };
        let g = generate_sod(&p).unwrap();
        assert!(matches!(g.lead_form, Some(LeadForm::Flat { .. })));
        assert!(close(g.leads[0].size.y, 0.32), "capped at 40 % of the body");
        assert!(close(g.body.position.z, g.body.size.z / 2.0), "no standoff");
    }

    #[test]
    fn do214_pads_are_the_folded_feet_and_the_body_sits_on_them() {
        let g = generate_do214(&sma()).unwrap();
        let (l1, l2) = (&g.leads[0], &g.leads[1]);
        let end = 5.105 / 2.0;
        assert!(close(l2.position.x + l2.size.x / 2.0, end));
        assert!(close(l2.size.x, 1.14), "L midpoint");
        assert!(close(l1.position.x, -l2.position.x));
        assert!(
            l2.position.x - l2.size.x / 2.0 < g.body.size.x / 2.0,
            "under the body"
        );
        assert!(close(l1.size.y, 1.45));
        assert!(close(g.body.position.z, 0.23 + 2.135 / 2.0));
    }

    #[test]
    fn wrong_counts_spans_and_families_are_errors() {
        let mut p = sod123();
        p.lead_count = 3;
        assert!(matches!(
            generate_sod(&p),
            Err(McadError::InvalidLeadCount(3))
        ));
        let mut p = sma();
        p.dimensions.overall_length = Some(openparts_core::Dimension {
            nominal: Some(4.3),
            min: None,
            max: None,
            unit: "mm".into(),
        });
        assert!(matches!(
            generate_do214(&p),
            Err(McadError::InconsistentDimensions(_))
        ));
    }
}
