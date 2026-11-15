mod bom;
mod install;
mod jobs;
mod kicad_detect;
mod schematic;
mod search;
mod stale;
mod ui;

use jobs::{Job, Waker};
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let project_dir = parse_project_dir_arg(std::env::args());
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1180.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "OpenParts for KiCad",
        options,
        Box::new(|cc| {
            let ctx = cc.egui_ctx.clone();
            let waker: Waker = Arc::new(move || ctx.request_repaint());
            Ok(Box::new(App::new(project_dir, waker)))
        }),
    )
}

/// Parses an optional `--project-dir <path>` / `--project-dir=<path>`
/// argument, used by `kicad-integration/openparts_launcher.py` to
/// pre-fill the project directory with KiCad's own currently-open
/// project path.
fn parse_project_dir_arg(args: impl Iterator<Item = String>) -> Option<String> {
    let mut args = args.skip(1);
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--project-dir=") {
            return Some(value.to_string());
        }
        if arg == "--project-dir" {
            return args.next();
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Search,
    Bom,
    Stale,
}

/// Everything the window shows; `ui.rs` draws it.
pub struct App {
    pub registry_url: String,
    pub project_dir: String,
    pub tab: Tab,
    pub search: search::SearchState,
    pub bom: bom::BomState,
    pub bom_source: bom::BomSource,
    pub stale: stale::StaleState,
    /// The last action's outcome (install, MPN edit), shown in the
    /// status bar.
    pub status: String,
    install: Option<(Job<String>, Tab)>,
    pub waker: Waker,
}

impl App {
    pub fn new(project_dir: Option<String>, waker: Waker) -> Self {
        App {
            registry_url: std::env::var("OPENPARTS_REGISTRY_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            project_dir: project_dir.unwrap_or_default(),
            tab: Tab::Search,
            search: search::SearchState::default(),
            bom: bom::BomState::default(),
            bom_source: bom::BomSource::Auto,
            stale: stale::StaleState::default(),
            status: String::new(),
            install: None,
            waker,
        }
    }

    pub fn project(&self) -> Option<PathBuf> {
        let p = self.project_dir.trim();
        (!p.is_empty()).then(|| PathBuf::from(p))
    }

    pub fn installing(&self) -> bool {
        self.install.is_some()
    }

    /// Installs parts (kicad-install) in the background; `from` is the
    /// tab to refresh afterwards.
    pub fn install(&mut self, parts: Vec<(String, String)>, revision: Option<String>, from: Tab) {
        let Some(project) = self.project() else {
            self.status = "Set the project directory first.".into();
            return;
        };
        if parts.is_empty() {
            return;
        }
        let client = openparts_client::Client::new(self.registry_url.trim());
        self.status = format!(
            "Installing {}…",
            parts
                .iter()
                .map(|p| p.1.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let job = Job::spawn(&self.waker, move || {
            let mut done = Vec::new();
            for (m, p) in &parts {
                install::install_part(&client, m, p, revision.as_deref(), &project)
                    .map_err(|e| format!("{m}/{p}: {e}"))?;
                done.push(p.clone());
            }
            Ok(format!(
                "Installed {} into {}",
                done.join(", "),
                project.display()
            ))
        });
        self.install = Some((job, from));
    }

    pub fn run_bom(&mut self) {
        match self.project() {
            Some(p) => {
                let registry = self.registry_url.trim().to_string();
                self.bom.run(p, &registry, self.bom_source, &self.waker)
            }
            None => self.status = "Set the project directory first.".into(),
        }
    }

    pub fn run_stale(&mut self) {
        match self.project() {
            Some(p) => {
                let registry = self.registry_url.trim().to_string();
                self.stale.run(p, &registry, &self.waker)
            }
            None => self.status = "Set the project directory first.".into(),
        }
    }

    /// Per-frame bookkeeping: finished jobs, due searches. Returns when
    /// to wake for a pending debounce.
    pub fn tick(&mut self) -> Option<std::time::Duration> {
        self.bom.tick();
        self.stale.tick();
        if let Some(r) = self.install.as_ref().and_then(|(j, _)| j.poll()) {
            let (_, from) = self.install.take().expect("polled above");
            match r {
                Ok(msg) => {
                    self.status = msg;
                    if from == Tab::Stale {
                        self.run_stale();
                    }
                }
                Err(e) => self.status = format!("Install failed: {e}"),
            }
        }
        let registry = self.registry_url.trim().to_string();
        self.search.tick(&registry, &self.waker)
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        if let Some(wait) = self.tick() {
            ctx.request_repaint_after(wait);
        }
        ui::draw(self, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dir_arg_absent_yields_none() {
        let args = ["openparts-kicad-plugin".to_string()];
        assert_eq!(parse_project_dir_arg(args.into_iter()), None);
    }

    #[test]
    fn project_dir_arg_space_separated() {
        let args = [
            "openparts-kicad-plugin".to_string(),
            "--project-dir".to_string(),
            "/home/user/my-project".to_string(),
        ];
        assert_eq!(
            parse_project_dir_arg(args.into_iter()),
            Some("/home/user/my-project".to_string())
        );
    }

    #[test]
    fn project_dir_arg_equals_separated() {
        let args = [
            "openparts-kicad-plugin".to_string(),
            "--project-dir=/home/user/my-project".to_string(),
        ];
        assert_eq!(
            parse_project_dir_arg(args.into_iter()),
            Some("/home/user/my-project".to_string())
        );
    }

    #[test]
    fn actions_need_a_project() {
        let mut app = App::new(None, jobs::no_waker());
        app.run_bom();
        assert!(app.status.contains("project directory"));
        app.status.clear();
        app.install(vec![("m".into(), "p".into())], None, Tab::Search);
        assert!(app.status.contains("project directory"));
        assert!(!app.installing());
    }
}
