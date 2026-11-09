//! BGA: solder balls on a laminate substrate under a drafted mold cap
//! (`templates/bga.yaml`); the grid comes from `lead_layout`.

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let Some(a1) = g
        .leads
        .iter()
        .find(|l| l.number == "A1")
        .or(g.leads.first())
    else {
        return Err(ModelError::NoLeads(input.package.family.clone()));
    };
    let (rows, cols) = match input.package.lead_layout.as_deref() {
        Some([r, c]) => (*r, *c),
        _ => (1, g.leads.len() as u32),
    };
    common::body_params(s, g)?;
    s.fragment(
        fragment!("bga"),
        &vars!(
            pitch = mm(input.package.pitch.nominal.unwrap_or(0.8)),
            rows = rows,
            cols = cols,
            ball_d = mm(a1.size.x * 1.25),
        ),
    )?;
    s.paint(&["Substrate".into()], looks::LAMINATE);
    s.paint(&["MoldCap".into()], looks::MOLD);
    s.paint(&common::copies("Ball", rows * cols), looks::TIN);
    Ok(())
}
