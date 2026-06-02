use super::*;
use rustix_platform::input::{InputEvent, InputManager, MouseButton, KeyCode};

#[test]
fn orbit_plain_wasd_does_nothing() {
    let mut cam = EditorCamera::new();
    cam.mode = CameraMode::Orbit;
    let initial_distance = cam.distance;
    let initial_yaw = cam.yaw;
    let initial_pitch = cam.pitch;

    let mut input = InputManager::new();
    // Press W, A, S, D without Shift
    for key in [KeyCode::W, KeyCode::A, KeyCode::S, KeyCode::D] {
        input.push_event(InputEvent::KeyPress(key));
    }
    input.poll();

    cam.update(&input, 0.016);

    assert!(
        (cam.distance - initial_distance).abs() < 0.0001,
        "plain WASD should NOT change distance, got delta={}",
        cam.distance - initial_distance
    );
    assert!(
        (cam.yaw - initial_yaw).abs() < 0.0001,
        "plain WASD should NOT change yaw, got delta={}",
        cam.yaw - initial_yaw
    );
    assert!(
        (cam.pitch - initial_pitch).abs() < 0.0001,
        "plain WASD should NOT change pitch, got delta={}",
        cam.pitch - initial_pitch
    );
}

#[test]
fn orbit_shift_w_zooms_in_shift_s_zooms_out() {
    let dt = 0.016;
    let zoom_speed = 3.0 * dt;

    // Shift+W decreases distance
    let mut cam = EditorCamera::new();
    cam.mode = CameraMode::Orbit;
    let initial_distance = cam.distance;
    let mut input = InputManager::new();
    input.push_event(InputEvent::KeyPress(KeyCode::ShiftLeft));
    input.push_event(InputEvent::KeyPress(KeyCode::W));
    input.poll();
    cam.update(&input, dt);
    let expected_distance = initial_distance - zoom_speed;
    assert!(
        (cam.distance - expected_distance).abs() < 0.0001,
        "Shift+W should decrease distance: got {} expected {}",
        cam.distance, expected_distance
    );

    // Shift+S increases distance
    let mut cam2 = EditorCamera::new();
    cam2.mode = CameraMode::Orbit;
    let initial_distance2 = cam2.distance;
    let mut input2 = InputManager::new();
    input2.push_event(InputEvent::KeyPress(KeyCode::ShiftRight));
    input2.push_event(InputEvent::KeyPress(KeyCode::S));
    input2.poll();
    cam2.update(&input2, dt);
    let expected_distance2 = initial_distance2 + zoom_speed;
    assert!(
        (cam2.distance - expected_distance2).abs() < 0.0001,
        "Shift+S should increase distance: got {} expected {}",
        cam2.distance, expected_distance2
    );
}

#[test]
fn orbit_shift_a_rotates_left_shift_d_rotates_right() {
    let dt = 0.016;
    let rot_speed = 2.0 * dt;

    // Shift+A decreases yaw
    let mut cam = EditorCamera::new();
    cam.mode = CameraMode::Orbit;
    let initial_yaw = cam.yaw;
    let mut input = InputManager::new();
    input.push_event(InputEvent::KeyPress(KeyCode::ShiftLeft));
    input.push_event(InputEvent::KeyPress(KeyCode::A));
    input.poll();
    cam.update(&input, dt);
    let expected_yaw = initial_yaw - rot_speed;
    assert!(
        (cam.yaw - expected_yaw).abs() < 0.0001,
        "Shift+A should decrease yaw: got {} expected {}",
        cam.yaw, expected_yaw
    );

    // Shift+D increases yaw
    let mut cam2 = EditorCamera::new();
    cam2.mode = CameraMode::Orbit;
    let initial_yaw2 = cam2.yaw;
    let mut input2 = InputManager::new();
    input2.push_event(InputEvent::KeyPress(KeyCode::ShiftRight));
    input2.push_event(InputEvent::KeyPress(KeyCode::D));
    input2.poll();
    cam2.update(&input2, dt);
    let expected_yaw2 = initial_yaw2 + rot_speed;
    assert!(
        (cam2.yaw - expected_yaw2).abs() < 0.0001,
        "Shift+D should increase yaw: got {} expected {}",
        cam2.yaw, expected_yaw2
    );
}

#[test]
fn orbit_shift_q_pitches_up_shift_e_pitches_down() {
    let dt = 0.016;
    let rot_speed = 2.0 * dt;

    // Shift+Q decreases pitch
    let mut cam = EditorCamera::new();
    cam.mode = CameraMode::Orbit;
    let initial_pitch = cam.pitch;
    let mut input = InputManager::new();
    input.push_event(InputEvent::KeyPress(KeyCode::ShiftLeft));
    input.push_event(InputEvent::KeyPress(KeyCode::Q));
    input.poll();
    cam.update(&input, dt);
    let expected_pitch = initial_pitch - rot_speed;
    assert!(
        (cam.pitch - expected_pitch).abs() < 0.0001,
        "Shift+Q should decrease pitch: got {} expected {}",
        cam.pitch, expected_pitch
    );

    // Shift+E increases pitch
    let mut cam2 = EditorCamera::new();
    cam2.mode = CameraMode::Orbit;
    let initial_pitch2 = cam2.pitch;
    let mut input2 = InputManager::new();
    input2.push_event(InputEvent::KeyPress(KeyCode::ShiftRight));
    input2.push_event(InputEvent::KeyPress(KeyCode::E));
    input2.poll();
    cam2.update(&input2, dt);
    let expected_pitch2 = initial_pitch2 + rot_speed;
    assert!(
        (cam2.pitch - expected_pitch2).abs() < 0.0001,
        "Shift+E should increase pitch: got {} expected {}",
        cam2.pitch, expected_pitch2
    );
}

#[test]
fn orbit_shift_qe_clamping() {
    let dt = 0.016;
    let rot_speed = 2.0 * dt;

    // Pitch should clamp at -1.4 when looking too far up
    let mut cam = EditorCamera::new();
    cam.mode = CameraMode::Orbit;
    cam.pitch = -1.39; // near upper clamp
    let mut input = InputManager::new();
    input.push_event(InputEvent::KeyPress(KeyCode::ShiftLeft));
    input.push_event(InputEvent::KeyPress(KeyCode::Q));
    input.poll();
    cam.update(&input, dt);
    assert!(
        cam.pitch >= -1.4,
        "pitch should be clamped >= -1.4, got {}", cam.pitch
    );

    // Pitch should clamp at 1.4 when looking too far down
    let mut cam2 = EditorCamera::new();
    cam2.mode = CameraMode::Orbit;
    cam2.pitch = 1.39; // near lower clamp
    let mut input2 = InputManager::new();
    input2.push_event(InputEvent::KeyPress(KeyCode::ShiftRight));
    input2.push_event(InputEvent::KeyPress(KeyCode::E));
    input2.poll();
    cam2.update(&input2, dt);
    assert!(
        cam2.pitch <= 1.4,
        "pitch should be clamped <= 1.4, got {}", cam2.pitch
    );
}

#[test]
fn orbit_ctrl_s_does_nothing() {
    let mut cam = EditorCamera::new();
    cam.mode = CameraMode::Orbit;
    let initial_distance = cam.distance;
    let initial_yaw = cam.yaw;
    let initial_pitch = cam.pitch;
    let initial_center = cam.center;

    let mut input = InputManager::new();
    input.push_event(InputEvent::KeyPress(KeyCode::ControlLeft));
    input.push_event(InputEvent::KeyPress(KeyCode::S));
    input.poll();

    cam.update(&input, 0.016);

    assert!(
        (cam.distance - initial_distance).abs() < 0.0001,
        "Ctrl+S should NOT change distance"
    );
    assert!(
        (cam.yaw - initial_yaw).abs() < 0.0001,
        "Ctrl+S should NOT change yaw"
    );
    assert!(
        (cam.pitch - initial_pitch).abs() < 0.0001,
        "Ctrl+S should NOT change pitch"
    );
    assert!(
        (cam.center - initial_center).length() < 0.0001,
        "Ctrl+S should NOT change center"
    );
}

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
fn first_person_right_click_drag_looks_around() {
    let mut cam = EditorCamera::new();
    cam.mode = CameraMode::FirstPerson;
    let initial_yaw = cam.yaw;
    let initial_pitch = cam.pitch;

    let mut input = InputManager::new();
    input.push_event(InputEvent::MouseButton(MouseButton::Right, true));
    input.push_event(InputEvent::MouseMove(100.0, 50.0));
    input.poll();

    cam.update(&input, 0.016);

    let expected_yaw_delta = 100.0 * 0.005;
    let expected_pitch_delta = -(50.0 * 0.005);
    assert!(
        (cam.yaw - initial_yaw - expected_yaw_delta).abs() < 0.0001,
        "FP right-click should change yaw"
    );
    assert!(
        (cam.pitch - initial_pitch - expected_pitch_delta).abs() < 0.0001,
        "FP right-click should change pitch"
    );
}

#[test]
fn first_person_shift_wasd_moves_position() {
    let dt = 0.016;
    let move_speed = 5.0 * dt;

    let mut cam = EditorCamera::new();
    cam.mode = CameraMode::FirstPerson;
    let initial_pos = cam.position;

    let mut input = InputManager::new();
    input.push_event(InputEvent::KeyPress(KeyCode::ShiftLeft));
    input.push_event(InputEvent::KeyPress(KeyCode::W));
    input.poll();
    cam.update(&input, dt);

    // W should move forward (position changes)
    assert!(
        cam.position != initial_pos,
        "Shift+W in FP should move position"
    );

    // S should move backward
    let mut cam2 = EditorCamera::new();
    cam2.mode = CameraMode::FirstPerson;
    let pos2 = cam2.position;
    let mut input2 = InputManager::new();
    input2.push_event(InputEvent::KeyPress(KeyCode::ShiftRight));
    input2.push_event(InputEvent::KeyPress(KeyCode::S));
    input2.poll();
    cam2.update(&input2, dt);
    assert!(
        cam2.position != pos2,
        "Shift+S in FP should move position"
    );
}

#[test]
fn first_person_plain_wasd_does_nothing() {
    let mut cam = EditorCamera::new();
    cam.mode = CameraMode::FirstPerson;
    let initial_pos = cam.position;

    let mut input = InputManager::new();
    for key in [KeyCode::W, KeyCode::A, KeyCode::S, KeyCode::D] {
        input.push_event(InputEvent::KeyPress(key));
    }
    input.poll();

    cam.update(&input, 0.016);

    assert!(
        (cam.position - initial_pos).length() < 0.0001,
        "plain WASD in FP mode should NOT move position"
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
