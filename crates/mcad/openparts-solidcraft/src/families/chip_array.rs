//! Chip resistor arrays (`chip_array`): an alumina substrate under a
//! black overcoat, a wrap-around terminal per lead -- convex (proud of the
//! edge) or concave (plated round a half-round notch in it).

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::Details;

/// How far a convex terminal stands proud of the substrate edge (mm).
const PROUD: f64 = 0.03;

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let first = g
        .leads
        .first()
        .ok_or(ModelError::NoLeads(input.geometry.family))?;
    let concave = matches!(g.details, Details::ChipArray { concave: true });
    let b = g.body.size;
    common::body_params(s, g)?;
    let term_len = first.size.x;
    s.fragment(
        fragment!("chip_array"),
        &vars!(
            term_len = mm(term_len),
            overcoat_t = mm((b.z * 0.12).clamp(0.02, 0.05)),
            term_t = mm(0.015)
        ),
    )?;
    let proud = if concave { 0.0 } else { PROUD };
    let mut terminals = Vec::new();
    for (i, lead) in g.leads.iter().enumerate() {
        let name = format!("Terminal{}", i + 1);
        let sign = lead.position.x.signum();
        let len = term_len + proud;
        let x = sign * (b.x / 2.0 + proud - len / 2.0);
        s.fragment(
            fragment!("chip_array_terminal"),
            &vars!(
                name = name.clone(),
                x = mm(x),
                y = mm(lead.position.y),
                w = mm(lead.size.y),
                len = mm(len)
            ),
        )?;
        if concave {
            s.fragment(
                fragment!("chip_array_notch"),
                &vars!(
                    name = name.clone(),
                    x = mm(sign * b.x / 2.0),
                    y = mm(lead.position.y),
                    d = mm((lead.size.y * 0.6).min(term_len * 1.2))
                ),
            )?;
        }
        terminals.push(name);
    }
    s.fragment(
        fragment!("inlay"),
        &vars!(target = "Core", tools = terminals.clone()),
    )?;
    s.paint(&["Core".into()], looks::ALUMINA);
    s.paint(&["Overcoat".into()], looks::OVERCOAT);
    s.paint(&terminals, looks::TIN);
    Ok(())
}
