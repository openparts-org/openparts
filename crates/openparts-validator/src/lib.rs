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

mod assembly;
mod circuit;
#[cfg(test)]
mod circuit_tests;
mod connector;
mod cross;
mod device;
mod diagnostic;
mod package;
mod parameter;
mod provenance;
mod review;

pub use assembly::{validate_assembly, AssemblyContext};
pub use circuit::{validate_circuit, validate_circuit_location, CircuitContext};
pub use cross::{validate_location, validate_source, DocKind};
pub use diagnostic::{Diagnostic, Severity};
pub use review::{check_review, is_github_login};

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
        id: "pin.diff_pair.invalid",
        meaning: "a pin's `diff_pair` is not one half of a consistent pair: its partner is not another pin of the Device, or the partner's `diff_pair` does not point back, has the same polarity, another standard, or only one of the two names a `function`; or its `function` is not one of the pin's `alternate_functions`; or `impedance_ohm` is not positive",
    },
    Rule {
        id: "dimension.out_of_range",
        meaning: "`typ` is not within `[min, max]` when all three are present",
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
        meaning: "a circuit fact (a net's voltage, a condition, a connection, a role's `requires`/`level`/`place`/`example`/`expects`, a formula, a literal input or result, a hint, an include) has no `src` at it or an ancestor; or a Part or Device `parameters` value has no provenance entry at `/parameters/<key>`; or a package lead/outline dimension (`overall_*`, `standoff`, `lead_*`, `tab_*`, and `module`'s `row_spacing`, `hole_diameter`, `aux_*`, `mounting_hole_*`; the header families' `mating_length`, `tail_length`, `pad_diameter`, `body_offset`; `lead_inset`, `actuator_diameter`), `construction`, `tab`, `aux_pads`, `pad_order` or `pads` is present without a provenance entry for its path, or a pin's `diff_pair` has none at `/pins/<n>/diff_pair` or `/pins/<n>`, or a part's `assembly` component has none at `/assembly/<i>` (these fields were added with field-level provenance required from the start)",
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
        meaning: "a parameter value has the wrong shape for its key (`value` on a numeric key, `min`/`typ`/`max`/`unit` on an enumerated one, or nothing; a `values` list on a key that is not multi-valued, with `value` too, or repeating a value), or an enumerated value outside the key's vocabulary",
    },
    Rule {
        id: "parameter.out_of_range",
        meaning: "a parameter's `min <= typ <= max` does not hold (compared only between stated values)",
    },
    Rule {
        id: "parameter.condition",
        meaning: "a parameter value's `at` condition uses a symbol outside the condition vocabulary (`openparts_core::CONDITION_SYMBOLS`) or a unit that doesn't fit the symbol's quantity, or two values of one key state the same conditions (Canonical Data Specification section 11.2)",
    },
    Rule {
        id: "provenance.locator_long",
        meaning: "warning: a provenance `locator` is longer than 120 characters; a locator is a short identifier (table, figure, row, column, section), not copied text (Canonical Data Specification section 25.1)",
    },
    Rule {
        id: "parameter.implausible",
        meaning: "warning: a parameter key is unusual for the Device's category (e.g. `capacitance` on a resistor)",
    },
    Rule {
        id: "parameter.implausible_value",
        meaning: "warning: a parameter number lies outside its key's plausible range (`ParameterSpec::range`: a frequency tolerance beyond ±1000 ppm, a viewing angle over 360°, a propagation delay over 1 ms...), almost always a unit slip",
    },
    Rule {
        id: "category.other",
        meaning: "warning: the Device's category is `other`; use the most specific node of the category tree (Canonical Data Specification section 14.1) its datasheet supports, keeping `other` for component types the tree doesn't have yet",
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
        meaning: "a `header`/`socket`/`shrouded_header`/`right_angle_header` package has no `lead_layout` of `[rows, pins_per_row]` with 1 or 2 rows (1 for `right_angle_header`) multiplying to `lead_count`, no `dimensions.hole_diameter` (the footprint drill) unless it has `smt` contacts, `smt` on a family other than `shrouded_header`/`right_angle_header`, two rows without `dimensions.row_spacing`, or is a `right_angle_header` without `dimensions.body_offset` (where its housing lies)",
    },
    Rule {
        id: "edge.invalid",
        meaning: "a package's `edge.pcb_edge` (the PCB edge line its source draws) is not a number, or a land reaches past it",
    },
    Rule {
        id: "connector.invalid",
        meaning: "a connector-family package (`card_socket`, `barrel_jack`, `terminal_block`, `fpc_connector`, `rf_coax`, `micro_usb_b`, `mini_usb_b`, `usb_b`) has no `pads`, a `lead_count` other than its plated pad count, no `dimensions.body_height` (except `rf_coax`), an SMD pad without a size or a land-less slot without `slot_axis`, no `dimensions.body_length` (except an edge-mount `rf_coax` on a `+y` PCB edge line at 0, whose frame then starts at that line), or lacks a `connector` field its family needs (card socket: `mechanism`, `interface`; FPC: `contact_side`; RF: `interface`, `orientation`); or a `connector` field doesn't fit the family; or an `edge_mount` connector has no `edge` with a `pcb_edge` line",
    },
    Rule {
        id: "pad_shape.invalid",
        meaning: "a stated pad's land shape can't be drawn: `chamfer`/`corners` without `shape: chamfered` (or a chamfer that isn't positive and at most half the shorter side), `points` without `shape: polygon` (or fewer than three, no area, a stated width/length, or the pad's x/y outside the outline), a `circle` whose width and length differ, a `cutout` on a non-rectangular pad, covering the pad's centre or the whole pad, missing it, or cutting more than one notch, or an `annular` pad whose width and length differ, without `inner_diameter` (or one not inside the ring), with a drill, or `inner_diameter` on another shape",
    },
    Rule {
        id: "pad_side.invalid",
        meaning: "a pad with `side: bottom` has a drill: only an SMD land can be on the bottom side (a hole goes through the board)",
    },
    Rule {
        id: "smd_led.invalid",
        meaning: "an `smd_led` package has no `pads`, a `lead_count` other than its plated pad count, a pad without a size, or neither `dimensions.body_height` nor `overall_height`",
    },
    Rule {
        id: "smd_electrolytic.invalid",
        meaning: "an `smd_electrolytic` package has no `pads`, a `lead_count` other than its plated pad count (at least two), a pad without a size, no `dimensions.overall_height` or no `dimensions.can_diameter`; or another family than `buzzer` states `can_diameter`",
    },
    Rule {
        id: "melf.invalid",
        meaning: "a `melf` package has a `lead_count` other than 2, no `pads`, other than two plated pads, a pad without a size or with a drill, or no `dimensions.body_length` or `body_width` (the diameter)",
    },
    Rule {
        id: "electromechanical.invalid",
        meaning: "a `relay`, `buzzer`, `trimmer` or `rotary_encoder` package has no `pads`, a `lead_count` other than its plated pad count, no `dimensions.body_length` or `body_height`, an SMD pad without a size or a land-less slot without `slot_axis`, `connector` facts, or an `overall_height` (rotor or shaft top) below the housing top",
    },
    Rule {
        id: "chip_array.invalid",
        meaning: "a `chip_array` package has an odd `lead_count` or fewer than 4, no `pads`, pads other than one sized SMD land per terminal numbered 1..`lead_count`, no `pitch` or no `dimensions.body_width`/`body_length`; or another family states `terminal_shape`",
    },
    Rule {
        id: "exposed_pad.invalid",
        meaning: "a top-face (pad-up, `dimensions.exposed_pad.face: top`) exposed pad on a family other than `soic`/`tssop`/`ssop`/`msop`, or with a board land (`land_width`/`land_length`)",
    },
    Rule {
        id: "lga.invalid",
        meaning: "an `lga` whose `lead_layout` counts fewer terminals than `lead_count` doesn't give exactly the rest as `pads` entries with other SMD pad numbers (a centre or grid pad, a ground ring)",
    },
    Rule {
        id: "keepout.invalid",
        meaning: "a package keep-out has both or neither of `rect` (located) and `size` (only sized), a non-positive length, or restricts nothing",
    },
    Rule {
        id: "pad_layout.invalid",
        meaning: "a package's `pad_rows`/`pad_grids`/`shield` is on a family other than `module`, has a non-positive size or count, a pitch not above its pad, a row `drill` not inside its pad, a pad number used twice (rows, grids and a module's plated `pads`), a grid with neither one number nor one per pad, `pad_grids` without `pad_rows` or `pads`, a `shield` without `dimensions.overall_height`, or `smt` has a non-positive pad size or an empty `mech_number`",
    },
    Rule {
        id: "assembly.invalid",
        meaning: "a module assembly component (Canonical Data Specification 20.12) has no or a repeated `ref`, not exactly one of `part` and `body`, a non-numeric position, a body without a positive width, length and height, or a `color` that is not `#rrggbb`",
    },
    Rule {
        id: "assembly.unknown_part",
        meaning: "an assembly component's `part` is not a part of the dataset",
    },
    Rule {
        id: "assembly.nested",
        meaning: "an assembly component's `part` is the module itself or has an `assembly` of its own (assemblies don't nest)",
    },
    Rule {
        id: "assembly.outside",
        meaning: "warning: an assembly component's pads or body reach past the board outline (expected for an overhanging connector; otherwise its x/y/rotation is off)",
    },
    Rule {
        id: "mpn.empty",
        meaning: "`part.mpn` is empty",
    },
    Rule {
        id: "tact_switch.invalid",
        meaning: "a `tact_switch_smd`/`tact_switch_tht` package has a `lead_count` other than 2 to 5 (an odd count adds a ground terminal), a ground terminal without `dimensions.ground_terminal_offset` (unless a THT package's `pads` place its holes), `ground_terminal_*` without one, `actuation: side` without `dimensions.actuator_length`, `actuator_width` without `actuator_thickness` or the reverse, side-actuator dimensions without `actuation: side`, or THT `pads` with a plated pad lacking a drill; or another family states `actuation`, `actuator_length`/`_elevation`/`_width`/`_thickness` or `ground_terminal_*`",
    },
    Rule {
        id: "pad_order.invalid",
        meaning: "a package's `pad_order` does not hold exactly `lead_count` entries, repeats a pad number, or is all empty (an empty entry is a lead position without a lead)",
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
        id: "source.checksum",
        meaning: "a Source `sha256` is not 64 lowercase hex digits",
    },
    Rule {
        id: "review.block",
        meaning: "a document's `review` block is incomplete or malformed: `by` (GitHub logins, no duplicates) and `pr` (the openparts-data pull request, 1 or more) are required, `date` is YYYY-MM-DD (Canonical Data Specification section 26.1)",
    },
    Rule {
        id: "circuit.host",
        meaning: "a circuit's `host` names a device the dataset doesn't have, or a match with nothing to match on",
    },
    Rule {
        id: "circuit.pin",
        meaning: "a circuit names a host pin (`connect`, `per_pin`, `host.<PIN>`, `conditions`) the host device doesn't have (for a generic host: one outside its `has_pins`)",
    },
    Rule {
        id: "circuit.connection",
        meaning: "a circuit connection is invalid: a target is not `$pin` (with `per_pin`), `host.<PIN>` or a net under `nets`, a `connect` entry has neither `net` nor `no_connect`, or a hint names an unknown net or role",
    },
    Rule {
        id: "circuit.part",
        meaning: "a circuit role's `requires.part` or `example.part` is not a part of the dataset",
    },
    Rule {
        id: "circuit.level",
        meaning: "a level's `word` is not one of the words of its level (required: must, shall, ...; recommended: should, recommend; optional: may, DNF), or an `example` has one",
    },
    Rule {
        id: "circuit.formula",
        meaning: "a circuit formula doesn't parse, uses a name that is not one of its inputs, or an input/target names an unknown role or a parameter the role doesn't state (warning: an input the expression doesn't use)",
    },
    Rule {
        id: "circuit.include",
        meaning: "an included circuit doesn't exist, is for another host without `host: { role }`, binds a net it doesn't have, or names an unknown role",
    },
    Rule {
        id: "circuit.invalid",
        meaning: "a circuit is malformed: empty title, id not `<publisher>/<name>`, `per_pin` with `count`, a role without `category` or `part`, a hint about nothing, a diff pair not of two nets",
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

    out.extend(review::check_review(&part_entity, part.review.as_ref()));
    out.extend(review::check_review(&device_entity, device.review.as_ref()));
    out.extend(review::check_review(
        &format!("package:{}", package.id),
        package.review.as_ref(),
    ));

    out.extend(check_duplicate_pin_names(&device_entity, device));
    out.extend(check_diff_pairs(&device_entity, device));
    if device.category == openparts_core::ComponentCategory::Other {
        out.push(Diagnostic::warning(
            "category.other",
            &device_entity,
            Some("/category"),
            "category is `other`: use a node of the category tree if one fits",
        ));
    }

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
    out.extend(check_package_tact_switch(&package_entity, package));
    out.extend(check_package_edge(&package_entity, package));
    out.extend(check_package_keepouts(&package_entity, package));
    out.extend(check_package_pad_layout(&package_entity, package));
    out.extend(connector::check_package_connector(&package_entity, package));
    out.extend(connector::check_pad_shapes(&package_entity, package));
    out.extend(connector::check_pad_sides(&package_entity, package));
    out.extend(connector::check_smd_led(&package_entity, package));
    out.extend(connector::check_smd_electrolytic(&package_entity, package));
    out.extend(connector::check_melf(&package_entity, package));
    out.extend(connector::check_chip_array(&package_entity, package));
    out.extend(connector::check_exposed_pad_face(&package_entity, package));
    out.extend(connector::check_lga(&package_entity, package));
    out.extend(connector::check_electromechanical(&package_entity, package));
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
            include_str!("review.rs"),
            include_str!("circuit.rs"),
            include_str!("connector.rs"),
            include_str!("assembly.rs"),
        ]
        .concat();
        let documented: BTreeSet<&str> = RULES.iter().map(|r| r.id).collect();
        let mut emitted = BTreeSet::new();
        for chunk in src
            .split("Diagnostic::error(")
            .chain(src.split("Diagnostic::warning("))
            .chain(src.split("self.error("))
            .chain(src.split("self.warning("))
            .chain(src.split("v.error("))
            .chain(src.split("v.warning("))
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
            assembly: vec![],
            provenance: BTreeMap::new(),
            review: None,
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
                diff_pair: None,
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
            review: None,
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
                typ: Some(0.5),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    typ: Some(5.0),
                    min: Some(4.9),
                    max: Some(5.1),
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    typ: Some(5.0),
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
            actuation: None,
            terminal_shape: None,
            connector: None,
            edge: None,
            keepouts: vec![],
            pad_rows: vec![],
            pad_grids: vec![],
            shield: None,
            smt: None,
            pads: vec![],
            construction: None,
            geometry: None,
            provenance: BTreeMap::new(),
            review: None,
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
            diff_pair: None,
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
    fn diff_pairs_must_be_symmetric_opposite_and_cited() {
        use openparts_core::{DiffPair, DiffPolarity, DiffStandard, Pin, PinType};
        let pin =
            |name: &str, alts: &[&str], pair: Option<(&str, DiffPolarity, Option<&str>)>| Pin {
                name: name.into(),
                pin_type: PinType::Io,
                alternate_functions: alts.iter().map(|a| a.to_string()).collect(),
                connected_to: None,
                diff_pair: pair.map(|(partner, polarity, function)| DiffPair {
                    partner: partner.into(),
                    polarity,
                    standard: DiffStandard::Usb2,
                    function: function.map(str::to_string),
                    impedance_ohm: None,
                }),
            };
        let cite = |device: &mut Device, path: &str| {
            device.provenance.insert(
                path.into(),
                vec![openparts_core::ProvenanceEntry {
                    source: SourceId::from("x/y"),
                    page: Some(1),
                    locator: None,
                }],
            );
        };
        let ids = |device: &Device| {
            check_diff_pairs("d", device)
                .into_iter()
                .map(|d| d.rule_id)
                .collect::<Vec<_>>()
        };
        let mut device = base_device();
        device.pins.clear();
        device.pins.insert(
            "1".into(),
            pin("DP", &[], Some(("2", DiffPolarity::P, None))),
        );
        device.pins.insert(
            "2".into(),
            pin("DM", &[], Some(("1", DiffPolarity::N, None))),
        );
        assert_eq!(
            ids(&device),
            vec!["provenance.missing", "provenance.missing"]
        );
        cite(&mut device, "/pins/1/diff_pair");
        cite(&mut device, "/pins/2");
        assert!(ids(&device).is_empty(), "{:?}", ids(&device));
        // Same polarity on both halves.
        device.pins.insert(
            "2".into(),
            pin("DM", &[], Some(("1", DiffPolarity::P, None))),
        );
        assert_eq!(
            ids(&device),
            vec!["pin.diff_pair.invalid", "pin.diff_pair.invalid"]
        );
        // Not pointing back.
        device.pins.insert("2".into(), pin("DM", &[], None));
        assert_eq!(ids(&device), vec!["pin.diff_pair.invalid"]);
        // Unknown partner, and the pin itself.
        device.pins.insert(
            "2".into(),
            pin("DM", &[], Some(("9", DiffPolarity::N, None))),
        );
        assert!(ids(&device).iter().all(|i| *i == "pin.diff_pair.invalid"));
        device.pins.insert(
            "2".into(),
            pin("DM", &[], Some(("2", DiffPolarity::N, None))),
        );
        assert!(ids(&device).contains(&"pin.diff_pair.invalid"));
        // A function must be one of the pin's alternate functions, and
        // both halves name one or neither does.
        device.pins.insert(
            "1".into(),
            pin(
                "GPIO13",
                &["USB_D+"],
                Some(("2", DiffPolarity::P, Some("USB_D+"))),
            ),
        );
        device.pins.insert(
            "2".into(),
            pin(
                "GPIO12",
                &["USB_D-"],
                Some(("1", DiffPolarity::N, Some("USB_D-"))),
            ),
        );
        assert!(ids(&device).is_empty(), "{:?}", ids(&device));
        device.pins.insert(
            "2".into(),
            pin(
                "GPIO12",
                &["USB_D-"],
                Some(("1", DiffPolarity::N, Some("USB_DM"))),
            ),
        );
        assert_eq!(ids(&device), vec!["pin.diff_pair.invalid"]);
        device.pins.insert(
            "2".into(),
            pin("GPIO12", &["USB_D-"], Some(("1", DiffPolarity::N, None))),
        );
        assert_eq!(
            ids(&device),
            vec!["pin.diff_pair.invalid", "pin.diff_pair.invalid"]
        );
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
                diff_pair: None,
            },
        );
        device.pins.insert(
            "3".to_string(),
            openparts_core::Pin {
                name: "PA0".into(),
                pin_type: openparts_core::PinType::Io,
                alternate_functions: vec![],
                connected_to: None,
                diff_pair: None,
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
                diff_pair: None,
            },
        );
        device.pins.insert(
            "3".to_string(),
            openparts_core::Pin {
                name: "VDD".into(),
                pin_type: openparts_core::PinType::Power,
                alternate_functions: vec![],
                connected_to: None,
                diff_pair: None,
            },
        );
        device.pins.insert(
            "4".to_string(),
            openparts_core::Pin {
                name: "VSS".into(),
                pin_type: openparts_core::PinType::Ground,
                alternate_functions: vec![],
                connected_to: None,
                diff_pair: None,
            },
        );
        device.pins.insert(
            "5".to_string(),
            openparts_core::Pin {
                name: "VSS".into(),
                pin_type: openparts_core::PinType::Ground,
                alternate_functions: vec![],
                connected_to: None,
                diff_pair: None,
            },
        );
        let diags = validate_part(&base_part(), &device, &base_package(), &BTreeSet::new());
        assert!(!diags.iter().any(|d| d.rule_id == "pin.duplicate_name"));
    }

    #[test]
    fn rejects_out_of_range_dimension() {
        let mut package = base_package();
        package.dimensions.body_width = Dimension {
            typ: Some(10.0),
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
            typ: Some(nominal),
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
        let num = |nominal: Option<f64>,
                   min: Option<f64>,
                   max: Option<f64>,
                   unit: &str|
         -> openparts_core::ParameterValues {
            ParameterValue {
                typ: nominal,
                min,
                max,
                unit: Some(unit.into()),
                ..Default::default()
            }
            .into()
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
            }
            .into(),
        );
        part.parameters
            .insert("resistance".into(), num(Some(10.0), None, None, "kΩ"));
        // Only a multi-valued key (data_interface, reference) takes a list.
        let mut list = base_part();
        list.parameters.insert(
            "dielectric".into(),
            ParameterValue {
                values: vec!["X7R".into(), "X5R".into()],
                ..Default::default()
            }
            .into(),
        );
        let ld = validate_part(&list, &device, &base_package(), &BTreeSet::new());
        assert!(ld.iter().any(|d| d.rule_id == "parameter.invalid"
            && d.field_path.as_deref() == Some("/parameters/dielectric")));
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

    #[test]
    fn conditions_and_variants_are_checked() {
        use openparts_core::{Condition, ParameterValue, ParameterValues};
        let cond = |sym: &str, v: f64, u: &str| {
            (
                sym.to_string(),
                Condition {
                    value: Some(v),
                    unit: u.into(),
                    ..Default::default()
                },
            )
        };
        let rds = |at: Vec<(String, Condition)>| ParameterValue {
            typ: Some(18.0),
            unit: Some("mΩ".into()),
            at: at.into_iter().collect(),
            ..Default::default()
        };
        let mut device = base_device();
        device.category = openparts_core::ComponentCategory::Transistor;
        let mut part = base_part();
        part.parameters.insert(
            "rds_on".into(),
            ParameterValues(vec![
                rds(vec![cond("VGS", 10.0, "V")]),
                rds(vec![cond("VGS", 10000.0, "mV")]),
                rds(vec![cond("VGS", 4.5, "A"), cond("VXX", 1.0, "V")]),
            ]),
        );
        let entry = openparts_core::ProvenanceEntry {
            source: SourceId::from("ex/DS1"),
            page: Some(2),
            locator: Some("x".repeat(121)),
        };
        part.provenance
            .insert("/parameters/rds_on/0".into(), vec![entry.clone()]);
        part.provenance
            .insert("/parameters/rds_on/1".into(), vec![entry]);
        let diags = validate_part(&part, &device, &base_package(), &BTreeSet::new());
        let paths = |rule: &str| -> Vec<String> {
            diags
                .iter()
                .filter(|d| d.rule_id == rule)
                .filter_map(|d| d.field_path.clone())
                .collect()
        };
        assert_eq!(
            paths("parameter.condition"),
            vec![
                "/parameters/rds_on/1",
                "/parameters/rds_on/2/at/VGS",
                "/parameters/rds_on/2/at/VXX"
            ]
        );
        assert_eq!(paths("provenance.missing"), vec!["/parameters/rds_on/2"]);
        assert_eq!(
            paths("provenance.locator_long"),
            vec!["/parameters/rds_on/0", "/parameters/rds_on/1"]
        );
    }

    #[test]
    fn zener_and_optocoupler_parameters_are_plausible() {
        use openparts_core::{ComponentCategory as C, ParameterValue};
        let num = |nominal: f64, unit: &str| ParameterValue {
            typ: Some(nominal),
            unit: Some(unit.into()),
            ..Default::default()
        };
        let implausible = |category: C, keys: &[(&str, ParameterValue)]| -> Vec<String> {
            let mut device = base_device();
            device.category = category;
            let mut part = base_part();
            for (key, value) in keys {
                part.parameters.insert((*key).into(), value.clone().into());
                part.provenance.insert(
                    format!("/parameters/{key}"),
                    vec![openparts_core::ProvenanceEntry {
                        source: SourceId::from("ex/DS1"),
                        page: Some(1),
                        locator: Some("Electrical characteristics".into()),
                    }],
                );
            }
            let diags = validate_part(&part, &device, &base_package(), &BTreeSet::new());
            assert!(
                !diags
                    .iter()
                    .any(|d| d.rule_id.starts_with("parameter.") && d.is_error()),
                "{diags:?}"
            );
            diags
                .iter()
                .filter(|d| d.rule_id == "parameter.implausible")
                .filter_map(|d| d.field_path.clone())
                .collect()
        };
        let zener = [
            ("zener_voltage", num(5.1, "V")),
            ("zener_test_current", num(5.0, "mA")),
            ("forward_voltage", num(0.9, "V")),
        ];
        assert!(implausible(C::Diode, &zener).is_empty());
        let opto = [
            ("isolation_voltage", num(5000.0, "V")),
            ("current_transfer_ratio", num(100.0, "%")),
            ("forward_voltage", num(1.25, "V")),
            ("forward_current", num(50.0, "mA")),
            ("reverse_voltage", num(5.0, "V")),
            ("collector_emitter_voltage", num(80.0, "V")),
            ("collector_current", num(50.0, "mA")),
        ];
        assert!(implausible(C::Optocoupler, &opto).is_empty());
        assert_eq!(
            implausible(C::Transistor, &opto[..2]),
            vec![
                "/parameters/current_transfer_ratio",
                "/parameters/isolation_voltage"
            ]
        );
        assert_eq!(
            implausible(C::Optocoupler, &zener[..1]),
            vec!["/parameters/zener_voltage"]
        );
    }

    #[test]
    fn timing_amplifier_and_logic_keys_are_plausible_and_ranged() {
        use openparts_core::{ComponentCategory as C, ParameterValue};
        let num = |nominal: f64, unit: &str| ParameterValue {
            typ: Some(nominal),
            unit: Some(unit.into()),
            ..Default::default()
        };
        let word = |v: &str| ParameterValue {
            value: Some(v.into()),
            ..Default::default()
        };
        let run = |category: C, keys: &[(&str, ParameterValue)]| -> Vec<(String, String)> {
            let mut device = base_device();
            device.category = category;
            let mut part = base_part();
            for (key, value) in keys {
                part.parameters.insert((*key).into(), value.clone().into());
                part.provenance.insert(
                    format!("/parameters/{key}"),
                    vec![openparts_core::ProvenanceEntry {
                        source: SourceId::from("ex/DS1"),
                        page: Some(1),
                        locator: Some("Electrical characteristics".into()),
                    }],
                );
            }
            let diags = validate_part(&part, &device, &base_package(), &BTreeSet::new());
            assert!(
                !diags
                    .iter()
                    .any(|d| d.rule_id.starts_with("parameter.") && d.is_error()),
                "{diags:?}"
            );
            diags
                .iter()
                .filter(|d| d.rule_id.starts_with("parameter."))
                .map(|d| {
                    (
                        d.rule_id.to_string(),
                        d.field_path.clone().unwrap_or_default(),
                    )
                })
                .collect()
        };
        // The categories added for motor/LED drivers, audio, sensors,
        // electromechanical parts, EMI parts and RF.
        let cases: &[(C, &[(&str, ParameterValue)])] = &[
            (
                C::MotorDriver,
                &[
                    ("motor_type", word("brushed_dc_or_stepper")),
                    ("motor_supply_voltage", num(10.8, "V")),
                    ("output_current", num(1.5, "A")),
                    ("output_current_peak", num(2.0, "A")),
                    ("microstepping", word("16")),
                    ("channel_count", word("2")),
                    ("rds_on", num(360.0, "mΩ")),
                    ("data_interface", word("step_dir")),
                ],
            ),
            (
                C::LedDriver,
                &[
                    ("led_driver_type", word("constant_current")),
                    ("channel_count", word("24")),
                    ("output_current", num(30.0, "mA")),
                ],
            ),
            (
                C::AudioAmplifier,
                &[
                    ("amplifier_class", word("d")),
                    ("output_power", num(2.5, "W")),
                    ("thd_n", num(10.0, "%")),
                    ("supply_voltage", num(5.0, "V")),
                    ("snr", num(90.0, "dB")),
                ],
            ),
            (
                C::Microphone,
                &[
                    ("sensitivity_digital", num(-26.0, "dBFS")),
                    ("acoustic_overload_point", num(120.0, "dBSPL")),
                    ("data_interface", word("pdm")),
                ],
            ),
            (
                C::Buzzer,
                &[
                    ("buzzer_type", word("magnetic")),
                    ("buzzer_drive", word("internal")),
                    ("rated_voltage", num(5.0, "V")),
                    ("frequency", num(2.3, "kHz")),
                    ("sound_pressure", num(85.0, "dB")),
                    ("coil_resistance", num(42.0, "Ω")),
                ],
            ),
            (
                C::Relay,
                &[
                    ("coil_voltage", num(5.0, "V")),
                    ("contact_form", word("1c")),
                    ("contact_current", num(10.0, "A")),
                    ("isolation_voltage", num(4000.0, "V")),
                ],
            ),
            (
                C::Potentiometer,
                &[
                    ("resistance", num(10.0, "kΩ")),
                    ("taper", word("linear")),
                    ("rotation_angle", num(240.0, "°")),
                ],
            ),
            (
                C::RotaryEncoder,
                &[
                    ("pulses_per_revolution", word("20")),
                    ("detents", word("20")),
                    ("push_switch", word("yes")),
                ],
            ),
            (
                C::FerriteBead,
                &[
                    ("impedance", num(600.0, "Ω")),
                    ("rated_current", num(500.0, "mA")),
                    ("dcr", num(0.15, "Ω")),
                ],
            ),
            (
                C::LightSensor,
                &[
                    ("light_sensor_type", word("ambient")),
                    ("illuminance_range", num(83.0, "klx")),
                    ("data_interface", word("i2c")),
                ],
            ),
            (
                C::GasSensor,
                &[
                    ("target_gas", word("co2")),
                    ("concentration_range", num(5000.0, "ppm")),
                ],
            ),
            (
                C::MagneticSensor,
                &[
                    ("magnetic_sensor_type", word("omnipolar_switch")),
                    ("operate_point", num(3.0, "mT")),
                    ("output_structure", word("push_pull")),
                ],
            ),
            (
                C::WirelessModule,
                &[
                    ("radio_protocol", word("wifi_ble")),
                    ("tx_power", num(20.0, "dBm")),
                    ("antenna_type", word("pcb_trace")),
                    ("supply_voltage", num(3.3, "V")),
                ],
            ),
            (
                C::Antenna,
                &[
                    ("frequency_band", num(2.45, "GHz")),
                    ("peak_gain", num(1.5, "dBi")),
                ],
            ),
        ];
        for (category, keys) in cases {
            assert!(run(*category, keys).is_empty(), "{category:?}");
        }
        // The schema-gap keys: PPTC fuses, analog switches, converters,
        // split supplies, linear Hall sensors, RTCs, isolators, triac
        // drivers / photorelays, relays, timers.
        let words = |ws: &[&str]| ParameterValue {
            values: ws.iter().map(|w| w.to_string()).collect(),
            ..Default::default()
        };
        let at = |mut v: ParameterValue, sym: &str, n: f64, unit: &str| {
            v.at.insert(
                sym.into(),
                openparts_core::Condition {
                    value: Some(n),
                    unit: unit.into(),
                    ..Default::default()
                },
            );
            v
        };
        let gaps: &[(C, Vec<(&str, ParameterValue)>)] = &[
            (
                C::Fuse,
                vec![
                    ("hold_current", at(num(0.5, "A"), "TA", 23.0, "°C")),
                    ("trip_current", num(1.0, "A")),
                    ("max_current", num(100.0, "A")),
                    ("max_voltage", num(13.2, "V")),
                ],
            ),
            (
                C::AnalogSwitch,
                vec![
                    ("on_resistance", num(0.8, "Ω")),
                    ("on_resistance_flatness", num(0.15, "Ω")),
                ],
            ),
            (
                C::Dac,
                vec![
                    ("resolution", word("12")),
                    ("sample_rate", num(1.0, "MSPS")),
                    ("data_interface", words(&["i2c", "spi"])),
                    ("reference", words(&["internal", "external"])),
                    ("analog_supply_voltage", num(5.0, "V")),
                    ("digital_supply_voltage", num(1.8, "V")),
                    ("analog_supply_current", num(1.2, "mA")),
                    ("digital_supply_current", num(30.0, "µA")),
                ],
            ),
            (C::Adc, vec![("data_interface", word("parallel"))]),
            (
                C::MagneticSensor,
                vec![
                    ("magnetic_sensitivity", num(2.5, "mV/G")),
                    ("bandwidth", num(20.0, "kHz")),
                ],
            ),
            (
                C::Rtc,
                vec![
                    (
                        "timekeeping_accuracy",
                        at(num(3.5, "ppm"), "TA", 25.0, "°C"),
                    ),
                    ("backup_current", at(num(0.45, "µA"), "VBAT", 3.0, "V")),
                    ("supply_current", at(num(70.0, "µA"), "FCLK", 400.0, "kHz")),
                ],
            ),
            (
                C::Optocoupler,
                vec![
                    ("supply_voltage", num(5.0, "V")),
                    ("propagation_delay", num(60.0, "ns")),
                    ("output_current_peak", num(2.5, "A")),
                    ("cmr", num(35.0, "kV/µs")),
                    ("trigger_current", num(10.0, "mA")),
                    ("off_state_voltage", num(600.0, "V")),
                ],
            ),
            (
                C::Relay,
                vec![
                    ("must_operate_voltage", num(3.75, "V")),
                    ("must_release_voltage", num(0.5, "V")),
                    ("input_impedance", num(1.0, "kΩ")),
                    ("on_resistance", num(2.0, "Ω")),
                    ("trigger_current", num(3.0, "mA")),
                ],
            ),
            (C::Timer, vec![("max_frequency", num(500.0, "kHz"))]),
            (
                C::LightSensor,
                vec![("light_sensor_type", word("ambient_color_proximity_gesture"))],
            ),
        ];
        for (category, keys) in gaps {
            assert!(run(*category, keys).is_empty(), "{category:?}");
        }
        // A % slip on an RTC's ppm accuracy.
        assert_eq!(
            run(C::Rtc, &[("timekeeping_accuracy", num(2.0, "%"))]),
            [(
                "parameter.implausible_value".to_string(),
                "/parameters/timekeeping_accuracy".to_string()
            )]
        );
        // A light sensor has no TX power.
        assert_eq!(
            run(C::LightSensor, &[("tx_power", num(0.0, "dBm"))]),
            [(
                "parameter.implausible".to_string(),
                "/parameters/tx_power".to_string()
            )]
        );
        let osc = [
            ("frequency", num(25.0, "MHz")),
            ("frequency_stability", num(50.0, "ppm")),
            ("supply_voltage", num(3.3, "V")),
            ("clock_output", word("cmos")),
            ("operating_temperature", num(25.0, "°C")),
        ];
        assert!(run(C::Oscillator, &osc).is_empty());
        let amp = [
            ("gain_bandwidth", num(1.0, "MHz")),
            ("slew_rate", num(0.6, "V/µs")),
            ("input_bias_current", num(1.0, "pA")),
            ("rail_to_rail_output", word("yes")),
            ("channel_count", word("2")),
        ];
        assert!(run(C::OpAmp, &amp).is_empty());
        let logic = [
            ("propagation_delay", num(3.5, "ns")),
            ("output_current_low", num(24.0, "mA")),
            ("logic_function", word("nand")),
        ];
        assert!(run(C::Logic, &logic).is_empty());
        // 20 % is not 20 ppm; a viewing angle of 1200° is a slip too.
        let slips = [
            ("frequency_tolerance", num(20.0, "%")),
            ("viewing_angle", num(1200.0, "°")),
        ];
        let d = run(C::Crystal, &slips);
        assert!(d.contains(&(
            "parameter.implausible_value".into(),
            "/parameters/frequency_tolerance".into()
        )));
        assert!(d.contains(&(
            "parameter.implausible_value".into(),
            "/parameters/viewing_angle".into()
        )));
    }
}
