//! QFN/DFN: a sawn mold body with flush lead-frame terminals and an
//! optional exposed pad, inlaid in its underside.

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    let rows = common::perimeter_rows(g);
    if rows.is_empty() {
        return Err(ModelError::NoLeads(input.package.family.clone()));
    }
    common::body_params(s, g)?;
    let (pad_len, pad_w) = rows[0].lead_size();
    s.fragment(
        fragment!("qfn"),
        &vars!(pad_len = mm(pad_len), pad_w = mm(pad_w)),
    )?;
    common::pin1_dimple(s, g, "Body", "body_h")?;
    let layouts = common::row_layouts(s, &rows, input.package.pitch.nominal);
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
    s.fragment(
        fragment!("inlay"),
        &vars!(target = "Body", tools = terminals.clone()),
    )?;
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&terminals, looks::TIN);
    Ok(())
}
