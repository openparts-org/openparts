//! `smd_electrolytic`: SMD aluminium electrolytic capacitors ("V-chip").
//! A round can (`can_diameter`, øD) stands on a square insulating base
//! plate (`body_width` x `body_length`, the source's A x B), and two flat
//! terminals leave the can through the plate and are bent under it,
//! reaching past the plate edges. The footprint is the source's
//! recommended land pattern (`Package::pads`, required); the frame is the
//! top view with the plate centred on the origin and the terminals along
//! X (the + terminal on the left in the zero orientation, the placement
//! model's two-terminal row).
//!
//! The 3D terminals are the ones the outline states -- `overall_width`
//! (terminal tip to tip, the source's H; a lone max is drawn there), `lead_width` (W) and
//! `lead_length` (the terminal's length under the plate, I) -- one at
//! each pad's Y on the pad's side; without them, the pads themselves.
//! The height is `overall_height` (the can top above the board, L). The
//! plate thickness, the terminal thickness and the plate's corner
//! chamfers on the + side are generator defaults (3D only).

use crate::defaults::{self, value};
use crate::{
    Body, BodyShape, Details, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3,
};
use openparts_core::{Package, PackageFamily as F};

/// What the 3D model needs beyond the body envelope and the terminals:
/// the can and the plate (datasheet values, else `defaults`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElectrolyticFeatures {
    /// The can's diameter (øD, stated).
    pub can_diameter: f64,
    /// The base plate's thickness (generator default).
    pub plate_thickness: f64,
    /// The terminals' thickness under the plate: `lead_thickness`, else
    /// a generator default. The plate sits on them.
    pub terminal_thickness: f64,
    /// The 45° cut at the plate's two corners on the + side, along each
    /// edge (generator default; the source's drawing shows the cut, not
    /// its size).
    pub chamfer: f64,
}

pub fn generate_smd_electrolytic(package: &Package) -> Result<MechanicalGeometry, McadError> {
    use defaults::smd_electrolytic as de;
    if package.family != F::SmdElectrolytic {
        return Err(McadError::UnsupportedFamily(package.family));
    }
    if package.pads.is_empty() {
        return Err(McadError::MissingDimension("pads"));
    }
    let plated = package.pads.iter().filter(|p| p.plated).count();
    if plated != package.lead_count as usize || plated < 2 {
        return Err(McadError::InvalidLeadCount(package.lead_count));
    }
    let d = &package.dimensions;
    let plate_w = defaults::dim(&d.body_width, "dimensions.body_width")?;
    let plate_l = defaults::dim(&d.body_length, "dimensions.body_length")?;
    let height = defaults::stated(&d.overall_height)
        .ok_or(McadError::MissingDimension("dimensions.overall_height"))?;
    let can_d = defaults::stated(&d.can_diameter)
        .ok_or(McadError::MissingDimension("dimensions.can_diameter"))?;
    if can_d <= 0.0 || plate_w <= 0.0 || plate_l <= 0.0 {
        return Err(McadError::InconsistentDimensions(
            "can_diameter and the plate must be positive".into(),
        ));
    }
    let terminal_t = value(&d.lead_thickness).unwrap_or(de::TERMINAL_THICKNESS);
    let plate_t = (height * de::PLATE_THICKNESS_RATIO)
        .clamp(de::PLATE_THICKNESS_MIN, de::PLATE_THICKNESS_MAX);
    if height <= terminal_t + plate_t {
        return Err(McadError::InconsistentDimensions(format!(
            "overall_height {height} leaves no can above the base plate"
        )));
    }
    let lands = crate::land::from_pads(package)?;

    // Terminals from the outline when it states them.
    // Outline tables often give the tip-to-tip span only as a max (drawn
    // there, like every lone limit).
    let terminal = match (defaults::stated(&d.overall_width), value(&d.lead_width)) {
        (Some(span), Some(w)) if span > 0.0 && w > 0.0 => {
            let len = value(&d.lead_length).unwrap_or(plate_w * de::TERMINAL_LENGTH_RATIO);
            Some((span, w, len.min(span / 2.0)))
        }
        _ => None,
    };
    let leads: Vec<Lead> = lands
        .iter()
        .filter(|l| l.mounting == Mounting::Smd)
        .map(|l| match terminal {
            Some((span, w, len)) => {
                let side = if l.position.x < 0.0 { -1.0 } else { 1.0 };
                Lead {
                    number: l.number.clone(),
                    position: Point3 {
                        x: side * (span / 2.0 - len / 2.0),
                        y: l.position.y,
                        z: terminal_t / 2.0,
                    },
                    size: Size3 {
                        x: len,
                        y: w,
                        z: terminal_t,
                    },
                    mounting: Mounting::Smd,
                    drill: None,
                    slot: None,
                    shape: crate::LandShape::Rect,
                    side: openparts_core::PadSide::Top,
                }
            }
            None => Lead {
                position: Point3 {
                    z: terminal_t / 2.0,
                    ..l.position
                },
                size: Size3 {
                    z: terminal_t,
                    ..l.size
                },
                shape: crate::LandShape::Rect,
                ..l.clone()
            },
        })
        .collect();

    Ok(MechanicalGeometry {
        family: package.family,
        pitch: defaults::required(&package.pitch),
        details: Details::Electrolytic(ElectrolyticFeatures {
            can_diameter: can_d,
            plate_thickness: plate_t,
            terminal_thickness: terminal_t,
            chamfer: plate_w.min(plate_l) * de::CHAMFER_RATIO,
        }),
        lands,
        // The envelope: the plate's outline up to the can's top.
        body: Body {
            position: Point3 {
                x: 0.0,
                y: 0.0,
                z: height / 2.0,
            },
            size: Size3 {
                x: plate_w,
                y: plate_l,
                z: height,
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
    use crate::{generate, Details};
    use openparts_core::Package;

    fn pkg(yaml: &str) -> Package {
        serde_yaml::from_str(&format!(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: smd_electrolytic\n{yaml}"
        ))
        .unwrap()
    }

    // Panasonic FK size D (ø6.3 x 5.8): plate 6.6, lands a 1.8, b 3.2,
    // c 1.6 turned so the terminals lie along X.
    const SIZE_D: &str = "lead_count: 2\npitch: {unit: mm}\n\
        dimensions:\n\
        \x20 body_width: {nominal: 6.6, unit: mm}\n\
        \x20 body_length: {nominal: 6.6, unit: mm}\n\
        \x20 overall_height: {nominal: 5.8, unit: mm}\n\
        \x20 can_diameter: {nominal: 6.3, unit: mm}\n\
        pads:\n\
        \x20 - {number: '1', x: -2.5, y: 0, width: 3.2, length: 1.6}\n\
        \x20 - {number: '2', x: 2.5, y: 0, width: 3.2, length: 1.6}\n";

    #[test]
    fn the_plate_is_the_body_and_the_pads_the_lands() {
        let g = generate(&pkg(SIZE_D)).unwrap();
        assert_eq!(g.lands.len(), 2);
        assert_eq!(g.leads.len(), 2);
        assert!((g.body.size.x - 6.6).abs() < 1e-9);
        assert!((g.body.size.z - 5.8).abs() < 1e-9);
        let Details::Electrolytic(f) = g.details else {
            panic!("no electrolytic details")
        };
        assert!((f.can_diameter - 6.3).abs() < 1e-9);
        assert!(f.plate_thickness > 0.0 && f.plate_thickness < 1.0);
        assert!(f.chamfer > 0.0);
    }

    #[test]
    fn stated_terminals_are_drawn_at_the_outline() {
        let yaml = SIZE_D.replace(
            "  can_diameter:",
            "  overall_width: {max: 7.8, unit: mm}\n  lead_width: {nominal: 0.65, unit: mm}\n  \
             lead_length: {nominal: 2.6, unit: mm}\n  can_diameter:",
        );
        let g = generate(&pkg(&yaml)).unwrap();
        // A span stated only as a max is drawn at the max.
        let l1 = g.leads.iter().find(|l| l.number == "1").unwrap();
        assert!((l1.position.x + (3.9 - 1.3)).abs() < 1e-9);
        assert!((l1.size.x - 2.6).abs() < 1e-9);
        assert!((l1.size.y - 0.65).abs() < 1e-9);
        // The footprint keeps the source's land.
        let p1 = g.lands.iter().find(|l| l.number == "1").unwrap();
        assert!((p1.size.x - 3.2).abs() < 1e-9);
    }

    #[test]
    fn pads_a_height_and_the_can_are_required() {
        let no_pads = SIZE_D.split("pads:").next().unwrap().to_string();
        assert!(generate(&pkg(&no_pads)).is_err());
        let no_height = SIZE_D.replace("  overall_height: {nominal: 5.8, unit: mm}\n", "");
        assert!(generate(&pkg(&no_height)).is_err());
        let no_can = SIZE_D.replace("  can_diameter: {nominal: 6.3, unit: mm}\n", "");
        assert!(generate(&pkg(&no_can)).is_err());
        let one_pad = SIZE_D.replace("lead_count: 2", "lead_count: 3");
        assert!(generate(&pkg(&one_pad)).is_err());
    }
}
