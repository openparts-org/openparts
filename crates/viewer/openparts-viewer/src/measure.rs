//! Axis-parallel measuring, as CAD tools do it: once the first point is
//! down, the second snaps onto the X, Y or Z line through it whenever
//! the cursor is near that line on screen (or the direction is nearly
//! parallel to it), so the distance is the axis-parallel one. A lock
//! forces one axis; "free" measures the straight (diagonal) distance.

use glam::{Vec2, Vec3};

pub const AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];

/// Linear RGB of the X, Y and Z axis colours (red, green, blue).
pub const AXIS_COLORS: [[f32; 3]; 3] = [[0.80, 0.06, 0.05], [0.10, 0.50, 0.04], [0.05, 0.22, 0.90]];

/// sRGB hex of the same, for HTML/SVG.
pub const AXIS_CSS: [&str; 3] = ["#e5383b", "#2f9e44", "#1c7ed6"];

pub const AXIS_NAMES: [&str; 3] = ["X", "Y", "Z"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lock {
    /// Snap to an axis when near one.
    Auto,
    /// Never: the straight distance (Shift held).
    Free,
    /// Always along this axis (the X/Y/Z keys).
    Axis(usize),
}

/// The measured end point and the axis it was held to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constrained {
    pub end: Vec3,
    pub axis: Option<usize>,
}

/// Snap when the cursor is this close (CSS px) to an axis line...
pub const SNAP_PX: f32 = 14.0;
/// ...or the direction is within this angle (degrees) of the axis.
pub const SNAP_DEG: f32 = 7.0;

fn point_line_distance(p: Vec2, a: Vec2, dir: Vec2) -> Option<f32> {
    let len = dir.length();
    (len > 1e-3).then(|| ((p - a).perp_dot(dir) / len).abs())
}

/// Where the second point goes, for the candidate `b` (the snapped or
/// surface point under the cursor) after the first point `a`.
/// `axes` are the axes allowed (X, Y, Z in 3D; X, Y on the footprint);
/// `project` maps to CSS pixels.
pub fn constrain(
    a: Vec3,
    b: Vec3,
    lock: Lock,
    axes: usize,
    project: impl Fn(Vec3) -> Option<Vec2>,
) -> Constrained {
    let d = b - a;
    let along = |i: usize| a + AXES[i] * d.dot(AXES[i]);
    match lock {
        Lock::Free => Constrained { end: b, axis: None },
        Lock::Axis(i) => Constrained {
            end: along(i),
            axis: Some(i),
        },
        Lock::Auto => {
            if d.length() < 1e-6 {
                return Constrained { end: b, axis: None };
            }
            let (pa, pb) = (project(a), project(b));
            // Nearly parallel in space wins (it also covers an axis seen
            // end on); else the axis line passing nearest the cursor.
            let by_angle = AXES[..axes]
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let cos = (d.dot(*e).abs() / d.length()).min(1.0);
                    (cos.acos().to_degrees(), i)
                })
                .filter(|(angle, _)| *angle <= SNAP_DEG)
                .min_by(|x, y| x.0.total_cmp(&y.0));
            let by_pixels = || {
                AXES[..axes]
                    .iter()
                    .enumerate()
                    .filter_map(|(i, e)| {
                        let pe = project(a + *e * d.length())?;
                        let px = point_line_distance(pb?, pa?, pe - pa?)?;
                        (px <= SNAP_PX).then_some((px, i))
                    })
                    .min_by(|x, y| x.0.total_cmp(&y.0))
            };
            let best = by_angle.or_else(by_pixels);
            match best {
                Some((_, i)) => Constrained {
                    end: along(i),
                    axis: Some(i),
                },
                None => Constrained { end: b, axis: None },
            }
        }
    }
}

/// The point on the line `a + t * dir` closest to the ray (origin, unit
/// direction): where a forced axis meets the cursor away from the model.
pub fn closest_on_line(a: Vec3, dir: Vec3, origin: Vec3, ray: Vec3) -> Vec3 {
    let w = a - origin;
    let b = dir.dot(ray);
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-6 {
        return a;
    }
    let t = (b * ray.dot(w) - dir.dot(w)) / denom;
    a + dir * t
}

/// `a`..`b` as dashes `dash` long with `gap` between them.
pub fn dashes(a: Vec3, b: Vec3, dash: f32, gap: f32) -> Vec<[Vec3; 2]> {
    let len = (b - a).length();
    if len < 1e-9 || dash <= 0.0 {
        return Vec::new();
    }
    let u = (b - a) / len;
    let n = ((len / (dash + gap)).ceil() as usize).min(400);
    let step = len / n.max(1) as f32;
    let dash = step * dash / (dash + gap);
    (0..n)
        .map(|i| {
            let s = a + u * (i as f32 * step);
            [s, s + u * dash]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A top view: screen x = world x, screen y = -world y, 10 px/mm; Z
    /// points at the viewer.
    fn top(p: Vec3) -> Option<Vec2> {
        Some(Vec2::new(p.x * 10.0, -p.y * 10.0))
    }

    #[test]
    fn snaps_near_an_axis_and_not_elsewhere() {
        let a = Vec3::ZERO;
        // 1 px off the X line, 5 mm along: X, 5 mm.
        let c = constrain(a, Vec3::new(5.0, 0.1, 0.3), Lock::Auto, 3, top);
        assert_eq!(c.axis, Some(0));
        assert_eq!(c.end, Vec3::new(5.0, 0.0, 0.0));
        // A diagonal stays diagonal.
        let b = Vec3::new(5.0, 4.0, 0.0);
        let c = constrain(a, b, Lock::Auto, 3, top);
        assert_eq!((c.axis, c.end), (None, b));
        // Free never snaps; a lock always does.
        let near = Vec3::new(5.0, 0.1, 0.0);
        assert_eq!(constrain(a, near, Lock::Free, 3, top).axis, None);
        let c = constrain(a, b, Lock::Axis(1), 3, top);
        assert_eq!(c.end, Vec3::new(0.0, 4.0, 0.0));
    }

    #[test]
    fn z_snaps_by_angle_when_it_points_at_the_viewer() {
        let c = constrain(Vec3::ZERO, Vec3::new(0.05, 0.0, 3.0), Lock::Auto, 3, top);
        assert_eq!(c.axis, Some(2));
        // The footprint only has X and Y.
        let c = constrain(Vec3::ZERO, Vec3::new(0.05, 0.0, 3.0), Lock::Auto, 2, top);
        assert_eq!(c.axis, Some(0));
    }

    #[test]
    fn closest_point_on_an_axis_to_a_ray() {
        let p = closest_on_line(Vec3::ZERO, Vec3::X, Vec3::new(3.0, 5.0, 10.0), -Vec3::Z);
        assert!((p - Vec3::new(3.0, 0.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn dashes_cover_the_line() {
        let d = dashes(Vec3::ZERO, Vec3::X * 10.0, 1.0, 1.0);
        assert_eq!(d.len(), 5);
        assert_eq!(d[0][0], Vec3::ZERO);
        assert!((d[4][1].x - 9.0).abs() < 1e-5);
    }
}
