//! openparts-bom: BOM checking and alternative-part suggestions over an
//! OpenParts dataset, shared by openparts-server (`POST /v1/bom/check`,
//! `GET /v1/parts/{manufacturer}/{mpn}/alternatives`) and the CLI
//! (`openparts bom check`).
//!
//! - [`parse`]: KiCad BOM CSV, KiCad netlist and generic CSV input;
//! - [`matcher`]: BOM lines -> parts, with lifecycle, coverage and
//!   footprint checks;
//! - [`alternatives`]: drop-in / footprint-compatible / functional
//!   replacements, ranked;
//! - [`rules`], [`pins`], [`footprint`]: the parameter, pinout and land
//!   pattern comparisons they are built from.
//!
//! The rules are documented in docs/spec/alternatives-and-bom.md.

pub mod alternatives;
pub mod footprint;
pub mod matcher;
pub mod parse;
pub mod pins;
pub mod rules;

pub use alternatives::{Alternative, AlternativeClass, AlternativesIndex, AlternativesResponse};
pub use matcher::{BomIndex, BomReport};
pub use parse::{decode, parse_bom, BomError, BomFormat, ParsedBom, MAX_BYTES, MAX_ROWS};
