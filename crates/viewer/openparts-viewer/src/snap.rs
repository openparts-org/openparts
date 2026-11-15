//! The measure tool's snapping: the model's vertices and edge midpoints
//! (feature edges only, not triangulation diagonals), and the pad centres
//! from the Mechanical Geometry; plus ray casting onto the triangles.

use crate::mesh::{key, Mesh};
use glam::{Vec2, Vec3};
use openparts_mcad::{MechanicalGeometry, Mounting};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    Vertex,
    EdgeMidpoint,
    PadCenter,
    /// No snap: a point on the surface.
    Surface,
}

impl SnapKind {
    /// Its label's i18n key.
    pub fn key(self) -> &'static str {
        match self {
            SnapKind::Vertex => "snap_vertex",
            SnapKind::EdgeMidpoint => "snap_edge",
            SnapKind::PadCenter => "snap_pad",
            SnapKind::Surface => "snap_surface",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub point: Vec3,
    pub kind: SnapKind,
}

/// An edge by its (sorted) end points' keys.
type EdgeKey = ([i64; 3], [i64; 3]);

/// Edges whose faces meet at more than this (degrees) are feature edges.
const FEATURE_ANGLE: f32 = 30.0;

/// Every snap target of the model.
pub fn candidates(mesh: &Mesh, geometry: Option<&MechanicalGeometry>) -> Vec<Candidate> {
    let mut edges: HashMap<EdgeKey, (Vec3, Vec3, Vec<Vec3>)> = HashMap::new();
    for t in mesh.positions.as_chunks::<3>().0 {
        let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (ka, kb) = (key(a), key(b));
            let k = if ka <= kb { (ka, kb) } else { (kb, ka) };
            edges
                .entry(k)
                .or_insert_with(|| (a, b, Vec::new()))
                .2
                .push(n);
        }
    }
    let cos = FEATURE_ANGLE.to_radians().cos();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut add = |point: Vec3, kind: SnapKind, out: &mut Vec<Candidate>| {
        if seen.insert((key(point), kind as u8)) {
            out.push(Candidate { point, kind });
        }
    };
    for (a, b, normals) in edges.values() {
        let feature = normals.len() != 2 || normals[0].dot(normals[1]) < cos;
        if feature {
            add(*a, SnapKind::Vertex, &mut out);
            add(*b, SnapKind::Vertex, &mut out);
            add((*a + *b) * 0.5, SnapKind::EdgeMidpoint, &mut out);
        }
    }
    if let Some(g) = geometry {
        let pads = if g.lands.is_empty() {
            &g.leads
        } else {
            &g.lands
        };
        for p in pads {
            let z = match p.mounting {
                Mounting::Smd => (p.position.z + p.size.z / 2.0) as f32,
                _ => 0.0,
            };
            add(
                Vec3::new(p.position.x as f32, p.position.y as f32, z),
                SnapKind::PadCenter,
                &mut out,
            );
        }
    }
    out
}

/// The nearest hit of the ray (origin, unit dir) on the mesh: distance
/// along the ray.
pub fn raycast(mesh: &Mesh, origin: Vec3, dir: Vec3) -> Option<f32> {
    let mut best: Option<f32> = None;
    for t in mesh.positions.as_chunks::<3>().0 {
        // Möller–Trumbore, both sides.
        let e1 = t[1] - t[0];
        let e2 = t[2] - t[0];
        let p = dir.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-12 {
            continue;
        }
        let inv = 1.0 / det;
        let s = origin - t[0];
        let u = s.dot(p) * inv;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = s.cross(e1);
        let v = dir.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let d = e2.dot(q) * inv;
        if d > 1e-6 && best.is_none_or(|b| d < b) {
            best = Some(d);
        }
    }
    best
}

/// The candidate nearest `cursor` (CSS pixels) within `radius` pixels,
/// pad centres favoured; `project` maps a point to the screen and
/// `visible` says whether it isn't hidden behind the model.
pub fn pick(
    candidates: &[Candidate],
    cursor: Vec2,
    radius: f32,
    project: impl Fn(Vec3) -> Option<Vec2>,
    visible: impl Fn(Vec3) -> bool,
) -> Option<Candidate> {
    let mut near: Vec<(f32, &Candidate)> = candidates
        .iter()
        .filter_map(|c| {
            let d = (project(c.point)? - cursor).length();
            let bonus = if c.kind == SnapKind::PadCenter {
                4.0
            } else {
                0.0
            };
            (d <= radius).then_some((d - bonus, c))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    near.into_iter()
        .take(12)
        .find(|(_, c)| visible(c.point))
        .map(|(_, c)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::STL_GREY;

    fn cube() -> Mesh {
        // A unit cube from 12 triangles.
        let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        let quads = [
            [v(0., 0., 0.), v(1., 0., 0.), v(1., 1., 0.), v(0., 1., 0.)],
            [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)],
            [v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.), v(0., 0., 1.)],
            [v(0., 1., 0.), v(1., 1., 0.), v(1., 1., 1.), v(0., 1., 1.)],
            [v(0., 0., 0.), v(0., 1., 0.), v(0., 1., 1.), v(0., 0., 1.)],
            [v(1., 0., 0.), v(1., 1., 0.), v(1., 1., 1.), v(1., 0., 1.)],
        ];
        let mut m = Mesh::default();
        for q in quads {
            m.positions.extend([q[0], q[1], q[2], q[0], q[2], q[3]]);
            m.colors.extend([STL_GREY; 6]);
        }
        m.compute_normals();
        m
    }

    #[test]
    fn snaps_to_corners_and_real_edges_only() {
        let c = candidates(&cube(), None);
        let vertices = c.iter().filter(|c| c.kind == SnapKind::Vertex).count();
        let mids = c
            .iter()
            .filter(|c| c.kind == SnapKind::EdgeMidpoint)
            .count();
        assert_eq!(vertices, 8);
        // 12 cube edges; the face diagonals are not features.
        assert_eq!(mids, 12);
    }

    #[test]
    fn raycasts_the_nearest_face() {
        let m = cube();
        let t = raycast(&m, Vec3::new(0.5, 0.5, 5.0), -Vec3::Z).unwrap();
        assert!((t - 4.0).abs() < 1e-5);
        assert_eq!(raycast(&m, Vec3::new(5.0, 5.0, 5.0), -Vec3::Z), None);
    }

    #[test]
    fn picks_the_nearest_visible_candidate() {
        let c = [
            Candidate {
                point: Vec3::new(10.0, 0.0, 0.0),
                kind: SnapKind::Vertex,
            },
            Candidate {
                point: Vec3::new(20.0, 0.0, 0.0),
                kind: SnapKind::PadCenter,
            },
        ];
        let project = |p: Vec3| Some(Vec2::new(p.x, p.y));
        let near = |x: f32| pick(&c, Vec2::new(x, 0.0), 8.0, project, |_| true);
        // Pad centres win ties within their bonus.
        assert_eq!(near(14.0).unwrap().kind, SnapKind::PadCenter);
        assert_eq!(near(11.0).unwrap().kind, SnapKind::Vertex);
        assert_eq!(near(40.0), None);
        let hidden = pick(&c, Vec2::new(14.0, 0.0), 8.0, project, |p| p.x < 12.0);
        assert_eq!(hidden.unwrap().kind, SnapKind::Vertex);
    }
}
