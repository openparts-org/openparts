//! openparts-data: reads and writes Canonical YAML into `openparts-core`
//! types (Architecture Specification section 7.2). Does not define its
//! own meaning for the data — it defers entirely to the Canonical Data
//! Specification and to `openparts-core`'s types.
//!
//! Loaders take a direct file path; scanning `openparts-data`'s directory
//! layout for a given ID is out of scope for the first vertical slice.

mod error;

pub use error::DataError;

use openparts_core::{Device, Kind, Manufacturer, Package, Part, Source};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;

fn load_yaml<T: DeserializeOwned>(path: &Path) -> Result<T, DataError> {
    let text = std::fs::read_to_string(path).map_err(|source| DataError::Io {
        path: path.display().to_string(),
        source,
    })?;
    serde_yaml::from_str(&text).map_err(|source| DataError::Yaml {
        path: path.display().to_string(),
        source,
    })
}

fn save_yaml<T: Serialize>(value: &T, path: &Path) -> Result<(), DataError> {
    let text = serde_yaml::to_string(value).map_err(|source| DataError::Yaml {
        path: path.display().to_string(),
        source,
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
        assert_eq!(part, reloaded);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn wrong_kind_is_rejected() {
        let dir = std::env::temp_dir().join(format!("openparts-data-test-kind-{}", std::process::id()));
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
        let dir = std::env::temp_dir().join(format!("openparts-data-test-dup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("device.yaml");

        let yaml = r#"
schema_version: "0.1"
kind: device
id: ex/DEV1
manufacturer: ex
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
        let dir = std::env::temp_dir().join(format!("openparts-data-test-pinkeys-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("device.yaml");

        let yaml = r#"
schema_version: "0.1"
kind: device
id: ex/DEV1
manufacturer: ex
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
}
