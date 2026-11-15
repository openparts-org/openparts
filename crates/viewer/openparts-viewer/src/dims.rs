//! Datasheet dimensions placed on the 3D model: each stated package
//! dimension (body width/length/height, overall span, pitch, lead
//! width/length) becomes a dimension line anchored on the Mechanical
//! Geometry the model itself was generated from (openparts-mcad), so the
//! lines sit exactly on the model's features.

use crate::format;
use glam::{Vec2, Vec3};
use openparts_core::{Dimension, Package};
use openparts_mcad::{Lead, MechanicalGeometry, Mounting};

/// One dimension line.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    /// What it measures; also the label's i18n key ("body_width", ...).
    pub key: &'static str,
    /// The package field's JSON pointer, for its provenance; `None` for
    /// a value measured on the model (the datasheet doesn't state it).
    pub pointer: Option<String>,
    /// The label: the datasheet value ("0.18 (0.13–0.23) mm") or the
    /// model's ("≈0.9 mm").
    pub text: String,
    /// The two feature points measured between.
    pub feature: [Vec3; 2],
    /// The dimension line's ends (offset from the features).
    pub line: [Vec3; 2],
}

impl Annotation {
    /// Where the label goes: the line's middle, pushed `offset` further
    /// away from the features so it doesn't cover the line.
    pub fn label_anchor(&self, offset: f32) -> Vec3 {
        let mid = (self.line[0] + self.line[1]) * 0.5;
        mid + self.outward() * offset
    }

    /// The unit direction from the features to the line (the extension
    /// lines' direction), or any perpendicular when they coincide.
    pub fn outward(&self) -> Vec3 {
        let d = (self.line[1] - self.line[0]).normalize_or_zero();
        let e = (self.line[0] - self.feature[0]) + (self.line[1] - self.feature[1]);
        let e = e - d * e.dot(d);
        if e.length() > 1e-6 {
            e.normalize()
        } else if d.abs().z > 0.9 {
            Vec3::X
        } else {
            d.cross(Vec3::Z).normalize_or_zero()
        }
    }

    /// The drawn segments: extension lines, the dimension line and its
    /// arrowheads, `arrow` long (world units). Short dimensions get
    /// their arrows outside, pointing in.
    pub fn segments(&self, arrow: f32) -> Vec<[Vec3; 2]> {
        let [a, b] = self.line;
        let mut s = Vec::new();
        for (f, p) in [(self.feature[0], a), (self.feature[1], b)] {
            let e = p - f;
            let l = e.length();
            if l > arrow * 0.3 {
                let u = e / l;
                s.push([f + u * (arrow * 0.3).min(l * 0.2), p + u * arrow * 0.5]);
            }
        }
        let len = (b - a).length();
        if len < 1e-6 {
            return s;
        }
        let d = (b - a) / len;
        let w = self.outward() * arrow * 0.32;
        if len > arrow * 2.4 {
            s.push([a, b]);
            for (tip, back) in [(a, d), (b, -d)] {
                s.push([tip, tip + back * arrow + w]);
                s.push([tip, tip + back * arrow - w]);
            }
        } else {
            s.push([a - d * arrow * 2.0, b + d * arrow * 2.0]);
            for (tip, back) in [(a, -d), (b, d)] {
                s.push([tip, tip + back * arrow + w]);
                s.push([tip, tip + back * arrow - w]);
            }
        }
        s
    }
}

/// The leads drawn around the body (not an exposed pad or a peg).
fn perimeter_leads(g: &MechanicalGeometry) -> Vec<&Lead> {
    let c = Vec2::new(g.body.position.x as f32, g.body.position.y as f32);
    let r = (g.body.size.x.min(g.body.size.y) as f32) * 0.15;
    g.leads
        .iter()
        .filter(|l| l.mounting != Mounting::NonPlated)
        .filter(|l| (xy(l) - c).length() > r)
        .collect()
}

fn xy(l: &Lead) -> Vec2 {
    Vec2::new(l.position.x as f32, l.position.y as f32)
}

/// Half the lead's extent along the (axis-aligned) direction `u`.
fn half(l: &Lead, u: Vec2) -> f32 {
    (u.x.abs() * l.size.x as f32 + u.y.abs() * l.size.y as f32) * 0.5
}

/// Where a lead's feature points sit: its top on an SMD part, the
/// board plane where a through-hole lead enters it.
fn lead_z(l: &Lead) -> f32 {
    match l.mounting {
        Mounting::Smd => (l.position.z + l.size.z / 2.0) as f32,
        _ => 0.0,
    }
}

fn v3(p: Vec2, z: f32) -> Vec3 {
    Vec3::new(p.x, p.y, z)
}

fn stated(d: &Dimension) -> bool {
    d.nominal.is_some() || d.min.is_some() || d.max.is_some()
}

/// The package's dimension annotations. `bounds` is the loaded model's
/// bounding box (the extents dimension lines are placed outside of).
pub fn annotations(
    package: &Package,
    g: &MechanicalGeometry,
    bounds: Option<(Vec3, Vec3)>,
) -> Vec<Annotation> {
    let b = g.body;
    let (bx0, bx1) = (
        (b.position.x - b.size.x / 2.0) as f32,
        (b.position.x + b.size.x / 2.0) as f32,
    );
    let (by0, by1) = (
        (b.position.y - b.size.y / 2.0) as f32,
        (b.position.y + b.size.y / 2.0) as f32,
    );
    let (bz0, bz1) = (
        (b.position.z - b.size.z / 2.0) as f32,
        (b.position.z + b.size.z / 2.0) as f32,
    );
    let leads = perimeter_leads(g);
    let (lo, hi) = bounds.unwrap_or_else(|| {
        let mut lo = Vec3::new(bx0, by0, bz0);
        let mut hi = Vec3::new(bx1, by1, bz1);
        for l in &g.leads {
            let p = Vec3::new(
                l.position.x as f32,
                l.position.y as f32,
                l.position.z as f32,
            );
            let s = Vec3::new(l.size.x as f32, l.size.y as f32, l.size.z as f32) * 0.5;
            lo = lo.min(p - s);
            hi = hi.max(p + s);
        }
        (lo, hi)
    });
    let size = (hi - lo).max_element().max(0.1);
    let gap = size * 0.09;
    let dims = &package.dimensions;
    let mut out = Vec::new();
    let mut push = |key: &'static str, pointer: &str, d: Option<&Dimension>, f: [Vec3; 2], l| {
        let (pointer, text) = match d.filter(|d| stated(d)) {
            Some(d) => (
                Some(pointer.to_string()),
                format::dimension(d).unwrap_or_default(),
            ),
            None => (None, format::measured((f[1] - f[0]).length() as f64)),
        };
        out.push(Annotation {
            key,
            pointer,
            text,
            feature: f,
            line: l,
        });
    };

    // Body width (X), front; the overall span (leads) one tier further.
    let y = lo.y - gap;
    push(
        "body_width",
        "/dimensions/body_width",
        Some(&dims.body_width),
        [Vec3::new(bx0, by0, bz0), Vec3::new(bx1, by0, bz0)],
        [Vec3::new(bx0, y, bz0), Vec3::new(bx1, y, bz0)],
    );
    // Body length (Y), right.
    let x = hi.x + gap;
    push(
        "body_length",
        "/dimensions/body_length",
        Some(&dims.body_length),
        [Vec3::new(bx1, by0, bz0), Vec3::new(bx1, by1, bz0)],
        [Vec3::new(x, by0, bz0), Vec3::new(x, by1, bz0)],
    );
    let extreme = |u: Vec2| {
        leads
            .iter()
            .copied()
            .max_by(|a, b| (xy(a).dot(u) + half(a, u)).total_cmp(&(xy(b).dot(u) + half(b, u))))
    };
    if let (Some(d), Some(l0), Some(l1)) = (
        dims.overall_width.as_ref().filter(|d| stated(d)),
        extreme(-Vec2::X),
        extreme(Vec2::X),
    ) {
        let f0 = v3(xy(l0) - Vec2::X * half(l0, Vec2::X), lead_z(l0));
        let f1 = v3(xy(l1) + Vec2::X * half(l1, Vec2::X), lead_z(l1));
        let y = y - gap;
        push(
            "overall_width",
            "/dimensions/overall_width",
            Some(d),
            [f0, f1],
            [Vec3::new(f0.x, y, 0.0), Vec3::new(f1.x, y, 0.0)],
        );
    }
    if let (Some(d), Some(l0), Some(l1)) = (
        dims.overall_length.as_ref().filter(|d| stated(d)),
        extreme(-Vec2::Y),
        extreme(Vec2::Y),
    ) {
        let f0 = v3(xy(l0) - Vec2::Y * half(l0, Vec2::Y), lead_z(l0));
        let f1 = v3(xy(l1) + Vec2::Y * half(l1, Vec2::Y), lead_z(l1));
        let x = x + gap;
        push(
            "overall_length",
            "/dimensions/overall_length",
            Some(d),
            [f0, f1],
            [Vec3::new(x, f0.y, 0.0), Vec3::new(x, f1.y, 0.0)],
        );
    }
    // Height, left front corner: seating plane to the body top.
    let z0 = bz0.min(0.0);
    let x = lo.x - gap;
    push(
        "body_height",
        "/dimensions/body_height",
        dims.body_height.as_ref(),
        [Vec3::new(bx0, by0, z0), Vec3::new(bx0, by0, bz1)],
        [Vec3::new(x, by0, z0), Vec3::new(x, by0, bz1)],
    );
    if let Some(d) = dims.overall_height.as_ref().filter(|d| stated(d)) {
        let x = x - gap;
        push(
            "overall_height",
            "/dimensions/overall_height",
            Some(d),
            [Vec3::new(lo.x, by0, z0), Vec3::new(lo.x, by0, hi.z)],
            [Vec3::new(x, by0, z0), Vec3::new(x, by0, hi.z)],
        );
    }

    // Lead dimensions, around lead 1 and its neighbour one pitch away.
    let Some(pitch) = package.pitch.nominal.or(g.pitch) else {
        return out;
    };
    let Some(l1) = leads
        .iter()
        .copied()
        .find(|l| l.number == "1")
        .or(leads.first().copied())
    else {
        return out;
    };
    let tol = (pitch * 0.03).max(0.01) as f32;
    let Some(l2) = leads
        .iter()
        .copied()
        .filter(|l| !std::ptr::eq(*l, l1))
        .filter(|l| ((xy(l) - xy(l1)).length() - pitch as f32).abs() < tol)
        .min_by(|a, b| {
            (xy(a) - xy(l1))
                .length()
                .total_cmp(&(xy(b) - xy(l1)).length())
        })
    else {
        return out;
    };
    let d = (xy(l2) - xy(l1)).normalize();
    let center = Vec2::new(b.position.x as f32, b.position.y as f32);
    let mut n = Vec2::new(-d.y, d.x);
    let r = n.dot(xy(l1) - center);
    if r < -1e-4 || (r.abs() <= 1e-4 && n.y < 0.0) {
        n = -n;
    }
    // How far out along `u` the lines clear the body and these leads.
    let body_reach =
        |u: Vec2| u.dot(center) + (u.x.abs() * b.size.x as f32 + u.y.abs() * b.size.y as f32) * 0.5;
    let reach = |u: Vec2, ls: &[&Lead]| {
        ls.iter()
            .map(|l| xy(l).dot(u) + half(l, u))
            .fold(body_reach(u), f32::max)
    };
    let small = gap * 0.6;
    let out_at = |p: Vec3, u: Vec2, t: f32| {
        let off = t + small - Vec2::new(p.x, p.y).dot(u);
        p + v3(u * off, 0.0)
    };
    let f = [v3(xy(l1), lead_z(l1)), v3(xy(l2), lead_z(l2))];
    let mut t = reach(n, &[l1, l2]);
    // Leads under the body (a radial can): the pitch goes in front, a
    // tier beyond the body width, rather than through the body.
    let lead_reach = [l1, l2]
        .iter()
        .map(|l| xy(l).dot(n) + half(l, n))
        .fold(f32::MIN, f32::max);
    if body_reach(n) > lead_reach + gap * 0.1 && d.x.abs() > 0.9 {
        n = -Vec2::Y;
        t = -lo.y + gap;
    }
    push(
        "pitch",
        "/pitch",
        Some(&package.pitch),
        f,
        [out_at(f[0], n, t), out_at(f[1], n, t)],
    );
    // Lead width at the far end of lead 1's row, on the same side; on the
    // other side when the row is just the two leads.
    if let Some(dw) = dims.lead_width.as_ref().filter(|d| stated(d)) {
        let row: Vec<&Lead> = leads
            .iter()
            .copied()
            .filter(|l| (xy(l) - xy(l1)).dot(n).abs() < 0.01)
            .collect();
        let far = row
            .iter()
            .copied()
            .max_by(|a, b| (xy(a) - xy(l1)).dot(d).total_cmp(&(xy(b) - xy(l1)).dot(d)))
            .unwrap_or(l1);
        let (lead, side) = if row.len() >= 3 { (far, n) } else { (l1, -n) };
        let c = xy(lead);
        let h = half(lead, d);
        let f = [v3(c - d * h, lead_z(lead)), v3(c + d * h, lead_z(lead))];
        let t = reach(side, &[lead]);
        push(
            "lead_width",
            "/dimensions/lead_width",
            Some(dw),
            f,
            [out_at(f[0], side, t), out_at(f[1], side, t)],
        );
    }
    // Lead length along the lead, beside lead 1 (before its row starts).
    if let Some(dl) = dims.lead_length.as_ref().filter(|d| stated(d)) {
        let c = xy(l1);
        let h = half(l1, n);
        let f = [v3(c - n * h, lead_z(l1)), v3(c + n * h, lead_z(l1))];
        let side = -d;
        let t = c.dot(side) + half(l1, side);
        let out_side = |p: Vec3| {
            let off = t + small - Vec2::new(p.x, p.y).dot(side);
            p + v3(side * off, 0.0)
        };
        push(
            "lead_length",
            "/dimensions/lead_length",
            Some(dl),
            f,
            [out_side(f[0]), out_side(f[1])],
        );
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn qfn() -> Package {
        serde_json::from_str(include_str!("../tests/qfn56.json")).unwrap()
    }

    #[test]
    fn qfn_dimensions_sit_on_the_geometry() {
        let p = qfn();
        let g = openparts_mcad::generate(&p).unwrap();
        let a = annotations(&p, &g, None);
        let get = |k: &str| a.iter().find(|x| x.key == k).unwrap();
        let w = get("body_width");
        assert_eq!(w.text, "7 mm");
        assert_eq!(w.pointer.as_deref(), Some("/dimensions/body_width"));
        assert!((w.feature[1].x - w.feature[0].x - 7.0).abs() < 1e-5);
        assert!(w.line[0].y < -3.5);
        // No height in the datasheet: the model's, marked as such.
        let h = get("body_height");
        assert_eq!(h.pointer, None);
        assert!(h.text.starts_with('≈'));
        // Pitch between leads 1 and 2, outside the left row.
        let pitch = get("pitch");
        assert_eq!(pitch.text, "0.4 mm");
        assert!(((pitch.feature[1] - pitch.feature[0]).length() - 0.4).abs() < 1e-5);
        assert!(pitch.line[0].x < -3.5);
        let lw = get("lead_width");
        assert_eq!(lw.text, "0.18 (0.13–0.23) mm");
        assert!(((lw.feature[1] - lw.feature[0]).length() - 0.18).abs() < 1e-5);
        let ll = get("lead_length");
        assert!(((ll.feature[1] - ll.feature[0]).length() - 0.4).abs() < 1e-5);
        // Every line runs parallel to what it measures.
        for x in &a {
            let f = (x.feature[1] - x.feature[0]).normalize();
            let l = (x.line[1] - x.line[0]).normalize();
            assert!(f.dot(l) > 0.999, "{}", x.key);
            assert!(!x.segments(0.2).is_empty());
        }
    }

    #[test]
    fn radial_pitch_goes_in_front_of_the_can() {
        let p: Package = serde_json::from_str(include_str!("../tests/hc49u.json")).unwrap();
        let g = openparts_mcad::generate(&p).unwrap();
        let a = annotations(&p, &g, None);
        let pitch = a.iter().find(|x| x.key == "pitch").unwrap();
        assert_eq!(pitch.text, "4.88 (4.68–5.08) mm");
        let width = a.iter().find(|x| x.key == "body_width").unwrap();
        assert_eq!(width.text, "≤11.5 mm");
        // In front of the body width line, not under the can.
        assert!(pitch.line[0].y < width.line[0].y);
    }

    #[test]
    fn usb_c_has_body_dimensions_and_no_pitch() {
        let p: Package = serde_json::from_str(include_str!("../tests/usb4105.json")).unwrap();
        let g = openparts_mcad::generate(&p).unwrap();
        let a = annotations(&p, &g, None);
        let keys: Vec<&str> = a.iter().map(|x| x.key).collect();
        assert_eq!(keys, ["body_width", "body_length", "body_height"]);
        assert!(a.iter().all(|x| x.pointer.is_some()));
    }
}
