use super::*;

#[test]
fn gizmo_mode_w_selects_translate() {
    assert_eq!(resolve_gizmo_mode_pure(2, true, false, false), 0, "W should select translate (0)");
}

#[test]
fn gizmo_mode_e_selects_rotate() {
    assert_eq!(resolve_gizmo_mode_pure(0, false, true, false), 1, "E should select rotate (1)");
}

#[test]
fn gizmo_mode_r_selects_scale() {
    assert_eq!(resolve_gizmo_mode_pure(0, false, false, true), 2, "R should select scale (2)");
}

#[test]
fn gizmo_mode_no_key_preserves_current() {
    assert_eq!(resolve_gizmo_mode_pure(1, false, false, false), 1, "no key should preserve current mode");
}

#[test]
fn gizmo_mode_w_takes_precedence_over_e_and_r() {
    // W pressed together with E and R → W wins (translate)
    assert_eq!(resolve_gizmo_mode_pure(2, true, true, true), 0, "W should take precedence over E and R");
}

#[test]
fn gizmo_mode_e_takes_precedence_over_r() {
    // E and R pressed without W → E wins (rotate)
    assert_eq!(resolve_gizmo_mode_pure(0, false, true, true), 1, "E should take precedence over R");
}

#[test]
fn gizmo_mode_keys_are_independent_of_shift() {
    // The pure function doesn't know about Shift — the caller (egui input check)
    // already filters out command-modified keys. Here we just verify the
    // mapping works regardless of what modifiers were pressed.
    assert_eq!(resolve_gizmo_mode_pure(2, true, false, false), 0, "W→translate regardless of modifiers");
    assert_eq!(resolve_gizmo_mode_pure(0, false, true, false), 1, "E→rotate regardless of modifiers");
    assert_eq!(resolve_gizmo_mode_pure(0, false, false, true), 2, "R→scale regardless of modifiers");
}

#[test]
fn gizmo_rotation_x_increases_with_right_drag() {
    let rot = Vec3::new(0.0, 0.0, 0.0);
    let delta = egui::vec2(100.0, 0.0); // drag right
    let new_rot = apply_gizmo_rotation(rot, 0, delta);
    assert!(new_rot.x > 0.0, "right drag on X axis should increase X rotation");
    assert_eq!(new_rot.y, 0.0, "Y rotation should stay unchanged");
    assert_eq!(new_rot.z, 0.0, "Z rotation should stay unchanged");
}

#[test]
fn gizmo_rotation_y_increases_with_up_drag() {
    let rot = Vec3::new(0.0, 0.0, 0.0);
    let delta = egui::vec2(0.0, -100.0); // drag up (negative y in screen space)
    let new_rot = apply_gizmo_rotation(rot, 1, delta);
    // sign for Y-axis uses -drag_delta.y.signum() → -(-1.0).signum() = 1.0
    assert!(new_rot.y > 0.0, "up drag on Y axis should increase Y rotation");
    assert_eq!(new_rot.x, 0.0, "X rotation should stay unchanged");
}

#[test]
fn gizmo_scale_increases_with_right_drag() {
    let scale = Vec3::new(1.0, 1.0, 1.0);
    let delta = egui::vec2(100.0, 0.0); // drag right
    let new_scale = apply_gizmo_scale(scale, 0, delta);
    assert!(new_scale.x > 1.0, "right drag should increase X scale");
    assert_eq!(new_scale.y, 1.0, "Y scale should stay unchanged");
    assert_eq!(new_scale.z, 1.0, "Z scale should stay unchanged");
}

#[test]
fn gizmo_scale_clamped_to_minimum() {
    let scale = Vec3::new(0.02, 0.02, 0.02);
    let delta = egui::vec2(-1000.0, 0.0); // large negative drag
    let new_scale = apply_gizmo_scale(scale, 0, delta);
    assert_eq!(new_scale.x, 0.01, "scale should be clamped to minimum 0.01");
}

#[test]
fn gizmo_translation_moves_along_axis() {
    let pos = Vec3::new(0.0, 0.0, 0.0);
    let axis = Vec3::X;
    let drag = egui::vec2(100.0, 0.0);
    let axis_2d = egui::vec2(1.0, 0.0); // axis points right on screen
    let new_pos = apply_gizmo_translation(pos, axis, drag, axis_2d, 10.0, false, 0.0);
    assert!(new_pos.x > 0.0, "drag along axis should move in +X");
    assert_eq!(new_pos.y, 0.0, "Y should stay unchanged");
    assert_eq!(new_pos.z, 0.0, "Z should stay unchanged");
}

#[test]
fn gizmo_translation_with_snap() {
    let pos = Vec3::new(0.0, 0.0, 0.0);
    let axis = Vec3::X;
    let drag = egui::vec2(100.0, 0.0);
    let axis_2d = egui::vec2(1.0, 0.0);
    let new_pos = apply_gizmo_translation(pos, axis, drag, axis_2d, 10.0, true, 1.0);
    // along = 100 * 1.0 * 10 * 0.01 = 10.0 → snapped to nearest 1.0 = 10.0
    assert_eq!(new_pos.x, 10.0, "position should snap to 1.0 grid");
}

#[test]
fn snap_vec3_rounds_to_grid() {
    let v = Vec3::new(1.3, 2.7, -0.4);
    let snapped = snap_vec3(v, 1.0);
    assert_eq!(snapped, Vec3::new(1.0, 3.0, 0.0), "should round to nearest integer grid");
}

#[test]
fn snap_vec3_with_half_grid() {
    let v = Vec3::new(1.3, 2.2, 0.6);
    let snapped = snap_vec3(v, 0.5);
    assert_eq!(snapped, Vec3::new(1.5, 2.0, 0.5), "should round to nearest 0.5 grid");
}
