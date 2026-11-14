//! TO-220, vertical: the molding in front of its metal tab (mounting hole
//! through both), straight leads down through the board.

use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    if g.leads.is_empty() {
        return Err(ModelError::NoLeads(input.geometry.family));
    }
    let openparts_mcad::Details::To220(f) = g.details else {
        return Err(ModelError::MissingDetails(g.family));
    };
    let b = &g.body;
    crate::common::body_params(s, g)?;
    s.fragment(
        fragment!("to220"),
        &vars!(
            y = mm(b.position.y),
            standoff = mm(b.position.z - b.size.z / 2.0),
            lead_w = mm(f.lead_width),
            lead_t = mm(f.lead_thickness),
            tail = mm(f.tail_length),
            tab_t = mm(f.tab_thickness.min(b.size.y * 0.5)),
            tab_top = mm(f.overall_height),
            hole_d = mm(f.hole_diameter),
            hole_inset = mm(f.hole_inset),
        ),
    )?;
    let mut legs = Vec::new();
    for l in &g.leads {
        let name = format!("Lead{}", l.number);
        s.fragment(
            fragment!("to220_leg"),
            &vars!(name = name.clone(), x = mm(l.position.x)),
        )?;
        legs.push(name);
    }
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&["Tab".into()], looks::TIN);
    s.paint(&legs, looks::TIN);
    Ok(())
}
