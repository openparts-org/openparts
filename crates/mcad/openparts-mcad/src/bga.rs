use crate::defaults;
use crate::{
    Body, BodyShape, Lead, Marker, MarkerKind, McadError, MechanicalGeometry, Mounting, Point3,
    Size3,
};
use openparts_core::Package;

/// Real JEDEC BGA row-letter skip set: confirmed directly from a real
/// KiCad reference footprint (`Package_BGA.pretty/
/// BGA-324_15.0x15.0mm_Layout18x18_P0.8mm_....kicad_mod`), whose 18
/// rows are lettered `A..H, J, K, L, M, N, P, R, T, U, V` -- I, O, Q,
/// and S are genuinely all absent, not just I/O/Q as commonly assumed.
const SKIPPED_ROW_LETTERS: [u8; 4] = *b"IOQS";

/// Maps a 0-based row index to its real BGA row letter, applying the
/// skip set above. Only single letters (A-Z minus the 4 skipped) are
/// supported -- roughly 22 rows -- a grid needing more rows than that
/// is out of scope for this generator (real double-letter designators
/// like "AA"/"AB" are not implemented).
fn row_letter(row_index: u32) -> Result<char, McadError> {
    let mut letter = b'A';
    let mut remaining = row_index;
    loop {
        if letter > b'Z' {
            return Err(McadError::TooManyBgaRows(row_index + 1));
        }
        if !SKIPPED_ROW_LETTERS.contains(&letter) {
            if remaining == 0 {
                return Ok(letter as char);
            }
            remaining -= 1;
        }
        letter += 1;
    }
}

/// Generates Mechanical Geometry for a full (non-depopulated) BGA-style
/// ball/pad grid array: a rectangular body with `rows x cols` SMD
/// `Lead`s evenly spaced by `pitch` in both axes, no exposed pad.
/// Covers BGA/CSP/FBGA/LGA/WLCSP -- all real-world variations of the
/// same full-grid shape (ball vs. flat-pad and pitch/size differ, but
/// this crate's box-lead approximation doesn't distinguish that any
/// more than it distinguishes a gull-wing lead from a flush no-lead
/// terminal for `soic`/DFN).
///
/// `rows`/`cols` reuse [`Package::lead_layout`] (already a `Vec<u32>`,
/// used by `sot.rs` for `[bottom_count, top_count]`) as `[rows, cols]`
/// instead -- a family-specific-meaning layout hint, avoiding a new
/// schema field. `rows * cols` must equal `lead_count`.
///
/// Numbering uses real JEDEC alphanumeric grid designators (row letter
/// combined with a 1-based column number, e.g. `"A1"`, `"B3"`) -- not
/// sequential `"1"/"2"/...` like every other family. Row A is the top
/// edge (+Y), column 1 is the left edge (-X).
pub fn generate_bga(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let layout = package
        .lead_layout
        .clone()
        .ok_or(McadError::MissingLeadLayout {
            family: package.family,
        })?;
    let invalid = || McadError::InvalidLeadLayout {
        layout: layout.clone(),
        lead_count: package.lead_count,
    };
    if layout.len() != 2 || layout.contains(&0) {
        return Err(invalid());
    }
    let (rows, cols) = (layout[0], layout[1]);
    if rows
        .checked_mul(cols)
        .is_none_or(|product| product != package.lead_count)
    {
        return Err(invalid());
    }

    let pitch = defaults::dim(&package.pitch, "pitch.typ")?;
    let body_w = defaults::dim(&package.dimensions.body_width, "dimensions.body_width.typ")?;
    let body_l = defaults::dim(
        &package.dimensions.body_length,
        "dimensions.body_length.typ",
    )?;
    let body_h = defaults::stated(&package.dimensions.body_height)
        .unwrap_or(crate::defaults::bga::BODY_HEIGHT);

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

    let row_half_span = (rows as f64 - 1.0) * pitch / 2.0;
    let col_half_span = (cols as f64 - 1.0) * pitch / 2.0;

    let pad_size = Size3 {
        x: crate::defaults::bga::PAD_DIAMETER,
        y: crate::defaults::bga::PAD_DIAMETER,
        z: crate::defaults::bga::LEAD_HEIGHT,
    };

    let mut leads = Vec::with_capacity((rows * cols) as usize);
    for r in 0..rows {
        let letter = row_letter(r)?;
        let y = row_half_span - r as f64 * pitch;
        for c in 0..cols {
            let x = -col_half_span + c as f64 * pitch;
            leads.push(Lead {
                number: format!("{letter}{}", c + 1),
                position: Point3 { x, y, z: 0.0 },
                size: pad_size,
                mounting: Mounting::Smd,
                drill: None,
                slot: None,
                shape: crate::LandShape::Rect,
                side: openparts_core::PadSide::Top,
            });
        }
    }

    let markers = vec![Marker {
        kind: MarkerKind::Pin1Dot,
        position: Point3 {
            x: -col_half_span,
            y: row_half_span,
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
        lead_form: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{Dimension, Kind, Package, PackageDimensions};
    use std::collections::BTreeMap;

    fn bga_6x8() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/BGA-TEST"),
            family: openparts_core::PackageFamily::Bga,
            lead_count: 48,
            pitch: Dimension {
                typ: Some(0.8),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    typ: Some(8.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    typ: Some(9.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                ..Default::default()
            },
            lead_layout: Some(vec![6, 8]),
            tab: None,
            aux_pads: vec![],
            pad_order: vec![],
            actuation: None,
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

    #[test]
    fn generates_rows_times_cols_leads() {
        let geometry = generate_bga(&bga_6x8()).unwrap();
        assert_eq!(geometry.leads.len(), 48);
    }

    #[test]
    fn every_lead_is_smd_with_no_drill() {
        let geometry = generate_bga(&bga_6x8()).unwrap();
        for lead in &geometry.leads {
            assert_eq!(lead.mounting, Mounting::Smd);
            assert_eq!(lead.drill, None);
        }
    }

    #[test]
    fn uses_real_alphanumeric_grid_numbering_skipping_i_o_q_s() {
        let geometry = generate_bga(&bga_6x8()).unwrap();
        let numbers: std::collections::BTreeSet<&str> =
            geometry.leads.iter().map(|l| l.number.as_str()).collect();
        assert!(numbers.contains("A1"));
        assert!(numbers.contains("F8"));
        // 6 rows -> A,B,C,D,E,F -- none of I/O/Q/S needed yet at this
        // size, but confirm the skip table itself is correct by testing
        // row_letter directly past the point where it matters.
        // Matches the real 18-row KiCad reference sequence exactly:
        // A,B,C,D,E,F,G,H,J,K,L,M,N,P,R,T,U,V (indices 0-17).
        assert_eq!(row_letter(0).unwrap(), 'A');
        assert_eq!(row_letter(7).unwrap(), 'H'); // A..H = 8 (no skip yet)
        assert_eq!(row_letter(8).unwrap(), 'J'); // I skipped
        assert_eq!(row_letter(12).unwrap(), 'N'); // J,K,L,M,N
        assert_eq!(row_letter(13).unwrap(), 'P'); // O skipped
        assert_eq!(row_letter(14).unwrap(), 'R'); // Q skipped
        assert_eq!(row_letter(15).unwrap(), 'T'); // S skipped
        assert_eq!(row_letter(17).unwrap(), 'V'); // the reference's last row
    }

    #[test]
    fn row_a_is_the_top_edge_and_column_1_is_the_left_edge() {
        let geometry = generate_bga(&bga_6x8()).unwrap();
        let a1 = geometry.leads.iter().find(|l| l.number == "A1").unwrap();
        let f8 = geometry.leads.iter().find(|l| l.number == "F8").unwrap();
        assert!(a1.position.x < 0.0 && a1.position.y > 0.0);
        assert!(f8.position.x > 0.0 && f8.position.y < 0.0);
    }

    #[test]
    fn missing_lead_layout_is_an_error_not_a_guessed_default() {
        let mut package = bga_6x8();
        package.lead_layout = None;
        let err = generate_bga(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingLeadLayout { .. }));
    }

    #[test]
    fn lead_layout_not_matching_lead_count_is_rejected() {
        let mut package = bga_6x8();
        package.lead_layout = Some(vec![6, 7]); // 42 != 48
        let err = generate_bga(&package).unwrap_err();
        assert!(matches!(err, McadError::InvalidLeadLayout { .. }));
    }

    #[test]
    fn missing_nominal_is_an_error_not_an_invented_value() {
        let mut package = bga_6x8();
        package.dimensions.body_width.typ = None;
        let err = generate_bga(&package).unwrap_err();
        assert!(matches!(err, McadError::MissingDimension(_)));
    }
}
