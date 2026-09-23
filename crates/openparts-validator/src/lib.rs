//! openparts-validator: cross-entity Reference / Engineering-consistency /
//! Provenance-structure checks (Testing and Quality Specification
//! section 6). Structural checks (required fields, types, `kind`,
//! duplicate YAML keys) already happen at the Syntax/Schema layer in
//! `openparts-data`'s loaders and are not repeated here.
//!
//! Every check returns a stable, documented [`Diagnostic`] rather than
//! failing fast (Testing and Quality Specification section 5): callers
//! see every problem in one pass, and rule IDs stay meaningful across
//! releases.
//!
//! # Rule ID table
//!
//! | Rule ID | Meaning |
//! |---|---|
//! | `ref.device_mismatch` | `part.device` does not match the loaded Device's `id` |
//! | `ref.package_mismatch` | `part.package` does not match the loaded Package's `id` |
//! | `ref.manufacturer_mismatch` | `part.manufacturer` does not match the Device's `manufacturer` |
//! | `ref.unknown_source` | a `sources` or `provenance` entry references a Source that was not supplied |
//! | `pin.duplicate_name` | two different pin numbers on one Device share the same *signal* pin name (power/ground/nc/reserved pins are exempt -- repeating `VDD`/`VSS`/`NC` across many pins is normal) |
//! | `dimension.out_of_range` | `nominal` is not within `[min, max]` when all three are present |
//! | `provenance.invalid_path` | a provenance key is not a JSON Pointer-shaped path (must start with `/`) |
//! | `mpn.empty` | `part.mpn` is empty |

use openparts_core::{Device, Package, Part, PinType, ProvenanceMap, SourceId};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub rule_id: &'static str,
    pub severity: Severity,
    /// e.g. "part:ex/EX1", "device:ex/DEV1".
    pub entity: String,
    /// JSON-Pointer-shaped path to the offending field, when the
    /// diagnostic is about a single field rather than the whole entity.
    pub field_path: Option<String>,
    pub message: String,
}

impl Diagnostic {
    fn error(
        rule_id: &'static str,
        entity: impl Into<String>,
        field_path: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            rule_id,
            severity: Severity::Error,
            entity: entity.into(),
            field_path: field_path.map(str::to_string),
            message: message.into(),
        }
    }
}

/// Runs every check for the vertical slice against one Part + its
/// resolved Device + Package. `known_sources` is the set of Source IDs
/// the caller actually loaded and can vouch for; any `sources`/
/// `provenance` reference outside that set is reported, never assumed
/// valid.
pub fn validate_part(
    part: &Part,
    device: &Device,
    package: &Package,
    known_sources: &BTreeSet<SourceId>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let part_entity = format!("part:{}", part.id);
    let device_entity = format!("device:{}", device.id);

    if part.device != device.id {
        out.push(Diagnostic::error(
            "ref.device_mismatch",
            &part_entity,
            Some("/device"),
            format!(
                "part.device ({}) does not match the loaded device's id ({})",
                part.device, device.id
            ),
        ));
    }

    if part.package != package.id {
        out.push(Diagnostic::error(
            "ref.package_mismatch",
            &part_entity,
            Some("/package"),
            format!(
                "part.package ({}) does not match the loaded package's id ({})",
                part.package, package.id
            ),
        ));
    }

    if part.manufacturer != device.manufacturer {
        out.push(Diagnostic::error(
            "ref.manufacturer_mismatch",
            &part_entity,
            Some("/manufacturer"),
            format!(
                "part.manufacturer ({}) does not match device.manufacturer ({})",
                part.manufacturer, device.manufacturer
            ),
        ));
    }

    if part.mpn.trim().is_empty() {
        out.push(Diagnostic::error(
            "mpn.empty",
            &part_entity,
            Some("/mpn"),
            "part.mpn is empty",
        ));
    }

    for source in &part.sources {
        if !known_sources.contains(source) {
            out.push(Diagnostic::error(
                "ref.unknown_source",
                &part_entity,
                Some("/sources"),
                format!(
                    "part references source \"{source}\" which was not supplied for validation"
                ),
            ));
        }
    }

    out.extend(check_provenance(
        &part_entity,
        &part.provenance,
        known_sources,
    ));
    out.extend(check_provenance(
        &device_entity,
        &device.provenance,
        known_sources,
    ));
    out.extend(check_provenance(
        &format!("package:{}", package.id),
        &package.provenance,
        known_sources,
    ));

    out.extend(check_duplicate_pin_names(&device_entity, device));

    let package_entity = format!("package:{}", package.id);
    out.extend(check_dimension(&package_entity, "/pitch", &package.pitch));
    out.extend(check_dimension(
        &package_entity,
        "/dimensions/body_width",
        &package.dimensions.body_width,
    ));
    out.extend(check_dimension(
        &package_entity,
        "/dimensions/body_length",
        &package.dimensions.body_length,
    ));
    if let Some(height) = &package.dimensions.body_height {
        out.extend(check_dimension(
            &package_entity,
            "/dimensions/body_height",
            height,
        ));
    }

    out
}

/// Pin types allowed to repeat a name across many pins without it being
/// a data-entry mistake (a package commonly has several `VDD`/`VSS`/`NC`
/// pins). Every other type ("signal" pins) is expected to have a unique
/// name per Device -- a repeat there is usually a copy-paste error.
fn allows_repeated_names(pin_type: PinType) -> bool {
    matches!(
        pin_type,
        PinType::Power | PinType::Ground | PinType::Nc | PinType::Reserved
    )
}

fn check_duplicate_pin_names(entity: &str, device: &Device) -> Vec<Diagnostic> {
    let mut seen: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for (number, pin) in &device.pins {
        if allows_repeated_names(pin.pin_type) {
            continue;
        }
        if let Some(&first_number) = seen.get(pin.name.as_str()) {
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

fn check_dimension(
    entity: &str,
    field_path: &str,
    dim: &openparts_core::Dimension,
) -> Option<Diagnostic> {
    let (Some(nominal), Some(min), Some(max)) = (dim.nominal, dim.min, dim.max) else {
        // Only checkable when all three are present (Testing and Quality
        // Specification section 6: never synthesize a missing value in
        // order to validate).
        return None;
    };
    if !(min <= nominal && nominal <= max) {
        Some(Diagnostic::error(
            "dimension.out_of_range",
            entity,
            Some(field_path),
            format!("nominal ({nominal}) is not within [{min}, {max}]"),
        ))
    } else {
        None
    }
}

fn check_provenance(
    entity: &str,
    provenance: &ProvenanceMap,
    known_sources: &BTreeSet<SourceId>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (path, entries) in provenance {
        if !path.starts_with('/') {
            out.push(Diagnostic::error(
                "provenance.invalid_path",
                entity,
                Some(path),
                format!(
                    "provenance path \"{path}\" is not JSON-Pointer-shaped (must start with \"/\")"
                ),
            ));
        }
        for entry in entries {
            if !known_sources.contains(&entry.source) {
                out.push(Diagnostic::error(
                    "ref.unknown_source",
                    entity,
                    Some(path),
                    format!(
                        "provenance for \"{path}\" cites source \"{}\" which was not supplied for validation",
                        entry.source
                    ),
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{
        Dimension, Existence, ExistenceStatus, Kind, Lifecycle, LifecycleStatus, ManufacturerId,
        PackageDimensions, PartId,
    };
    use std::collections::BTreeMap;

    fn base_part() -> Part {
        Part {
            schema_version: "0.1".into(),
            kind: Kind::Part,
            id: PartId::from("ex/EX1"),
            manufacturer: ManufacturerId::from("ex"),
            mpn: "EX1".into(),
            device: openparts_core::DeviceId::from("ex/DEV1"),
            package: openparts_core::PackageId::from("standards/PKG1"),
            existence: Existence {
                status: ExistenceStatus::Unverified,
            },
            lifecycle: Lifecycle {
                status: LifecycleStatus::Active,
                replacement: vec![],
            },
            sources: vec![],
            provenance: BTreeMap::new(),
        }
    }

    fn base_device() -> Device {
        let mut pins = BTreeMap::new();
        pins.insert(
            "1".to_string(),
            openparts_core::Pin {
                name: "VBAT".into(),
                pin_type: openparts_core::PinType::Power,
                alternate_functions: vec![],
            },
        );
        Device {
            schema_version: "0.1".into(),
            kind: Kind::Device,
            id: openparts_core::DeviceId::from("ex/DEV1"),
            manufacturer: ManufacturerId::from("ex"),
            family: None,
            pins,
            revisions: BTreeMap::new(),
            provenance: BTreeMap::new(),
        }
    }

    fn base_package() -> Package {
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: openparts_core::PackageId::from("standards/PKG1"),
            family: "lqfp".into(),
            lead_count: 4,
            pitch: Dimension {
                nominal: Some(0.5),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(5.0),
                    min: Some(4.9),
                    max: Some(5.1),
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(5.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_height: None,
                exposed_pad: None,
            },
            lead_layout: None,
            geometry: None,
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn accepts_consistent_triple() {
        let diags = validate_part(
            &base_part(),
            &base_device(),
            &base_package(),
            &BTreeSet::new(),
        );
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn rejects_device_mismatch() {
        let mut device = base_device();
        device.id = openparts_core::DeviceId::from("ex/OTHER");
        let diags = validate_part(&base_part(), &device, &base_package(), &BTreeSet::new());
        assert!(diags.iter().any(|d| d.rule_id == "ref.device_mismatch"));
    }

    #[test]
    fn rejects_duplicate_signal_pin_names() {
        let mut device = base_device();
        device.pins.insert(
            "2".to_string(),
            openparts_core::Pin {
                name: "PA0".into(),
                pin_type: openparts_core::PinType::Io,
                alternate_functions: vec![],
            },
        );
        device.pins.insert(
            "3".to_string(),
            openparts_core::Pin {
                name: "PA0".into(),
                pin_type: openparts_core::PinType::Io,
                alternate_functions: vec![],
            },
        );
        let diags = validate_part(&base_part(), &device, &base_package(), &BTreeSet::new());
        assert!(diags.iter().any(|d| d.rule_id == "pin.duplicate_name"));
    }

    #[test]
    fn allows_repeated_power_and_ground_pin_names() {
        let mut device = base_device();
        // base_device already has pin "1" = VBAT/Power. Add several more
        // power/ground pins reusing common names -- this is normal on
        // real packages and must not be flagged.
        device.pins.insert(
            "2".to_string(),
            openparts_core::Pin {
                name: "VDD".into(),
                pin_type: openparts_core::PinType::Power,
                alternate_functions: vec![],
            },
        );
        device.pins.insert(
            "3".to_string(),
            openparts_core::Pin {
                name: "VDD".into(),
                pin_type: openparts_core::PinType::Power,
                alternate_functions: vec![],
            },
        );
        device.pins.insert(
            "4".to_string(),
            openparts_core::Pin {
                name: "VSS".into(),
                pin_type: openparts_core::PinType::Ground,
                alternate_functions: vec![],
            },
        );
        device.pins.insert(
            "5".to_string(),
            openparts_core::Pin {
                name: "VSS".into(),
                pin_type: openparts_core::PinType::Ground,
                alternate_functions: vec![],
            },
        );
        let diags = validate_part(&base_part(), &device, &base_package(), &BTreeSet::new());
        assert!(!diags.iter().any(|d| d.rule_id == "pin.duplicate_name"));
    }

    #[test]
    fn rejects_out_of_range_dimension() {
        let mut package = base_package();
        package.dimensions.body_width = Dimension {
            nominal: Some(10.0),
            min: Some(4.9),
            max: Some(5.1),
            unit: "mm".into(),
        };
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert!(diags.iter().any(|d| d.rule_id == "dimension.out_of_range"));
    }

    #[test]
    fn does_not_invent_a_missing_nominal() {
        // body_length has no min/max in base_package -> not checkable, no
        // diagnostic should be synthesized just because it's incomplete.
        let diags = validate_part(
            &base_part(),
            &base_device(),
            &base_package(),
            &BTreeSet::new(),
        );
        assert!(!diags
            .iter()
            .any(|d| d.field_path.as_deref() == Some("/dimensions/body_length")));
    }

    #[test]
    fn rejects_unknown_source_reference() {
        let mut part = base_part();
        part.sources = vec![SourceId::from("ex/UNKNOWN-DS")];
        let diags = validate_part(&part, &base_device(), &base_package(), &BTreeSet::new());
        assert!(diags.iter().any(|d| d.rule_id == "ref.unknown_source"));
    }

    #[test]
    fn accepts_known_source_reference() {
        let mut part = base_part();
        let known = SourceId::from("ex/DS1");
        part.sources = vec![known.clone()];
        let mut known_sources = BTreeSet::new();
        known_sources.insert(known);
        let diags = validate_part(&part, &base_device(), &base_package(), &known_sources);
        assert!(!diags.iter().any(|d| d.rule_id == "ref.unknown_source"));
    }
}
