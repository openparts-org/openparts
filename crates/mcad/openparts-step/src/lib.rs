//! openparts-step: Mechanical Geometry -> STEP (ISO-10303-21 AP214) text.
//! No CAD library dependency -- every entity (points, lines, planar
//! faces, closed shells) is hand-written. Geometry is approximated as
//! axis-aligned boxes (one for the body, one per lead); this is
//! structurally valid but not datasheet-precision geometry, matching the
//! Architecture Specification's non-goal of photorealistic reproduction.
//!
//! NOTE (Testing and Quality Specification section 9): this crate has no
//! independent STEP reader, so output is verified only by structural
//! checks (entity counts, presence of the expected entity kinds), never
//! claimed as "read-back verified". An independent read-back check is
//! not yet implemented.

use openparts_mcad::MechanicalGeometry;

#[derive(Debug, thiserror::Error)]
pub enum StepError {
    #[error("geometry has no boxes to export (no body?)")]
    Empty,
}

struct StepWriter {
    next_id: u32,
    lines: Vec<String>,
}

impl StepWriter {
    fn new() -> Self {
        StepWriter {
            next_id: 1,
            lines: Vec::new(),
        }
    }

    fn alloc(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn emit(&mut self, body: &str) -> u32 {
        let id = self.alloc();
        self.lines.push(format!("#{id} = {body};"));
        id
    }

    fn point(&mut self, p: [f64; 3]) -> u32 {
        self.emit(&format!(
            "CARTESIAN_POINT('', ({:.6}, {:.6}, {:.6}))",
            p[0], p[1], p[2]
        ))
    }

    fn vertex(&mut self, point_id: u32) -> u32 {
        self.emit(&format!("VERTEX_POINT('', #{point_id})"))
    }

    /// One straight edge from vertex `v0` (at point `p0`) to vertex `v1`
    /// (at point `p1`).
    fn edge(&mut self, v0: u32, p0: [f64; 3], v1: u32, p1: [f64; 3]) -> u32 {
        let dir = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let unit = if len > 0.0 {
            [dir[0] / len, dir[1] / len, dir[2] / len]
        } else {
            [1.0, 0.0, 0.0]
        };
        let direction_id = self.emit(&format!(
            "DIRECTION('', ({:.6}, {:.6}, {:.6}))",
            unit[0], unit[1], unit[2]
        ));
        let vector_id = self.emit(&format!("VECTOR('', #{direction_id}, {len:.6})"));
        let line_id = self.emit(&format!("LINE('', #{v0}, #{vector_id})"));
        self.emit(&format!("EDGE_CURVE('', #{v0}, #{v1}, #{line_id}, .T.)"))
    }

    /// A planar quad face from 4 (edge_id, same_sense) pairs, in order.
    fn face(&mut self, edges: [(u32, bool); 4], origin: [f64; 3], normal: [f64; 3]) -> u32 {
        let oriented: Vec<u32> = edges
            .iter()
            .map(|(edge_id, same_sense)| {
                let sense = if *same_sense { ".T." } else { ".F." };
                self.emit(&format!("ORIENTED_EDGE('', *, *, #{edge_id}, {sense})"))
            })
            .collect();
        let loop_list = oriented
            .iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(", ");
        let edge_loop_id = self.emit(&format!("EDGE_LOOP('', ({loop_list}))"));
        let bound_id = self.emit(&format!("FACE_OUTER_BOUND('', #{edge_loop_id}, .T.)"));

        let origin_id = self.point(origin);
        let normal_id = self.emit(&format!(
            "DIRECTION('', ({:.6}, {:.6}, {:.6}))",
            normal[0], normal[1], normal[2]
        ));
        // Arbitrary reference direction not parallel to normal; the exact
        // in-plane rotation doesn't matter for a flat rectangular face.
        let ref_dir = if normal[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let ref_id = self.emit(&format!(
            "DIRECTION('', ({:.6}, {:.6}, {:.6}))",
            ref_dir[0], ref_dir[1], ref_dir[2]
        ));
        let axis_id = self.emit(&format!(
            "AXIS2_PLACEMENT_3D('', #{origin_id}, #{normal_id}, #{ref_id})"
        ));
        let plane_id = self.emit(&format!("PLANE('', #{axis_id})"));
        self.emit(&format!(
            "ADVANCED_FACE('', (#{bound_id}), #{plane_id}, .T.)"
        ))
    }

    /// Writes one axis-aligned box centered at `center` with the given
    /// `size`, returning the id of its MANIFOLD_SOLID_BREP entity.
    fn write_box(&mut self, center: [f64; 3], size: [f64; 3]) -> u32 {
        let (cx, cy, cz) = (center[0], center[1], center[2]);
        let (dx, dy, dz) = (size[0] / 2.0, size[1] / 2.0, size[2] / 2.0);

        // P1..P4 bottom (z-), P5..P8 top (z+), matching corners stacked.
        let coords: [[f64; 3]; 8] = [
            [cx - dx, cy - dy, cz - dz], // P1
            [cx + dx, cy - dy, cz - dz], // P2
            [cx + dx, cy + dy, cz - dz], // P3
            [cx - dx, cy + dy, cz - dz], // P4
            [cx - dx, cy - dy, cz + dz], // P5
            [cx + dx, cy - dy, cz + dz], // P6
            [cx + dx, cy + dy, cz + dz], // P7
            [cx - dx, cy + dy, cz + dz], // P8
        ];
        let points: Vec<u32> = coords.iter().map(|p| self.point(*p)).collect();
        let verts: Vec<u32> = points.iter().map(|&p| self.vertex(p)).collect();

        // 12 edges: 4 bottom, 4 top, 4 vertical.
        let edge_defs: [(usize, usize); 12] = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0), // bottom loop
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4), // top loop
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7), // verticals
        ];
        let mut edges = std::collections::HashMap::new();
        for (a, b) in edge_defs {
            let id = self.edge(verts[a], coords[a], verts[b], coords[b]);
            edges.insert((a, b), id);
        }
        let edge_between = |edges: &std::collections::HashMap<(usize, usize), u32>,
                            a: usize,
                            b: usize|
         -> (u32, bool) {
            if let Some(&id) = edges.get(&(a, b)) {
                (id, true)
            } else {
                (edges[&(b, a)], false)
            }
        };

        // 6 faces, vertices in outward-normal order.
        let bottom = self.face(
            [
                edge_between(&edges, 0, 1),
                edge_between(&edges, 1, 2),
                edge_between(&edges, 2, 3),
                edge_between(&edges, 3, 0),
            ],
            coords[0],
            [0.0, 0.0, -1.0],
        );
        let top = self.face(
            [
                edge_between(&edges, 4, 5),
                edge_between(&edges, 5, 6),
                edge_between(&edges, 6, 7),
                edge_between(&edges, 7, 4),
            ],
            coords[4],
            [0.0, 0.0, 1.0],
        );
        let front = self.face(
            [
                edge_between(&edges, 0, 1),
                edge_between(&edges, 1, 5),
                edge_between(&edges, 5, 4),
                edge_between(&edges, 4, 0),
            ],
            coords[0],
            [0.0, -1.0, 0.0],
        );
        let back = self.face(
            [
                edge_between(&edges, 2, 3),
                edge_between(&edges, 3, 7),
                edge_between(&edges, 7, 6),
                edge_between(&edges, 6, 2),
            ],
            coords[2],
            [0.0, 1.0, 0.0],
        );
        let left = self.face(
            [
                edge_between(&edges, 3, 0),
                edge_between(&edges, 0, 4),
                edge_between(&edges, 4, 7),
                edge_between(&edges, 7, 3),
            ],
            coords[3],
            [-1.0, 0.0, 0.0],
        );
        let right = self.face(
            [
                edge_between(&edges, 1, 2),
                edge_between(&edges, 2, 6),
                edge_between(&edges, 6, 5),
                edge_between(&edges, 5, 1),
            ],
            coords[1],
            [1.0, 0.0, 0.0],
        );

        let faces = [bottom, top, front, back, left, right];
        let face_list = faces
            .iter()
            .map(|f| format!("#{f}"))
            .collect::<Vec<_>>()
            .join(", ");
        let shell_id = self.emit(&format!("CLOSED_SHELL('', ({face_list}))"));
        self.emit(&format!("MANIFOLD_SOLID_BREP('', #{shell_id})"))
    }
}

/// Renders `geometry` (one box per body + lead) as a complete STEP AP214
/// file named after `product_name`.
pub fn generate_step(
    geometry: &MechanicalGeometry,
    product_name: &str,
) -> Result<String, StepError> {
    let mut w = StepWriter::new();

    let mut brep_ids = Vec::new();
    brep_ids.push(w.write_box(
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
    ));
    for lead in &geometry.leads {
        brep_ids.push(w.write_box(
            [lead.position.x, lead.position.y, lead.position.z],
            [lead.size.x, lead.size.y, lead.size.z],
        ));
    }

    if brep_ids.is_empty() {
        return Err(StepError::Empty);
    }

    // Standard AP214 application/product/context boilerplate.
    let app_context = w.emit("APPLICATION_CONTEXT('automotive_design')");
    let _app_protocol = w.emit(&format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard', 'automotive_design', 2003, #{app_context})"
    ));
    let length_unit = w.emit("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI., .METRE.))");
    let angle_unit = w.emit("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($, .RADIAN.))");
    let solid_angle_unit = w.emit("(NAMED_UNIT(*) SI_UNIT($, .STERADIAN.) SOLID_ANGLE_UNIT())");
    let uncertainty = w.emit(&format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.0E-6), #{length_unit}, 'distance_accuracy_value', 'confusion accuracy')"
    ));
    let geom_context = w.emit(&format!(
        "(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length_unit}, #{angle_unit}, #{solid_angle_unit})) REPRESENTATION_CONTEXT('Context #1', '3D Context with UNIT and UNCERTAINTY'))"
    ));

    let product_context = w.emit(&format!(
        "PRODUCT_CONTEXT('', #{app_context}, 'mechanical')"
    ));
    let product = w.emit(&format!(
        "PRODUCT('{product_name}', '{product_name}', '', (#{product_context}))"
    ));
    let pdf = w.emit(&format!("PRODUCT_DEFINITION_FORMATION('', '', #{product})"));
    let pd_context = w.emit(&format!(
        "PRODUCT_DEFINITION_CONTEXT('part definition', #{app_context}, 'design')"
    ));
    let product_definition = w.emit(&format!(
        "PRODUCT_DEFINITION('design', '', #{pdf}, #{pd_context})"
    ));
    let pds = w.emit(&format!(
        "PRODUCT_DEFINITION_SHAPE('', '', #{product_definition})"
    ));

    let brep_list = brep_ids
        .iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(", ");
    let absr = w.emit(&format!(
        "ADVANCED_BREP_SHAPE_REPRESENTATION('', ({brep_list}), #{geom_context})"
    ));
    w.emit(&format!("SHAPE_DEFINITION_REPRESENTATION(#{pds}, #{absr})"));

    let mut out = String::new();
    out.push_str("ISO-10303-21;\n");
    out.push_str("HEADER;\n");
    out.push_str("FILE_DESCRIPTION((''), '2;1');\n");
    out.push_str(&format!(
        "FILE_NAME('{product_name}.step', '2026-01-01T00:00:00', ('OpenParts'), (''), 'OpenParts STEP Exporter', '', '');\n"
    ));
    out.push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 3 1 1 }'));\n");
    out.push_str("ENDSEC;\n");
    out.push_str("DATA;\n");
    for line in &w.lines {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("ENDSEC;\n");
    out.push_str("END-ISO-10303-21;\n");

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
    fn produces_a_well_formed_step_file() {
        let step = generate_step(&tiny_geometry(), "TEST").unwrap();
        assert!(step.starts_with("ISO-10303-21;\n"));
        assert!(step.trim_end().ends_with("END-ISO-10303-21;"));
        assert!(step.contains("HEADER;"));
        assert!(step.contains("DATA;"));
    }

    #[test]
    fn has_one_manifold_solid_brep_per_box() {
        let step = generate_step(&tiny_geometry(), "TEST").unwrap();
        let brep_count = step.matches("MANIFOLD_SOLID_BREP(").count();
        // 1 body + 2 leads = 3 boxes.
        assert_eq!(brep_count, 3);
    }

    #[test]
    fn has_six_faces_per_box() {
        let step = generate_step(&tiny_geometry(), "TEST").unwrap();
        let face_count = step.matches("ADVANCED_FACE(").count();
        assert_eq!(face_count, 6 * 3);
    }
}
