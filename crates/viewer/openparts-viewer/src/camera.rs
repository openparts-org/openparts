//! The view: an orbit camera around a target, Z up (the artifacts'
//! frame), perspective or orthographic, with the CAD-style moves (orbit
//! about a pivot, zoom toward the cursor, standard views, fit) and
//! smooth transitions between views.

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    pub target: Vec3,
    /// Eye to target. In orthographic mode it only sets the scale: the
    /// view is as tall as a perspective view's at the target.
    pub distance: f32,
    /// Around Z, radians (0 looks along +Y, from the front).
    pub azimuth: f32,
    /// Above the XY plane, radians, -π/2 (from below) ..= π/2 (top).
    pub elevation: f32,
    /// Vertical field of view, radians.
    pub fov: f32,
    /// The model's radius: clip planes and zoom limits follow it.
    pub radius: f32,
    pub orthographic: bool,
}

/// A named view direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardView {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
    Iso,
}

impl StandardView {
    /// (azimuth, elevation) looking from that side.
    pub fn angles(self) -> (f32, f32) {
        match self {
            StandardView::Front => (0.0, 0.0),
            StandardView::Back => (PI, 0.0),
            StandardView::Right => (FRAC_PI_2, 0.0),
            StandardView::Left => (-FRAC_PI_2, 0.0),
            StandardView::Top => (0.0, FRAC_PI_2),
            StandardView::Bottom => (0.0, -FRAC_PI_2),
            StandardView::Iso => (-PI / 4.0, (1.0f32 / 2f32.sqrt()).atan()),
        }
    }

    /// Looking from the end of axis `axis` (0 X, 1 Y, 2 Z), `positive`
    /// or negative, toward the model.
    pub fn from_axis(axis: usize, positive: bool) -> StandardView {
        match (axis, positive) {
            (0, true) => StandardView::Right,
            (0, false) => StandardView::Left,
            (1, true) => StandardView::Back,
            (1, false) => StandardView::Front,
            (2, true) => StandardView::Top,
            _ => StandardView::Bottom,
        }
    }

    /// Its labels' i18n key.
    pub fn key(self) -> &'static str {
        match self {
            StandardView::Front => "view_front",
            StandardView::Back => "view_back",
            StandardView::Left => "view_left",
            StandardView::Right => "view_right",
            StandardView::Top => "view_top",
            StandardView::Bottom => "view_bottom",
            StandardView::Iso => "view_iso",
        }
    }
}

/// A right-handed view matrix from an eye and its unit basis.
fn view_matrix(eye: Vec3, right: Vec3, up: Vec3, forward: Vec3) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(right.x, up.x, -forward.x, 0.0),
        Vec4::new(right.y, up.y, -forward.y, 0.0),
        Vec4::new(right.z, up.z, -forward.z, 0.0),
        Vec4::new(-right.dot(eye), -up.dot(eye), forward.dot(eye), 1.0),
    )
}

/// A right-handed perspective projection to depth 0..1 (WebGPU's).
pub fn perspective_rh(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let h = 1.0 / (fov_y * 0.5).tan();
    let r = far / (near - far);
    Mat4::from_cols(
        Vec4::new(h / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, h, 0.0, 0.0),
        Vec4::new(0.0, 0.0, r, -1.0),
        Vec4::new(0.0, 0.0, r * near, 0.0),
    )
}

/// A right-handed orthographic projection to depth 0..1.
pub fn orthographic_rh(half_w: f32, half_h: f32, near: f32, far: f32) -> Mat4 {
    let r = 1.0 / (near - far);
    Mat4::from_cols(
        Vec4::new(1.0 / half_w, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0 / half_h, 0.0, 0.0),
        Vec4::new(0.0, 0.0, r, 0.0),
        Vec4::new(0.0, 0.0, r * near, 1.0),
    )
}

fn wrap(a: f32) -> f32 {
    let mut a = (a + PI).rem_euclid(2.0 * PI) - PI;
    if a <= -PI {
        a += 2.0 * PI;
    }
    a
}

impl OrbitCamera {
    /// The isometric view of `lo`..`hi`, fitted.
    pub fn fit(lo: Vec3, hi: Vec3) -> OrbitCamera {
        let (azimuth, elevation) = StandardView::Iso.angles();
        let mut c = OrbitCamera {
            target: (lo + hi) * 0.5,
            distance: 1.0,
            azimuth,
            elevation,
            fov: 30f32.to_radians(),
            radius: ((hi - lo).length() * 0.5).max(0.1),
            orthographic: true,
        };
        c.fit_box(lo, hi, 1.0);
        c
    }

    /// The camera's unit (right, up, forward).
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let (se, ce) = self.elevation.sin_cos();
        let (sa, ca) = self.azimuth.sin_cos();
        let forward = -Vec3::new(sa * ce, -ca * ce, se);
        let right = Vec3::new(ca, sa, 0.0);
        let up = right.cross(forward);
        (right, up, forward)
    }

    fn rotation(&self) -> Quat {
        let (r, u, f) = self.basis();
        Quat::from_mat3(&glam::Mat3::from_cols(r, u, -f))
    }

    pub fn eye(&self) -> Vec3 {
        // Orthographic: far enough back that nothing is behind the eye.
        let d = if self.orthographic {
            self.distance.max(self.radius * 6.0)
        } else {
            self.distance
        };
        self.target - self.basis().2 * d
    }

    pub fn view(&self) -> Mat4 {
        let (r, u, f) = self.basis();
        view_matrix(self.eye(), r, u, f)
    }

    pub fn near_far(&self) -> (f32, f32) {
        let d = (self.eye() - self.target).length();
        let near = (d - self.radius * 8.0).max(d * 0.01);
        (near, d + self.radius * 30.0)
    }

    /// Half the view's height at the target (world units).
    pub fn half_height(&self) -> f32 {
        self.distance * (self.fov * 0.5).tan()
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        let aspect = aspect.max(1e-3);
        let (near, far) = self.near_far();
        if self.orthographic {
            let h = self.half_height();
            orthographic_rh(h * aspect, h, near, far)
        } else {
            perspective_rh(self.fov, aspect, near, far)
        }
    }

    pub fn view_projection(&self, size: Vec2) -> Mat4 {
        self.projection(size.x / size.y.max(1.0)) * self.view()
    }

    /// `p` in CSS pixels (origin top left) and its clip w, or `None`
    /// behind the camera.
    pub fn project(&self, p: Vec3, size: Vec2) -> Option<(Vec2, f32)> {
        let c = self.view_projection(size) * p.extend(1.0);
        if c.w <= 1e-6 {
            return None;
        }
        let n = c.truncate() / c.w;
        Some((
            Vec2::new((n.x + 1.0) * 0.5 * size.x, (1.0 - n.y) * 0.5 * size.y),
            c.w,
        ))
    }

    /// The ray through CSS pixel `px`: (origin, unit direction).
    pub fn ray(&self, px: Vec2, size: Vec2) -> (Vec3, Vec3) {
        let inv = self.view_projection(size).inverse();
        let ndc = Vec2::new(px.x / size.x * 2.0 - 1.0, 1.0 - px.y / size.y * 2.0);
        let unproject = |z: f32| {
            let v: Vec4 = inv * Vec4::new(ndc.x, ndc.y, z, 1.0);
            v.truncate() / v.w
        };
        let a = unproject(0.0);
        let b = unproject(1.0);
        (a, (b - a).normalize())
    }

    /// The point under `px` on the plane through the target facing the
    /// camera (where the cursor "is" when it isn't over the model).
    pub fn on_target_plane(&self, px: Vec2, size: Vec2) -> Vec3 {
        let (o, d) = self.ray(px, size);
        let f = self.basis().2;
        let denom = d.dot(f);
        if denom.abs() < 1e-6 {
            return self.target;
        }
        o + d * ((self.target - o).dot(f) / denom)
    }

    /// World units per CSS pixel at `p`.
    pub fn world_per_pixel(&self, p: Vec3, viewport_height: f32) -> f32 {
        if self.orthographic {
            return 2.0 * self.half_height() / viewport_height.max(1.0);
        }
        let depth = (p - self.eye()).dot(self.basis().2);
        2.0 * depth.max(1e-4) * (self.fov * 0.5).tan() / viewport_height.max(1.0)
    }

    /// Drag by `d` CSS pixels, turning about `pivot` (which stays put on
    /// screen).
    pub fn orbit_about(&mut self, d: Vec2, pivot: Vec3) {
        let before = self.rotation();
        self.azimuth = wrap(self.azimuth - d.x * 0.008);
        self.elevation = (self.elevation + d.y * 0.008).clamp(-FRAC_PI_2, FRAC_PI_2);
        let turn = self.rotation() * before.inverse();
        self.target = pivot + turn * (self.target - pivot);
    }

    /// Moves the view so the point under the cursor follows a drag of
    /// `d` CSS pixels.
    pub fn pan(&mut self, d: Vec2, viewport_height: f32) {
        let k = self.world_per_pixel(self.target, viewport_height);
        let (right, up, _) = self.basis();
        self.target += (-right * d.x + up * d.y) * k;
    }

    fn clamp_factor(&self, factor: f32) -> f32 {
        let d = (self.distance * factor).clamp(self.radius * 0.02, self.radius * 60.0);
        d / self.distance
    }

    /// Zooms by `factor` (> 1 out) keeping `at` where it is on screen.
    pub fn zoom_at(&mut self, factor: f32, at: Vec3) {
        let f = self.clamp_factor(factor);
        self.distance *= f;
        self.target = at + (self.target - at) * f;
    }

    /// Fits the box `lo`..`hi` in the current direction, `margin` (> 1)
    /// leaving room around it. `aspect` is the viewport's width/height.
    pub fn fit_box_aspect(&mut self, lo: Vec3, hi: Vec3, margin: f32, aspect: f32) {
        let (r, u, f) = self.basis();
        let c = (lo + hi) * 0.5;
        let (mut hw, mut hh, mut hd) = (0.0f32, 0.0f32, 0.0f32);
        for i in 0..8 {
            let p = Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            ) - c;
            hw = hw.max(p.dot(r).abs());
            hh = hh.max(p.dot(u).abs());
            hd = hd.max(p.dot(f).abs());
        }
        let half = hh.max(hw / aspect.max(1e-3)).max(1e-3) * margin;
        self.target = c;
        self.distance = half / (self.fov * 0.5).tan() + if self.orthographic { 0.0 } else { hd };
        self.radius = self.radius.max((hi - lo).length() * 0.5);
    }

    pub fn fit_box(&mut self, lo: Vec3, hi: Vec3, margin: f32) {
        self.fit_box_aspect(lo, hi, margin, 1.6);
    }

    pub fn set_view(&mut self, view: StandardView) {
        let (a, e) = view.angles();
        self.azimuth = a;
        self.elevation = e;
    }

    /// Between `self` (t = 0) and `to` (t = 1), eased: the shorter way
    /// round in azimuth, scale changes geometric.
    pub fn lerp(&self, to: &OrbitCamera, t: f32) -> OrbitCamera {
        let s = t * t * (3.0 - 2.0 * t);
        let da = wrap(to.azimuth - self.azimuth);
        OrbitCamera {
            target: self.target.lerp(to.target, s),
            distance: self.distance * (to.distance / self.distance).powf(s),
            azimuth: wrap(self.azimuth + da * s),
            elevation: self.elevation + (to.elevation - self.elevation) * s,
            fov: to.fov,
            radius: to.radius,
            orthographic: if s < 0.5 {
                self.orthographic
            } else {
                to.orthographic
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cams() -> [OrbitCamera; 2] {
        let mut a = OrbitCamera::fit(Vec3::splat(-1.0), Vec3::splat(1.0));
        let b = a;
        a.orthographic = false;
        [a, b]
    }

    #[test]
    fn projects_and_unprojects_consistently() {
        let size = Vec2::new(800.0, 600.0);
        for cam in cams() {
            let (px, _) = cam.project(cam.target, size).unwrap();
            assert!((px - size * 0.5).length() < 1e-2);
            let p = Vec3::new(0.5, -0.3, 0.2);
            let (px, _) = cam.project(p, size).unwrap();
            let (o, d) = cam.ray(px, size);
            let t = (p - o).dot(d);
            assert!((o + d * t - p).length() < 1e-3, "{cam:?}");
        }
    }

    #[test]
    fn standard_views_look_along_the_axes() {
        let mut c = cams()[1];
        c.set_view(StandardView::Top);
        let (r, u, f) = c.basis();
        assert!((f + Vec3::Z).length() < 1e-5);
        assert!((r - Vec3::X).length() < 1e-5 && (u - Vec3::Y).length() < 1e-5);
        c.set_view(StandardView::Front);
        assert!((c.basis().2 - Vec3::Y).length() < 1e-5);
        c.set_view(StandardView::Right);
        assert!((c.basis().2 + Vec3::X).length() < 1e-5);
        c.set_view(StandardView::Bottom);
        assert!((c.basis().2 - Vec3::Z).length() < 1e-5);
    }

    #[test]
    fn orbit_keeps_the_pivot_on_screen() {
        let size = Vec2::new(800.0, 600.0);
        for mut cam in cams() {
            let pivot = Vec3::new(0.7, 0.2, -0.4);
            let before = cam.project(pivot, size).unwrap().0;
            cam.orbit_about(Vec2::new(60.0, -25.0), pivot);
            let after = cam.project(pivot, size).unwrap().0;
            assert!((before - after).length() < 0.05, "{before} {after}");
        }
    }

    #[test]
    fn zoom_keeps_the_cursor_point_on_screen() {
        let size = Vec2::new(800.0, 600.0);
        for mut cam in cams() {
            let at = cam.on_target_plane(Vec2::new(620.0, 140.0), size);
            cam.zoom_at(0.5, at);
            let px = cam.project(at, size).unwrap().0;
            assert!((px - Vec2::new(620.0, 140.0)).length() < 0.05, "{px}");
        }
    }

    #[test]
    fn fit_puts_the_box_in_view() {
        let size = Vec2::new(800.0, 400.0);
        for mut cam in cams() {
            cam.set_view(StandardView::Front);
            let (lo, hi) = (Vec3::new(-10.0, -1.0, 0.0), Vec3::new(10.0, 1.0, 2.0));
            cam.fit_box_aspect(lo, hi, 1.1, 2.0);
            for x in [lo.x, hi.x] {
                let px = cam.project(Vec3::new(x, 0.0, 1.0), size).unwrap().0;
                assert!(px.x > 0.0 && px.x < size.x, "{px}");
                assert!(px.x < 60.0 || px.x > size.x - 60.0, "{px}");
            }
        }
    }

    #[test]
    fn transitions_take_the_short_way_round() {
        let mut a = cams()[1];
        a.azimuth = 3.0;
        let mut b = a;
        b.azimuth = -3.0;
        let mid = a.lerp(&b, 0.5);
        assert!(mid.azimuth.abs() > 3.0);
        assert!((a.lerp(&b, 1.0).azimuth - b.azimuth).abs() < 1e-5);
    }
}
