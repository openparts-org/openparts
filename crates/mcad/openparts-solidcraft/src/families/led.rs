//! `smd_led` (openparts-mcad's `led.rs`): a white housing with a round
//! emitting window -- a clear lens over the dies -- and a terminal on
//! every SMD lead.

use crate::common;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::Mounting;

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    if g.leads.is_empty() {
        return Err(ModelError::NoLeads(g.family));
    }
    let b = &g.body;
    common::body_params(s, g)?;
    s.fragment(
        fragment!("connector_params"),
        &vars!(z0 = mm(b.position.z - b.size.z / 2.0)),
    )?;
    // The window: most of the smaller body side (a 5050's round cavity).
    let window_d = b.size.x.min(b.size.y) * 0.8;
    s.fragment(fragment!("smd_led"), &vars!(window_d = mm(window_d)))?;
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
    s.paint(&["Housing".into()], looks::LED_HOUSING);
    s.paint(&["Lens".into()], looks::LENS);
    s.paint(&["Die".into()], looks::DIE);
    s.paint(&terminals, looks::TIN);
    Ok(())
}
