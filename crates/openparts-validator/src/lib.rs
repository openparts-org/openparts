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
//!
//! Every rule is listed, with what it means, in [`RULES`].

mod cross;
mod device;
mod diagnostic;
mod package;
mod parameter;
mod provenance;

pub use cross::{validate_location, validate_source, DocKind};
pub use diagnostic::{Diagnostic, Severity};

use device::*;
use openparts_core::{Device, Package, Part, SourceId};
use package::*;
use parameter::*;
use provenance::*;
use std::collections::BTreeSet;

/// A validation rule: its id (as in [`Diagnostic::rule_id`]) and meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub meaning: &'static str,
}

pub const RULES: &[Rule] = &[
    Rule {
        id: "ref.device_mismatch",
        meaning: "`part.device` does not match the loaded Device's `id`",
    },
    Rule {
        id: "ref.package_mismatch",
        meaning: "`part.package` does not match the loaded Package's `id`",
    },
    Rule {
        id: "ref.manufacturer_mismatch",
        meaning: "`part.manufacturer` does not match the Device's `manufacturer`",
    },
    Rule {
        id: "ref.unknown_source",
        meaning: "a `sources` or `provenance` entry references a Source that was not supplied",
    },
    Rule {
        id: "pin.duplicate_name",
        meaning: "two different pin numbers on one Device share the same *signal* pin name (power/ground/nc/reserved pins are exempt -- repeating `VDD`/`VSS`/`NC` across many pins is normal -- and so are pins linked by `connected_to`)",
    },
    Rule {
        id: "pin.connected_to.invalid",
        meaning: "a pin's `connected_to` names no other pin of the Device",
    },
    Rule {
        id: "dimension.out_of_range",
        meaning: "`nominal` is not within `[min, max]` when all three are present",
    },
    Rule {
        id: "dimension.inconsistent",
        meaning: "two package dimensions contradict each other: a lead span (`overall_width`/`overall_length`) not larger than the body along the same axis, or a `standoff`/`body_height` not smaller than `overall_height` (compared only between stated values)",
    },
    Rule {
        id: "provenance.invalid_path",
        meaning: "a provenance key is not a JSON Pointer-shaped path (must start with `/`)",
    },
    Rule {
        id: "provenance.missing",
        meaning: "a Part or Device `parameters` value has no provenance entry at `/parameters/<key>`; or a package lead/outline dimension (`overall_*`, `standoff`, `lead_*`, `tab_*`, and `module`'s `row_spacing`, `hole_diameter`, `aux_*`, `mounting_hole_*`; the header families' `mating_length`, `tail_length`, `pad_diameter`, `body_offset`; `lead_inset`, `actuator_diameter`), `construction`, `tab`, `aux_pads`, `pad_order` or `pads` is present without a provenance entry for its path (these fields were added with field-level provenance required from the start)",
    },
    Rule {
        id: "parameter.unknown_key",
        meaning: "a `parameters` key is not in the parameter vocabulary (`openparts_core::PARAMETERS`, Canonical Data Specification section 11.1)",
    },
    Rule {
        id: "parameter.unit",
        meaning: "a numeric parameter has no `unit`, or a unit that is not a unit of its key's quantity (e.g. `V` for `capacitance`)",
    },
    Rule {
        id: "parameter.invalid",
        meaning: "a parameter value has the wrong shape for its key (`value` on a numeric key, `nominal`/`min`/`max`/`unit` on an enumerated one, or nothing), or an enumerated value outside the key's vocabulary",
    },
    Rule {
        id: "parameter.out_of_range",
        meaning: "a parameter's `min <= nominal <= max` does not hold (compared only between stated values)",
    },
    Rule {
        id: "parameter.implausible",
        meaning: "warning: a parameter key is unusual for the Device's category (e.g. `capacitance` on a resistor)",
    },
    Rule {
        id: "tab.invalid",
        meaning: "a package `tab` has an empty number, a trimmed lead that is not a lead position (`1..=lead_count`), or no `dimensions.tab_width`",
    },
    Rule {
        id: "aux_pads.invalid",
        meaning: "a package's `aux_pads` has an empty or repeated number, a number that is also an edge pin (`1..=lead_count`), or is missing one of `dimensions.aux_pad_width`/`aux_pad_length`/`aux_row_x`/`aux_row_y`",
    },
    Rule {
        id: "header.invalid",
        meaning: "a `header`/`socket`/`shrouded_header`/`right_angle_header` package has no `lead_layout` of `[rows, pins_per_row]` with 1 or 2 rows (1 for `right_angle_header`) multiplying to `lead_count`, no `dimensions.hole_diameter` (the footprint drill), two rows without `dimensions.row_spacing`, or is a `right_angle_header` without `dimensions.body_offset` (where its housing lies)",
    },
    Rule {
        id: "mpn.empty",
        meaning: "`part.mpn` is empty",
    },
    Rule {
        id: "pad_order.invalid",
        meaning: "a package's `pad_order` does not hold exactly `lead_count` distinct numbers",
    },
    Rule {
        id: "pin.no_pad",
        meaning: "a device pin has no pad in the package's footprint (pins in `tab.trimmed_leads` excepted)",
    },
    Rule {
        id: "pad.no_pin",
        meaning: "warning: a footprint pad (other than `EP`) has no device pin",
    },
    Rule {
        id: "provenance.unknown_path",
        meaning: "a provenance key points at a field the document doesn't have",
    },
    Rule {
        id: "existence.unsupported",
        meaning: "warning: `existence: verified` without an `/existence` provenance entry saying where the MPN is printed",
    },
    Rule {
        id: "source.date_format",
        meaning: "a Source `date` is not YYYY, YYYY-MM, YYYY-MM-DD or `unknown`, or `retrieved` is not YYYY-MM-DD",
    },
    Rule {
        id: "source.url",
        meaning: "a Source `url` is not an http(s) link",
    },
    Rule {
        id: "id.location",
        meaning: "a document's id doesn't match the directory and file it is stored in",
    },
];
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
    out.extend(check_package_pad_order(&package_entity, package));
    out.extend(cross::check_pins_and_pads(&device_entity, device, package));
    out.extend(cross::check_provenance_paths(
        &part_entity,
        part,
        &part.provenance,
    ));
    out.extend(cross::check_provenance_paths(
        &device_entity,
        device,
        &device.provenance,
    ));
    out.extend(cross::check_provenance_paths(
        &package_entity,
        package,
        &package.provenance,
    ));
    out.extend(cross::check_existence_evidence(&part_entity, part));
    out.extend(check_parameters(
        &part_entity,
        &part.parameters,
        &part.provenance,
        device.category,
    ));
    out.extend(check_parameters(
        &device_entity,
        &device.parameters,
        &device.provenance,
        device.category,
    ));

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every rule id the checks can emit is documented in `RULES`, and
    /// every documented rule is emitted somewhere.
    #[test]
    fn rules_table_matches_the_checks() {
        let src = [
            include_str!("lib.rs"),
            include_str!("device.rs"),
            include_str!("package.rs"),
            include_str!("provenance.rs"),
            include_str!("cross.rs"),
            include_str!("parameter.rs"),
        ]
        .concat();
        let documented: BTreeSet<&str> = RULES.iter().map(|r| r.id).collect();
        let mut emitted = BTreeSet::new();
        for chunk in src
            .split("Diagnostic::error(")
            .chain(src.split("Diagnostic::warning("))
            .filter(|c| c.trim_start().starts_with('"'))
        {
            let id = chunk.trim_start().trim_start_matches('"');
            let id = &id[..id.find('"').unwrap()];
            // Skip this test's own text; rule ids look like `area.name`.
            if id.contains('.')
                && id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_')
            {
                emitted.insert(id);
            }
        }
        assert_eq!(documented, emitted);
    }
    use openparts_core::{
        Dimension, Existence, ExistenceStatus, Kind, Lifecycle, LifecycleStatus, ManufacturerId,
        PackageDimensions, PartId,
    };
    use std::collections::BTreeMap;

    fn base_part() -> Part {
        Part {
            schema_version: "0.1".into(),
            kind: Kind::Part,
            parameters: Default::default(),
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
            parameters: Default::default(),
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
            family: openparts_core::PackageFamily::Lqfp,
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
                ..Default::default()
            },
            lead_layout: None,
            tab: None,
            aux_pads: vec![],
            pad_order: vec![],
            pads: vec![],
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
        // The minimal fixture device has fewer pins than pads: warnings only.
        let errors: Vec<_> = diags.iter().filter(|d| d.is_error()).collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
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
        package.family = openparts_core::PackageFamily::Dpak;
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
        package.family = openparts_core::PackageFamily::Module;
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
        package.family = openparts_core::PackageFamily::Header;
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
        package.family = openparts_core::PackageFamily::Lqfp;
        package.lead_layout = None;
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert!(paths(&diags, "header.invalid").is_empty());

        // A right-angle header: one row, and where its housing lies.
        package.family = openparts_core::PackageFamily::RightAngleHeader;
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
        package.family = openparts_core::PackageFamily::ShroudedHeader;
        let diags = validate_part(&base_part(), &base_device(), &package, &BTreeSet::new());
        assert!(paths(&diags, "header.invalid").is_empty());
    }

    #[test]
    fn parameters_are_typed_cited_and_plausible() {
        use openparts_core::ParameterValue;
        let num =
            |nominal: Option<f64>, min: Option<f64>, max: Option<f64>, unit: &str| ParameterValue {
                nominal,
                min,
                max,
                unit: Some(unit.into()),
                value: None,
            };
        let cite = |part: &mut Part, path: &str| {
            part.provenance.insert(
                path.into(),
                vec![openparts_core::ProvenanceEntry {
                    source: SourceId::from("ex/DS1"),
                    page: Some(1),
                    locator: Some("Specifications".into()),
                }],
            );
        };
        let mut device = base_device();
        device.category = openparts_core::ComponentCategory::Capacitor;
        let mut part = base_part();
        part.parameters
            .insert("capacitance".into(), num(Some(100.0), None, None, "nF"));
        part.parameters
            .insert("rated_voltage".into(), num(Some(50.0), None, None, "nF"));
        part.parameters
            .insert("tolerance".into(), num(Some(5.0), Some(10.0), None, "%"));
        part.parameters
            .insert("colour".into(), num(Some(1.0), None, None, "V"));
        part.parameters.insert(
            "dielectric".into(),
            ParameterValue {
                value: Some("X9Q".into()),
                ..Default::default()
            },
        );
        part.parameters
            .insert("resistance".into(), num(Some(10.0), None, None, "kΩ"));
        cite(&mut part, "/parameters/capacitance");
        let diags = validate_part(&part, &device, &base_package(), &BTreeSet::new());
        let paths = |rule: &str| -> Vec<String> {
            diags
                .iter()
                .filter(|d| d.rule_id == rule)
                .filter_map(|d| d.field_path.clone())
                .collect()
        };
        assert_eq!(paths("parameter.unknown_key"), vec!["/parameters/colour"]);
        assert_eq!(paths("parameter.unit"), vec!["/parameters/rated_voltage"]);
        assert_eq!(
            paths("parameter.out_of_range"),
            vec!["/parameters/tolerance"]
        );
        assert_eq!(paths("parameter.invalid"), vec!["/parameters/dielectric"]);
        assert_eq!(
            paths("parameter.implausible"),
            vec!["/parameters/resistance"]
        );
        assert!(diags
            .iter()
            .any(|d| d.rule_id == "parameter.implausible" && !d.is_error()));
        assert_eq!(
            paths("provenance.missing"),
            vec![
                "/parameters/colour",
                "/parameters/dielectric",
                "/parameters/rated_voltage",
                "/parameters/resistance",
                "/parameters/tolerance"
            ]
        );

        // A clean, cited set passes; a Device-level value is checked too.
        let mut part = base_part();
        part.parameters
            .insert("capacitance".into(), num(Some(100.0), None, None, "nF"));
        part.parameters
            .insert("tolerance".into(), num(None, Some(-10.0), Some(10.0), "%"));
        cite(&mut part, "/parameters/capacitance");
        cite(&mut part, "/parameters/tolerance");
        device
            .parameters
            .insert("rated_voltage".into(), num(Some(50.0), None, None, "V"));
        let diags = validate_part(&part, &device, &base_package(), &BTreeSet::new());
        let found: Vec<_> = diags
            .iter()
            .filter(|d| d.rule_id.starts_with("parameter.") || d.rule_id == "provenance.missing")
            .map(|d| (d.entity.as_str(), d.field_path.as_deref()))
            .collect();
        assert_eq!(
            found,
            vec![("device:ex/DEV1", Some("/parameters/rated_voltage"))]
        );
    }
}
