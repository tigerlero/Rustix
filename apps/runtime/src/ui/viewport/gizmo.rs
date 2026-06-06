use rustix_core::math::Vec3;

/// Resolve gizmo mode from egui keyboard input.
/// E → Translate (0), R → Rotate (1), T → Scale (2).
/// Command-modified keys are ignored so they don't conflict with shortcuts.
pub fn resolve_gizmo_mode(current: usize, ctx: &egui::Context) -> usize {
    let e = ctx.input(|i| i.key_pressed(egui::Key::E) && !i.modifiers.command);
    let r = ctx.input(|i| i.key_pressed(egui::Key::R) && !i.modifiers.command);
    let t = ctx.input(|i| i.key_pressed(egui::Key::T) && !i.modifiers.command);
    resolve_gizmo_mode_pure(current, e, r, t)
}

/// Apply rotation gizmo drag: rotates the entity around the active axis.
/// `drag_delta` is in screen pixels; magnitude maps to rotation angle.
pub fn apply_gizmo_rotation(entity_rot: Vec3, gizmo_active: usize, drag_delta: egui::Vec2) -> Vec3 {
    let angle = drag_delta.length() * 0.01;
    let sign = if drag_delta.x.abs() > drag_delta.y.abs() { drag_delta.x.signum() } else { -drag_delta.y.signum() };
    let s = sign * if gizmo_active == 0 { 1.0 } else if gizmo_active == 1 { 1.0 } else { 1.0 };
    let mut new_rot = entity_rot;
    match gizmo_active {
        0 => { new_rot.x += angle * s; }
        1 => { new_rot.y += angle * s; }
        _ => { new_rot.z += angle * s; }
    }
    new_rot
}

/// Apply scale gizmo drag: scales the entity along the active axis.
/// Result clamped to minimum 0.01 to prevent zero/negative scale.
pub fn apply_gizmo_scale(entity_scale: Vec3, gizmo_active: usize, drag_delta: egui::Vec2) -> Vec3 {
    let scale_delta = drag_delta.length() * 0.01;
    let sign = if drag_delta.x.abs() > drag_delta.y.abs() { drag_delta.x.signum() } else { drag_delta.y.signum() };
    let mut new_scale = entity_scale;
    let val = (new_scale.to_array()[gizmo_active] + scale_delta * sign).max(0.01);
    match gizmo_active {
        0 => { new_scale.x = val; }
        1 => { new_scale.y = val; }
        _ => { new_scale.z = val; }
    }
    new_scale
}

/// Snap a Vec3 to the nearest grid multiple of `snap_size`.
pub fn snap_vec3(v: Vec3, snap_size: f32) -> Vec3 {
    Vec3::new(
        (v.x / snap_size).round() * snap_size,
        (v.y / snap_size).round() * snap_size,
        (v.z / snap_size).round() * snap_size,
    )
}

/// Apply translation gizmo drag: moves entity along `axis_dir` by projected screen delta.
/// `axis_2d` is the normalized screen-space direction of the axis.
pub fn apply_gizmo_translation(
    entity_pos: Vec3, axis_dir: Vec3, drag_delta: egui::Vec2, axis_2d: egui::Vec2,
    cam_distance: f32, snap_enabled: bool, snap_size: f32,
) -> Vec3 {
    let along = (drag_delta.x * axis_2d.x + drag_delta.y * axis_2d.y) * cam_distance * 0.01;
    let mut new_pos = entity_pos + axis_dir * along;
    if snap_enabled && snap_size > 0.0 {
        new_pos = snap_vec3(new_pos, snap_size);
    }
    new_pos
}

/// Pure function resolving gizmo mode from key press booleans.
/// E → Translate (0), R → Rotate (1), T → Scale (2).
/// If multiple keys are pressed, E takes precedence, then R, then T.
pub fn resolve_gizmo_mode_pure(current: usize, e_pressed: bool, r_pressed: bool, t_pressed: bool) -> usize {
    if e_pressed { 0 }
    else if r_pressed { 1 }
    else if t_pressed { 2 }
    else { current }
}
