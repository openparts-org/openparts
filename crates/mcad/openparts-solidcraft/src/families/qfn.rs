//! QFN/DFN/LGA: a sawn mold body with lead-frame terminals (flush with
//! the edge, or set in on an LGA) and an optional exposed pad, inlaid in
//! its underside.

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::{LandShape, LeadForm};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let rows = common::perimeter_rows(g);
    if rows.is_empty() {
        return Err(ModelError::NoLeads(input.geometry.family));
    }
    let Some(LeadForm::Flush { thickness }) = g.lead_form else {
        return Err(ModelError::LeadForm {
            family: g.family,
            expected: "flush-terminal",
        });
    };
    common::body_params(s, g)?;
    let (pad_len, pad_w) = rows[0].lead_size();
    let half = if rows[0].side.across_x() {
        g.body.size.x
    } else {
        g.body.size.y
    } / 2.0;
    let inset = (half - rows[0].outer_extent()).max(0.0);
    s.fragment(
        fragment!("qfn"),
        &vars!(
            pad_len = mm(pad_len),
            pad_w = mm(pad_w),
            term_t = mm(thickness.min(g.body.size.z * 0.5)),
            pad_inset = mm(inset)
        ),
    )?;
    common::pin1_dimple(s, g, "Body", "body_h")?;
    let layouts = common::row_layouts(s, &rows, input.geometry.pitch);
    let mut terminals = common::lead_rows(s, &rows, &layouts, fragment!("qfn_pad_row"), "Pad")?;
    if let Some(ep) = g.leads.iter().find(|l| l.number == "EP") {
        s.fragment(
            fragment!("qfn_exposed_pad"),
            &vars!(
                ep_w = mm(ep.size.x),
                ep_l = mm(ep.size.y),
                x = mm(ep.position.x),
                y = mm(ep.position.y)
            ),
        )?;
        terminals.push("ExposedPad".into());
    }
    // An LGA's terminals off the perimeter (a centre pad, a ground ring),
    // each at its land, in its shape.
    if let openparts_mcad::Details::ExtraTerminals(names) = &g.details {
        for (i, lead) in g
            .leads
            .iter()
            .filter(|l| names.contains(&l.number))
            .enumerate()
        {
            let name = format!("Terminal{}", i + 1);
            let (x, y) = (mm(lead.position.x), mm(lead.position.y));
            match lead.shape {
                LandShape::Circle => s.fragment(
                    fragment!("lga_terminal_disc"),
                    &vars!(name = name.clone(), d = mm(lead.size.x), x = x, y = y),
                )?,
                LandShape::Annulus { inner_diameter } => s.fragment(
                    fragment!("lga_terminal_ring"),
                    &vars!(
                        name = name.clone(),
                        d = mm(lead.size.x),
                        d_in = mm(inner_diameter),
                        x = x,
                        y = y
                    ),
                )?,
                // Rectangles, and the extent of a chamfered or polygon land.
                _ => s.fragment(
                    fragment!("lga_terminal_box"),
                    &vars!(
                        name = name.clone(),
                        w = mm(lead.size.x),
                        l = mm(lead.size.y),
                        x = x,
                        y = y
                    ),
                )?,
            }
            terminals.push(name);
        }
    }
    s.fragment(
        fragment!("inlay"),
        &vars!(target = "Body", tools = terminals.clone()),
    )?;
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&terminals, looks::TIN);
    Ok(())
}
