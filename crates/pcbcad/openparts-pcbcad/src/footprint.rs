//! Footprint: Mechanical Geometry leads (or stated lands) -> pads.

use crate::*;
use openparts_mcad::MechanicalGeometry;

/// Builds a Footprint by projecting Mechanical Geometry leads onto the
/// XY plane. Lead size/position from the generator already encode the
/// correct per-side orientation, so no rotation is needed here.
///
/// Pads are the package's stated land pattern when it has one
/// (`MechanicalGeometry::lands`), else the leads' outlines.
///
/// Pad shape: SMD pads are `Rect`. A square through-hole pad marks pin 1
/// as KiCad's own footprints do (`Capacitor_THT.pretty/
/// CP_Radial_D5.0mm_P2.00mm.kicad_mod`): pin "1" `rect`, the others
/// `circle`; a longer-than-wide through-hole pad (a slotted tab) is
/// `Oval`.
///
/// The footprint carries a 3D model reference by default:
/// [`default_model_path`]`(name)` with the identity transform, i.e. the
/// STEP file that `openparts-cli generate` writes next to the
/// footprint. Use [`PcbFootprint::with_model`] / `without_model` to
/// change that.
pub fn build_footprint(name: &str, geometry: &MechanicalGeometry) -> PcbFootprint {
    use openparts_mcad::Mounting;
    // The package's stated land pattern when it has one, else the leads.
    let lands = if geometry.lands.is_empty() {
        &geometry.leads
    } else {
        &geometry.lands
    };
    let pads = lands
        .iter()
        .map(|lead| {
            let square = (lead.size.x - lead.size.y).abs() < 1e-9;
            let shape = match lead.mounting {
                Mounting::Smd => PadShape::Rect,
                Mounting::ThroughHole if lead.number == "1" && square => PadShape::Rect,
                Mounting::ThroughHole | Mounting::NonPlated if square => PadShape::Circle,
                Mounting::ThroughHole | Mounting::NonPlated => PadShape::Oval,
            };
            Pad {
                number: lead.number.clone(),
                position: Point2 {
                    x: lead.position.x,
                    y: lead.position.y,
                },
                size: (lead.size.x, lead.size.y),
                shape,
                mounting: lead.mounting,
                drill: lead.drill,
                slot: lead.slot,
            }
        })
        .collect();

    let pads: Vec<Pad> = pads;
    let graphics = crate::outline::drawings(geometry, &pads);
    PcbFootprint {
        name: name.to_string(),
        courtyard: Some(crate::outline::courtyard(geometry, &pads, &graphics)),
        graphics,
        pads,
        model: Some(Model3d::new(default_model_path(name))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footprint_pad_count_matches_lead_count() {
        use openparts_mcad::{Body, BodyShape, Lead, MechanicalGeometry, Mounting, Point3, Size3};
        let geometry = MechanicalGeometry {
            family: openparts_mcad::PackageFamily::Qfn,
            pitch: None,
            details: openparts_mcad::Details::None,
            lands: vec![],
            body: Body {
                position: Point3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                size: Size3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                shape: BodyShape::Box,
            },
            leads: vec![Lead {
                number: "1".into(),
                position: Point3 {
                    x: 1.0,
                    y: 2.0,
                    z: 0.0,
                },
                size: Size3 {
                    x: 0.3,
                    y: 0.2,
                    z: 0.1,
                },
                mounting: Mounting::Smd,
                drill: None,
                slot: None,
            }],
            markers: vec![],
            lead_form: None,
        };
        let footprint = build_footprint("EX1", &geometry);
        assert_eq!(footprint.pads.len(), 1);
        assert_eq!(footprint.pads[0].number, "1");
        assert_eq!(footprint.pads[0].position, Point2 { x: 1.0, y: 2.0 });
        assert_eq!(footprint.pads[0].size, (0.3, 0.2));
    }

    #[test]
    fn through_hole_pin_1_is_squared_and_the_rest_are_round() {
        // Matches a real KiCad reference footprint's own convention
        // (KiCad/kicad-footprints, Capacitor_THT.pretty/
        // CP_Radial_D5.0mm_P2.00mm.kicad_mod: pin 1 "rect", pin 2
        // "circle") -- applied generically to any through-hole part's
        // pin 1, not hardcoded to radial capacitors.
        use openparts_mcad::{Body, BodyShape, Lead, MechanicalGeometry, Mounting, Point3, Size3};
        let lead = |number: &str, x: f64| Lead {
            number: number.into(),
            position: Point3 { x, y: 0.0, z: 0.0 },
            size: Size3 {
                x: 1.6,
                y: 1.6,
                z: 3.0,
            },
            mounting: Mounting::ThroughHole,
            drill: Some(0.8),
            slot: None,
        };
        let geometry = MechanicalGeometry {
            family: openparts_mcad::PackageFamily::Qfn,
            pitch: None,
            details: openparts_mcad::Details::None,
            lands: vec![],
            body: Body {
                position: Point3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                size: Size3 {
                    x: 5.0,
                    y: 5.0,
                    z: 11.0,
                },
                shape: BodyShape::Cylinder,
            },
            leads: vec![lead("1", -1.0), lead("2", 1.0)],
            markers: vec![],
            lead_form: None,
        };
        let footprint = build_footprint("EX1", &geometry);
        let pad1 = footprint.pads.iter().find(|p| p.number == "1").unwrap();
        let pad2 = footprint.pads.iter().find(|p| p.number == "2").unwrap();
        assert_eq!(pad1.shape, PadShape::Rect);
        assert_eq!(pad2.shape, PadShape::Circle);
        for pad in &footprint.pads {
            assert_eq!(pad.mounting, Mounting::ThroughHole);
            assert_eq!(pad.drill, Some(0.8));
        }
    }

    #[test]
    fn smd_leads_stay_rect_regardless_of_pin_number() {
        use openparts_mcad::{Body, BodyShape, Lead, MechanicalGeometry, Mounting, Point3, Size3};
        let lead = |number: &str, x: f64| Lead {
            number: number.into(),
            position: Point3 { x, y: 0.0, z: 0.0 },
            size: Size3 {
                x: 0.5,
                y: 0.3,
                z: 0.1,
            },
            mounting: Mounting::Smd,
            drill: None,
            slot: None,
        };
        let geometry = MechanicalGeometry {
            family: openparts_mcad::PackageFamily::Qfn,
            pitch: None,
            details: openparts_mcad::Details::None,
            lands: vec![],
            body: Body {
                position: Point3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                size: Size3 {
                    x: 1.6,
                    y: 0.8,
                    z: 0.45,
                },
                shape: BodyShape::Box,
            },
            leads: vec![lead("1", -0.5), lead("2", 0.5)],
            markers: vec![],
            lead_form: None,
        };
        let footprint = build_footprint("EX1", &geometry);
        for pad in &footprint.pads {
            assert_eq!(pad.shape, PadShape::Rect);
            assert_eq!(pad.mounting, Mounting::Smd);
            assert_eq!(pad.drill, None);
        }
    }

    #[test]
    fn build_footprint_references_the_sibling_step_by_default() {
        use openparts_mcad::{Body, BodyShape, MechanicalGeometry, Point3, Size3};
        let geometry = MechanicalGeometry {
            family: openparts_mcad::PackageFamily::Qfn,
            pitch: None,
            details: openparts_mcad::Details::None,
            lands: vec![],
            body: Body {
                position: Point3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                size: Size3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                shape: BodyShape::Box,
            },
            leads: vec![],
            markers: vec![],
            lead_form: None,
        };
        let footprint = build_footprint("LPC1114FBD48/302", &geometry);
        let model = footprint.model.clone().expect("default model reference");
        assert_eq!(model.path, "LPC1114FBD48-302.step");
        assert_eq!(model.offset, [0.0; 3]);
        assert_eq!(model.scale, [1.0; 3]);
        assert_eq!(model.rotate, [0.0; 3]);

        let custom = footprint
            .clone()
            .with_model(Model3d::new("${KIPRJMOD}/x.step"));
        assert_eq!(custom.model.unwrap().path, "${KIPRJMOD}/x.step");
        assert_eq!(footprint.without_model().model, None);
    }
}
