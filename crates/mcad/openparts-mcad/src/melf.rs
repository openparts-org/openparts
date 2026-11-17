//! `melf`: MELF two-terminal parts (MicroMELF, MiniMELF / SOD-80,
//! MELF): a cylindrical body lying along X with a metal end cap at each
//! end. The footprint is the source's land pattern (`pads`, required).
//!
//! Frame: top view, origin at the body's centre, axis along X, terminal
//! `"1"` at -X and `"2"` at +X (a diode's cathode band where the device
//! names it; the footprint takes the pads' own numbers and places).
//! `body_length` is the length along the axis (end cap to end cap),
//! `body_width` the diameter; `lead_length` is an end cap's length (else
//! a generator default share of the body).

use crate::defaults::{self, melf as dm};
use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::{Package, PackageFamily as F};

pub fn generate_melf(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if package.family != F::Melf {
        return Err(McadError::UnsupportedFamily(package.family));
    }
    if package.lead_count != 2 {
        return Err(McadError::InvalidLeadCount(package.lead_count));
    }
    if package.pads.is_empty() {
        return Err(McadError::MissingDimension("pads"));
    }
    let d = &package.dimensions;
    let length = defaults::dim(&d.body_length, "dimensions.body_length")?;
    let diameter = defaults::dim(&d.body_width, "dimensions.body_width")?;
    // An end cap is printed as a lone max as often as not: draw it there.
    let cap = defaults::stated(&d.lead_length)
        .unwrap_or(length * dm::CAP_RATIO)
        .min(length * dm::MAX_CAP_RATIO);
    if cap <= 0.0 {
        return Err(McadError::InconsistentDimensions(
            "dimensions.lead_length (the end cap) must be positive".into(),
        ));
    }
    let terminal = |number: &str, sign: f64| Lead {
        number: number.to_string(),
        position: Point3 {
            x: sign * (length - cap) / 2.0,
            y: 0.0,
            z: diameter / 2.0,
        },
        size: Size3 {
            x: cap,
            y: diameter,
            z: diameter,
        },
        mounting: Mounting::Smd,
        drill: None,
        slot: None,
        shape: crate::LandShape::Rect,
        side: openparts_core::PadSide::Top,
    };
    Ok(MechanicalGeometry {
        family: package.family,
        pitch: defaults::required(&package.pitch),
        details: crate::Details::None,
        lands: vec![],
        body: Body {
            position: Point3 {
                x: 0.0,
                y: 0.0,
                z: diameter / 2.0,
            },
            size: Size3 {
                x: length,
                y: diameter,
                z: diameter,
            },
            shape: BodyShape::CylinderX,
        },
        leads: vec![terminal("1", -1.0), terminal("2", 1.0)],
        markers: vec![],
        lead_form: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::{generate, BodyShape, McadError};

    /// Vishay MiniMELF (SOD-80), LL4148 datasheet rev 2.2: 3.3..3.7 long,
    /// ø1.4..1.6, end cap 0.47 max, lands 1.25 x 2 with a 2.5 gap.
    pub(crate) fn mini_melf() -> openparts_core::Package {
        serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: melf\n\
             lead_count: 2\npitch: {unit: mm}\ndimensions:\n\
             \x20 body_length: {min: 3.3, max: 3.7, unit: mm}\n\
             \x20 body_width: {min: 1.4, max: 1.6, unit: mm}\n\
             \x20 lead_length: {max: 0.47, unit: mm}\n\
             pads:\n\
             \x20 - {number: '1', x: -1.875, y: 0, width: 1.25, length: 2.0}\n\
             \x20 - {number: '2', x: 1.875, y: 0, width: 1.25, length: 2.0}\n",
        )
        .unwrap()
    }

    #[test]
    fn a_melf_is_a_cylinder_along_x_with_end_caps_on_its_lands() {
        let g = generate(&mini_melf()).unwrap();
        assert_eq!(g.body.shape, BodyShape::CylinderX);
        assert!((g.body.size.x - 3.5).abs() < 1e-9);
        assert!((g.body.size.y - 1.5).abs() < 1e-9 && (g.body.size.z - 1.5).abs() < 1e-9);
        assert!((g.body.position.z - 0.75).abs() < 1e-9, "lies on the board");
        // End caps drawn at the lone max, at the body's ends.
        let cap = &g.leads[0];
        assert!((cap.size.x - 0.47).abs() < 1e-9);
        assert!((cap.position.x + (3.5 - 0.47) / 2.0).abs() < 1e-9);
        // The footprint is the stated land pattern.
        assert_eq!(g.lands.len(), 2);
        assert!((g.lands[1].position.x - 1.875).abs() < 1e-9);
        assert!((g.lands[1].size.y - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_melf_needs_its_lands_two_terminals_and_a_body() {
        let mut p = mini_melf();
        p.pads.clear();
        assert!(matches!(
            generate(&p),
            Err(McadError::MissingDimension("pads"))
        ));
        let mut p = mini_melf();
        p.lead_count = 3;
        assert!(generate(&p).is_err());
        let mut p = mini_melf();
        p.dimensions.body_width.min = None;
        p.dimensions.body_width.max = None;
        assert!(generate(&p).is_err());
        // Without a stated cap: the default share of the body.
        let mut p = mini_melf();
        p.dimensions.lead_length = None;
        let g = generate(&p).unwrap();
        assert!((g.leads[0].size.x - 3.5 * crate::defaults::melf::CAP_RATIO).abs() < 1e-9);
    }
}
