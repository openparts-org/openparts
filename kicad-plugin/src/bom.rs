//! BOM check state: extract the project's BOM, `POST /v1/bom/check`,
//! and the per-line actions (install a candidate, put its MPN on the
//! symbols).

use crate::jobs::{Job, Waker};
use crate::schematic::{self, SymbolInstance};
use openparts_client::api::{Alternative, BomLine, BomReport, PartBrief};
use openparts_client::Client;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where the BOM comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomSource {
    /// `kicad-cli`'s netlist when it is installed, else the schematic.
    Auto,
    /// The `.kicad_sch` files' symbol fields.
    Schematic,
}

/// What a check ran on.
#[derive(Debug, Clone)]
pub struct Extracted {
    /// The request body.
    pub body: String,
    pub symbols: Vec<SymbolInstance>,
    /// `schematic` or `kicad-cli netlist`, and the root file.
    pub how: String,
}

/// Reads the project's BOM: the symbols always come from the schematic
/// (the edit actions need their locations); the body sent is kicad-cli's
/// netlist when `source` allows and kicad-cli runs, else a BOM CSV of
/// those symbols.
pub fn extract(project: &Path, source: BomSource) -> Result<Extracted, String> {
    let root = schematic::root_schematic(project).map_err(|e| e.to_string())?;
    let symbols = schematic::read_symbols(&root).map_err(|e| e.to_string())?;
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if source == BomSource::Auto {
        if let Some(cli) = schematic::find_kicad_cli() {
            if let Ok(netlist) = schematic::export_netlist(&cli, &root) {
                return Ok(Extracted {
                    body: netlist,
                    symbols,
                    how: format!("{name} via kicad-cli"),
                });
            }
        }
    }
    Ok(Extracted {
        body: schematic::bom_csv(&symbols),
        symbols,
        how: name,
    })
}

/// A part a line could use: its match, or an alternative.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub part: PartBrief,
    /// `match` or the alternative's class (`drop_in`, ...).
    pub role: String,
}

/// `fetched`: alternatives looked up on demand
/// (`GET .../alternatives`) beyond those the check listed.
pub fn candidates(line: &BomLine, fetched: &[Alternative]) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = line
        .matches
        .iter()
        .map(|m| Candidate {
            part: m.part.clone(),
            role: "match".into(),
        })
        .collect();
    for a in line.alternatives.iter().chain(fetched) {
        if !out
            .iter()
            .any(|c| c.part.manufacturer == a.part.manufacturer && c.part.mpn == a.part.mpn)
        {
            out.push(Candidate {
                part: a.part.clone(),
                role: a.class.clone(),
            });
        }
    }
    out
}

/// How a line's symbols can take a new MPN.
#[derive(Debug, PartialEq)]
pub enum MpnPlan<'a> {
    /// Write it into their existing MPN fields.
    Write(Vec<&'a SymbolInstance>),
    /// Not reliably: copy it to the clipboard instead, and why.
    Clipboard(String),
}

pub fn mpn_plan<'a>(line: &BomLine, symbols: &'a [SymbolInstance]) -> MpnPlan<'a> {
    let mut found = Vec::new();
    for r in &line.references {
        let Some(s) = symbols.iter().find(|s| &s.reference == r) else {
            return MpnPlan::Clipboard(format!("{r} not found in the schematic"));
        };
        if let Some(why) = schematic::mpn_edit_blocker(s) {
            return MpnPlan::Clipboard(why);
        }
        found.push(s);
    }
    if found.is_empty() {
        return MpnPlan::Clipboard("no references on this line".into());
    }
    MpnPlan::Write(found)
}

/// One-word status of a line, for the table.
pub fn status_label(status: &str) -> &'static str {
    match status {
        "ok" => "OK",
        "review" => "Review",
        "warning" => "Warning",
        "not_found" => "Not found",
        _ => "?",
    }
}

#[derive(Default)]
pub struct BomState {
    pub report: Option<BomReport>,
    pub extracted: Option<Extracted>,
    pub selected: Option<usize>,
    pub status: String,
    /// Lines with status `ok` are hidden unless set.
    pub show_ok: bool,
    /// Alternatives fetched per line index.
    pub fetched: BTreeMap<usize, Vec<Alternative>>,
    job: Option<Job<(Extracted, BomReport)>>,
    alt_job: Option<Job<(usize, Vec<Alternative>)>>,
}

impl BomState {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn run(&mut self, project: PathBuf, registry: &str, source: BomSource, waker: &Waker) {
        let client = Client::new(registry);
        self.status = "Checking…".into();
        self.job = Some(Job::spawn(waker, move || {
            let extracted = extract(&project, source)?;
            let report = client
                .bom_check(&extracted.body)
                .map_err(|e| e.to_string())?;
            Ok((extracted, report))
        }));
    }

    pub fn tick(&mut self) {
        if let Some(r) = self.job.as_ref().and_then(Job::poll) {
            self.job = None;
            self.apply(r);
        }
        if let Some(r) = self.alt_job.as_ref().and_then(Job::poll) {
            self.alt_job = None;
            match r {
                Ok((line, alts)) => {
                    self.fetched.insert(line, alts);
                }
                Err(e) => self.status = format!("Alternatives failed: {e}"),
            }
        }
    }

    pub fn fetching_alternatives(&self) -> bool {
        self.alt_job.is_some()
    }

    /// Looks up alternatives for line `index`'s first match.
    pub fn fetch_alternatives(&mut self, index: usize, registry: &str, waker: &Waker) {
        let Some(m) = self
            .report
            .as_ref()
            .and_then(|r| r.lines.get(index)?.matches.first())
        else {
            return;
        };
        let (mfr, mpn) = (m.part.manufacturer.clone(), m.part.mpn.clone());
        let client = Client::new(registry);
        self.alt_job = Some(Job::spawn(waker, move || {
            client
                .alternatives(&mfr, &mpn)
                .map(|r| (index, r.alternatives))
                .map_err(|e| e.to_string())
        }));
    }

    fn apply(&mut self, r: Result<(Extracted, BomReport), String>) {
        match r {
            Ok((extracted, report)) => {
                let s = &report.summary;
                self.status = format!(
                    "{}: {} lines, {} ok, {} review, {} warning, {} not found",
                    extracted.how, s.lines, s.ok, s.review, s.warning, s.not_found
                );
                self.selected = None;
                self.fetched.clear();
                self.extracted = Some(extracted);
                self.report = Some(report);
            }
            Err(e) => self.status = format!("BOM check failed: {e}"),
        }
    }

    /// Indices of the lines shown.
    pub fn visible_lines(&self) -> Vec<usize> {
        self.report
            .as_ref()
            .map(|r| {
                r.lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| self.show_ok || l.status != "ok")
                    .map(|(i, _)| i)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn selected_line(&self) -> Option<&BomLine> {
        self.report.as_ref()?.lines.get(self.selected?)
    }

    #[cfg(test)]
    pub fn wait(&mut self) {
        if let Some(job) = self.job.take() {
            let r = job.wait();
            self.apply(r);
        }
    }
}

/// Puts `part`'s MPN on the line's symbols: written into their MPN
/// fields when that is reliable, else returned for the clipboard.
/// `Ok(message)`, with `Some(text)` to copy.
pub fn use_mpn(
    line: &BomLine,
    symbols: &[SymbolInstance],
    part: &PartBrief,
) -> Result<(String, Option<String>), String> {
    match mpn_plan(line, symbols) {
        MpnPlan::Write(targets) => {
            let manufacturer =
                (!part.manufacturer_name.is_empty()).then_some(part.manufacturer_name.as_str());
            let files = schematic::write_mpn(&targets, &part.mpn, manufacturer)
                .map_err(|e| e.to_string())?;
            let names: Vec<String> = files
                .iter()
                .filter_map(|f| f.file_name().map(|n| n.to_string_lossy().to_string()))
                .collect();
            Ok((
                format!(
                    "Set MPN {} on {} in {}. If the schematic is open, reload it (File › Revert).",
                    part.mpn,
                    line.references.join(","),
                    names.join(", ")
                ),
                None,
            ))
        }
        MpnPlan::Clipboard(why) => Ok((
            format!("Copied {} to the clipboard ({why}).", part.mpn),
            Some(part.mpn.clone()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::no_waker;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pico")
    }

    #[test]
    fn extracts_the_pico_bom_from_the_schematic() {
        let e = extract(&fixture(), BomSource::Schematic).unwrap();
        assert_eq!(e.how, "pico.kicad_sch");
        assert_eq!(e.symbols.len(), 33);
        assert!(e.body.starts_with("\"Reference\",\"Value\",\"Footprint\""));
        assert!(e
            .body
            .contains("\"U1\",\"RP2040\",\"openparts:RP2040\",\"1\""));
    }

    #[test]
    fn pico_lines_without_mpn_fields_fall_back_to_the_clipboard() {
        let e = extract(&fixture(), BomSource::Schematic).unwrap();
        let line = BomLine {
            references: vec!["U1".into()],
            ..Default::default()
        };
        let part = PartBrief {
            mpn: "RP2350A".into(),
            ..Default::default()
        };
        let (msg, copy) = use_mpn(&line, &e.symbols, &part).unwrap();
        assert_eq!(copy.as_deref(), Some("RP2350A"));
        assert!(msg.contains("U1 has no MPN field"), "{msg}");
    }

    #[test]
    fn writes_the_mpn_where_the_field_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("p.kicad_pro"), "{}").unwrap();
        std::fs::write(
            dir.path().join("p.kicad_sch"),
            "(kicad_sch (version 20231120)\n  (symbol (lib_id \"Device:C\") (in_bom yes) (uuid \"a\")\n    (property \"Reference\" \"C1\") (property \"Value\" \"100n\") (property \"MPN\" \"\")))\n",
        )
        .unwrap();
        let e = extract(dir.path(), BomSource::Schematic).unwrap();
        let line = BomLine {
            references: vec!["C1".into()],
            ..Default::default()
        };
        let part = PartBrief {
            mpn: "CL10B104KB8NNNC".into(),
            manufacturer_name: "Samsung".into(),
            ..Default::default()
        };
        let (msg, copy) = use_mpn(&line, &e.symbols, &part).unwrap();
        assert_eq!(copy, None);
        assert!(msg.starts_with("Set MPN CL10B104KB8NNNC on C1"));
        let again = extract(dir.path(), BomSource::Schematic).unwrap();
        assert_eq!(again.symbols[0].mpn, "CL10B104KB8NNNC");
    }

    #[test]
    fn candidates_list_matches_then_new_alternatives() {
        let brief = |m: &str| PartBrief {
            manufacturer: "x".into(),
            mpn: m.into(),
            ..Default::default()
        };
        let line: BomLine = BomLine {
            matches: vec![openparts_client::api::BomMatch {
                part: brief("A"),
                ..Default::default()
            }],
            alternatives: vec![
                openparts_client::api::Alternative {
                    part: brief("A"),
                    class: "drop_in".into(),
                    ..Default::default()
                },
                openparts_client::api::Alternative {
                    part: brief("B"),
                    class: "drop_in".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let c = candidates(&line, &[]);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].role, "match");
        assert_eq!(c[1].part.mpn, "B");
    }

    /// Against a real server: `OPENPARTS_TEST_REGISTRY=http://127.0.0.1:8092
    /// cargo test -p openparts-kicad-plugin -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_pico_bom_check() {
        let registry = std::env::var("OPENPARTS_TEST_REGISTRY").unwrap();
        let mut state = BomState::default();
        state.run(fixture(), &registry, BomSource::Schematic, &no_waker());
        state.wait();
        println!("{}", state.status);
        for l in &state.report.unwrap().lines {
            let m = l
                .matches
                .first()
                .map(|m| format!("{}/{}", m.part.manufacturer, m.part.mpn));
            let issues: Vec<&str> = l.issues.iter().map(|i| i.code.as_str()).collect();
            println!(
                "{:<9} {:<40} {:?} {:?} alts={}",
                l.status,
                l.references.join(","),
                m,
                issues,
                l.alternatives.len()
            );
        }
    }

    /// The whole run against a one-shot fake server: the request is
    /// the fixture's BOM CSV.
    #[test]
    fn run_posts_the_extracted_bom() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut req = Vec::new();
            let mut buf = [0u8; 65536];
            loop {
                let n = stream.read(&mut buf).unwrap();
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req).to_string();
                if let Some(h) = text.find("\r\n\r\n") {
                    let len: usize = text[..h]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap())
                        })
                        .unwrap_or(0);
                    if req.len() >= h + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let body = r#"{"format":"kicad_csv","summary":{"lines":2,"ok":1,"not_found":1},"lines":[{"references":["U1"],"status":"ok"},{"references":["J1"],"status":"not_found"}]}"#;
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
            String::from_utf8_lossy(&req).to_string()
        });
        let mut state = BomState::default();
        state.run(
            fixture(),
            &format!("http://{addr}"),
            BomSource::Schematic,
            &no_waker(),
        );
        state.wait();
        let req = server.join().unwrap();
        assert!(req.starts_with("POST /v1/bom/check"));
        assert!(req.contains("\"C1,C2,C3,C4,C5,C6,C7,C8,C9,C10,C11\",\"CL10B104KB8NNNC\""));
        assert_eq!(state.report.as_ref().unwrap().lines.len(), 2);
        assert!(state.status.contains("1 ok"), "{}", state.status);
        assert_eq!(state.visible_lines(), vec![1]);
    }
}
