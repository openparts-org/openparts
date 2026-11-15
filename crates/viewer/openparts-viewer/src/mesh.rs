//! The model's triangles: parsed from the server's 3MF (one colour per
//! body, SolidCraft's materials) or, failing that, its binary/ASCII STL,
//! in the generator's frame (millimetres, Z up; `frame.rs` moves it
//! into the Placement Model's, the footprint's).

use glam::Vec3;
use std::collections::HashMap;
use std::io::Read;

/// The colour the uncoloured STL is drawn in (linear RGB of #8f97a3).
pub const STL_GREY: [f32; 3] = [0.275, 0.309, 0.366];

/// A triangle soup: three entries per triangle in every array.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    /// Per corner, smoothed across edges flatter than `SMOOTH_ANGLE`.
    pub normals: Vec<Vec3>,
    /// Per corner, linear RGB.
    pub colors: Vec<[f32; 3]>,
}

/// Neighbouring faces closer than this (degrees) share a smooth normal;
/// sharper edges stay crisp (and are snap-to edges, see `snap.rs`).
pub const SMOOTH_ANGLE: f32 = 35.0;

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.positions.len() / 3
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// (min, max) corners; `None` when empty.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let first = *self.positions.first()?;
        Some(
            self.positions
                .iter()
                .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p))),
        )
    }

    fn push_triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: [f32; 3]) {
        // Degenerate triangles add nothing and break normals.
        if (b - a).cross(c - a).length_squared() <= 1e-18 {
            return;
        }
        self.positions.extend([a, b, c]);
        self.colors.extend([color; 3]);
    }

    /// Fills `normals`: each corner averages the (area-weighted) normals
    /// of the faces meeting at its position that are within
    /// `SMOOTH_ANGLE` of its own face.
    pub fn compute_normals(&mut self) {
        let faces: Vec<Vec3> = self
            .positions
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| (t[1] - t[0]).cross(t[2] - t[0]))
            .collect();
        let mut at: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
        for (i, p) in self.positions.iter().enumerate() {
            at.entry(key(*p)).or_default().push(i / 3);
        }
        let cos = SMOOTH_ANGLE.to_radians().cos();
        self.normals = self
            .positions
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let own = faces[i / 3].normalize_or_zero();
                let mut sum = Vec3::ZERO;
                for &f in &at[&key(*p)] {
                    if faces[f].normalize_or_zero().dot(own) >= cos {
                        sum += faces[f];
                    }
                }
                let n = sum.normalize_or_zero();
                if n == Vec3::ZERO {
                    own
                } else {
                    n
                }
            })
            .collect();
    }
}

/// Positions closer than 1 µm are the same vertex.
pub(crate) fn key(p: Vec3) -> [i64; 3] {
    [
        (p.x as f64 * 1e3).round() as i64,
        (p.y as f64 * 1e3).round() as i64,
        (p.z as f64 * 1e3).round() as i64,
    ]
}

/// sRGB "#RRGGBB[AA]" -> linear RGB.
pub fn parse_color(s: &str) -> Option<[f32; 3]> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let channel = |i: usize| -> Option<f32> {
        let v = u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()? as f32 / 255.0;
        Some(if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        })
    };
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// A binary or ASCII STL, in `STL_GREY`.
pub fn parse_stl(bytes: &[u8]) -> Result<Mesh, String> {
    let mut mesh = Mesh::default();
    let binary_len = bytes
        .get(80..84)
        .map(|n| 84 + 50 * u32::from_le_bytes([n[0], n[1], n[2], n[3]]) as usize);
    if binary_len == Some(bytes.len()) {
        for t in bytes[84..].as_chunks::<50>().0 {
            let v = |o: usize| {
                let f = |k: usize| {
                    f32::from_le_bytes([t[o + k], t[o + k + 1], t[o + k + 2], t[o + k + 3]])
                };
                Vec3::new(f(0), f(4), f(8))
            };
            mesh.push_triangle(v(12), v(24), v(36), STL_GREY);
        }
    } else {
        let text = std::str::from_utf8(bytes).map_err(|_| "STL: neither binary nor ASCII")?;
        let mut corners = Vec::new();
        for line in text.lines() {
            let mut words = line.split_whitespace();
            if words.next() == Some("vertex") {
                let c: Vec<f32> = words.filter_map(|w| w.parse().ok()).collect();
                if c.len() != 3 {
                    return Err("STL: bad vertex".into());
                }
                corners.push(Vec3::new(c[0], c[1], c[2]));
                if corners.len() == 3 {
                    mesh.push_triangle(corners[0], corners[1], corners[2], STL_GREY);
                    corners.clear();
                }
            }
        }
    }
    if mesh.is_empty() {
        return Err("STL: no triangles".into());
    }
    mesh.compute_normals();
    Ok(mesh)
}

/// A 3MF package: every build item's object (and the components it is
/// made of), with the item's transform, coloured by its base material.
pub fn parse_3mf(bytes: &[u8]) -> Result<Mesh, String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("3MF: {e}"))?;
    let model_path = start_part(&mut archive).unwrap_or_else(|| "3D/3dmodel.model".to_string());
    let mut xml = String::new();
    archive
        .by_name(model_path.trim_start_matches('/'))
        .map_err(|e| format!("3MF: {model_path}: {e}"))?
        .read_to_string(&mut xml)
        .map_err(|e| format!("3MF: {e}"))?;
    let mesh = parse_model_xml(&xml)?;
    if mesh.is_empty() {
        return Err("3MF: no triangles".into());
    }
    Ok(mesh)
}

/// The model part `_rels/.rels` points at.
fn start_part<R: Read + std::io::Seek>(archive: &mut zip::ZipArchive<R>) -> Option<String> {
    let mut rels = String::new();
    archive
        .by_name("_rels/.rels")
        .ok()?
        .read_to_string(&mut rels)
        .ok()?;
    let doc = roxmltree::Document::parse(&rels).ok()?;
    doc.descendants()
        .filter(|n| n.has_tag_name("Relationship"))
        .find(|n| n.attribute("Type").is_some_and(|t| t.ends_with("/3dmodel")))
        .and_then(|n| n.attribute("Target"))
        .map(str::to_string)
}

/// A 3MF transform: 3x4, row vectors (`p' = [x y z 1] * M`).
type Transform = [f32; 12];
const IDENTITY: Transform = [1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.];

fn parse_transform(s: Option<&str>) -> Transform {
    let Some(s) = s else { return IDENTITY };
    let v: Vec<f32> = s
        .split_whitespace()
        .filter_map(|w| w.parse().ok())
        .collect();
    v.try_into().unwrap_or(IDENTITY)
}

fn apply(m: &Transform, p: Vec3) -> Vec3 {
    Vec3::new(
        p.x * m[0] + p.y * m[3] + p.z * m[6] + m[9],
        p.x * m[1] + p.y * m[4] + p.z * m[7] + m[10],
        p.x * m[2] + p.y * m[5] + p.z * m[8] + m[11],
    )
}

/// `outer` after `inner`.
fn compose(inner: &Transform, outer: &Transform) -> Transform {
    let row = |r: usize| apply(outer, Vec3::new(inner[r], inner[r + 1], inner[r + 2]));
    let lin = |r: usize| {
        let o = row(r) - apply(outer, Vec3::ZERO);
        [o.x, o.y, o.z]
    };
    let t = apply(outer, Vec3::new(inner[9], inner[10], inner[11]));
    let (a, b, c) = (lin(0), lin(3), lin(6));
    [
        a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2], t.x, t.y, t.z,
    ]
}

/// The core-spec model document (3MF materials: `basematerials`, and
/// `colorgroup` from the materials extension).
pub fn parse_model_xml(xml: &str) -> Result<Mesh, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("3MF: {e}"))?;
    let mut palettes: HashMap<&str, Vec<[f32; 3]>> = HashMap::new();
    let mut objects: HashMap<&str, roxmltree::Node> = HashMap::new();
    for n in doc.descendants() {
        match n.tag_name().name() {
            "basematerials" | "colorgroup" => {
                let colors = n
                    .children()
                    .filter(|c| c.is_element())
                    .map(|c| {
                        c.attribute("displaycolor")
                            .or(c.attribute("color"))
                            .and_then(parse_color)
                            .unwrap_or(STL_GREY)
                    })
                    .collect();
                if let Some(id) = n.attribute("id") {
                    palettes.insert(id, colors);
                }
            }
            "object" => {
                if let Some(id) = n.attribute("id") {
                    objects.insert(id, n);
                }
            }
            _ => {}
        }
    }
    let color = |pid: Option<&str>, index: Option<&str>| -> Option<[f32; 3]> {
        let i: usize = index?.parse().ok()?;
        palettes.get(pid?)?.get(i).copied()
    };
    let mut mesh = Mesh::default();
    // Objects drawn as (object id, transform, inherited colour).
    let mut todo: Vec<(&str, Transform, [f32; 3], u32)> = doc
        .descendants()
        .filter(|n| n.has_tag_name("item"))
        .filter_map(|n| {
            Some((
                n.attribute("objectid")?,
                parse_transform(n.attribute("transform")),
                STL_GREY,
                0,
            ))
        })
        .collect();
    while let Some((id, transform, inherited, depth)) = todo.pop() {
        let Some(object) = objects.get(id) else {
            continue;
        };
        if depth > 16 {
            return Err("3MF: components nest too deep".into());
        }
        let object_pid = object.attribute("pid");
        let object_color = color(object_pid, object.attribute("pindex")).unwrap_or(inherited);
        for child in object.children().filter(|c| c.is_element()) {
            match child.tag_name().name() {
                "mesh" => {
                    let mut vertices = Vec::new();
                    for v in child.descendants().filter(|v| v.has_tag_name("vertex")) {
                        let c = |a: &str| v.attribute(a).and_then(|s| s.parse::<f32>().ok());
                        let (Some(x), Some(y), Some(z)) = (c("x"), c("y"), c("z")) else {
                            return Err("3MF: bad vertex".into());
                        };
                        vertices.push(apply(&transform, Vec3::new(x, y, z)));
                    }
                    for t in child.descendants().filter(|t| t.has_tag_name("triangle")) {
                        let index = |a: &str| -> Result<Vec3, String> {
                            t.attribute(a)
                                .and_then(|s| s.parse::<usize>().ok())
                                .and_then(|i| vertices.get(i).copied())
                                .ok_or_else(|| "3MF: bad triangle".to_string())
                        };
                        let c = color(t.attribute("pid").or(object_pid), t.attribute("p1"))
                            .unwrap_or(object_color);
                        mesh.push_triangle(index("v1")?, index("v2")?, index("v3")?, c);
                    }
                }
                "components" => {
                    for c in child.children().filter(|c| c.has_tag_name("component")) {
                        if let Some(oid) = c.attribute("objectid") {
                            let inner = parse_transform(c.attribute("transform"));
                            todo.push((oid, compose(&inner, &transform), object_color, depth + 1));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    mesh.compute_normals();
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
 <resources>
  <basematerials id="1">
   <base name="Body" displaycolor="#1C1C1E"/>
   <base name="Pad" displaycolor="#FFFFFF"/>
  </basematerials>
  <object id="2" type="model" pid="1" pindex="1">
   <mesh>
    <vertices>
     <vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/>
    </vertices>
    <triangles><triangle v1="0" v2="1" v3="2"/></triangles>
   </mesh>
  </object>
  <object id="3" type="model">
   <components><component objectid="2" transform="1 0 0 0 1 0 0 0 1 10 0 0"/></components>
  </object>
 </resources>
 <build><item objectid="2"/><item objectid="3" transform="1 0 0 0 1 0 0 0 1 0 0 5"/></build>
</model>"##;

    #[test]
    fn reads_items_components_transforms_and_colours() {
        let mesh = parse_model_xml(MODEL).unwrap();
        assert_eq!(mesh.triangle_count(), 2);
        let (lo, hi) = mesh.bounds().unwrap();
        assert_eq!(lo, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(hi, Vec3::new(11.0, 1.0, 5.0));
        assert!(mesh.colors.iter().all(|c| (c[0] - 1.0).abs() < 1e-6));
        assert!(mesh.normals.iter().all(|n| (*n - Vec3::Z).length() < 1e-6));
    }

    #[test]
    fn reads_the_servers_3mf() {
        let mesh = parse_3mf(include_bytes!("../tests/rp2040.3mf")).unwrap();
        let (lo, hi) = mesh.bounds().unwrap();
        // The 7 x 7 mm QFN body, Z up from the board.
        assert!((hi.x - lo.x - 7.0).abs() < 0.01 && (hi.y - lo.y - 7.0).abs() < 0.01);
        assert!(lo.z.abs() < 1e-4 && hi.z > 0.5);
        // Black body, light terminals.
        assert!(mesh.colors.iter().any(|c| c[0] < 0.05));
        assert!(mesh.colors.iter().any(|c| c[0] > 0.5));
        assert_eq!(mesh.normals.len(), mesh.positions.len());
    }

    #[test]
    fn reads_binary_and_ascii_stl() {
        let mut bin = vec![0u8; 80];
        bin.extend(1u32.to_le_bytes());
        for f in [0.0f32, 0., 1., 0., 0., 0., 2., 0., 0., 0., 2., 0.] {
            bin.extend(f.to_le_bytes());
        }
        bin.extend([0, 0]);
        let mesh = parse_stl(&bin).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.bounds().unwrap().1, Vec3::new(2.0, 2.0, 0.0));
        let ascii = "solid x\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\n\
                     vertex 0 1 0\nendloop\nendfacet\nendsolid x\n";
        assert_eq!(parse_stl(ascii.as_bytes()).unwrap().triangle_count(), 1);
        assert!(parse_stl(b"nothing").is_err());
    }

    #[test]
    fn smooths_shallow_edges_only() {
        let mut m = Mesh::default();
        // Two faces at 90 degrees: crisp.
        m.push_triangle(Vec3::ZERO, Vec3::X, Vec3::Y, STL_GREY);
        m.push_triangle(Vec3::ZERO, Vec3::Z, Vec3::X, STL_GREY);
        m.compute_normals();
        assert!((m.normals[0] - Vec3::Z).length() < 1e-6);
        assert!((m.normals[3] - Vec3::Y).length() < 1e-6);
    }

    #[test]
    fn colours_are_linearised() {
        assert_eq!(parse_color("#FFFFFF"), Some([1.0, 1.0, 1.0]));
        assert_eq!(parse_color("#000000FF"), Some([0.0, 0.0, 0.0]));
        assert_eq!(parse_color("red"), None);
    }
}
