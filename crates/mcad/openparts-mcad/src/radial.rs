use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::Package;

/// Engineering defaults used only when the datasheet doesn't supply a
/// value -- generator implementation details for a geometric
/// approximation, never written back into Canonical Data as a fact
/// (same policy as this crate's other generators' defaults).
///
/// Electrolytic can heights vary hugely by capacitance/voltage (a few
/// mm to 30+ mm); this is a rough mid-range fallback, not a
/// representative "typical" value.
const DEFAULT_BODY_HEIGHT_MM: f64 = 11.0;
/// Drill diameter and pad size, both taken directly from a real
/// official KiCad footprint (`Capacitor_THT.pretty/
/// CP_Radial_D5.0mm_P2.00mm.kicad_mod`, `KiCad/kicad-footprints`):
/// `(pad ... (size 1.6 1.6) (drill 0.8) ...)` -- not invented.
const DEFAULT_DRILL_DIAMETER_MM: f64 = 0.8;
const DEFAULT_PAD_DIAMETER_MM: f64 = 1.6;
const DEFAULT_LEAD_HEIGHT_MM: f64 = 3.0;

/// Generates Mechanical Geometry for a radial electrolytic capacitor:
/// a cylindrical can (`BodyShape::Cylinder`, not an axis-aligned box --
/// see `MechanicalGeometry::Body`'s docs for why the exact shape is
/// kept in the shared geometry model rather than pre-approximated)
/// with 2 through-hole leads spaced `pitch` apart at the can's base.
///
/// Numbering/polarity convention: pin 1 (positive, on the left) and
/// pin 2 (negative, on the right), per IPC-7351B -- confirmed directly
/// against the same real KiCad reference footprint's own pad order
/// (`(pad 1 thru_hole rect (at 0 0) ...) (pad 2 thru_hole circle (at 2
/// 0) ...)`). That reference file is also where the pin1-squared/
/// pin2-round pad-shape polarity convention this generator relies on
/// comes from -- applied generically in
/// `openparts_pcbcad::build_footprint` (first lead of a through-hole
/// part is squared), not hardcoded here.
pub fn generate_radial(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if !package.family.eq_ignore_ascii_case("radial") {
        return Err(McadError::UnsupportedFamily(package.family.clone()));
    }
    if let Some(geometry) = &package.geometry {
        if let Some(generator) = &geometry.generator {
            if !generator.eq_ignore_ascii_case("radial") {
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
    // Both represent the can's diameter -- a true radial can has no
    // separate width/length, but the schema has no dedicated
    // "diameter" field, so this generator expects (without enforcing)
    // that the two agree.
    let diameter = package
        .dimensions
        .body_width
        .nominal
        .ok_or(McadError::MissingDimension("dimensions.body_width.nominal"))?;
    let body_h = package
        .dimensions
        .body_height
        .as_ref()
        .and_then(|d| d.nominal)
        .unwrap_or(DEFAULT_BODY_HEIGHT_MM);

    let body = Body {
        position: Point3 {
            x: 0.0,
            y: 0.0,
            z: body_h / 2.0,
        },
        size: Size3 {
            x: diameter,
            y: diameter,
            z: body_h,
        },
        shape: BodyShape::Cylinder,
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

    let markers = vec![Marker {
        kind: MarkerKind::NegativeStripe,
        position: Point3 {
            x: pitch / 2.0,
            y: 0.0,
            z: body_h,
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

    fn radial_d5() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/RADIAL-D5.0-P2.00-TEST"),
            family: "radial".into(),
            lead_count: 2,
            pitch: Dimension {
                nominal: Some(2.0),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(5.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(5.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_height: Some(Dimension {
                    nominal: Some(11.0),
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
        let geometry = generate_radial(&radial_d5()).unwrap();
        assert_eq!(geometry.leads.len(), 2);
        for lead in &geometry.leads {
            assert_eq!(lead.mounting, Mounting::ThroughHole);
            assert_eq!(lead.drill, Some(DEFAULT_DRILL_DIAMETER_MM));
        }
    }

    #[test]
    fn pin_1_is_positive_on_the_left() {
        let geometry = generate_radial(&radial_d5()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        let pin2 = geometry.leads.iter().find(|l| l.number == "2").unwrap();
        assert!(pin1.position.x < 0.0);
        assert!(pin2.position.x > 0.0);
        assert!((pin1.position.x + pin2.position.x).abs() < 1e-9);
        assert!((pin2.position.x - pin1.position.x - 2.0).abs() < 1e-9); // pitch
    }

    #[test]
    fn body_is_a_cylinder_not_a_box() {
        let geometry = generate_radial(&radial_d5()).unwrap();
        assert_eq!(geometry.body.shape, BodyShape::Cylinder);
        assert_eq!(geometry.body.size.x, geometry.body.size.y); // diameter
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = radial_d5();
        package.dimensions.body_width.nominal = None;
        let err = generate_radial(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }

    #[test]
    fn rejects_non_radial_family() {
        let mut package = radial_d5();
        package.family = "chip".into();
        let err = generate_radial(&package).unwrap_err();
        assert!(matches!(err, McadError::UnsupportedFamily(_)));
    }

    #[test]
    fn rejects_wrong_lead_count() {
        let mut package = radial_d5();
        package.lead_count = 3;
        let err = generate_radial(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadCount(3)));
    }
}
