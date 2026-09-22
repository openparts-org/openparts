//! openparts-kicad: KiCad adapter behind the openparts-pcbcad IR
//! (Architecture Specification section 13). Renders `.kicad_sym` /
//! `.kicad_mod` text (KiCad 6+ S-expression format). Never reads
//! Canonical YAML directly -- only `PcbSymbol`/`PcbFootprint`.

mod footprint;
mod symbol;

pub use footprint::{parse_footprint_pads, render_footprint, ParsedPad};
pub use symbol::{parse_symbol_pins, render_symbol, ParsedPin};
