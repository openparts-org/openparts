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
//! | `pin.duplicate_name` | two different pin numbers on one Device share the same *signal* pin name (power/ground/nc/reserved pins are exempt -- repeating `VDD`/`VSS`/`NC` across many pins is normal -- and so are pins linked by `connected_to`) |
//! | `pin.connected_to.invalid` | a pin's `connected_to` names no other pin of the Device |
//! | `dimension.out_of_range` | `nominal` is not within `[min, max]` when all three are present |
//! | `dimension.inconsistent` | two package dimensions contradict each other: a lead span (`overall_width`/`overall_length`) not larger than the body along the same axis, or a `standoff`/`body_height` not smaller than `overall_height` (compared only between stated values) |
//! | `provenance.invalid_path` | a provenance key is not a JSON Pointer-shaped path (must start with `/`) |
//! | `provenance.missing` | a package lead/outline dimension (`overall_*`, `standoff`, `lead_*`, `tab_*`, and `module`'s `row_spacing`, `hole_diameter`, `aux_*`, `mounting_hole_*`; the header families' `mating_length`, `tail_length`, `pad_diameter`, `body_offset`), `construction`, `tab` or `aux_pads` is present without a provenance entry for its path (these fields were added with field-level provenance required from the start) |
//! | `tab.invalid` | a package `tab` has an empty number, a trimmed lead that is not a lead position (`1..=lead_count`), or no `dimensions.tab_width` |
//! | `aux_pads.invalid` | a package's `aux_pads` has an empty or repeated number, a number that is also an edge pin (`1..=lead_count`), or is missing one of `dimensions.aux_pad_width`/`aux_pad_length`/`aux_row_x`/`aux_row_y` |
//! | `header.invalid` | a `header`/`socket`/`shrouded_header`/`right_angle_header` package has no `lead_layout` of `[rows, pins_per_row]` with 1 or 2 rows (1 for `right_angle_header`) multiplying to `lead_count`, no `dimensions.hole_diameter` (the footprint drill), two rows without `dimensions.row_spacing`, or is a `right_angle_header` without `dimensions.body_offset` (where its housing lies) |
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
    for (path, dim) in package.dimensions.entries() {
        out.extend(check_dimension(&package_entity, path, dim));
    }
    out.extend(check_package_consistency(&package_entity, package));
    out.extend(check_package_provenance(&package_entity, package));
    out.extend(check_package_tab(&package_entity, package));
    out.extend(check_package_aux_pads(&package_entity, package));
    out.extend(check_package_header(&package_entity, package));

    out
}

/// Paths of the package fields that must carry their own provenance.
const PROVENANCE_REQUIRED: [&str; 22] = [
    "/dimensions/overall_width",
    "/dimensions/overall_length",
    "/dimensions/overall_height",
    "/dimensions/standoff",
    "/dimensions/lead_width",
    "/dimensions/lead_length",
    "/dimensions/lead_thickness",
    "/dimensions/tab_width",
    "/dimensions/tab_length",
    "/dimensions/row_spacing",
    "/dimensions/hole_diameter",
    "/dimensions/aux_pad_width",
    "/dimensions/aux_pad_length",
    "/dimensions/aux_row_x",
    "/dimensions/aux_row_y",
    "/dimensions/mounting_hole_diameter",
    "/dimensions/mounting_hole_spacing",
    "/dimensions/mounting_hole_inset",
    "/dimensions/mating_length",
    "/dimensions/tail_length",
    "/dimensions/pad_diameter",
    "/dimensions/body_offset",
];

fn check_package_provenance(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let mut present: Vec<&str> = package
        .dimensions
        .entries()
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| PROVENANCE_REQUIRED.contains(path))
        .collect();
    if package.construction.is_some() {
        present.push("/construction");
    }
    if package.tab.is_some() {
        present.push("/tab");
    }
    if !package.aux_pads.is_empty() {
        present.push("/aux_pads");
    }
    present
        .into_iter()
        .filter(|path| {
            // An entry for the field itself or for one of its parents
            // (e.g. "/dimensions") covers it.
            !package.provenance.keys().any(|key| {
                path.strip_prefix(key.as_str())
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
            })
        })
        .map(|path| {
            Diagnostic::error(
                "provenance.missing",
                entity,
                Some(path),
                format!("\"{path}\" has no provenance entry"),
            )
        })
        .collect()
}

/// A tab's trimmed leads must be lead positions of the package, and a
/// tab needs its width (the generators can't draw a tab without it).
fn check_package_tab(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let Some(tab) = &package.tab else {
        return vec![];
    };
    let mut out = Vec::new();
    if tab.number.trim().is_empty() {
        out.push(Diagnostic::error(
            "tab.invalid",
            entity,
            Some("/tab/number"),
            "tab.number is empty".to_string(),
        ));
    }
    for lead in &tab.trimmed_leads {
        let valid = lead
            .parse::<u32>()
            .is_ok_and(|n| (1..=package.lead_count).contains(&n));
        if !valid {
            out.push(Diagnostic::error(
                "tab.invalid",
                entity,
                Some("/tab/trimmed_leads"),
                format!(
                    "trimmed lead \"{lead}\" is not a lead position (1..={})",
                    package.lead_count
                ),
            ));
        }
    }
    if package.dimensions.tab_width.is_none() {
        out.push(Diagnostic::error(
            "tab.invalid",
            entity,
            Some("/dimensions/tab_width"),
            "a package with a tab must state dimensions.tab_width".to_string(),
        ));
    }
    out
}

/// Auxiliary pads (`module`): distinct, non-empty numbers that don't
/// reuse an edge pin's number, and the row's size and place stated (the
/// generator can't draw them otherwise).
fn check_package_aux_pads(entity: &str, package: &Package) -> Vec<Diagnostic> {
    if package.aux_pads.is_empty() {
        return vec![];
    }
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for number in &package.aux_pads {
        let edge_pin = number
            .parse::<u32>()
            .is_ok_and(|n| (1..=package.lead_count).contains(&n));
        let problem = if number.trim().is_empty() {
            Some("an aux pad number is empty".to_string())
        } else if edge_pin {
            Some(format!(
                "aux pad \"{number}\" reuses an edge pin number (1..={})",
                package.lead_count
            ))
        } else if !seen.insert(number.as_str()) {
            Some(format!("aux pad \"{number}\" is listed twice"))
        } else {
            None
        };
        if let Some(message) = problem {
            out.push(Diagnostic::error(
                "aux_pads.invalid",
                entity,
                Some("/aux_pads"),
                message,
            ));
        }
    }
    let d = &package.dimensions;
    for (path, dim) in [
        ("/dimensions/aux_pad_width", &d.aux_pad_width),
        ("/dimensions/aux_pad_length", &d.aux_pad_length),
        ("/dimensions/aux_row_x", &d.aux_row_x),
        ("/dimensions/aux_row_y", &d.aux_row_y),
    ] {
        if dim.is_none() {
            out.push(Diagnostic::error(
                "aux_pads.invalid",
                entity,
                Some(path),
                format!("a package with aux_pads must state {path}"),
            ));
        }
    }
    out
}

/// Pin headers and sockets: the grid and the drill must be stated (the
/// generator never guesses either), and a right-angle header's housing
/// position.
fn check_package_header(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let family = package.family.to_ascii_lowercase();
    if !["header", "socket", "shrouded_header", "right_angle_header"].contains(&family.as_str()) {
        return vec![];
    }
    let right_angle = family == "right_angle_header";
    let max_rows = if right_angle { 1 } else { 2 };
    let mut out = Vec::new();
    let rows = match package.lead_layout.as_deref() {
        Some(&[rows, per_row])
            if (1..=max_rows).contains(&rows) && rows * per_row == package.lead_count =>
        {
            Some(rows)
        }
        _ => {
            out.push(Diagnostic::error(
                "header.invalid",
                entity,
                Some("/lead_layout"),
                format!(
                    "a {} needs lead_layout [rows, pins_per_row] with {} and \
                     rows x pins_per_row = lead_count ({})",
                    package.family,
                    if right_angle { "1 row" } else { "1 or 2 rows" },
                    package.lead_count
                ),
            ));
            None
        }
    };
    let d = &package.dimensions;
    if d.hole_diameter.is_none() {
        out.push(Diagnostic::error(
            "header.invalid",
            entity,
            Some("/dimensions/hole_diameter"),
            "a header must state dimensions.hole_diameter (the recommended PCB hole)".to_string(),
        ));
    }
    if rows == Some(2) && d.row_spacing.is_none() {
        out.push(Diagnostic::error(
            "header.invalid",
            entity,
            Some("/dimensions/row_spacing"),
            "a 2-row header must state dimensions.row_spacing".to_string(),
        ));
    }
    if right_angle && d.body_offset.is_none() {
        out.push(Diagnostic::error(
            "header.invalid",
            entity,
            Some("/dimensions/body_offset"),
            "a right-angle header must state dimensions.body_offset (where its housing lies \
             beside the holes)"
                .to_string(),
        ));
    }
    out
}

/// The largest / smallest value a dimension states (nominal, min or
/// max) -- comparisons use only stated numbers, never a synthesized one.
fn largest(dim: &openparts_core::Dimension) -> Option<f64> {
    [dim.nominal, dim.min, dim.max]
        .into_iter()
        .flatten()
        .reduce(f64::max)
}
fn smallest(dim: &openparts_core::Dimension) -> Option<f64> {
    [dim.nominal, dim.min, dim.max]
        .into_iter()
        .flatten()
        .reduce(f64::min)
}

fn check_package_consistency(entity: &str, package: &Package) -> Vec<Diagnostic> {
    let d = &package.dimensions;
    let mut out = Vec::new();
    // (larger, smaller): `larger` must exceed `smaller` even comparing
    // the most pessimistic stated values.
    let pairs = [
        (
            ("/dimensions/overall_width", d.overall_width.as_ref()),
            ("/dimensions/body_width", Some(&d.body_width)),
        ),
        (
            ("/dimensions/overall_length", d.overall_length.as_ref()),
            ("/dimensions/body_length", Some(&d.body_length)),
        ),
        (
            ("/dimensions/overall_height", d.overall_height.as_ref()),
            ("/dimensions/standoff", d.standoff.as_ref()),
        ),
        (
            ("/dimensions/overall_height", d.overall_height.as_ref()),
            ("/dimensions/body_height", d.body_height.as_ref()),
        ),
    ];
    for ((big_path, big), (small_path, small)) in pairs {
        let (Some(big), Some(small)) = (big, small) else {
            continue;
        };
        let (Some(big_v), Some(small_v)) = (largest(big), smallest(small)) else {
            continue;
        };
        if big_v <= small_v {
            out.push(Diagnostic::error(
                "dimension.inconsistent",
                entity,
                Some(big_path),
                format!(
                    "{big_path} (at most {big_v}) must be larger than {small_path} (at least {small_v})"
                ),
            ));
        }
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
                connected_to: None,
            },
        );
        Device {
            schema_version: "0.1".into(),
            kind: Kind::Device,
            id: openparts_core::DeviceId::from("ex/DEV1"),
            manufacturer: ManufacturerId::from("ex"),
            category: openparts_core::ComponentCategory::Other,
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
                overall_width: None,
                overall_length: None,
                overall_height: None,
                standoff: None,
                lead_width: None,
                lead_length: None,
                lead_thickness: None,
                tab_width: None,
                tab_length: None,
                row_spacing: None,
                hole_diameter: None,
                aux_pad_width: None,
                aux_pad_length: None,
                aux_row_x: None,
                aux_row_y: None,
                mounting_hole_diameter: None,
                mounting_hole_spacing: None,
                mounting_hole_inset: None,
                mating_length: None,
                tail_length: None,
                pad_diameter: None,
                body_offset: None,
            },
            lead_layout: None,
            tab: None,
            aux_pads: vec![],
            construction: None,
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
    fn connected_pins_may_share_a_name_but_must_point_at_a_real_pin() {
        let pin = |name: &str, to: Option<&str>| openparts_core::Pin {
            name: name.into(),
            pin_type: openparts_core::PinType::Passive,
            alternate_functions: vec![],
            connected_to: to.map(str::to_string),
        };
        let mut device = base_device();
        device.pins.clear();
        device.pins.insert("1".into(), pin("BASE", None));
        device.pins.insert("2".into(), pin("COLLECTOR", None));
        device.pins.insert("3".into(), pin("EMITTER", None));
        device.pins.insert("4".into(), pin("COLLECTOR", Some("2")));
        let diags = check_duplicate_pin_names("d", &device);
        assert!(diags.is_empty(), "{diags:?}");
        device.pins.insert("4".into(), pin("COLLECTOR", None));
        assert!(check_duplicate_pin_names("d", &device)
            .iter()
            .any(|d| d.rule_id == "pin.duplicate_name"));
        device.pins.insert("4".into(), pin("COLLECTOR", Some("9")));
        assert!(check_duplicate_pin_names("d", &device)
            .iter()
            .any(|d| d.rule_id == "pin.connected_to.invalid"));
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
                connected_to: None,
            },
        );
        device.pins.insert(
            "3".to_string(),
            openparts_core::Pin {
                name: "PA0".into(),
                pin_type: openparts_core::PinType::Io,
                alternate_functions: vec![],
                connected_to: None,
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
                connected_to: None,
            },
        );
        device.pins.insert(
            "3".to_string(),
            openparts_core::Pin {
                name: "VDD".into(),
                pin_type: openparts_core::PinType::Power,
                alternate_functions: vec![],
                connected_to: None,
            },
        );
        device.pins.insert(
            "4".to_string(),
            openparts_core::Pin {
                name: "VSS".into(),
                pin_type: openparts_core::PinType::Ground,
                alternate_functions: vec![],
                connected_to: None,
            },
        );
        device.pins.insert(
            "5".to_string(),
            openparts_core::Pin {
                name: "VSS".into(),
                pin_type: openparts_core::PinType::Ground,
                alternate_functions: vec![],
                connected_to: None,
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

    fn mm(nominal: f64) -> Option<Dimension> {
        Some(Dimension {
            nominal: Some(nominal),
            min: None,
            max: None,
            unit: "mm".into(),
        })
    }

    fn cite(package: &mut Package, path: &str) {
        package.provenance.insert(
            path.into(),
            vec![openparts_core::ProvenanceEntry {
                source: SourceId::from("ex/DS1"),
                page: Some(1),
                locator: Some("Table 1".into()),
            }],
        );
    }

    #[test]
    fn rejects_lead_span_not_larger_than_body() {
        let mut package = base_package();
        package.dimensions.overall_width = mm(4.9);
        cite(&mut package, "/dimensions/overall_width");
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert!(diags.iter().any(|d| d.rule_id == "dimension.inconsistent"
            && d.field_path.as_deref() == Some("/dimensions/overall_width")));
    }

    #[test]
    fn new_package_fields_require_provenance() {
        let mut package = base_package();
        package.dimensions.overall_width = mm(7.0);
        package.dimensions.lead_width = mm(0.22);
        package.construction = Some(openparts_core::PackageConstruction::Molded);
        cite(&mut package, "/dimensions/lead_width");
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        let missing: Vec<_> = diags
            .iter()
            .filter(|d| d.rule_id == "provenance.missing")
            .filter_map(|d| d.field_path.as_deref())
            .collect();
        assert_eq!(missing, vec!["/dimensions/overall_width", "/construction"]);
        assert!(!diags.iter().any(|d| d.rule_id == "dimension.inconsistent"));
    }

    #[test]
    fn tab_needs_provenance_width_and_real_trimmed_leads() {
        let mut package = base_package();
        package.family = "dpak".into();
        package.lead_count = 3;
        package.tab = Some(openparts_core::PackageTab {
            number: "2".into(),
            trimmed_leads: vec!["2".into(), "7".into()],
        });
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        let paths = |rule: &str| -> Vec<String> {
            diags
                .iter()
                .filter(|d| d.rule_id == rule)
                .filter_map(|d| d.field_path.clone())
                .collect()
        };
        assert_eq!(paths("provenance.missing"), vec!["/tab"]);
        assert_eq!(
            paths("tab.invalid"),
            vec!["/tab/trimmed_leads", "/dimensions/tab_width"]
        );

        package.tab.as_mut().unwrap().trimmed_leads = vec!["2".into()];
        package.dimensions.tab_width = mm(5.2);
        cite(&mut package, "/tab");
        cite(&mut package, "/dimensions/tab_width");
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert!(
            !diags
                .iter()
                .any(|d| d.rule_id == "tab.invalid" || d.rule_id == "provenance.missing"),
            "{diags:?}"
        );
    }

    #[test]
    fn aux_pads_need_distinct_numbers_their_row_and_provenance() {
        let mut package = base_package();
        package.family = "module".into();
        package.aux_pads = vec!["D1".into(), "D1".into(), "3".into()];
        package.dimensions.aux_pad_width = mm(1.6);
        cite(&mut package, "/dimensions/aux_pad_width");
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        let paths = |rule: &str| -> Vec<String> {
            diags
                .iter()
                .filter(|d| d.rule_id == rule)
                .filter_map(|d| d.field_path.clone())
                .collect()
        };
        assert_eq!(paths("provenance.missing"), vec!["/aux_pads"]);
        assert_eq!(
            paths("aux_pads.invalid"),
            vec![
                "/aux_pads",
                "/aux_pads",
                "/dimensions/aux_pad_length",
                "/dimensions/aux_row_x",
                "/dimensions/aux_row_y"
            ],
            "a repeated number and an edge-pin number (lead_count 4)"
        );

        package.aux_pads = vec!["D1".into(), "D2".into(), "D3".into()];
        for path in [
            "/aux_pads",
            "/dimensions/aux_pad_length",
            "/dimensions/aux_row_x",
            "/dimensions/aux_row_y",
        ] {
            cite(&mut package, path);
        }
        package.dimensions.aux_pad_length = mm(3.2);
        package.dimensions.aux_row_x = mm(10.5);
        package.dimensions.aux_row_y = mm(0.8);
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert!(
            !diags
                .iter()
                .any(|d| d.rule_id == "aux_pads.invalid" || d.rule_id == "provenance.missing"),
            "{diags:?}"
        );
    }

    #[test]
    fn headers_need_their_grid_drill_and_provenance() {
        let mut package = base_package();
        package.family = "header".into();
        package.lead_layout = Some(vec![2, 3]);
        package.dimensions.tail_length = mm(3.0);
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        let paths = |diags: &[Diagnostic], rule: &str| -> Vec<String> {
            diags
                .iter()
                .filter(|d| d.rule_id == rule)
                .filter_map(|d| d.field_path.clone())
                .collect()
        };
        assert_eq!(
            paths(&diags, "provenance.missing"),
            vec!["/dimensions/tail_length"]
        );
        assert_eq!(
            paths(&diags, "header.invalid"),
            vec!["/lead_layout", "/dimensions/hole_diameter"],
            "2 x 3 is not lead_count 4"
        );

        package.lead_layout = Some(vec![2, 2]);
        package.dimensions.hole_diameter = mm(1.0);
        cite(&mut package, "/dimensions/hole_diameter");
        cite(&mut package, "/dimensions/tail_length");
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert_eq!(
            paths(&diags, "header.invalid"),
            vec!["/dimensions/row_spacing"]
        );
        package.dimensions.row_spacing = mm(2.54);
        cite(&mut package, "/dimensions/row_spacing");
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert!(
            !diags
                .iter()
                .any(|d| d.rule_id == "header.invalid" || d.rule_id == "provenance.missing"),
            "{diags:?}"
        );
        // One row needs no row spacing; other families aren't checked.
        package.lead_layout = Some(vec![1, 4]);
        package.dimensions.row_spacing = None;
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert!(paths(&diags, "header.invalid").is_empty());
        package.family = "lqfp".into();
        package.lead_layout = None;
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert!(paths(&diags, "header.invalid").is_empty());

        // A right-angle header: one row, and where its housing lies.
        package.family = "right_angle_header".into();
        package.lead_layout = Some(vec![2, 2]);
        package.dimensions.row_spacing = mm(2.54);
        package.dimensions.body_offset = mm(2.77);
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert_eq!(paths(&diags, "header.invalid"), vec!["/lead_layout"]);
        assert_eq!(
            paths(&diags, "provenance.missing"),
            vec!["/dimensions/body_offset"]
        );
        package.lead_layout = Some(vec![1, 4]);
        package.dimensions.body_offset = None;
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert_eq!(
            paths(&diags, "header.invalid"),
            vec!["/dimensions/body_offset"]
        );
        // A shrouded header may be centred (no offset stated).
        package.family = "shrouded_header".into();
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert!(paths(&diags, "header.invalid").is_empty());
    }
}
