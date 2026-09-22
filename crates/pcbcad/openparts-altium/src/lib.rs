//! openparts-altium: Altium adapter (Architecture Specification
//! section 12/13).
//!
//! TODO: deliberately deferred, unlike openparts-kicad/openparts-librepcb.
//! Altium's native SchLib/PcbLib format is a proprietary binary (OLE
//! compound file) -- there is no text-based interchange format to hand-
//! write the way KiCad's S-expressions or LibrePCB's `.lp` files allow,
//! and no Altium installation exists anywhere to verify output against
//! even if we guessed at the binary layout.
//!
//! Planned approach for whoever picks this up: Altium's scripting engine
//! (DelphiScript/Pascal) can build library items programmatically when
//! run *inside* Altium -- analogous to how openparts-step generates STEP
//! without a CAD library, this crate would emit a `.pas` script (driven
//! by the same `PcbSymbol`/`PcbFootprint` IR every other adapter uses)
//! for the user to run inside their own Altium install, rather than
//! writing a `.SchLib`/`.PcbLib` file directly. That still requires a
//! real Altium install to ever verify against, so it wasn't built in
//! this pass -- only documented here so the decision isn't lost.

use openparts_pcbcad::{PcbFootprint, PcbSymbol};

#[derive(Debug, thiserror::Error)]
pub enum AltiumError {
    #[error("openparts-altium is not implemented yet")]
    NotImplemented,
}

pub fn render_symbol(_symbol: &PcbSymbol) -> Result<String, AltiumError> {
    Err(AltiumError::NotImplemented)
}

pub fn render_footprint(_footprint: &PcbFootprint) -> Result<String, AltiumError> {
    Err(AltiumError::NotImplemented)
}
