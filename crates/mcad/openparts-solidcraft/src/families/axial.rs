//! Axial diodes: a can lying on the board, the cathode band at pin 1
//! (KiCad D_DO-*), leads bent down into their holes. DO-35 is glass.

use super::wire_d;
use crate::common::edge_point;
use crate::script::{looks, mm, Look};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let leads = &input.geometry.leads;
    let Some(pin1) = leads.first() else {
        return Err(ModelError::NoLeads(input.package.family.clone()));
    };
    let body = &input.geometry.body;
    let glass = input
        .package
        .id
        .to_string()
        .to_ascii_uppercase()
        .contains("DO35");
    let (hl, d) = (body.size.x / 2.0, body.size.y);
    let cathode_left = pin1.position.x <= 0.0;
    let top = d; // the top of the can's end circles (it rests on z = 0)
    let (band_x, body_x, band_end, body_end) = if cathode_left {
        ("-(can_len / 2)", "-(can_len / 2) + band_w", -hl, hl)
    } else {
        ("(can_len / 2) - (band_w)", "-(can_len / 2)", hl, -hl)
    };
    s.fragment(
        fragment!("axial"),
        &vars!(
            can_len = mm(body.size.x),
            can_d = mm(d),
            pin_d = mm(wire_d(pin1, 0.6)),
            band_x = band_x,
            body_x = body_x,
            band_edge = edge_point([band_end, 0.0, top]),
            body_edge = edge_point([body_end, 0.0, top]),
            end_r = mm(d * 0.15),
        ),
    )?;
    let mut wires = Vec::new();
    for l in leads {
        let name = format!("Lead{}", l.number);
        let start = if l.position.x < 0.0 {
            "-(can_len / 2)"
        } else {
            "can_len / 2"
        };
        s.fragment(
            fragment!("axial_lead"),
            &vars!(
                name = name.clone(),
                y = mm(l.position.y),
                start_x = start,
                x = mm(l.position.x)
            ),
        )?;
        wires.push(name);
    }
    if glass {
        s.paint(
            &["Body".into()],
            Look {
                color: "#e07b24",
                metallic: 0.0,
                roughness: 0.1,
                opacity: 0.7,
            },
        );
        s.paint(&["Band".into()], looks::MOLD);
    } else {
        s.paint(&["Body".into()], looks::MOLD);
        s.paint(&["Band".into()], looks::MARK_LIGHT);
    }
    s.paint(&wires, looks::TIN);
    Ok(())
}
