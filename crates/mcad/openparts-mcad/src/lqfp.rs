use crate::{Body, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Point3, Size3};
use openparts_core::Package;

/// Engineering defaults used only when the datasheet doesn't supply a
/// value. These are generator implementation details for a geometric
/// approximation -- never written back into Canonical Data as a fact
/// (Canonical Data Specification section 7: never invent a nominal).
const DEFAULT_BODY_HEIGHT_MM: f64 = 1.4;
const DEFAULT_LEAD_WIDTH_MM: f64 = 0.22;
const DEFAULT_LEAD_PROTRUSION_MM: f64 = 0.6;
const DEFAULT_LEAD_HEIGHT_MM: f64 = 0.15;

/// Generates Mechanical Geometry for an LQFP/TQFP-style package: a
/// rectangular body with `lead_count / 4` gull-wing leads evenly spaced
/// at `pitch` on each of the 4 sides.
///
/// Numbering convention (matches common LQFP/TQFP datasheets): pin 1 at
/// the top of the LEFT edge, proceeding down the left edge, then
/// left-to-right along the bottom, then up the right edge, then
/// right-to-left along the top, ending just short of pin 1.
pub fn generate_lqfp(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if !package.family.eq_ignore_ascii_case("lqfp") {
        return Err(McadError::UnsupportedFamily(package.family.clone()));
    }
    if let Some(geometry) = &package.geometry {
        if let Some(generator) = &geometry.generator {
            if !generator.eq_ignore_ascii_case("lqfp") {
                return Err(McadError::UnsupportedGenerator(generator.clone()));
            }
        }
    }

    let lead_count = package.lead_count;
    if lead_count == 0 || !lead_count.is_multiple_of(4) {
        return Err(McadError::InvalidLeadCount(lead_count));
    }
    let pins_per_side = lead_count / 4;

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
    };

    let span = (pins_per_side as f64 - 1.0) * pitch;
    let half_span = span / 2.0;

    let lead_size_along = Size3 {
        x: DEFAULT_LEAD_PROTRUSION_MM,
        y: DEFAULT_LEAD_WIDTH_MM,
        z: DEFAULT_LEAD_HEIGHT_MM,
    };
    let lead_size_perp = Size3 {
        x: DEFAULT_LEAD_WIDTH_MM,
        y: DEFAULT_LEAD_PROTRUSION_MM,
        z: DEFAULT_LEAD_HEIGHT_MM,
    };

    let mut leads = Vec::with_capacity(lead_count as usize);
    let mut pin_num = 1u32;

    // Left edge: x = -body_w/2 - protrusion/2, y from +half_span down to -half_span.
    for i in 0..pins_per_side {
        let y = half_span - i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x: -(body_w / 2.0 + DEFAULT_LEAD_PROTRUSION_MM / 2.0),
                y,
                z: DEFAULT_LEAD_HEIGHT_MM / 2.0,
            },
            size: lead_size_along,
        });
        pin_num += 1;
    }
    // Bottom edge: y = -body_l/2 - protrusion/2, x from -half_span to +half_span.
    for i in 0..pins_per_side {
        let x = -half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x,
                y: -(body_l / 2.0 + DEFAULT_LEAD_PROTRUSION_MM / 2.0),
                z: DEFAULT_LEAD_HEIGHT_MM / 2.0,
            },
            size: lead_size_perp,
        });
        pin_num += 1;
    }
    // Right edge: x = +body_w/2 + protrusion/2, y from -half_span to +half_span.
    for i in 0..pins_per_side {
        let y = -half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x: body_w / 2.0 + DEFAULT_LEAD_PROTRUSION_MM / 2.0,
                y,
                z: DEFAULT_LEAD_HEIGHT_MM / 2.0,
            },
            size: lead_size_along,
        });
        pin_num += 1;
    }
    // Top edge: y = +body_l/2 + protrusion/2, x from +half_span down to -half_span.
    for i in 0..pins_per_side {
        let x = half_span - i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x,
                y: body_l / 2.0 + DEFAULT_LEAD_PROTRUSION_MM / 2.0,
                z: DEFAULT_LEAD_HEIGHT_MM / 2.0,
            },
            size: lead_size_perp,
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

    fn lqfp8() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/LQFP8-TEST"),
            family: "lqfp".into(),
            lead_count: 8,
            pitch: Dimension {
                nominal: Some(0.5),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(3.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(3.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_height: None,
                exposed_pad: None,
            },
            geometry: None,
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn generates_expected_lead_count_and_numbering() {
        let geometry = generate_lqfp(&lqfp8()).unwrap();
        assert_eq!(geometry.leads.len(), 8);
        let numbers: Vec<&str> = geometry.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, vec!["1", "2", "3", "4", "5", "6", "7", "8"]);
    }

    #[test]
    fn pin_1_is_on_the_left_edge_top() {
        let geometry = generate_lqfp(&lqfp8()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        // 2 pins/side, pitch 0.5 -> half_span = 0.25. Pin 1 at top of left edge.
        assert!((pin1.position.x - (-(3.0 / 2.0 + 0.3))).abs() < 1e-9);
        assert!((pin1.position.y - 0.25).abs() < 1e-9);
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = lqfp8();
        package.dimensions.body_width.nominal = None;
        let err = generate_lqfp(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }

    #[test]
    fn rejects_non_lqfp_family() {
        let mut package = lqfp8();
        package.family = "qfn".into();
        let err = generate_lqfp(&package).unwrap_err();
        assert!(matches!(err, McadError::UnsupportedFamily(_)));
    }
}
