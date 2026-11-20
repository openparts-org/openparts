//! The generation pipeline (Architecture Specification section 51):
//! Part resolution -> validation -> Effective Model -> geometry -> every
//! artifact. `openparts generate`, `openparts bundle` and
//! openparts-server all build artifacts through this crate, so they
//! cannot drift apart.

pub mod bundle;
pub mod pcbcraft;

use openparts_core::{Device, EffectiveModel, Package, Part, SourceId};
use openparts_data::Dataset;
use openparts_mcad::MechanicalGeometry;
use openparts_pcbcad::{PcbFootprint, PcbSymbol};
pub use openparts_placement::{Density, PlacementModel};
use openparts_solidcraft::{Format, ModelInput};
use std::collections::BTreeSet;

pub use openparts_librepcb::LpFile;

/// An artifact a part is published as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArtifactKind {
    KicadSymbol,
    KicadFootprint,
    /// A browser preview of the footprint (top view).
    FootprintSvg,
    /// A browser preview of the KiCad symbol (every unit).
    SymbolSvg,
    Step,
    Stl,
    ThreeMf,
    /// The editable SolidCraft design (`.sc`).
    SolidCraft,
}

impl ArtifactKind {
    pub const ALL: [ArtifactKind; 8] = [
        ArtifactKind::KicadSymbol,
        ArtifactKind::KicadFootprint,
        ArtifactKind::FootprintSvg,
        ArtifactKind::SymbolSvg,
        ArtifactKind::Step,
        ArtifactKind::Stl,
        ArtifactKind::ThreeMf,
        ArtifactKind::SolidCraft,
    ];
    /// The 3D kinds, all exported from one SolidCraft run.
    pub const MODELS: [ArtifactKind; 4] = [
        ArtifactKind::Step,
        ArtifactKind::Stl,
        ArtifactKind::ThreeMf,
        ArtifactKind::SolidCraft,
    ];

    /// The id used in URLs and the bundle manifest.
    pub fn id(self) -> &'static str {
        match self {
            ArtifactKind::KicadSymbol => "kicad-symbol",
            ArtifactKind::KicadFootprint => "kicad-footprint",
            ArtifactKind::FootprintSvg => "footprint-svg",
            ArtifactKind::SymbolSvg => "symbol-svg",
            ArtifactKind::Step => "step",
            ArtifactKind::Stl => "stl",
            ArtifactKind::ThreeMf => "3mf",
            ArtifactKind::SolidCraft => "solidcraft",
        }
    }

    pub fn from_id(id: &str) -> Option<ArtifactKind> {
        ArtifactKind::ALL.into_iter().find(|k| k.id() == id)
    }

    pub fn extension(self) -> &'static str {
        match self {
            ArtifactKind::KicadSymbol => "kicad_sym",
            ArtifactKind::KicadFootprint => "kicad_mod",
            ArtifactKind::FootprintSvg => "footprint.svg",
            ArtifactKind::SymbolSvg => "symbol.svg",
            ArtifactKind::Step => "step",
            ArtifactKind::Stl => "stl",
            ArtifactKind::ThreeMf => "3mf",
            ArtifactKind::SolidCraft => "sc",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            ArtifactKind::KicadSymbol | ArtifactKind::KicadFootprint => "text/plain; charset=utf-8",
            ArtifactKind::FootprintSvg | ArtifactKind::SymbolSvg => "image/svg+xml",
            ArtifactKind::Step => "model/step",
            ArtifactKind::Stl => "model/stl",
            ArtifactKind::ThreeMf => "model/3mf",
            ArtifactKind::SolidCraft => "application/octet-stream",
        }
    }

    /// The SolidCraft export for a 3D kind.
    pub fn model_format(self) -> Option<Format> {
        match self {
            ArtifactKind::Step => Some(Format::Step),
            ArtifactKind::Stl => Some(Format::Stl),
            ArtifactKind::ThreeMf => Some(Format::ThreeMf),
            ArtifactKind::SolidCraft => Some(Format::Design),
            _ => None,
        }
    }

    /// Whether the artifact depends on the silicon revision (pins), not
    /// only on the package.
    pub fn depends_on_revision(self) -> bool {
        matches!(self, ArtifactKind::KicadSymbol | ArtifactKind::SymbolSvg)
    }
}

/// The pipeline stage that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Resolve,
    Validation,
    EffectiveModel,
    Geometry,
    Model3d,
    /// Resolving a module's `assembly` (its components' parts).
    Assembly,
}

impl Stage {
    pub fn id(self) -> &'static str {
        match self {
            Stage::Resolve => "resolve",
            Stage::Validation => "validation",
            Stage::EffectiveModel => "effective-model",
            Stage::Geometry => "geometry",
            Stage::Model3d => "3d-model",
            Stage::Assembly => "assembly",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(transparent)]
    Resolve(#[from] openparts_data::ResolveError),
    #[error("{} validation diagnostic(s): {}", .0.len(), describe(.0))]
    Validation(Vec<openparts_validator::Diagnostic>),
    #[error(transparent)]
    EffectiveModel(#[from] openparts_core::EffectiveModelError),
    #[error(transparent)]
    Geometry(#[from] openparts_mcad::McadError),
    #[error(transparent)]
    Model3d(#[from] openparts_solidcraft::Error),
    /// A component of the part's `assembly` could not be resolved or
    /// built (section 20.12); only the 3D model fails.
    #[error("assembly: {0}")]
    Assembly(String),
}

impl BuildError {
    pub fn stage(&self) -> Stage {
        match self {
            BuildError::Resolve(_) => Stage::Resolve,
            BuildError::Validation(_) => Stage::Validation,
            BuildError::EffectiveModel(_) => Stage::EffectiveModel,
            BuildError::Geometry(_) => Stage::Geometry,
            BuildError::Model3d(_) => Stage::Model3d,
            BuildError::Assembly(_) => Stage::Assembly,
        }
    }
}

/// One line per diagnostic, `; `-separated.
pub fn describe(diags: &[openparts_validator::Diagnostic]) -> String {
    diags
        .iter()
        .map(|d| {
            format!(
                "[{:?}] {} {} {}: {}",
                d.severity,
                d.rule_id,
                d.entity,
                d.field_path.as_deref().unwrap_or(""),
                d.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// A validated part's Effective Model, from which every artifact is
/// built. Its geometry is generated once; a geometry error fails only
/// the artifacts that need it (the symbol doesn't).
#[derive(Debug)]
pub struct PartBuild {
    pub model: EffectiveModel,
    /// The part's datasheet URL, when built from a dataset that has it.
    pub datasheet: Option<String>,
    geometry: Result<MechanicalGeometry, openparts_mcad::McadError>,
    /// The components mounted on a module (`Part::assembly`), resolved
    /// against a dataset ([`PartBuild::with_assembly`]); an error fails
    /// the 3D model only.
    assembly: Result<Vec<Mounted>, String>,
}

/// One resolved `assembly` entry: its placement and, for a `part:`
/// component, that part's own build.
#[derive(Debug)]
pub struct Mounted {
    pub component: openparts_core::AssemblyComponent,
    pub build: Option<Box<PartBuild>>,
}

impl PartBuild {
    /// Validates `part` against `device`, `package` and the known
    /// sources, then builds its Effective Model at `revision` (the base
    /// device when `None`).
    pub fn new(
        part: &Part,
        device: &Device,
        package: &Package,
        sources: &BTreeSet<SourceId>,
        revision: Option<&str>,
    ) -> Result<PartBuild, BuildError> {
        let diags = openparts_validator::validate_part(part, device, package, sources);
        if diags.iter().any(|d| d.is_error()) {
            return Err(BuildError::Validation(
                diags.into_iter().filter(|d| d.is_error()).collect(),
            ));
        }
        let model =
            openparts_core::build_effective_model(part.clone(), device, package.clone(), revision)?;
        let geometry = openparts_mcad::generate(&model.package);
        let assembly = if part.assembly.is_empty() {
            Ok(vec![])
        } else {
            Err("the part's assembly was not resolved against a dataset".to_string())
        };
        Ok(PartBuild {
            model,
            datasheet: None,
            geometry,
            assembly,
        })
    }

    /// Resolves the part's `assembly` against `dataset`: each `part:`
    /// component is validated and built like any part (its own
    /// assembly, if any, is not drawn: no nesting). A failure is kept
    /// and fails the 3D model only.
    pub fn with_assembly(mut self, dataset: &Dataset) -> PartBuild {
        let sources = dataset.source_ids();
        self.assembly =
            self.model
                .part
                .assembly
                .iter()
                .map(|c| {
                    let build = match &c.part {
                        None => None,
                        Some(id) => {
                            let part =
                                dataset.parts.iter().find(|p| p.id == *id).ok_or_else(|| {
                                    format!("{}: part {id} not found", c.reference)
                                })?;
                            let (device, package) = dataset
                                .resolve(part)
                                .map_err(|e| format!("{}: {e}", c.reference))?;
                            let build = PartBuild::new(part, device, package, &sources, None)
                                .map_err(|e| format!("{} ({id}): {e}", c.reference))?;
                            Some(Box::new(build))
                        }
                    };
                    if build.is_none() && c.body.is_none() {
                        return Err(format!("{}: neither part nor body", c.reference));
                    }
                    Ok(Mounted {
                        component: c.clone(),
                        build,
                    })
                })
                .collect();
        self
    }

    /// The resolved assembly (empty for an ordinary part).
    pub fn assembly(&self) -> Result<&[Mounted], BuildError> {
        self.assembly
            .as_deref()
            .map_err(|e| BuildError::Assembly(e.clone()))
    }

    /// [`PartBuild::new`] for a part of `dataset`, validated against the
    /// dataset's sources.
    pub fn from_dataset(
        dataset: &Dataset,
        part: &Part,
        revision: Option<&str>,
    ) -> Result<PartBuild, BuildError> {
        let (device, package) = dataset.resolve(part)?;
        let mut build = PartBuild::new(part, device, package, &dataset.source_ids(), revision)?
            .with_assembly(dataset);
        build.datasheet = part
            .sources
            .iter()
            .filter_map(|id| dataset.sources.get(&id.0))
            .find(|s| s.document_type == openparts_core::DocumentType::Datasheet)
            .map(|s| s.url.clone());
        Ok(build)
    }

    pub fn mpn(&self) -> &str {
        &self.model.part.mpn
    }

    /// A file name stem for this part's artifacts (MPNs may hold `/`).
    pub fn file_stem(&self) -> String {
        openparts_pcbcad::safe_filename(self.mpn())
    }

    /// `<stem>.<extension>` for `kind`.
    pub fn file_name(&self, kind: ArtifactKind) -> String {
        format!("{}.{}", self.file_stem(), kind.extension())
    }

    pub fn symbol(&self) -> PcbSymbol {
        let mut symbol = openparts_pcbcad::build_symbol(self.mpn(), &self.model.device);
        if let Some(url) = &self.datasheet {
            symbol.datasheet = url.clone();
        }
        symbol
    }

    pub fn geometry(&self) -> Result<&MechanicalGeometry, BuildError> {
        self.geometry
            .as_ref()
            .map_err(|e| BuildError::Geometry(e.clone()))
    }

    /// The part's Placement Model at `density`: the package's geometry
    /// with the device's pin names and shared contacts.
    pub fn placement(&self, density: Density) -> Result<PlacementModel, BuildError> {
        Ok(openparts_placement::build(
            &self.model.package,
            Some(&self.model.device),
            self.geometry()?,
            density,
        ))
    }

    /// The footprint: the Placement Model at density level N (with
    /// overlaid pads for pins that share a contact, `connected_to`),
    /// carrying the model's hash.
    pub fn footprint(&self) -> Result<PcbFootprint, BuildError> {
        Ok(openparts_pcbcad::footprint_from_placement(
            self.mpn(),
            &self.placement(Density::N)?,
        ))
    }

    /// The 3D model's input: geometry plus the device's category and
    /// polarity pin.
    pub fn model_input(&self) -> Result<ModelInput<'_>, BuildError> {
        Ok(ModelInput::for_device(
            &self.model.package,
            self.geometry()?,
            &self.model.device,
        ))
    }

    /// The 3D `kinds` (each one of [`ArtifactKind::MODELS`]), in order,
    /// from one SolidCraft run. A module with an `assembly` is the board
    /// with each component's own model (or placeholder box) on it.
    pub fn models(&self, kinds: &[ArtifactKind]) -> Result<Vec<Vec<u8>>, BuildError> {
        let formats: Vec<Format> = kinds
            .iter()
            .map(|k| k.model_format().expect("models() takes 3D artifact kinds"))
            .collect();
        let assembly = self.assembly()?;
        if assembly.is_empty() {
            return Ok(openparts_solidcraft::generate(
                &self.model_input()?,
                self.mpn(),
                &formats,
            )?);
        }
        let placed = self.placed(assembly)?;
        Ok(openparts_solidcraft::generate_assembly(
            &self.model_input()?,
            &placed,
            self.mpn(),
            &formats,
        )?)
    }

    /// Each assembly component as SolidCraft places it: the component's
    /// model frame (its generator's) taken to its Placement Model frame
    /// (`model3d`), then onto the board by its `x`/`y`/`rotation`/`side`.
    fn placed<'a>(
        &self,
        assembly: &'a [Mounted],
    ) -> Result<Vec<openparts_solidcraft::Placed<'a>>, BuildError> {
        let body = &self.geometry()?.body;
        let top = body.position.z + body.size.z / 2.0;
        let bottom = body.position.z - body.size.z / 2.0;
        assembly
            .iter()
            .map(|m| {
                let c = &m.component;
                let on_board = c.matrix(top, bottom);
                let (mounted, matrix) = match (&m.build, &c.body) {
                    (Some(build), _) => {
                        let fail =
                            |e: BuildError| BuildError::Assembly(format!("{}: {e}", c.reference));
                        let m3d = build.placement(Density::N).map_err(fail)?.model3d;
                        let (s, co) = m3d.rotation.to_radians().sin_cos();
                        let align = [
                            co,
                            -s,
                            0.0,
                            m3d.offset[0],
                            s,
                            co,
                            0.0,
                            m3d.offset[1],
                            0.0,
                            0.0,
                            1.0,
                            m3d.offset[2],
                        ];
                        let input = build.model_input().map_err(fail)?;
                        (
                            openparts_solidcraft::Mounted::Model {
                                key: build.model.part.id.to_string(),
                                input,
                            },
                            compose(&on_board, &align),
                        )
                    }
                    (None, Some(b)) => (
                        openparts_solidcraft::Mounted::Box {
                            size: [b.width, b.length, b.height],
                            color: b.color.clone(),
                        },
                        on_board,
                    ),
                    (None, None) => {
                        return Err(BuildError::Assembly(format!(
                            "{}: neither part nor body",
                            c.reference
                        )))
                    }
                };
                Ok(openparts_solidcraft::Placed {
                    name: c.reference.clone(),
                    mounted,
                    matrix,
                })
            })
            .collect()
    }

    /// One artifact's bytes.
    pub fn render(&self, kind: ArtifactKind) -> Result<Vec<u8>, BuildError> {
        Ok(match kind {
            ArtifactKind::KicadSymbol => {
                openparts_kicad::render_symbol(&self.symbol()).into_bytes()
            }
            ArtifactKind::KicadFootprint => {
                openparts_kicad::render_footprint(&self.footprint()?).into_bytes()
            }
            ArtifactKind::FootprintSvg => {
                openparts_pcbcad::render_svg(&self.footprint()?).into_bytes()
            }
            ArtifactKind::SymbolSvg => {
                openparts_pcbcad::render_symbol_svg(&self.symbol()).into_bytes()
            }
            _ => self.models(&[kind])?.remove(0),
        })
    }

    /// The LibrePCB library element, embedding `step` as its 3D model.
    pub fn librepcb(&self, step: Option<&str>) -> Result<Vec<LpFile>, BuildError> {
        Ok(openparts_librepcb::generate_library_with_step(
            self.mpn(),
            &self.symbol(),
            &self.footprint()?,
            step,
        ))
    }
}

/// `a · b` for rigid row-major 3x4 transforms.
fn compose(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
    let mut out = [0.0; 12];
    for r in 0..3 {
        for c in 0..4 {
            let mut v: f64 = (0..3).map(|k| a[r * 4 + k] * b[k * 4 + c]).sum();
            if c == 3 {
                v += a[r * 4 + 3];
            }
            out[r * 4 + c] = v;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_through_their_ids() {
        for kind in ArtifactKind::ALL {
            assert_eq!(ArtifactKind::from_id(kind.id()), Some(kind));
        }
        assert!(ArtifactKind::MODELS
            .iter()
            .all(|k| k.model_format().is_some()));
    }

    #[test]
    fn builds_the_real_dataset_footprints() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../openparts-data");
        if !root.exists() {
            return;
        }
        let ds = Dataset::load(&root).unwrap();
        let part = ds
            .part("raspberrypi", "RP2040")
            .expect("RP2040 is in the dataset");
        let build = PartBuild::from_dataset(&ds, part, None).unwrap();
        assert!(build
            .render(ArtifactKind::KicadFootprint)
            .unwrap()
            .starts_with(b"(footprint"));
        assert!(build.render(ArtifactKind::KicadSymbol).unwrap().len() > 100);
    }

    #[test]
    fn rigid_transforms_compose() {
        let (sn, c) = 45f64.to_radians().sin_cos();
        let a = [c, -sn, 0.0, 1.0, sn, c, 0.0, 2.0, 0.0, 0.0, 1.0, 3.0];
        let b = [c, sn, 0.0, -1.0, -sn, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.5];
        let m = compose(&a, &b);
        // a · b = a rotation of 0 deg: identity rotation, translated.
        for (i, v) in [
            1.0,
            0.0,
            0.0,
            1.0 - c,
            0.0,
            1.0,
            0.0,
            2.0 - sn,
            0.0,
            0.0,
            1.0,
            3.5,
        ]
        .iter()
        .enumerate()
        {
            assert!((m[i] - v).abs() < 1e-12, "{i}: {} vs {v}", m[i]);
        }
    }

    /// The Raspberry Pi Pico's assembly resolves against the dataset: its
    /// RP2040, flash and regulator parts are built, the rest are boxes;
    /// the assembled model holds them all.
    #[test]
    fn a_pico_is_built_with_its_components() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../openparts-data");
        if !root.exists() {
            return;
        }
        let ds = Dataset::load(&root).unwrap();
        let Some(part) = ds.part("raspberrypi", "SC0915") else {
            return;
        };
        if part.assembly.is_empty() {
            return;
        }
        let build = PartBuild::from_dataset(&ds, part, None).unwrap();
        let assembly = build.assembly().unwrap();
        assert_eq!(assembly.len(), part.assembly.len());
        let u1 = assembly
            .iter()
            .find(|m| m.component.reference == "U1")
            .unwrap();
        assert_eq!(u1.build.as_ref().unwrap().mpn(), "RP2040");
        let placed = build.placed(assembly).unwrap();
        // U1 sits on the board's top surface (1 mm), unrotated.
        let m = placed.iter().find(|p| p.name == "U1").unwrap().matrix;
        assert!((m[11] - 1.0).abs() < 1e-9 && (m[0] - 1.0).abs() < 1e-9);
        if openparts_solidcraft::available_for_test(module_path!()) {
            let step = build.render(ArtifactKind::Step).unwrap();
            let solids = String::from_utf8(step)
                .unwrap()
                .matches("MANIFOLD_SOLID_BREP(")
                .count();
            assert!(solids > 40 + assembly.len(), "{solids} solids");
        }
    }
}
