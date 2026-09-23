use crate::device::Device;
use crate::ids::DeviceId;
use crate::package::Package;
use crate::part::Part;
use crate::pin::Pin;

/// Architecture Specification section 10: Base Device + Silicon Revision +
/// Revision Overrides + Package -> Effective Model. Generators consume
/// this, never the raw Device/Package directly.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveModel {
    pub part: Part,
    /// The base device with the selected revision's pin overrides merged
    /// in (or, if no revision was selected, identical to the base device).
    pub device: Device,
    pub package: Package,
    pub applied_revision: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum EffectiveModelError {
    #[error("device {device} does not define revision \"{revision}\"")]
    UnknownRevision { device: DeviceId, revision: String },
    #[error(
        "override for pin \"{number}\" on device {device} introduces a new pin but is missing required field \"{field}\""
    )]
    IncompleteNewPin {
        device: DeviceId,
        number: String,
        field: &'static str,
    },
}

/// Builds the Effective Model for `device` at `revision` (or the base
/// device when `revision` is `None`). `device` is taken by reference and
/// is never mutated — building an Effective Model for one revision must
/// not change another revision or the Base Definition (Testing and
/// Quality Specification section 4).
pub fn build_effective_model(
    part: Part,
    device: &Device,
    package: Package,
    revision: Option<&str>,
) -> Result<EffectiveModel, EffectiveModelError> {
    let mut effective_device = device.clone();

    if let Some(rev_id) = revision {
        let rev =
            device
                .revisions
                .get(rev_id)
                .ok_or_else(|| EffectiveModelError::UnknownRevision {
                    device: device.id.clone(),
                    revision: rev_id.to_string(),
                })?;

        for (number, pin_override) in &rev.overrides.pins {
            let merged = match effective_device.pins.get(number) {
                Some(base_pin) => base_pin.apply_override(pin_override),
                None => Pin {
                    name: pin_override.name.clone().ok_or_else(|| {
                        EffectiveModelError::IncompleteNewPin {
                            device: device.id.clone(),
                            number: number.clone(),
                            field: "name",
                        }
                    })?,
                    pin_type: pin_override.pin_type.ok_or_else(|| {
                        EffectiveModelError::IncompleteNewPin {
                            device: device.id.clone(),
                            number: number.clone(),
                            field: "type",
                        }
                    })?,
                    alternate_functions: pin_override
                        .alternate_functions
                        .clone()
                        .unwrap_or_default(),
                },
            };
            effective_device.pins.insert(number.clone(), merged);
        }
    }

    Ok(EffectiveModel {
        part,
        device: effective_device,
        package,
        applied_revision: revision.map(|s| s.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::existence::{Existence, ExistenceStatus};
    use crate::ids::{ManufacturerId, PackageId, PartId};
    use crate::kind::Kind;
    use crate::lifecycle::{Lifecycle, LifecycleStatus};
    use crate::package::{Package, PackageDimensions};
    use crate::pin::{Pin, PinOverride, PinType};
    use std::collections::BTreeMap;

    fn sample_part() -> Part {
        Part {
            schema_version: "0.1".into(),
            kind: Kind::Part,
            id: PartId::from("ex/EX1"),
            manufacturer: ManufacturerId::from("ex"),
            mpn: "EX1".into(),
            device: DeviceId::from("ex/DEV1"),
            package: PackageId::from("standards/PKG1"),
            existence: Existence {
                status: ExistenceStatus::Unverified,
            },
            lifecycle: Lifecycle {
                status: LifecycleStatus::Active,
                replacement: vec![],
            },
            sources: vec![],
            provenance: BTreeMap::new(),
        }
    }

    fn sample_package() -> Package {
        use crate::dimension::Dimension;
        Package {
            schema_version: "0.1".into(),
            kind: Kind::Package,
            id: PackageId::from("standards/PKG1"),
            family: "lqfp".into(),
            lead_count: 4,
            pitch: Dimension {
                nominal: Some(0.5),
                min: None,
                max: None,
                unit: "mm".into(),
            },
            dimensions: PackageDimensions {
                body_width: Dimension {
                    nominal: Some(5.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_length: Dimension {
                    nominal: Some(5.0),
                    min: None,
                    max: None,
                    unit: "mm".into(),
                },
                body_height: None,
                exposed_pad: None,
            },
            lead_layout: None,
            geometry: None,
            provenance: BTreeMap::new(),
        }
    }

    fn sample_device() -> Device {
        let mut pins = BTreeMap::new();
        pins.insert(
            "1".to_string(),
            Pin {
                name: "VSS".into(),
                pin_type: PinType::Ground,
                alternate_functions: vec![],
            },
        );
        pins.insert(
            "42".to_string(),
            Pin {
                name: "VSS".into(),
                pin_type: PinType::Ground,
                alternate_functions: vec![],
            },
        );

        let mut overrides_pins = BTreeMap::new();
        overrides_pins.insert(
            "42".to_string(),
            PinOverride {
                name: Some("PDR_ON".into()),
                pin_type: None,
                alternate_functions: None,
            },
        );
        let mut revisions = BTreeMap::new();
        revisions.insert(
            "rev-b".to_string(),
            crate::device::DeviceRevision {
                manufacturer_revision: "B".into(),
                detection: vec![],
                overrides: crate::device::DeviceOverrides {
                    pins: overrides_pins,
                },
            },
        );

        Device {
            schema_version: "0.1".into(),
            kind: Kind::Device,
            id: DeviceId::from("ex/DEV1"),
            manufacturer: ManufacturerId::from("ex"),
            family: None,
            pins,
            revisions,
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn no_revision_keeps_base_pins() {
        let device = sample_device();
        let model = build_effective_model(sample_part(), &device, sample_package(), None).unwrap();
        assert_eq!(model.device.pins["42"].name, "VSS");
        assert_eq!(model.applied_revision, None);
    }

    #[test]
    fn revision_override_applies_without_mutating_base() {
        let device = sample_device();
        let before = device.clone();

        let model =
            build_effective_model(sample_part(), &device, sample_package(), Some("rev-b")).unwrap();

        assert_eq!(model.device.pins["42"].name, "PDR_ON");
        // Unrelated pin and unrelated field are untouched.
        assert_eq!(model.device.pins["1"].name, "VSS");
        assert_eq!(model.device.pins["42"].pin_type, PinType::Ground);

        // The base Device passed in by reference must be unchanged.
        assert_eq!(device, before);
        assert_eq!(device.pins["42"].name, "VSS");
    }

    #[test]
    fn unknown_revision_is_rejected() {
        let device = sample_device();
        let err = build_effective_model(sample_part(), &device, sample_package(), Some("rev-z"))
            .unwrap_err();
        assert!(matches!(err, EffectiveModelError::UnknownRevision { .. }));
    }
}
