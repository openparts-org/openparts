//! LibrePCB requires a real UUID on every element (library, symbol,
//! each pin, package, each pad, component, each signal, device). Random
//! UUIDs would break Reproducibility (Architecture Specification
//! section 4.9: the same input must produce the same output), so every
//! UUID here is derived deterministically with UUID v5 (name-based,
//! SHA-1) from a fixed OpenParts namespace plus a stable string built
//! from the input data (MPN, pin number, ...). Re-running generation on
//! the same Part/Device/Package always produces byte-identical files.

use uuid::Uuid;

/// Fixed, arbitrary namespace UUID owned by this project. Any valid v4
/// UUID works as a namespace; what matters is that it never changes
/// (changing it would change every derived UUID and break
/// Reproducibility for existing libraries).
const OPENPARTS_NAMESPACE: Uuid = Uuid::from_bytes([
    0x4f, 0x70, 0x65, 0x6e, 0x50, 0x61, 0x72, 0x74, 0x73, 0x2d, 0x4c, 0x50, 0x2d, 0x76, 0x31, 0x00,
]);

pub fn derive(name: &str) -> Uuid {
    Uuid::new_v5(&OPENPARTS_NAMESPACE, name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_deterministic() {
        assert_eq!(derive("sym:FOO"), derive("sym:FOO"));
    }

    #[test]
    fn differs_by_input() {
        assert_ne!(derive("sym:FOO"), derive("sym:BAR"));
    }
}
