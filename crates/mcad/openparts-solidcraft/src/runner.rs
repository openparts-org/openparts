//! Runs a [`Script`] through the locally installed SolidCraft CLI.
//!
//! Which `solidcraft-cli` is used, in order:
//! 1. `OPENPARTS_SOLIDCRAFT_CLI`: the path of a `solidcraft-cli` binary.
//! 2. `OPENPARTS_SOLIDCRAFT_FLATPAK`: a Flatpak app id to run it from.
//! 3. `solidcraft-cli` on `PATH`.
//! 4. The Flatpak app `io.tangencylabs.solidcraft`, when installed.
//!
//! A Flatpak app only sees the host directories it is granted, so each run
//! grants exactly its own temporary working directory (`--filesystem`).

use crate::Script;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

pub const DEFAULT_FLATPAK_APP: &str = "io.tangencylabs.solidcraft";

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(
        "no SolidCraft CLI found: install SolidCraft (Flatpak {DEFAULT_FLATPAK_APP}), put \
         solidcraft-cli on PATH, or set OPENPARTS_SOLIDCRAFT_CLI"
    )]
    NotFound,
    #[error("I/O error around the SolidCraft run: {0}")]
    Io(#[from] std::io::Error),
    #[error("SolidCraft failed ({status}): {stderr}")]
    Failed { status: String, stderr: String },
    #[error("SolidCraft reported success but wrote no {0}")]
    MissingOutput(String),
}

/// What to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// AP242 STEP with per-body colours.
    Step,
    /// Binary STL.
    Stl,
    /// The SolidCraft design itself (`.sc`): the parametric model, for
    /// editing in SolidCraft.
    Design,
    /// A PNG preview (isometric).
    Png,
}

impl Format {
    fn extension(self) -> &'static str {
        match self {
            Format::Step => "step",
            Format::Stl => "stl",
            Format::Design => "sc",
            Format::Png => "png",
        }
    }
}

#[derive(Debug, Clone)]
enum Cli {
    Binary(PathBuf),
    Flatpak(String),
}

fn cli() -> Result<&'static Cli, RunError> {
    static CLI: OnceLock<Option<Cli>> = OnceLock::new();
    CLI.get_or_init(resolve).as_ref().ok_or(RunError::NotFound)
}

fn resolve() -> Option<Cli> {
    if let Some(path) = std::env::var_os("OPENPARTS_SOLIDCRAFT_CLI") {
        return Some(Cli::Binary(PathBuf::from(path)));
    }
    if let Ok(app) = std::env::var("OPENPARTS_SOLIDCRAFT_FLATPAK") {
        return Some(Cli::Flatpak(app));
    }
    if let Some(path) = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("solidcraft-cli"))
            .find(|candidate| candidate.is_file())
    }) {
        return Some(Cli::Binary(path));
    }
    let installed = Command::new("flatpak")
        .args(["info", DEFAULT_FLATPAK_APP])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    installed.then(|| Cli::Flatpak(DEFAULT_FLATPAK_APP.to_string()))
}

impl Cli {
    fn command(&self, workdir: &Path) -> Command {
        match self {
            Cli::Binary(path) => Command::new(path),
            Cli::Flatpak(app) => {
                let mut c = Command::new("flatpak");
                c.arg("run")
                    .arg(format!("--filesystem={}", workdir.display()))
                    .arg("--command=solidcraft-cli")
                    .arg(app);
                c
            }
        }
    }
}

/// A private scratch directory, removed on drop.
struct WorkDir(PathBuf);

impl WorkDir {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "openparts-solidcraft-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir)?;
        Ok(WorkDir(dir))
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `name` made safe as a file stem. SolidCraft names the STEP product
/// after the output file, so this is also the product name.
fn file_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.+".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "part".into()
    } else {
        s
    }
}

/// Runs `script` and returns each requested export's bytes, in order.
pub fn run(
    script: &Script,
    product_name: &str,
    formats: &[Format],
) -> Result<Vec<Vec<u8>>, RunError> {
    let cli = cli()?;
    let work = WorkDir::new()?;
    let script_path = work.0.join("script.json");
    std::fs::write(&script_path, script.to_string_pretty())?;
    let stem = file_stem(product_name);

    let mut outputs = Vec::new();
    let mut files = Vec::new();
    let mut png = None;
    for format in formats {
        let path = work.0.join(format!("{stem}.{}", format.extension()));
        match format {
            Format::Png => png = Some(path.clone()),
            Format::Design => {
                outputs.extend(["--save".into(), path.display().to_string()]);
            }
            _ => outputs.extend(["--out".into(), path.display().to_string()]),
        }
        files.push(path);
    }

    if !outputs.is_empty() {
        // Every command is checked against SolidCraft's own parameter
        // contract, so a misspelt template key fails loudly.
        let mut cmd = cli.command(&work.0);
        cmd.arg("run").arg(&script_path).args(&outputs).args([
            "--quiet",
            "--param-check",
            "strict",
        ]);
        check(cmd.output()?)?;
    }
    if let Some(png) = png {
        let mut cmd = cli.command(&work.0);
        cmd.arg("snapshot")
            .arg(&script_path)
            .arg("--out")
            .arg(&png)
            .args([
                "--view",
                "iso",
                "--width",
                "900",
                "--height",
                "700",
                "--no-grid",
                "--no-axes",
            ]);
        check(cmd.output()?)?;
    }

    files
        .iter()
        .map(|path| {
            std::fs::read(path).map_err(|_| RunError::MissingOutput(path.display().to_string()))
        })
        .collect()
}

fn check(output: std::process::Output) -> Result<(), RunError> {
    if output.status.success() {
        return Ok(());
    }
    Err(RunError::Failed {
        status: output.status.to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

/// The SolidCraft CLI's version line (`solidcraft-cli 0.30.1`), for
/// generation reports.
pub fn version() -> Result<String, RunError> {
    let cli = cli()?;
    let dir = std::env::temp_dir();
    let output = cli.command(&dir).arg("--version").output()?;
    check(output.clone())?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Whether a SolidCraft CLI can be found (tests that need one skip
/// themselves without it).
pub fn available() -> bool {
    cli().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_stems_are_safe() {
        assert_eq!(file_stem("LPC1114FBD48/302"), "LPC1114FBD48_302");
        assert_eq!(file_stem(""), "part");
    }
}
