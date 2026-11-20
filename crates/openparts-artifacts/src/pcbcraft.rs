//! Dataset parts -> PCBCraft native libraries (openparts-pcbcraft): the
//! one input builder `openparts pcbcraft` and openparts-server share, so
//! a part fetched from the server and a library exported by the CLI
//! carry the same records under the same addresses.

use crate::PartBuild;
use openparts_core::{ComponentCategory, Part};
use openparts_data::Dataset;
use openparts_pcbcad::openparts_placement::{self, Density};
use openparts_pcbcraft::ledger::Ledger;
use openparts_pcbcraft::{
    DeviceInput, ExportError, Input, Library, PackageInput, PartInput, Refusal,
};
use std::collections::BTreeMap;

pub use openparts_pcbcraft as adapter;

/// How parts are split into libraries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    /// One library for everything (at most 8,192 files: PCBCraft's
    /// per-source limit).
    None,
    /// One library per top-level category (`<library-id>-<group>`).
    CategoryGroup,
}

#[derive(Debug, Clone)]
pub struct Settings {
    /// The library id (the prefix with [`GroupBy::CategoryGroup`]).
    pub library_id: String,
    pub library_name: String,
    /// The licence every record declares (SPDX).
    pub license: String,
    /// The source triple's `reference`.
    pub source_reference: String,
    /// The registry the records' locators point at (no trailing `/`).
    pub locator_base: String,
    pub group_by: GroupBy,
}

/// `openparts-data <revision>; openparts-pcbcraft <version>`.
pub fn source_reference(dataset_revision: &str) -> String {
    format!(
        "openparts-data {dataset_revision}; openparts-pcbcraft {}",
        env!("CARGO_PKG_VERSION")
    )
}

/// The library id and name of a category group (`openparts-power`).
pub fn library_of(settings: &Settings, group: Option<ComponentCategory>) -> (String, String) {
    match group {
        None => (settings.library_id.clone(), settings.library_name.clone()),
        Some(c) => {
            let slug = c.name().replace('_', "-");
            (
                format!("{}-{slug}", settings.library_id),
                format!("{} {}", settings.library_name, slug.replace('-', " ")),
            )
        }
    }
}

/// The library group of `part` under `settings`.
pub fn group_of(ds: &Dataset, settings: &Settings, part: &Part) -> Option<ComponentCategory> {
    match settings.group_by {
        GroupBy::None => None,
        GroupBy::CategoryGroup => ds.devices.get(&part.device.0).map(|d| d.category.root()),
    }
}

/// Percent-encodes one URL path segment (an MPN may hold `/`).
pub fn segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The export inputs per library group, and the parts that could not
/// even be built (`not buildable: ...`).
pub fn inputs(
    ds: &Dataset,
    parts: &[&Part],
    settings: &Settings,
) -> BTreeMap<Option<ComponentCategory>, (Input, Vec<Refusal>)> {
    let base = settings.locator_base.trim_end_matches('/');
    let mut groups: BTreeMap<Option<ComponentCategory>, (Input, Vec<Refusal>)> = BTreeMap::new();
    for part in parts {
        let (input, refused) = groups.entry(group_of(ds, settings, part)).or_default();
        let build = match PartBuild::from_dataset(ds, part, None) {
            Ok(b) => b,
            Err(e) => {
                refused.push(Refusal {
                    part: part.id.0.clone(),
                    reason: format!("not buildable: {e}"),
                });
                continue;
            }
        };
        let device = &build.model.device;
        let package = &build.model.package;
        if !input.devices.iter().any(|d| d.id == device.id.0) {
            input.devices.push(DeviceInput {
                id: device.id.0.clone(),
                name: device.id.0.clone(),
                category: device.category,
                symbol: openparts_pcbcad::build_symbol(&device.id.0, device),
                locator: format!("{base}/v1/devices/{}", device.id.0),
            });
        }
        if !input.packages.iter().any(|p| p.id == package.id.0) {
            // The package's own footprint: its Placement Model without
            // any device's pins.
            let footprint = build.geometry().map_err(|e| e.to_string()).map(|g| {
                let model = openparts_placement::build(package, None, g, Density::N);
                openparts_pcbcad::footprint_from_placement(&package.id.0, &model)
            });
            input.packages.push(PackageInput {
                id: package.id.0.clone(),
                name: package.id.0.clone(),
                footprint,
                locator: format!("{base}/v1/packages/{}", package.id.0),
            });
        }
        let effective =
            openparts_core::parameter::effective_parameters(&part.parameters, &device.parameters);
        input.parts.push(PartInput {
            id: part.id.0.clone(),
            mpn: part.mpn.clone(),
            manufacturer: ds
                .manufacturers
                .get(&part.manufacturer.0)
                .map_or_else(|| part.manufacturer.0.clone(), |m| m.name.clone()),
            device: device.id.0.clone(),
            package: package.id.0.clone(),
            footprint: build.footprint().map_err(|e| e.to_string()),
            datasheet: build.datasheet.clone(),
            locator: format!(
                "{base}/v1/parts/{}/{}",
                segment(&part.manufacturer.0),
                segment(&part.mpn)
            ),
            parameters: effective
                .into_iter()
                .map(|(k, (v, _))| (k.to_string(), v.clone()))
                .collect(),
        });
    }
    groups
}

/// One exported library and the group it is.
#[derive(Debug, Clone)]
pub struct Exported {
    pub group: Option<ComponentCategory>,
    /// Its records; `refusals` include the parts that could not be built.
    pub library: Library,
}

/// Exports `parts` as libraries, taking ids and revisions from (and
/// recording new ones in) `ledger`. Limits are not checked here
/// ([`Library::check_limits`]).
pub fn export(
    ds: &Dataset,
    parts: &[&Part],
    settings: &Settings,
    ledger: &mut Ledger,
) -> Result<Vec<Exported>, ExportError> {
    let mut out = Vec::new();
    for (group, (input, refused)) in inputs(ds, parts, settings) {
        let (id, name) = library_of(settings, group);
        let options = openparts_pcbcraft::Options {
            library_id: id.clone(),
            library_name: name,
            license: settings.license.clone(),
            source_reference: settings.source_reference.clone(),
        };
        let mut library = openparts_pcbcraft::export(&input, &options, ledger.library_mut(&id))?;
        library.refusals.extend(refused);
        library.refusals.sort_by(|a, b| a.part.cmp(&b.part));
        out.push(Exported { group, library });
    }
    Ok(out)
}
