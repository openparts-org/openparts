use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::Package;

/// Engineering defaults used only when the datasheet doesn't supply a
/// value -- same policy as every other generator's own defaults, not a
/// fact about any specific part.
const DEFAULT_BODY_HEIGHT_MM: f64 = 4.6;
/// Drill diameter and pad size, both taken directly from a real KiCad
/// reference footprint (`Package_TO_SOT_THT.pretty/TO-92.kicad_mod`,
/// `KiCad/kicad-footprints`): `(pad ... (size 1.3 1.3) (drill 0.75) ...)`
/// -- not invented.
const DEFAULT_DRILL_DIAMETER_MM: f64 = 0.75;
const DEFAULT_PAD_DIAMETER_MM: f64 = 1.3;
const DEFAULT_LEAD_HEIGHT_MM: f64 = 3.0;

/// Generates Mechanical Geometry for a TO-92-style through-hole
/// transistor package: a box-approximated body (this generator's own
/// "not photorealistic" simplification of TO-92's real D-shaped/
/// part-cylindrical body, matching the box-body convention every other
/// family already uses) with exactly 3 through-hole leads evenly
/// spaced by `pitch` along one edge -- but with the *middle* lead
/// recessed by `pitch` toward the body's back edge, not sitting on the
/// same front line as the two outer leads.
///
/// This staggered-middle-pin layout is a real, standard TO-92 mechanical
/// feature, confirmed directly from the same real KiCad reference
/// footprint above: pin 1 at `(0, 0)`, pin 2 at `(1.27, -1.27)`, pin 3
/// at `(2.54, 0)` -- i.e. the recess distance equals the pitch itself
/// in that reference (1.27mm each). This generator reuses that same
/// 1:1 ratio as its own default, since no dataset-wide source gives a
/// different one; not claimed as a universal TO-92 fact.
///
/// Numbering convention (matches the real reference footprint's own pin
/// order, and `sot.rs`'s left-to-right precedent): pin 1 on the left
/// (squared pad, per `openparts_pcbcad::build_footprint`'s existing
/// pin1-vs-rest rule), pin 2 the recessed middle pin, pin 3 on the
/// right.
pub fn generate_to92(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if !package.family.eq_ignore_ascii_case("to92") {
        return Err(McadError::UnsupportedFamily(package.family.clone()));
    }
    if let Some(geometry) = &package.geometry {
        if let Some(generator) = &geometry.generator {
            if !generator.eq_ignore_ascii_case("to92") {
                return Err(McadError::UnsupportedGenerator(generator.clone()));
            }
        }
    }
    if package.lead_count != 3 {
        return Err(McadError::InvalidLeadCount(package.lead_count));
    }

    let pitch = package
        .pitch
        .nominal
        .ok_or(McadError::MissingDimension("pitch.nominal"))?;
    let body_w = package
        .dimensions
        .body_width
        .nominal
        .ok_or(McadError::MissingDimension("dimensions.body_width.nominal"))?;
    let body_l = package
        .dimensions
        .body_length
        .nominal
        .ok_or(McadError::MissingDimension(
            "dimensions.body_length.nominal",
        ))?;
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
            x: body_w,
            y: body_l,
            z: body_h,
        },
        shape: BodyShape::Box,
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
                x: -pitch,
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
                x: 0.0,
                y: -pitch,
                z: 0.0,
            },
            size: pad_size,
            mounting: Mounting::ThroughHole,
            drill: Some(DEFAULT_DRILL_DIAMETER_MM),
        },
        Lead {
            number: "3".to_string(),
            position: Point3 {
                x: pitch,
                y: 0.0,
                z: 0.0,
            },
            size: pad_size,
            mounting: Mounting::ThroughHole,
            drill: Some(DEFAULT_DRILL_DIAMETER_MM),
        },
    ];

    Ok(MechanicalGeometry {
        body,
        leads,
        markers: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Dimension, Kind, Package, PackageDimensions};
    use std::collections::BTreeMap;

    fn to92_test() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/TO92-TEST"),
            family: "to92".into(),
            lead_count: 3,
            pitch: Dimension {
                nominal: Some(1.27),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(4.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(4.5),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_height: Some(Dimension {
                    nominal: Some(4.6),
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
    fn generates_three_through_hole_leads() {
        let geometry = generate_to92(&to92_test()).unwrap();
        assert_eq!(geometry.leads.len(), 3);
        for lead in &geometry.leads {
            assert_eq!(lead.mounting, Mounting::ThroughHole);
            assert_eq!(lead.drill, Some(DEFAULT_DRILL_DIAMETER_MM));
        }
    }

    #[test]
    fn middle_lead_is_recessed_and_outer_leads_are_on_the_front_line() {
        let geometry = generate_to92(&to92_test()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        let pin2 = geometry.leads.iter().find(|l| l.number == "2").unwrap();
        let pin3 = geometry.leads.iter().find(|l| l.number == "3").unwrap();
        assert_eq!(pin1.position.y, 0.0);
        assert_eq!(pin3.position.y, 0.0);
        assert!(pin2.position.y < 0.0);
        assert!(pin1.position.x < pin3.position.x);
        assert_eq!(pin2.position.x, 0.0);
    }

    #[test]
    fn leads_are_evenly_spaced_by_pitch() {
        let geometry = generate_to92(&to92_test()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        let pin3 = geometry.leads.iter().find(|l| l.number == "3").unwrap();
        assert!((pin3.position.x - pin1.position.x - 2.0 * 1.27).abs() < 1e-9);
    }

    #[test]
    fn body_is_a_box_not_a_cylinder() {
        let geometry = generate_to92(&to92_test()).unwrap();
        assert_eq!(geometry.body.shape, BodyShape::Box);
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = to92_test();
        package.dimensions.body_width.nominal = None;
        let err = generate_to92(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }

    #[test]
    fn rejects_non_to92_family() {
        let mut package = to92_test();
        package.family = "sot".into();
        let err = generate_to92(&package).unwrap_err();
        assert!(matches!(err, McadError::UnsupportedFamily(_)));
    }

    #[test]
    fn rejects_wrong_lead_count() {
        let mut package = to92_test();
        package.lead_count = 2;
        let err = generate_to92(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadCount(2)));
    }
}
