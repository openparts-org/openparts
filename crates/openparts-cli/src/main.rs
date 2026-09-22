use anyhow::Context;
use clap::{Parser, Subcommand};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "openparts", about = "OpenParts command-line interface")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
    Validate {
        #[arg(long)]
        part: PathBuf,
        #[arg(long)]
        device: PathBuf,
        #[arg(long)]
        package: PathBuf,
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
    Provenance {
        manufacturer: String,
        mpn: String,
    },
    Diff {
        from: String,
        to: String,
    },
    Install,
    Update,
    Bundle,
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
            part,
            device,
            package,
            sources,
        } => cmd_validate(part, device, package, sources),
        _ => {
            eprintln!(
                "this subcommand is not implemented yet (Architecture Specification section 7.5)"
            );
            Ok(())
        }
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

fn run_validation(
    part: &openparts_core::Part,
    device: &openparts_core::Device,
    package: &openparts_core::Package,
    known_sources: &BTreeSet<openparts_core::SourceId>,
) -> Vec<openparts_validator::Diagnostic> {
    openparts_validator::validate_part(part, device, package, known_sources)
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
    let diags = run_validation(&part, &device, &package, &known_sources);
    if diags.is_empty() {
        println!("OK: no validation diagnostics");
        Ok(())
    } else {
        print_diagnostics(&diags);
        anyhow::bail!("{} validation diagnostic(s)", diags.len());
    }
}

fn cmd_generate(
    part_path: PathBuf,
    device_path: PathBuf,
    package_path: PathBuf,
    sources: Vec<PathBuf>,
    revision: Option<String>,
    out_dir: PathBuf,
) -> anyhow::Result<()> {
    let (part, device, package) = load_triple(&part_path, &device_path, &package_path)?;
    let known_sources = load_known_sources(&sources)?;

    let diags = run_validation(&part, &device, &package, &known_sources);
    if !diags.is_empty() {
        print_diagnostics(&diags);
        anyhow::bail!(
            "{} validation diagnostic(s); aborting generate",
            diags.len()
        );
    }

    let model = openparts_core::build_effective_model(
        part.clone(),
        &device,
        package.clone(),
        revision.as_deref(),
    )
    .context("building effective model")?;

    let geometry = openparts_mcad::generate_lqfp(&model.package).context("generating geometry")?;

    std::fs::create_dir_all(&out_dir)?;

    let symbol = openparts_pcbcad::build_symbol(&part.mpn, &model.device);
    let symbol_text = openparts_kicad::render_symbol(&symbol);
    std::fs::write(out_dir.join(format!("{}.kicad_sym", part.mpn)), symbol_text)?;

    let footprint = openparts_pcbcad::build_footprint(&part.mpn, &geometry);
    let footprint_text = openparts_kicad::render_footprint(&footprint);
    std::fs::write(
        out_dir.join(format!("{}.kicad_mod", part.mpn)),
        footprint_text,
    )?;

    let step_text =
        openparts_step::generate_step(&geometry, &part.mpn).context("generating STEP")?;
    std::fs::write(out_dir.join(format!("{}.step", part.mpn)), step_text)?;

    // Reproducibility record (Testing and Quality Specification section
    // 10): the conditions this specific output was generated under.
    let report = format!(
        "schema_version: \"{}\"\npart_id: \"{}\"\napplied_revision: {}\ngenerator:\n  openparts_mcad: \"{}\"\n  openparts_kicad: \"{}\"\n  openparts_step: \"{}\"\ntoolchain:\n  rustc: \"{}\"\n",
        part.schema_version,
        part.id,
        model.applied_revision.as_deref().map(|r| format!("\"{r}\"")).unwrap_or_else(|| "null".to_string()),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_VERSION"),
        rustc_version(),
    );
    std::fs::write(
        out_dir.join(format!("{}.generation-report.yaml", part.mpn)),
        report,
    )?;

    println!(
        "Generated KiCad symbol/footprint, STEP model, and a generation report into {}",
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
