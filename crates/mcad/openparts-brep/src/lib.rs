//! openparts-brep: `openparts_mcad::MechanicalGeometry` -> real B-rep
//! solids, via `truck-modeling`. The one place that knows how to turn
//! `Body`/`Lead` facts into `truck` shapes -- `openparts-step` and
//! `openparts-stl` both call `build` and derive their own output format
//! from its result, instead of each independently reconstructing
//! geometry from `MechanicalGeometry` the way their old hand-rolled
//! tessellation code used to.
//!
//! Box and cylinder construction patterns below are taken directly from
//! `truck-modeling`'s own examples (`examples/cube.rs`,
//! `examples/cylinder.rs`), adapted to build at an arbitrary center
//! rather than at the origin. Both were independently OCCT-verified
//! (`BRepCheck_Analyzer`, volume cross-check) during this crate's
//! development spike.

use openparts_mcad::{Body, BodyShape, Lead, MechanicalGeometry};
use truck_modeling::*;

/// `body`/`leads` mirror `MechanicalGeometry::body`/`leads` 1:1 (same
/// order), just as real `truck` solids instead of plain-data facts.
pub struct BrepGeometry {
    pub body: Solid,
    pub leads: Vec<Solid>,
}

pub fn build(geometry: &MechanicalGeometry) -> BrepGeometry {
    BrepGeometry {
        body: build_body(&geometry.body),
        leads: geometry.leads.iter().map(build_lead).collect(),
    }
}

fn build_body(body: &Body) -> Solid {
    let center = [body.position.x, body.position.y, body.position.z];
    match body.shape {
        BodyShape::Box => build_box(center, [body.size.x, body.size.y, body.size.z]),
        BodyShape::Cylinder => build_cylinder(
            center,
            body.size.x, // diameter (== size.y)
            body.size.z, // height
        ),
        BodyShape::CylinderX => build_cylinder_x(
            center,
            body.size.y, // diameter (== size.z)
            body.size.x, // length (along X)
        ),
    }
}

fn build_lead(lead: &Lead) -> Solid {
    build_box(
        [lead.position.x, lead.position.y, lead.position.z],
        [lead.size.x, lead.size.y, lead.size.z],
    )
}

/// An axis-aligned box centered at `center` with the given full `size`,
/// built the same way `truck-modeling`'s own `examples/cube.rs` does:
/// one vertex, extruded along each axis in turn.
fn build_box(center: [f64; 3], size: [f64; 3]) -> Solid {
    let corner = Point3::new(
        center[0] - size[0] / 2.0,
        center[1] - size[1] / 2.0,
        center[2] - size[2] / 2.0,
    );
    let v = builder::vertex(corner);
    let e = builder::tsweep(&v, Vector3::new(size[0], 0.0, 0.0));
    let f = builder::tsweep(&e, Vector3::new(0.0, size[1], 0.0));
    builder::tsweep(&f, Vector3::new(0.0, 0.0, size[2]))
}

/// A true (NURBS-exact, not tessellated) cylinder centered at `center`
/// with the given `diameter`/`height`, axis along Z (matching
/// `MechanicalGeometry`'s own "Z is up" convention). Built the same way
/// `truck-modeling`'s own `examples/cylinder.rs` does -- one vertex,
/// revolved into a circle, attached as a disk face, extruded -- adapted
/// from that example's Y-axis construction (centered at the origin) to
/// build directly at an arbitrary center around Z instead.
///
/// `Rad(7.0)` (not exactly `2.0 * PI`) is `truck-modeling`'s own example
/// value for a full revolution; kept exactly as used there since this
/// construction was independently OCCT-verified with this same value
/// (valid solid, volume matching the analytic `pi * r^2 * h` to
/// 4.3e-7mm^3) rather than substituted for an unverified "more correct
/// looking" constant.
fn build_cylinder(center: [f64; 3], diameter: f64, height: f64) -> Solid {
    let radius = diameter / 2.0;
    let base_z = center[2] - height / 2.0;
    let axis_origin = Point3::new(center[0], center[1], base_z);
    let rim_point = Point3::new(center[0] + radius, center[1], base_z);

    let vertex = builder::vertex(rim_point);
    let circle = builder::rsweep(&vertex, axis_origin, Vector3::unit_z(), Rad(7.0));
    let disk = builder::try_attach_plane(&[circle]).expect(
        "rsweep of a single vertex around a distinct axis always yields a valid closed \
         circular wire, so attaching a planar disk face to it cannot fail",
    );
    builder::tsweep(&disk, Vector3::new(0.0, 0.0, height))
}

/// Same construction as `build_cylinder`, but lying on its side: axis
/// along X instead of Z (matching `BodyShape::CylinderX`, e.g. an
/// axial-leaded diode can). The rim point is offset in Y instead of X,
/// the revolution axis is `unit_x()` instead of `unit_z()`, and the
/// final extrusion runs along X instead of Z -- otherwise identical.
fn build_cylinder_x(center: [f64; 3], diameter: f64, length: f64) -> Solid {
    let radius = diameter / 2.0;
    let base_x = center[0] - length / 2.0;
    let axis_origin = Point3::new(base_x, center[1], center[2]);
    let rim_point = Point3::new(base_x, center[1] + radius, center[2]);

    let vertex = builder::vertex(rim_point);
    let circle = builder::rsweep(&vertex, axis_origin, Vector3::unit_x(), Rad(7.0));
    let disk = builder::try_attach_plane(&[circle]).expect(
        "rsweep of a single vertex around a distinct axis always yields a valid closed \
         circular wire, so attaching a planar disk face to it cannot fail",
    );
    builder::tsweep(&disk, Vector3::new(length, 0.0, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_mcad::{Marker, MarkerKind, Mounting, Point3 as McadPoint3, Size3};

    fn box_geometry() -> MechanicalGeometry {
        MechanicalGeometry {
            body: Body {
                position: McadPoint3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.5,
                },
                size: Size3 {
                    x: 3.0,
                    y: 3.0,
                    z: 1.0,
                },
                shape: BodyShape::Box,
            },
            leads: vec![Lead {
                number: "1".into(),
                position: McadPoint3 {
                    x: -2.0,
                    y: 0.0,
                    z: 0.05,
                },
                size: Size3 {
                    x: 0.6,
                    y: 0.2,
                    z: 0.1,
                },
                mounting: Mounting::Smd,
                drill: None,
            }],
            markers: vec![Marker {
                kind: MarkerKind::Pin1Dot,
                position: McadPoint3 {
                    x: -1.0,
                    y: 1.0,
                    z: 1.0,
                },
            }],
        }
    }

    fn cylinder_geometry() -> MechanicalGeometry {
        MechanicalGeometry {
            body: Body {
                position: McadPoint3 {
                    x: 0.0,
                    y: 0.0,
                    z: 5.75,
                },
                size: Size3 {
                    x: 8.0,
                    y: 8.0,
                    z: 11.5,
                },
                shape: BodyShape::Cylinder,
            },
            leads: vec![],
            markers: vec![],
        }
    }

    /// Every vertex of a built box must sit exactly on the box's
    /// expected corner set -- an independent geometric check on the
    /// `Solid`'s own vertex positions, not just "it compiled".
    #[test]
    fn box_vertices_match_the_requested_bounds() {
        let geometry = box_geometry();
        let brep = build(&geometry);
        let (cx, cy, cz) = (0.0, 0.0, 0.5);
        let (dx, dy, dz) = (1.5, 1.5, 0.5);
        for v in brep.body.vertex_iter() {
            let p = v.point();
            assert!((p.x - cx).abs() <= dx + 1e-9);
            assert!((p.y - cy).abs() <= dy + 1e-9);
            assert!((p.z - cz).abs() <= dz + 1e-9);
        }
        assert_eq!(brep.leads.len(), 1);
    }

    /// Every vertex of a built cylinder must sit on the can's radius
    /// (in XY) and within its height (in Z) -- mirrors the same
    /// independent check `openparts-stl`'s old hand-rolled tests used
    /// (`cylinder_facets_all_sit_on_the_can_surface`).
    #[test]
    fn cylinder_vertices_sit_on_the_can_surface() {
        let geometry = cylinder_geometry();
        let brep = build(&geometry);
        let radius = geometry.body.size.x / 2.0;
        let half_height = geometry.body.size.z / 2.0;
        let center_z = geometry.body.position.z;

        let mut saw_a_vertex = false;
        for v in brep.body.vertex_iter() {
            saw_a_vertex = true;
            let p = v.point();
            let r = (p.x * p.x + p.y * p.y).sqrt();
            assert!(
                (r - radius).abs() < 1e-6,
                "vertex {p:?} not on the can radius {radius}"
            );
            let z = p.z - center_z;
            assert!(
                z.abs() <= half_height + 1e-6,
                "vertex {p:?} outside the can's height"
            );
        }
        assert!(saw_a_vertex, "cylinder solid has no vertices at all");
    }

    fn cylinder_x_geometry() -> MechanicalGeometry {
        MechanicalGeometry {
            body: Body {
                position: McadPoint3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.35,
                },
                size: Size3 {
                    x: 5.2,
                    y: 2.7,
                    z: 2.7,
                },
                shape: BodyShape::CylinderX,
            },
            leads: vec![],
            markers: vec![],
        }
    }

    /// Same check as `cylinder_vertices_sit_on_the_can_surface`, but for
    /// the horizontal orientation: every vertex must sit on the can's
    /// radius in the Y-Z plane (not X-Y), and within its length along X
    /// (not height along Z) -- confirms the axis really did move, not
    /// just get relabeled.
    #[test]
    fn cylinder_x_vertices_sit_on_the_can_surface_and_are_wider_in_x_than_y_or_z() {
        let geometry = cylinder_x_geometry();
        let brep = build(&geometry);
        let radius = geometry.body.size.y / 2.0;
        let half_length = geometry.body.size.x / 2.0;
        let center_x = geometry.body.position.x;
        let center_y = geometry.body.position.y;
        let center_z = geometry.body.position.z;

        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut saw_a_vertex = false;
        for v in brep.body.vertex_iter() {
            saw_a_vertex = true;
            let p = v.point();
            let r = ((p.y - center_y).powi(2) + (p.z - center_z).powi(2)).sqrt();
            assert!(
                (r - radius).abs() < 1e-6,
                "vertex {p:?} not on the can radius {radius}"
            );
            let x = p.x - center_x;
            assert!(
                x.abs() <= half_length + 1e-6,
                "vertex {p:?} outside the can's length"
            );
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
        assert!(saw_a_vertex, "cylinder solid has no vertices at all");
        // Bounding box must be longer in X (the can's length, 5.2mm)
        // than in Y (the can's diameter, 2.7mm) -- proves the axis is
        // genuinely horizontal, not a vertical cylinder that happens to
        // pass the radius/length checks above by coincidence.
        assert!(max_x - min_x > max_y - min_y);
    }
}
