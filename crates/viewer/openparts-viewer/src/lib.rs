//! openparts-viewer: the web site's part viewer, compiled to WebAssembly.
//!
//! - A 3D view drawn with wgpu (WebGPU, or WebGL2 where WebGPU isn't
//!   available) of the part's 3MF model (STL when the 3MF fails), with an
//!   orbit camera, a millimetre grid on the board plane, and
//! - datasheet dimensions (`dims`): dimension lines on the model, anchored
//!   on the same Mechanical Geometry the model was generated from, their
//!   labels HTML over the canvas, each showing its provenance on click;
//! - a measure tool (`snap`): two clicked points' distance and ΔX/ΔY/ΔZ,
//!   snapping to vertices, edge midpoints and pad centres;
//! - the footprint (`footprint`): openparts-pcbcad's SVG with pad size,
//!   pitch, pad span and courtyard dimensions, and the measure tool.
//!
//! Everything but `web` (the browser side) is plain Rust and tested
//! natively; `web` exists only on wasm32.

pub mod camera;
pub mod dims;
pub mod footprint;
pub mod format;
pub mod labels;
pub mod mesh;
pub mod snap;

#[cfg(target_arch = "wasm32")]
mod web;
