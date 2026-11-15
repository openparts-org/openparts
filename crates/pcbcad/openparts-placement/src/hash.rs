//! The model's content hash (spec section 13).

use crate::{Basis, PlacementModel};
use sha2::{Digest, Sha256};

/// `sha256:<hex>` of the model's geometry: its JSON with `hash` empty,
/// `basis` cleared and every pad's `pin` left out -- where a number
/// came from, and the device's pin names, don't change the copper.
pub fn content_hash(model: &PlacementModel) -> String {
    let mut m = model.clone();
    m.hash = String::new();
    m.basis = Basis::default();
    for pad in &mut m.pads {
        pad.pin = None;
    }
    let json = serde_json::to_vec(&m).expect("the model serialises");
    let digest = Sha256::digest(&json);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}
