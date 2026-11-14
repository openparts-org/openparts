use crate::defaults::{self, Flush};
use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::{Package, PackageFamily as F};

/// Generates Mechanical Geometry for a QFN/DFN-style leadframe package:
/// a rectangular body with `lead_count / 4` bottom-side contact pads
/// evenly spaced at `pitch` on each of the 4 sides, flush with the body
/// edge, plus an optional center Exposed Pad (`dimensions.exposed_pad`).
/// Terminals are `lead_length` x `lead_width` x `lead_thickness`;
/// unstated dimensions fall back to `defaults::QFN`.
///
/// Numbering convention matches [`crate::generate_lqfp`]: pin 1 at the
/// top of the LEFT edge, proceeding down the left edge, then
/// left-to-right along the bottom, up the right edge, right-to-left
/// along the top. The Exposed Pad (when present) is a `Lead` keyed
/// `"EP"`, matching the Device pin key convention for exposed pads
/// (Canonical Data Specification section 14).
///
/// Families `dfn` and `son` (DFN/SON/WSON/VSON/USON...) are the same
/// construction with terminals on the left and right edges only:
/// `lead_count / 2` per side, pin 1 at the top of the left edge, down the
/// left edge, then up the right edge (the SOIC order). `body_width` is
/// the body across the two rows (X).
///
/// Family `lga` (sensor LGAs: BME280, LIS3DH, BMI270...) takes its
/// per-side terminal counts from `lead_layout: [left, bottom, right,
/// top]` (zeros allowed), numbered in the same counter-clockwise order,
/// each side centred at `pitch`; `dimensions.lead_inset` sets the
/// terminals in from the body edge.
pub fn generate_qfn(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let kind = match package.family {
        F::Qfn => Kind::Qfn,
        F::Dfn | F::Son => Kind::TwoSided,
        F::Lga => Kind::Lga,
        other => return Err(McadError::UnsupportedFamily(other)),
    };

    let n = package.lead_count;
    // Terminals per side: [left, bottom, right, top].
    let counts: [u32; 4] = match kind {
        Kind::Qfn if n > 0 && n.is_multiple_of(4) => [n / 4; 4],
        Kind::TwoSided if n > 0 && n.is_multiple_of(2) => [n / 2, 0, n / 2, 0],
        Kind::Lga => {
            let layout = package
                .lead_layout
                .as_ref()
                .ok_or(McadError::MissingLeadLayout {
                    family: package.family,
                })?;
            match layout.as_slice() {
                [l, b, r, t] if l + b + r + t == n && n > 0 => [*l, *b, *r, *t],
                _ => {
                    return Err(McadError::InvalidLeadLayout {
                        layout: layout.clone(),
                        lead_count: n,
                    })
                }
            }
        }
        _ => return Err(McadError::InvalidLeadCount(n)),
    };

    let pitch = defaults::dim(&package.pitch, "pitch.nominal")?;
    let body_w = defaults::dim(
        &package.dimensions.body_width,
        "dimensions.body_width.nominal",
    )?;
    let body_l = defaults::dim(
        &package.dimensions.body_length,
        "dimensions.body_length.nominal",
    )?;
    let shape = Flush::resolve(
        package,
        if kind == Kind::Lga {
            &defaults::LGA
        } else {
            &defaults::QFN
        },
    );
    let body_h = shape.body_height;
    let (lead_len, lead_w, lead_t) = (shape.lead_length, shape.lead_width, shape.thickness);
    let inset = defaults::value(&package.dimensions.lead_inset).unwrap_or(0.0);
    if inset + lead_len > body_w.min(body_l) / 2.0 {
        return Err(McadError::InconsistentDimensions(format!(
            "terminals ({lead_len} long, {inset} in from the edge) reach past the body centre"
        )));
    }

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

    let along_y = Size3 {
        x: lead_len,
        y: lead_w,
        z: lead_t,
    };
    let along_x = Size3 {
        x: lead_w,
        y: lead_len,
        z: lead_t,
    };
    let (cx, cy) = (
        body_w / 2.0 - inset - lead_len / 2.0,
        body_l / 2.0 - inset - lead_len / 2.0,
    );
    let z = lead_t / 2.0;

    let mut leads = Vec::with_capacity(n as usize + 1);
    let mut pin = 1u32;
    let mut push = |x: f64, y: f64, size: Size3| {
        leads.push(Lead {
            number: pin.to_string(),
            position: Point3 { x, y, z },
            size,
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        });
        pin += 1;
    };
    let half = |count: u32| (count as f64 - 1.0) * pitch / 2.0;
    // Counter-clockwise from the top of the left edge (top view): down
    // the left edge, left to right along the bottom, up the right edge,
    // right to left along the top.
    for i in 0..counts[0] {
        push(-cx, half(counts[0]) - i as f64 * pitch, along_y);
    }
    for i in 0..counts[1] {
        push(-half(counts[1]) + i as f64 * pitch, -cy, along_x);
    }
    for i in 0..counts[2] {
        push(cx, -half(counts[2]) + i as f64 * pitch, along_y);
    }
    for i in 0..counts[3] {
        push(half(counts[3]) - i as f64 * pitch, cy, along_x);
    }

    if let Some(ep) = &package.dimensions.exposed_pad {
        let ep_w = defaults::dim(&ep.width, "dimensions.exposed_pad.width.nominal")?;
        let ep_l = defaults::dim(&ep.length, "dimensions.exposed_pad.length.nominal")?;
        leads.push(Lead {
            number: "EP".to_string(),
            position: Point3 { x: 0.0, y: 0.0, z },
            size: Size3 {
                x: ep_w,
                y: ep_l,
                z: lead_t,
            },
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        });
    }

    // Pin-1 mark on the top face, in the corner nearest pin 1.
    let pin1 = leads[0].position;
    let inset = (body_w.min(body_l) * 0.15).min(0.5);
    let markers = vec![Marker {
        kind: MarkerKind::Pin1Dot,
        position: Point3 {
            x: if pin1.x <= 0.0 {
                -body_w / 2.0 + inset
            } else {
                body_w / 2.0 - inset
            },
            y: if pin1.y >= 0.0 {
                body_l / 2.0 - inset
            } else {
                -body_l / 2.0 + inset
            },
            z: body_h,
        },
    }];

    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],

        pitch: crate::defaults::required(&package.pitch),

        details: crate::Details::None,
        body,
        leads,
        markers,
        lead_form: Some(shape.lead_form()),
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Qfn,
    TwoSided,
    Lga,
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
            family: openparts_core::PackageFamily::Qfn,
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
                ..Default::default()
            },
            lead_layout: None,
            tab: None,
            aux_pads: vec![],
            pad_order: vec![],
            pads: vec![],
            construction: None,
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
    fn dfn_has_two_rows_numbered_like_soic() {
        let mut package = qfn8_with_ep();
        package.family = openparts_core::PackageFamily::Dfn;
        let g = generate_qfn(&package).unwrap();
        let pos = |n: &str| g.leads.iter().find(|l| l.number == n).unwrap().position;
        assert_eq!(g.leads.len(), 9);
        assert!(pos("1").x < 0.0 && pos("4").x < 0.0 && pos("5").x > 0.0);
        assert!((pos("1").y - 0.75).abs() < 1e-9, "pin 1 top-left");
        assert!((pos("4").y + 0.75).abs() < 1e-9);
        assert!((pos("5").y + 0.75).abs() < 1e-9, "right edge upward");
        assert!((pos("8").y - 0.75).abs() < 1e-9);
        package.lead_count = 6;
        package.family = openparts_core::PackageFamily::Son;
        assert_eq!(generate_qfn(&package).unwrap().leads.len(), 7);
        package.lead_count = 7;
        assert!(generate_qfn(&package).is_err());
    }

    #[test]
    fn lga_uses_per_side_counts_and_insets_its_terminals() {
        let mut package = qfn8_with_ep();
        package.family = openparts_core::PackageFamily::Lga;
        package.dimensions.exposed_pad = None;
        assert!(matches!(
            generate_qfn(&package),
            Err(McadError::MissingLeadLayout { .. })
        ));
        package.lead_layout = Some(vec![3, 1, 3, 1]);
        package.dimensions.lead_inset = Some(Dimension {
            nominal: Some(0.1),
            min: None,
            max: None,
            unit: "mm".into(),
        });
        let g = generate_qfn(&package).unwrap();
        let pos = |n: &str| g.leads.iter().find(|l| l.number == n).unwrap();
        assert_eq!(g.leads.len(), 8);
        let p1 = pos("1");
        assert!(
            (p1.position.x - p1.size.x / 2.0 + 1.4).abs() < 1e-9,
            "0.1 in from -1.5"
        );
        assert!((p1.position.y - 0.5).abs() < 1e-9);
        assert!(pos("4").position.x.abs() < 1e-9 && pos("4").position.y < 0.0);
        assert!(pos("8").position.x.abs() < 1e-9 && pos("8").position.y > 0.0);
        package.lead_layout = Some(vec![3, 1, 3]);
        assert!(generate_qfn(&package).is_err());
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
