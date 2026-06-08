//! Viewport camera controls: orbit, fly, and fps modes.

use rustix_core::math::{Vec3, Mat4, Quat};

/// Camera control mode for the editor viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Orbit,
    Fly,
    Fps,
}

/// Editor viewport camera state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorCamera {
    pub mode: CameraMode,
    pub position: Vec3,
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub fov_deg: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for EditorCamera {
    fn default() -> Self {
        Self {
            mode: CameraMode::Orbit,
            position: Vec3::new(10.0, 10.0, 10.0),
            target: Vec3::ZERO,
            yaw: -std::f32::consts::FRAC_PI_4,
            pitch: -std::f32::consts::FRAC_PI_4,
            distance: 20.0,
            fov_deg: 60.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl EditorCamera {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mode(mut self, m: CameraMode) -> Self {
        self.mode = m;
        self
    }

    /// Update camera from mouse delta (orbit mode).
    pub fn orbit_drag(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.01;
        self.pitch = (self.pitch - dy * 0.01).clamp(-1.5, 1.5);
        self.recalc_orbit();
    }

    /// Zoom orbit camera.
    pub fn orbit_zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta * 0.5).clamp(0.5, 500.0);
        self.recalc_orbit();
    }

    /// Pan the orbit target.
    pub fn orbit_pan(&mut self, dx: f32, dy: f32) {
        let right = self.right();
        let up = self.up();
        self.target += right * dx * 0.01 * self.distance * 0.1
                     - up * dy * 0.01 * self.distance * 0.1;
        self.recalc_orbit();
    }

    /// Fly / FPS movement.
    pub fn fly_move(&mut self, forward: f32, right: f32, up: f32, speed: f32) {
        let dir = self.forward() * forward + self.right() * right + Vec3::Y * up;
        self.position += dir.normalize_or_zero() * speed;
        if self.mode == CameraMode::Orbit {
            self.target += dir.normalize_or_zero() * speed;
        }
    }

    /// Fly / FPS look.
    pub fn fly_look(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.003;
        self.pitch = (self.pitch - dy * 0.003).clamp(-1.5, 1.5);
        if self.mode == CameraMode::Orbit {
            self.recalc_orbit();
        } else {
            let rot = Quat::from_euler(
                glam::EulerRot::YXZ,
                self.yaw,
                self.pitch,
                0.0,
            );
            self.position = self.target - rot * Vec3::Z * self.distance;
        }
    }

    fn recalc_orbit(&mut self) {
        let rot = Quat::from_euler(
            glam::EulerRot::YXZ,
            self.yaw,
            self.pitch,
            0.0,
        );
        self.position = self.target - rot * Vec3::Z * self.distance;
    }

    pub fn forward(&self) -> Vec3 {
        let rot = Quat::from_euler(
            glam::EulerRot::YXZ,
            self.yaw,
            self.pitch,
            0.0,
        );
        rot * Vec3::NEG_Z
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize_or_zero()
    }

    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward()).normalize_or_zero()
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.position + self.forward(), Vec3::Y)
    }

    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(
            self.fov_deg.to_radians(),
            aspect,
            self.near,
            self.far,
        )
    }
}
