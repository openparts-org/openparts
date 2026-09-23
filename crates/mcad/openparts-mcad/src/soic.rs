use crate::{Body, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Point3, Size3};
use openparts_core::Package;

/// Engineering defaults used only when the datasheet doesn't supply a
/// value -- generator implementation details for a geometric
/// approximation, never written back into Canonical Data as a fact
/// (same policy as `lqfp.rs`/`qfn.rs`'s own defaults). SOIC leads are
/// smaller than LQFP's (e.g. JEDEC MS-012 vs. MS-026), so these are
/// distinct constants, not shared with `lqfp.rs`.
const DEFAULT_BODY_HEIGHT_MM: f64 = 1.75;
const DEFAULT_LEAD_WIDTH_MM: f64 = 0.4;
const DEFAULT_LEAD_PROTRUSION_MM: f64 = 0.5;
const DEFAULT_LEAD_HEIGHT_MM: f64 = 0.2;

/// Generates Mechanical Geometry for a SOIC/SOP-style package: a
/// rectangular body with `lead_count / 2` gull-wing leads evenly spaced
/// at `pitch` on each of the 2 long sides (left/right) -- structurally
/// [`crate::generate_lqfp`]'s left-edge/right-edge pattern with the
/// top/bottom edges dropped, since SOIC's physical layout (unlike
/// SOT's) is unambiguous and fully determined by `lead_count`.
///
/// Numbering convention (matches common SOIC/JEDEC MS-012 datasheets):
/// pin 1 at the top of the LEFT edge, proceeding down the left edge,
/// then up the right edge from its bottom to its top.
pub fn generate_soic(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if !package.family.eq_ignore_ascii_case("soic") {
        return Err(McadError::UnsupportedFamily(package.family.clone()));
    }
    if let Some(geometry) = &package.geometry {
        if let Some(generator) = &geometry.generator {
            if !generator.eq_ignore_ascii_case("soic") {
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
    };

    let span = (pins_per_side as f64 - 1.0) * pitch;
    let half_span = span / 2.0;

    let lead_size = Size3 {
        x: DEFAULT_LEAD_PROTRUSION_MM,
        y: DEFAULT_LEAD_WIDTH_MM,
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
            size: lead_size,
        });
        pin_num += 1;
    }
    // Right edge: x = +body_w/2 + protrusion/2, y from -half_span up to +half_span
    // -- numbering continues directly from the left edge (no bottom/top
    // edges exist on a 2-sided package).
    for i in 0..pins_per_side {
        let y = -half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x: body_w / 2.0 + DEFAULT_LEAD_PROTRUSION_MM / 2.0,
                y,
                z: DEFAULT_LEAD_HEIGHT_MM / 2.0,
            },
            size: lead_size,
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

    fn soic8() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/SOIC8-TEST"),
            family: "soic".into(),
            lead_count: 8,
            pitch: Dimension {
                nominal: Some(1.27),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(3.9),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(4.9),
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
        let geometry = generate_soic(&soic8()).unwrap();
        assert_eq!(geometry.leads.len(), 8);
        let numbers: Vec<&str> = geometry.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, vec!["1", "2", "3", "4", "5", "6", "7", "8"]);
    }

    #[test]
    fn pin_1_is_on_the_left_edge_top() {
        let geometry = generate_soic(&soic8()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        // 4 pins/side, pitch 1.27 -> half_span = 1.905.
        assert!((pin1.position.x - (-(3.9 / 2.0 + 0.25))).abs() < 1e-9);
        assert!((pin1.position.y - 1.905).abs() < 1e-9);
    }

    #[test]
    fn only_left_and_right_edges_are_populated() {
        // A 2-sided package: every lead's x is at one of exactly two
        // values (no bottom/top-edge leads at intermediate x positions).
        let geometry = generate_soic(&soic8()).unwrap();
        let xs: std::collections::BTreeSet<i64> = geometry
            .leads
            .iter()
            .map(|l| (l.position.x * 1000.0).round() as i64)
            .collect();
        assert_eq!(
            xs.len(),
            2,
            "expected leads on exactly 2 distinct x positions"
        );
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = soic8();
        package.dimensions.body_width.nominal = None;
        let err = generate_soic(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }

    #[test]
    fn rejects_non_soic_family() {
        let mut package = soic8();
        package.family = "qfn".into();
        let err = generate_soic(&package).unwrap_err();
        assert!(matches!(err, McadError::UnsupportedFamily(_)));
    }

    #[test]
    fn rejects_odd_lead_count() {
        let mut package = soic8();
        package.lead_count = 7;
        let err = generate_soic(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadCount(7)));
    }
}
