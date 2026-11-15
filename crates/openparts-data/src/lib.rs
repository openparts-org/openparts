//! openparts-data: reads and writes Canonical YAML into `openparts-core`
//! types (Architecture Specification section 7.2). Does not define its
//! own meaning for the data — it defers entirely to the Canonical Data
//! Specification and to `openparts-core`'s types.
//!
//! The `load_*` functions read one file; [`Dataset::load`] reads a whole
//! `openparts-data` tree. [`format`] is the file format: it reads schema
//! 0.2 (and 0.1 during the transition) and writes 0.2.

mod dataset;
pub mod equiv;
mod error;
pub mod format;
pub mod migrate;

pub use dataset::{yaml_files, Dataset, LoadIssue, ResolveError};
pub use error::DataError;

use openparts_core::{Circuit, Device, Kind, Manufacturer, Package, Part, Source};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;

/// A document of type `T` from a file's text (any schema version).
pub fn parse_document<T: DeserializeOwned>(path: &Path, text: &str) -> Result<T, DataError> {
    let yaml = |source| DataError::Yaml {
        path: path.display().to_string(),
        source,
    };
    let raw: serde_yaml::Value = serde_yaml::from_str(text).map_err(yaml)?;
    let doc = format::decode(raw).map_err(|message| DataError::Format {
        path: path.display().to_string(),
        message,
    })?;
    serde_yaml::from_value(doc).map_err(yaml)
}

fn load_yaml<T: DeserializeOwned>(path: &Path) -> Result<T, DataError> {
    let text = std::fs::read_to_string(path).map_err(|source| DataError::Io {
        path: path.display().to_string(),
        source,
    })?;
    parse_document(path, &text)
}

/// Writes `value` as a schema 0.2 file.
fn save_yaml<T: Serialize>(value: &T, path: &Path) -> Result<(), DataError> {
    let doc = serde_yaml::to_value(value).map_err(|source| DataError::Yaml {
        path: path.display().to_string(),
        source,
    })?;
    let text =
        format::write(&doc, &format::Comments::default()).map_err(|message| DataError::Format {
            path: path.display().to_string(),
            message,
        })?;
    std::fs::write(path, text).map_err(|source| DataError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn check_kind(path: &Path, expected: Kind, found: Kind) -> Result<(), DataError> {
    if found != expected {
        return Err(DataError::KindMismatch {
            path: path.display().to_string(),
            expected,
            found,
        });
    }
    Ok(())
}

pub fn load_manufacturer<P: AsRef<Path>>(path: P) -> Result<Manufacturer, DataError> {
    let path = path.as_ref();
    let doc: Manufacturer = load_yaml(path)?;
    check_kind(path, Kind::Manufacturer, doc.kind)?;
    Ok(doc)
}

pub fn save_manufacturer<P: AsRef<Path>>(doc: &Manufacturer, path: P) -> Result<(), DataError> {
    save_yaml(doc, path.as_ref())
}

pub fn load_part<P: AsRef<Path>>(path: P) -> Result<Part, DataError> {
    let path = path.as_ref();
    let doc: Part = load_yaml(path)?;
    check_kind(path, Kind::Part, doc.kind)?;
    Ok(doc)
}

pub fn save_part<P: AsRef<Path>>(doc: &Part, path: P) -> Result<(), DataError> {
    save_yaml(doc, path.as_ref())
}

pub fn load_device<P: AsRef<Path>>(path: P) -> Result<Device, DataError> {
    let path = path.as_ref();
    let doc: Device = load_yaml(path)?;
    check_kind(path, Kind::Device, doc.kind)?;
    Ok(doc)
}

pub fn save_device<P: AsRef<Path>>(doc: &Device, path: P) -> Result<(), DataError> {
    save_yaml(doc, path.as_ref())
}

pub fn load_package<P: AsRef<Path>>(path: P) -> Result<Package, DataError> {
    let path = path.as_ref();
    let doc: Package = load_yaml(path)?;
    check_kind(path, Kind::Package, doc.kind)?;
    Ok(doc)
}

pub fn save_package<P: AsRef<Path>>(doc: &Package, path: P) -> Result<(), DataError> {
    save_yaml(doc, path.as_ref())
}

pub fn load_source<P: AsRef<Path>>(path: P) -> Result<Source, DataError> {
    let path = path.as_ref();
    let doc: Source = load_yaml(path)?;
    check_kind(path, Kind::Source, doc.kind)?;
    Ok(doc)
}

pub fn save_source<P: AsRef<Path>>(doc: &Source, path: P) -> Result<(), DataError> {
    save_yaml(doc, path.as_ref())
}

pub fn load_circuit<P: AsRef<Path>>(path: P) -> Result<Circuit, DataError> {
    let path = path.as_ref();
    let doc: Circuit = load_yaml(path)?;
    check_kind(path, Kind::Circuit, doc.kind)?;
    Ok(doc)
}

pub fn save_circuit<P: AsRef<Path>>(doc: &Circuit, path: P) -> Result<(), DataError> {
    save_yaml(doc, path.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_core::{ExistenceStatus, LifecycleStatus};

    #[test]
    fn part_round_trips_through_yaml() {
        let dir = std::env::temp_dir().join(format!("openparts-data-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("part.yaml");

        let yaml = r#"
schema_version: "0.1"
kind: part
id: ex/EX1
manufacturer: ex
mpn: EX1
device: ex/DEV1
package: standards/PKG1
existence:
  status: unverified
lifecycle:
  status: active
sources: []
"#;
        std::fs::write(&path, yaml).unwrap();

        let part = load_part(&path).unwrap();
        assert_eq!(part.mpn, "EX1");
        assert_eq!(part.existence.status, ExistenceStatus::Unverified);
        assert_eq!(part.lifecycle.status, LifecycleStatus::Active);

        let path2 = dir.join("part-roundtrip.yaml");
        save_part(&part, &path2).unwrap();
        let reloaded = load_part(&path2).unwrap();
        assert_eq!(reloaded.schema_version, "0.2");
        assert_eq!(
            Part {
                schema_version: "0.2".into(),
                ..part
            },
            reloaded
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn wrong_kind_is_rejected() {
        let dir =
            std::env::temp_dir().join(format!("openparts-data-test-kind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("part.yaml");

        // This document declares kind: device but is loaded as a Part.
        let yaml = r#"
schema_version: "0.1"
kind: device
id: ex/EX1
manufacturer: ex
mpn: EX1
device: ex/DEV1
package: standards/PKG1
existence:
  status: unverified
lifecycle:
  status: active
"#;
        std::fs::write(&path, yaml).unwrap();

        let err = load_part(&path).unwrap_err();
        assert!(matches!(err, DataError::KindMismatch { .. }));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn duplicate_pin_key_is_rejected() {
        let dir =
            std::env::temp_dir().join(format!("openparts-data-test-dup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("device.yaml");

        let yaml = r#"
schema_version: "0.1"
kind: device
id: ex/DEV1
manufacturer: ex
category: other
pins:
  "1":
    name: VBAT
    type: power
  "1":
    name: DUPLICATE
    type: power
"#;
        std::fs::write(&path, yaml).unwrap();

        let err = load_device(&path).unwrap_err();
        assert!(matches!(err, DataError::Yaml { .. }));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn non_numeric_pin_keys_are_preserved_as_strings() {
        let dir = std::env::temp_dir().join(format!(
            "openparts-data-test-pinkeys-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("device.yaml");

        let yaml = r#"
schema_version: "0.1"
kind: device
id: ex/DEV1
manufacturer: ex
category: other
pins:
  "A1":
    name: VDD
    type: power
  "EP":
    name: GND
    type: ground
"#;
        std::fs::write(&path, yaml).unwrap();

        let device = load_device(&path).unwrap();
        assert!(device.pins.contains_key("A1"));
        assert!(device.pins.contains_key("EP"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn package_lead_dimensions_and_construction_are_optional_and_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "openparts-data-test-pkgdims-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("old.yaml");
        let new = dir.join("new.yaml");

        let body = r#"
schema_version: "0.1"
kind: package
id: standards/PKG1
family: lqfp
lead_count: 48
pitch:
  nominal: 0.5
  unit: mm
dimensions:
  body_width:
    nominal: 7.0
    unit: mm
  body_length:
    nominal: 7.0
    unit: mm
"#;
        std::fs::write(&old, body).unwrap();
        let package = load_package(&old).unwrap();
        assert!(package.dimensions.overall_width.is_none());
        assert!(package.construction.is_none());

        let extended = body.replace(
            "dimensions:\n",
            "construction: molded\ndimensions:\n  overall_width:\n    nominal: 9.0\n    unit: mm\n  lead_thickness:\n    min: 0.09\n    max: 0.2\n    unit: mm\n",
        );
        std::fs::write(&new, extended).unwrap();
        let package = load_package(&new).unwrap();
        assert_eq!(
            package.construction,
            Some(openparts_core::PackageConstruction::Molded)
        );
        assert_eq!(
            package.dimensions.overall_width.as_ref().unwrap().typ,
            Some(9.0)
        );
        let thickness = package.dimensions.lead_thickness.as_ref().unwrap();
        assert_eq!((thickness.typ, thickness.max), (None, Some(0.2)));

        save_package(&package, &new).unwrap();
        let mut package = package;
        package.schema_version = "0.2".into();
        assert_eq!(load_package(&new).unwrap(), package);

        std::fs::remove_dir_all(&dir).ok();
    }
}
