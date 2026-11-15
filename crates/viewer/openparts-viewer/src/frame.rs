//! One frame for both views: the Placement Model's
//! (docs/spec/placement-model.md section 2) -- mm, top view +X right,
//! +Y up, +Z up from the board, the origin at the land-pattern centroid,
//! the part turned to IPC-7351B Level A (pin 1 upper left).
//!
//! The footprint view is drawn in it already (`openparts-pcbcad` builds
//! the footprint from the model). The server's 3MF / STL, and the
//! Mechanical Geometry the dimensions and pad snaps are computed from,
//! are in the generator's own frame; [`Alignment`] is the model's
//! `model3d` (`p_frame = R(rotation) · p_model + offset`) and [`scene`]
//! moves all of them into the model frame, so the mesh, the dimension
//! anchors, the snap points, the measure readouts and the gizmo's axes
//! all mean the footprint's X / Y.

use crate::dims::{self, Annotation};
use crate::mesh::Mesh;
use crate::snap::{self, Candidate};
use glam::Vec3;
use openparts_core::Package;
use openparts_mcad::MechanicalGeometry;
use openparts_placement::{Density, Model3d};

/// The generated 3D model's placement in the model frame: a turn about
/// +Z, then an offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alignment {
    /// cos and sin of the turn (exact on quarter turns).
    cos: f32,
    sin: f32,
    pub offset: Vec3,
}

impl Default for Alignment {
    fn default() -> Self {
        Alignment {
            cos: 1.0,
            sin: 0.0,
            offset: Vec3::ZERO,
        }
    }
}

impl Alignment {
    /// The model's `model3d`.
    pub fn of(m: &Model3d) -> Self {
        let quarter = m.rotation / 90.0;
        let (sin, cos) = if (quarter - quarter.round()).abs() < 1e-9 {
            match (quarter.round() as i64).rem_euclid(4) {
                0 => (0.0, 1.0),
                1 => (1.0, 0.0),
                2 => (0.0, -1.0),
                _ => (-1.0, 0.0),
            }
        } else {
            m.rotation.to_radians().sin_cos()
        };
        Alignment {
            cos: cos as f32,
            sin: sin as f32,
            offset: Vec3::new(m.offset[0] as f32, m.offset[1] as f32, m.offset[2] as f32),
        }
    }

    /// The alignment of `g`'s 3D model: its Placement Model's `model3d`
    /// (the same one the server's `/placement` and the KiCad footprint
    /// carry; it does not depend on the density level).
    pub fn of_geometry(g: &MechanicalGeometry) -> Self {
        Self::of(&openparts_placement::from_geometry(g, Density::N).model3d)
    }

    /// The turn in degrees, CCW from the top.
    pub fn rotation(&self) -> f32 {
        self.sin.atan2(self.cos).to_degrees()
    }

    /// A direction (no offset).
    pub fn vector(&self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.cos * v.x - self.sin * v.y,
            self.sin * v.x + self.cos * v.y,
            v.z,
        )
    }

    /// A generator-frame point in the model frame.
    pub fn point(&self, p: Vec3) -> Vec3 {
        self.vector(p) + self.offset
    }

    /// A model-frame point back in the generator frame.
    pub fn invert(&self, p: Vec3) -> Vec3 {
        let d = p - self.offset;
        Vec3::new(
            self.cos * d.x + self.sin * d.y,
            -self.sin * d.x + self.cos * d.y,
            d.z,
        )
    }

    /// Moves the mesh (positions and normals; a turn keeps the winding).
    pub fn mesh(&self, m: &mut Mesh) {
        for p in &mut m.positions {
            *p = self.point(*p);
        }
        for n in &mut m.normals {
            *n = self.vector(*n);
        }
    }

    pub fn annotation(&self, a: &mut Annotation) {
        for p in a.feature.iter_mut().chain(a.line.iter_mut()) {
            *p = self.point(*p);
        }
    }

    pub fn candidate(&self, c: &mut Candidate) {
        c.point = self.point(c.point);
    }
}

/// What the 3D view draws, in the model frame.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub mesh: Mesh,
    /// The mesh's (min, max) corners.
    pub bounds: Option<(Vec3, Vec3)>,
    pub annotations: Vec<Annotation>,
    pub snaps: Vec<Candidate>,
    pub alignment: Alignment,
}

/// The 3D view's contents: `mesh` (the server's 3MF / STL, generator
/// frame) with the datasheet dimensions and snap targets computed on it
/// and on `geometry` (the same frame), all moved into the Placement
/// Model's frame by `geometry`'s alignment. Without a geometry, the mesh
/// stays as it is and there are no dimensions or pad snaps.
pub fn scene(mut mesh: Mesh, package: &Package, geometry: Option<&MechanicalGeometry>) -> Scene {
    // Computed where the generator laid them out (dims places its lines
    // by the generator's own X / Y: body width along X, ...), then moved.
    let raw_bounds = mesh.bounds();
    let mut annotations = geometry
        .map(|g| dims::annotations(package, g, raw_bounds))
        .unwrap_or_default();
    let mut snaps = snap::candidates(&mesh, geometry);
    let alignment = geometry.map(Alignment::of_geometry).unwrap_or_default();
    alignment.mesh(&mut mesh);
    annotations.iter_mut().for_each(|a| alignment.annotation(a));
    snaps.iter_mut().for_each(|c| alignment.candidate(c));
    Scene {
        bounds: mesh.bounds(),
        mesh,
        annotations,
        snaps,
        alignment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::parse_3mf;
    use crate::snap::{raycast, SnapKind};
    use openparts_placement::PlacementModel;

    fn load(json: &str) -> (Package, MechanicalGeometry, PlacementModel) {
        let p: Package = serde_json::from_str(json).unwrap();
        let g = openparts_mcad::generate(&p).unwrap();
        let m = openparts_placement::build(&p, None, &g, Density::N);
        (p, g, m)
    }

    fn close(a: f32, b: f64, tol: f32) -> bool {
        (a - b as f32).abs() <= tol
    }

    /// Whether a ray straight down at (x, y) hits the mesh.
    fn solid_at(mesh: &Mesh, x: f64, y: f64) -> bool {
        raycast(
            mesh,
            Vec3::new(x as f32, y as f32, 50.0),
            Vec3::new(0.0, 0.0, -1.0),
        )
        .is_some()
    }

    #[test]
    fn quarter_turns_are_exact_and_invert() {
        let a = Alignment::of(&Model3d {
            rotation: 270.0,
            offset: [0.5, -2.0, 0.0],
        });
        assert_eq!(a.point(Vec3::new(1.0, 0.0, 3.0)), Vec3::new(0.5, -3.0, 3.0));
        assert_eq!(a.rotation(), -90.0);
        let p = Vec3::new(0.3, -1.7, 0.2);
        assert!((a.invert(a.point(p)) - p).length() < 1e-6);
        let b = Alignment::of(&Model3d {
            rotation: 30.0,
            offset: [0.0; 3],
        });
        assert!((b.rotation() - 30.0).abs() < 1e-4);
    }

    #[test]
    fn sot23_mesh_turns_into_the_footprint_frame() {
        let (p, g, m) = load(include_str!("../tests/sot23.json"));
        // The SOT row turns -90 degrees to put pin 1 upper left.
        assert_eq!(m.model3d.rotation, 270.0);
        let raw = parse_3mf(include_bytes!("../tests/ao3400a.3mf")).unwrap();
        let s = scene(raw.clone(), &p, Some(&g));
        let (lo, hi) = s.bounds.unwrap();

        // The body: 1.6 (X) x 2.9 (Y) about the origin, on the board
        // (the model's body sits on its standoff).
        let body = &m.body.regions[0];
        assert!(close(hi.y - lo.y, body.size.y, 0.02), "{lo} {hi}");
        assert!(close((lo.y + hi.y) / 2.0, body.center.y, 0.02));
        assert!(close(lo.z, 0.0, 0.02), "{lo}");
        assert!(close(hi.z, m.body.height + m.body.standoff, 0.02), "{hi}");
        // The leads reach across X past the body, over the pads.
        let (pad_lo, pad_hi) = m.pads.iter().map(|p| p.bounds()).fold(
            ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN)),
            |(l, h), (a, b)| ((l.0.min(a.x), l.1.min(a.y)), (h.0.max(b.x), h.1.max(b.y))),
        );
        assert!(lo.x as f64 > pad_lo.0 && (hi.x as f64) < pad_hi.0);
        assert!(lo.y as f64 > pad_lo.1 - 0.5 && (hi.y as f64) < pad_hi.1 + 0.5);
        assert!(close((lo.x + hi.x) / 2.0, 0.0, 0.02));
        // A lead over every pad (pin 1 upper left), none off them.
        for pad in &m.pads {
            assert!(
                solid_at(&s.mesh, pad.position.x, pad.position.y),
                "pad {}",
                pad.number
            );
        }
        assert!(!solid_at(&s.mesh, 1.1, 0.95));
        // Untransformed, pin 1's pad would have no lead over it.
        assert!(!solid_at(&raw, m.pads[0].position.x, m.pads[0].position.y));

        // Pad snaps sit on the placement's pads.
        let pads: Vec<Vec3> = s
            .snaps
            .iter()
            .filter(|c| c.kind == SnapKind::PadCenter)
            .map(|c| c.point)
            .collect();
        assert_eq!(pads.len(), m.pads.len());
        for pad in &m.pads {
            assert!(pads
                .iter()
                .any(|c| close(c.x, pad.position.x, 1e-3) && close(c.y, pad.position.y, 1e-3)));
        }
        // The body width (2.9, the generator's X) now runs along Y; the
        // pitch's two leads are pins 1 and 2, on the left.
        let a = |k: &str| s.annotations.iter().find(|a| a.key == k).unwrap();
        let w = a("body_width");
        assert!(close((w.feature[1] - w.feature[0]).length(), 2.9, 1e-4));
        assert!((w.feature[1] - w.feature[0]).normalize().y.abs() > 0.999);
        let pitch = a("pitch");
        for f in pitch.feature {
            assert!(close(f.x, -1.1, 0.3), "{f}");
        }
    }

    #[test]
    fn usb_c_mesh_turns_and_moves_onto_its_pads() {
        let (p, g, m) = load(include_str!("../tests/usb4105.json"));
        assert_eq!(m.model3d.rotation, 180.0);
        assert!(m.model3d.offset[1].abs() > 1.0);
        let raw = parse_3mf(include_bytes!("../tests/usb4105.3mf")).unwrap();
        let s = scene(raw.clone(), &p, Some(&g));
        let (lo, hi) = s.bounds.unwrap();
        let body = &m.body.regions[0];
        // The shell's through-hole legs (below the board) stand in the
        // placement's SHELL pads; untransformed they would not.
        let legs_on_pads = |mesh: &Mesh| {
            mesh.positions.iter().filter(|p| p.z < -0.2).all(|p| {
                m.pads
                    .iter()
                    .filter(|pad| pad.number == "SHELL")
                    .any(|pad| {
                        let (a, b) = pad.bounds();
                        (a.x..=b.x).contains(&(p.x as f64)) && (a.y..=b.y).contains(&(p.y as f64))
                    })
            })
        };
        assert!(legs_on_pads(&s.mesh));
        assert!(!legs_on_pads(&raw));
        // The shell: its outline in X and Y, at the placement's body.
        assert!(close(lo.x, body.center.x - body.size.x / 2.0, 0.05), "{lo}");
        assert!(close(hi.x, body.center.x + body.size.x / 2.0, 0.05), "{hi}");
        assert!(close(lo.y, body.center.y - body.size.y / 2.0, 0.05), "{lo}");
        assert!(close(hi.z, body.z_max, 0.05));
        // Through-hole shell legs below the board.
        assert!(lo.z < 0.0);
        // The contact row (A1 .. A12) under the rear edge, pin A1 left.
        let a1 = m.pads.iter().find(|p| p.number == "A1").unwrap();
        assert!(a1.position.x < 0.0 && a1.position.y > 0.0);
        assert!(hi.y as f64 > a1.position.y - a1.size.y / 2.0);
        for pad in m.pads.iter().filter(|p| p.number.starts_with('A')) {
            assert!(
                solid_at(&s.mesh, pad.position.x, pad.position.y),
                "pad {}",
                pad.number
            );
        }
        // Dimension lines measure the same lengths, now in this frame.
        let w = s
            .annotations
            .iter()
            .find(|a| a.key == "body_width")
            .unwrap();
        assert!(w.feature.iter().all(|f| f.x.abs() <= 4.5 && f.y < 2.6));
    }

    #[test]
    fn no_geometry_leaves_the_mesh_as_is() {
        let (p, _, _) = load(include_str!("../tests/qfn56.json"));
        let mesh = parse_3mf(include_bytes!("../tests/rp2040.3mf")).unwrap();
        let s = scene(mesh.clone(), &p, None);
        assert_eq!(s.alignment, Alignment::default());
        assert_eq!(s.bounds, mesh.bounds());
        assert!(s.annotations.is_empty());
    }
}
