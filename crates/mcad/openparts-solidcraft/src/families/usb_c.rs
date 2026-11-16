//! USB Type-C receptacles (openparts-mcad's `usb_c`): the stadium-section
//! shell with its tongue, the SMD contact feet on their pads and the
//! shell tabs down through their holes.

use crate::common::edge_points;
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};
use openparts_mcad::Mounting;

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    if g.leads.is_empty() {
        return Err(ModelError::NoLeads(input.geometry.family));
    }
    let openparts_mcad::Details::UsbC(f) = g.details else {
        return Err(ModelError::MissingDetails(g.family));
    };
    let b = &g.body;
    let (w, h, y) = (b.size.x, b.size.z, b.position.y);
    let standoff = b.position.z - h / 2.0;
    let t = f.shell_thickness.min(h * 0.2);
    let r = h * 0.45;
    // The four long edges along Y, outside and inside the shell.
    let edges = |hw: f64, z0: f64, z1: f64| {
        edge_points(&[[-hw, y, z0], [hw, y, z0], [-hw, y, z1], [hw, y, z1]])
    };
    crate::common::body_params(s, g)?;
    s.fragment(
        fragment!("usb_c"),
        &vars!(
            y = mm(y),
            standoff = mm(standoff),
            shell_t = mm(t),
            r = mm(r),
            outer_edges = edges(w / 2.0, standoff, standoff + h),
            inner_edges = edges(w / 2.0 - t, standoff + t, standoff + h - t),
        ),
    )?;
    let mut contacts = Vec::new();
    let mut tabs = Vec::new();
    for (i, l) in g.leads.iter().enumerate() {
        match l.mounting {
            Mounting::Smd => {
                let name = format!("Contact{}", i + 1);
                s.fragment(
                    fragment!("usb_c_pad"),
                    &vars!(
                        name = name.clone(),
                        x = mm(l.position.x),
                        y = mm(l.position.y),
                        z = mm(0.05),
                        w = mm(l.size.x * 0.8),
                        l = mm(l.size.y * 0.8),
                    ),
                )?;
                contacts.push(name);
            }
            // Alignment-peg holes: the pegs are part of the housing.
            Mounting::NonPlated => {}
            Mounting::ThroughHole => {
                let name = format!("Tab{}", i + 1);
                let drill = l.drill.unwrap_or(0.6);
                let long = l.slot.unwrap_or(drill) * 0.85;
                let thin = drill * 0.6;
                let (tw, tl) = if l.size.x >= l.size.y {
                    (long, thin)
                } else {
                    (thin, long)
                };
                s.fragment(
                    fragment!("usb_c_tab"),
                    &vars!(
                        name = name.clone(),
                        x = mm(l.position.x),
                        y = mm(l.position.y),
                        w = mm(tw),
                        l = mm(tl),
                        tail = mm(f.tail_length),
                    ),
                )?;
                tabs.push(name);
            }
        }
    }
    s.paint(&["Shell".into()], looks::NICKEL);
    s.paint(&["Tongue".into()], looks::MOLD);
    s.paint(&contacts, looks::GOLD);
    s.paint(&tabs, looks::NICKEL);
    Ok(())
}
