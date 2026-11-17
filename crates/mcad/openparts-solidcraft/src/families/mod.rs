//! One builder per package family. Each picks templates and fills their
//! variables from the geometry; the shapes are in `templates/`.

mod axial;
mod bga;
mod chip;
mod chip_array;
mod connector;
mod diode;
mod dip;
mod electrolytic;
mod gullwing;
mod header;
mod led;
mod melf;
mod module;
mod module_rows;
mod qfn;
mod radial;
mod switch;
mod tabbed;
mod to220;
mod to92;
mod usb_c;

use crate::{ModelError, ModelInput, Script};
use openparts_mcad::Lead;

pub fn build(input: &ModelInput<'_>) -> Result<Script, ModelError> {
    let mut s = Script::new();
    use openparts_mcad::PackageFamily as F;
    // Exhaustive: a new family needs a model builder before this compiles.
    match input.geometry.family {
        F::Lqfp => gullwing::build(&mut s, input, &gullwing::QFP)?,
        F::Soic => gullwing::build(&mut s, input, &gullwing::SOIC)?,
        F::Tssop | F::Msop => gullwing::build(&mut s, input, &gullwing::THIN_SOP)?,
        F::Ssop => gullwing::build(&mut s, input, &gullwing::SSOP)?,
        F::Sot => gullwing::build(&mut s, input, &gullwing::SOT)?,
        F::Sot223 => tabbed::sot223(&mut s, input)?,
        F::Dpak | F::D2pak => tabbed::dpak(&mut s, input)?,
        F::Sod => diode::sod(&mut s, input)?,
        F::SodFlat => diode::sod_flat(&mut s, input)?,
        F::Do214 => diode::do214(&mut s, input)?,
        F::To220 => to220::build(&mut s, input)?,
        F::TactSwitchSmd | F::TactSwitchTht => switch::build(&mut s, input)?,
        F::UsbC => usb_c::build(&mut s, input)?,
        F::CardSocket
        | F::BarrelJack
        | F::TerminalBlock
        | F::FpcConnector
        | F::RfCoax
        | F::MicroUsbB
        | F::MiniUsbB
        | F::UsbB
        | F::Relay
        | F::Buzzer
        | F::Trimmer
        | F::RotaryEncoder
        | F::Potentiometer
        | F::SlidePotentiometer => connector::build(&mut s, input)?,
        F::SmdLed => led::build(&mut s, input)?,
        F::SmdElectrolytic => electrolytic::build(&mut s, input)?,
        F::Melf => melf::build(&mut s, input)?,
        F::Qfn | F::Dfn | F::Son | F::Lga => qfn::build(&mut s, input)?,
        F::Chip => chip::build(&mut s, input)?,
        F::ChipArray => chip_array::build(&mut s, input)?,
        F::Radial => radial::build(&mut s, input)?,
        F::Axial => axial::build(&mut s, input)?,
        F::To92 => to92::build(&mut s, input)?,
        F::Dip => dip::dip(&mut s, input)?,
        F::Sip => dip::sip(&mut s, input)?,
        F::Bga => bga::build(&mut s, input)?,
        F::Module => module::build(&mut s, input)?,
        F::Header | F::Socket | F::ShroudedHeader | F::RightAngleHeader => {
            header::build(&mut s, input)?
        }
    }
    Ok(s)
}

/// Wire diameter for a lead drilled `drill` wide (typical hole clearance).
fn wire_d(lead: &Lead, default: f64) -> f64 {
    lead.drill.map_or(default, |d| (d * 0.68).max(0.3))
}
