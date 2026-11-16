//! Tactile switches: a square housing with a round actuator on top and
//! two or four terminals at its corners -- SMD gull-wing feet
//! (`tact_switch_smd`) or straight through-hole pins (`tact_switch_tht`).
//!
//! Frame: terminals on the left and right of the housing, `pitch` apart
//! along Y; positions numbered like SOIC-4 -- 1 top-left, 2 bottom-left,
//! 3 bottom-right, 4 top-right (two terminals: 1 left, 2 right, on the X
//! axis). Vendors number the corners differently: `pad_order` gives the
//! source's numbers in this position order. `body_width` is the housing
//! along X, `body_length` along Y, `body_height` its height;
//! `overall_height` the actuator top above the board and
//! `actuator_diameter` its diameter (3D only).
//!
//! - SMD: `overall_width` is the toe-to-toe span (X); each pad is the
//!   foot, `lead_length` in from the toe (else from the housing edge),
//!   `lead_width` wide.
//! - THT: `row_spacing` is the X distance between the pin columns,
//!   `hole_diameter` the drill, `pad_diameter` the land (else drill +
//!   2 x 0.4 mm); `standoff` lifts the housing (stand-off bumps).

use crate::defaults::{self, value};
use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::{Package, PackageFamily as F};

pub fn generate_tact_switch(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let tht = match package.family {
        F::TactSwitchSmd => false,
        F::TactSwitchTht => true,
        other => return Err(McadError::UnsupportedFamily(other)),
    };
    let n = package.lead_count;
    if n != 2 && n != 4 {
        return Err(McadError::InvalidLeadCount(n));
    }
    // Position numbers; `pad_order` is applied by `crate::generate`.
    let numbers: Vec<String> = (1..=n).map(|i| i.to_string()).collect();
    let d = &package.dimensions;
    let body_w = defaults::required(&d.body_width)
        .ok_or(McadError::MissingDimension("dimensions.body_width"))?;
    let body_l = defaults::required(&d.body_length)
        .ok_or(McadError::MissingDimension("dimensions.body_length"))?;
    let body_h = defaults::stated(&d.body_height)
        .ok_or(McadError::MissingDimension("dimensions.body_height"))?;
    let dy = if n == 4 {
        defaults::dim(&package.pitch, "pitch")? / 2.0
    } else {
        0.0
    };
    // Position order: top-left, bottom-left, bottom-right, top-right.
    let corners: Vec<(f64, f64)> = if n == 4 {
        vec![(-1.0, dy), (-1.0, -dy), (1.0, -dy), (1.0, dy)]
    } else {
        vec![(-1.0, 0.0), (1.0, 0.0)]
    };

    let (standoff, leads) = if tht {
        let span =
            value(&d.row_spacing).ok_or(McadError::MissingDimension("dimensions.row_spacing"))?;
        let drill = value(&d.hole_diameter)
            .ok_or(McadError::MissingDimension("dimensions.hole_diameter"))?;
        let land = value(&d.pad_diameter).unwrap_or(drill + 2.0 * crate::defaults::tact::THT_RING);
        let standoff = value(&d.standoff).unwrap_or(0.0);
        let leads = corners
            .iter()
            .zip(&numbers)
            .map(|(&(sx, y), number)| Lead {
                number: number.clone(),
                position: Point3 {
                    x: sx * span / 2.0,
                    y,
                    z: 0.0,
                },
                size: Size3 {
                    x: land,
                    y: land,
                    z: standoff,
                },
                mounting: Mounting::ThroughHole,
                drill: Some(drill),
                slot: None,
                shape: crate::LandShape::Rect,
            })
            .collect();
        (standoff, leads)
    } else {
        let span = value(&d.overall_width)
            .ok_or(McadError::MissingDimension("dimensions.overall_width"))?;
        if span <= body_w {
            return Err(McadError::InconsistentDimensions(format!(
                "overall_width ({span}) must exceed body_width ({body_w})"
            )));
        }
        let toe = span / 2.0;
        let foot = value(&d.lead_length).unwrap_or(toe - body_w / 2.0).min(toe);
        let width =
            value(&d.lead_width).ok_or(McadError::MissingDimension("dimensions.lead_width"))?;
        let t = value(&d.lead_thickness).unwrap_or(crate::defaults::tact::SMD_LEAD_T);
        let leads = corners
            .iter()
            .zip(&numbers)
            .map(|(&(sx, y), number)| Lead {
                number: number.clone(),
                position: Point3 {
                    x: sx * (toe - foot / 2.0),
                    y,
                    z: t / 2.0,
                },
                size: Size3 {
                    x: foot,
                    y: width,
                    z: t,
                },
                mounting: Mounting::Smd,
                drill: None,
                slot: None,
                shape: crate::LandShape::Rect,
            })
            .collect();
        (0.0, leads)
    };

    let mut geometry = MechanicalGeometry {
        family: package.family,
        lands: vec![],

        pitch: crate::defaults::required(&package.pitch),

        details: crate::Details::None,
        body: Body {
            position: Point3 {
                x: 0.0,
                y: 0.0,
                z: standoff + body_h / 2.0,
            },
            size: Size3 {
                x: body_w,
                y: body_l,
                z: body_h,
            },
            shape: BodyShape::Box,
        },
        leads,
        markers: vec![],
        lead_form: None,
    };
    geometry.details = crate::Details::Switch(switch_features(package, &geometry));
    Ok(geometry)
}

/// The actuator and lead sizes a switch's 3D model needs: the package's
/// own values, else drawing defaults (a centred plunger half the housing
/// wide rising 1 mm above it; 0.25 mm feet; THT pins 70 % of the drill,
/// 3 mm below the board).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchFeatures {
    pub actuator_diameter: f64,
    /// Actuator top above the board.
    pub actuator_top: f64,
    pub lead_thickness: f64,
    pub tail_length: f64,
}

pub fn switch_features(package: &Package, geometry: &MechanicalGeometry) -> SwitchFeatures {
    let d = &package.dimensions;
    let b = &geometry.body;
    let top = b.position.z + b.size.z / 2.0;
    SwitchFeatures {
        actuator_diameter: value(&d.actuator_diameter)
            .unwrap_or(b.size.x.min(b.size.y) * 0.5)
            .min(b.size.x.min(b.size.y)),
        actuator_top: defaults::stated(&d.overall_height)
            .filter(|h| *h > top)
            .unwrap_or(top + 1.0),
        lead_thickness: value(&d.lead_thickness).unwrap_or(crate::defaults::tact::SMD_LEAD_T),
        tail_length: value(&d.tail_length).unwrap_or(crate::defaults::tact::TAIL_LENGTH),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(yaml: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nlead_count: 4\n{yaml}"
        ))
        .unwrap()
    }

    fn tht() -> Package {
        pkg(
            "family: tact_switch_tht\npitch: {nominal: 4.5, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 6.0, unit: mm}\n\
             \x20 body_length: {nominal: 6.0, unit: mm}\n\
             \x20 body_height: {nominal: 3.5, unit: mm}\n\
             \x20 row_spacing: {nominal: 6.5, unit: mm}\n\
             \x20 hole_diameter: {nominal: 1.0, unit: mm}\n",
        )
    }

    #[test]
    fn tht_pins_sit_at_the_corners_in_soic_order() {
        let g = generate_tact_switch(&tht()).unwrap();
        let at: Vec<(f64, f64)> = g
            .leads
            .iter()
            .map(|l| (l.position.x, l.position.y))
            .collect();
        assert_eq!(
            at,
            [(-3.25, 2.25), (-3.25, -2.25), (3.25, -2.25), (3.25, 2.25)]
        );
        assert!(g
            .leads
            .iter()
            .all(|l| l.drill == Some(1.0) && (l.size.x - 1.8).abs() < 1e-9));
    }

    #[test]
    fn pad_order_renumbers_the_positions() {
        let mut p = tht();
        p.pad_order = vec!["1".into(), "3".into(), "4".into(), "2".into()];
        let g = crate::generate(&p).unwrap();
        let numbers: Vec<&str> = g.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, ["1", "3", "4", "2"]);
        p.pad_order.pop();
        assert!(crate::generate(&p).is_err());
    }

    #[test]
    fn smd_feet_end_at_the_overall_width() {
        let p = pkg(
            "family: tact_switch_smd\npitch: {nominal: 4.5, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 6.0, unit: mm}\n\
             \x20 body_length: {nominal: 6.0, unit: mm}\n\
             \x20 body_height: {nominal: 3.5, unit: mm}\n\
             \x20 overall_width: {nominal: 9.0, unit: mm}\n\
             \x20 lead_width: {nominal: 0.7, unit: mm}\n\
             \x20 lead_length: {nominal: 1.0, unit: mm}\n",
        );
        let g = crate::generate(&p).unwrap();
        let l4 = &g.leads[3];
        assert!((l4.position.x + l4.size.x / 2.0 - 4.5).abs() < 1e-9);
        assert!((l4.size.x - 1.0).abs() < 1e-9 && (l4.size.y - 0.7).abs() < 1e-9);
        assert_eq!(l4.mounting, Mounting::Smd);
    }
}
