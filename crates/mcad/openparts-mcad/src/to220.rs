//! TO-220, mounted vertically: a molded body standing on its leads, the
//! metal tab (with its mounting hole) rising behind it.
//!
//! Frame (as KiCad's `TO-220-3_Vertical`): leads in one row along X at
//! `pitch`, centred on the origin, pin 1 at -X (front view, leads down);
//! the tab side is +Y. `body_width` is the molding across the leads (X),
//! `body_length` its thickness (Y, tab included), `body_height` the
//! molding's height above its underside (Z); `overall_height` (the tab
//! top), `mounting_hole_*` and `standoff` shape only the 3D model.

use crate::defaults::{self, value};
use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::Package;

/// Generates Mechanical Geometry for a vertical TO-220-style package
/// (`to220`): `lead_count` through-hole leads, `hole_diameter` drills
/// (else `defaults::TO220.drill`) in `pad_diameter` lands. The body
/// centre sits `body_offset` behind the lead row, toward the tab (else
/// the KiCad footprint's 0.95 mm), and `standoff` above the board (else
/// on it).
pub fn generate_to220(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let n = package.lead_count;
    if !(2..=15).contains(&n) {
        return Err(McadError::InvalidLeadCount(n));
    }
    let d = &package.dimensions;
    let pitch = defaults::dim(&package.pitch, "pitch.nominal")?;
    let body_w = defaults::required(&d.body_width)
        .ok_or(McadError::MissingDimension("dimensions.body_width"))?;
    let body_t = defaults::required(&d.body_length)
        .ok_or(McadError::MissingDimension("dimensions.body_length"))?;
    // `overall_height` is the tab top here, never the molding: only a
    // stated body_height counts.
    let body_h = defaults::stated(&d.body_height).unwrap_or(defaults::TO220.body_height);
    let standoff = value(&d.standoff).unwrap_or(0.0);
    let offset = value(&d.body_offset).unwrap_or(defaults::TO220.body_offset);
    let span = (n - 1) as f64 * pitch;
    if span + pitch / 2.0 > body_w {
        return Err(McadError::InconsistentDimensions(format!(
            "{n} leads at {pitch} mm pitch do not fit under a {body_w} mm wide body"
        )));
    }
    let drill = value(&d.hole_diameter).unwrap_or(defaults::TO220.drill);
    let pad = value(&d.pad_diameter)
        .unwrap_or(defaults::TO220.pad_diameter)
        .min(pitch - 0.25)
        .max(drill + 0.2);

    let body = Body {
        position: Point3 {
            x: 0.0,
            y: offset,
            z: standoff + body_h / 2.0,
        },
        size: Size3 {
            x: body_w,
            y: body_t,
            z: body_h,
        },
        shape: BodyShape::Box,
    };
    let leads = (0..n)
        .map(|i| Lead {
            number: (i + 1).to_string(),
            position: Point3 {
                x: -span / 2.0 + i as f64 * pitch,
                y: 0.0,
                z: 0.0,
            },
            size: Size3 {
                x: pad,
                y: pad,
                z: standoff,
            },
            mounting: Mounting::ThroughHole,
            drill: Some(drill),
            slot: None,
        })
        .collect();
    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],
        pitch: crate::defaults::required(&package.pitch),
        details: crate::Details::To220(to220_features(package)),
        body,
        leads,
        markers: vec![],
        lead_form: None,
    })
}

/// What a TO-220's 3D model needs beyond its geometry: each value the
/// package's own, else `defaults::TO220` (drawing only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct To220Features {
    pub lead_width: f64,
    pub lead_thickness: f64,
    pub tail_length: f64,
    pub tab_thickness: f64,
    /// Tab top above the body underside.
    pub overall_height: f64,
    pub hole_diameter: f64,
    /// Tab top to the hole centre.
    pub hole_inset: f64,
}

pub fn to220_features(package: &Package) -> To220Features {
    let d = &package.dimensions;
    let t = &defaults::TO220;
    let body_h = defaults::stated(&d.body_height).unwrap_or(t.body_height);
    let overall = defaults::stated(&d.overall_height)
        .filter(|h| *h > body_h)
        .unwrap_or(t.overall_height.max(body_h + 4.0));
    let hole = value(&d.mounting_hole_diameter).unwrap_or(t.hole_diameter);
    let inset = value(&d.mounting_hole_inset).unwrap_or(t.hole_inset);
    To220Features {
        lead_width: value(&d.lead_width).unwrap_or(t.lead_width),
        lead_thickness: value(&d.lead_thickness).unwrap_or(t.lead_thickness),
        tail_length: value(&d.tail_length).unwrap_or(t.tail_length),
        tab_thickness: t.tab_thickness,
        overall_height: overall,
        hole_diameter: hole.min((overall - body_h) * 0.8),
        hole_inset: inset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to220() -> Package {
        serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: to220\n\
             lead_count: 3\npitch: {min: 2.42, max: 2.66, unit: mm}\ndimensions:\n\
             \x20 body_width: {min: 9.66, max: 10.66, unit: mm}\n\
             \x20 body_length: {min: 4.4, max: 4.6, unit: mm}\n\
             \x20 body_height: {min: 8.6, max: 9.4, unit: mm}\n",
        )
        .unwrap()
    }

    #[test]
    fn three_leads_in_a_row_with_the_body_behind() {
        let g = generate_to220(&to220()).unwrap();
        let x: Vec<f64> = g.leads.iter().map(|l| l.position.x).collect();
        assert!((x[0] + 2.54).abs() < 1e-9 && x[1].abs() < 1e-9 && (x[2] - 2.54).abs() < 1e-9);
        assert_eq!(g.leads[0].number, "1");
        assert!(g
            .leads
            .iter()
            .all(|l| l.drill == Some(1.1) && l.mounting == Mounting::ThroughHole));
        assert!((g.body.position.y - 0.95).abs() < 1e-9);
        assert!((g.body.size.z - 9.0).abs() < 1e-9);
    }

    #[test]
    fn too_many_leads_for_the_body_is_an_error() {
        let mut p = to220();
        p.lead_count = 5;
        assert!(
            generate_to220(&p).is_err(),
            "5 x 2.54 does not fit a 10.16 body"
        );
        p.pitch.min = Some(1.6);
        p.pitch.max = Some(1.8);
        assert!(generate_to220(&p).is_ok());
    }
}
