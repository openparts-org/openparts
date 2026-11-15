//! Stale footprint state: scan the project, compare placement hashes
//! (openparts_client::kicad_stale), and which entries the
//! library update (kicad-install) can fix.

use crate::jobs::{Job, Waker};
use openparts_client::kicad_stale::{self, Origin, StaleEntry, StaleStatus};
use openparts_client::Client;
use std::path::PathBuf;

/// The library kicad-install writes.
pub const OPENPARTS_LIBRARY: &str = "openparts";

/// The part to reinstall to refresh this entry's library footprint, or
/// why that wouldn't help.
pub fn update_target(entry: &StaleEntry) -> Result<(String, String), String> {
    let Some(part) = entry.part.clone() else {
        return Err("part not found".into());
    };
    let lib = entry
        .footprint
        .lib_id
        .split_once(':')
        .map(|(l, _)| l)
        .unwrap_or_default();
    if lib != OPENPARTS_LIBRARY {
        return Err(format!(
            "from library \"{lib}\", which kicad-install doesn't write"
        ));
    }
    Ok(part)
}

pub fn status_label(status: &StaleStatus) -> &'static str {
    match status {
        StaleStatus::Current => "Current",
        StaleStatus::Stale => "Stale",
        StaleStatus::NoHash => "No hash",
        StaleStatus::UnknownPart => "Unknown part",
        StaleStatus::Error(_) => "Error",
    }
}

#[derive(Default)]
pub struct StaleState {
    pub entries: Vec<StaleEntry>,
    pub status: String,
    /// Show current and hash-less footprints too.
    pub show_all: bool,
    pub scanned: bool,
    job: Option<Job<Vec<StaleEntry>>>,
}

impl StaleState {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn run(&mut self, project: PathBuf, registry: &str, waker: &Waker) {
        let client = Client::new(registry);
        self.status = "Scanning…".into();
        self.job = Some(Job::spawn(waker, move || {
            let scans = kicad_stale::scan_project(&project).map_err(|e| e.to_string())?;
            Ok(kicad_stale::check(&client, &scans))
        }));
    }

    pub fn tick(&mut self) {
        if let Some(r) = self.job.as_ref().and_then(Job::poll) {
            self.job = None;
            self.apply(r);
        }
    }

    fn apply(&mut self, r: Result<Vec<StaleEntry>, String>) {
        match r {
            Ok(entries) => {
                self.scanned = true;
                let n = |s: StaleStatus| entries.iter().filter(|e| e.status == s).count();
                let errors = entries
                    .iter()
                    .filter(|e| matches!(e.status, StaleStatus::Error(_)))
                    .count();
                self.status = format!(
                    "{} footprints: {} current, {} stale, {} unknown, {} errors",
                    entries.len(),
                    n(StaleStatus::Current),
                    n(StaleStatus::Stale),
                    n(StaleStatus::UnknownPart),
                    errors
                );
                self.entries = entries;
            }
            Err(e) => self.status = format!("Scan failed: {e}"),
        }
    }

    pub fn visible(&self) -> Vec<&StaleEntry> {
        self.entries
            .iter()
            .filter(|e| {
                self.show_all
                    || matches!(
                        e.status,
                        StaleStatus::Stale | StaleStatus::Error(_) | StaleStatus::UnknownPart
                    )
            })
            .collect()
    }

    /// Distinct parts whose openparts-library footprint can be
    /// refreshed (stale entries only).
    pub fn updatable(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .entries
            .iter()
            .filter(|e| e.is_stale())
            .filter_map(|e| update_target(e).ok())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    pub fn stale_on_board(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.is_stale() && matches!(e.footprint.origin, Origin::Board { .. }))
    }

    #[cfg(test)]
    pub fn wait(&mut self) {
        if let Some(job) = self.job.take() {
            let r = job.wait();
            self.apply(r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_client::kicad_stale::{FootprintScan, PlacementSource};
    use std::path::Path;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pico")
    }

    /// Answers with the hash each fixture footprint carries, except
    /// RC0603FR-0727RL's, which "changed".
    struct Registry(Vec<FootprintScan>);

    impl PlacementSource for Registry {
        fn resolve(&self, mpn: &str, _: Option<&str>) -> Result<Option<(String, String)>, String> {
            Ok(self
                .0
                .iter()
                .any(|s| s.name == mpn)
                .then(|| ("mfr".to_string(), mpn.to_string())))
        }
        fn placement_hash(&self, _: &str, mpn: &str) -> Result<String, String> {
            if mpn == "RC0603FR-0727RL" {
                return Ok("sha256:new".into());
            }
            Ok(self
                .0
                .iter()
                .find(|s| s.name == mpn && s.hash.is_some())
                .and_then(|s| s.hash.clone())
                .unwrap_or_default())
        }
    }

    #[test]
    fn scans_the_pico_fixture() {
        let scans = kicad_stale::scan_project(&fixture()).unwrap();
        // 8 board footprints, 3 openparts.pretty, 1 board.pretty.
        assert_eq!(scans.len(), 12);
        let hashed = scans.iter().filter(|s| s.hash.is_some()).count();
        // openparts.pretty/61302021121 predates the hash.
        assert_eq!(hashed, 11);
        let u = scans
            .iter()
            .find(|s| s.lib_id == "openparts:RP2040")
            .unwrap();
        assert!(matches!(u.origin, Origin::Library { .. }));
        assert_eq!(
            u.hash.as_deref(),
            Some("sha256:6d6d901a8e5a8b6a97850cd96a587d345c0712c84b313714b0f9df1c5d8cd6e1")
        );
    }

    #[test]
    fn finds_the_stale_ones_and_what_to_update() {
        let scans = kicad_stale::scan_project(&fixture()).unwrap();
        let registry = Registry(scans.clone());
        let entries = kicad_stale::check(&registry, &scans);
        let mut state = StaleState::default();
        state.apply(Ok(entries));
        let stale: Vec<String> = state
            .entries
            .iter()
            .filter(|e| e.is_stale())
            .map(|e| e.footprint.label())
            .collect();
        assert_eq!(
            stale,
            vec![
                "R1 (openparts:RC0603FR-0727RL)".to_string(),
                "R2 (openparts:RC0603FR-0727RL)".to_string()
            ]
        );
        assert_eq!(
            state.updatable(),
            vec![("mfr".to_string(), "RC0603FR-0727RL".to_string())]
        );
        assert!(state.stale_on_board());
        assert!(state.status.contains("2 stale"), "{}", state.status);
        // Only problems by default.
        assert_eq!(state.visible().len(), 2);
        state.show_all = true;
        assert_eq!(state.visible().len(), 12);
    }

    #[test]
    fn board_library_footprints_are_not_updatable() {
        let scans = kicad_stale::scan_project(&fixture()).unwrap();
        let registry = Registry(scans.clone());
        let entries = kicad_stale::check(&registry, &scans);
        let board_lib = entries
            .iter()
            .find(|e| e.footprint.lib_id == "board:61302011121-pico")
            .unwrap();
        // Resolved by its Value.
        assert_eq!(
            board_lib.footprint.mpn_candidates(),
            vec!["61302011121-pico".to_string(), "61302011121".to_string()]
        );
        let _ = Path::new("");
        assert!(update_target(board_lib).is_err());
    }
}
