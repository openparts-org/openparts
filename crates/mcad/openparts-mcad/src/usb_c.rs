//! USB Type-C receptacles (`usb_c`), from an explicit land pattern: every
//! pad -- SMD contacts and the shell's plated (often slotted) tabs -- is
//! listed in `Package::pads` as the source's recommended PCB layout
//! gives it.
//!
//! Frame: top view, X right, Y up, the origin on the receptacle's centre
//! line (USB-C layouts are symmetric about it), the mating opening
//! facing +Y. `body_width` is the shell across the opening (X),
//! `body_length` its depth (Y), `body_height` its height;
//! `body_offset` is the shell centre's Y in the pads' frame (required:
//! where the shell sits over its pads), `standoff` lifts it (else on the
//! board). `lead_count` is the number of pads (tabs included).

use crate::defaults::{self, value};
use crate::{Body, BodyShape, Lead, McadError, MechanicalGeometry, Mounting, Point3, Size3};
use openparts_core::Package;

pub fn generate_usb_c(package: &Package) -> Result<MechanicalGeometry, McadError> {
    if package.pads.is_empty() {
        return Err(McadError::MissingDimension("pads"));
    }
    if package.pads.len() != package.lead_count as usize {
        return Err(McadError::InvalidLeadCount(package.lead_count));
    }
    let d = &package.dimensions;
    let body_w = defaults::required(&d.body_width)
        .ok_or(McadError::MissingDimension("dimensions.body_width"))?;
    let body_l = defaults::required(&d.body_length)
        .ok_or(McadError::MissingDimension("dimensions.body_length"))?;
    let body_h = defaults::stated(&d.body_height)
        .ok_or(McadError::MissingDimension("dimensions.body_height"))?;
    let offset =
        value(&d.body_offset).ok_or(McadError::MissingDimension("dimensions.body_offset"))?;
    let standoff = value(&d.standoff).unwrap_or(0.0);

    let mut leads = Vec::with_capacity(package.pads.len());
    for p in &package.pads {
        if p.width <= 0.0 || p.length <= 0.0 {
            return Err(McadError::InconsistentDimensions(format!(
                "pad {} has a non-positive size",
                p.number
            )));
        }
        if let Some(drill) = p.drill {
            if drill >= p.width.min(p.length) && p.drill_length.is_none() {
                return Err(McadError::InconsistentDimensions(format!(
                    "pad {}: drill {drill} leaves no copper",
                    p.number
                )));
            }
            if p.drill_length
                .is_some_and(|l| l < drill || l >= p.width.max(p.length))
            {
                return Err(McadError::InconsistentDimensions(format!(
                    "pad {}: slot length must lie between the drill and the pad length",
                    p.number
                )));
            }
        } else if p.drill_length.is_some() {
            return Err(McadError::InconsistentDimensions(format!(
                "pad {}: drill_length without a drill",
                p.number
            )));
        }
        let tht = p.drill.is_some();
        leads.push(Lead {
            number: p.number.clone(),
            position: Point3 {
                x: p.x,
                y: p.y,
                z: if tht {
                    0.0
                } else {
                    crate::defaults::usb_c::PAD_T / 2.0
                },
            },
            size: Size3 {
                x: p.width,
                y: p.length,
                z: if tht {
                    standoff
                } else {
                    crate::defaults::usb_c::PAD_T
                },
            },
            mounting: if tht {
                Mounting::ThroughHole
            } else {
                Mounting::Smd
            },
            drill: p.drill,
            slot: p.drill_length,
        });
    }

    Ok(MechanicalGeometry {
        family: package.family,

        pitch: crate::defaults::required(&package.pitch),

        details: crate::Details::UsbC(usb_c_features(package)),
        body: Body {
            position: Point3 {
                x: 0.0,
                y: offset,
                z: standoff + body_h / 2.0,
            },
            size: Size3 {
                x: body_w,
                y: body_l,
                z: body_h,
            },
            shape: BodyShape::Box,
        },
        leads,
        markers: vec![],
        lead_form: None,
    })
}

/// Drawing values for a receptacle's 3D model: shell sheet thickness
/// 0.2 mm and shell tabs reaching `tail_length` (else 1.0 mm) below the
/// board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsbCFeatures {
    pub shell_thickness: f64,
    pub tail_length: f64,
}

pub fn usb_c_features(package: &Package) -> UsbCFeatures {
    UsbCFeatures {
        shell_thickness: crate::defaults::usb_c::SHELL_THICKNESS,
        tail_length: value(&package.dimensions.tail_length).unwrap_or(crate::defaults::usb_c::TAIL_LENGTH),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb() -> Package {
        serde_yaml::from_str(
            "schema_version: '0.1'\nkind: package\nid: standards/TEST\nfamily: usb_c\n\
             lead_count: 3\npitch: {unit: mm}\ndimensions:\n\
             \x20 body_width: {nominal: 8.94, unit: mm}\n\
             \x20 body_length: {nominal: 7.35, unit: mm}\n\
             \x20 body_height: {nominal: 3.16, unit: mm}\n\
             \x20 body_offset: {nominal: -1.0, unit: mm}\n\
             pads:\n\
             \x20 - {number: A5, x: -0.25, y: 3.0, width: 0.3, length: 1.0}\n\
             \x20 - {number: B5, x: 0.25, y: 3.0, width: 0.3, length: 1.0}\n\
             \x20 - {number: S1, x: -4.3, y: 2.6, width: 1.0, length: 2.1, drill: 0.6, drill_length: 1.7}\n",
        )
        .unwrap()
    }

    #[test]
    fn pads_come_straight_from_the_land_pattern() {
        let g = generate_usb_c(&usb()).unwrap();
        assert_eq!(g.leads.len(), 3);
        assert_eq!(g.leads[0].mounting, Mounting::Smd);
        let s1 = &g.leads[2];
        assert_eq!(s1.mounting, Mounting::ThroughHole);
        assert_eq!((s1.drill, s1.slot), (Some(0.6), Some(1.7)));
        assert!((g.body.position.y + 1.0).abs() < 1e-9);
    }

    #[test]
    fn bad_slots_and_counts_are_errors() {
        let mut p = usb();
        p.pads[2].drill_length = Some(2.5);
        assert!(generate_usb_c(&p).is_err());
        let mut p = usb();
        p.lead_count = 4;
        assert!(generate_usb_c(&p).is_err());
        let mut p = usb();
        p.dimensions.body_offset = None;
        assert!(generate_usb_c(&p).is_err());
    }
}
