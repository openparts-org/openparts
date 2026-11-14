//! The generation pipeline (Architecture Specification section 51):
//! Part resolution -> validation -> Effective Model -> geometry -> every
//! artifact. `openparts generate`, `openparts bundle` and
//! openparts-server all build artifacts through this crate, so they
//! cannot drift apart.

pub mod bundle;

use openparts_core::{Device, EffectiveModel, Package, Part, SourceId};
use openparts_data::Dataset;
use openparts_mcad::MechanicalGeometry;
use openparts_pcbcad::{PcbFootprint, PcbSymbol};
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
    Step,
    Stl,
    ThreeMf,
    /// The editable SolidCraft design (`.sc`).
    SolidCraft,
}

impl ArtifactKind {
    pub const ALL: [ArtifactKind; 7] = [
        ArtifactKind::KicadSymbol,
        ArtifactKind::KicadFootprint,
        ArtifactKind::FootprintSvg,
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
            ArtifactKind::Step => "step",
            ArtifactKind::Stl => "stl",
            ArtifactKind::ThreeMf => "3mf",
            ArtifactKind::SolidCraft => "sc",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            ArtifactKind::KicadSymbol | ArtifactKind::KicadFootprint => "text/plain; charset=utf-8",
            ArtifactKind::FootprintSvg => "image/svg+xml",
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
        self == ArtifactKind::KicadSymbol
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
}

impl Stage {
    pub fn id(self) -> &'static str {
        match self {
            Stage::Resolve => "resolve",
            Stage::Validation => "validation",
            Stage::EffectiveModel => "effective-model",
            Stage::Geometry => "geometry",
            Stage::Model3d => "3d-model",
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
}

impl BuildError {
    pub fn stage(&self) -> Stage {
        match self {
            BuildError::Resolve(_) => Stage::Resolve,
            BuildError::Validation(_) => Stage::Validation,
            BuildError::EffectiveModel(_) => Stage::EffectiveModel,
            BuildError::Geometry(_) => Stage::Geometry,
            BuildError::Model3d(_) => Stage::Model3d,
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
    geometry: Result<MechanicalGeometry, openparts_mcad::McadError>,
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
        Ok(PartBuild { model, geometry })
    }

    /// [`PartBuild::new`] for a part of `dataset`, validated against the
    /// dataset's sources.
    pub fn from_dataset(
        dataset: &Dataset,
        part: &Part,
        revision: Option<&str>,
    ) -> Result<PartBuild, BuildError> {
        let (device, package) = dataset.resolve(part)?;
        PartBuild::new(part, device, package, &dataset.source_ids(), revision)
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
        openparts_pcbcad::build_symbol(self.mpn(), &self.model.device)
    }

    pub fn geometry(&self) -> Result<&MechanicalGeometry, BuildError> {
        self.geometry
            .as_ref()
            .map_err(|e| BuildError::Geometry(e.clone()))
    }

    /// The footprint, with overlaid pads for pins that share a contact
    /// (`connected_to`).
    pub fn footprint(&self) -> Result<PcbFootprint, BuildError> {
        let mut fp = openparts_pcbcad::build_footprint(self.mpn(), self.geometry()?);
        openparts_pcbcad::add_shared_pads(&mut fp, &self.model.device);
        Ok(fp)
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
    /// from one SolidCraft run.
    pub fn models(&self, kinds: &[ArtifactKind]) -> Result<Vec<Vec<u8>>, BuildError> {
        let formats: Vec<Format> = kinds
            .iter()
            .map(|k| k.model_format().expect("models() takes 3D artifact kinds"))
            .collect();
        Ok(openparts_solidcraft::generate(
            &self.model_input()?,
            self.mpn(),
            &formats,
        )?)
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
}
