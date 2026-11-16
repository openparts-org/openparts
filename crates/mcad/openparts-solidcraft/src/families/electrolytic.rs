//! `smd_electrolytic` (openparts-mcad's `electrolytic.rs`): a black base
//! plate with its + side corners cut, an aluminium can with a crimp
//! groove and a dark marking over the - terminal, and a flat terminal
//! under the plate on every SMD lead.

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::{Details, Mounting};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let Details::Electrolytic(f) = g.details else {
        return Err(ModelError::MissingDetails(g.family));
    };
    if g.leads.is_empty() {
        return Err(ModelError::NoLeads(g.family));
    }
    common::body_params(s, g)?;
    // The + terminal (from the device's pin names, else pin 1) carries
    // the plate's corner cuts; the marking sits over the other side.
    let plus_left = common::marked_lead(input).is_none_or(|l| l.position.x <= 0.0);
    let (pos, neg) = if plus_left { ("-", "") } else { ("", "-") };
    s.fragment(
        fragment!("smd_electrolytic"),
        &vars!(
            can_d = mm(f.can_diameter),
            term_t = mm(f.terminal_thickness),
            plate_t = mm(f.plate_thickness),
            chamfer = mm(f.chamfer),
            pos = pos,
            neg = neg,
        ),
    )?;
    let mut terminals = Vec::new();
    for (i, l) in g.leads.iter().enumerate() {
        if l.mounting != Mounting::Smd {
            continue;
        }
        let name = format!("Terminal{}", i + 1);
        s.fragment(
            fragment!("led_terminal"),
            &vars!(
                name = name.clone(),
                x = mm(l.position.x),
                y = mm(l.position.y),
                w = mm(l.size.x),
                l = mm(l.size.y),
                h = mm(l.size.z.max(0.05)),
            ),
        )?;
        terminals.push(name);
    }
    s.paint(&["Plate".into()], looks::MOLD);
    s.paint(&["Can".into()], looks::ALUMINIUM);
    s.paint(&["Mark".into()], looks::OVERCOAT);
    s.paint(&terminals, looks::TIN);
    Ok(())
}
