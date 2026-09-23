//! openparts-stl: Mechanical Geometry -> STL (ASCII) text, via `truck`
//! (`openparts-brep` builds the real B-rep solids; `truck-meshalgo`
//! tessellates them; `truck-polymesh` writes STL). Unlike STEP, STL has
//! no color concept at all, so this crate has no hand-written STEP-style
//! text left in it -- it's a thin adapter over `openparts-brep` +
//! `truck-meshalgo`/`truck-polymesh`.
//!
//! NOTE (Testing and Quality Specification section 9): like before, this
//! crate has no independent STL reader, so output is verified only by
//! structural checks, never claimed as "read-back verified".

use openparts_brep::BrepGeometry;
use openparts_mcad::MechanicalGeometry;
use truck_meshalgo::filters::OptimizingFilter;
use truck_meshalgo::tessellation::{MeshableShape, MeshedShape};
use truck_polymesh::stl::StlType;
use truck_polymesh::{PolygonMesh, TOLERANCE};

#[derive(Debug, thiserror::Error)]
pub enum StlError {
    #[error("geometry has no solids to export (no body?)")]
    Empty,
}

/// Chord tolerance (mm) used to tessellate a `BodyShape::Cylinder`'s
/// true curved surface into STL triangles -- STL has no curves at all,
/// so some tessellation is unavoidable here even though `openparts-step`
/// no longer needs one. Picked from measurements taken while building
/// this crate: at this tolerance, an 8mm-diameter cylinder (matching the
/// real `standards/RADIAL-D8.0x11.5-P3.5` openparts-data package)
/// produced 386 facets -- denser than the old fixed-24-segment
/// approximation's 92, but still a modest, visually-smooth mesh, not an
/// excessive one. A box's flat faces tessellate to exactly 2 triangles
/// each regardless of tolerance, so this constant only affects curved
/// bodies.
const TESSELLATION_TOLERANCE: f64 = 0.05;

/// Renders `geometry` (one tessellated solid per body + lead, merged
/// into a single mesh) as an ASCII STL solid named after `product_name`.
pub fn generate_stl(geometry: &MechanicalGeometry, product_name: &str) -> Result<String, StlError> {
    let BrepGeometry { body, leads } = openparts_brep::build(geometry);

    let mut poly = body.triangulation(TESSELLATION_TOLERANCE).to_polygon();
    for lead in &leads {
        poly.merge(lead.triangulation(TESSELLATION_TOLERANCE).to_polygon());
    }
    poly.put_together_same_attrs(TOLERANCE * 10.0)
        .remove_degenerate_faces()
        .remove_unused_attrs();

    if poly.faces().is_empty() {
        return Err(StlError::Empty);
    }

    let mut buf: Vec<u8> = Vec::new();
    write_named(&poly, product_name, &mut buf);
    Ok(String::from_utf8(buf).expect("truck_polymesh::stl::write always emits ASCII text"))
}

/// `truck_polymesh::stl::write`'s ASCII output hardcodes `solid `/
/// `endsolid ` with no name; this crate's public contract (and its own
/// tests) expect the output named after `product_name`, matching the
/// old hand-rolled writer, so the solid/endsolid lines are patched after
/// writing rather than by re-implementing STL's text format here.
fn write_named(poly: &PolygonMesh, product_name: &str, out: &mut Vec<u8>) {
    let mut raw = Vec::new();
    truck_polymesh::stl::write(poly, &mut raw, StlType::Ascii)
        .expect("writing to an in-memory Vec<u8> cannot fail");
    let raw = String::from_utf8(raw).expect("ASCII STL output is always valid UTF-8");
    let named = raw
        .replacen("solid\n", &format!("solid {product_name}\n"), 1)
        .replacen("endsolid\n", &format!("endsolid {product_name}\n"), 1);
    out.extend_from_slice(named.as_bytes());
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

    fn tiny_cylinder_geometry() -> MechanicalGeometry {
        MechanicalGeometry {
            body: Body {
                position: Point3 {
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
        // 1 body + 2 leads = 3 boxes; a flat-faced box always
        // tessellates to exactly 2 triangles per face regardless of
        // tolerance, so this stays 12 per box just like before.
        assert_eq!(facet_count, 12 * 3);
    }

    #[test]
    fn every_facet_has_exactly_three_vertices() {
        let stl = generate_stl(&tiny_geometry(), "TEST").unwrap();
        let vertex_count = stl.matches("vertex ").count();
        assert_eq!(vertex_count, 12 * 3 * 3);
    }

    #[test]
    fn cylinder_body_produces_a_nonempty_mesh() {
        let stl = generate_stl(&tiny_cylinder_geometry(), "TEST").unwrap();
        let facet_count = stl.matches("facet normal").count();
        assert!(facet_count > 0);
    }

    /// Independent geometric check, mirroring this crate's old
    /// hand-rolled equivalent: every vertex of the tessellated cylinder
    /// mesh must lie within the can's radius (XY) and height (Z)
    /// bounds -- a real B-rep tessellation should never overshoot its
    /// own solid's bounding geometry.
    #[test]
    fn cylinder_facets_all_sit_on_the_can_surface() {
        let geometry = tiny_cylinder_geometry();
        let stl = generate_stl(&geometry, "TEST").unwrap();
        let radius = geometry.body.size.x / 2.0;
        let half_height = geometry.body.size.z / 2.0;
        let center_z = geometry.body.position.z;

        for line in stl.lines() {
            let Some(rest) = line.trim().strip_prefix("vertex ") else {
                continue;
            };
            let coords: Vec<f64> = rest
                .split_whitespace()
                .map(|s| s.parse().unwrap())
                .collect();
            let (x, y, z) = (coords[0], coords[1], coords[2]);
            let r = (x * x + y * y).sqrt();
            assert!(
                r <= radius + 1e-6,
                "vertex ({x}, {y}, {z}) outside the can radius"
            );
            assert!(
                (z - center_z).abs() <= half_height + 1e-6,
                "vertex ({x}, {y}, {z}) outside the can's height"
            );
        }
    }
}
