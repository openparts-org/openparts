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
        // LINE's origin must be a CARTESIAN_POINT, not a VERTEX_POINT --
        // `v0` is the topological vertex id, which is a different entity
        // type. Emitting a fresh point here (rather than trying to reuse
        // one) keeps this function's signature simple and costs only one
        // extra small entity per edge.
        let line_origin_id = self.point(p0);
        let line_id = self.emit(&format!("LINE('', #{line_origin_id}, #{vector_id})"));
        self.emit(&format!("EDGE_CURVE('', #{v0}, #{v1}, #{line_id}, .T.)"))
    }

    /// A planar face from N (edge_id, same_sense) pairs, in order --
    /// N=4 for every box/prism-side quad, N=`segments` for a prism's
    /// two polygonal end caps.
    fn face(&mut self, edges: &[(u32, bool)], origin: [f64; 3], normal: [f64; 3]) -> u32 {
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

    /// Attaches an RGB (0.0-1.0 per channel) color to `target_id` (a
    /// MANIFOLD_SOLID_BREP here) via a STYLED_ITEM, matching the standard
    /// AP214 presentation-style chain real STEP exporters use for color.
    fn style(&mut self, target_id: u32, rgb: [f64; 3]) {
        let colour_id = self.emit(&format!(
            "COLOUR_RGB('', {:.6}, {:.6}, {:.6})",
            rgb[0], rgb[1], rgb[2]
        ));
        let fill_colour_id = self.emit(&format!("FILL_AREA_STYLE_COLOUR('', #{colour_id})"));
        let fill_area_style_id = self.emit(&format!("FILL_AREA_STYLE('', (#{fill_colour_id}))"));
        let surface_fill_id = self.emit(&format!("SURFACE_STYLE_FILL_AREA(#{fill_area_style_id})"));
        let side_style_id = self.emit(&format!("SURFACE_SIDE_STYLE('', (#{surface_fill_id}))"));
        let usage_id = self.emit(&format!("SURFACE_STYLE_USAGE(.BOTH., #{side_style_id})"));
        let assignment_id = self.emit(&format!("PRESENTATION_STYLE_ASSIGNMENT((#{usage_id}))"));
        self.emit(&format!(
            "STYLED_ITEM('', (#{assignment_id}), #{target_id})"
        ));
    }

    /// Writes one axis-aligned box centered at `center` with the given
    /// `size` and RGB `color`, returning the id of its
    /// MANIFOLD_SOLID_BREP entity.
    fn write_box(&mut self, center: [f64; 3], size: [f64; 3], color: [f64; 3]) -> u32 {
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
            &[
                edge_between(&edges, 0, 3),
                edge_between(&edges, 3, 2),
                edge_between(&edges, 2, 1),
                edge_between(&edges, 1, 0),
            ],
            coords[0],
            [0.0, 0.0, -1.0],
        );
        let top = self.face(
            &[
                edge_between(&edges, 4, 5),
                edge_between(&edges, 5, 6),
                edge_between(&edges, 6, 7),
                edge_between(&edges, 7, 4),
            ],
            coords[4],
            [0.0, 0.0, 1.0],
        );
        let front = self.face(
            &[
                edge_between(&edges, 0, 1),
                edge_between(&edges, 1, 5),
                edge_between(&edges, 5, 4),
                edge_between(&edges, 4, 0),
            ],
            coords[0],
            [0.0, -1.0, 0.0],
        );
        let back = self.face(
            &[
                edge_between(&edges, 2, 3),
                edge_between(&edges, 3, 7),
                edge_between(&edges, 7, 6),
                edge_between(&edges, 6, 2),
            ],
            coords[2],
            [0.0, 1.0, 0.0],
        );
        let left = self.face(
            &[
                edge_between(&edges, 3, 0),
                edge_between(&edges, 0, 4),
                edge_between(&edges, 4, 7),
                edge_between(&edges, 7, 3),
            ],
            coords[3],
            [-1.0, 0.0, 0.0],
        );
        let right = self.face(
            &[
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
        let brep_id = self.emit(&format!("MANIFOLD_SOLID_BREP('', #{shell_id})"));
        self.style(brep_id, color);
        brep_id
    }

    /// Writes a regular `segments`-sided prism approximating a cylinder
    /// (`openparts_mcad::BodyShape::Cylinder`) centered at `center`,
    /// with the given `diameter`/`height` and RGB `color`, returning
    /// the id of its MANIFOLD_SOLID_BREP entity. Geometrically the same
    /// technique as `write_box` (N=4 is exactly `write_box`'s own
    /// shape: N sides + 2 caps), generalized to N sides using
    /// `openparts_mcad::cylinder_ring` for the vertex positions -- see
    /// that function's docs for why the segment count lives here, not
    /// in the shared geometry model.
    fn write_prism(
        &mut self,
        center: [f64; 3],
        diameter: f64,
        height: f64,
        segments: u32,
        color: [f64; 3],
    ) -> u32 {
        let (cx, cy, cz) = (center[0], center[1], center[2]);
        let dz = height / 2.0;
        let n = segments as usize;
        let ring = openparts_mcad::cylinder_ring(diameter, segments);

        let bottom_coords: Vec<[f64; 3]> = ring
            .iter()
            .map(|&(x, y)| [cx + x, cy + y, cz - dz])
            .collect();
        let top_coords: Vec<[f64; 3]> = ring
            .iter()
            .map(|&(x, y)| [cx + x, cy + y, cz + dz])
            .collect();

        let bottom_verts: Vec<u32> = bottom_coords
            .iter()
            .map(|&p| {
                let point_id = self.point(p);
                self.vertex(point_id)
            })
            .collect();
        let top_verts: Vec<u32> = top_coords
            .iter()
            .map(|&p| {
                let point_id = self.point(p);
                self.vertex(point_id)
            })
            .collect();

        // Ring edge i goes from index i to index (i+1) % n; vertical
        // edge i goes from bottom[i] to top[i] -- same indexing
        // convention `write_box` uses for its 4-sided case.
        let bottom_edges: Vec<u32> = (0..n)
            .map(|i| {
                let j = (i + 1) % n;
                self.edge(
                    bottom_verts[i],
                    bottom_coords[i],
                    bottom_verts[j],
                    bottom_coords[j],
                )
            })
            .collect();
        let top_edges: Vec<u32> = (0..n)
            .map(|i| {
                let j = (i + 1) % n;
                self.edge(top_verts[i], top_coords[i], top_verts[j], top_coords[j])
            })
            .collect();
        let vertical_edges: Vec<u32> = (0..n)
            .map(|i| {
                self.edge(
                    bottom_verts[i],
                    bottom_coords[i],
                    top_verts[i],
                    top_coords[i],
                )
            })
            .collect();

        // Side face i: bottom[i] -> bottom[i+1] -> top[i+1] -> top[i] --
        // this loop order (verified against the same right-hand-rule
        // convention `every_face_winding_matches_its_declared_normal`
        // checks) gives the correct outward radial normal, computed
        // directly rather than approximated, since it varies per
        // segment (unlike a box's exactly axis-aligned side normals).
        let mut side_faces = Vec::with_capacity(n);
        for i in 0..n {
            let j = (i + 1) % n;
            let normal = normalize3(cross3(
                sub3(bottom_coords[j], bottom_coords[i]),
                sub3(top_coords[i], bottom_coords[i]),
            ));
            let edges = [
                (bottom_edges[i], true),
                (vertical_edges[j], true),
                (top_edges[i], false),
                (vertical_edges[i], false),
            ];
            side_faces.push(self.face(&edges, bottom_coords[i], normal));
        }

        // End caps: N-gon faces, same reversed-vs-forward traversal
        // convention `write_box` uses for its bottom/top (a -Z-normal
        // face needs the ring traversed in reverse for its winding to
        // agree with that downward normal; a +Z-normal face uses the
        // ring's own forward order).
        let bottom_loop: Vec<(u32, bool)> =
            bottom_edges.iter().rev().map(|&e| (e, false)).collect();
        let bottom_face = self.face(&bottom_loop, bottom_coords[0], [0.0, 0.0, -1.0]);
        let top_loop: Vec<(u32, bool)> = top_edges.iter().map(|&e| (e, true)).collect();
        let top_face = self.face(&top_loop, top_coords[0], [0.0, 0.0, 1.0]);

        let mut faces = side_faces;
        faces.push(bottom_face);
        faces.push(top_face);
        let face_list = faces
            .iter()
            .map(|f| format!("#{f}"))
            .collect::<Vec<_>>()
            .join(", ");
        let shell_id = self.emit(&format!("CLOSED_SHELL('', ({face_list}))"));
        let brep_id = self.emit(&format!("MANIFOLD_SOLID_BREP('', #{shell_id})"));
        self.style(brep_id, color);
        brep_id
    }
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize3(v: [f64; 3]) -> [f64; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 0.0 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        v
    }
}

/// Number of sides `write_prism` uses to approximate
/// `openparts_mcad::BodyShape::Cylinder` -- see `cylinder_ring`'s docs
/// for why this constant lives here (an output-format rendering
/// choice) rather than in the shared geometry model.
const STEP_CYLINDER_SEGMENTS: u32 = 24;

/// Dark charcoal, matching real epoxy mold compound IC body resin.
const BODY_COLOR: [f64; 3] = [0.15, 0.15, 0.15];
/// Metallic silver, matching exposed leads/pads.
const LEAD_COLOR: [f64; 3] = [0.75, 0.75, 0.75];

/// Renders `geometry` (one box per body + lead) as a complete STEP AP214
/// file named after `product_name`.
pub fn generate_step(
    geometry: &MechanicalGeometry,
    product_name: &str,
) -> Result<String, StepError> {
    let mut w = StepWriter::new();

    let mut brep_ids = Vec::new();
    let body_center = [
        geometry.body.position.x,
        geometry.body.position.y,
        geometry.body.position.z,
    ];
    brep_ids.push(match geometry.body.shape {
        openparts_mcad::BodyShape::Box => w.write_box(
            body_center,
            [
                geometry.body.size.x,
                geometry.body.size.y,
                geometry.body.size.z,
            ],
            BODY_COLOR,
        ),
        openparts_mcad::BodyShape::Cylinder => w.write_prism(
            body_center,
            geometry.body.size.x, // diameter (== size.y)
            geometry.body.size.z, // height
            STEP_CYLINDER_SEGMENTS,
            BODY_COLOR,
        ),
    });
    for lead in &geometry.leads {
        brep_ids.push(w.write_box(
            [lead.position.x, lead.position.y, lead.position.z],
            [lead.size.x, lead.size.y, lead.size.z],
            LEAD_COLOR,
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

    /// A radial-capacitor-shaped fixture: a `BodyShape::Cylinder` body
    /// with 2 through-hole leads, exercising `write_prism` the same
    /// way `tiny_geometry` exercises `write_box`.
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
            leads: vec![
                Lead {
                    number: "1".into(),
                    position: Point3 {
                        x: -1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    size: Size3 {
                        x: 1.6,
                        y: 1.6,
                        z: 3.0,
                    },
                    mounting: Mounting::ThroughHole,
                    drill: Some(0.8),
                },
                Lead {
                    number: "2".into(),
                    position: Point3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    size: Size3 {
                        x: 1.6,
                        y: 1.6,
                        z: 3.0,
                    },
                    mounting: Mounting::ThroughHole,
                    drill: Some(0.8),
                },
            ],
            markers: vec![Marker {
                kind: MarkerKind::NegativeStripe,
                position: Point3 {
                    x: 1.0,
                    y: 0.0,
                    z: 11.0,
                },
            }],
        }
    }

    #[test]
    fn every_solid_has_a_styled_color() {
        let step = generate_step(&tiny_geometry(), "TEST").unwrap();
        let styled_item_count = step.matches("STYLED_ITEM(").count();
        let colour_count = step.matches("COLOUR_RGB(").count();
        // 1 body + 2 leads = 3 boxes, one STYLED_ITEM/COLOUR_RGB pair each.
        assert_eq!(styled_item_count, 3);
        assert_eq!(colour_count, 3);
        assert!(step.contains(&format!(
            "COLOUR_RGB('', {:.6}, {:.6}, {:.6})",
            BODY_COLOR[0], BODY_COLOR[1], BODY_COLOR[2]
        )));
        assert!(step.contains(&format!(
            "COLOUR_RGB('', {:.6}, {:.6}, {:.6})",
            LEAD_COLOR[0], LEAD_COLOR[1], LEAD_COLOR[2]
        )));
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

    #[test]
    fn cylinder_body_produces_one_brep_with_segments_plus_two_faces() {
        let step = generate_step(&tiny_cylinder_geometry(), "TEST").unwrap();
        let brep_count = step.matches("MANIFOLD_SOLID_BREP(").count();
        // 1 cylinder body + 2 box leads = 3 solids.
        assert_eq!(brep_count, 3);
        let face_count = step.matches("ADVANCED_FACE(").count();
        // A box is this same "N sides + 2 caps" formula's N=4 case
        // (4+2=6, matching `has_six_faces_per_box` above exactly).
        assert_eq!(face_count, (STEP_CYLINDER_SEGMENTS as usize + 2) + 6 * 2);
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
        let advanced_face_count = entities
            .values()
            .filter(|(ty, _)| ty == "ADVANCED_FACE")
            .count();
        assert_eq!(advanced_face_count, 6 * 3);
        assert_winding_is_consistent(&entities);
    }

    /// Same check as `every_face_winding_matches_its_declared_normal`,
    /// but against a `BodyShape::Cylinder` body -- `write_prism`'s side
    /// faces compute their own normal per segment (unlike a box's
    /// hardcoded axis-aligned ones), so this is the one case where a
    /// declared-vs-computed-normal mismatch is a realistic risk, not
    /// just a defensive check.
    #[test]
    fn cylinder_face_winding_matches_its_declared_normal() {
        let step = generate_step(&tiny_cylinder_geometry(), "TEST").unwrap();
        let entities = parse_entities(&step);
        // segments side faces + 2 caps for the cylinder body, plus 6
        // for each of the 2 box leads.
        let advanced_face_count = entities
            .values()
            .filter(|(ty, _)| ty == "ADVANCED_FACE")
            .count();
        assert_eq!(
            advanced_face_count,
            (STEP_CYLINDER_SEGMENTS as usize + 2) + 6 * 2
        );
        assert_winding_is_consistent(&entities);
    }

    fn assert_winding_is_consistent(
        entities: &std::collections::HashMap<u32, (String, Vec<String>)>,
    ) {
        let advanced_faces: Vec<(&u32, &(String, Vec<String>))> = entities
            .iter()
            .filter(|(_, (ty, _))| ty == "ADVANCED_FACE")
            .collect();

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

    /// Independent type-checking pass: STEP entity references are just
    /// integer ids with no type information at the reference site, so
    /// nothing about `#emit`-based generation prevents accidentally
    /// pointing at an entity of the wrong kind. This caught a real bug --
    /// `LINE`'s origin argument must be a `CARTESIAN_POINT` (pure
    /// geometry), not a `VERTEX_POINT` (topology) -- which was
    /// syntactically valid STEP and passed every check above, but made
    /// every edge's underlying curve fail to construct in a real STEP
    /// reader (confirmed against OpenCascade), which cascaded into every
    /// face, shell, and solid failing to form. No automated Rust test
    /// caught that until this one was written in response.
    #[test]
    fn line_entities_reference_a_cartesian_point_not_a_vertex_point() {
        let step = generate_step(&tiny_geometry(), "TEST").unwrap();
        let entities = parse_entities(&step);

        let lines: Vec<(&u32, &(String, Vec<String>))> = entities
            .iter()
            .filter(|(_, (ty, _))| ty == "LINE")
            .collect();
        assert!(!lines.is_empty());

        for (line_id, (_, line_args)) in lines {
            let origin_id = parse_ref(&line_args[1]);
            let (origin_type, _) = &entities[&origin_id];
            assert_eq!(
                origin_type, "CARTESIAN_POINT",
                "LINE {line_id}'s origin (#{origin_id}) is a {origin_type}, not a \
                 CARTESIAN_POINT -- STEP readers need a pure geometric point here, \
                 not a topological VERTEX_POINT"
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
