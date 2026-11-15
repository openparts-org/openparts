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

/// Differential pairs (Canonical Data Specification section 14.2): the
/// partner is another pin of the Device whose own `diff_pair` points
/// back, with the opposite polarity, the same standard and the same
/// alternate function (which must be one of each pin's
/// `alternate_functions`); and the pair is cited at `/pins/<n>/diff_pair`
/// (or `/pins/<n>`).
pub(crate) fn check_diff_pairs(entity: &str, device: &Device) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (number, pin) in &device.pins {
        let Some(pair) = &pin.diff_pair else {
            continue;
        };
        let path = format!("/pins/{number}/diff_pair");
        let mut bad = |why: String| {
            out.push(Diagnostic::error(
                "pin.diff_pair.invalid",
                entity,
                Some(&path),
                format!("pin \"{number}\": {why}"),
            ))
        };
        if let Some(f) = &pair.function {
            if !pin.alternate_functions.iter().any(|a| a == f) {
                bad(format!(
                    "diff_pair function \"{f}\" is not one of the pin's alternate_functions"
                ));
            }
        }
        if let Some(z) = pair.impedance_ohm {
            if !(z.is_finite() && z > 0.0) {
                bad(format!("impedance_ohm {z} is not a positive number"));
            }
        }
        match device.pins.get(&pair.partner) {
            None => bad(format!(
                "diff_pair partner \"{}\" is not a pin of this device",
                pair.partner
            )),
            Some(_) if pair.partner == *number => bad("diff_pair partner is the pin itself".into()),
            Some(other) => match &other.diff_pair {
                None => bad(format!(
                    "partner \"{}\" has no diff_pair pointing back",
                    pair.partner
                )),
                Some(back) => {
                    if back.partner != *number {
                        bad(format!(
                            "partner \"{}\" names \"{}\" as its partner, not this pin",
                            pair.partner, back.partner
                        ));
                    }
                    if back.polarity != pair.polarity.opposite() {
                        bad(format!(
                            "partner \"{}\" has the same polarity",
                            pair.partner
                        ));
                    }
                    if back.standard != pair.standard {
                        bad(format!(
                            "partner \"{}\" names another standard",
                            pair.partner
                        ));
                    }
                    if back.function.is_some() != pair.function.is_some() {
                        bad(format!(
                            "only one of this pin and partner \"{}\" names a function",
                            pair.partner
                        ));
                    }
                }
            },
        }
        let cited = device
            .provenance
            .keys()
            .any(|k| *k == path || *k == format!("/pins/{number}"));
        if !cited {
            out.push(Diagnostic::error(
                "provenance.missing",
                entity,
                Some(&path),
                format!("\"{path}\" has no provenance entry (at it or at /pins/{number})"),
            ));
        }
    }
    out
}
