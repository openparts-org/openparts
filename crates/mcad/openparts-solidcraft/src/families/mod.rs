//! One builder per package family. Each picks templates and fills their
//! variables from the geometry; the shapes are in `templates/`.

mod axial;
mod bga;
mod chip;
mod dip;
mod gullwing;
mod header;
mod module;
mod qfn;
mod radial;
mod tabbed;
mod to92;

use crate::{ModelError, ModelInput, Script};
use openparts_mcad::Lead;

pub fn build(input: &ModelInput<'_>) -> Result<Script, ModelError> {
    let mut s = Script::new();
    match input.package.family.to_ascii_lowercase().as_str() {
        "lqfp" | "tqfp" => gullwing::build(&mut s, input, &gullwing::QFP)?,
        "soic" => gullwing::build(&mut s, input, &gullwing::SOIC)?,
        "tssop" | "msop" => gullwing::build(&mut s, input, &gullwing::THIN_SOP)?,
        "ssop" => gullwing::build(&mut s, input, &gullwing::SSOP)?,
        "sot223" => tabbed::sot223(&mut s, input)?,
        "dpak" | "to252" => tabbed::dpak(&mut s, input)?,
        "sot" => gullwing::build(&mut s, input, &gullwing::SOT)?,
        "qfn" => qfn::build(&mut s, input)?,
        "chip" => chip::build(&mut s, input)?,
        "radial" => radial::build(&mut s, input)?,
        "axial" => axial::build(&mut s, input)?,
        "to92" => to92::build(&mut s, input)?,
        "dip" => dip::dip(&mut s, input)?,
        "sip" => dip::sip(&mut s, input)?,
        "bga" => bga::build(&mut s, input)?,
        "module" => module::build(&mut s, input)?,
        "header" | "socket" => header::build(&mut s, input)?,
        other => return Err(ModelError::UnsupportedFamily(other.to_string())),
    }
    Ok(s)
}

/// Wire diameter for a lead drilled `drill` wide (typical hole clearance).
fn wire_d(lead: &Lead, default: f64) -> f64 {
    lead.drill.map_or(default, |d| (d * 0.68).max(0.3))
}
