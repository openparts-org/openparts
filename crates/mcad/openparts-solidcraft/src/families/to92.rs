//! TO-92: the D-shaped molding raised on its legs, the staggered middle
//! leg formed back to its hole.

use crate::common::{self, edge_point};
use crate::script::{looks, mm};
use crate::template::{fragment, vars};
use crate::{ModelError, ModelInput, Script};

pub fn build(s: &mut Script, input: &ModelInput<'_>) -> Result<(), ModelError> {
    let g = input.geometry;
    if g.leads.is_empty() {
        return Err(ModelError::NoLeads(input.package.family.clone()));
    }
    let body = &g.body;
    common::body_params(s, g)?;
    s.fragment(
        fragment!("to92"),
        &vars!(
            x = mm(body.position.x),
            y = mm(body.position.y),
            // seat_h (2 mm) + body_h: the top of the round back.
            top_back_edge = edge_point([
                body.position.x,
                body.position.y - body.size.y / 2.0,
                body.size.z + 2.0
            ]),
        ),
    )?;
    let mut legs = Vec::new();
    for l in &g.leads {
        let name = format!("Lead{}", l.number);
        let v = vars!(
            name = name.clone(),
            x = mm(l.position.x),
            y = mm(l.position.y)
        );
        if l.position.y.abs() < 1e-9 {
            s.fragment(fragment!("to92_leg"), &v)?;
        } else {
            s.fragment(fragment!("to92_formed_leg"), &v)?;
        }
        legs.push(name);
    }
    s.paint(&["Body".into()], looks::MOLD);
    s.paint(&legs, looks::TIN);
    Ok(())
}
