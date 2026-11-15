//! openparts-kicad: KiCad adapter behind the openparts-pcbcad IR
//! (Architecture Specification section 13). Renders `.kicad_sym` /
//! `.kicad_mod` text (KiCad 6+ S-expression format). Never reads
//! Canonical YAML directly -- only `PcbSymbol`/`PcbFootprint`.

mod footprint;
pub mod library;
pub mod placement;
mod symbol;

pub use footprint::{
    parse_footprint_model, parse_footprint_pads, parse_placement_hash, render_footprint,
    ParsedModel, ParsedPad, PLACEMENT_HASH_PROPERTY,
};
pub use symbol::{parse_symbol_pins, render_symbol, ParsedPin};
