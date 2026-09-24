use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::Package;

/// Drill diameter and pad size, both taken directly from a real KiCad
/// reference footprint (`Package_DIP.pretty/DIP-8_W7.62mm.kicad_mod`,
/// `KiCad/kicad-footprints`): `(pad ... (size 1.6 1.6) (drill 0.8) ...)`
/// -- not invented.
const DEFAULT_DRILL_DIAMETER_MM: f64 = 0.8;
const DEFAULT_PAD_DIAMETER_MM: f64 = 1.6;
const DEFAULT_LEAD_HEIGHT_MM: f64 = 3.0;
/// Generator default when the datasheet doesn't supply a body height --
/// same policy as every other family's own defaults, not a fact about
/// any specific part.
const DEFAULT_BODY_HEIGHT_MM: f64 = 3.3;

/// Generates Mechanical Geometry for a DIP-style through-hole package:
/// a rectangular body with `lead_count / 2` leads evenly spaced at
/// `pitch` on each of the 2 long sides -- structurally identical to
/// [`crate::generate_soic`]'s left-edge/right-edge layout and exact
/// numbering convention (DIP is where SOIC's own JEDEC numbering
/// originated: pin 1 at the top of the left edge, down the left edge,
/// then up the right edge), differing only in `Mounting`: every lead
/// here is `ThroughHole` with a drill, confirmed against a real KiCad
/// DIP-8 reference footprint (`DIP-8_W7.62mm.kicad_mod`), which also
/// confirms the same pin1-squared/rest-round polarity convention this
/// project's other THT families already rely on
/// (`openparts_pcbcad::build_footprint`).
pub fn generate_dip(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if !package.family.eq_ignore_ascii_case("dip") {
        return Err(McadError::UnsupportedFamily(package.family.clone()));
    }
    if let Some(geometry) = &package.geometry {
        if let Some(generator) = &geometry.generator {
            if !generator.eq_ignore_ascii_case("dip") {
                return Err(McadError::UnsupportedGenerator(generator.clone()));
            }
        }
    }

    let lead_count = package.lead_count;
    if lead_count == 0 || !lead_count.is_multiple_of(2) {
        return Err(McadError::InvalidLeadCount(lead_count));
    }
    let pins_per_side = lead_count / 2;

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

    let span = (pins_per_side as f64 - 1.0) * pitch;
    let half_span = span / 2.0;

    let pad_size = Size3 {
        x: DEFAULT_PAD_DIAMETER_MM,
        y: DEFAULT_PAD_DIAMETER_MM,
        z: DEFAULT_LEAD_HEIGHT_MM,
    };

    let mut leads = Vec::with_capacity(lead_count as usize);
    let mut pin_num = 1u32;

    // Left edge: x = -body_w/2, y from +half_span down to -half_span.
    for i in 0..pins_per_side {
        let y = half_span - i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x: -body_w / 2.0,
                y,
                z: 0.0,
            },
            size: pad_size,
            mounting: Mounting::ThroughHole,
            drill: Some(DEFAULT_DRILL_DIAMETER_MM),
        });
        pin_num += 1;
    }
    // Right edge: x = +body_w/2, y from -half_span up to +half_span --
    // numbering continues directly from the left edge.
    for i in 0..pins_per_side {
        let y = -half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x: body_w / 2.0,
                y,
                z: 0.0,
            },
            size: pad_size,
            mounting: Mounting::ThroughHole,
            drill: Some(DEFAULT_DRILL_DIAMETER_MM),
        });
        pin_num += 1;
    }

    let markers = vec![Marker {
        kind: MarkerKind::Pin1Dot,
        position: Point3 {
            x: -body_w / 2.0 + 0.5,
            y: body_l / 2.0 - 0.5,
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

    fn dip8() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/DIP8-TEST"),
            family: "dip".into(),
            lead_count: 8,
            pitch: Dimension {
                nominal: Some(2.54),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(7.62),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(9.4),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_height: None,
                exposed_pad: None,
            },
            lead_layout: None,
            geometry: None,
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn generates_expected_lead_count_and_numbering() {
        let geometry = generate_dip(&dip8()).unwrap();
        assert_eq!(geometry.leads.len(), 8);
        let numbers: Vec<&str> = geometry.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, vec!["1", "2", "3", "4", "5", "6", "7", "8"]);
    }

    #[test]
    fn every_lead_is_through_hole_with_a_drill() {
        let geometry = generate_dip(&dip8()).unwrap();
        for lead in &geometry.leads {
            assert_eq!(lead.mounting, Mounting::ThroughHole);
            assert_eq!(lead.drill, Some(DEFAULT_DRILL_DIAMETER_MM));
        }
    }

    #[test]
    fn pin_1_is_on_the_left_edge_top() {
        let geometry = generate_dip(&dip8()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        // 4 pins/side, pitch 2.54 -> half_span = 3.81.
        assert!((pin1.position.x - (-7.62 / 2.0)).abs() < 1e-9);
        assert!((pin1.position.y - 3.81).abs() < 1e-9);
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = dip8();
        package.dimensions.body_width.nominal = None;
        let err = generate_dip(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }

    #[test]
    fn rejects_non_dip_family() {
        let mut package = dip8();
        package.family = "soic".into();
        let err = generate_dip(&package).unwrap_err();
        assert!(matches!(err, McadError::UnsupportedFamily(_)));
    }

    #[test]
    fn rejects_odd_lead_count() {
        let mut package = dip8();
        package.lead_count = 7;
        let err = generate_dip(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadCount(7)));
    }
}
