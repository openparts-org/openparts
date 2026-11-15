//! The navigation gizmo (as in Fusion/Onshape): the X/Y/Z axes as they
//! currently face the viewer, an SVG redrawn as the camera turns —
//! coloured balls with labels on the positive ends, hollow circles on
//! the negative ones. Clicking a ball looks from that end of the axis.

use crate::camera::OrbitCamera;
use crate::measure::{AXES, AXIS_CSS, AXIS_NAMES};

/// The SVG's half size (user units; the element is sized by CSS).
pub const HALF: f32 = 50.0;
const REACH: f32 = 33.0;

/// The gizmo's inner SVG (no `<svg>` element) for `camera`'s
/// orientation. `titles[axis][positive]` are the balls' tooltips.
pub fn svg(camera: &OrbitCamera, titles: &[[String; 2]; 3]) -> String {
    let (r, u, f) = camera.basis();
    let mut balls: Vec<(f32, usize, bool, f32, f32)> = Vec::new();
    for (i, e) in AXES.iter().enumerate() {
        for positive in [true, false] {
            let p = if positive { *e } else { -*e };
            balls.push((
                -p.dot(f),
                i,
                positive,
                p.dot(r) * REACH + 0.0,
                -p.dot(u) * REACH + 0.0,
            ));
        }
    }
    // Back to front.
    balls.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut s = format!("<circle class=\"opv-gizmo-bg\" r=\"{}\"/>", HALF - 1.0);
    for &(_, i, positive, x, y) in &balls {
        if positive {
            s.push_str(&format!(
                "<line x1=\"0\" y1=\"0\" x2=\"{x:.2}\" y2=\"{y:.2}\" stroke=\"{}\" stroke-width=\"2.5\" \
                 stroke-linecap=\"round\"/>",
                AXIS_CSS[i]
            ));
        }
    }
    for &(depth, i, positive, x, y) in &balls {
        let title = crate::footprint::esc(&titles[i][positive as usize]);
        // Balls turned away fade a little.
        let opacity = if depth < -0.2 { 0.75 } else { 1.0 };
        let body = if positive {
            format!(
                "<circle r=\"9.5\" fill=\"{c}\"/><text dy=\"0.35em\" text-anchor=\"middle\" font-size=\"11\" \
                 font-weight=\"700\" fill=\"#fff\">{n}</text>",
                c = AXIS_CSS[i],
                n = AXIS_NAMES[i]
            )
        } else {
            format!(
                "<circle r=\"7\" fill=\"rgba(255,255,255,0.85)\" stroke=\"{}\" stroke-width=\"2\"/>",
                AXIS_CSS[i]
            )
        };
        s.push_str(&format!(
            "<g class=\"opv-gizmo-ball\" data-axis=\"{i}\" data-positive=\"{}\" \
             transform=\"translate({x:.2} {y:.2})\" opacity=\"{opacity}\"><title>{title}</title>{body}</g>",
            positive as u8
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::StandardView;
    use glam::Vec3;

    fn titles() -> [[String; 2]; 3] {
        Default::default()
    }

    #[test]
    fn top_view_shows_x_right_y_up_z_in_front() {
        let mut c = OrbitCamera::fit(Vec3::ZERO, Vec3::ONE);
        c.set_view(StandardView::Top);
        let s = svg(&c, &titles());
        assert!(
            s.contains("data-axis=\"0\" data-positive=\"1\" transform=\"translate(33.00 0.00)\"")
        );
        assert!(
            s.contains("data-axis=\"1\" data-positive=\"1\" transform=\"translate(0.00 -33.00)\"")
        );
        // +Z faces the viewer: drawn last.
        let z = s.rfind("data-axis=").unwrap();
        assert!(s[z..].starts_with("data-axis=\"2\" data-positive=\"1\""));
    }
}
