//! Checks across documents and of formats: device pins against package
//! pads, provenance paths against the document, evidence for verified
//! existence, date formats and where each file lives.

use crate::Diagnostic;
use openparts_core::{Device, ExistenceStatus, Package, Part, ProvenanceMap, Source};
use std::collections::BTreeSet;

/// Every device pin has a pad and every pad a pin. A pin cut short at
/// the body (`tab.trimmed_leads`) needs no pad, nor does one sharing a
/// pin's contact (`connected_to`); a pad without a pin is
/// only a warning (an exposed pad the datasheet doesn't assign is left
/// out on purpose). Packages that don't generate are skipped: the
/// generation tests report those.
pub(crate) fn check_pins_and_pads(
    entity: &str,
    device: &Device,
    package: &Package,
) -> Vec<Diagnostic> {
    let Ok(g) = openparts_mcad::generate(package) else {
        return vec![];
    };
    let lands = if g.lands.is_empty() {
        &g.leads
    } else {
        &g.lands
    };
    let pads: BTreeSet<&str> = lands
        .iter()
        .filter(|l| l.mounting != openparts_mcad::Mounting::NonPlated)
        .map(|l| l.number.as_str())
        .collect();
    let trimmed: BTreeSet<&str> = package
        .tab
        .iter()
        .flat_map(|t| t.trimmed_leads.iter().map(String::as_str))
        .collect();
    let mut out = Vec::new();
    // A pin sharing another pin's contact (`connected_to`) gets that pad
    // overlaid under its own number (`openparts_pcbcad::add_shared_pads`).
    let shares_pad = |pin: &openparts_core::Pin| {
        pin.connected_to
            .as_deref()
            .is_some_and(|t| pads.contains(t))
    };
    for (number, pin) in &device.pins {
        if !pads.contains(number.as_str()) && !trimmed.contains(number.as_str()) && !shares_pad(pin)
        {
            out.push(Diagnostic::error(
                "pin.no_pad",
                entity,
                Some(&format!("/pins/{}", pointer_escape(number))),
                format!(
                    "pin \"{number}\" has no pad in package {} (pads: {})",
                    package.id,
                    pads.iter().copied().collect::<Vec<_>>().join(", ")
                ),
            ));
        }
    }
    for pad in pads {
        if pad != "EP" && !device.pins.contains_key(pad) {
            out.push(Diagnostic::warning(
                "pad.no_pin",
                entity,
                None,
                format!("pad \"{pad}\" of package {} has no device pin", package.id),
            ));
        }
    }
    out
}

/// Each provenance key points at a field the document actually has.
pub(crate) fn check_provenance_paths<T: serde::Serialize>(
    entity: &str,
    doc: &T,
    provenance: &ProvenanceMap,
) -> Vec<Diagnostic> {
    let Ok(value) = serde_json::to_value(doc) else {
        return vec![];
    };
    provenance
        .keys()
        .filter(|path| path.starts_with('/') && value.pointer(path).is_none())
        .map(|path| {
            Diagnostic::error(
                "provenance.unknown_path",
                entity,
                Some(path),
                format!("provenance names {path}, which the document doesn't have"),
            )
        })
        .collect()
}

/// `existence: verified` cites where the MPN is printed.
pub(crate) fn check_existence_evidence(entity: &str, part: &Part) -> Vec<Diagnostic> {
    let verified = matches!(
        part.existence.status,
        ExistenceStatus::Verified | ExistenceStatus::HistoricalVerified
    );
    let cited = part
        .provenance
        .keys()
        .any(|k| k == "/existence" || k.starts_with("/existence/"));
    if verified && !cited {
        vec![Diagnostic::warning(
            "existence.unsupported",
            entity,
            Some("/existence"),
            "existence is verified but no /existence provenance says where the MPN is printed",
        )]
    } else {
        vec![]
    }
}

/// A Source's `date` is `YYYY`, `YYYY-MM`, `YYYY-MM-DD` or `unknown`;
/// `retrieved` is `YYYY-MM-DD`.
pub fn validate_source(source: &Source) -> Vec<Diagnostic> {
    let entity = format!("source:{}", source.id);
    let mut out = Vec::new();
    if source.date != "unknown" && !is_date(&source.date, 1) {
        out.push(Diagnostic::error(
            "source.date_format",
            &entity,
            Some("/date"),
            format!(
                "date \"{}\" is not YYYY, YYYY-MM, YYYY-MM-DD or unknown",
                source.date
            ),
        ));
    }
    if let Some(r) = &source.retrieved {
        if !is_date(r, 3) {
            out.push(Diagnostic::error(
                "source.date_format",
                &entity,
                Some("/retrieved"),
                format!("retrieved \"{r}\" is not YYYY-MM-DD"),
            ));
        }
    }
    if !source.url.starts_with("https://") && !source.url.starts_with("http://") {
        out.push(Diagnostic::error(
            "source.url",
            &entity,
            Some("/url"),
            format!("url \"{}\" is not an http(s) link", source.url),
        ));
    }
    out
}

/// `s` is a date with at least `min_parts` of year, month, day.
fn is_date(s: &str, min_parts: usize) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    let widths = [4, 2, 2];
    (min_parts..=3).contains(&parts.len())
        && parts
            .iter()
            .zip(widths)
            .all(|(p, w)| p.len() == w && p.bytes().all(|b| b.is_ascii_digit()))
        && parts.get(1).is_none_or(|m| ("01"..="12").contains(m))
        && parts.get(2).is_none_or(|d| ("01"..="31").contains(d))
}

/// The document kinds a dataset file can hold, for [`validate_location`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Part,
    Device,
    Package,
    Source,
    Manufacturer,
}

/// Where a document of `kind` with `id` belongs in the dataset tree:
/// its directory follows the id's first segment(s) and its file name
/// the id's last segment (MPN characters unsafe in file names may be
/// replaced, and a Source revision suffix `@revN` may appear as `-revN`).
pub fn validate_location(kind: DocKind, id: &str, rel_path: &str) -> Vec<Diagnostic> {
    let path = rel_path.replace('\\', "/");
    let segs: Vec<&str> = id.split('/').collect();
    let first = segs[0];
    let last = *segs.last().unwrap();
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".yaml");
    let safe = |s: &str| s.replace(['/', '\\', ' ', '(', ')', ',', '@'], "-");
    let (dir_ok, name_ok) = match kind {
        DocKind::Part => (path.starts_with(&format!("parts/{first}/")), true),
        DocKind::Device => (
            path.starts_with(&format!("devices/{first}/")),
            safe(stem) == safe(last),
        ),
        DocKind::Package => (
            path.starts_with(&format!("packages/{first}/")),
            safe(stem) == safe(last),
        ),
        DocKind::Source => (path.starts_with(&format!("sources/{first}/")), true),
        DocKind::Manufacturer => (path == format!("manufacturers/{id}.yaml"), true),
    };
    if dir_ok && name_ok {
        return vec![];
    }
    vec![Diagnostic::error(
        "id.location",
        format!("{kind:?}:{id}").to_lowercase(),
        Some("/id"),
        format!("id {id} doesn't match its file {path}"),
    )]
}

/// JSON Pointer escaping of one key.
pub(crate) fn pointer_escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_take_year_month_day_prefixes_only() {
        for ok in ["2026", "2026-04", "2026-04-10"] {
            assert!(is_date(ok, 1), "{ok}");
        }
        for bad in [
            "26-04",
            "2026-4",
            "2026-13",
            "2026-04-32",
            "April 2026",
            "2026/04",
        ] {
            assert!(!is_date(bad, 1), "{bad}");
        }
        assert!(!is_date("2026-04", 3), "retrieved needs a day");
    }

    #[test]
    fn ids_follow_their_directories() {
        let ok = |k, id, path| validate_location(k, id, path).is_empty();
        assert!(ok(
            DocKind::Part,
            "ti/LM317KCS",
            "parts/ti/lm317/LM317KCS.yaml"
        ));
        assert!(ok(
            DocKind::Device,
            "aos/AO3400A",
            "devices/aos/AO3400A.yaml"
        ));
        assert!(ok(
            DocKind::Package,
            "manufacturers/ti/KTT0003B",
            "packages/manufacturers/ti/KTT0003B.yaml"
        ));
        assert!(ok(
            DocKind::Package,
            "standards/QFN56-RP2040",
            "packages/standards/qfn/QFN56-RP2040.yaml"
        ));
        assert!(ok(
            DocKind::Source,
            "diodes/DS30086@rev32-2",
            "sources/diodes/DS30086-rev32-2.yaml"
        ));
        assert!(!ok(
            DocKind::Package,
            "bosch/LGA10",
            "packages/manufacturers/bosch/LGA10.yaml"
        ));
        assert!(!ok(DocKind::Device, "ti/A", "devices/ti/B.yaml"));
    }

    #[test]
    fn pins_need_pads_unless_trimmed_or_sharing_a_contact() {
        let package: Package = serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/T\nfamily: soic\nlead_count: 4\n\
             pitch: {nominal: 1.27, unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 3.9, unit: mm}\n\
             \x20 body_length: {nominal: 3.0, unit: mm}\n",
        )
        .unwrap();
        let device: Device = serde_yaml::from_str(
            "schema_version: '0.1'\nkind: device\nid: t/D\nmanufacturer: t\ncategory: other\npins:\n\
             \x20 '1': {name: A, type: passive}\n\
             \x20 '2': {name: B, type: passive}\n\
             \x20 '3': {name: C, type: passive}\n\
             \x20 '5': {name: A, type: passive, connected_to: '1'}\n\
             \x20 '6': {name: X, type: passive}\n",
        )
        .unwrap();
        let diags = check_pins_and_pads("device:t/D", &device, &package);
        let errors: Vec<_> = diags.iter().filter(|d| d.is_error()).collect();
        assert_eq!(errors.len(), 1, "{diags:?}");
        assert!(errors[0].message.contains("\"6\""));
        assert!(diags
            .iter()
            .any(|d| d.rule_id == "pad.no_pin" && d.message.contains("\"4\"")));
    }
}
