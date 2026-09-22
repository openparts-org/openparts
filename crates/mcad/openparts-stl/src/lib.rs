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

/// Renders `geometry` (one box per body + lead) as an ASCII STL solid
/// named after `product_name`.
pub fn generate_stl(geometry: &MechanicalGeometry, product_name: &str) -> Result<String, StlError> {
    let mut facets = box_facets(
        [
            geometry.body.position.x,
            geometry.body.position.y,
            geometry.body.position.z,
        ],
        [
            geometry.body.size.x,
            geometry.body.size.y,
            geometry.body.size.z,
        ],
    );
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
    use openparts_mcad::{Body, Lead, Marker, MarkerKind, Point3, Size3};

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
}
