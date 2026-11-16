//! Footprint: the Placement Model's pads and courtyard, plus drawn
//! fabrication and silkscreen layers.

use crate::*;
use openparts_mcad::MechanicalGeometry;
use openparts_placement::{Density, PadKind, PlacementModel};

/// Builds a Footprint from a geometry alone: its Placement Model
/// (`openparts_placement::from_geometry`, density level N) through
/// [`footprint_from_placement`], without a placement hash (that belongs
/// to a part's full model: `openparts_placement::build`).
pub fn build_footprint(name: &str, geometry: &MechanicalGeometry) -> PcbFootprint {
    let model = openparts_placement::from_geometry(geometry, Density::N);
    let mut fp = footprint_from_placement(name, &model);
    fp.placement_hash = None;
    fp
}

/// Builds a Footprint from a Placement Model: its pads (in the model's
/// frame: land-pattern centroid origin, IPC-7351B Level A orientation),
/// its courtyard, fabrication and silkscreen drawings, the model's
/// hash, and a 3D model reference ([`default_model_path`]`(name)`, the
/// STEP that `openparts-cli generate` writes next to the footprint)
/// carrying the model's 3D alignment. Use [`PcbFootprint::with_model`] /
/// `without_model` to change that.
///
/// Pad shape (the model's): SMD pads are `Rect`. A square through-hole
/// pad marks pin 1 as KiCad's own footprints do (`Capacitor_THT.pretty/
/// CP_Radial_D5.0mm_P2.00mm.kicad_mod`): pin "1" `rect`, the others
/// `circle`; a longer-than-wide through-hole pad (a slotted tab) is
/// `Oval`. A shape the source states (round, chamfered, polygon) is kept.
pub fn footprint_from_placement(name: &str, model: &PlacementModel) -> PcbFootprint {
    use openparts_mcad::Mounting;
    let pads: Vec<Pad> = model
        .pads
        .iter()
        .map(|p| Pad {
            number: p.number.clone(),
            position: Point2 {
                x: p.position.x,
                y: p.position.y,
            },
            size: (p.size.x, p.size.y),
            shape: match (p.shape, &p.chamfer, &p.polygon) {
                (openparts_placement::PadShape::Circle, _, _) => PadShape::Circle,
                (openparts_placement::PadShape::Oval, _, _) => PadShape::Oval,
                (openparts_placement::PadShape::Chamfered, Some(c), _) => PadShape::Chamfered {
                    size: c.size,
                    corners: c.corners.clone(),
                },
                (openparts_placement::PadShape::Polygon, _, Some(points)) => {
                    PadShape::Polygon(points.iter().map(|q| Point2 { x: q.x, y: q.y }).collect())
                }
                // The generators draw no rounded pads yet.
                _ => PadShape::Rect,
            },
            mounting: match p.kind {
                PadKind::Smd => Mounting::Smd,
                PadKind::Tht => Mounting::ThroughHole,
                PadKind::Npth => Mounting::NonPlated,
            },
            drill: p.drill,
            slot: p.slot,
        })
        .collect();
    let graphics = crate::outline::drawings(model, &pads);
    let courtyard = model.courtyard.polygons.first().map(|poly| Courtyard {
        outline: poly.iter().map(|q| Point2 { x: q.x, y: q.y }).collect(),
    });
    let m3 = &model.model3d;
    PcbFootprint {
        name: name.to_string(),
        courtyard,
        graphics,
        pads,
        model: Some(Model3d {
            offset: m3.offset,
            rotate: [0.0, 0.0, m3.rotation],
            ..Model3d::new(default_model_path(name))
        }),
        placement_hash: Some(model.hash.clone()),
    }
}

/// Pads for device pins that share another pin's contact: a pin with
/// no pad of its own whose `connected_to` pin has one (USB-C "A1 B12",
/// one contact numbered twice) gets a copy of that pad under its own
/// number, as KiCad's own footprints overlay them.
pub fn add_shared_pads(footprint: &mut PcbFootprint, device: &openparts_core::Device) {
    let mut extra = Vec::new();
    for (number, pin) in &device.pins {
        if footprint.pads.iter().any(|p| &p.number == number) {
            continue;
        }
        let Some(target) = &pin.connected_to else {
            continue;
        };
        extra.extend(
            footprint
                .pads
                .iter()
                .filter(|p| &p.number == target)
                .map(|p| Pad {
                    number: number.clone(),
                    ..p.clone()
                }),
        );
    }
    footprint.pads.extend(extra);
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
                shape: openparts_mcad::LandShape::Rect,
            }],
            markers: vec![],
            lead_form: None,
        };
        let footprint = build_footprint("EX1", &geometry);
        assert_eq!(footprint.pads.len(), 1);
        assert_eq!(footprint.pads[0].number, "1");
        // The origin is the land pattern's centroid: the one pad.
        assert_eq!(footprint.pads[0].position, Point2 { x: 0.0, y: 0.0 });
        // The 3D model, drawn in the generator's frame, moves with it.
        assert_eq!(footprint.model.as_ref().unwrap().offset, [-1.0, -2.0, 0.0]);
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
            shape: openparts_mcad::LandShape::Rect,
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
            shape: openparts_mcad::LandShape::Rect,
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
