use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::Package;

/// Drill diameter and pad size, both taken directly from a real KiCad
/// reference footprint (`Diode_THT.pretty/
/// D_DO-41_SOD81_P10.16mm_Horizontal.kicad_mod`, `KiCad/kicad-footprints`):
/// `(pad ... (size 2.2 2.2) (drill 1.1) ...)` -- not invented.
const DEFAULT_DRILL_DIAMETER_MM: f64 = 1.1;
const DEFAULT_PAD_DIAMETER_MM: f64 = 2.2;
const DEFAULT_LEAD_HEIGHT_MM: f64 = 3.0;

/// Generates Mechanical Geometry for an axial-leaded through-hole part
/// (the classic DO-41-style diode shape): a cylindrical can
/// (`BodyShape::CylinderX` -- lying horizontally, axis along X, unlike
/// `radial`'s vertical can) with 2 through-hole leads exiting from
/// *opposite ends* of the can along its axis, each extending out to
/// `pitch`/2 before entering the board -- a different lead topology
/// from `radial` (whose 2 leads both exit the same, bottom, end).
///
/// Numbering/polarity convention: pin 1 = cathode, pin 2 = anode --
/// confirmed directly against the same real KiCad reference footprint
/// above, whose pin 1 (a squared `rect` pad) sits next to its own "K"
/// (kathode) silkscreen/fab marking, and whose pin 2 (an `oval` pad --
/// this crate's pad-shape vocabulary only has `Rect`/`Circle`, so
/// `openparts_pcbcad::build_footprint`'s existing "first lead of a
/// through-hole part is squared, the rest round" rule already produces
/// the right pin-1-vs-rest distinction without needing an oval shape)
/// is the anode. This is the reverse of `radial`'s pin1=positive
/// convention -- a real, deliberate difference between the two real
/// reference footprints, not an inconsistency to "fix".
pub fn generate_axial(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if !package.family.eq_ignore_ascii_case("axial") {
        return Err(McadError::UnsupportedFamily(package.family.clone()));
    }
    if let Some(geometry) = &package.geometry {
        if let Some(generator) = &geometry.generator {
            if !generator.eq_ignore_ascii_case("axial") {
                return Err(McadError::UnsupportedGenerator(generator.clone()));
            }
        }
    }
    if package.lead_count != 2 {
        return Err(McadError::InvalidLeadCount(package.lead_count));
    }

    let pitch = package
        .pitch
        .nominal
        .ok_or(McadError::MissingDimension("pitch.nominal"))?;
    // `body_width`/`body_length` both represent the can's diameter, same
    // convention `radial.rs` uses -- a true cylinder has no separate
    // width/length, but the schema has no dedicated "diameter" field.
    let diameter = package
        .dimensions
        .body_width
        .nominal
        .ok_or(McadError::MissingDimension("dimensions.body_width.nominal"))?;
    // `body_height` is reused here as the can's axial length (the
    // dimension along X, not a vertical height) -- same schema field,
    // different physical meaning depending on family, matching how
    // `radial.rs` already reuses `body_height` for its can's length.
    let length = package
        .dimensions
        .body_height
        .as_ref()
        .and_then(|d| d.nominal)
        .ok_or(McadError::MissingDimension(
            "dimensions.body_height.nominal",
        ))?;

    let body = Body {
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
    };

    let pad_size = Size3 {
        x: DEFAULT_PAD_DIAMETER_MM,
        y: DEFAULT_PAD_DIAMETER_MM,
        z: DEFAULT_LEAD_HEIGHT_MM,
    };
    let leads = vec![
        Lead {
            number: "1".to_string(),
            position: Point3 {
                x: -pitch / 2.0,
                y: 0.0,
                z: 0.0,
            },
            size: pad_size,
            mounting: Mounting::ThroughHole,
            drill: Some(DEFAULT_DRILL_DIAMETER_MM),
        },
        Lead {
            number: "2".to_string(),
            position: Point3 {
                x: pitch / 2.0,
                y: 0.0,
                z: 0.0,
            },
            size: pad_size,
            mounting: Mounting::ThroughHole,
            drill: Some(DEFAULT_DRILL_DIAMETER_MM),
        },
    ];

    // Cathode (pin 1) band, at the left end of the can.
    let markers = vec![Marker {
        kind: MarkerKind::NegativeStripe,
        position: Point3 {
            x: -length / 2.0,
            y: 0.0,
            z: diameter / 2.0,
        },
    }];

    Ok(MechanicalGeometry {
        body,
        leads,
        markers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Dimension, Kind, Package, PackageDimensions};
    use std::collections::BTreeMap;

    fn do41_test() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/AXIAL-DO41-TEST"),
            family: "axial".into(),
            lead_count: 2,
            pitch: Dimension {
                nominal: Some(10.16),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(2.7),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(2.7),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_height: Some(Dimension {
                    nominal: Some(5.2),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                }),
                exposed_pad: None,
            },
            lead_layout: None,
            geometry: None,
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn generates_two_through_hole_leads() {
        let geometry = generate_axial(&do41_test()).unwrap();
        assert_eq!(geometry.leads.len(), 2);
        for lead in &geometry.leads {
            assert_eq!(lead.mounting, Mounting::ThroughHole);
            assert_eq!(lead.drill, Some(DEFAULT_DRILL_DIAMETER_MM));
        }
    }

    #[test]
    fn pin_1_is_cathode_on_the_left() {
        let geometry = generate_axial(&do41_test()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        let pin2 = geometry.leads.iter().find(|l| l.number == "2").unwrap();
        assert!(pin1.position.x < 0.0);
        assert!(pin2.position.x > 0.0);
        assert!((pin2.position.x - pin1.position.x - 10.16).abs() < 1e-9);
    }

    #[test]
    fn body_is_a_horizontal_cylinder_not_a_box_or_vertical_can() {
        let geometry = generate_axial(&do41_test()).unwrap();
        assert_eq!(geometry.body.shape, BodyShape::CylinderX);
        // y == z (diameter), x (length) independent -- opposite axis
        // pairing from `BodyShape::Cylinder`.
        assert_eq!(geometry.body.size.y, geometry.body.size.z);
        assert_ne!(geometry.body.size.x, geometry.body.size.y);
    }

    #[test]
    fn leads_extend_past_the_body_on_each_end() {
        // Real axial parts have straight lead wire between the can and
        // where the lead enters the board -- pitch should be larger
        // than the can's own length, matching the real reference
        // footprint (body 5.2mm, pitch 10.16mm).
        let geometry = generate_axial(&do41_test()).unwrap();
        assert!(10.16 > geometry.body.size.x);
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = do41_test();
        package.dimensions.body_width.nominal = None;
        let err = generate_axial(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }

    #[test]
    fn rejects_non_axial_family() {
        let mut package = do41_test();
        package.family = "radial".into();
        let err = generate_axial(&package).unwrap_err();
        assert!(matches!(err, McadError::UnsupportedFamily(_)));
    }

    #[test]
    fn rejects_wrong_lead_count() {
        let mut package = do41_test();
        package.lead_count = 3;
        let err = generate_axial(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadCount(3)));
    }
}
