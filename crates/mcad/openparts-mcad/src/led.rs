//! `smd_led`: top-view SMD LEDs with their terminals on two opposite
//! sides (5050/3535 PLCC, 2020 chip-scale), addressable RGB LEDs among
//! them. The footprint is the source's recommended land pattern
//! (`Package::pads`, required); the frame is the top view with the body
//! outline `body_width` x `body_length` centred on the origin.
//!
//! The 3D terminals are the leads the outline states -- `overall_width`
//! (terminal tip to tip along X), `lead_width` and `lead_length` -- one
//! at each pad's Y on the pad's side; without them, the pads themselves.
//! The body height is `body_height`, else `overall_height` less the
//! standoff.

use crate::defaults::{self, value};
use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::{Package, PackageFamily as F};

pub fn generate_smd_led(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if package.family != F::SmdLed {
        return Err(McadError::UnsupportedFamily(package.family));
    }
    if package.pads.is_empty() {
        return Err(McadError::MissingDimension("pads"));
    }
    let plated = package.pads.iter().filter(|p| p.plated).count();
    if plated != package.lead_count as usize {
        return Err(McadError::InvalidLeadCount(package.lead_count));
    }
    let d = &package.dimensions;
    let body_w = defaults::dim(&d.body_width, "dimensions.body_width")?;
    let body_l = defaults::dim(&d.body_length, "dimensions.body_length")?;
    let standoff = value(&d.standoff).unwrap_or(0.0);
    let body_h = defaults::stated(&d.body_height)
        .or_else(|| defaults::stated(&d.overall_height).map(|h| h - standoff))
        .filter(|h| *h > 0.0)
        .ok_or(McadError::MissingDimension(
            "dimensions.body_height or overall_height",
        ))?;
    let lands = crate::land::from_pads(package)?;

    // Terminals from the outline when it states them.
    let terminal = match (
        value(&d.overall_width),
        value(&d.lead_width),
        value(&d.lead_length),
    ) {
        (Some(span), Some(w), Some(len)) if span > 0.0 && w > 0.0 && len > 0.0 => {
            Some((span, w, len))
        }
        _ => None,
    };
    let lead_t = (body_h * 0.15).clamp(0.05, 0.3);
    let leads: Vec<Lead> = match terminal {
        Some((span, w, len)) => lands
            .iter()
            .filter(|l| l.mounting == Mounting::Smd)
            .map(|l| {
                let side = if l.position.x < 0.0 { -1.0 } else { 1.0 };
                Lead {
                    number: l.number.clone(),
                    position: Point3 {
                        x: side * (span / 2.0 - len / 2.0),
                        y: l.position.y,
                        z: lead_t / 2.0,
                    },
                    size: Size3 {
                        x: len,
                        y: w,
                        z: lead_t,
                    },
                    mounting: Mounting::Smd,
                    drill: None,
                    slot: None,
                    shape: crate::LandShape::Rect,
                    side: openparts_core::PadSide::Top,
                }
            })
            .collect(),
        None => lands.clone(),
    };

    Ok(MechanicalGeometry {
        family: package.family,
        pitch: defaults::required(&package.pitch),
        details: crate::Details::None,
        lands,
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
    })
}

#[cfg(test)]
mod tests {
    use crate::{generate, LandShape};
    use openparts_core::Package;

    fn pkg(yaml: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: smd_led\n{yaml}"
        ))
        .unwrap()
    }

    const LED_5050: &str = "lead_count: 4\npitch: {nominal: 3.3, unit: mm}\n\
        dimensions:\n\
        \x20 body_width: {nominal: 5.0, unit: mm}\n\
        \x20 body_length: {nominal: 5.0, unit: mm}\n\
        \x20 overall_height: {nominal: 1.57, unit: mm}\n\
        pads:\n\
        \x20 - {number: '1', x: -2.45, y: 1.65, width: 1.5, length: 1.0}\n\
        \x20 - {number: '2', x: -2.45, y: -1.65, width: 1.5, length: 1.0}\n\
        \x20 - {number: '3', x: 2.45, y: -1.65, width: 1.5, length: 1.0}\n\
        \x20 - {number: '4', x: 2.45, y: 1.65, width: 1.5, length: 1.0}\n";

    #[test]
    fn the_stated_pads_are_the_lands_and_without_lead_dimensions_the_leads() {
        let g = generate(&pkg(LED_5050)).unwrap();
        assert_eq!(g.lands.len(), 4);
        assert_eq!(g.leads.len(), 4);
        assert!((g.body.size.z - 1.57).abs() < 1e-9);
        assert!((g.body.position.z - 0.785).abs() < 1e-9);
        assert!(g.lands.iter().all(|l| l.shape == LandShape::Rect));
    }

    #[test]
    fn stated_terminals_are_drawn_at_the_outline() {
        let yaml = LED_5050.replace(
            "  overall_height:",
            "  overall_width: {nominal: 5.4, unit: mm}\n  lead_width: {nominal: 0.9, unit: mm}\n  \
             lead_length: {nominal: 0.9, unit: mm}\n  overall_height:",
        );
        let g = generate(&pkg(&yaml)).unwrap();
        let l1 = g.leads.iter().find(|l| l.number == "1").unwrap();
        assert!((l1.position.x + 2.25).abs() < 1e-9);
        assert!((l1.position.y - 1.65).abs() < 1e-9);
        // The footprint keeps the source's land.
        let p1 = g.lands.iter().find(|l| l.number == "1").unwrap();
        assert!((p1.size.x - 1.5).abs() < 1e-9);
    }

    #[test]
    fn pads_and_a_height_are_required() {
        assert!(generate(&pkg("lead_count: 4\npitch: {unit: mm}\ndimensions:\n  body_width: {nominal: 5.0, unit: mm}\n  \
             body_length: {nominal: 5.0, unit: mm}\n  body_height: {nominal: 1.5, unit: mm}\n"))
        .is_err());
        let no_height = LED_5050.replace("  overall_height: {nominal: 1.57, unit: mm}\n", "");
        assert!(generate(&pkg(&no_height)).is_err());
    }
}
