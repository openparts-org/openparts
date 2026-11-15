//! `openparts kicad-check-stale <project>`: footprints whose
//! `OpenParts placement` hash is not the registry's current one
//! (openparts_client::kicad_stale).

use openparts_client::kicad_stale::{self, Origin, StaleStatus};
use openparts_client::Client;
use std::path::Path;

pub fn run(
    registry: &str,
    project: &Path,
    all: bool,
    json: bool,
    strict: bool,
) -> anyhow::Result<()> {
    let scans = kicad_stale::scan_project(project)?;
    let client = Client::new(registry);
    let entries = kicad_stale::check(&client, &scans);
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        for e in &entries {
            let status = match &e.status {
                StaleStatus::Current => "current",
                StaleStatus::Stale => "STALE",
                StaleStatus::NoHash => "no hash",
                StaleStatus::UnknownPart => "unknown",
                StaleStatus::Error(_) => "ERROR",
            };
            let show = all || matches!(e.status, StaleStatus::Stale | StaleStatus::Error(_));
            if !show {
                continue;
            }
            let place = match &e.footprint.origin {
                Origin::Board { reference, .. } => format!("board {reference}"),
                Origin::Library { .. } => "library".to_string(),
            };
            let part = e
                .part
                .as_ref()
                .map(|(m, p)| format!("{m}/{p}"))
                .unwrap_or_default();
            println!("{status:<8} {place:<12} {:<36} {part}", e.footprint.lib_id);
            if let StaleStatus::Error(msg) = &e.status {
                println!("         {msg}");
            }
        }
        let count = |f: fn(&StaleStatus) -> bool| entries.iter().filter(|e| f(&e.status)).count();
        let stale = count(|s| *s == StaleStatus::Stale);
        println!(
            "{} footprints: {} current, {stale} stale, {} unknown part, {} error, {} without a hash",
            entries.len(),
            count(|s| *s == StaleStatus::Current),
            count(|s| *s == StaleStatus::UnknownPart),
            count(|s| matches!(s, StaleStatus::Error(_))),
            count(|s| *s == StaleStatus::NoHash),
        );
        if stale > 0 {
            println!(
                "Update stale library footprints with `openparts kicad-install --project <dir> <manufacturer>/<mpn>`, \
                 then board footprints in KiCad: Tools > Update Footprints from Library."
            );
        }
    }
    if strict && entries.iter().any(|e| e.is_stale()) {
        anyhow::bail!("stale footprints");
    }
    Ok(())
}
