//! openparts-core: the Canonical Rust Data Model.
//!
//! Architecture Specification section 7.1: this crate must not depend on
//! YAML, HTTP, PostgreSQL, KiCad, STEP, glTF, or any server implementation.
//! It only knows about Rust types and the format-agnostic `serde` traits.

pub mod device;
pub mod dimension;
pub mod effective_model;
pub mod erratum;
pub mod existence;
pub mod ids;
pub mod kind;
pub mod lifecycle;
pub mod manufacturer;
pub mod package;
pub mod part;
pub mod pin;
pub mod provenance;
pub mod rejected;
pub mod serde_util;
pub mod source;

pub use device::{Device, DeviceOverrides, DeviceRevision, RevisionDetection};
pub use dimension::Dimension;
pub use effective_model::{build_effective_model, EffectiveModel, EffectiveModelError};
pub use erratum::{Applicability, Erratum, ErratumCategory};
pub use existence::{Existence, ExistenceStatus};
pub use ids::{DeviceId, ManufacturerId, PackageId, PartId, SourceId};
pub use kind::Kind;
pub use lifecycle::{Lifecycle, LifecycleStatus};
pub use manufacturer::{Manufacturer, Website};
pub use package::{GeometryType, Package, PackageDimensions, PackageGeometrySpec};
pub use part::Part;
pub use pin::{Pin, PinOverride, PinType};
pub use provenance::{ProvenanceEntry, ProvenanceMap};
pub use rejected::Rejected;
pub use source::{DocumentType, Source};
