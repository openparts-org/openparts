//! `openparts bom check <file>`: the server's BOM check
//! (`POST /v1/bom/check`), run locally on an openparts-data checkout
//! with the same library (openparts-bom).

use anyhow::Context;
use openparts_bom::matcher::{LineStatus, MatchKind, Severity};
use std::path::Path;

pub struct Options<'a> {
    pub file: &'a Path,
    pub data: &'a Path,
    pub json: bool,
    /// Exit with an error when a line is not found or has a warning.
    pub strict: bool,
}

pub fn check(o: Options) -> anyhow::Result<()> {
    let bytes = std::fs::read(o.file).with_context(|| format!("reading {}", o.file.display()))?;
    let text = openparts_bom::decode(&bytes)?;
    let bom = openparts_bom::parse_bom(&text)?;
    let ds = openparts_data::Dataset::load(o.data)
        .with_context(|| format!("loading {}", o.data.display()))?;
    let index = openparts_bom::BomIndex::build(&ds);
    let report = index.check(&ds, &bom);
    if o.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for l in &report.lines {
            let status = match l.status {
                LineStatus::Ok => "ok",
                LineStatus::Review => "review",
                LineStatus::Warning => "WARNING",
                LineStatus::NotFound => "NOT FOUND",
            };
            let kind = match l.kind {
                MatchKind::Exact => "exact",
                MatchKind::Normalized => "normalized",
                MatchKind::Variant => "variant",
                MatchKind::ValueFootprint => "value+footprint",
                MatchKind::None => "-",
            };
            let what = if l.mpn.is_empty() { &l.value } else { &l.mpn };
            let refs = l.references.join(",");
            let matched = l
                .matches
                .first()
                .map(|m| format!("{}/{}", m.part.manufacturer, m.part.mpn))
                .unwrap_or_default();
            let more = l.matches.len().saturating_sub(1);
            let more = if more > 0 {
                format!(" (+{more})")
            } else {
                String::new()
            };
            println!(
                "{status:<9} {refs:<16} x{:<3} {what:<22} {kind:<15} {matched}{more}",
                l.quantity
            );
            for i in &l.issues {
                let mark = if i.severity == Severity::Warning {
                    "!"
                } else {
                    "-"
                };
                let code = serde_json::to_value(i.code)?;
                let code = code.as_str().unwrap_or_default();
                match &i.detail {
                    Some(d) => println!("          {mark} {code}: {d}"),
                    None => println!("          {mark} {code}"),
                }
            }
            for a in &l.alternatives {
                let class = serde_json::to_value(a.class)?;
                println!(
                    "          > alternative ({}): {}/{}",
                    class.as_str().unwrap_or_default(),
                    a.part.manufacturer,
                    a.part.mpn
                );
            }
        }
        let s = &report.summary;
        println!(
            "\n{} lines, {} components: {} ok, {} review, {} warning, {} not found",
            s.lines, s.components, s.ok, s.review, s.warning, s.not_found
        );
    }
    let s = &report.summary;
    if o.strict && (s.warning > 0 || s.not_found > 0) {
        anyhow::bail!("{} warning, {} not found", s.warning, s.not_found);
    }
    Ok(())
}
