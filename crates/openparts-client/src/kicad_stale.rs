//! Stale footprint detection for a KiCad project.
//!
//! An OpenParts footprint carries its density-N Placement Model hash
//! as the footprint property `OpenParts placement`
//! (docs/spec/placement-model.md section 13). It is stale when that
//! value differs from the hash `GET .../placement` returns now. This
//! module scans a project's boards (`*.kicad_pcb`) and its
//! project-local footprint libraries (`fp-lib-table` rows under
//! `${KIPRJMOD}`) for that property, and compares.
//!
//! A footprint names no manufacturer, so the part is found by MPN: the
//! footprint's `MPN` field if it has one, else its name (OpenParts
//! names footprints after the MPN), else its `Value`.

use crate::sexpr::{self, Sexpr};
use crate::Client;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The footprint property holding the Placement Model hash.
pub const PLACEMENT_PROPERTY: &str = "OpenParts placement";

/// Field names taken as the MPN, as KiCad users spell them.
const MPN_FIELDS: &[&str] = &[
    "MPN",
    "Mpn",
    "mpn",
    "Manufacturer Part Number",
    "Manufacturer_Part_Number",
    "MFR Part Number",
    "Mfr Part Number",
    "MFN",
    "PartNumber",
    "Part Number",
];
const MANUFACTURER_FIELDS: &[&str] =
    &["Manufacturer", "MANUFACTURER", "manufacturer", "MFR", "Mfr"];

/// Where a scanned footprint lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    /// Placed on a board, with its reference designator.
    Board { file: PathBuf, reference: String },
    /// A `.kicad_mod` of a project library (`nickname`).
    Library { file: PathBuf, nickname: String },
}

/// One footprint and what it says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FootprintScan {
    pub origin: Origin,
    /// `lib:name` as the board names it, or `nickname:name`.
    pub lib_id: String,
    /// The name without the library nickname.
    pub name: String,
    pub value: Option<String>,
    pub mpn_field: Option<String>,
    pub manufacturer_field: Option<String>,
    /// The `OpenParts placement` property.
    pub hash: Option<String>,
}

impl FootprintScan {
    /// MPNs to look the part up by, most specific first.
    pub fn mpn_candidates(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for c in [
            self.mpn_field.as_deref(),
            Some(self.name.as_str()),
            self.value.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let c = c.trim();
            if !c.is_empty() && !c.contains("${") && !out.iter().any(|o| o == c) {
                out.push(c.to_string());
            }
        }
        out
    }

    pub fn label(&self) -> String {
        match &self.origin {
            Origin::Board { reference, .. } => format!("{reference} ({})", self.lib_id),
            Origin::Library { .. } => self.lib_id.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Syntax {
        path: PathBuf,
        #[source]
        source: sexpr::SexprError,
    },
    #[error("no .kicad_pcb and no project footprint library found in {0}")]
    NothingToScan(PathBuf),
}

fn read(path: &Path) -> Result<String, ScanError> {
    std::fs::read_to_string(path).map_err(|source| ScanError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn parse_file(path: &Path, text: &str) -> Result<Sexpr, ScanError> {
    sexpr::parse(text).map_err(|source| ScanError::Syntax {
        path: path.to_path_buf(),
        source,
    })
}

fn first_property(fp: &Sexpr, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|n| fp.property(n))
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "~")
        .map(str::to_string)
}

/// The value of a footprint's `fp_text <kind> "..."` (KiCad 6/7 files).
fn fp_text(fp: &Sexpr, kind: &str) -> Option<String> {
    fp.children("fp_text")
        .find(|t| t.arg(1) == Some(kind))
        .and_then(|t| t.arg(2))
        .map(str::to_string)
}

fn scan_footprint(fp: &Sexpr, origin: Origin, nickname: Option<&str>) -> FootprintScan {
    let raw = fp.arg(1).unwrap_or_default().to_string();
    let (lib, name) = match raw.split_once(':') {
        Some((lib, name)) => (Some(lib.to_string()), name.to_string()),
        None => (nickname.map(str::to_string), raw.clone()),
    };
    let lib_id = match &lib {
        Some(lib) => format!("{lib}:{name}"),
        None => name.clone(),
    };
    FootprintScan {
        origin,
        lib_id,
        name,
        value: fp
            .property("Value")
            .map(str::to_string)
            .or_else(|| fp_text(fp, "value")),
        mpn_field: first_property(fp, MPN_FIELDS),
        manufacturer_field: first_property(fp, MANUFACTURER_FIELDS),
        hash: fp.property(PLACEMENT_PROPERTY).map(str::to_string),
    }
}

/// Every footprint placed on a board (`.kicad_pcb` text).
pub fn scan_board(file: &Path, text: &str) -> Result<Vec<FootprintScan>, ScanError> {
    let pcb = parse_file(file, text)?;
    Ok(pcb
        .children("footprint")
        .chain(pcb.children("module"))
        .map(|fp| {
            let reference = fp
                .property("Reference")
                .map(str::to_string)
                .or_else(|| fp_text(fp, "reference"))
                .unwrap_or_default();
            scan_footprint(
                fp,
                Origin::Board {
                    file: file.to_path_buf(),
                    reference,
                },
                None,
            )
        })
        .collect())
}

/// One library footprint (`.kicad_mod` text).
pub fn scan_footprint_file(
    file: &Path,
    nickname: &str,
    text: &str,
) -> Result<FootprintScan, ScanError> {
    let fp = parse_file(file, text)?;
    Ok(scan_footprint(
        &fp,
        Origin::Library {
            file: file.to_path_buf(),
            nickname: nickname.to_string(),
        },
        Some(nickname),
    ))
}

/// The project-local footprint libraries of `fp-lib-table` (nickname,
/// directory): rows whose URI is under `${KIPRJMOD}`. Without a table,
/// `openparts.pretty` if it exists.
pub fn project_footprint_libraries(project_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(text) = std::fs::read_to_string(project_dir.join("fp-lib-table")) {
        if let Ok(table) = sexpr::parse(&text) {
            for lib in table.children("lib") {
                let field = |n: &str| lib.child(n).and_then(|c| c.arg(1)).map(str::to_string);
                let (Some(name), Some(uri)) = (field("name"), field("uri")) else {
                    continue;
                };
                let rel = uri
                    .strip_prefix("${KIPRJMOD}")
                    .or_else(|| uri.strip_prefix("$(KIPRJMOD)"));
                if let Some(rel) = rel {
                    let dir = project_dir.join(rel.trim_start_matches(['/', '\\']));
                    if dir.is_dir() {
                        out.push((name, dir));
                    }
                }
            }
            return out;
        }
    }
    let dir = project_dir.join("openparts.pretty");
    if dir.is_dir() {
        out.push(("openparts".to_string(), dir));
    }
    out
}

/// The boards of a project: `project` itself when it is a `.kicad_pcb`,
/// the `.kicad_pcb` next to a `.kicad_pro`, or every `.kicad_pcb` in a
/// directory.
fn boards(project: &Path) -> (PathBuf, Vec<PathBuf>) {
    if project.is_file() {
        let dir = project.parent().unwrap_or(Path::new(".")).to_path_buf();
        let pcb = project.with_extension("kicad_pcb");
        return (dir, if pcb.is_file() { vec![pcb] } else { vec![] });
    }
    let mut pcbs: Vec<PathBuf> = std::fs::read_dir(project)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "kicad_pcb")
                && !p
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("_autosave"))
        })
        .collect();
    pcbs.sort();
    (project.to_path_buf(), pcbs)
}

/// Scans a project (a directory, `.kicad_pro` or `.kicad_pcb`): its
/// boards, then its project libraries' footprints.
pub fn scan_project(project: &Path) -> Result<Vec<FootprintScan>, ScanError> {
    let (dir, pcbs) = boards(project);
    let libs = project_footprint_libraries(&dir);
    if pcbs.is_empty() && libs.is_empty() {
        return Err(ScanError::NothingToScan(project.to_path_buf()));
    }
    let mut out = Vec::new();
    for pcb in &pcbs {
        out.extend(scan_board(pcb, &read(pcb)?)?);
    }
    for (nickname, lib_dir) in &libs {
        let mut files: Vec<PathBuf> = std::fs::read_dir(lib_dir)
            .map_err(|source| ScanError::Io {
                path: lib_dir.clone(),
                source,
            })?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "kicad_mod"))
            .collect();
        files.sort();
        for f in files {
            out.push(scan_footprint_file(&f, nickname, &read(&f)?)?);
        }
    }
    Ok(out)
}

/// The two lookups a check needs; [`Client`] implements it over HTTP.
pub trait PlacementSource {
    /// The (manufacturer, MPN) of the part with this MPN, if one.
    fn resolve(
        &self,
        mpn: &str,
        manufacturer: Option<&str>,
    ) -> Result<Option<(String, String)>, String>;
    /// The part's current density-N placement hash.
    fn placement_hash(&self, manufacturer: &str, mpn: &str) -> Result<String, String>;
}

impl PlacementSource for Client {
    fn resolve(
        &self,
        mpn: &str,
        manufacturer: Option<&str>,
    ) -> Result<Option<(String, String)>, String> {
        self.resolve_mpn(mpn, manufacturer)
            .map(|o| o.map(|i| (i.manufacturer, i.mpn)))
            .map_err(|e| e.to_string())
    }
    fn placement_hash(&self, manufacturer: &str, mpn: &str) -> Result<String, String> {
        Client::placement_hash(self, manufacturer, mpn).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", content = "detail", rename_all = "snake_case")]
pub enum StaleStatus {
    Current,
    Stale,
    /// No `OpenParts placement` property: not an OpenParts footprint,
    /// or one rendered before footprints carried the hash.
    NoHash,
    /// No part with any of its MPN candidates.
    UnknownPart,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StaleEntry {
    pub footprint: FootprintScan,
    /// (manufacturer, MPN) when resolved.
    pub part: Option<(String, String)>,
    pub current_hash: Option<String>,
    #[serde(flatten)]
    pub status: StaleStatus,
}

impl StaleEntry {
    pub fn is_stale(&self) -> bool {
        self.status == StaleStatus::Stale
    }
}

/// Compares each footprint's hash with the current one. Footprints
/// without the property are reported `NoHash` without a lookup;
/// each MPN and part is looked up once.
pub fn check(source: &dyn PlacementSource, scans: &[FootprintScan]) -> Vec<StaleEntry> {
    type Resolved = Result<Option<(String, String)>, String>;
    let mut resolved: BTreeMap<(String, Option<String>), Resolved> = BTreeMap::new();
    let mut hashes: BTreeMap<(String, String), Result<String, String>> = BTreeMap::new();
    scans
        .iter()
        .map(|scan| {
            let entry = |part, current_hash, status| StaleEntry {
                footprint: scan.clone(),
                part,
                current_hash,
                status,
            };
            let Some(hash) = scan.hash.as_deref() else {
                return entry(None, None, StaleStatus::NoHash);
            };
            let mut last_err = None;
            let mut part = None;
            for mpn in scan.mpn_candidates() {
                let key = (mpn.clone(), scan.manufacturer_field.clone());
                let r = resolved
                    .entry(key)
                    .or_insert_with(|| source.resolve(&mpn, scan.manufacturer_field.as_deref()));
                match r {
                    Ok(Some(p)) => {
                        part = Some(p.clone());
                        break;
                    }
                    Ok(None) => {}
                    Err(e) => last_err = Some(e.clone()),
                }
            }
            let Some(part) = part else {
                return match last_err {
                    Some(e) => entry(None, None, StaleStatus::Error(e)),
                    None => entry(None, None, StaleStatus::UnknownPart),
                };
            };
            let current = hashes
                .entry(part.clone())
                .or_insert_with(|| source.placement_hash(&part.0, &part.1))
                .clone();
            match current {
                Ok(current) if current == hash => {
                    entry(Some(part), Some(current), StaleStatus::Current)
                }
                Ok(current) => entry(Some(part), Some(current), StaleStatus::Stale),
                Err(e) => entry(Some(part), None, StaleStatus::Error(e)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew")
  (footprint "openparts:RP2040" (layer "F.Cu")
    (property "Reference" "U1" (at 0 0 0) (layer "F.SilkS"))
    (property "Value" "RP2040" (at 0 0 0) (layer "F.Fab"))
    (property "OpenParts placement" "sha256:old" (at 0 0 0) (layer "F.Fab") (hide yes)))
  (footprint "board:61302011121-pico" (layer "F.Cu")
    (property "Reference" "J2")
    (property "Value" "61302011121")
    (property "OpenParts placement" "sha256:hdr"))
  (footprint "Resistor_SMD:R_0603" (layer "F.Cu")
    (property "Reference" "R9")
    (property "Value" "10k"))
  (footprint "openparts:CL10B104KB8NNNC" (layer "F.Cu")
    (property "Reference" "C1") (property "Value" "100n") (property "MPN" "CL10B104KB8NNNC")
    (property "Manufacturer" "Samsung")
    (property "OpenParts placement" "sha256:cap"))
)
"#;

    struct Fake {
        parts: Vec<(&'static str, &'static str, &'static str)>,
        lookups: RefCell<usize>,
    }

    impl PlacementSource for Fake {
        fn resolve(&self, mpn: &str, _: Option<&str>) -> Result<Option<(String, String)>, String> {
            *self.lookups.borrow_mut() += 1;
            Ok(self
                .parts
                .iter()
                .find(|p| p.1 == mpn)
                .map(|p| (p.0.to_string(), p.1.to_string())))
        }
        fn placement_hash(&self, m: &str, p: &str) -> Result<String, String> {
            self.parts
                .iter()
                .find(|x| x.0 == m && x.1 == p)
                .map(|x| x.2.to_string())
                .ok_or_else(|| "404".to_string())
        }
    }

    #[test]
    fn scans_board_footprints_with_their_fields_and_hash() {
        let scans = scan_board(Path::new("b.kicad_pcb"), BOARD).unwrap();
        assert_eq!(scans.len(), 4);
        assert_eq!(scans[0].lib_id, "openparts:RP2040");
        assert_eq!(scans[0].name, "RP2040");
        assert_eq!(scans[0].hash.as_deref(), Some("sha256:old"));
        assert_eq!(
            scans[0].origin,
            Origin::Board {
                file: "b.kicad_pcb".into(),
                reference: "U1".into()
            }
        );
        assert_eq!(scans[2].hash, None);
        assert_eq!(scans[3].mpn_field.as_deref(), Some("CL10B104KB8NNNC"));
        assert_eq!(scans[3].manufacturer_field.as_deref(), Some("Samsung"));
        assert_eq!(
            scans[1].mpn_candidates(),
            vec!["61302011121-pico".to_string(), "61302011121".to_string()]
        );
    }

    #[test]
    fn compares_hashes_and_looks_each_mpn_up_once() {
        let mut scans = scan_board(Path::new("b.kicad_pcb"), BOARD).unwrap();
        scans.push(scans[0].clone());
        let fake = Fake {
            parts: vec![
                ("raspberrypi", "RP2040", "sha256:new"),
                ("wurth", "61302011121", "sha256:hdr"),
                ("samsung", "CL10B104KB8NNNC", "sha256:cap"),
            ],
            lookups: RefCell::new(0),
        };
        let r = check(&fake, &scans);
        let status: Vec<&StaleStatus> = r.iter().map(|e| &e.status).collect();
        assert_eq!(
            status,
            vec![
                &StaleStatus::Stale,
                &StaleStatus::Current,
                &StaleStatus::NoHash,
                &StaleStatus::Current,
                &StaleStatus::Stale,
            ]
        );
        assert_eq!(r[0].current_hash.as_deref(), Some("sha256:new"));
        assert_eq!(r[1].part, Some(("wurth".into(), "61302011121".into())));
        // RP2040, 61302011121-pico, 61302011121, CL10B104KB8NNNC.
        assert_eq!(*fake.lookups.borrow(), 4);
    }

    #[test]
    fn unknown_parts_are_reported() {
        let scans = scan_board(Path::new("b.kicad_pcb"), BOARD).unwrap();
        let fake = Fake {
            parts: vec![],
            lookups: RefCell::new(0),
        };
        let r = check(&fake, &scans);
        assert_eq!(r[0].status, StaleStatus::UnknownPart);
    }

    #[test]
    fn scans_a_project_directory_board_and_libraries() {
        let dir = tempfile_dir("project");
        std::fs::write(dir.join("p.kicad_pcb"), BOARD).unwrap();
        std::fs::write(dir.join("_autosave-p.kicad_pcb"), BOARD).unwrap();
        std::fs::create_dir_all(dir.join("openparts.pretty")).unwrap();
        std::fs::create_dir_all(dir.join("other.pretty")).unwrap();
        std::fs::write(
            dir.join("openparts.pretty/RP2040.kicad_mod"),
            "(footprint \"RP2040\" (version 20221018) (generator openparts)\n  (layer \"F.Cu\")\n  (property \"OpenParts placement\" \"sha256:new\")\n  (fp_text value \"RP2040\" (at 0 5.15) (layer \"F.Fab\")))\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("other.pretty/X.kicad_mod"),
            "(footprint \"X\" (layer \"F.Cu\"))",
        )
        .unwrap();
        std::fs::write(
            dir.join("fp-lib-table"),
            "(fp_lib_table\n  (lib (name \"openparts\")(type \"KiCad\")(uri \"${KIPRJMOD}/openparts.pretty\")(options \"\")(descr \"\"))\n  (lib (name \"sys\")(type \"KiCad\")(uri \"/usr/share/x.pretty\")(options \"\")(descr \"\"))\n)\n",
        )
        .unwrap();
        let scans = scan_project(&dir).unwrap();
        assert_eq!(scans.len(), 5);
        let lib = &scans[4];
        assert_eq!(lib.lib_id, "openparts:RP2040");
        assert_eq!(lib.value.as_deref(), Some("RP2040"));
        assert_eq!(lib.hash.as_deref(), Some("sha256:new"));
        assert!(matches!(lib.origin, Origin::Library { .. }));
        // A .kicad_pro selects its board.
        std::fs::write(dir.join("p.kicad_pro"), "{}").unwrap();
        assert_eq!(scan_project(&dir.join("p.kicad_pro")).unwrap().len(), 5);
        assert!(scan_project(&dir.join("other.pretty")).is_err());
    }

    fn tempfile_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "openparts-client-stale-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
