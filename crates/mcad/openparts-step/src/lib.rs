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

        // 6 faces, vertices in outward-normal order (right-hand rule: the
        // loop's winding, viewed from outside the box, must match the
        // declared normal below -- otherwise the face's topology
        // contradicts its own surface orientation, which OpenCascade-based
        // importers like KiCad's silently reject as an invalid solid
        // instead of raising a parse error).
        let bottom = self.face(
            [
                edge_between(&edges, 0, 3),
                edge_between(&edges, 3, 2),
                edge_between(&edges, 2, 1),
                edge_between(&edges, 1, 0),
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

    /// Independent geometric consistency check: for every ADVANCED_FACE in
    /// the output, walks its EDGE_LOOP (respecting each ORIENTED_EDGE's
    /// sense) to get the polygon's vertices, computes the normal implied
    /// by that winding order via the right-hand rule, and checks it points
    /// the same way as the face's own declared surface normal
    /// (AXIS2_PLACEMENT_3D's axis direction).
    ///
    /// This parses the emitted STEP text with logic entirely separate from
    /// `StepWriter` itself -- unlike the entity-count checks above, a
    /// syntactically well-formed but topologically inconsistent face
    /// (loop winding disagreeing with its declared normal) fails this
    /// test. Real STEP consumers built on OpenCascade (including KiCad's
    /// 3D viewer) can silently refuse to render such a face instead of
    /// raising a parse error, which is exactly what an inverted winding on
    /// `write_box`'s bottom face used to cause.
    #[test]
    fn every_face_winding_matches_its_declared_normal() {
        let step = generate_step(&tiny_geometry(), "TEST").unwrap();
        let entities = parse_entities(&step);

        let advanced_faces: Vec<(&u32, &(String, Vec<String>))> = entities
            .iter()
            .filter(|(_, (ty, _))| ty == "ADVANCED_FACE")
            .collect();
        assert_eq!(advanced_faces.len(), 6 * 3);

        for (face_id, (_, face_args)) in advanced_faces {
            let bound_id = parse_ref(strip_outer_parens(&face_args[1]));
            let plane_id = parse_ref(&face_args[2]);

            let (_, bound_args) = &entities[&bound_id];
            let loop_id = parse_ref(&bound_args[1]);

            let (_, loop_args) = &entities[&loop_id];
            let oe_ids: Vec<u32> = split_top_level(strip_outer_parens(&loop_args[1]))
                .iter()
                .map(|s| parse_ref(s))
                .collect();

            let mut polygon: Vec<[f64; 3]> = Vec::new();
            for oe_id in &oe_ids {
                let (_, oe_args) = &entities[oe_id];
                let edge_id = parse_ref(&oe_args[3]);
                let sense = oe_args[4].trim() == ".T.";

                let (_, edge_args) = &entities[&edge_id];
                let v0 = parse_ref(&edge_args[1]);
                let v1 = parse_ref(&edge_args[2]);
                let start_vertex = if sense { v0 } else { v1 };

                let (_, vp_args) = &entities[&start_vertex];
                let point_id = parse_ref(&vp_args[1]);
                let (_, pt_args) = &entities[&point_id];
                polygon.push(parse_point(&pt_args[1]));
            }

            assert!(polygon.len() >= 3, "face {face_id} has a degenerate loop");
            let e1 = sub(polygon[1], polygon[0]);
            let e2 = sub(polygon[2], polygon[1]);
            let computed_normal = cross(e1, e2);

            let (_, plane_args) = &entities[&plane_id];
            let axis_id = parse_ref(&plane_args[1]);
            let (_, axis_args) = &entities[&axis_id];
            let normal_dir_id = parse_ref(&axis_args[2]);
            let (_, dir_args) = &entities[&normal_dir_id];
            let declared_normal = parse_point(&dir_args[1]);

            let agreement = dot(computed_normal, declared_normal);
            assert!(
                agreement > 0.0,
                "face {face_id}: loop winding implies normal {computed_normal:?}, \
                 which disagrees with the declared surface normal {declared_normal:?} \
                 (dot = {agreement})"
            );
        }
    }

    fn parse_entities(step: &str) -> std::collections::HashMap<u32, (String, Vec<String>)> {
        let mut entities = std::collections::HashMap::new();
        for line in step.lines() {
            let line = line.trim();
            if !line.starts_with('#') {
                continue;
            }
            let Some(eq) = line.find(" = ") else {
                continue;
            };
            let Ok(id) = line[1..eq].parse::<u32>() else {
                continue;
            };
            let rest = line[eq + 3..].trim_end_matches(';');
            let Some(paren) = rest.find('(') else {
                continue;
            };
            let type_name = rest[..paren].to_string();
            if type_name.is_empty() {
                // Compound unit/context entities like
                // "(GEOMETRIC_REPRESENTATION_CONTEXT(3) ...)" -- not
                // needed for face-winding checks.
                continue;
            }
            let args = split_top_level(strip_outer_parens(&rest[paren..]));
            entities.insert(id, (type_name, args));
        }
        entities
    }

    fn strip_outer_parens(s: &str) -> &str {
        let s = s.trim();
        &s[1..s.len() - 1]
    }

    fn split_top_level(s: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut depth = 0i32;
        let mut current = String::new();
        for c in s.chars() {
            match c {
                '(' => {
                    depth += 1;
                    current.push(c);
                }
                ')' => {
                    depth -= 1;
                    current.push(c);
                }
                ',' if depth == 0 => {
                    parts.push(current.trim().to_string());
                    current.clear();
                }
                _ => current.push(c),
            }
        }
        if !current.trim().is_empty() {
            parts.push(current.trim().to_string());
        }
        parts
    }

    fn parse_ref(s: &str) -> u32 {
        s.trim().trim_start_matches('#').parse().unwrap()
    }

    fn parse_point(s: &str) -> [f64; 3] {
        let inner = strip_outer_parens(s.trim());
        let parts: Vec<f64> = inner
            .split(',')
            .map(|p| p.trim().parse().unwrap())
            .collect();
        [parts[0], parts[1], parts[2]]
    }

    fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
}
