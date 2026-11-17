//! Axial diodes: a can lying on the board, the cathode band at the
//! cathode (from the device's pin names, else pin 1)
//! (KiCad D_DO-*), leads bent down into their holes. A package whose
//! source states a glass body (`construction: glass`, e.g. DO-35) is
//! drawn as tinted glass with a dark band; anything else as a molding.

use super::wire_d;
use crate::common::{self, edge_point};
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_core::PackageConstruction;

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let leads = &input.geometry.leads;
    let Some(pin1) = leads.first() else {
        return Err(ModelError::NoLeads(input.geometry.family));
    };
    let body = &input.geometry.body;
    let glass = input.package.construction == Some(PackageConstruction::Glass);
    let (hl, d) = (body.size.x / 2.0, body.size.y);
    let cathode_left = common::marked_lead(input).is_none_or(|l| l.position.x <= 0.0);
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
        s.paint(&["Body".into()], looks::GLASS);
        s.paint(&["Band".into()], looks::MOLD);
    } else {
        s.paint(&["Body".into()], looks::MOLD);
        s.paint(&["Band".into()], looks::MARK_LIGHT);
    }
    s.paint(&wires, looks::TIN);
    Ok(())
}
