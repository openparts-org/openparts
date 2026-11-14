//! Device checks: pin names and internal connections.

use crate::Diagnostic;
use openparts_core::{Device, PinType};

/// Pin types allowed to repeat a name across many pins without it being
/// a data-entry mistake (a package commonly has several `VDD`/`VSS`/`NC`
/// pins). Every other type ("signal" pins) is expected to have a unique
/// name per Device -- a repeat there is usually a copy-paste error.
pub(crate) fn allows_repeated_names(pin_type: PinType) -> bool {
    matches!(
        pin_type,
        PinType::Power | PinType::PowerOut | PinType::Ground | PinType::Nc | PinType::Reserved
    )
}

pub(crate) fn check_duplicate_pin_names(entity: &str, device: &Device) -> Vec<Diagnostic> {
    let mut seen: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for (number, pin) in &device.pins {
        if let Some(target) = &pin.connected_to {
            match device.pins.get(target) {
                Some(_) if target != number => {}
                _ => out.push(Diagnostic::error(
                    "pin.connected_to.invalid",
                    entity,
                    Some(&format!("/pins/{number}/connected_to")),
                    format!("pin \"{number}\" is connected_to \"{target}\", which is not another pin of this device"),
                )),
            }
        }
    }
    // Two pins the source says are internally connected (a tab tied to
    // pin 2) are one terminal and may share its name.
    let linked = |a: &str, b: &str| {
        let to = |x: &str| device.pins.get(x).and_then(|p| p.connected_to.as_deref());
        to(a) == Some(b) || to(b) == Some(a)
    };
    for (number, pin) in &device.pins {
        if allows_repeated_names(pin.pin_type) {
            continue;
        }
        if let Some(&first_number) = seen.get(pin.name.as_str()) {
            if linked(first_number, number) {
                continue;
            }
            out.push(Diagnostic::error(
                "pin.duplicate_name",
                entity,
                Some(&format!("/pins/{number}/name")),
                format!(
                    "pins \"{first_number}\" and \"{number}\" both use the name \"{}\"",
                    pin.name
                ),
            ));
        } else {
            seen.insert(pin.name.as_str(), number.as_str());
        }
    }
    out
}
