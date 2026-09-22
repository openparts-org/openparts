use crate::{Body, Lead, McadError, MechanicalGeometry, Point3, Size3};
use openparts_core::Package;

/// Engineering defaults used only when the datasheet doesn't supply a
/// value -- generator implementation details for a geometric
/// approximation, never written back into Canonical Data as a fact.
const DEFAULT_BODY_HEIGHT_MM: f64 = 0.9;
/// Contact length in the direction perpendicular to the package edge.
/// QFN leads are flush/near-flush with the body edge, unlike LQFP's
/// outward gull-wing.
const DEFAULT_LEAD_LENGTH_MM: f64 = 0.4;
const DEFAULT_LEAD_WIDTH_MM: f64 = 0.25;
const DEFAULT_LEAD_HEIGHT_MM: f64 = 0.05;

/// Generates Mechanical Geometry for a QFN/DFN-style leadframe package:
/// a rectangular body with `lead_count / 4` bottom-side contact pads
/// evenly spaced at `pitch` on each of the 4 sides, flush with the body
/// edge, plus an optional center Exposed Pad (`dimensions.exposed_pad`).
///
/// Numbering convention matches [`crate::generate_lqfp`]: pin 1 at the
/// top of the LEFT edge, proceeding down the left edge, then
/// left-to-right along the bottom, up the right edge, right-to-left
/// along the top. The Exposed Pad (when present) is a `Lead` keyed
/// `"EP"`, matching the Device pin key convention for exposed pads
/// (Canonical Data Specification section 14).
pub fn generate_qfn(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if !package.family.eq_ignore_ascii_case("qfn") {
        return Err(McadError::UnsupportedFamily(package.family.clone()));
    }
    if let Some(geometry) = &package.geometry {
        if let Some(generator) = &geometry.generator {
            if !generator.eq_ignore_ascii_case("qfn") {
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
        x: DEFAULT_LEAD_LENGTH_MM,
        y: DEFAULT_LEAD_WIDTH_MM,
        z: DEFAULT_LEAD_HEIGHT_MM,
    };
    let lead_size_perp = Size3 {
        x: DEFAULT_LEAD_WIDTH_MM,
        y: DEFAULT_LEAD_LENGTH_MM,
        z: DEFAULT_LEAD_HEIGHT_MM,
    };
    let lead_z = DEFAULT_LEAD_HEIGHT_MM / 2.0;

    let mut leads = Vec::with_capacity(lead_count as usize + 1);
    let mut pin_num = 1u32;

    // Left edge: contact flush with x = -body_w/2, y from +half_span down to -half_span.
    for i in 0..pins_per_side {
        let y = half_span - i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x: -(body_w / 2.0 - DEFAULT_LEAD_LENGTH_MM / 2.0),
                y,
                z: lead_z,
            },
            size: lead_size_along,
        });
        pin_num += 1;
    }
    // Bottom edge: flush with y = -body_l/2, x from -half_span to +half_span.
    for i in 0..pins_per_side {
        let x = -half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x,
                y: -(body_l / 2.0 - DEFAULT_LEAD_LENGTH_MM / 2.0),
                z: lead_z,
            },
            size: lead_size_perp,
        });
        pin_num += 1;
    }
    // Right edge: flush with x = +body_w/2, y from -half_span to +half_span.
    for i in 0..pins_per_side {
        let y = -half_span + i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x: body_w / 2.0 - DEFAULT_LEAD_LENGTH_MM / 2.0,
                y,
                z: lead_z,
            },
            size: lead_size_along,
        });
        pin_num += 1;
    }
    // Top edge: flush with y = +body_l/2, x from +half_span down to -half_span.
    for i in 0..pins_per_side {
        let x = half_span - i as f64 * pitch;
        leads.push(Lead {
            number: pin_num.to_string(),
            position: Point3 {
                x,
                y: body_l / 2.0 - DEFAULT_LEAD_LENGTH_MM / 2.0,
                z: lead_z,
            },
            size: lead_size_perp,
        });
        pin_num += 1;
    }

    if let Some(ep) = &package.dimensions.exposed_pad {
        let ep_w = ep.width.nominal.ok_or(McadError::MissingDimension(
            "dimensions.exposed_pad.width.nominal",
        ))?;
        let ep_l = ep.length.nominal.ok_or(McadError::MissingDimension(
            "dimensions.exposed_pad.length.nominal",
        ))?;
        leads.push(Lead {
            number: "EP".to_string(),
            position: Point3 {
                x: 0.0,
                y: 0.0,
                z: lead_z,
            },
            size: Size3 {
                x: ep_w,
                y: ep_l,
                z: DEFAULT_LEAD_HEIGHT_MM,
            },
        });
    }

    Ok(MechanicalGeometry {
        body,
        leads,
        markers: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Dimension, ExposedPadDimensions, Kind, Package, PackageDimensions};
    use std::collections::BTreeMap;

    fn qfn8_with_ep() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/QFN8-TEST"),
            family: "qfn".into(),
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
                exposed_pad: Some(ExposedPadDimensions {
                    width: Dimension {
                        nominal: Some(1.5),
                        min: None,
                        max: None,
                        unit: "mm".into(),
                    },
                    length: Dimension {
                        nominal: Some(1.5),
                        min: None,
                        max: None,
                        unit: "mm".into(),
                    },
                }),
            },
            geometry: None,
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn generates_leads_plus_exposed_pad() {
        let geometry = generate_qfn(&qfn8_with_ep()).unwrap();
        // 8 leads + 1 EP.
        assert_eq!(geometry.leads.len(), 9);
        let ep = geometry.leads.iter().find(|l| l.number == "EP").unwrap();
        assert_eq!(
            ep.position,
            Point3 {
                x: 0.0,
                y: 0.0,
                z: ep.position.z
            }
        );
        assert_eq!(ep.size.x, 1.5);
        assert_eq!(ep.size.y, 1.5);
    }

    #[test]
    fn leads_are_flush_with_body_edge_not_protruding() {
        let geometry = generate_qfn(&qfn8_with_ep()).unwrap();
        let pin1 = geometry.leads.iter().find(|l| l.number == "1").unwrap();
        // body_w/2 = 1.5; lead outer half-extent = lead center + length/2
        // must not exceed the body edge (QFN leads don't protrude like LQFP).
        let outer_edge = pin1.position.x - pin1.size.x / 2.0;
        assert!(
            outer_edge >= -1.5 - 1e-9,
            "lead protrudes beyond body: {outer_edge}"
        );
    }

    #[test]
    fn no_exposed_pad_field_means_no_ep_lead() {
        let mut package = qfn8_with_ep();
        package.dimensions.exposed_pad = None;
        let geometry = generate_qfn(&package).unwrap();
        assert_eq!(geometry.leads.len(), 8);
        assert!(geometry.leads.iter().all(|l| l.number != "EP"));
    }
}
