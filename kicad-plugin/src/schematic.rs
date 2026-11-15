//! Reading a KiCad project's schematic for the BOM check: every placed
//! symbol with its fields, across the sheet hierarchy, grouped into BOM
//! lines (a KiCad-style BOM CSV the server's `POST /v1/bom/check`
//! reads), and writing an MPN field back.
//!
//! Symbols come straight from the `.kicad_sch` files, so no KiCad
//! install is needed. When `kicad-cli` is available the netlist it
//! exports is the alternative input ([`export_netlist`]).

use openparts_client::sexpr::{self, Sexpr};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Field names read as the MPN (first present wins), the usual KiCad
/// spellings.
pub const MPN_FIELDS: &[&str] = &[
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
pub const MANUFACTURER_FIELDS: &[&str] =
    &["Manufacturer", "MANUFACTURER", "manufacturer", "MFR", "Mfr"];

/// A field value's location: the quoted string's byte span in `file`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSpan {
    pub name: String,
    pub span: std::ops::Range<usize>,
}

/// One placed symbol (one entry per reference: a multi-unit symbol's
/// units are merged; a symbol on a sheet used twice yields two).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolInstance {
    pub reference: String,
    pub value: String,
    pub footprint: String,
    pub mpn: String,
    pub manufacturer: String,
    pub dnp: bool,
    /// The sheet file the symbol is in.
    pub file: PathBuf,
    pub uuid: String,
    /// The MPN field of each unit (a multi-unit symbol stores its
    /// fields once per unit).
    pub mpn_fields: Vec<FieldSpan>,
    pub manufacturer_fields: Vec<FieldSpan>,
    /// Units without an MPN field.
    pub units_without_mpn: usize,
    /// How many times `file` is placed in the hierarchy: a field edit
    /// in a sheet used more than once changes every copy.
    pub sheet_uses: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum SchematicError {
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
    #[error("no root schematic found in {0}")]
    NoSchematic(PathBuf),
    #[error("{0}")]
    Edit(String),
}

/// The project's root schematic: `<name>.kicad_sch` for the directory's
/// `.kicad_pro` (or the given `.kicad_pro`/`.kicad_sch`), else the only
/// `.kicad_sch` there.
pub fn root_schematic(project: &Path) -> Result<PathBuf, SchematicError> {
    if project.is_file() {
        let sch = project.with_extension("kicad_sch");
        return if sch.is_file() {
            Ok(sch)
        } else {
            Err(SchematicError::NoSchematic(project.to_path_buf()))
        };
    }
    let files: Vec<PathBuf> = std::fs::read_dir(project)
        .map_err(|source| SchematicError::Io {
            path: project.to_path_buf(),
            source,
        })?
        .flatten()
        .map(|e| e.path())
        .collect();
    let with_ext = |ext: &str| -> Vec<&PathBuf> {
        let mut v: Vec<&PathBuf> = files
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == ext))
            .filter(|p| {
                !p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("_autosave"))
            })
            .collect();
        v.sort();
        v
    };
    for pro in with_ext("kicad_pro") {
        let sch = pro.with_extension("kicad_sch");
        if sch.is_file() {
            return Ok(sch);
        }
    }
    match with_ext("kicad_sch").as_slice() {
        [only] => Ok((*only).clone()),
        _ => Err(SchematicError::NoSchematic(project.to_path_buf())),
    }
}

fn first_field(symbol: &Sexpr, names: &[&str]) -> Option<(String, FieldSpan)> {
    for name in names {
        if let Some((_, v)) = symbol.properties().find(|(k, _)| k == name) {
            let text = v.atom().unwrap_or_default().trim().to_string();
            return Some((
                if text == "~" { String::new() } else { text },
                FieldSpan {
                    name: name.to_string(),
                    span: v.span(),
                },
            ));
        }
    }
    None
}

fn yes(node: Option<&Sexpr>) -> Option<bool> {
    node.and_then(|n| n.arg(1)).map(|v| v == "yes")
}

/// References a symbol has per instance: `(instances (project ..
/// (path ".." (reference "R1"))))`, KiCad 7+; a KiCad 6 root's
/// `symbol_instances` (paths ending in the symbol's uuid); else its
/// `Reference` field.
fn references(symbol: &Sexpr, legacy: &BTreeMap<String, Vec<String>>, uuid: &str) -> Vec<String> {
    let mut refs: Vec<String> = Vec::new();
    for inst in symbol.children("instances") {
        for project in inst.children("project") {
            for path in project.children("path") {
                if let Some(r) = path.child("reference").and_then(|r| r.arg(1)) {
                    refs.push(r.to_string());
                }
            }
        }
    }
    if refs.is_empty() {
        if let Some(r) = legacy.get(uuid) {
            refs.extend(r.iter().cloned());
        }
    }
    if refs.is_empty() {
        if let Some(r) = symbol.property("Reference") {
            refs.push(r.to_string());
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

/// Every BOM-relevant symbol of the hierarchy under `root`: not power
/// symbols (`#` references), not `in_bom no`.
pub fn read_symbols(root: &Path) -> Result<Vec<SymbolInstance>, SchematicError> {
    // Sheet files in hierarchy order, with their use counts.
    let mut uses: BTreeMap<PathBuf, usize> = BTreeMap::new();
    let mut order: Vec<PathBuf> = Vec::new();
    let mut texts: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    let mut legacy: BTreeMap<String, Vec<String>> = BTreeMap::new();
    while let Some(file) = stack.pop() {
        *uses.entry(file.clone()).or_default() += 1;
        if texts.contains_key(&file) {
            continue;
        }
        let text = std::fs::read_to_string(&file).map_err(|source| SchematicError::Io {
            path: file.clone(),
            source,
        })?;
        let tree = sexpr::parse(&text).map_err(|source| SchematicError::Syntax {
            path: file.clone(),
            source,
        })?;
        let dir = file.parent().unwrap_or(Path::new("."));
        let mut subs = Vec::new();
        for sheet in tree.children("sheet") {
            let name = sheet
                .property("Sheetfile")
                .or_else(|| sheet.property("Sheet file"));
            if let Some(name) = name {
                subs.push(dir.join(name));
            }
        }
        for si in tree.children("symbol_instances") {
            for path in si.children("path") {
                let (Some(p), Some(r)) =
                    (path.arg(1), path.child("reference").and_then(|r| r.arg(1)))
                else {
                    continue;
                };
                let uuid = p.rsplit('/').next().unwrap_or_default().to_string();
                legacy.entry(uuid).or_default().push(r.to_string());
            }
        }
        order.push(file.clone());
        texts.insert(file, text);
        // Depth-first, in file order.
        stack.extend(subs.into_iter().rev());
    }

    let mut out: Vec<SymbolInstance> = Vec::new();
    for file in &order {
        let text = &texts[file];
        let tree = sexpr::parse(text).map_err(|source| SchematicError::Syntax {
            path: file.clone(),
            source,
        })?;
        for symbol in tree.children("symbol") {
            if symbol.child("lib_id").is_none() {
                continue;
            }
            if yes(symbol.child("in_bom")) == Some(false) {
                continue;
            }
            let uuid = symbol
                .child("uuid")
                .and_then(|u| u.arg(1))
                .unwrap_or_default()
                .to_string();
            let mpn = first_field(symbol, MPN_FIELDS);
            let manufacturer = first_field(symbol, MANUFACTURER_FIELDS);
            let field = |n: &str| {
                symbol
                    .property(n)
                    .map(str::trim)
                    .filter(|v| *v != "~")
                    .unwrap_or_default()
                    .to_string()
            };
            for reference in references(symbol, &legacy, &uuid) {
                if reference.starts_with('#') {
                    continue;
                }
                // Another unit of a symbol already read.
                if let Some(s) = out.iter_mut().find(|s| s.reference == reference) {
                    if s.file == *file {
                        s.mpn_fields.extend(mpn.as_ref().map(|m| m.1.clone()));
                        s.manufacturer_fields
                            .extend(manufacturer.as_ref().map(|m| m.1.clone()));
                        s.units_without_mpn += usize::from(mpn.is_none());
                    }
                    continue;
                }
                out.push(SymbolInstance {
                    reference,
                    value: field("Value"),
                    footprint: field("Footprint"),
                    mpn: mpn.as_ref().map(|m| m.0.clone()).unwrap_or_default(),
                    manufacturer: manufacturer
                        .as_ref()
                        .map(|m| m.0.clone())
                        .unwrap_or_default(),
                    dnp: yes(symbol.child("dnp")).unwrap_or(false),
                    file: file.clone(),
                    uuid: uuid.clone(),
                    mpn_fields: mpn.as_ref().map(|m| m.1.clone()).into_iter().collect(),
                    manufacturer_fields: manufacturer
                        .as_ref()
                        .map(|m| m.1.clone())
                        .into_iter()
                        .collect(),
                    units_without_mpn: usize::from(mpn.is_none()),
                    sheet_uses: uses[file],
                });
            }
        }
    }
    out.sort_by_key(|s| natural_key(&s.reference));
    Ok(out)
}

/// `R10` after `R9`.
fn natural_key(reference: &str) -> (String, u64, String) {
    let split = reference
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(reference.len());
    let (prefix, rest) = reference.split_at(split);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let tail = rest[digits.len()..].to_string();
    (prefix.to_string(), digits.parse().unwrap_or(0), tail)
}

fn csv_cell(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Groups symbols into BOM lines (same value, footprint, MPN,
/// manufacturer and DNP) as a KiCad-style BOM CSV.
pub fn bom_csv(symbols: &[SymbolInstance]) -> String {
    type Key<'a> = (&'a str, &'a str, &'a str, &'a str, bool);
    let mut groups: Vec<(Key, Vec<&str>)> = Vec::new();
    for s in symbols {
        let key = (
            s.value.as_str(),
            s.footprint.as_str(),
            s.mpn.as_str(),
            s.manufacturer.as_str(),
            s.dnp,
        );
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, refs)) => refs.push(&s.reference),
            None => groups.push((key, vec![&s.reference])),
        }
    }
    let mut out = String::from(
        "\"Reference\",\"Value\",\"Footprint\",\"Qty\",\"MPN\",\"Manufacturer\",\"DNP\"\n",
    );
    for ((value, footprint, mpn, manufacturer, dnp), refs) in groups {
        out.push_str(
            &[
                csv_cell(&refs.join(",")),
                csv_cell(value),
                csv_cell(footprint),
                csv_cell(&refs.len().to_string()),
                csv_cell(mpn),
                csv_cell(manufacturer),
                csv_cell(if dnp { "DNP" } else { "" }),
            ]
            .join(","),
        );
        out.push('\n');
    }
    out
}

/// `kicad-cli` on `PATH` (or `KICAD_CLI`), if any.
pub fn find_kicad_cli() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("KICAD_CLI").map(PathBuf::from) {
        if p.is_file() {
            return Some(p);
        }
    }
    let exe = if cfg!(windows) {
        "kicad-cli.exe"
    } else {
        "kicad-cli"
    };
    let mut candidates: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join(exe)).collect())
        .unwrap_or_default();
    if cfg!(windows) {
        for v in ["10.0", "9.0", "8.0"] {
            candidates.push(PathBuf::from(format!(
                "C:\\Program Files\\KiCad\\{v}\\bin\\kicad-cli.exe"
            )));
        }
    } else if cfg!(target_os = "macos") {
        candidates.push("/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli".into());
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// The netlist `kicad-cli sch export netlist` writes for `root`
/// (KiCad s-expression format, which `POST /v1/bom/check` reads).
pub fn export_netlist(kicad_cli: &Path, root: &Path) -> Result<String, String> {
    let out =
        std::env::temp_dir().join(format!("openparts-kicad-plugin-{}.net", std::process::id()));
    let status = std::process::Command::new(kicad_cli)
        .args(["sch", "export", "netlist", "--format", "kicadsexpr", "-o"])
        .arg(&out)
        .arg(root)
        .output()
        .map_err(|e| format!("running {}: {e}", kicad_cli.display()))?;
    if !status.status.success() {
        return Err(format!(
            "kicad-cli failed: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        ));
    }
    let text = std::fs::read_to_string(&out).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&out);
    text
}

/// One field rewrite: (value span, value read, new value).
type Edit = (std::ops::Range<usize>, String, String);

/// Why an MPN can't be written into the schematic for `symbol`, if so.
pub fn mpn_edit_blocker(symbol: &SymbolInstance) -> Option<String> {
    if symbol.units_without_mpn > 0 {
        return Some(format!("{} has no MPN field", symbol.reference));
    }
    if symbol.sheet_uses > 1 {
        return Some(format!(
            "{} is on a sheet used {} times",
            symbol.reference, symbol.sheet_uses
        ));
    }
    None
}

/// Writes `mpn` (and `manufacturer`, where the symbol has that field)
/// into each symbol's existing fields, in place. Every symbol must pass
/// [`mpn_edit_blocker`]; the files must still hold what was read (a
/// field's span is checked against its old value first), else nothing
/// is written. Returns the files changed.
pub fn write_mpn(
    symbols: &[&SymbolInstance],
    mpn: &str,
    manufacturer: Option<&str>,
) -> Result<Vec<PathBuf>, SchematicError> {
    if let Some(why) = symbols.iter().find_map(|s| mpn_edit_blocker(s)) {
        return Err(SchematicError::Edit(why));
    }
    let mut edits: BTreeMap<&Path, Vec<Edit>> = BTreeMap::new();
    for s in symbols {
        let list = edits.entry(&s.file).or_default();
        for f in &s.mpn_fields {
            list.push((f.span.clone(), s.mpn.clone(), mpn.to_string()));
        }
        if let Some(m) = manufacturer {
            for f in &s.manufacturer_fields {
                list.push((f.span.clone(), s.manufacturer.clone(), m.to_string()));
            }
        }
    }
    let mut new_texts = Vec::new();
    for (file, mut list) in edits {
        let mut text = std::fs::read_to_string(file).map_err(|source| SchematicError::Io {
            path: file.to_path_buf(),
            source,
        })?;
        list.sort_by_key(|(span, _, _)| std::cmp::Reverse(span.start));
        list.dedup_by_key(|(span, _, _)| span.start);
        for (span, old, new) in list {
            let current = text
                .get(span.clone())
                .and_then(|q| sexpr::parse(&format!("(x {q})")).ok())
                .and_then(|e| e.arg(1).map(|s| s.trim().to_string()));
            let old_matches = current
                .as_deref()
                .is_some_and(|c| c == old || (c == "~" && old.is_empty()));
            if !old_matches {
                return Err(SchematicError::Edit(format!(
                    "{} changed since it was read; run the check again",
                    file.display()
                )));
            }
            text.replace_range(span, &sexpr::quote(&new));
        }
        // The result must still parse.
        sexpr::parse(&text).map_err(|source| SchematicError::Syntax {
            path: file.to_path_buf(),
            source,
        })?;
        new_texts.push((file.to_path_buf(), text));
    }
    let mut changed = Vec::new();
    for (file, text) in new_texts {
        std::fs::write(&file, text).map_err(|source| SchematicError::Io {
            path: file.clone(),
            source,
        })?;
        changed.push(file);
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pico")
    }

    #[test]
    fn reads_the_pico_schematic() {
        let root = root_schematic(&fixture()).unwrap();
        assert_eq!(root.file_name().unwrap(), "pico.kicad_sch");
        let symbols = read_symbols(&root).unwrap();
        // 35 symbols, less the two #FLG power flags.
        assert_eq!(symbols.len(), 33);
        assert_eq!(symbols[0].reference, "C1");
        assert_eq!(symbols[1].reference, "C2");
        assert_eq!(symbols[9].reference, "C10");
        let u1 = symbols.iter().find(|s| s.reference == "U1").unwrap();
        assert_eq!(u1.value, "RP2040");
        assert_eq!(u1.footprint, "openparts:RP2040");
        assert_eq!(u1.mpn, "");
        assert!(u1.mpn_fields.is_empty());
        assert_eq!(u1.sheet_uses, 1);
        assert!(mpn_edit_blocker(u1).unwrap().contains("no MPN field"));
    }

    #[test]
    fn groups_the_pico_bom() {
        let symbols = read_symbols(&root_schematic(&fixture()).unwrap()).unwrap();
        let csv = bom_csv(&symbols);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines[0],
            "\"Reference\",\"Value\",\"Footprint\",\"Qty\",\"MPN\",\"Manufacturer\",\"DNP\""
        );
        assert!(lines.contains(
            &"\"C1,C2,C3,C4,C5,C6,C7,C8,C9,C10,C11\",\"CL10B104KB8NNNC\",\"openparts:CL10B104KB8NNNC\",\"11\",\"\",\"\",\"\""
        ));
        assert!(lines.contains(
            &"\"J2,J3\",\"61302011121\",\"board:61302011121-pico\",\"2\",\"\",\"\",\"\""
        ));
        let qty: usize = lines[1..]
            .iter()
            .map(|l| l.split("\",\"").nth(3).unwrap().parse::<usize>().unwrap())
            .sum();
        assert_eq!(qty, 33);
    }

    const ROOT: &str = r##"(kicad_sch (version 20231120) (generator "eeschema")
  (uuid "root-uuid")
  (symbol (lib_id "power:GND") (in_bom yes) (uuid "p1")
    (property "Reference" "#PWR01") (property "Value" "GND"))
  (symbol (lib_id "Device:R") (in_bom yes) (dnp yes) (uuid "r1")
    (property "Reference" "R1") (property "Value" "10k")
    (property "Footprint" "Resistor_SMD:R_0603_1608Metric")
    (property "MPN" "RC0603FR-0710KL") (property "Manufacturer" "Yageo")
    (instances (project "p" (path "/root-uuid" (reference "R1") (unit 1)))))
  (symbol (lib_id "Device:R") (in_bom no) (uuid "r2")
    (property "Reference" "R2") (property "Value" "0"))
  (symbol (lib_id "MCU:X") (in_bom yes) (unit 1) (uuid "u1a")
    (property "Reference" "U1") (property "Value" "X") (property "MPN" "OLD-1")
    (instances (project "p" (path "/root-uuid" (reference "U1") (unit 1)))))
  (symbol (lib_id "MCU:X") (in_bom yes) (unit 2) (uuid "u1b")
    (property "Reference" "U1") (property "Value" "X") (property "MPN" "OLD-1")
    (instances (project "p" (path "/root-uuid" (reference "U1") (unit 2)))))
  (sheet (at 0 0) (size 10 10) (uuid "s1")
    (property "Sheetname" "A") (property "Sheetfile" "sub.kicad_sch"))
  (sheet (at 20 0) (size 10 10) (uuid "s2")
    (property "Sheetname" "B") (property "Sheetfile" "sub.kicad_sch"))
)
"##;
    const SUB: &str = r#"(kicad_sch (version 20231120) (generator "eeschema")
  (uuid "sub-uuid")
  (symbol (lib_id "Device:C") (in_bom yes) (uuid "c1")
    (property "Reference" "C1") (property "Value" "100n") (property "MPN" "~")
    (instances (project "p"
      (path "/root-uuid/s1" (reference "C1") (unit 1))
      (path "/root-uuid/s2" (reference "C2") (unit 1)))))
)
"#;

    fn hier_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("p.kicad_pro"), "{}").unwrap();
        std::fs::write(dir.path().join("p.kicad_sch"), ROOT).unwrap();
        std::fs::write(dir.path().join("sub.kicad_sch"), SUB).unwrap();
        dir
    }

    #[test]
    fn walks_the_hierarchy_and_merges_units() {
        let dir = hier_project();
        let symbols = read_symbols(&root_schematic(dir.path()).unwrap()).unwrap();
        let refs: Vec<&str> = symbols.iter().map(|s| s.reference.as_str()).collect();
        assert_eq!(refs, vec!["C1", "C2", "R1", "U1"]);
        let r1 = &symbols[2];
        assert!(r1.dnp);
        assert_eq!(r1.mpn, "RC0603FR-0710KL");
        assert_eq!(r1.manufacturer, "Yageo");
        assert_eq!(symbols[0].mpn, "");
        assert_eq!(symbols[0].sheet_uses, 2);
        assert!(mpn_edit_blocker(&symbols[0])
            .unwrap()
            .contains("used 2 times"));
        let csv = bom_csv(&symbols);
        assert!(csv.contains("\"C1,C2\",\"100n\",\"\",\"2\",\"\",\"\",\"\""));
        assert!(csv.contains("\"R1\",\"10k\",\"Resistor_SMD:R_0603_1608Metric\",\"1\",\"RC0603FR-0710KL\",\"Yageo\",\"DNP\""));
    }

    #[test]
    fn writes_mpn_and_manufacturer_into_existing_fields() {
        let dir = hier_project();
        let symbols = read_symbols(&root_schematic(dir.path()).unwrap()).unwrap();
        let r1 = symbols.iter().find(|s| s.reference == "R1").unwrap();
        let changed = write_mpn(&[r1], "RC0603JR-0710KL", Some("YAGEO \"x\"")).unwrap();
        assert_eq!(changed, vec![dir.path().join("p.kicad_sch")]);
        let again = read_symbols(&root_schematic(dir.path()).unwrap()).unwrap();
        let r1b = again.iter().find(|s| s.reference == "R1").unwrap();
        assert_eq!(r1b.mpn, "RC0603JR-0710KL");
        assert_eq!(r1b.manufacturer, "YAGEO \"x\"");
        // Nothing else moved.
        let text = std::fs::read_to_string(dir.path().join("p.kicad_sch")).unwrap();
        assert_eq!(
            text.replace("\"RC0603JR-0710KL\"", "\"RC0603FR-0710KL\"")
                .replace("\"YAGEO \\\"x\\\"\"", "\"Yageo\""),
            ROOT
        );
        // A stale read is refused (r1 still holds the old value).
        assert!(write_mpn(&[r1], "Z", None).is_err());
        // A shared sheet is refused.
        let c1 = again.iter().find(|s| s.reference == "C1").unwrap();
        assert!(write_mpn(&[c1], "Z", None).is_err());
    }

    #[test]
    fn writes_every_unit_of_a_multi_unit_symbol() {
        let dir = hier_project();
        let symbols = read_symbols(&root_schematic(dir.path()).unwrap()).unwrap();
        let u1 = symbols.iter().find(|s| s.reference == "U1").unwrap();
        assert_eq!(u1.mpn_fields.len(), 2);
        write_mpn(&[u1], "NEW-1", None).unwrap();
        let text = std::fs::read_to_string(dir.path().join("p.kicad_sch")).unwrap();
        assert_eq!(text.matches("\"NEW-1\"").count(), 2);
        assert!(!text.contains("OLD-1"));
    }
}
