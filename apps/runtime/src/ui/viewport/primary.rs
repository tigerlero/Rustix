use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4, Mat4};
use rustix_audio::{AudioSource, AudioListener};
use rustix_physics::{RigidBody, Collider};
use rustix_render::Camera;

use crate::camera::EditorCamera;
use crate::project::{AppScreen, CameraBookmark};
use crate::scene::{Transform, Name, MeshComponent, Material, world_transform};
use crate::undo::{UndoHistory, EditorAction};

use super::gizmo;
use super::manager::viewport_texture_id;

fn draw_grid(painter: &egui::Painter, world_to_screen: impl Fn(Vec3) -> Option<egui::Pos2>) {
    let grid_half = 20.0f32;
    let grid_step = 1.0f32;
    let major_step = 5.0f32;
    let grid_color_minor = egui::Color32::from_rgba_premultiplied(100, 110, 130, 30);
    let grid_color_major = egui::Color32::from_rgba_premultiplied(100, 110, 130, 70);

    let mut z = -grid_half;
    while z <= grid_half {
        let near = Vec3::new(-grid_half, 0.0, z);
        let far = Vec3::new(grid_half, 0.0, z);
        if let (Some(a), Some(b)) = (world_to_screen(near), world_to_screen(far)) {
            let is_major = (z % major_step).abs() < 0.01;
            let col = if is_major { grid_color_major } else { grid_color_minor };
            painter.line_segment([a, b], egui::Stroke::new(if is_major { 1.5 } else { 0.5 }, col));
        }
        z += grid_step;
    }
    let mut x = -grid_half;
    while x <= grid_half {
        let near = Vec3::new(x, 0.0, -grid_half);
        let far = Vec3::new(x, 0.0, grid_half);
        if let (Some(a), Some(b)) = (world_to_screen(near), world_to_screen(far)) {
            let is_major = (x % major_step).abs() < 0.01;
            let col = if is_major { grid_color_major } else { grid_color_minor };
            painter.line_segment([a, b], egui::Stroke::new(if is_major { 1.5 } else { 0.5 }, col));
        }
        x += grid_step;
    }
}

fn draw_camera_overlay(painter: &egui::Painter, rect: egui::Rect, cam: &EditorCamera) {
    let text = format!(
        "Camera: dist={:.1} yaw={:.2} pitch={:.2} | Right-drag to orbit | Middle-drag to pan",
        cam.distance, cam.yaw, cam.pitch
    );
    painter.text(
        rect.left_bottom() + egui::vec2(8.0, -8.0),
        egui::Align2::LEFT_BOTTOM,
        text,
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgba_premultiplied(200, 200, 200, 180),
    );
}

fn draw_toolbar(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cam: &mut EditorCamera,
    bookmarks: &mut Vec<CameraBookmark>,
    screen: &mut AppScreen,
) -> (usize, bool, bool, f32) {
    let gizmo_mode_id = egui::Id::new("gizmo_mode");
    let gizmo_space_id = egui::Id::new("gizmo_space");
    let snap_id = egui::Id::new("gizmo_snap");
    let snap_size_id = egui::Id::new("gizmo_snap_size");
    let bookmark_popup_id = egui::Id::new("viewport_bookmark_popup");

    let mut gizmo_mode = ctx.data(|d| d.get_temp::<usize>(gizmo_mode_id)).unwrap_or(0);
    let mut gizmo_space = ctx.data(|d| d.get_temp::<bool>(gizmo_space_id)).unwrap_or(false);
    let mut snap_enabled = ctx.data(|d| d.get_temp::<bool>(snap_id)).unwrap_or(false);
    let mut snap_size = ctx.data(|d| d.get_temp::<f32>(snap_size_id)).unwrap_or(0.5);

    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.add_space(4.0);
            let toolbar_bg = egui::Color32::from_rgba_premultiplied(30, 30, 38, 220);
            let toolbar_rect = ui.available_rect_before_wrap();
            let rect = egui::Rect::from_min_size(toolbar_rect.min, egui::vec2(320.0, 28.0));
            ui.painter().rect_filled(rect, 6.0, toolbar_bg);
            ui.painter().rect_stroke(rect, 6.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 75)), egui::StrokeKind::Inside);
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(4.0)), |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Gizmo:").weak().size(11.0));
                    if ui.selectable_label(gizmo_mode == 0, egui::RichText::new("E").size(12.0).strong()).clicked() { gizmo_mode = 0; }
                    if ui.selectable_label(gizmo_mode == 1, egui::RichText::new("R").size(12.0).strong()).clicked() { gizmo_mode = 1; }
                    if ui.selectable_label(gizmo_mode == 2, egui::RichText::new("T").size(12.0).strong()).clicked() { gizmo_mode = 2; }
                    ui.add_space(4.0);
                    if ui.selectable_label(gizmo_space, egui::RichText::new("Local").size(11.0)).clicked() { gizmo_space = !gizmo_space; }
                    ui.add_space(4.0);
                    if ui.selectable_label(snap_enabled, egui::RichText::new("Snap").size(11.0)).clicked() { snap_enabled = !snap_enabled; }
                    if snap_enabled {
                        ui.add(egui::DragValue::new(&mut snap_size).speed(0.1).range(0.01..=10.0).prefix("").suffix("").clamp_existing_to_range(false));
                    }
                    ui.add_space(8.0);
                    if *screen == AppScreen::PlayTest {
                        if ui.button(egui::RichText::new("⏹ Stop").size(11.0).color(egui::Color32::from_rgb(255, 100, 100))).clicked() {
                            *screen = AppScreen::Editor;
                        }
                    } else {
                        if ui.button(egui::RichText::new("▶ Play").size(11.0).color(egui::Color32::from_rgb(100, 255, 100))).clicked() {
                            *screen = AppScreen::PlayTest;
                        }
                    }
                    ui.add_space(8.0);
                    let btn = ui.button(egui::RichText::new("Bookmarks").size(11.0));
                    let mut show_bookmarks = ctx.data(|d| d.get_temp::<bool>(bookmark_popup_id)).unwrap_or(false);
                    if btn.clicked() {
                        show_bookmarks = !show_bookmarks;
                        ctx.data_mut(|d| d.insert_temp(bookmark_popup_id, show_bookmarks));
                    }
                    if show_bookmarks {
                        let popup_pos = btn.rect.left_bottom() + egui::vec2(0.0, 4.0);
                        egui::Window::new("Bookmarks")
                            .id(bookmark_popup_id)
                            .title_bar(false)
                            .resizable(false)
                            .auto_sized()
                            .fixed_pos(popup_pos)
                            .show(ctx, |ui| {
                                ui.set_min_width(160.0);
                                if ui.button("Save Current View").clicked() {
                                    let name = format!("Bookmark {}", bookmarks.len() + 1);
                                    bookmarks.push(CameraBookmark {
                                        name,
                                        position: cam.position.into(),
                                        center: cam.center.into(),
                                        yaw: cam.yaw,
                                        pitch: cam.pitch,
                                        distance: cam.distance,
                                        mode: cam.mode,
                                    });
                                    ctx.data_mut(|d| d.insert_temp(bookmark_popup_id, false));
                                }
                                ui.separator();
                                let mut to_remove = None;
                                for (i, bm) in bookmarks.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        if ui.selectable_label(false, &bm.name).clicked() {
                                            cam.position = Vec3::from(bm.position);
                                            cam.center = Vec3::from(bm.center);
                                            cam.yaw = bm.yaw;
                                            cam.pitch = bm.pitch;
                                            cam.distance = bm.distance;
                                            cam.mode = bm.mode;
                                        }
                                        if ui.small_button("×").clicked() {
                                            to_remove = Some(i);
                                        }
                                    });
                                }
                                if let Some(idx) = to_remove {
                                    bookmarks.remove(idx);
                                }
                                if bookmarks.is_empty() {
                                    ui.label(egui::RichText::new("No bookmarks").weak());
                                }
                            });
                    }
                });
            });
        });
    });

    ctx.data_mut(|d| d.insert_temp(snap_id, snap_enabled));
    ctx.data_mut(|d| d.insert_temp(snap_size_id, snap_size));
    ctx.data_mut(|d| d.insert_temp(gizmo_space_id, gizmo_space));
    ctx.data_mut(|d| d.insert_temp(gizmo_mode_id, gizmo_mode));

    (gizmo_mode, gizmo_space, snap_enabled, snap_size)
}

fn draw_scene_overlays(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    rect: egui::Rect,
    world: &mut EcsWorld,
    cam: &EditorCamera,
    selected_entities: &std::cell::RefCell<Vec<hecs::Entity>>,
    world_to_screen: impl Fn(Vec3) -> Option<egui::Pos2>,
    undo_history: &std::cell::RefCell<UndoHistory>,
    dirty: &std::cell::Cell<bool>,
) -> Option<hecs::Entity> {
    let gizmo_active_id = egui::Id::new("gizmo_active");
    let gizmo_drag_start_id = egui::Id::new("gizmo_drag_start");
    let gizmo_entity_pos_id = egui::Id::new("gizmo_entity_pos");
    let gizmo_entity_rot_id = egui::Id::new("gizmo_entity_rot");
    let gizmo_entity_scale_id = egui::Id::new("gizmo_entity_scale");
    let gizmo_undo_transform_id = egui::Id::new("gizmo_undo_transform");
    let gizmo_dragging_id = egui::Id::new("gizmo_dragging");
    let gizmo_local_axes_id = egui::Id::new("gizmo_local_axes");
    let gizmo_drag_mode_id = egui::Id::new("gizmo_drag_mode");
    let gizmo_mode_id = egui::Id::new("gizmo_mode");
    let gizmo_space_id = egui::Id::new("gizmo_space");
    let snap_id = egui::Id::new("gizmo_snap");
    let snap_size_id = egui::Id::new("gizmo_snap_size");

    let mut gizmo_active = ctx.data(|d| d.get_temp::<usize>(gizmo_active_id)).unwrap_or(usize::MAX);
    let mut gizmo_drag_start = ctx.data(|d| d.get_temp::<egui::Vec2>(gizmo_drag_start_id)).unwrap_or(egui::Vec2::ZERO);
    let mut gizmo_entity_pos = ctx.data(|d| d.get_temp::<Vec3>(gizmo_entity_pos_id)).unwrap_or(Vec3::ZERO);
    let mut gizmo_entity_rot = ctx.data(|d| d.get_temp::<Vec3>(gizmo_entity_rot_id)).unwrap_or(Vec3::ZERO);
    let mut gizmo_entity_scale = ctx.data(|d| d.get_temp::<Vec3>(gizmo_entity_scale_id)).unwrap_or(Vec3::ONE);
    let mut gizmo_undo_transform: Option<Transform> = ctx.data(|d| d.get_temp::<Transform>(gizmo_undo_transform_id));
    let mut gizmo_dragging = ctx.data(|d| d.get_temp::<bool>(gizmo_dragging_id)).unwrap_or(false);

    let gizmo_mode = ctx.data(|d| d.get_temp::<usize>(gizmo_mode_id)).unwrap_or(0);
    let gizmo_space = ctx.data(|d| d.get_temp::<bool>(gizmo_space_id)).unwrap_or(false);
    let snap_enabled = ctx.data(|d| d.get_temp::<bool>(snap_id)).unwrap_or(false);
    let snap_size = ctx.data(|d| d.get_temp::<f32>(snap_size_id)).unwrap_or(0.5);

    let mut clicked_entity = None;
    let mut deferred_new_pos: Option<Vec3> = None;
    let mut deferred_new_rot: Option<Vec3> = None;
    let mut deferred_new_scale: Option<Vec3> = None;

    let mut entities: Vec<(hecs::Entity, Transform, Vec3, f32)> = Vec::new();
    for (entity, transform) in world.query::<(Entity, &Transform)>().iter() {
        let world_pos = {
            let m = world_transform(world, entity);
            let (_s, _r, pos) = m.to_scale_rotation_translation();
            pos
        };
        let dist = (world_pos - cam.center).length();
        entities.push((entity, *transform, world_pos, dist));
    }
    entities.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));

    for (entity, transform, world_pos, _dist) in entities {
        let is_selected = selected_entities.borrow().contains(&entity);
        if let Some(screen_pos) = world_to_screen(world_pos) {
            let entity_color = if is_selected {
                egui::Color32::from_rgb(70, 150, 250)
            } else {
                egui::Color32::from_rgb(200, 200, 220)
            };
            let hover_color = egui::Color32::from_rgb(255, 255, 100);

            let avg_scale = (transform.scale.x + transform.scale.y + transform.scale.z) / 3.0;
            let pick_radius = ((avg_scale / _dist.max(0.001)) * rect.height() * 0.5).max(12.0).min(120.0);

            let ent_hit_rect = egui::Rect::from_center_size(screen_pos, egui::vec2(pick_radius * 2.0, pick_radius * 2.0));
            let ent_resp = ui.interact(
                ent_hit_rect,
                egui::Id::new(("vp_ent", entity.to_bits().get())),
                egui::Sense::click()
            );
            if ent_resp.clicked() { clicked_entity = Some(entity); }

            let dot_color = if ent_resp.hovered() { hover_color } else { entity_color };
            ui.painter().circle_filled(screen_pos, 5.0, dot_color);
            ui.painter().circle_stroke(screen_pos, 5.0, egui::Stroke::new(1.5, egui::Color32::WHITE));

            if let Ok(aud) = world.get::<&AudioSource>(entity) {
                let aud_min_color = egui::Color32::from_rgba_premultiplied(255, 120, 180, 80);
                let aud_max_color = egui::Color32::from_rgba_premultiplied(255, 120, 180, 30);
                for (radius, color) in &[(aud.min_distance, aud_min_color), (aud.max_distance, aud_max_color)] {
                    let num_segments = 48u32;
                    let heights = [-0.5f32, 0.0, 0.5];
                    for &h_frac in &heights {
                        let h = *radius * h_frac * 0.3;
                        let mut last_point: Option<egui::Pos2> = None;
                        for i in 0..=num_segments {
                            let angle = (i as f32 / num_segments as f32) * std::f32::consts::TAU;
                            let r_horiz = (*radius * (1.0 - (h_frac * 0.3).powi(2)).sqrt()).max(0.0);
                            let wp = world_pos + Vec3::new(r_horiz * angle.cos(), h, r_horiz * angle.sin());
                            if let Some(sp) = world_to_screen(wp) {
                                if let Some(lp) = last_point {
                                    ui.painter().line_segment([lp, sp], egui::Stroke::new(1.0, *color));
                                }
                                last_point = Some(sp);
                            }
                        }
                    }
                }
            }

            if is_selected {
                let o = screen_pos;
                ui.painter().line_segment([o - egui::vec2(5.0, 0.0), o + egui::vec2(5.0, 0.0)], egui::Stroke::new(1.0, egui::Color32::WHITE));
                ui.painter().line_segment([o - egui::vec2(0.0, 5.0), o + egui::vec2(0.0, 5.0)], egui::Stroke::new(1.0, egui::Color32::WHITE));

                let show_gizmo = selected_entities.borrow().len() == 1 && selected_entities.borrow().first() == Some(&entity);
                if show_gizmo {
                let local_axes = if gizmo_space {
                    let m = world_transform(world, entity);
                    let (_s, rot, _p) = m.to_scale_rotation_translation();
                    [rot * Vec3::X, rot * Vec3::Y, rot * Vec3::Z]
                } else {
                    [Vec3::X, Vec3::Y, Vec3::Z]
                };
                let axis_colors = [
                    (local_axes[0], egui::Color32::from_rgb(220, 60, 60)),
                    (local_axes[1], egui::Color32::from_rgb(60, 200, 60)),
                    (local_axes[2], egui::Color32::from_rgb(60, 100, 220)),
                ];
                let gizmo_len = 85.0;
                let effective_mode = if !gizmo_dragging {
                    if ui.input(|i| i.modifiers.ctrl) { 2 }
                    else if ui.input(|i| i.modifiers.shift) { 1 }
                    else { gizmo_mode }
                } else {
                    ctx.data(|d| d.get_temp::<usize>(gizmo_drag_mode_id)).unwrap_or(gizmo_mode)
                };

                for (axis_idx, (axis_dir, color)) in axis_colors.iter().enumerate() {
                    let is_active = gizmo_active == axis_idx;
                    let handle_color = if is_active { egui::Color32::WHITE } else { *color };
                    let handle_id = egui::Id::new(("gizmo_h", entity.to_bits().get(), axis_idx));

                    match effective_mode {
                        1 => {
                            let ring_world_r = cam.distance * 0.12;
                            let (plane_a, plane_b) = match axis_idx {
                                0 => (local_axes[1], local_axes[2]),
                                1 => (local_axes[0], local_axes[2]),
                                _ => (local_axes[0], local_axes[1]),
                            };
                            let num_segments = 32;
                            let mut last_screen: Option<egui::Pos2> = None;
                            for i in 0..=num_segments {
                                let angle = (i as f32 / num_segments as f32) * std::f32::consts::TAU;
                                let pt_world = world_pos + ring_world_r * (angle.cos() * plane_a + angle.sin() * plane_b);
                                if let Some(pt_screen) = world_to_screen(pt_world) {
                                    if let Some(last) = last_screen {
                                        ui.painter().line_segment([last, pt_screen], egui::Stroke::new(1.5, *color));
                                    }
                                    last_screen = Some(pt_screen);
                                }
                            }
                            let handle_world = world_pos + ring_world_r * (0.707 * plane_a + 0.707 * plane_b);
                            if let Some(handle_screen) = world_to_screen(handle_world) {
                                let handle_rect = egui::Rect::from_center_size(handle_screen, egui::vec2(48.0, 48.0));
                                let handle_resp = ui.interact(handle_rect, handle_id, egui::Sense::drag());
                                ui.painter().circle_filled(handle_screen, 6.0, handle_color);
                                ui.painter().circle_stroke(handle_screen, 6.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                                if handle_resp.drag_started() {
                                    gizmo_active = axis_idx;
                                    gizmo_drag_start = handle_resp.interact_pointer_pos().unwrap_or(handle_screen).to_vec2();
                                    gizmo_entity_pos = world_pos;
                                    gizmo_entity_rot = transform.rotation;
                                    gizmo_entity_scale = transform.scale;
                                    gizmo_undo_transform = Some(Transform { position: transform.position, rotation: transform.rotation, scale: transform.scale });
                                    ctx.data_mut(|d| d.insert_temp(gizmo_local_axes_id, local_axes));
                                    ctx.data_mut(|d| d.insert_temp(gizmo_drag_mode_id, effective_mode));
                                    gizmo_dragging = true;
                                }
                            }
                        }
                        2 => {
                            let tip_world = world_pos + *axis_dir * 2.0;
                            if let Some(tip_screen) = world_to_screen(tip_world) {
                                let v = tip_screen - screen_pos;
                                let len = (v.x * v.x + v.y * v.y).sqrt();
                                let dir_2d = if len > 0.0 { v / len } else { egui::Vec2::ZERO };
                                let handle_screen = screen_pos + dir_2d * gizmo_len;

                                ui.painter().line_segment([screen_pos, handle_screen], egui::Stroke::new(1.5, *color));

                                let handle_rect = egui::Rect::from_center_size(handle_screen, egui::vec2(44.0, 44.0));
                                let handle_resp = ui.interact(handle_rect, handle_id, egui::Sense::drag());

                                let half = 7.0;
                                let square_rect = egui::Rect::from_center_size(handle_screen, egui::vec2(half * 2.0, half * 2.0));
                                ui.painter().rect_filled(square_rect, 0.0, handle_color);
                                ui.painter().rect_stroke(square_rect, 0.0, egui::Stroke::new(1.5, egui::Color32::WHITE), egui::StrokeKind::Inside);

                                if handle_resp.drag_started() {
                                    gizmo_active = axis_idx;
                                    gizmo_drag_start = handle_resp.interact_pointer_pos().unwrap_or(handle_screen).to_vec2();
                                    gizmo_entity_pos = world_pos;
                                    gizmo_entity_rot = transform.rotation;
                                    gizmo_entity_scale = transform.scale;
                                    gizmo_undo_transform = Some(Transform { position: transform.position, rotation: transform.rotation, scale: transform.scale });
                                    ctx.data_mut(|d| d.insert_temp(gizmo_local_axes_id, local_axes));
                                    ctx.data_mut(|d| d.insert_temp(gizmo_drag_mode_id, effective_mode));
                                    gizmo_dragging = true;
                                }
                            }
                        }
                        _ => {
                            let tip_world = world_pos + *axis_dir * 2.0;
                            if let Some(tip_screen) = world_to_screen(tip_world) {
                                let v = tip_screen - screen_pos;
                                let len = (v.x * v.x + v.y * v.y).sqrt();
                                let dir_2d = if len > 0.0 { v / len } else { egui::Vec2::ZERO };
                                let handle_screen = screen_pos + dir_2d * gizmo_len;

                                ui.painter().line_segment([screen_pos, handle_screen], egui::Stroke::new(1.5, *color));

                                let handle_rect = egui::Rect::from_center_size(handle_screen, egui::vec2(44.0, 44.0));
                                let handle_resp = ui.interact(handle_rect, handle_id, egui::Sense::drag());

                                ui.painter().circle_filled(handle_screen, 5.5, handle_color);
                                ui.painter().circle_stroke(handle_screen, 5.5, egui::Stroke::new(2.0, egui::Color32::WHITE));

                                if handle_resp.drag_started() {
                                    gizmo_active = axis_idx;
                                    gizmo_drag_start = handle_resp.interact_pointer_pos().unwrap_or(handle_screen).to_vec2();
                                    gizmo_entity_pos = world_pos;
                                    gizmo_entity_rot = transform.rotation;
                                    gizmo_entity_scale = transform.scale;
                                    gizmo_undo_transform = Some(Transform { position: transform.position, rotation: transform.rotation, scale: transform.scale });
                                    ctx.data_mut(|d| d.insert_temp(gizmo_local_axes_id, local_axes));
                                    ctx.data_mut(|d| d.insert_temp(gizmo_drag_mode_id, effective_mode));
                                    gizmo_dragging = true;
                                }
                            }
                        }
                    }
                }
                
                if gizmo_active != usize::MAX {
                    if let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.latest_pos()) {
                        if ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                            let stored_axes = ctx.data(|d| d.get_temp::<[Vec3; 3]>(gizmo_local_axes_id))
                                .unwrap_or([Vec3::X, Vec3::Y, Vec3::Z]);
                            let axis_dir = stored_axes[gizmo_active];
                            let drag_delta = pointer_pos.to_vec2() - gizmo_drag_start;
                            let gizmo_drag_mode = ctx.data(|d| d.get_temp::<usize>(gizmo_drag_mode_id)).unwrap_or(gizmo_mode);

                            match gizmo_drag_mode {
                                1 => {
                                    deferred_new_rot = Some(gizmo::apply_gizmo_rotation(
                                        gizmo_entity_rot, gizmo_active, drag_delta,
                                    ));
                                }
                                2 => {
                                    deferred_new_scale = Some(gizmo::apply_gizmo_scale(
                                        gizmo_entity_scale, gizmo_active, drag_delta,
                                    ));
                                }
                                _ => {
                                    if let (Some(tip), Some(base)) = (
                                        world_to_screen(gizmo_entity_pos + axis_dir),
                                        world_to_screen(gizmo_entity_pos),
                                    ) {
                                        let v = tip - base;
                                        let len = (v.x * v.x + v.y * v.y).sqrt();
                                        let axis_2d = if len > 0.0 { v / len } else { egui::Vec2::ZERO };
                                        deferred_new_pos = Some(gizmo::apply_gizmo_translation(
                                            gizmo_entity_pos, axis_dir, drag_delta, axis_2d,
                                            cam.distance, snap_enabled, snap_size,
                                        ));
                                    }
                                }
                            }
                        } else {
                            gizmo_active = usize::MAX;
                        }
                    } else {
                        gizmo_active = usize::MAX;
                    }
                }
                }
            }
        }
    }

    // Apply deferred transforms
    let primary_entity = selected_entities.borrow().first().copied();
    if let (Some(sel), Some(new_pos)) = (primary_entity, deferred_new_pos) {
        let parent_entity = world.get::<&crate::scene::Parent>(sel).ok().and_then(|p| p.0);
        let parent_matrix = parent_entity.map(|pe| world_transform(world, pe)).unwrap_or(Mat4::IDENTITY);
        let parent_inv = parent_matrix.inverse();
        for (e, t) in world.query_mut::<(Entity, &mut Transform)>() {
            if e == sel {
                let local = parent_inv * Vec4::new(new_pos.x, new_pos.y, new_pos.z, 1.0);
                t.position = Vec3::new(local.x, local.y, local.z);
                dirty.set(true);
                break;
            }
        }
    }
    if let (Some(sel), Some(new_rot)) = (primary_entity, deferred_new_rot) {
        for (e, t) in world.query_mut::<(Entity, &mut Transform)>() {
            if e == sel { t.rotation = new_rot; dirty.set(true); break; }
        }
    }
    if let (Some(sel), Some(new_scale)) = (primary_entity, deferred_new_scale) {
        for (e, t) in world.query_mut::<(Entity, &mut Transform)>() {
            if e == sel { t.scale = new_scale; dirty.set(true); break; }
        }
    }

    // Record undo when drag ends
    if gizmo_dragging && ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
        if let (Some(target), Some(old)) = (primary_entity, gizmo_undo_transform.take()) {
            undo_history.borrow_mut().push(EditorAction::TransformEntity { entity: target, old_transform: old });
        }
        gizmo_dragging = false;
        ctx.data_mut(|d| d.remove_temp::<[Vec3; 3]>(gizmo_local_axes_id));
        ctx.data_mut(|d| d.remove_temp::<usize>(gizmo_drag_mode_id));
    }

    // Persist gizmo state
    ctx.data_mut(|d| d.insert_temp(gizmo_active_id, gizmo_active));
    ctx.data_mut(|d| d.insert_temp(gizmo_drag_start_id, gizmo_drag_start));
    ctx.data_mut(|d| d.insert_temp(gizmo_entity_pos_id, gizmo_entity_pos));
    ctx.data_mut(|d| d.insert_temp(gizmo_entity_rot_id, gizmo_entity_rot));
    ctx.data_mut(|d| d.insert_temp(gizmo_entity_scale_id, gizmo_entity_scale));
    if let Some(t) = gizmo_undo_transform {
        ctx.data_mut(|d| d.insert_temp(gizmo_undo_transform_id, t));
    } else {
        ctx.data_mut(|d| d.remove_temp::<Transform>(gizmo_undo_transform_id));
    }
    ctx.data_mut(|d| d.insert_temp(gizmo_dragging_id, gizmo_dragging));
    if !gizmo_dragging {
        ctx.data_mut(|d| d.remove_temp::<usize>(gizmo_drag_mode_id));
    }

    clicked_entity
}

fn handle_viewport_keys(
    ctx: &egui::Context,
    world: &mut EcsWorld,
    selected_entities: &std::cell::RefCell<Vec<hecs::Entity>>,
    cam: &mut EditorCamera,
    dirty: &std::cell::Cell<bool>,
    undo_history: &std::cell::RefCell<UndoHistory>,
) {
    if ctx.input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.command) {
        if let Some(sel) = selected_entities.borrow().first().copied() {
            let matrix = world_transform(world, sel);
            let (_s, _r, pos) = matrix.to_scale_rotation_translation();
            cam.center = pos;
            cam.distance = 4.0;
        }
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
        cam.center = Vec3::ZERO;
        cam.yaw = 0.0;
        cam.pitch = -0.3;
        cam.distance = 8.0;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
        let to_delete: Vec<hecs::Entity> = selected_entities.borrow().iter().filter(|e| world.get::<&Name>(**e).is_ok()).copied().collect();
        if !to_delete.is_empty() {
            for sel in to_delete {
                let snapshot = crate::scene::entity_to_scene_entity(world, sel);
                let _ = world.despawn(sel);
                undo_history.borrow_mut().push(EditorAction::DeleteEntity { entity: sel, snapshot });
            }
            selected_entities.borrow_mut().clear();
            dirty.set(true);
        }
    }
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::D)) {
        let to_dup: Vec<hecs::Entity> = selected_entities.borrow().iter().copied().collect();
        if !to_dup.is_empty() {
            let mut new_selection = Vec::new();
            for (idx, sel) in to_dup.iter().enumerate() {
                let name = world.get::<&Name>(*sel).ok().map(|n| n.0.clone()).unwrap_or_default();
                let transform = world.get::<&Transform>(*sel).ok().map(|r| (*r).clone()).unwrap_or_default();
                let mesh = world.get::<&MeshComponent>(*sel).ok().map(|r| (*r).clone());
                let material = world.get::<&Material>(*sel).ok().map(|r| (*r).clone());
                let dirlight = world.get::<&rustix_render::DirectionalLight>(*sel).ok().map(|r| (*r).clone());
                let pointlight = world.get::<&rustix_render::PointLight>(*sel).ok().map(|r| (*r).clone());
                let spotlight = world.get::<&rustix_render::SpotLight>(*sel).ok().map(|r| (*r).clone());
                let audio = world.get::<&AudioSource>(*sel).ok().map(|r| (*r).clone());
                let rigidbody = world.get::<&RigidBody>(*sel).ok().map(|r| *r);
                let collider = world.get::<&Collider>(*sel).ok().map(|r| *r);
                let audiolistener = world.get::<&AudioListener>(*sel).ok().map(|r| *r);
                let camera = world.get::<&Camera>(*sel).ok().map(|r| *r);

                let mut new_transform = transform;
                new_transform.position.x += 1.0 + idx as f32 * 0.5;

                let mut builder = hecs::EntityBuilder::new();
                builder.add(Name(format!("{} Copy", name)));
                builder.add(new_transform);
                if let Some(m) = mesh { builder.add(m); }
                if let Some(m) = material { builder.add(m); }
                if let Some(l) = dirlight { builder.add(l); }
                if let Some(l) = pointlight { builder.add(l); }
                if let Some(l) = spotlight { builder.add(l); }
                if let Some(a) = audio { builder.add(a); }
                if let Some(rb) = rigidbody { builder.add(rb); }
                if let Some(c) = collider { builder.add(c); }
                if let Some(al) = audiolistener { builder.add(al); }
                if let Some(cam) = camera { builder.add(cam); }
                let new_entity = world.spawn(builder.build());

                let snapshot = crate::scene::entity_to_scene_entity(world, new_entity);
                undo_history.borrow_mut().push(EditorAction::AddEntity { entity: new_entity, snapshot });
                new_selection.push(new_entity);
            }
            *selected_entities.borrow_mut() = new_selection;
            dirty.set(true);
        }
    }
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::C)) {
        let copies: Vec<crate::scene::SceneEntity> = selected_entities.borrow().iter().map(|sel| crate::scene::entity_to_scene_entity(world, *sel)).collect();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("copied_entities"), copies));
    }
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::V)) {
        if let Some(copied) = ctx.data(|d| d.get_temp::<Vec<crate::scene::SceneEntity>>(egui::Id::new("copied_entities"))) {
            let mut new_selection = Vec::new();
            for (idx, mut pasted) in copied.iter().cloned().enumerate() {
                pasted.name = format!("{} (Pasted)", pasted.name);
                pasted.position[0] += 1.0 + idx as f32 * 0.5;
                let new_entity = crate::scene::spawn_entity(world, &pasted);
                let snapshot = crate::scene::entity_to_scene_entity(world, new_entity);
                undo_history.borrow_mut().push(EditorAction::AddEntity { entity: new_entity, snapshot });
                new_selection.push(new_entity);
            }
            *selected_entities.borrow_mut() = new_selection;
            dirty.set(true);
        }
    }
}

#[allow(deprecated)]
pub fn show_viewport(
    ctx: &egui::Context,
    cam: &mut EditorCamera,
    world: &mut EcsWorld,
    selected_entities: &std::cell::RefCell<Vec<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    undo_history: &std::cell::RefCell<UndoHistory>,
    bookmarks: &mut Vec<CameraBookmark>,
    is_playing: bool,
    screen: &mut AppScreen,
) {
    let mut frame = egui::Frame::central_panel(&ctx.global_style());
    frame.fill = egui::Color32::TRANSPARENT;
    egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
        let rect = ui.max_rect();

        // Store viewport rect for next frame's offscreen render sizing
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("viewport_rect_0"), rect));

        // Display the offscreen-rendered 3D scene
        let valid_key = egui::Id::new("viewport_offscreen_valid_0");
        let has_offscreen = ctx.data(|d| d.get_temp::<bool>(valid_key)).unwrap_or(false);
        tracing::trace!("show_viewport: has_offscreen={} rect={:?}", has_offscreen, rect);
        if has_offscreen {
            let tex_id = viewport_texture_id(0);
            let size = rect.size();
            if size.x > 0.0 && size.y > 0.0 {
                let image_rect = egui::Rect::from_min_size(rect.min, size);
                ui.painter().image(tex_id, image_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                tracing::trace!("show_viewport: drew image with tex_id={:?} size={:?}", tex_id, size);
            }
        }

        // Projection
        let aspect = rect.width() / rect.height().max(1.0);
        let vp = cam.view_proj(aspect);
        let world_to_screen = |wpos: Vec3| -> Option<egui::Pos2> {
            let clip = vp * Vec4::new(wpos.x, wpos.y, wpos.z, 1.0);
            if clip.w <= 0.0 { return None; }
            let ndc = clip.truncate() / clip.w;
            let x = rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width();
            let y = rect.min.y + (1.0 - (ndc.y * 0.5 + 0.5)) * rect.height();
            Some(egui::pos2(x, y))
        };

        // Resolve gizmo mode from keyboard (E/R/T)
        let gizmo_mode_id = egui::Id::new("gizmo_mode");
        let gizmo_mode = gizmo::resolve_gizmo_mode(
            ctx.data(|d| d.get_temp::<usize>(gizmo_mode_id)).unwrap_or(0),
            ctx
        );
        ctx.data_mut(|d| d.insert_temp(gizmo_mode_id, gizmo_mode));

        if !is_playing {
            // Ground grid
            let show_grid = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("viewport_show_grid")).unwrap_or(true));
            if show_grid {
                draw_grid(ui.painter(), &world_to_screen);
            }

            // Camera debug text
            draw_camera_overlay(ui.painter(), rect, cam);

            // Entity dots, selection, gizmos, and drag interaction
            let clicked_entity = draw_scene_overlays(
                ui, ctx, rect, world, cam, selected_entities, &world_to_screen,
                undo_history, dirty,
            );

            // Click selection / background deselect
            if let Some(e) = clicked_entity {
                if ui.ctx().input(|i| i.modifiers.ctrl) {
                    let mut sel = selected_entities.borrow_mut();
                    if let Some(pos) = sel.iter().position(|x| *x == e) {
                        sel.remove(pos);
                    } else {
                        sel.push(e);
                    }
                } else {
                    *selected_entities.borrow_mut() = vec![e];
                }
            } else if ui.interact(rect, egui::Id::new("vp_bg_click"), egui::Sense::click()).clicked() {
                selected_entities.borrow_mut().clear();
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("gizmo_active"), usize::MAX));
            }

            // Keyboard shortcuts
            handle_viewport_keys(ctx, world, selected_entities, cam, dirty, undo_history);
        }

        // Toolbar + bookmarks — drawn LAST so it sits on top of the viewport
        // background click area and receives input first.
        draw_toolbar(ui, ctx, cam, bookmarks, screen);
    });
}
