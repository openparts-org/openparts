//! Through-hole pin headers (`header`) and socket strips (`socket`,
//! female headers), straight (vertical).
//!
//! Frame (top view, the KiCad library's orientation): `lead_layout` is
//! `[rows, pins_per_row]`. A row runs along Y at `pitch`, the rows sit
//! `dimensions.row_spacing` apart along X; the grid is centred on the
//! origin. Pin 1 is at the top-left (-X, +Y); numbering zig-zags across
//! the rows: pin 2 beside pin 1 in the next row, pin 3 below pin 1, ...
//! (odd pins in the first row, even pins in the second for a 2-row
//! part). Rows 1 and 2 only: parts numbered row by row, or with three
//! rows, need their own convention.
//!
//! Footprint: a round land per pin on the source's recommended hole
//! (`dimensions.hole_diameter`, required), `dimensions.pad_diameter`
//! when the source gives the land, else the hole plus
//! [`defaults::HEADER`]'s annular ring.
//!
//! Body: the housing (insulator), `body_width` (across the rows) x
//! `body_length` (along them) x `body_height`, centred on the pin grid,
//! `standoff` above the board (0 when not stated). What only the 3D model
//! needs (pin size, post and tail lengths) is resolved by
//! [`header_features`] with the same value rules.

use crate::defaults::{self, stated, HEADER};
use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::Package;

/// Pin header or socket strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderKind {
    /// Male pins standing above the housing (`header`).
    Pin,
    /// Female contacts inside the housing, entries on its top (`socket`).
    Socket,
}

/// What a header's 3D model draws beyond its `MechanicalGeometry`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeaderFeatures {
    pub kind: HeaderKind,
    pub rows: u32,
    pub pins_per_row: u32,
    /// Centre distance of the rows (the pitch for a 1-row part).
    pub row_spacing: f64,
    /// Square pin (post and tail) size: `lead_width`, else the default.
    pub pin_width: f64,
    /// Post above the housing (`mating_length`); 0 for a socket.
    pub mating_length: f64,
    /// Tail below the housing (`tail_length`).
    pub tail_length: f64,
    /// Housing underside above the board (`standoff`, else 0).
    pub standoff: f64,
}

fn kind(package: &Package) -> Result<HeaderKind, McadError> {
    match package.family.to_ascii_lowercase().as_str() {
        "header" => Ok(HeaderKind::Pin),
        "socket" => Ok(HeaderKind::Socket),
        _ => Err(McadError::UnsupportedFamily(package.family.clone())),
    }
}

/// `[rows, pins_per_row]`, checked against `lead_count`.
fn layout(package: &Package) -> Result<(u32, u32), McadError> {
    let Some(layout) = package.lead_layout.as_deref() else {
        return Err(McadError::MissingLeadLayout {
            family: package.family.clone(),
        });
    };
    match *layout {
        [rows @ (1 | 2), per_row] if per_row > 0 && rows * per_row == package.lead_count => {
            Ok((rows, per_row))
        }
        _ => Err(McadError::InvalidLeadLayout {
            layout: layout.to_vec(),
            lead_count: package.lead_count,
        }),
    }
}

/// Resolves the header's [`HeaderFeatures`] (nominal, else the min..max
/// midpoint, else the one stated limit; else the default for the grid).
pub fn header_features(package: &Package) -> Result<HeaderFeatures, McadError> {
    let kind = kind(package)?;
    let (rows, pins_per_row) = layout(package)?;
    let pitch = defaults::dim(&package.pitch, "pitch.nominal")?;
    let d = &package.dimensions;
    let row_spacing = if rows > 1 {
        stated(&d.row_spacing).ok_or(McadError::MissingDimension("dimensions.row_spacing"))?
    } else {
        pitch
    };
    let scale = pitch / 2.54;
    Ok(HeaderFeatures {
        kind,
        rows,
        pins_per_row,
        row_spacing,
        pin_width: stated(&d.lead_width).unwrap_or(HEADER.pin_width * scale),
        mating_length: match kind {
            HeaderKind::Pin => stated(&d.mating_length).unwrap_or(HEADER.mating_length * scale),
            HeaderKind::Socket => 0.0,
        },
        tail_length: stated(&d.tail_length).unwrap_or(HEADER.tail_length * scale),
        standoff: stated(&d.standoff).unwrap_or(0.0),
    })
}

/// Generates Mechanical Geometry for a straight pin header or socket
/// strip (see the module docs).
pub fn generate_header(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let kind = kind(package)?;
    if let Some(generator) = package.geometry.as_ref().and_then(|g| g.generator.as_ref()) {
        if !["header", "socket"].contains(&generator.to_ascii_lowercase().as_str()) {
            return Err(McadError::UnsupportedGenerator(generator.clone()));
        }
    }
    let f = header_features(package)?;
    let d = &package.dimensions;
    let pitch = defaults::dim(&package.pitch, "pitch.nominal")?;
    let body_w = defaults::dim(&d.body_width, "dimensions.body_width")?;
    let body_l = defaults::dim(&d.body_length, "dimensions.body_length")?;
    let body_h = stated(&d.body_height).unwrap_or(
        pitch / 2.54
            * match kind {
                HeaderKind::Pin => HEADER.header_height,
                HeaderKind::Socket => HEADER.socket_height,
            },
    );
    let hole =
        stated(&d.hole_diameter).ok_or(McadError::MissingDimension("dimensions.hole_diameter"))?;

    // Nearest neighbour distance on the grid.
    let grid = if f.rows > 1 {
        pitch.min(f.row_spacing)
    } else {
        pitch
    };
    let pad = match stated(&d.pad_diameter) {
        Some(pad) => pad,
        None => {
            let ring = HEADER
                .annular_ring
                .min((grid - hole - HEADER.min_gap) / 2.0);
            if ring < HEADER.min_ring {
                return Err(McadError::InconsistentDimensions(format!(
                    "dimensions.hole_diameter ({hole}) leaves no land on a {grid} mm grid"
                )));
            }
            hole + 2.0 * ring
        }
    };
    if hole <= 0.0 || pad <= hole || (f.pins_per_row > 1 || f.rows > 1) && pad >= grid {
        return Err(McadError::InconsistentDimensions(format!(
            "pad_diameter ({pad}) must exceed hole_diameter ({hole}) and stay under the \
             {grid} mm grid"
        )));
    }

    let x0 = -(f.rows as f64 - 1.0) * f.row_spacing / 2.0;
    let y0 = (f.pins_per_row as f64 - 1.0) * pitch / 2.0;
    let leads = (0..package.lead_count)
        .map(|i| {
            let (row, col) = (i % f.rows, i / f.rows);
            Lead {
                number: (i + 1).to_string(),
                position: Point3 {
                    x: x0 + row as f64 * f.row_spacing,
                    y: y0 - col as f64 * pitch,
                    z: 0.0,
                },
                size: Size3 {
                    x: pad,
                    y: pad,
                    z: f.tail_length,
                },
                mounting: Mounting::ThroughHole,
                drill: Some(hole),
            }
        })
        .collect();

    let body = Body {
        position: Point3 {
            x: 0.0,
            y: 0.0,
            z: f.standoff + body_h / 2.0,
        },
        size: Size3 {
            x: body_w,
            y: body_l,
            z: body_h,
        },
        shape: BodyShape::Box,
    };
    // No pin-1 marker: the housings this family covers carry none (the
    // footprint's square pad 1 marks it on the board).
    Ok(MechanicalGeometry {
        body,
        leads,
        markers: vec![],
        lead_form: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Dimension, Kind, PackageDimensions};
    use std::collections::BTreeMap;

    fn mm(v: f64) -> Dimension {
        Dimension {
            nominal: Some(v),
            min: None,
            max: None,
            unit: "mm".into(),
        }
    }

    /// Würth WR-PHD 61301021121 (2 x 5, 2.54 mm): housing 12.7 x 5.08 x
    /// 2.54, post 6.0, tail 3.0, recommended hole 1.10.
    fn wrphd_2x5() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/HEADER-TEST"),
            family: "header".into(),
            lead_count: 10,
            pitch: mm(2.54),
            dimensions: PackageDimensions {
                body_width: mm(5.08),
                body_length: mm(12.7),
                body_height: Some(Dimension {
                    nominal: Some(2.54),
                    min: Some(2.39),
                    max: Some(2.69),
                    unit: "mm".into(),
                }),
                exposed_pad: None,
                overall_width: None,
                overall_length: None,
                overall_height: None,
                standoff: None,
                lead_width: Some(mm(0.64)),
                lead_length: None,
                lead_thickness: None,
                tab_width: None,
                tab_length: None,
                row_spacing: Some(mm(2.54)),
                hole_diameter: Some(Dimension {
                    nominal: Some(1.1),
                    min: Some(0.95),
                    max: Some(1.25),
                    unit: "mm".into(),
                }),
                aux_pad_width: None,
                aux_pad_length: None,
                aux_row_x: None,
                aux_row_y: None,
                mounting_hole_diameter: None,
                mounting_hole_spacing: None,
                mounting_hole_inset: None,
                mating_length: Some(mm(6.0)),
                tail_length: Some(mm(3.0)),
                pad_diameter: None,
            },
            lead_layout: Some(vec![2, 5]),
            tab: None,
            aux_pads: vec![],
            construction: None,
            geometry: None,
            provenance: BTreeMap::new(),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn lead<'a>(g: &'a MechanicalGeometry, number: &str) -> &'a Lead {
        g.leads.iter().find(|l| l.number == number).unwrap()
    }

    #[test]
    fn two_rows_number_odd_even_from_the_top_left() {
        let g = generate_header(&wrphd_2x5()).unwrap();
        assert_eq!(g.leads.len(), 10);
        let at = |n: &str| (lead(&g, n).position.x, lead(&g, n).position.y);
        // Pin 1 top-left, pin 2 beside it, pin 3 below pin 1, pin 10
        // bottom-right; the grid is centred.
        assert!(close(at("1").0, -1.27) && close(at("1").1, 5.08));
        assert!(close(at("2").0, 1.27) && close(at("2").1, 5.08));
        assert!(close(at("3").0, -1.27) && close(at("3").1, 2.54));
        assert!(close(at("10").0, 1.27) && close(at("10").1, -5.08));
        for l in &g.leads {
            assert_eq!(l.mounting, Mounting::ThroughHole);
            assert_eq!(l.drill, Some(1.1));
            // No land given: hole + 2 x 0.35 ring.
            assert!(close(l.size.x, 1.8) && close(l.size.y, 1.8));
        }
        assert!(close(g.body.size.x, 5.08) && close(g.body.size.y, 12.7));
        assert!(close(g.body.position.z, 1.27));
        assert!(g.markers.is_empty());
    }

    #[test]
    fn one_row_runs_down_from_pin_1() {
        let mut p = wrphd_2x5();
        p.lead_layout = Some(vec![1, 10]);
        p.dimensions.row_spacing = None;
        let g = generate_header(&p).unwrap();
        assert!(g.leads.iter().all(|l| close(l.position.x, 0.0)));
        assert!(close(lead(&g, "1").position.y, 11.43));
        assert!(close(lead(&g, "2").position.y, 11.43 - 2.54));
        assert!(close(lead(&g, "10").position.y, -11.43));
    }

    #[test]
    fn the_land_is_the_stated_pad_or_a_ring_that_fits_the_grid() {
        let mut p = wrphd_2x5();
        p.dimensions.pad_diameter = Some(mm(1.7));
        let g = generate_header(&p).unwrap();
        assert!(close(g.leads[0].size.x, 1.7));
        // A 1.27 mm grid: the ring shrinks to keep 0.25 mm of copper gap.
        let mut p = wrphd_2x5();
        p.pitch = mm(1.27);
        p.dimensions.row_spacing = Some(mm(1.27));
        p.dimensions.hole_diameter = Some(mm(0.65));
        let g = generate_header(&p).unwrap();
        assert!(close(g.leads[0].size.x, 1.27 - 0.25));
        // A hole too large for the grid, or a land over it: errors.
        p.dimensions.hole_diameter = Some(mm(1.0));
        assert!(matches!(
            generate_header(&p),
            Err(McadError::InconsistentDimensions(_))
        ));
        let mut p = wrphd_2x5();
        p.dimensions.pad_diameter = Some(mm(2.6));
        assert!(matches!(
            generate_header(&p),
            Err(McadError::InconsistentDimensions(_))
        ));
    }

    #[test]
    fn layout_hole_and_row_spacing_are_required_never_guessed() {
        let mut p = wrphd_2x5();
        p.lead_layout = None;
        assert!(matches!(
            generate_header(&p),
            Err(McadError::MissingLeadLayout { .. })
        ));
        for layout in [vec![2, 4], vec![10], vec![5, 2]] {
            let mut p = wrphd_2x5();
            p.lead_layout = Some(layout);
            assert!(matches!(
                generate_header(&p),
                Err(McadError::InvalidLeadLayout { .. })
            ));
        }
        let mut p = wrphd_2x5();
        p.dimensions.row_spacing = None;
        assert!(matches!(
            generate_header(&p),
            Err(McadError::MissingDimension("dimensions.row_spacing"))
        ));
        let mut p = wrphd_2x5();
        p.dimensions.hole_diameter = None;
        assert!(matches!(
            generate_header(&p),
            Err(McadError::MissingDimension("dimensions.hole_diameter"))
        ));
    }

    #[test]
    fn features_take_the_stated_lengths_else_the_scaled_defaults() {
        let f = header_features(&wrphd_2x5()).unwrap();
        assert_eq!(f.kind, HeaderKind::Pin);
        assert_eq!((f.rows, f.pins_per_row), (2, 5));
        assert!(close(f.pin_width, 0.64) && close(f.mating_length, 6.0));
        assert!(close(f.tail_length, 3.0) && close(f.standoff, 0.0));
        let mut p = wrphd_2x5();
        p.family = "socket".into();
        p.pitch = mm(2.0);
        p.dimensions.row_spacing = Some(mm(2.0));
        p.dimensions.hole_diameter = Some(mm(0.8));
        p.dimensions.lead_width = None;
        p.dimensions.tail_length = None;
        p.dimensions.body_height = None;
        let f = header_features(&p).unwrap();
        assert_eq!(f.kind, HeaderKind::Socket);
        assert!(close(f.mating_length, 0.0));
        assert!(close(f.pin_width, 0.64 * 2.0 / 2.54));
        assert!(close(f.tail_length, 3.0 * 2.0 / 2.54));
        let g = generate_header(&p).unwrap();
        assert!(close(g.body.size.z, 8.5 * 2.0 / 2.54));
        assert!(crate::generate(&p).is_ok(), "socket dispatched");
        assert!(crate::generate(&wrphd_2x5()).is_ok(), "header dispatched");
        p.family = "dip".into();
        assert!(matches!(
            generate_header(&p),
            Err(McadError::UnsupportedFamily(_))
        ));
    }
}
