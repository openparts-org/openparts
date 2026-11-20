//! openparts-pcbcraft: PCBCraft native library adapter behind the
//! openparts-pcbcad IR (Architecture Specification sections 12/13).
//!
//! PCBCraft (github.com/tangency-labs/pcbcraft) reads a library folder
//! -- `library.json` plus flat `symbols/`, `footprints/`, `categories/`,
//! `parts/` and `manufacturer-parts/` directories of canonical JSON
//! records -- or the same documents in one `pcbcraft-library-bundle/1`
//! file. Records are addressed by (library, kind, id, revision), and a
//! revision is immutable. The mapping:
//!
//! | OpenParts | PCBCraft |
//! |---|---|
//! | Device (its IR symbol) | `symbols/<id>.json` + generic part `parts/<id>.json` |
//! | Package (its Placement Model footprint) | `footprints/<id>.json`, one package variant of each generic part using it |
//! | Part (MPN) | `manufacturer-parts/<id>.json`, with typed specifications |
//! | ComponentCategory | `categories/<id>.json` with the fields its parts state |
//!
//! Every format detail follows PCBCraft's published JSON Schemas
//! (`docs/schemas/*.schema.json`) and the rules of its part, symbol and
//! footprint schema documents. What PCBCraft cannot represent is not
//! approximated: the part is refused, with the reason
//! ([`Library::refusals`]).
//!
//! This crate is pure: the caller (the CLI) builds the IR and supplies
//! the licence and provenance; ids and revisions are kept stable
//! across exports by a [`ledger::Ledger`].

mod footprint;
mod labels;
pub mod ledger;
pub mod record;
mod spec;
mod symbol;

use ledger::{sha256, LedgerLibrary};
use openparts_core::parameter::Parameters;
use openparts_core::ComponentCategory;
use openparts_pcbcad::{PcbFootprint, PcbSymbol};
use record::*;
use std::collections::{BTreeMap, BTreeSet};

/// A reason a record cannot be written.
pub(crate) type Refuse = String;

/// PCBCraft's source bounds per registered library (folder or bundle).
pub const MAX_FILES: usize = 8192;
pub const MAX_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_LIBRARY_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_BUNDLE_BYTES: usize = 16 * 1024 * 1024;

/// What the caller decides; nothing here has a default in this crate.
#[derive(Debug, Clone)]
pub struct Options {
    /// The library id (1-64 lowercase letters, digits, inner hyphens).
    pub library_id: String,
    pub library_name: String,
    /// The licence every record declares (an SPDX expression).
    pub license: String,
    /// The source triple's `reference`: which dataset revision and
    /// generator made the records.
    pub source_reference: String,
}

/// A device: its symbol and generic part.
#[derive(Debug, Clone)]
pub struct DeviceInput {
    /// The OpenParts device id.
    pub id: String,
    pub name: String,
    pub category: ComponentCategory,
    pub symbol: PcbSymbol,
    /// Where the device is described (its registry URL).
    pub locator: String,
}

/// A package: its footprint without any device's pin data.
#[derive(Debug, Clone)]
pub struct PackageInput {
    pub id: String,
    pub name: String,
    pub footprint: Result<PcbFootprint, String>,
    pub locator: String,
}

/// An orderable part.
#[derive(Debug, Clone)]
pub struct PartInput {
    pub id: String,
    pub mpn: String,
    /// The manufacturer's display name.
    pub manufacturer: String,
    pub device: String,
    pub package: String,
    /// The footprint built with the part's device (shared contacts,
    /// polarity marks); written as the package's own footprint when it
    /// equals [`PackageInput::footprint`], else as a device-specific one.
    pub footprint: Result<PcbFootprint, String>,
    pub datasheet: Option<String>,
    pub locator: String,
    /// The part's effective parameters (device values overlaid by the
    /// part's own).
    pub parameters: Parameters,
}

#[derive(Debug, Clone, Default)]
pub struct Input {
    pub devices: Vec<DeviceInput>,
    pub packages: Vec<PackageInput>,
    pub parts: Vec<PartInput>,
}

/// A part left out, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub part: String,
    pub reason: String,
}

/// One native library.
#[derive(Debug, Clone)]
pub struct Library {
    pub id: String,
    /// Relative path (`library.json`, `parts/<id>.json`...) -> text.
    pub documents: BTreeMap<String, String>,
    pub refusals: Vec<Refusal>,
    /// Manufacturer parts written.
    pub parts: usize,
    /// OpenParts part id -> its manufacturer part's document path
    /// (`manufacturer-parts/<id>.json`).
    pub manufacturer_parts: BTreeMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("{0}")]
    Invalid(String),
}

/// `library-common-v1#/$defs/id`.
pub fn check_id(id: &str) -> Result<(), String> {
    let ok = (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-')
        && !id.ends_with('-');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{id:?} is not 1-64 lowercase letters, digits and inner hyphens"
        ))
    }
}

/// `library-common-v1#/$defs/terminal_id` (pin and pad numbers).
pub(crate) fn check_terminal_id(id: &str) -> Result<(), String> {
    let ok = (1..=8).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'_' | b'-'));
    if ok {
        Ok(())
    } else {
        Err(format!("{id:?} is not 1-8 letters, digits, +, - or _"))
    }
}

/// Trimmed, control-free text of at most `max` characters, or a refusal.
pub(crate) fn clean_text(what: &str, s: &str, max: usize) -> Result<String, Refuse> {
    let t = s.trim();
    if t.is_empty() {
        return Err(format!("the {what} is empty"));
    }
    if t.chars().any(char::is_control) {
        return Err(format!("the {what} {t:?} contains control characters"));
    }
    if t.chars().count() > max {
        return Err(format!("the {what} {t:?} is longer than {max} characters"));
    }
    Ok(t.to_string())
}

/// A display name: control characters dropped, cut to `max` characters
/// (and bytes) with a trailing ellipsis.
pub(crate) fn display(what: &str, s: &str, max: usize) -> Result<String, Refuse> {
    let t: String = s.trim().chars().filter(|c| !c.is_control()).collect();
    if t.is_empty() {
        return Err(format!("the {what} is empty"));
    }
    if t.chars().count() <= max && t.len() <= max {
        return Ok(t);
    }
    let mut out = String::new();
    for c in t.chars() {
        if out.chars().count() + 1 >= max || out.len() + c.len_utf8() + '…'.len_utf8() > max {
            break;
        }
        out.push(c);
    }
    Ok(format!("{}…", out.trim_end()))
}

/// Millimetres -> integer nanometres.
pub(crate) fn nm(mm: f64) -> Result<i64, Refuse> {
    let v = (mm * 1e6).round();
    if !v.is_finite() || v.abs() > 1e12 {
        return Err(format!("the length {mm} mm is not representable"));
    }
    Ok(v as i64)
}

/// `manufacturer-part-v1#/properties/datasheet`.
fn datasheet(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let ok = url.len() <= 2048
        && host_end > 0
        && !url.chars().any(char::is_whitespace)
        && !url.chars().any(char::is_control);
    ok.then(|| url.to_string())
}

/// A record id from an OpenParts id: lowercase, other characters
/// folded to hyphens.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn hashed(candidate: &str, key: &str, len: usize) -> String {
    let base: String = candidate.chars().take(63 - len).collect();
    let base = base.trim_end_matches('-');
    let hash = &sha256(key.as_bytes())[..len];
    if base.is_empty() {
        format!("x-{hash}")
    } else {
        format!("{base}-{hash}")
    }
}

/// Ids for `(key, slug source)` pairs of one kind: the ledger's, else
/// the slug, else -- when the slug is too long or is not this key's
/// alone (taken in the ledger, or shared with another key here) -- the
/// slug with a hash of the key. Collision-safe and, with the ledger,
/// stable whatever else an export contains.
fn assign_ids(
    ledger: &LedgerLibrary,
    kind: &str,
    keys: &[(String, String)],
) -> BTreeMap<String, String> {
    let known = ledger.ids(kind);
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let mut groups: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (key, source) in keys {
        match known.get(key) {
            Some(id) => {
                out.insert(key.clone(), id.clone());
            }
            None => groups.entry(slug(source)).or_default().push(key),
        }
    }
    let mut used: BTreeSet<String> = out.values().cloned().collect();
    for (candidate, members) in &groups {
        for key in members {
            let free = |id: &str, used: &BTreeSet<String>| {
                ledger.taken(kind, id).is_none() && !used.contains(id)
            };
            let mut id = candidate.clone();
            if members.len() > 1 || check_id(&id).is_err() || !free(&id, &used) {
                id = hashed(candidate, key, 8);
                if !free(&id, &used) {
                    id = hashed(candidate, key, 16);
                }
            }
            used.insert(id.clone());
            out.insert(key.to_string(), id);
        }
    }
    out
}

/// Gives `record` its revision from the ledger (and, for a reused
/// revision, that revision's own source reference).
fn publish<T: Revisioned>(
    ledger: &mut LedgerLibrary,
    kind: &str,
    id: &str,
    key: &str,
    mut record: T,
) -> (T, u32) {
    let mut blank = record.clone();
    blank.set_revision(0);
    let reference = blank.reference_mut().map(std::mem::take);
    let digest = sha256(render(&blank).as_bytes());
    let (revision, reference) = ledger.revision(kind, id, key, &digest, reference.as_deref());
    record.set_revision(revision);
    if let (Some(slot), Some(reference)) = (record.reference_mut(), reference) {
        *slot = reference;
    }
    (record, revision)
}

fn humanize(snake: &str) -> String {
    let s = snake.replace('_', " ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => s,
    }
}

/// Terminal -> its pads.
type Mapping = BTreeMap<String, Vec<String>>;

/// Every terminal bound to its pads; the other pads unconnected.
fn map_pads(
    symbol: &symbol::SymbolDraft,
    fp: &footprint::FootprintDraft,
) -> Result<(Mapping, Vec<String>), Refuse> {
    let mut mapping = BTreeMap::new();
    let mut used = BTreeSet::new();
    for (terminal, pins) in &symbol.terminals {
        let pads: Vec<String> = pins
            .iter()
            .flat_map(|p| fp.pads.get(p).cloned().unwrap_or_default())
            .collect();
        if pads.is_empty() {
            return Err(format!(
                "pin {} has no pad in the footprint",
                pins.join("/")
            ));
        }
        used.extend(pads.iter().cloned());
        mapping.insert(terminal.clone(), pads);
    }
    let unconnected = fp
        .record
        .pads
        .iter()
        .map(|p| p.number.clone())
        .filter(|n| !used.contains(n))
        .collect();
    Ok((mapping, unconnected))
}

struct Accepted<'a> {
    part: &'a PartInput,
    device: &'a DeviceInput,
    footprint: String,
    mapping: BTreeMap<String, Vec<String>>,
    unconnected: Vec<String>,
    specs: Vec<(spec::FieldDef, ParameterValue)>,
    datasheet: Option<String>,
}

/// Builds one native library from `input`, taking ids and revisions
/// from (and recording new ones in) `ledger`.
pub fn export(
    input: &Input,
    options: &Options,
    ledger: &mut LedgerLibrary,
) -> Result<Library, ExportError> {
    let invalid = ExportError::Invalid;
    check_id(&options.library_id).map_err(|e| invalid(format!("library id {e}")))?;
    let library_name = clean_text("library name", &options.library_name, 64).map_err(invalid)?;
    let license = clean_text("license", &options.license, 128).map_err(invalid)?;
    let reference =
        clean_text("source reference", &options.source_reference, 2048).map_err(invalid)?;
    let library = options.library_id.clone();
    let source = |locator: &str| Source {
        kind: "openparts".into(),
        reference: reference.clone(),
        locator: locator.to_string(),
    };

    let devices: BTreeMap<&str, &DeviceInput> =
        input.devices.iter().map(|d| (d.id.as_str(), d)).collect();
    let packages: BTreeMap<&str, &PackageInput> =
        input.packages.iter().map(|p| (p.id.as_str(), p)).collect();
    let symbols: BTreeMap<&str, Result<symbol::SymbolDraft, Refuse>> = devices
        .iter()
        .map(|(id, d)| {
            let name = display("device name", &d.name, 64);
            (*id, name.and_then(|n| symbol::convert(&d.symbol, &n)))
        })
        .collect();
    let generic: BTreeMap<&str, Result<footprint::FootprintDraft, Refuse>> = packages
        .iter()
        .map(|(id, p)| {
            let fp = p
                .footprint
                .clone()
                .and_then(|f| footprint::convert(&f, &p.name));
            (*id, fp)
        })
        .collect();

    // Footprint key -> (draft, slug source, locator).
    let mut footprints: BTreeMap<String, (footprint::FootprintDraft, String, String)> =
        BTreeMap::new();
    let mut accepted = Vec::new();
    let mut refusals = Vec::new();
    let mut parts: Vec<&PartInput> = input.parts.iter().collect();
    parts.sort_by(|a, b| a.id.cmp(&b.id));
    for part in parts {
        let result = (|| -> Result<Accepted, Refuse> {
            let device = devices
                .get(part.device.as_str())
                .ok_or_else(|| format!("device {} is not in the export", part.device))?;
            let package = packages
                .get(part.package.as_str())
                .ok_or_else(|| format!("package {} is not in the export", part.package))?;
            let symbol = symbols[part.device.as_str()]
                .as_ref()
                .map_err(|e| format!("symbol: {e}"))?;
            let own = part
                .footprint
                .clone()
                .and_then(|f| footprint::convert(&f, &package.name))
                .map_err(|e| format!("footprint: {e}"))?;
            let (key, draft, source) = match &generic[part.package.as_str()] {
                Ok(g) if render(&g.record) == render(&own.record) => (
                    format!("package:{}", package.id),
                    g.clone(),
                    package.id.clone(),
                ),
                _ => {
                    let mut own = own;
                    own.record.name = display(
                        "footprint name",
                        &format!("{} ({})", package.name, device.name),
                        200,
                    )?;
                    (
                        format!("package:{}@device:{}", package.id, device.id),
                        own,
                        format!("{}-{}", package.id, device.id),
                    )
                }
            };
            let (mapping, unconnected) =
                map_pads(symbol, &draft).map_err(|e| format!("package map: {e}"))?;
            clean_text("MPN", &part.mpn, 128)?;
            clean_text("manufacturer", &part.manufacturer, 128)?;
            clean_text("locator", &part.locator, 1024)?;
            clean_text("device locator", &device.locator, 1024)?;
            clean_text("package locator", &package.locator, 1024)?;
            footprints
                .entry(key.clone())
                .or_insert_with(|| (draft, source, package.locator.clone()));
            Ok(Accepted {
                part,
                device,
                footprint: key,
                mapping,
                unconnected,
                specs: spec::specifications(&part.parameters),
                datasheet: part.datasheet.as_deref().and_then(datasheet),
            })
        })();
        match result {
            Ok(a) => accepted.push(a),
            Err(reason) => refusals.push(Refusal {
                part: part.id.clone(),
                reason,
            }),
        }
    }

    // Categories: the fields their accepted parts state, at most 64
    // (the most used first).
    let mut usage: BTreeMap<ComponentCategory, BTreeMap<String, (usize, spec::FieldDef)>> =
        BTreeMap::new();
    for a in &accepted {
        let fields = usage.entry(a.device.category).or_default();
        for (f, _) in &a.specs {
            fields.entry(f.id.clone()).or_insert((0, f.clone())).0 += 1;
        }
    }
    let mut category_fields: BTreeMap<ComponentCategory, BTreeMap<String, Field>> = BTreeMap::new();
    for (category, fields) in usage {
        let mut ranked: Vec<_> = fields.into_values().collect();
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
        let chosen: BTreeMap<String, Field> = ranked
            .into_iter()
            .take(64)
            .map(|(_, f)| {
                (
                    f.id,
                    Field {
                        name: f.name,
                        value_type: f.value_type,
                    },
                )
            })
            .collect();
        if !chosen.is_empty() {
            category_fields.insert(category, chosen);
        }
    }

    let category_key = |c: ComponentCategory| format!("category:{}", c.name());
    let category_ids = assign_ids(
        ledger,
        "category",
        &category_fields
            .keys()
            .map(|c| (category_key(*c), c.name().to_string()))
            .collect::<Vec<_>>(),
    );
    let used_devices: BTreeSet<&str> = accepted.iter().map(|a| a.device.id.as_str()).collect();
    let device_keys: Vec<(String, String)> = used_devices
        .iter()
        .map(|d| (format!("device:{d}"), d.to_string()))
        .collect();
    let symbol_ids = assign_ids(ledger, "symbol", &device_keys);
    let part_ids = assign_ids(ledger, "part", &device_keys);
    let footprint_ids = assign_ids(
        ledger,
        "footprint",
        &footprints
            .iter()
            .map(|(k, (_, s, _))| (k.clone(), s.clone()))
            .collect::<Vec<_>>(),
    );
    let mpart_ids = assign_ids(
        ledger,
        "manufacturer-part",
        &accepted
            .iter()
            .map(|a| (format!("part:{}", a.part.id), a.part.id.clone()))
            .collect::<Vec<_>>(),
    );

    let mut documents = BTreeMap::new();
    documents.insert(
        "library.json".to_string(),
        render(&Manifest {
            format: LIBRARY_FORMAT,
            id: library.clone(),
            name: library_name,
            license: license.clone(),
        }),
    );
    let reference_to = |id: &str, revision: u32| Reference {
        library: library.clone(),
        id: id.to_string(),
        revision,
    };

    let mut category_refs: BTreeMap<ComponentCategory, (Reference, BTreeSet<String>)> =
        BTreeMap::new();
    for (category, fields) in category_fields {
        let key = category_key(category);
        let id = &category_ids[&key];
        let field_ids = fields.keys().cloned().collect();
        let record = Category {
            format: CATEGORY_FORMAT,
            library: library.clone(),
            id: id.clone(),
            revision: 0,
            name: humanize(category.name()),
            license: license.clone(),
            fields,
        };
        let (record, revision) = publish(ledger, "category", id, &key, record);
        documents.insert(format!("categories/{id}.json"), render(&record));
        category_refs.insert(category, (reference_to(id, revision), field_ids));
    }

    let mut symbol_refs = BTreeMap::new();
    for device in &used_devices {
        let key = format!("device:{device}");
        let id = &symbol_ids[&key];
        let mut record = symbols[device].as_ref().expect("accepted").record.clone();
        record.id = id.clone();
        let (record, revision) = publish(ledger, "symbol", id, &key, record);
        documents.insert(format!("symbols/{id}.json"), render(&record));
        symbol_refs.insert(*device, reference_to(id, revision));
    }

    let mut footprint_refs = BTreeMap::new();
    for (key, (draft, _, locator)) in &footprints {
        let id = &footprint_ids[key];
        let mut record = draft.record.clone();
        record.library = library.clone();
        record.id = id.clone();
        record.license = license.clone();
        record.source = source(locator);
        let (record, revision) = publish(ledger, "footprint", id, key, record);
        documents.insert(format!("footprints/{id}.json"), render(&record));
        footprint_refs.insert(key.clone(), reference_to(id, revision));
    }

    let mut part_refs = BTreeMap::new();
    for device in &used_devices {
        let d = devices[device];
        let key = format!("device:{device}");
        let id = &part_ids[&key];
        let mut packages = BTreeMap::new();
        for a in accepted.iter().filter(|a| a.device.id == *device) {
            let fp = &footprint_refs[&a.footprint];
            packages
                .entry(fp.id.clone())
                .or_insert_with(|| PackageVariant {
                    name: footprints[&a.footprint]
                        .0
                        .record
                        .name
                        .chars()
                        .take(64)
                        .collect(),
                    footprint: fp.clone(),
                    mapping: a.mapping.clone(),
                    unconnected_pads: a.unconnected.clone(),
                });
        }
        let category = category_refs
            .get(&d.category)
            .map(|(r, fields)| CategoryBinding {
                definition: r.clone(),
                primary_value: spec::primary_field(d.category)
                    .filter(|f| fields.contains(*f))
                    .map(str::to_string),
            });
        let record = Part {
            format: PART_FORMAT,
            library: library.clone(),
            id: id.clone(),
            revision: 0,
            name: display("device name", &d.name, 64).expect("checked"),
            license: license.clone(),
            source: source(&d.locator),
            symbol: symbol_refs[device].clone(),
            category,
            packages,
        };
        let (record, revision) = publish(ledger, "part", id, &key, record);
        documents.insert(format!("parts/{id}.json"), render(&record));
        part_refs.insert(*device, reference_to(id, revision));
    }

    let mut manufacturer_parts = BTreeMap::new();
    for a in &accepted {
        let key = format!("part:{}", a.part.id);
        let id = &mpart_ids[&key];
        let fields = category_refs.get(&a.device.category).map(|(_, f)| f);
        let specifications = a
            .specs
            .iter()
            .filter(|(f, _)| fields.is_some_and(|fs| fs.contains(&f.id)))
            .map(|(f, v)| (f.id.clone(), v.clone()))
            .collect();
        let record = ManufacturerPart {
            format: MANUFACTURER_PART_FORMAT,
            library: library.clone(),
            id: id.clone(),
            revision: 0,
            name: display("MPN", &a.part.mpn, 64).expect("checked"),
            license: license.clone(),
            source: source(&a.part.locator),
            manufacturer: a.part.manufacturer.trim().to_string(),
            mpn: a.part.mpn.trim().to_string(),
            part: part_refs[a.device.id.as_str()].clone(),
            package: footprint_refs[&a.footprint].id.clone(),
            datasheet: a.datasheet.clone(),
            specifications,
        };
        let (record, _) = publish(ledger, "manufacturer-part", id, &key, record);
        let path = format!("manufacturer-parts/{id}.json");
        documents.insert(path.clone(), render(&record));
        manufacturer_parts.insert(a.part.id.clone(), path);
    }

    Ok(Library {
        id: library,
        parts: accepted.len(),
        manufacturer_parts,
        documents,
        refusals,
    })
}

impl Library {
    /// PCBCraft's per-source bounds: 8,192 files, 1 MiB per file,
    /// 32 MiB in all.
    pub fn check_limits(&self) -> Result<(), String> {
        if self.documents.len() > MAX_FILES {
            return Err(format!(
                "{} files: a PCBCraft library source holds at most {MAX_FILES}",
                self.documents.len()
            ));
        }
        if let Some((path, text)) = self
            .documents
            .iter()
            .find(|(_, t)| t.len() > MAX_FILE_BYTES)
        {
            return Err(format!(
                "{path} is {} bytes: PCBCraft reads at most 1 MiB per file",
                text.len()
            ));
        }
        let total: usize = self.documents.values().map(String::len).sum();
        if total > MAX_LIBRARY_BYTES {
            return Err(format!(
                "{total} bytes: a PCBCraft library source holds at most 32 MiB"
            ));
        }
        Ok(())
    }

    /// The `pcbcraft-library-bundle/1` document.
    pub fn bundle(&self) -> String {
        bundle_of(&self.documents)
    }

    /// The documents one OpenParts part needs, as they are in this
    /// library: `library.json`, its manufacturer part, generic part,
    /// symbol, category and the footprint of every package variant of
    /// the generic part (its exact dependency closure). `None` if the
    /// part is not in the library (or was refused).
    pub fn part_documents(&self, part: &str) -> Option<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        let mut take = |path: String| -> Option<serde_json::Value> {
            let text = self.documents.get(&path)?;
            let value = serde_json::from_str(text).ok()?;
            out.insert(path, text.clone());
            Some(value)
        };
        take("library.json".into())?;
        let mpart = take(self.manufacturer_parts.get(part)?.clone())?;
        // References inside one library: the id names the file of the
        // kind the field refers to.
        let id = |v: &serde_json::Value| -> Option<String> {
            (v.get("library")?.as_str()? == self.id).then_some(())?;
            Some(v.get("id")?.as_str()?.to_string())
        };
        let generic = take(format!("parts/{}.json", id(mpart.get("part")?)?))?;
        take(format!("symbols/{}.json", id(generic.get("symbol")?)?))?;
        if let Some(c) = generic.get("category") {
            take(format!("categories/{}.json", id(c.get("definition")?)?))?;
        }
        for variant in generic.get("packages")?.as_object()?.values() {
            take(format!(
                "footprints/{}.json",
                id(variant.get("footprint")?)?
            ))?;
        }
        Some(out)
    }
}

/// A `pcbcraft-library-bundle/1` document of `documents`.
pub fn bundle_of(documents: &BTreeMap<String, String>) -> String {
    #[derive(serde::Serialize)]
    struct Bundle<'a> {
        format: &'static str,
        documents: &'a BTreeMap<String, String>,
    }
    render(&Bundle {
        format: BUNDLE_FORMAT,
        documents,
    })
}

#[cfg(test)]
mod tests;
