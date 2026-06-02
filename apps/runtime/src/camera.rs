use rustix_core::math::{Vec3, Mat4};
use rustix_platform::input::{InputManager, KeyCode};

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CameraMode {
    Orbit,
    FirstPerson,
}

pub struct EditorCamera {
    pub position: Vec3,
    pub center: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub mode: CameraMode,
    pub follow_target: bool,
}

impl EditorCamera {
    pub fn new() -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 5.0),
            center: Vec3::ZERO,
            yaw: 0.0,
            pitch: -0.3,
            distance: 8.0,
            mode: CameraMode::Orbit,
            follow_target: false,
        }
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let proj = Mat4::perspective_rh_gl(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);
        match self.mode {
            CameraMode::Orbit => {
                let eye = self.eye_pos();
                proj * Mat4::look_at_rh(eye, self.center, Vec3::Y)
            }
            CameraMode::FirstPerson => {
                let forward = Vec3::new(
                    self.pitch.cos() * self.yaw.sin(),
                    self.pitch.sin(),
                    self.pitch.cos() * self.yaw.cos(),
                );
                let look_at = self.position + forward;
                proj * Mat4::look_at_rh(self.position, look_at, Vec3::Y)
            }
        }
    }

    pub fn eye_pos(&self) -> Vec3 {
        match self.mode {
            CameraMode::Orbit => Vec3::new(
                self.center.x + self.distance * self.pitch.cos() * self.yaw.sin(),
                self.center.y + self.distance * self.pitch.sin(),
                self.center.z + self.distance * self.pitch.cos() * self.yaw.cos(),
            ),
            CameraMode::FirstPerson => self.position,
        }
    }

    pub fn follow(&mut self, target: Option<Vec3>) {
        if !self.follow_target { return; }
        if let Some(pos) = target {
            self.center = pos;
            if self.mode == CameraMode::FirstPerson {
                self.position = pos + Vec3::new(0.0, 1.6, 0.0);
            }
        }
    }

    pub fn update(&mut self, input: &InputManager, dt: f32) {
        let k = input.keyboard();
        let rot_speed = 2.0 * dt;
        let zoom_speed = 3.0 * dt;
        let move_speed = 5.0 * dt;

        let (dx, dy) = input.mouse().delta();
        let shift_held = k.down(KeyCode::ShiftLeft) || k.down(KeyCode::ShiftRight);

        match self.mode {
            CameraMode::Orbit => {
                if shift_held {
                    if k.down(KeyCode::W) { self.distance -= zoom_speed; }
                    if k.down(KeyCode::S) { self.distance += zoom_speed; }
                    if k.down(KeyCode::A) { self.yaw -= rot_speed; }
                    if k.down(KeyCode::D) { self.yaw += rot_speed; }
                    if k.down(KeyCode::Q) { self.pitch = (self.pitch - rot_speed).clamp(-1.4, 1.4); }
                    if k.down(KeyCode::E) { self.pitch = (self.pitch + rot_speed).clamp(-1.4, 1.4); }
                }
                self.distance = self.distance.max(0.5);

                if input.mouse().down(rustix_platform::input::MouseButton::Right) {
                    self.yaw += dx * 0.005;
                    self.pitch = (self.pitch - dy * 0.005).clamp(-1.4, 1.4);
                }
                if input.mouse().down(rustix_platform::input::MouseButton::Middle) {
                    self.center += Vec3::new(-dx * 0.01 * self.distance * 0.05, dy * 0.01 * self.distance * 0.05, 0.0);
                }
            }
            CameraMode::FirstPerson => {
                let forward = Vec3::new(
                    self.pitch.cos() * self.yaw.sin(),
                    0.0,
                    self.pitch.cos() * self.yaw.cos(),
                ).normalize();
                let right = Vec3::new(forward.z, 0.0, -forward.x).normalize();

                if shift_held {
                    if k.down(KeyCode::W) { self.position += forward * move_speed; }
                    if k.down(KeyCode::S) { self.position -= forward * move_speed; }
                    if k.down(KeyCode::A) { self.position -= right * move_speed; }
                    if k.down(KeyCode::D) { self.position += right * move_speed; }
                    if k.down(KeyCode::Q) { self.position.y -= move_speed; }
                    if k.down(KeyCode::E) { self.position.y += move_speed; }
                }

                if input.mouse().down(rustix_platform::input::MouseButton::Right) {
                    self.yaw += dx * 0.005;
                    self.pitch = (self.pitch - dy * 0.005).clamp(-1.4, 1.4);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix_platform::input::{InputEvent, InputManager, MouseButton};

    #[test]
    fn orbit_middle_click_drag_pans_center() {
        let mut cam = EditorCamera::new();
        cam.mode = CameraMode::Orbit;
        cam.distance = 10.0;
        let initial_center = cam.center;

        let mut input = InputManager::new();
        // Hold middle-click
        input.push_event(InputEvent::MouseButton(MouseButton::Middle, true));
        // Move mouse by (100, 50) pixels
        input.push_event(InputEvent::MouseMove(100.0, 50.0));
        input.poll();

        cam.update(&input, 0.016);

        let expected_dx = -100.0 * 0.01 * cam.distance * 0.05;
        let expected_dy = 50.0 * 0.01 * cam.distance * 0.05;

        assert!(
            (cam.center.x - initial_center.x - expected_dx).abs() < 0.0001,
            "middle-click drag should pan center.x, got delta={} expected={}",
            cam.center.x - initial_center.x, expected_dx
        );
        assert!(
            (cam.center.y - initial_center.y - expected_dy).abs() < 0.0001,
            "middle-click drag should pan center.y, got delta={} expected={}",
            cam.center.y - initial_center.y, expected_dy
        );
        assert!(
            (cam.center.z - initial_center.z).abs() < 0.0001,
            "middle-click drag should NOT change center.z"
        );
    }

    #[test]
    fn orbit_right_click_does_not_pan() {
        let mut cam = EditorCamera::new();
        cam.mode = CameraMode::Orbit;
        cam.distance = 10.0;
        let initial_center = cam.center;

        let mut input = InputManager::new();
        // Hold right-click
        input.push_event(InputEvent::MouseButton(MouseButton::Right, true));
        // Move mouse by (100, 50) pixels
        input.push_event(InputEvent::MouseMove(100.0, 50.0));
        input.poll();

        cam.update(&input, 0.016);

        assert!(
            (cam.center.x - initial_center.x).abs() < 0.0001,
            "right-click drag should NOT pan center.x, got delta={}",
            cam.center.x - initial_center.x
        );
        assert!(
            (cam.center.y - initial_center.y).abs() < 0.0001,
            "right-click drag should NOT pan center.y, got delta={}",
            cam.center.y - initial_center.y
        );
    }

    #[test]
    fn orbit_left_click_drag_does_not_rotate() {
        let mut cam = EditorCamera::new();
        cam.mode = CameraMode::Orbit;
        let initial_yaw = cam.yaw;
        let initial_pitch = cam.pitch;

        let mut input = InputManager::new();
        // Hold left-click
        input.push_event(InputEvent::MouseButton(MouseButton::Left, true));
        // Move mouse by (100, 50) pixels
        input.push_event(InputEvent::MouseMove(100.0, 50.0));
        input.poll();

        cam.update(&input, 0.016);

        assert!(
            (cam.yaw - initial_yaw).abs() < 0.0001,
            "left-click drag should NOT change yaw, got delta={}",
            cam.yaw - initial_yaw
        );
        assert!(
            (cam.pitch - initial_pitch).abs() < 0.0001,
            "left-click drag should NOT change pitch, got delta={}",
            cam.pitch - initial_pitch
        );
    }

    #[test]
    fn orbit_right_click_drag_rotates_camera() {
        let mut cam = EditorCamera::new();
        cam.mode = CameraMode::Orbit;
        let initial_yaw = cam.yaw;
        let initial_pitch = cam.pitch;

        let mut input = InputManager::new();
        // Hold right-click
        input.push_event(InputEvent::MouseButton(MouseButton::Right, true));
        // Move mouse by (100, 50) pixels
        input.push_event(InputEvent::MouseMove(100.0, 50.0));
        input.poll();

        cam.update(&input, 0.016);

        let expected_yaw_delta = 100.0 * 0.005;
        let expected_pitch_delta = -(50.0 * 0.005);

        assert!(
            (cam.yaw - initial_yaw - expected_yaw_delta).abs() < 0.0001,
            "yaw should increase by dx * 0.005, got yaw={} expected_delta={}",
            cam.yaw - initial_yaw, expected_yaw_delta
        );
        assert!(
            (cam.pitch - initial_pitch - expected_pitch_delta).abs() < 0.0001,
            "pitch should decrease by dy * 0.005, got pitch={} expected_delta={}",
            cam.pitch - initial_pitch, expected_pitch_delta
        );
    }
}
