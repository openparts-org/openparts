mod bundle;

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "openparts", about = "OpenParts command-line interface")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// openparts-server instance used by network-dependent subcommands
    /// (search, show). openparts-server is optional (Architecture
    /// Specification section 4.8) -- every other subcommand works
    /// entirely offline against local `openparts-data` files.
    #[arg(
        long,
        global = true,
        env = "OPENPARTS_REGISTRY_URL",
        default_value = "http://127.0.0.1:8080"
    )]
    registry: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Load Part/Device/Package YAML, validate, build the Effective
    /// Model, and generate KiCad symbol/footprint + STEP.
    Generate {
        #[arg(long)]
        part: PathBuf,
        #[arg(long)]
        device: PathBuf,
        #[arg(long)]
        package: PathBuf,
        /// Source YAML files the part/device/package are allowed to
        /// reference (repeatable).
        #[arg(long = "source")]
        sources: Vec<PathBuf>,
        /// Silicon revision to apply when building the Effective Model.
        #[arg(long)]
        revision: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run openparts-validator against a Part/Device/Package triple.
    /// Validate one part (`--part/--device/--package/--source`) or a
    /// whole dataset (`--data <dir>`).
    Validate {
        #[arg(long, conflicts_with_all = ["part", "device", "package"])]
        data: Option<PathBuf>,
        #[arg(long, requires_all = ["device", "package"])]
        part: Option<PathBuf>,
        #[arg(long)]
        device: Option<PathBuf>,
        #[arg(long)]
        package: Option<PathBuf>,
        #[arg(long = "source")]
        sources: Vec<PathBuf>,
    },
    Search {
        query: String,
    },
    /// Install parts from the registry into a KiCad project's own
    /// libraries (`openparts.kicad_sym`, `openparts.pretty`, 3D models,
    /// library tables).
    KicadInstall {
        #[arg(long)]
        project: PathBuf,
        /// Build the parts from a local openparts-data checkout instead
        /// of fetching them from the registry (works offline, and sees
        /// data the running server has not loaded yet).
        #[arg(long)]
        data: Option<PathBuf>,
        /// `<manufacturer>/<mpn>` (the MPN may itself contain `/`).
        parts: Vec<String>,
    },
    Show {
        manufacturer: String,
        mpn: String,
    },
    /// Prebuild every part of an openparts-data checkout for
    /// distribution: validate, build the Effective Model (base revision)
    /// and write content-addressed STEP, binary STL, 3MF, SolidCraft
    /// design and KiCad symbol/footprint artifacts plus a manifest.json
    /// (see `openparts-server`'s OPENPARTS_ARTIFACT_DIR). Needs
    /// SolidCraft.
    Bundle {
        /// The openparts-data checkout.
        #[arg(long, default_value = "../openparts-data")]
        data: PathBuf,
        /// Output directory (`manifest.json`, `artifacts/sha256/<hex>`).
        #[arg(long)]
        out: PathBuf,
        /// Parts built at once (default: the number of CPUs).
        #[arg(long)]
        jobs: Option<usize>,
        /// Exit with an error unless every part was bundled.
        #[arg(long)]
        strict: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Generate {
            part,
            device,
            package,
            sources,
            revision,
            out,
        } => cmd_generate(part, device, package, sources, revision, out),
        Commands::Validate {
            data: Some(data), ..
        } => cmd_validate_dataset(&data),
        Commands::Validate {
            part: Some(part),
            device: Some(device),
            package: Some(package),
            sources,
            ..
        } => cmd_validate(part, device, package, sources),
        Commands::Validate { .. } => {
            anyhow::bail!("validate needs --data <dir>, or --part, --device and --package")
        }
        Commands::Search { query } => cmd_search(&cli.registry, &query),
        Commands::KicadInstall {
            project,
            data: Some(data),
            parts,
        } => cmd_kicad_install_local(&data, &project, &parts),
        Commands::KicadInstall {
            project,
            data: None,
            parts,
        } => cmd_kicad_install(&cli.registry, &project, &parts),
        Commands::Show { manufacturer, mpn } => cmd_show(&cli.registry, &manufacturer, &mpn),
        Commands::Bundle {
            data,
            out,
            jobs,
            strict,
        } => bundle::run(bundle::Options {
            data,
            out,
            jobs: jobs
                .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get())),
            strict,
        }),
    }
}

fn load_known_sources(paths: &[PathBuf]) -> anyhow::Result<BTreeSet<openparts_core::SourceId>> {
    let mut known = BTreeSet::new();
    for path in paths {
        let source = openparts_data::load_source(path)
            .with_context(|| format!("loading source {}", path.display()))?;
        known.insert(source.id);
    }
    Ok(known)
}

fn load_triple(
    part: &PathBuf,
    device: &PathBuf,
    package: &PathBuf,
) -> anyhow::Result<(
    openparts_core::Part,
    openparts_core::Device,
    openparts_core::Package,
)> {
    let part = openparts_data::load_part(part).context("loading part")?;
    let device = openparts_data::load_device(device).context("loading device")?;
    let package = openparts_data::load_package(package).context("loading package")?;
    Ok((part, device, package))
}

fn print_diagnostics(diags: &[openparts_validator::Diagnostic]) {
    for d in diags {
        let field = d.field_path.as_deref().unwrap_or("");
        eprintln!(
            "[{:?}] {} {} {}: {}",
            d.severity, d.rule_id, d.entity, field, d.message
        );
    }
}

fn cmd_validate(
    part: PathBuf,
    device: PathBuf,
    package: PathBuf,
    sources: Vec<PathBuf>,
) -> anyhow::Result<()> {
    let (part, device, package) = load_triple(&part, &device, &package)?;
    let known_sources = load_known_sources(&sources)?;
    let diags = openparts_validator::validate_part(&part, &device, &package, &known_sources);
    print_diagnostics(&diags);
    let errors = diags.iter().filter(|d| d.is_error()).count();
    if errors == 0 {
        println!("OK: no validation errors");
        Ok(())
    } else {
        anyhow::bail!("{errors} validation error(s)");
    }
}

/// Validates the whole dataset at `data`: every part against the
/// dataset's own devices, packages and sources, every source's formats
/// and every file's location. Errors fail; warnings are listed.
fn cmd_validate_dataset(data: &std::path::Path) -> anyhow::Result<()> {
    use openparts_validator::{validate_location, validate_source, DocKind};
    let dataset = openparts_data::Dataset::load(data)?;
    let sources = dataset.source_ids();
    let mut failures = 0;
    let mut warnings = Vec::new();
    let mut unresolved = 0;
    for issue in &dataset.issues {
        eprintln!("FAIL {}: {}", issue.path.display(), issue.error);
        failures += 1;
    }
    let mut results: Vec<(String, Vec<openparts_validator::Diagnostic>)> = Vec::new();
    let mut report = |shown: &str, diags: Vec<openparts_validator::Diagnostic>| {
        results.push((shown.to_string(), diags));
    };
    use openparts_core::Kind;
    let shown = |kind: Kind, id: &str| {
        dataset
            .path_of(kind, id)
            .map_or_else(|| id.to_string(), |p| p.display().to_string())
    };
    for part in &dataset.parts {
        let diags = match dataset.resolve(part) {
            Ok((device, package)) => {
                openparts_validator::validate_part(part, device, package, &sources)
            }
            Err(e) => {
                eprintln!("FAIL {}: {e}", shown(Kind::Part, &part.id.0));
                unresolved += 1;
                continue;
            }
        };
        report(&shown(Kind::Part, &part.id.0), diags);
    }
    for source in dataset.sources.values() {
        report(&shown(Kind::Source, &source.id.0), validate_source(source));
    }
    let located = dataset
        .parts
        .iter()
        .map(|p| (DocKind::Part, Kind::Part, &p.id.0))
        .chain(
            dataset
                .devices
                .values()
                .map(|d| (DocKind::Device, Kind::Device, &d.id.0)),
        )
        .chain(
            dataset
                .packages
                .values()
                .map(|p| (DocKind::Package, Kind::Package, &p.id.0)),
        )
        .chain(
            dataset
                .sources
                .values()
                .map(|s| (DocKind::Source, Kind::Source, &s.id.0)),
        )
        .chain(
            dataset
                .manufacturers
                .values()
                .map(|m| (DocKind::Manufacturer, Kind::Manufacturer, &m.id.0)),
        );
    for (doc_kind, kind, id) in located {
        if let Some(path) = dataset.path_of(kind, id) {
            let path = path.display().to_string();
            report(&path, validate_location(doc_kind, id, &path));
        }
    }
    for (shown, diags) in results {
        let (errors, warns): (Vec<_>, Vec<_>) = diags.into_iter().partition(|d| d.is_error());
        if !errors.is_empty() {
            eprintln!("FAIL {shown}:");
            print_diagnostics(&errors);
            failures += 1;
        }
        warnings.extend(warns.into_iter().map(|w| (shown.clone(), w)));
    }
    let mut by_rule: std::collections::BTreeMap<&str, usize> = Default::default();
    for (_, w) in &warnings {
        *by_rule.entry(w.rule_id).or_default() += 1;
    }
    if std::env::var_os("OPENPARTS_SHOW_WARNINGS").is_some() {
        for (shown, w) in &warnings {
            eprintln!("WARN {shown}: {} {}", w.rule_id, w.message);
        }
    }
    for (rule, n) in &by_rule {
        println!("warning: {n} x {rule} (OPENPARTS_SHOW_WARNINGS=1 lists them)");
    }
    println!(
        "Checked {} part(s), {} source(s).",
        dataset.parts.len(),
        dataset.sources.len()
    );
    failures += unresolved;
    if failures > 0 {
        anyhow::bail!("{failures} file(s) failed");
    }
    println!("OK: no validation errors");
    Ok(())
}

/// `openparts search <query>` (Architecture Specification section 7.5).
/// Talks to `openparts-server` via `openparts-client`, the crate the
/// rest of the workspace already relies on to abstract this instead of
/// each integration reimplementing its own HTTP handling (section 7.4).
fn cmd_search(registry: &str, query: &str) -> anyhow::Result<()> {
    let client = openparts_client::Client::new(registry);
    let results = client
        .search(query)
        .with_context(|| format!("searching {registry} for {query:?}"))?;
    if results.is_empty() {
        println!("no parts matched {query:?}");
        return Ok(());
    }
    for part in &results {
        println!(
            "{}/{}  existence={:?}  lifecycle={:?}",
            part.manufacturer, part.mpn, part.existence, part.lifecycle
        );
    }
    println!("{} part(s) found", results.len());
    Ok(())
}

/// `openparts kicad-install --project <dir> <manufacturer>/<mpn>...`:
/// every artifact comes from the registry (`--registry`).
fn cmd_kicad_install(
    registry: &str,
    project: &std::path::Path,
    parts: &[String],
) -> anyhow::Result<()> {
    use openparts_client::ArtifactKind;
    let client = openparts_client::Client::new(registry);
    std::fs::create_dir_all(project)?;
    for spec in parts {
        let (manufacturer, mpn) = spec
            .split_once('/')
            .with_context(|| format!("{spec}: expected <manufacturer>/<mpn>"))?;
        let get = |kind| {
            client
                .get_artifact(manufacturer, mpn, kind, None)
                .with_context(|| format!("fetching {spec} {kind:?} from {registry}"))
        };
        let symbol = get(ArtifactKind::KicadSymbol)?;
        let footprint = get(ArtifactKind::KicadFootprint)?;
        let step = get(ArtifactKind::Step)?;
        openparts_kicad::library::install_into_project(
            project,
            mpn,
            &symbol.content,
            &footprint.content,
            step.content.as_bytes(),
        )
        .with_context(|| format!("installing {spec}"))?;
        println!("installed {spec}");
    }
    Ok(())
}

/// `openparts kicad-install --data <dir>`: the same installation, with
/// each part built from a local openparts-data checkout.
fn cmd_kicad_install_local(
    data: &std::path::Path,
    project: &std::path::Path,
    parts: &[String],
) -> anyhow::Result<()> {
    use openparts_artifacts::{ArtifactKind, PartBuild};
    let dataset = openparts_data::Dataset::load(data)
        .with_context(|| format!("loading {}", data.display()))?;
    std::fs::create_dir_all(project)?;
    for spec in parts {
        let (manufacturer, mpn) = spec
            .split_once('/')
            .with_context(|| format!("{spec}: expected <manufacturer>/<mpn>"))?;
        let part = dataset
            .part(manufacturer, mpn)
            .with_context(|| format!("{spec}: no such part in {}", data.display()))?;
        let build = PartBuild::from_dataset(&dataset, part, None)
            .with_context(|| format!("building {spec}"))?;
        let symbol = String::from_utf8(build.render(ArtifactKind::KicadSymbol)?)?;
        let footprint = String::from_utf8(build.render(ArtifactKind::KicadFootprint)?)?;
        let step = build
            .models(&[ArtifactKind::Step])
            .with_context(|| format!("generating the 3D model of {spec} with SolidCraft"))?;
        openparts_kicad::library::install_into_project(project, mpn, &symbol, &footprint, &step[0])
            .with_context(|| format!("installing {spec}"))?;
        println!("installed {spec}");
    }
    Ok(())
}

/// `openparts show <manufacturer>/<mpn>` (Architecture Specification
/// section 7.5).
fn cmd_show(registry: &str, manufacturer: &str, mpn: &str) -> anyhow::Result<()> {
    let client = openparts_client::Client::new(registry);
    let part = client
        .get_part(manufacturer, mpn)
        .with_context(|| format!("fetching {manufacturer}/{mpn} from {registry}"))?;
    println!("id:          {}", part.id);
    println!("manufacturer: {}", part.manufacturer);
    println!("mpn:         {}", part.mpn);
    println!("device:      {}", part.device);
    println!("package:     {}", part.package);
    println!("existence:   {:?}", part.existence.status);
    println!("lifecycle:   {:?}", part.lifecycle.status);
    if !part.lifecycle.replacement.is_empty() {
        let replacements: Vec<String> = part
            .lifecycle
            .replacement
            .iter()
            .map(ToString::to_string)
            .collect();
        println!("replacement: {}", replacements.join(", "));
    }
    if !part.sources.is_empty() {
        let sources: Vec<String> = part.sources.iter().map(ToString::to_string).collect();
        println!("sources:     {}", sources.join(", "));
    }
    Ok(())
}

fn cmd_generate(
    part_path: PathBuf,
    device_path: PathBuf,
    package_path: PathBuf,
    sources: Vec<PathBuf>,
    revision: Option<String>,
    out_dir: PathBuf,
) -> anyhow::Result<()> {
    use openparts_artifacts::{ArtifactKind, BuildError, PartBuild};

    let (part, device, package) = load_triple(&part_path, &device_path, &package_path)?;
    let known_sources = load_known_sources(&sources)?;
    let build = match PartBuild::new(
        &part,
        &device,
        &package,
        &known_sources,
        revision.as_deref(),
    ) {
        Ok(b) => b,
        Err(BuildError::Validation(diags)) => {
            print_diagnostics(&diags);
            anyhow::bail!(
                "{} validation diagnostic(s); aborting generate",
                diags.len()
            );
        }
        Err(e) => return Err(e).context("building the effective model"),
    };

    std::fs::create_dir_all(&out_dir)?;
    let write = |kind: ArtifactKind, bytes: &[u8]| {
        std::fs::write(out_dir.join(build.file_name(kind)), bytes)
    };
    for kind in [
        ArtifactKind::KicadSymbol,
        ArtifactKind::KicadFootprint,
        ArtifactKind::FootprintSvg,
    ] {
        write(kind, &build.render(kind)?)?;
    }
    // The detailed 3D model, built and exported by SolidCraft in one run.
    let models = build
        .models(&ArtifactKind::MODELS)
        .context("generating the 3D model with SolidCraft")?;
    for (kind, bytes) in ArtifactKind::MODELS.into_iter().zip(&models) {
        write(kind, bytes)?;
    }

    // LibrePCB library element (a directory tree under <out>/<mpn>.lplib/),
    // embedding the same STEP as its 3D model.
    let librepcb_dir = out_dir.join(format!("{}.lplib", build.file_stem()));
    let step_text = std::str::from_utf8(&models[0]).context("SolidCraft STEP is not UTF-8")?;
    for file in build.librepcb(Some(step_text))? {
        let path = librepcb_dir.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, file.content)?;
    }

    // Reproducibility record (Testing and Quality Specification section
    // 10): the conditions this output was generated under.
    let report = format!(
        "schema_version: \"{}\"\npart_id: \"{}\"\napplied_revision: {}\ngenerator:\n  openparts: \"{}\"\ntoolchain:\n  rustc: \"{}\"\n  solidcraft: \"{}\"\n",
        part.schema_version,
        part.id,
        build
            .model
            .applied_revision
            .as_deref()
            .map_or_else(|| "null".to_string(), |r| format!("\"{r}\"")),
        env!("CARGO_PKG_VERSION"),
        rustc_version(),
        openparts_solidcraft::runner::version().unwrap_or_else(|_| "unknown".to_string()),
    );
    std::fs::write(
        out_dir.join(format!("{}.generation-report.yaml", build.file_stem())),
        report,
    )?;

    println!(
        "Generated KiCad symbol/footprint, STEP/STL/3MF models and a SolidCraft design, a LibrePCB library, and a generation report into {}",
        out_dir.display()
    );
    Ok(())
}

fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
