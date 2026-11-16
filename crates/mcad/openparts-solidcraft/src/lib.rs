//! openparts-solidcraft: detailed 3D models through SolidCraft.
//!
//! `MechanicalGeometry` (openparts-mcad) stays the canonical geometry: it
//! fixes where the body and every lead are, and the footprint generators
//! read the same data. This crate turns it, per package family and
//! component category, into a parametric SolidCraft command script that
//! adds what a real part looks like -- drafted mold bodies, bent
//! gull-wing leads, plated terminations, polarity marks, cans and lenses
//! -- and runs it through the locally installed SolidCraft to export
//! STEP (with colours), STL, a `.sc` design or a preview image.
//!
//! The scripts are parametric: shared dimensions are design parameters
//! (`body_w`, `pitch`, `leads_per_side`, `lead_t`, ...) referenced by
//! expressions, so a generated `.sc` design can be edited in SolidCraft
//! and rebuilds consistently. Lead dimensions come from the geometry
//! (datasheet values where the package records them, else
//! openparts-mcad's defaults); purely presentational details (mold draft,
//! bend radii, plating colours, ...) are generator defaults,
//! never written back into Canonical Data.

mod common;
mod families;
pub mod runner;
mod script;
mod template;

pub use runner::{available, available_for_test, run, version, Format, RunError};
pub use script::{looks, mm, Look, Script};

use openparts_core::{ComponentCategory, Device, Package, PackageFamily};
use openparts_mcad::MechanicalGeometry;

/// Everything a detailed model is built from.
#[derive(Debug, Clone, Copy)]
pub struct ModelInput<'a> {
    pub package: &'a Package,
    pub geometry: &'a MechanicalGeometry,
    /// The device's category, when known: a `chip` package is drawn as a
    /// resistor, a capacitor, an LED, ... accordingly.
    pub category: Option<ComponentCategory>,
    /// The pin a polarity mark belongs to (an LED's or diode's cathode,
    /// a polarised capacitor's + terminal), from the device's pin names
    /// -- see [`polarity_pin`]. `None` falls back to pin "1".
    pub marked_pin: Option<&'a str>,
}

impl<'a> ModelInput<'a> {
    /// The input for `device` in `package`: its category and its
    /// polarity pin.
    pub fn for_device(
        package: &'a Package,
        geometry: &'a MechanicalGeometry,
        device: &'a Device,
    ) -> Self {
        ModelInput {
            package,
            geometry,
            category: Some(device.category),
            marked_pin: polarity_pin(device),
        }
    }

    /// The pin the polarity mark sits at: [`Self::marked_pin`], else "1".
    pub(crate) fn mark(&self) -> &str {
        self.marked_pin.unwrap_or("1")
    }
}

/// The pin a part's polarity mark belongs to, read from the datasheet's
/// pin names (never assumed from pin numbers: Kingbright LEDs number the
/// cathode 1, Everlight the anode): the cathode of an LED or diode, the
/// positive (`+`, anode) terminal of a polarised capacitor. `None` when
/// the device has no single such pin.
pub fn polarity_pin(device: &Device) -> Option<&str> {
    device.polarity_pin().map(|(number, _)| number)
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("family \"{0}\": the geometry has no leads where the model needs them")]
    NoLeads(PackageFamily),
    #[error("family \"{family}\": the geometry has no {expected} lead form")]
    LeadForm {
        family: PackageFamily,
        expected: &'static str,
    },
    #[error("family \"{0}\": the geometry lacks the family's 3D details")]
    MissingDetails(PackageFamily),
    #[error(transparent)]
    Template(#[from] template::TemplateError),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Run(#[from] RunError),
}

/// The SolidCraft script for `input`'s detailed model.
pub fn build_script(input: &ModelInput<'_>) -> Result<Script, ModelError> {
    families::build(input)
}

/// Builds the model and exports it in each of `formats`, in order.
pub fn generate(
    input: &ModelInput<'_>,
    product_name: &str,
    formats: &[Format],
) -> Result<Vec<Vec<u8>>, Error> {
    let script = build_script(input)?;
    Ok(run(&script, product_name, formats)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(category: &str, pins: &str) -> Device {
        serde_yaml::from_str(&format!(
            "schema_version: \"0.1\"\nkind: device\nid: t/T\nmanufacturer: t\ncategory: {category}\npins:\n{pins}"
        ))
        .unwrap()
    }

    #[test]
    fn the_polarity_pin_comes_from_pin_names_not_numbers() {
        // Kingbright numbers the cathode 1, Everlight the anode.
        let kingbright = device(
            "led",
            "  \"1\": { name: Cathode, type: passive }\n  \"2\": { name: Anode, type: passive }\n",
        );
        let everlight = device(
            "led",
            "  \"1\": { name: Anode, type: passive }\n  \"2\": { name: Cathode, type: passive }\n",
        );
        assert_eq!(polarity_pin(&kingbright), Some("1"));
        assert_eq!(polarity_pin(&everlight), Some("2"));
        // A polarised capacitor is marked at its + terminal.
        let tantalum = device(
            "capacitor",
            "  \"1\": { name: \"+\", type: passive }\n  \"2\": { name: \"-\", type: passive }\n",
        );
        assert_eq!(polarity_pin(&tantalum), Some("1"));
        // No single matching pin (an MLCC, a dual diode): no mark pin.
        let mlcc = device(
            "capacitor",
            "  \"1\": { name: \"1\", type: passive }\n  \"2\": { name: \"2\", type: passive }\n",
        );
        assert_eq!(polarity_pin(&mlcc), None);
    }
}
