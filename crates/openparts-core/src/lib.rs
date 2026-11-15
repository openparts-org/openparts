//! openparts-core: the Canonical Rust Data Model.
//!
//! Architecture Specification section 7.1: this crate must not depend on
//! YAML, HTTP, PostgreSQL, KiCad, STEP, glTF, or any server implementation.
//! It only knows about Rust types and the format-agnostic `serde` traits.

pub mod category;
pub mod circuit;
pub mod condition;
pub mod device;
pub mod dimension;
pub mod effective_model;
pub mod erratum;
pub mod existence;
pub mod family;
pub mod ids;
pub mod kind;
pub mod lifecycle;
pub mod manufacturer;
pub mod package;
pub mod parameter;
pub mod part;
pub mod pin;
pub mod provenance;
pub mod rejected;
pub mod review;
pub mod serde_util;
pub mod source;
pub mod structure;
pub mod text;

pub use category::ComponentCategory;
pub use circuit::{
    Categories, Circuit, CircuitHost, CircuitNet, Connect, Example, Formula, FormulaInput,
    HostMatch, Include, IncludeHost, Level, LevelValue, NetKind, Place, PlacementHint, Requirement,
    Role, RoleParam, RoutingExample, RoutingHint,
};
pub use condition::{
    condition_symbol, conditions_label, Condition, ConditionError, ConditionSymbol, Conditions,
    CONDITION_SYMBOLS,
};
pub use device::{Device, DeviceOverrides, DeviceRevision, RevisionDetection};
pub use dimension::Dimension;
pub use effective_model::{build_effective_model, EffectiveModel, EffectiveModelError};
pub use erratum::{Applicability, Erratum, ErratumCategory};
pub use existence::{Existence, ExistenceStatus};
pub use family::PackageFamily;
pub use ids::{DeviceId, ManufacturerId, PackageId, PartId, SourceId};
pub use kind::Kind;
pub use lifecycle::{Lifecycle, LifecycleStatus};
pub use manufacturer::{Manufacturer, Website};
pub use package::{
    EdgeRequirement, ExposedPadDimensions, GeometryType, KeepoutPurpose, KeepoutRestriction,
    Package, PackageConstruction, PackageDimensions, PackageEdge, PackageGeometrySpec,
    PackageKeepout, PackagePad, PackagePadGrid, PackagePadRow, PackageRect, PackageSide,
    PackageSmt, PackageTab,
};
pub use parameter::{
    effective_parameters, format_value, parameter_spec, parse_value, Normalized, ParameterError,
    ParameterShape, ParameterSpec, ParameterValue, ParameterValues, Parameters, Quantity,
    PARAMETERS,
};
pub use part::Part;
pub use pin::{DiffPair, DiffPolarity, DiffStandard, Pin, PinOverride, PinType};
pub use provenance::{ProvenanceEntry, ProvenanceMap};
pub use rejected::Rejected;
pub use review::{DocumentReview, ReviewStatus};
pub use source::{DocumentType, Source};
pub use structure::{Gender, Orientation, PackageStructure};
