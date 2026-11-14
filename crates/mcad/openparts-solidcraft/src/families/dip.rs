//! DIP and SIP: molded bodies on shouldered through-hole legs.

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};

pub fn dip(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let rows = common::perimeter_rows(g);
    if rows.is_empty() {
        return Err(ModelError::NoLeads(input.geometry.family));
    }
    let row_span = 2.0 * rows[0].leads[0].position.x.abs();
    // Declared before body_params, so this molding width is the body_w.
    s.fragment(
        fragment!("dip"),
        &vars!(
            row_span = mm(row_span),
            mold_w = mm(g.body.size.x.min(row_span - 1.27))
        ),
    )?;
    common::body_params(s, g)?;
    common::molded_body(s, 0.5, 0.45, 7.0)?;
    common::pin1_dimple(s, g, "Body", "standoff + body_h")?;
    s.fragment(fragment!("dip_details"), &vars!())?;
    let layouts = common::row_layouts(s, &rows, input.geometry.pitch);
    let legs = common::lead_rows(s, &rows, &layouts, fragment!("dip_leg_row"), "Leg")?;
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&legs, looks::TIN);
    Ok(())
}

pub fn sip(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let Some(first) = g.leads.first() else {
        return Err(ModelError::NoLeads(input.geometry.family));
    };
    common::body_params(s, g)?;
    common::molded_body(s, 0.8, 0.5, 4.0)?;
    common::pin1_dimple(s, g, "Body", "standoff + body_h")?;
    let n = g.leads.len() as u32;
    s.fragment(
        fragment!("sip"),
        &vars!(
            pitch = mm(input.geometry.pitch.unwrap_or(2.54)),
            count = n,
            x = mm(first.position.x),
            y = mm(first.position.y),
        ),
    )?;
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&common::copies("Leg", n), looks::TIN);
    Ok(())
}
