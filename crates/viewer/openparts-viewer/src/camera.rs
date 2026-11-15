//! An orbit camera around a target, Z up (the artifacts' frame).

use glam::{Mat4, Vec2, Vec3, Vec4};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    /// Around Z, radians (0 looks along +Y from -Y).
    pub azimuth: f32,
    /// Above the XY plane, radians.
    pub elevation: f32,
    /// Vertical field of view, radians.
    pub fov: f32,
    /// The model's radius: clip planes and zoom limits follow it.
    pub radius: f32,
}

const MAX_ELEVATION: f32 = 1.55;

/// A right-handed view matrix (the camera looks down its -Z).
pub fn look_at_rh(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    let f = (target - eye).normalize();
    let s = f.cross(up).normalize_or(Vec3::X);
    let u = s.cross(f);
    Mat4::from_cols(
        Vec4::new(s.x, u.x, -f.x, 0.0),
        Vec4::new(s.y, u.y, -f.y, 0.0),
        Vec4::new(s.z, u.z, -f.z, 0.0),
        Vec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
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

impl OrbitCamera {
    /// Looks at `bounds` from the front right, above, the whole of it in
    /// view.
    pub fn fit(lo: Vec3, hi: Vec3) -> OrbitCamera {
        let radius = ((hi - lo).length() * 0.5).max(0.1);
        let fov = 40f32.to_radians();
        OrbitCamera {
            target: (lo + hi) * 0.5,
            distance: radius / (fov * 0.5).sin() * 1.15,
            azimuth: -0.6,
            elevation: 0.62,
            fov,
            radius,
        }
    }

    pub fn eye(&self) -> Vec3 {
        let (se, ce) = self.elevation.sin_cos();
        let (sa, ca) = self.azimuth.sin_cos();
        self.target + Vec3::new(sa * ce, -ca * ce, se) * self.distance
    }

    pub fn view(&self) -> Mat4 {
        look_at_rh(self.eye(), self.target, Vec3::Z)
    }

    pub fn near_far(&self) -> (f32, f32) {
        let near = (self.distance - self.radius * 4.0).max(self.distance * 0.01);
        (near, self.distance + self.radius * 20.0)
    }

    /// Depth in 0..1 (WebGPU's convention).
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let (near, far) = self.near_far();
        perspective_rh(self.fov, aspect.max(1e-3), near, far)
    }

    pub fn view_projection(&self, size: Vec2) -> Mat4 {
        self.projection(size.x / size.y.max(1.0)) * self.view()
    }

    /// `p` in CSS pixels (origin top left) and its depth along the view,
    /// or `None` behind the camera.
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

    /// World units per CSS pixel at `p`'s distance.
    pub fn world_per_pixel(&self, p: Vec3, viewport_height: f32) -> f32 {
        let depth = (p - self.eye()).dot((self.target - self.eye()).normalize());
        2.0 * depth.max(1e-4) * (self.fov * 0.5).tan() / viewport_height.max(1.0)
    }

    /// Drag by `d` CSS pixels.
    pub fn orbit(&mut self, d: Vec2) {
        self.azimuth -= d.x * 0.008;
        self.elevation = (self.elevation + d.y * 0.008).clamp(-MAX_ELEVATION, MAX_ELEVATION);
    }

    /// Moves the target so the point under the cursor follows a drag of
    /// `d` CSS pixels.
    pub fn pan(&mut self, d: Vec2, viewport_height: f32) {
        let k = self.world_per_pixel(self.target, viewport_height);
        let forward = (self.target - self.eye()).normalize();
        let right = forward.cross(Vec3::Z).normalize_or(Vec3::X);
        let up = right.cross(forward);
        self.target += (-right * d.x + up * d.y) * k;
    }

    /// `factor` > 1 zooms out.
    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(self.radius * 0.3, self.radius * 40.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_and_unprojects_consistently() {
        let cam = OrbitCamera::fit(Vec3::splat(-1.0), Vec3::splat(1.0));
        let size = Vec2::new(800.0, 600.0);
        let (px, _) = cam.project(cam.target, size).unwrap();
        assert!((px - size * 0.5).length() < 1e-3);
        let p = Vec3::new(0.5, -0.3, 0.2);
        let (px, _) = cam.project(p, size).unwrap();
        let (o, d) = cam.ray(px, size);
        // The ray passes through p.
        let t = (p - o).dot(d);
        assert!((o + d * t - p).length() < 1e-3);
    }

    #[test]
    fn orbit_keeps_distance_and_clamps_elevation() {
        let mut cam = OrbitCamera::fit(Vec3::ZERO, Vec3::ONE);
        let before = (cam.eye() - cam.target).length();
        cam.orbit(Vec2::new(100.0, 10_000.0));
        assert!((cam.eye() - cam.target).length() - before < 1e-3);
        assert!(cam.elevation <= MAX_ELEVATION);
        let d = cam.distance;
        cam.zoom(1e9);
        assert!(cam.distance > d && cam.distance <= cam.radius * 40.0);
    }
}
