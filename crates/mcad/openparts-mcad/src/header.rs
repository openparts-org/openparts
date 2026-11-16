//! Through-hole pin headers (`header`), socket strips (`socket`, female
//! headers), shrouded wire-to-board headers (`shrouded_header`, e.g. JST
//! XH/PH) and right-angle pin headers (`right_angle_header`).
//!
//! Frame (top view, the KiCad library's orientation): `lead_layout` is
//! `[rows, pins_per_row]`. A row runs along Y at `pitch`, the rows sit
//! `dimensions.row_spacing` apart along X; the grid is centred on the
//! origin. Pin 1 is at the top-left (-X, +Y); numbering zig-zags across
//! the rows: pin 2 beside pin 1 in the next row, pin 3 below pin 1, ...
//! (odd pins in the first row, even pins in the second for a 2-row
//! part). Rows 1 and 2 only: parts numbered row by row, or with three
//! rows, need their own convention. A right-angle header has one row.
//!
//! Footprint: a round land per pin on the source's recommended hole
//! (`dimensions.hole_diameter`, required), `dimensions.pad_diameter`
//! when the source gives the land, else the hole plus
//! [`defaults::HEADER`]'s annular ring.
//!
//! Body: the housing (insulator), `body_width` (along X) x `body_length`
//! (along Y) x `body_height`, `standoff` above the board (0 when not
//! stated). Its centre sits `dimensions.body_offset` along X from the
//! grid's centre (0 when not stated; required for a right-angle header,
//! whose housing lies on the board beside its holes, its pins bent 90°
//! towards +X for a positive offset). What only the 3D model needs (pin
//! size, post and tail lengths, the shroud's walls) is resolved by
//! [`header_features`] with the same value rules.

use crate::defaults::{self, stated, HEADER};
use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::{Package, PackageFamily as F};

/// The header families' shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderKind {
    /// Male pins standing above the housing (`header`).
    Pin,
    /// Female contacts inside the housing, entries on its top (`socket`).
    Socket,
    /// Male posts inside an open-top box housing (`shrouded_header`).
    Shrouded,
    /// Pins bent 90°: a tail down into each hole, the post running
    /// along X through a housing lying on the board
    /// (`right_angle_header`).
    RightAngle,
}

/// A shrouded header's box (generator defaults: wire-to-board drawings
/// dimension the outline only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shroud {
    /// Wall thickness.
    pub wall: f64,
    /// Cavity floor above the housing underside.
    pub floor: f64,
    /// Post tips below the shroud's rim.
    pub post_recess: f64,
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
    /// `Pin`: post above the housing; `RightAngle`: post beyond the
    /// housing's front face (`mating_length`, else the default); 0
    /// otherwise.
    pub mating_length: f64,
    /// Tail below the housing (`tail_length`).
    pub tail_length: f64,
    /// Housing underside above the board (`standoff`, else 0).
    pub standoff: f64,
    /// Housing height (`body_height`, else the family default).
    pub body_height: f64,
    /// Housing centre's X offset from the grid centre (`body_offset`,
    /// else 0).
    pub body_offset: f64,
    /// `Shrouded` only.
    pub shroud: Option<Shroud>,
    /// `RightAngle` only: the bent post's centre line above the board
    /// (the housing's mid-height: drawings rarely dimension it).
    pub bend_height: f64,
    /// Surface-mount contacts (`Package::smt`): no holes, an SMD land
    /// per contact and two mechanical pads.
    pub smt: Option<HeaderSmt>,
}

/// A surface-mount header's lands (`Package::smt`), for the 3D model's
/// solder tails and retention tabs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeaderSmt {
    /// Signal land `[X, Y]`, centred on the contact row (x = 0).
    pub pad: [f64; 2],
    /// Mechanical pad `[X, Y]`.
    pub mech_pad: [f64; 2],
    /// Centre of the `+y` mechanical pad (the other mirrors it).
    pub mech_at: [f64; 2],
}

fn kind(package: &Package) -> Result<HeaderKind, McadError> {
    match package.family {
        F::Header => Ok(HeaderKind::Pin),
        F::Socket => Ok(HeaderKind::Socket),
        F::ShroudedHeader => Ok(HeaderKind::Shrouded),
        F::RightAngleHeader => Ok(HeaderKind::RightAngle),
        other => Err(McadError::UnsupportedFamily(other)),
    }
}

/// `[rows, pins_per_row]`, checked against `lead_count` (one row for a
/// right-angle header).
fn layout(package: &Package, kind: HeaderKind) -> Result<(u32, u32), McadError> {
    let Some(layout) = package.lead_layout.as_deref() else {
        return Err(McadError::MissingLeadLayout {
            family: package.family,
        });
    };
    let max_rows = if kind == HeaderKind::RightAngle { 1 } else { 2 };
    match *layout {
        [rows, per_row]
            if (1..=max_rows).contains(&rows)
                && per_row > 0
                && rows * per_row == package.lead_count =>
        {
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
    let (rows, pins_per_row) = layout(package, kind)?;
    let pitch = defaults::dim(&package.pitch, "pitch.typ")?;
    let d = &package.dimensions;
    let row_spacing = if rows > 1 {
        stated(&d.row_spacing).ok_or(McadError::MissingDimension("dimensions.row_spacing"))?
    } else {
        pitch
    };
    let scale = pitch / 2.54;
    let standoff = stated(&d.standoff).unwrap_or(0.0);
    let body_height = stated(&d.body_height).unwrap_or(
        scale
            * match kind {
                HeaderKind::Pin | HeaderKind::RightAngle => HEADER.header_height,
                HeaderKind::Socket => HEADER.socket_height,
                HeaderKind::Shrouded => HEADER.shroud_height,
            },
    );
    let body_offset = match stated(&d.body_offset) {
        Some(offset) => offset,
        None if kind == HeaderKind::RightAngle => {
            return Err(McadError::MissingDimension("dimensions.body_offset"))
        }
        None => 0.0,
    };
    let pin_width = stated(&d.lead_width).unwrap_or(HEADER.pin_width * scale);
    let shroud = (kind == HeaderKind::Shrouded).then_some(Shroud {
        wall: HEADER.shroud_wall * scale,
        floor: body_height * HEADER.shroud_floor_ratio,
        post_recess: HEADER.shroud_post_recess * scale,
    });
    Ok(HeaderFeatures {
        kind,
        rows,
        pins_per_row,
        row_spacing,
        pin_width,
        mating_length: match kind {
            HeaderKind::Pin | HeaderKind::RightAngle => {
                stated(&d.mating_length).unwrap_or(HEADER.mating_length * scale)
            }
            HeaderKind::Socket | HeaderKind::Shrouded => 0.0,
        },
        tail_length: stated(&d.tail_length).unwrap_or(HEADER.tail_length * scale),
        standoff,
        body_height,
        body_offset,
        shroud,
        bend_height: if kind == HeaderKind::RightAngle {
            standoff + body_height / 2.0
        } else {
            0.0
        },
        smt: package.smt.as_ref().map(|s| HeaderSmt {
            pad: s.pad,
            mech_pad: s.mech_pad,
            mech_at: s.mech_pad_at,
        }),
    })
}

/// The pin rows' X positions, pin 1's row first.
fn row_xs(f: &HeaderFeatures) -> impl Iterator<Item = f64> + '_ {
    let x0 = -(f.rows as f64 - 1.0) * f.row_spacing / 2.0;
    (0..f.rows).map(move |row| x0 + row as f64 * f.row_spacing)
}

/// Generates Mechanical Geometry for a header, socket strip, shrouded
/// header or right-angle header (see the module docs).
pub fn generate_header(package: &Package) -> Result<MechanicalGeometry, McadError> {
    kind(package)?;
    let f = header_features(package)?;
    if f.smt.is_some() {
        return generate_smt(package, f);
    }
    let d = &package.dimensions;
    let pitch = defaults::dim(&package.pitch, "pitch.typ")?;
    let body_w = defaults::dim(&d.body_width, "dimensions.body_width")?;
    let body_l = defaults::dim(&d.body_length, "dimensions.body_length")?;
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
    // Where the housing stands relative to the pins.
    let reach = row_xs(&f)
        .map(|x| (x - f.body_offset).abs())
        .fold(0.0, f64::max);
    match f.kind {
        HeaderKind::RightAngle if reach - f.pin_width / 2.0 <= body_w / 2.0 => {
            return Err(McadError::InconsistentDimensions(format!(
                "a right-angle header's housing lies beside its holes: body_offset ({}) must \
                 exceed half the body width ({body_w}) plus half the pin",
                f.body_offset
            )));
        }
        HeaderKind::Pin | HeaderKind::Socket | HeaderKind::Shrouded
            if reach + f.pin_width / 2.0 >= body_w / 2.0 =>
        {
            return Err(McadError::InconsistentDimensions(format!(
                "the pin rows must lie inside the housing: body_offset ({}) leaves a row \
                 outside the {body_w} mm body width",
                f.body_offset
            )));
        }
        _ => {}
    }

    let xs: Vec<f64> = row_xs(&f).collect();
    let y0 = (f.pins_per_row as f64 - 1.0) * pitch / 2.0;
    let leads = (0..package.lead_count)
        .map(|i| {
            let (row, col) = (i % f.rows, i / f.rows);
            Lead {
                number: (i + 1).to_string(),
                position: Point3 {
                    x: xs[row as usize],
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
                slot: None,
                shape: crate::LandShape::Rect,
            }
        })
        .collect();

    let body = Body {
        position: Point3 {
            x: f.body_offset,
            y: 0.0,
            z: f.standoff + f.body_height / 2.0,
        },
        size: Size3 {
            x: body_w,
            y: body_l,
            z: f.body_height,
        },
        shape: BodyShape::Box,
    };
    // No pin-1 marker: the housings this family covers carry none, or an
    // undimensioned one (the footprint's square pad 1 marks it on the
    // board).
    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],
        pitch: crate::defaults::required(&package.pitch),
        details: crate::Details::Header(f),
        body,
        leads,
        markers: vec![],
        lead_form: None,
    })
}

/// A surface-mount wire-to-board header (`Package::smt`, section 20.5):
/// one contact row along Y at `pitch`, centred on the origin, pin 1 at
/// the top (`+y`); an SMD land per contact at x = 0, and two mechanical
/// pads (`smt.mech_number`) at `smt.mech_pad_at` and its mirror across
/// `y = 0`. The housing is the family's: `body_width` x `body_length` x
/// `body_height`, `body_offset` along X from the contact row. Top entry
/// is `shrouded_header`, side entry `right_angle_header` (the opening
/// faces away from the contact row).
fn generate_smt(package: &Package, f: HeaderFeatures) -> Result<MechanicalGeometry, McadError> {
    let Some(smt) = f.smt else {
        return Err(McadError::UnsupportedFamily(package.family));
    };
    if f.rows != 1 || !matches!(f.kind, HeaderKind::Shrouded | HeaderKind::RightAngle) {
        return Err(McadError::InconsistentDimensions(format!(
            "surface-mount contacts are drawn for a 1-row shrouded_header or \
             right_angle_header, not a {}-row {}",
            f.rows, package.family
        )));
    }
    let d = &package.dimensions;
    let pitch = defaults::dim(&package.pitch, "pitch.typ")?;
    let body_w = defaults::dim(&d.body_width, "dimensions.body_width")?;
    let body_l = defaults::dim(&d.body_length, "dimensions.body_length")?;
    let [pw, pl] = smt.pad;
    let [mw, ml] = smt.mech_pad;
    let [mx, my] = smt.mech_at;
    let y0 = (f.pins_per_row as f64 - 1.0) * pitch / 2.0;
    if pw <= 0.0 || pl <= 0.0 || mw <= 0.0 || ml <= 0.0 {
        return Err(McadError::InconsistentDimensions(
            "smt pad sizes must be positive".into(),
        ));
    }
    if f.pins_per_row > 1 && pl >= pitch {
        return Err(McadError::InconsistentDimensions(format!(
            "smt signal pads ({pl} along the row) overlap at pitch {pitch}"
        )));
    }
    // The mechanical pads clear the end contacts' lands.
    let clear_y = my - ml / 2.0 > y0 + pl / 2.0;
    let clear_x = (mx.abs() - mw / 2.0) > pw / 2.0;
    if !(clear_y || clear_x) || my <= 0.0 {
        return Err(McadError::InconsistentDimensions(format!(
            "the mechanical pad at ({mx}, {my}) overlaps the contact lands"
        )));
    }
    let pad_t = defaults::land::PAD_T;
    let lead = |number: String, x: f64, y: f64, sx: f64, sy: f64| Lead {
        number,
        position: Point3 {
            x,
            y,
            z: pad_t / 2.0,
        },
        size: Size3 {
            x: sx,
            y: sy,
            z: pad_t,
        },
        mounting: Mounting::Smd,
        drill: None,
        slot: None,
        shape: crate::LandShape::Rect,
    };
    let mech = package
        .smt
        .as_ref()
        .map_or("MP".to_string(), |s| s.mech_number.clone());
    let mut leads: Vec<Lead> = (0..f.pins_per_row)
        .map(|i| lead((i + 1).to_string(), 0.0, y0 - i as f64 * pitch, pw, pl))
        .collect();
    leads.push(lead(mech.clone(), mx, my, mw, ml));
    leads.push(lead(mech, mx, -my, mw, ml));

    let body = Body {
        position: Point3 {
            x: f.body_offset,
            y: 0.0,
            z: f.standoff + f.body_height / 2.0,
        },
        size: Size3 {
            x: body_w,
            y: body_l,
            z: f.body_height,
        },
        shape: BodyShape::Box,
    };
    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],
        pitch: crate::defaults::required(&package.pitch),
        details: crate::Details::Header(f),
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
            typ: Some(v),
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
            family: openparts_core::PackageFamily::Header,
            lead_count: 10,
            pitch: mm(2.54),
            dimensions: PackageDimensions {
                body_width: mm(5.08),
                body_length: mm(12.7),
                body_height: Some(Dimension {
                    typ: Some(2.54),
                    min: Some(2.39),
                    max: Some(2.69),
                    unit: "mm".into(),
                }),
                lead_width: Some(mm(0.64)),
                row_spacing: Some(mm(2.54)),
                hole_diameter: Some(Dimension {
                    typ: Some(1.1),
                    min: Some(0.95),
                    max: Some(1.25),
                    unit: "mm".into(),
                }),
                mating_length: Some(mm(6.0)),
                tail_length: Some(mm(3.0)),
                ..Default::default()
            },
            lead_layout: Some(vec![2, 5]),
            tab: None,
            aux_pads: vec![],
            pad_order: vec![],
            connector: None,
            edge: None,
            keepouts: vec![],
            pad_rows: vec![],
            pad_grids: vec![],
            shield: None,
            smt: None,
            pads: vec![],
            construction: None,
            geometry: None,
            provenance: BTreeMap::new(),
            review: None,
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
        p.family = openparts_core::PackageFamily::Socket;
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
        p.family = openparts_core::PackageFamily::Dip;
        assert!(matches!(
            generate_header(&p),
            Err(McadError::UnsupportedFamily(_))
        ));
    }

    /// JST B4B-XH-A: 1 x 4 at 2.5 mm, housing 12.4 x 5.75 x 7, the pin
    /// row 2.35 from the lock-side (+X) face: centre at -0.525.
    fn xh_4() -> Package {
        let mut p = wrphd_2x5();
        p.family = openparts_core::PackageFamily::ShroudedHeader;
        p.lead_count = 4;
        p.lead_layout = Some(vec![1, 4]);
        p.pitch = mm(2.5);
        p.dimensions.row_spacing = None;
        p.dimensions.body_width = mm(5.75);
        p.dimensions.body_length = mm(12.4);
        p.dimensions.body_height = Some(mm(7.0));
        p.dimensions.mating_length = None;
        p.dimensions.tail_length = Some(mm(3.4));
        p.dimensions.hole_diameter = Some(mm(0.9));
        p.dimensions.body_offset = Some(mm(-0.525));
        p
    }

    #[test]
    fn a_shrouded_header_sits_off_centre_around_its_row() {
        let g = crate::generate(&xh_4()).unwrap();
        assert!(close(g.body.position.x, -0.525) && close(g.body.size.x, 5.75));
        assert!(close(g.body.position.z, 3.5));
        assert!(close(lead(&g, "1").position.y, 3.75));
        assert!(close(lead(&g, "4").position.y, -3.75));
        assert!(g.leads.iter().all(|l| close(l.position.x, 0.0)));
        assert!(g.leads.iter().all(|l| l.drill == Some(0.9)));
        let f = header_features(&xh_4()).unwrap();
        assert_eq!(f.kind, HeaderKind::Shrouded);
        assert!(close(f.mating_length, 0.0) && close(f.tail_length, 3.4));
        let shroud = f.shroud.expect("a shroud");
        assert!(shroud.wall > 0.0 && shroud.floor > 0.0 && shroud.floor < 7.0);
        // No offset stated: centred. A row outside the housing: error.
        let mut p = xh_4();
        p.dimensions.body_offset = None;
        assert!(close(crate::generate(&p).unwrap().body.position.x, 0.0));
        p.dimensions.body_offset = Some(mm(3.0));
        assert!(matches!(
            generate_header(&p),
            Err(McadError::InconsistentDimensions(_))
        ));
    }

    /// Würth 61300411021: 1 x 4 right-angle, housing 2.54 deep along the
    /// pins, its back face 1.5 from the holes (centre at 1.5 + 1.27).
    fn wrphd_ra_4() -> Package {
        let mut p = wrphd_2x5();
        p.family = openparts_core::PackageFamily::RightAngleHeader;
        p.lead_count = 4;
        p.lead_layout = Some(vec![1, 4]);
        p.dimensions.row_spacing = None;
        p.dimensions.body_width = mm(2.54);
        p.dimensions.body_length = mm(10.16);
        p.dimensions.body_offset = Some(mm(2.77));
        p
    }

    #[test]
    fn a_right_angle_header_lies_beside_its_holes() {
        let g = crate::generate(&wrphd_ra_4()).unwrap();
        assert!(close(g.body.position.x, 2.77) && close(g.body.position.z, 1.27));
        assert!(close(lead(&g, "1").position.y, 3.81));
        assert!(g.leads.iter().all(|l| close(l.position.x, 0.0)));
        let f = header_features(&wrphd_ra_4()).unwrap();
        assert_eq!(f.kind, HeaderKind::RightAngle);
        assert!(close(f.mating_length, 6.0) && close(f.bend_height, 1.27));
        assert!(f.shroud.is_none());
        // The offset is required, the housing must clear the holes, and
        // one row only.
        let mut p = wrphd_ra_4();
        p.dimensions.body_offset = None;
        assert!(matches!(
            generate_header(&p),
            Err(McadError::MissingDimension("dimensions.body_offset"))
        ));
        p.dimensions.body_offset = Some(mm(1.0));
        assert!(matches!(
            generate_header(&p),
            Err(McadError::InconsistentDimensions(_))
        ));
        let mut p = wrphd_ra_4();
        p.lead_layout = Some(vec![2, 2]);
        p.dimensions.row_spacing = Some(mm(2.54));
        assert!(matches!(
            generate_header(&p),
            Err(McadError::InvalidLeadLayout { .. })
        ));
        // A generator naming another family is refused.
        let mut p = wrphd_ra_4();
        p.geometry = Some(openparts_core::PackageGeometrySpec {
            geometry_type: openparts_core::GeometryType::Parametric,
            generator: Some("header".into()),
        });
        assert!(matches!(
            crate::generate(&p),
            Err(McadError::UnsupportedGenerator(_))
        ));
    }

    /// JST SM04B-SRSS-TB-style side entry: lands 1.55 x 0.6 on a 1.0
    /// pitch, mechanical pads 1.8 x 1.2 at (-3.875, +-2.8).
    fn sm04b() -> Package {
        serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\n\
             family: right_angle_header\nlead_count: 4\nlead_layout: [1, 4]\n\
             pitch: {nominal: 1.0, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 4.25, unit: mm}\n\
             \x20 body_length: {nominal: 6.0, unit: mm}\n\
             \x20 body_height: {nominal: 2.9, unit: mm}\n\
             \x20 body_offset: {nominal: -2.45, unit: mm}\n\
             smt: {pad: [1.55, 0.6], mech_pad: [1.8, 1.2], mech_pad_at: [-3.875, 2.8]}\n",
        )
        .unwrap()
    }

    #[test]
    fn smt_headers_get_lands_on_the_row_and_two_mechanical_pads() {
        let g = crate::generate(&sm04b()).unwrap();
        let numbers: Vec<&str> = g.leads.iter().map(|l| l.number.as_str()).collect();
        assert_eq!(numbers, ["1", "2", "3", "4", "MP", "MP"]);
        assert!(g.leads.iter().all(|l| l.mounting == Mounting::Smd));
        assert!(g.lands.is_empty(), "the stated lands are the footprint");
        let p1 = &g.leads[0];
        assert!(p1.position.x.abs() < 1e-9 && (p1.position.y - 1.5).abs() < 1e-9);
        assert!((p1.size.x - 1.55).abs() < 1e-9 && (p1.size.y - 0.6).abs() < 1e-9);
        assert!((g.leads[5].position.y + 2.8).abs() < 1e-9);
        assert!((g.body.position.x + 2.45).abs() < 1e-9);
        // No hole needed; a mechanical pad on the lands is an error.
        let mut p = sm04b();
        p.smt.as_mut().unwrap().mech_pad_at = [0.0, 1.0];
        assert!(matches!(
            crate::generate(&p),
            Err(McadError::InconsistentDimensions(_))
        ));
        let mut p = sm04b();
        p.family = openparts_core::PackageFamily::Header;
        assert!(
            crate::generate(&p).is_err(),
            "smt is for wire-to-board headers"
        );
    }
}
