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
    if diags.is_empty() {
        println!("OK: no validation diagnostics");
        Ok(())
    } else {
        print_diagnostics(&diags);
        anyhow::bail!("{} validation diagnostic(s)", diags.len());
    }
}

/// Validates every part of the dataset at `data` against the dataset's
/// own devices, packages and sources; files that fail to load count as
/// failures too.
fn cmd_validate_dataset(data: &std::path::Path) -> anyhow::Result<()> {
    let dataset = openparts_data::Dataset::load(data)?;
    let sources = dataset.source_ids();
    let mut failures = 0;
    for issue in &dataset.issues {
        eprintln!("FAIL {}: {}", issue.path.display(), issue.error);
        failures += 1;
    }
    for part in &dataset.parts {
        let path = dataset.paths.get(&part.id.to_string());
        let shown = path.map_or_else(|| part.id.to_string(), |p| p.display().to_string());
        let diags = match dataset.resolve(part) {
            Ok((device, package)) => {
                openparts_validator::validate_part(part, device, package, &sources)
            }
            Err(e) => {
                eprintln!("FAIL {shown}: {e}");
                failures += 1;
                continue;
            }
        };
        if !diags.is_empty() {
            eprintln!("FAIL {shown}:");
            print_diagnostics(&diags);
            failures += 1;
        }
    }
    println!("Checked {} part(s).", dataset.parts.len());
    if failures > 0 {
        anyhow::bail!("{failures} part(s) or file(s) failed");
    }
    println!("OK: no validation diagnostics");
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
