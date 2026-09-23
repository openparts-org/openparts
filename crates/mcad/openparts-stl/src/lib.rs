//! openparts-stl: Mechanical Geometry -> STL (ASCII) text. No CAD
//! library dependency, same hand-written-text-format policy as
//! openparts-step. Geometry is the same set of axis-aligned boxes STEP
//! uses (one for the body, one per lead) -- structurally valid, not
//! datasheet-precision geometry.
//!
//! NOTE (Testing and Quality Specification section 9): like STEP, this
//! crate has no independent STL reader, so output is verified only by
//! structural checks, never claimed as "read-back verified".

use openparts_mcad::MechanicalGeometry;

#[derive(Debug, thiserror::Error)]
pub enum StlError {
    #[error("geometry has no boxes to export (no body?)")]
    Empty,
}

type Point = [f64; 3];
type Facet = ([f64; 3], [Point; 3]); // (normal, 3 vertices)

/// Same 8-corner numbering and 6-face outward-normal convention as
/// `openparts-step`'s `write_box`, but each face becomes 2 triangles
/// (independent vertices, no shared topology -- STL has none).
fn box_facets(center: [f64; 3], size: [f64; 3]) -> Vec<Facet> {
    let (cx, cy, cz) = (center[0], center[1], center[2]);
    let (dx, dy, dz) = (size[0] / 2.0, size[1] / 2.0, size[2] / 2.0);

    let p1 = [cx - dx, cy - dy, cz - dz];
    let p2 = [cx + dx, cy - dy, cz - dz];
    let p3 = [cx + dx, cy + dy, cz - dz];
    let p4 = [cx - dx, cy + dy, cz - dz];
    let p5 = [cx - dx, cy - dy, cz + dz];
    let p6 = [cx + dx, cy - dy, cz + dz];
    let p7 = [cx + dx, cy + dy, cz + dz];
    let p8 = [cx - dx, cy + dy, cz + dz];

    let quad = |normal: [f64; 3], a: Point, b: Point, c: Point, d: Point, out: &mut Vec<Facet>| {
        out.push((normal, [a, b, c]));
        out.push((normal, [a, c, d]));
    };

    let mut facets = Vec::with_capacity(12);
    quad([0.0, 0.0, -1.0], p1, p4, p3, p2, &mut facets); // bottom
    quad([0.0, 0.0, 1.0], p5, p6, p7, p8, &mut facets); // top
    quad([0.0, -1.0, 0.0], p1, p2, p6, p5, &mut facets); // front (-y)
    quad([0.0, 1.0, 0.0], p3, p4, p8, p7, &mut facets); // back (+y)
    quad([-1.0, 0.0, 0.0], p4, p1, p5, p8, &mut facets); // left (-x)
    quad([1.0, 0.0, 0.0], p2, p3, p7, p6, &mut facets); // right (+x)
    facets
}

/// Same `openparts_mcad::cylinder_ring`-based tessellation and winding
/// convention as `openparts-step`'s `write_prism` (side face i:
/// `bottom[i] -> bottom[i+1] -> top[i+1] -> top[i]`; bottom cap
/// traversed in reverse for its outward -Z normal, top cap forward for
/// +Z), each quad/n-gon fan-triangulated into independent STL facets.
/// `openparts-step`'s `every_face_winding_matches_its_declared_normal`-
/// style test already verified this same winding scheme is correct;
/// see `openparts-mcad::cylinder_ring`'s docs for why the segment
/// count is this writer's own choice, not shared geometry state.
const STL_CYLINDER_SEGMENTS: u32 = 24;

fn cylinder_facets(center: [f64; 3], diameter: f64, height: f64, segments: u32) -> Vec<Facet> {
    let (cx, cy, cz) = (center[0], center[1], center[2]);
    let dz = height / 2.0;
    let n = segments as usize;
    let ring = openparts_mcad::cylinder_ring(diameter, segments);
    let bottom: Vec<Point> = ring
        .iter()
        .map(|&(x, y)| [cx + x, cy + y, cz - dz])
        .collect();
    let top: Vec<Point> = ring
        .iter()
        .map(|&(x, y)| [cx + x, cy + y, cz + dz])
        .collect();

    let mut facets = Vec::with_capacity(4 * n - 4);

    for i in 0..n {
        let j = (i + 1) % n;
        let normal = normalize(cross(sub(bottom[j], bottom[i]), sub(top[i], bottom[i])));
        facets.push((normal, [bottom[i], bottom[j], top[j]]));
        facets.push((normal, [bottom[i], top[j], top[i]]));
    }

    // Bottom cap: fan-triangulated from a reverse-order traversal
    // (bottom[0], bottom[n-1], bottom[n-2], ...) so its winding agrees
    // with the -Z outward normal; top cap uses the ring's own forward
    // order for +Z.
    let bottom_loop: Vec<Point> = (0..n).map(|k| bottom[(n - k) % n]).collect();
    for i in 1..n - 1 {
        facets.push((
            [0.0, 0.0, -1.0],
            [bottom_loop[0], bottom_loop[i], bottom_loop[i + 1]],
        ));
    }
    for i in 1..n - 1 {
        facets.push(([0.0, 0.0, 1.0], [top[0], top[i], top[i + 1]]));
    }

    facets
}

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: Point) -> Point {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 0.0 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        v
    }
}

/// Renders `geometry` (one box per body + lead) as an ASCII STL solid
/// named after `product_name`.
pub fn generate_stl(geometry: &MechanicalGeometry, product_name: &str) -> Result<String, StlError> {
    let body_center = [
        geometry.body.position.x,
        geometry.body.position.y,
        geometry.body.position.z,
    ];
    let mut facets = match geometry.body.shape {
        openparts_mcad::BodyShape::Box => box_facets(
            body_center,
            [
                geometry.body.size.x,
                geometry.body.size.y,
                geometry.body.size.z,
            ],
        ),
        openparts_mcad::BodyShape::Cylinder => cylinder_facets(
            body_center,
            geometry.body.size.x, // diameter (== size.y)
            geometry.body.size.z, // height
            STL_CYLINDER_SEGMENTS,
        ),
    };
    for lead in &geometry.leads {
        facets.extend(box_facets(
            [lead.position.x, lead.position.y, lead.position.z],
            [lead.size.x, lead.size.y, lead.size.z],
        ));
    }

    if facets.is_empty() {
        return Err(StlError::Empty);
    }

    let mut out = String::new();
    out.push_str(&format!("solid {product_name}\n"));
    for (normal, verts) in &facets {
        out.push_str(&format!(
            "  facet normal {:.6} {:.6} {:.6}\n",
            normal[0], normal[1], normal[2]
        ));
        out.push_str("    outer loop\n");
        for v in verts {
            out.push_str(&format!(
                "      vertex {:.6} {:.6} {:.6}\n",
                v[0], v[1], v[2]
            ));
        }
        out.push_str("    endloop\n");
        out.push_str("  endfacet\n");
    }
    out.push_str(&format!("endsolid {product_name}\n"));

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openparts_mcad::{Body, BodyShape, Lead, Marker, MarkerKind, Mounting, Point3, Size3};

    fn tiny_geometry() -> MechanicalGeometry {
        MechanicalGeometry {
            body: Body {
                position: Point3 {
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
            leads: vec![
                Lead {
                    number: "1".into(),
                    position: Point3 {
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
                },
                Lead {
                    number: "2".into(),
                    position: Point3 {
                        x: 2.0,
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
                },
            ],
            markers: vec![Marker {
                kind: MarkerKind::Pin1Dot,
                position: Point3 {
                    x: -1.0,
                    y: 1.0,
                    z: 1.0,
                },
            }],
        }
    }

    #[test]
    fn produces_a_well_formed_stl_file() {
        let stl = generate_stl(&tiny_geometry(), "TEST").unwrap();
        assert!(stl.starts_with("solid TEST\n"));
        assert!(stl.trim_end().ends_with("endsolid TEST"));
    }

    #[test]
    fn has_twelve_facets_per_box() {
        let stl = generate_stl(&tiny_geometry(), "TEST").unwrap();
        let facet_count = stl.matches("facet normal").count();
        // 1 body + 2 leads = 3 boxes, 12 facets each.
        assert_eq!(facet_count, 12 * 3);
    }

    #[test]
    fn every_facet_has_exactly_three_vertices() {
        let stl = generate_stl(&tiny_geometry(), "TEST").unwrap();
        let vertex_count = stl.matches("vertex ").count();
        assert_eq!(vertex_count, 12 * 3 * 3);
    }

    fn tiny_cylinder_geometry() -> MechanicalGeometry {
        MechanicalGeometry {
            body: Body {
                position: Point3 {
                    x: 0.0,
                    y: 0.0,
                    z: 5.5,
                },
                size: Size3 {
                    x: 5.0,
                    y: 5.0,
                    z: 11.0,
                },
                shape: BodyShape::Cylinder,
            },
            leads: vec![],
            markers: vec![],
        }
    }

    #[test]
    fn cylinder_body_produces_four_segments_minus_four_facets() {
        let stl = generate_stl(&tiny_cylinder_geometry(), "TEST").unwrap();
        let facet_count = stl.matches("facet normal").count();
        // A box is this same tessellation formula's degenerate N=4
        // case (4*4-4=12, matching `has_twelve_facets_per_box` above).
        assert_eq!(facet_count, 4 * STL_CYLINDER_SEGMENTS as usize - 4);
    }

    #[test]
    fn cylinder_facets_all_sit_on_the_can_surface() {
        // Independent geometric check, mirroring openparts-step's own
        // winding verification: every vertex of every side/cap facet
        // must lie within the can's radius (XY) and height (Z) bounds.
        let geometry = tiny_cylinder_geometry();
        let radius = geometry.body.size.x / 2.0;
        let half_height = geometry.body.size.z / 2.0;
        let facets = cylinder_facets(
            [
                geometry.body.position.x,
                geometry.body.position.y,
                geometry.body.position.z,
            ],
            geometry.body.size.x,
            geometry.body.size.z,
            STL_CYLINDER_SEGMENTS,
        );
        for (_, verts) in &facets {
            for v in verts {
                let r = (v[0] * v[0] + v[1] * v[1]).sqrt();
                assert!(
                    (r - radius).abs() < 1e-9,
                    "vertex {v:?} not on the can radius"
                );
                let z = v[2] - geometry.body.position.z;
                assert!(
                    z.abs() <= half_height + 1e-9,
                    "vertex {v:?} outside the can's height"
                );
            }
        }
    }
}
