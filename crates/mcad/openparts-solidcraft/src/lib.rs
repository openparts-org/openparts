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

pub use runner::{available, run, version, Format, RunError};
pub use script::{looks, mm, Look, Script};

use openparts_core::{ComponentCategory, Package};
use openparts_mcad::MechanicalGeometry;

/// Everything a detailed model is built from.
#[derive(Debug, Clone, Copy)]
pub struct ModelInput<'a> {
    pub package: &'a Package,
    pub geometry: &'a MechanicalGeometry,
    /// The device's category, when known: a `chip` package is drawn as a
    /// resistor, a capacitor, an LED, ... accordingly.
    pub category: Option<ComponentCategory>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("package family \"{0}\" has no SolidCraft model builder")]
    UnsupportedFamily(String),
    #[error("geometry has no leads where family \"{0}\" needs them")]
    NoLeads(String),
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
