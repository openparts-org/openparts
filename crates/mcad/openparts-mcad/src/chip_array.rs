//! `chip_array`: chip resistor arrays and networks (Yageo YC124/YC164,
//! Panasonic EXB) -- Canonical Data Specification section 20.15.

use crate::defaults;
use crate::{Body, BodyShape, Details, Lead, Marker, MarkerKind, McadError, MechanicalGeometry};
use crate::{Mounting, Point3, Size3};
use openparts_core::{Package, TerminalShape};

/// A chip body with `lead_count / 2` terminals in each of two rows along
/// its long sides, as SOIC: `body_width` (X) is across the rows,
/// `body_length` (Y) along them; pin 1 at the top of the left row, down
/// the left row, up the right one (element i joins pin i and pin
/// `lead_count + 1 - i`). Terminals are `lead_length` in from the body
/// edge (X) by `lead_width` (Y), centred on each row at `pitch`. The
/// footprint is the source's land pattern (`pads`, required).
pub fn generate_chip_array(package: &Package) -> Result<MechanicalGeometry, McadError> {
    let n = package.lead_count;
    if n < 4 || !n.is_multiple_of(2) {
        return Err(McadError::InvalidLeadCount(n));
    }
    if package.pads.is_empty() {
        return Err(McadError::InconsistentDimensions(
            "a chip array's footprint is its land pattern: state `pads`".into(),
        ));
    }
    let per_row = n / 2;
    let pitch = defaults::dim(&package.pitch, "pitch.typ")?;
    let body_w = defaults::dim(&package.dimensions.body_width, "dimensions.body_width.typ")?;
    let body_l = defaults::dim(
        &package.dimensions.body_length,
        "dimensions.body_length.typ",
    )?;
    let body_h = defaults::stated(&package.dimensions.body_height)
        .unwrap_or(defaults::chip_array::BODY_HEIGHT);
    let term_len = defaults::value(&package.dimensions.lead_length)
        .unwrap_or(body_w * defaults::chip_array::TERMINAL_RATIO);
    let term_w = defaults::value(&package.dimensions.lead_width)
        .unwrap_or(pitch * defaults::chip_array::TERMINAL_WIDTH_RATIO);
    let span = (per_row as f64 - 1.0) * pitch;
    if term_len <= 0.0 || 2.0 * term_len >= body_w {
        return Err(McadError::InconsistentDimensions(format!(
            "terminals {term_len} long from each side leave nothing of the {body_w} body between the rows"
        )));
    }
    if term_w <= 0.0 || term_w >= pitch || span + term_w > body_l + 1e-9 {
        return Err(McadError::InconsistentDimensions(format!(
            "{per_row} terminals {term_w} wide at pitch {pitch} don't fit the {body_l} body length"
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
    let x = body_w / 2.0 - term_len / 2.0;
    let lead = |number: u32, x: f64, y: f64| Lead {
        number: number.to_string(),
        position: Point3 {
            x,
            y,
            z: body_h / 2.0,
        },
        size: Size3 {
            x: term_len,
            y: term_w,
            z: body_h,
        },
        mounting: Mounting::Smd,
        drill: None,
        slot: None,
        shape: crate::LandShape::Rect,
        side: openparts_core::PadSide::Top,
    };
    let mut leads = Vec::with_capacity(n as usize);
    for i in 0..per_row {
        leads.push(lead(i + 1, -x, span / 2.0 - i as f64 * pitch));
    }
    for i in 0..per_row {
        leads.push(lead(per_row + i + 1, x, -span / 2.0 + i as f64 * pitch));
    }
    Ok(MechanicalGeometry {
        family: package.family,
        lands: vec![],
        pitch: Some(pitch),
        details: Details::ChipArray {
            concave: package.terminal_shape == Some(TerminalShape::Concave),
        },
        body,
        leads,
        markers: vec![Marker {
            kind: MarkerKind::Pin1Dot,
            position: Point3 {
                x: -body_w / 2.0 + term_len,
                y: body_l / 2.0,
                z: body_h,
            },
        }],
        lead_form: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::generate;
    use openparts_core::Package;

    /// Yageo YC124 (4 x 0402 elements, 2.0 x 1.0 mm, pitch 0.5), its
    /// recommended land pattern.
    pub(crate) const YC124: &str = "schema_version: '0.1'\nkind: package\nid: standards/YC124\n\
        family: chip_array\nlead_count: 8\nterminal_shape: convex\n\
        pitch: {nominal: 0.5, unit: mm}\ndimensions:\n\
        \x20 body_width: {nominal: 1.0, unit: mm}\n\
        \x20 body_length: {nominal: 2.0, unit: mm}\n\
        \x20 body_height: {nominal: 0.35, unit: mm}\n\
        \x20 lead_width: {nominal: 0.3, unit: mm}\n\
        \x20 lead_length: {nominal: 0.2, unit: mm}\n\
        pads:\n\
        \x20 - {number: '1', x: -0.5, y: 0.75, width: 0.5, length: 0.3}\n\
        \x20 - {number: '2', x: -0.5, y: 0.25, width: 0.5, length: 0.3}\n\
        \x20 - {number: '3', x: -0.5, y: -0.25, width: 0.5, length: 0.3}\n\
        \x20 - {number: '4', x: -0.5, y: -0.75, width: 0.5, length: 0.3}\n\
        \x20 - {number: '5', x: 0.5, y: -0.75, width: 0.5, length: 0.3}\n\
        \x20 - {number: '6', x: 0.5, y: -0.25, width: 0.5, length: 0.3}\n\
        \x20 - {number: '7', x: 0.5, y: 0.25, width: 0.5, length: 0.3}\n\
        \x20 - {number: '8', x: 0.5, y: 0.75, width: 0.5, length: 0.3}\n";

    #[test]
    fn terminals_run_down_the_left_row_and_up_the_right() {
        let p: Package = serde_yaml::from_str(YC124).unwrap();
        let g = generate(&p).unwrap();
        assert_eq!(g.leads.len(), 8);
        let at = |n: &str| g.leads.iter().find(|l| l.number == n).unwrap().position;
        // Pin 1 top-left, pin 8 across from it (element 1 is 1-8).
        assert!((at("1").x + 0.4).abs() < 1e-9 && (at("1").y - 0.75).abs() < 1e-9);
        assert!((at("8").x - 0.4).abs() < 1e-9 && (at("8").y - 0.75).abs() < 1e-9);
        assert!((at("4").y + 0.75).abs() < 1e-9 && (at("5").y + 0.75).abs() < 1e-9);
        // The footprint is the land pattern.
        assert_eq!(g.lands.len(), 8);
        assert!((g.lands[0].size.x - 0.5).abs() < 1e-9);
        assert_eq!(g.details, crate::Details::ChipArray { concave: false });
    }

    #[test]
    fn a_chip_array_needs_pads_an_even_count_and_room_for_its_terminals() {
        let p: Package = serde_yaml::from_str(YC124).unwrap();
        let mut no_pads = p.clone();
        no_pads.pads.clear();
        assert!(generate(&no_pads).is_err());
        let mut odd = p.clone();
        odd.lead_count = 7;
        assert!(generate(&odd).is_err());
        let mut tight = p.clone();
        tight.pitch.typ = Some(0.6);
        assert!(generate(&tight).is_err(), "4 x 0.6 pitch overruns 2.0");
        let mut concave = p;
        concave.terminal_shape = Some(openparts_core::TerminalShape::Concave);
        assert_eq!(
            generate(&concave).unwrap().details,
            crate::Details::ChipArray { concave: true }
        );
    }
}
