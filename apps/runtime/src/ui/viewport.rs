use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4, Mat4, Quat, EulerRot};
use rustix_audio::{AudioSource, AudioListener};
use rustix_physics::{RigidBody, Collider};
use rustix_render::Camera;

use crate::camera::EditorCamera;
use crate::scene::{Transform, Name, MeshComponent, Material, world_transform};
use crate::undo::{UndoHistory, EditorAction};

pub fn show_viewport(
    ctx: &egui::Context,
    cam: &mut EditorCamera,
    world: &mut EcsWorld,
    selected_entity: &std::cell::RefCell<Option<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    undo_history: &std::cell::RefCell<UndoHistory>,
) {
    let mut frame = egui::Frame::central_panel(&ctx.style());
    frame.fill = egui::Color32::TRANSPARENT;
    egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
        let rect = ui.max_rect();
        let screen_rect = ctx.screen_rect();

        let mut clicked_entity = None;

        // The 3D scene is rendered to the full window, so projection must match the window aspect.
        let aspect = screen_rect.width() / screen_rect.height().max(1.0);
        let vp = cam.view_proj(aspect);

        let world_to_screen = |wpos: Vec3| -> Option<egui::Pos2> {
            let clip = vp * Vec4::new(wpos.x, wpos.y, wpos.z, 1.0);
            if clip.w <= 0.0 { return None; }
            let ndc = clip.truncate() / clip.w;
            let x = screen_rect.min.x + (ndc.x * 0.5 + 0.5) * screen_rect.width();
            let y = screen_rect.min.y + (1.0 - (ndc.y * 0.5 + 0.5)) * screen_rect.height();
            Some(egui::pos2(x, y))
        };
        
        let gizmo_mode_id = egui::Id::new("gizmo_mode");
        let mut gizmo_mode = ctx.data(|d| d.get_temp::<usize>(gizmo_mode_id).unwrap_or(0));
        gizmo_mode = resolve_gizmo_mode(gizmo_mode, ctx);
        ctx.data_mut(|d| d.insert_temp(gizmo_mode_id, gizmo_mode));
        let gizmo_active_id = egui::Id::new("gizmo_active");
        let gizmo_drag_start_id = egui::Id::new("gizmo_drag_start");
        let gizmo_entity_pos_id = egui::Id::new("gizmo_entity_pos");
        let gizmo_entity_rot_id = egui::Id::new("gizmo_entity_rot");
        let gizmo_entity_scale_id = egui::Id::new("gizmo_entity_scale");
        let gizmo_undo_transform_id = egui::Id::new("gizmo_undo_transform");
        let gizmo_dragging_id = egui::Id::new("gizmo_dragging");
        let gizmo_space_id = egui::Id::new("gizmo_space");
        let gizmo_local_axes_id = egui::Id::new("gizmo_local_axes");
        let gizmo_drag_mode_id = egui::Id::new("gizmo_drag_mode");
        let mut gizmo_active = ctx.data(|d| d.get_temp::<usize>(gizmo_active_id).unwrap_or(usize::MAX));
        let mut gizmo_drag_start = ctx.data(|d| d.get_temp::<egui::Vec2>(gizmo_drag_start_id).unwrap_or(egui::Vec2::ZERO));
        let mut gizmo_entity_pos = ctx.data(|d| d.get_temp::<Vec3>(gizmo_entity_pos_id).unwrap_or(Vec3::ZERO));
        let mut gizmo_entity_rot = ctx.data(|d| d.get_temp::<Vec3>(gizmo_entity_rot_id).unwrap_or(Vec3::ZERO));
        let mut gizmo_entity_scale = ctx.data(|d| d.get_temp::<Vec3>(gizmo_entity_scale_id).unwrap_or(Vec3::ONE));
        let mut gizmo_undo_transform: Option<Transform> = ctx.data(|d| d.get_temp::<Transform>(gizmo_undo_transform_id));
        let mut gizmo_dragging = ctx.data(|d| d.get_temp::<bool>(gizmo_dragging_id).unwrap_or(false));
        let mut gizmo_space = ctx.data(|d| d.get_temp::<bool>(gizmo_space_id).unwrap_or(false));
        
        let mut deferred_new_pos: Option<Vec3> = None;
        let mut deferred_new_rot: Option<Vec3> = None;
        let mut deferred_new_scale: Option<Vec3> = None;
        
        let show_grid = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("viewport_show_grid")).unwrap_or(true));
        if show_grid {
            let grid_half = 20.0;
            let grid_step = 1.0;
            let major_step = 5.0;
            let grid_color_minor = egui::Color32::from_rgba_premultiplied(100, 110, 130, 30);
            let grid_color_major = egui::Color32::from_rgba_premultiplied(100, 110, 130, 70);
            
            let mut z = -grid_half;
            while z <= grid_half {
                let near = Vec3::new(-grid_half, 0.0, z);
                let far = Vec3::new(grid_half, 0.0, z);
                if let (Some(a), Some(b)) = (world_to_screen(near), world_to_screen(far)) {
                    let is_major = (z % major_step).abs() < 0.01;
                    let col = if is_major { grid_color_major } else { grid_color_minor };
                    ui.painter().line_segment([a, b], egui::Stroke::new(if is_major { 1.5 } else { 0.5 }, col));
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
                    ui.painter().line_segment([a, b], egui::Stroke::new(if is_major { 1.5 } else { 0.5 }, col));
                }
                x += grid_step;
            }
        }

        // Gizmo mode toolbar
        let snap_id = egui::Id::new("gizmo_snap");
        let snap_size_id = egui::Id::new("gizmo_snap_size");
        let mut snap_enabled = ctx.data(|d| d.get_temp::<bool>(snap_id).unwrap_or(false));
        let mut snap_size = ctx.data(|d| d.get_temp::<f32>(snap_size_id).unwrap_or(0.5));
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.add_space(4.0);
                let toolbar_bg = egui::Color32::from_rgba_premultiplied(30, 30, 38, 220);
                let toolbar_rect = ui.available_rect_before_wrap();
                let rect = egui::Rect::from_min_size(toolbar_rect.min, egui::vec2(260.0, 28.0));
                ui.painter().rect_filled(rect, 6.0, toolbar_bg);
                ui.painter().rect_stroke(rect, 6.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 75)), egui::StrokeKind::Inside);
                ui.allocate_ui_at_rect(rect.shrink(4.0), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Gizmo:").weak().size(11.0));
                        if ui.selectable_label(gizmo_mode == 0, egui::RichText::new("T").size(12.0).strong()).clicked() { gizmo_mode = 0; }
                        if ui.selectable_label(gizmo_mode == 1, egui::RichText::new("R").size(12.0).strong()).clicked() { gizmo_mode = 1; }
                        if ui.selectable_label(gizmo_mode == 2, egui::RichText::new("S").size(12.0).strong()).clicked() { gizmo_mode = 2; }
                        ui.add_space(4.0);
                        if ui.selectable_label(gizmo_space, egui::RichText::new("Local").size(11.0)).clicked() { gizmo_space = !gizmo_space; }
                        ui.add_space(4.0);
                        if ui.selectable_label(snap_enabled, egui::RichText::new("Snap").size(11.0)).clicked() { snap_enabled = !snap_enabled; }
                        if snap_enabled {
                            ui.add(egui::DragValue::new(&mut snap_size).speed(0.1).range(0.01..=10.0).prefix("").suffix("").clamp_to_range(false));
                        }
                    });
                });
            });
        });
        ctx.data_mut(|d| d.insert_temp(snap_id, snap_enabled));
        ctx.data_mut(|d| d.insert_temp(snap_size_id, snap_size));
        ctx.data_mut(|d| d.insert_temp(gizmo_space_id, gizmo_space));

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
        // Process closer entities first so their interaction rects take priority.
        entities.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));

        for (entity, transform, world_pos, _dist) in entities {
            let is_selected = *selected_entity.borrow() == Some(entity);

            if let Some(screen_pos) = world_to_screen(world_pos) {
                let entity_color = if is_selected {
                    egui::Color32::from_rgb(70, 150, 250)
                } else {
                    egui::Color32::from_rgb(200, 200, 220)
                };
                let hover_color = egui::Color32::from_rgb(255, 255, 100);

                // Approximate screen-space radius from object scale and camera distance.
                let avg_scale = (transform.scale.x + transform.scale.y + transform.scale.z) / 3.0;
                let pick_radius = ((avg_scale / _dist.max(0.001)) * screen_rect.height() * 0.5).max(12.0).min(120.0);

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
                    let aud_label_color = egui::Color32::from_rgba_premultiplied(255, 160, 200, 200);

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
                                        let _alpha = if *radius == aud.min_distance { 80 } else { 25 };
                                        ui.painter().line_segment([lp, sp], egui::Stroke::new(1.0, *color));
                                    }
                                    last_point = Some(sp);
                                }
                            }
                        }
                    }

                }

                if is_selected {
                    // Small white cross at origin so the pivot point is visible.
                    let o = screen_pos;
                    ui.painter().line_segment([o - egui::vec2(5.0, 0.0), o + egui::vec2(5.0, 0.0)], egui::Stroke::new(1.0, egui::Color32::WHITE));
                    ui.painter().line_segment([o - egui::vec2(0.0, 5.0), o + egui::vec2(0.0, 5.0)], egui::Stroke::new(1.0, egui::Color32::WHITE));

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
                    let gizmo_len = 60.0;
                    // Ctrl temporarily switches gizmos to scale mode when not already dragging.
                    let effective_mode = if !gizmo_dragging && ui.input(|i| i.modifiers.ctrl) { 2 } else { gizmo_mode };

                    for (axis_idx, (axis_dir, color)) in axis_colors.iter().enumerate() {
                        let is_active = gizmo_active == axis_idx;
                        let handle_color = if is_active { egui::Color32::WHITE } else { *color };
                        let handle_id = egui::Id::new(("gizmo_h", entity.to_bits().get(), axis_idx));

                        match effective_mode {
                            1 => {
                                // Rotation: draw a ring in the plane perpendicular to the axis.
                                let ring_world_r = cam.distance * 0.04;
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
                                // Handle at 45 degrees on the ring.
                                let handle_world = world_pos + ring_world_r * (0.707 * plane_a + 0.707 * plane_b);
                                if let Some(handle_screen) = world_to_screen(handle_world) {
                                    let handle_rect = egui::Rect::from_center_size(handle_screen, egui::vec2(32.0, 32.0));
                                    let handle_resp = ui.interact(handle_rect, handle_id, egui::Sense::drag());
                                    ui.painter().circle_filled(handle_screen, 4.0, handle_color);
                                    ui.painter().circle_stroke(handle_screen, 4.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
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
                                // Scale: line + square handle.
                                let tip_world = world_pos + *axis_dir * 2.0;
                                if let Some(tip_screen) = world_to_screen(tip_world) {
                                    let v = tip_screen - screen_pos;
                                    let len = (v.x * v.x + v.y * v.y).sqrt();
                                    let dir_2d = if len > 0.0 { v / len } else { egui::Vec2::ZERO };
                                    let handle_screen = screen_pos + dir_2d * gizmo_len;

                                    ui.painter().line_segment([screen_pos, handle_screen], egui::Stroke::new(1.5, *color));

                                    let handle_rect = egui::Rect::from_center_size(handle_screen, egui::vec2(32.0, 32.0));
                                    let handle_resp = ui.interact(handle_rect, handle_id, egui::Sense::drag());

                                    let half = 5.0;
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
                                // Translate: line + circle handle.
                                let tip_world = world_pos + *axis_dir * 2.0;
                                if let Some(tip_screen) = world_to_screen(tip_world) {
                                    let v = tip_screen - screen_pos;
                                    let len = (v.x * v.x + v.y * v.y).sqrt();
                                    let dir_2d = if len > 0.0 { v / len } else { egui::Vec2::ZERO };
                                    let handle_screen = screen_pos + dir_2d * gizmo_len;

                                    ui.painter().line_segment([screen_pos, handle_screen], egui::Stroke::new(1.5, *color));

                                    let handle_rect = egui::Rect::from_center_size(handle_screen, egui::vec2(32.0, 32.0));
                                    let handle_resp = ui.interact(handle_rect, handle_id, egui::Sense::drag());

                                    ui.painter().circle_filled(handle_screen, 4.0, handle_color);
                                    ui.painter().circle_stroke(handle_screen, 4.0, egui::Stroke::new(1.5, egui::Color32::WHITE));

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
                                        deferred_new_rot = Some(apply_gizmo_rotation(
                                            gizmo_entity_rot, gizmo_active, drag_delta,
                                        ));
                                    }
                                    2 => {
                                        deferred_new_scale = Some(apply_gizmo_scale(
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
                                            deferred_new_pos = Some(apply_gizmo_translation(
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
        
        if let (Some(sel), Some(new_pos)) = (*selected_entity.borrow(), deferred_new_pos) {
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
        if let (Some(sel), Some(new_rot)) = (*selected_entity.borrow(), deferred_new_rot) {
            for (e, t) in world.query_mut::<(Entity, &mut Transform)>() {
                if e == sel { t.rotation = new_rot; dirty.set(true); break; }
            }
        }
        if let (Some(sel), Some(new_scale)) = (*selected_entity.borrow(), deferred_new_scale) {
            for (e, t) in world.query_mut::<(Entity, &mut Transform)>() {
                if e == sel { t.scale = new_scale; dirty.set(true); break; }
            }
        }

        if gizmo_dragging && ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            if let (Some(target), Some(old)) = (*selected_entity.borrow(), gizmo_undo_transform.take()) {
                undo_history.borrow_mut().push(EditorAction::TransformEntity { entity: target, old_transform: old });
            }
            gizmo_dragging = false;
            ctx.data_mut(|d| d.remove_temp::<[Vec3; 3]>(gizmo_local_axes_id));
            ctx.data_mut(|d| d.remove_temp::<usize>(gizmo_drag_mode_id));
        }

        if let Some(e) = clicked_entity {
            *selected_entity.borrow_mut() = Some(e);
        } else if ui.interact(rect, egui::Id::new("vp_bg_click"), egui::Sense::click()).clicked() {
            *selected_entity.borrow_mut() = None;
            gizmo_active = usize::MAX;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.command) {
            if let Some(sel) = *selected_entity.borrow() {
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
            if let Some(sel) = *selected_entity.borrow() {
                if world.get::<&Name>(sel).is_ok() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, sel);
                    let _ = world.despawn(sel);
                    undo_history.borrow_mut().push(EditorAction::DeleteEntity { entity: sel, snapshot });
                    *selected_entity.borrow_mut() = None;
                    dirty.set(true);
                }
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::D)) {
            if let Some(sel) = *selected_entity.borrow() {
                let name = world.get::<&Name>(sel).ok().map(|n| n.0.clone()).unwrap_or_default();
                let transform = world.get::<&Transform>(sel).ok().map(|r| (*r).clone()).unwrap_or_default();
                let mesh = world.get::<&MeshComponent>(sel).ok().map(|r| (*r).clone());
                let material = world.get::<&Material>(sel).ok().map(|r| (*r).clone());
                let dirlight = world.get::<&rustix_render::DirectionalLight>(sel).ok().map(|r| (*r).clone());
                let pointlight = world.get::<&rustix_render::PointLight>(sel).ok().map(|r| (*r).clone());
                let spotlight = world.get::<&rustix_render::SpotLight>(sel).ok().map(|r| (*r).clone());
                let audio = world.get::<&AudioSource>(sel).ok().map(|r| (*r).clone());
                let rigidbody = world.get::<&RigidBody>(sel).ok().map(|r| *r);
                let collider = world.get::<&Collider>(sel).ok().map(|r| *r);
                let audiolistener = world.get::<&AudioListener>(sel).ok().map(|r| *r);
                let camera = world.get::<&Camera>(sel).ok().map(|r| *r);

                let mut new_transform = transform;
                new_transform.position.x += 1.0;

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
                *selected_entity.borrow_mut() = Some(new_entity);
                dirty.set(true);
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::C)) {
            if let Some(sel) = *selected_entity.borrow() {
                let snapshot = crate::scene::entity_to_scene_entity(world, sel);
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("copied_entity"), snapshot));
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::V)) {
            if let Some(copied) = ctx.data(|d| d.get_temp::<crate::scene::SceneEntity>(egui::Id::new("copied_entity"))) {
                let mut pasted = copied.clone();
                pasted.name = format!("{} (Pasted)", pasted.name);
                pasted.position[0] += 1.0;
                let new_entity = crate::scene::spawn_entity(world, &pasted);
                let snapshot = crate::scene::entity_to_scene_entity(world, new_entity);
                undo_history.borrow_mut().push(EditorAction::AddEntity { entity: new_entity, snapshot });
                *selected_entity.borrow_mut() = Some(new_entity);
                dirty.set(true);
            }
        }

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
        ctx.data_mut(|d| d.insert_temp(gizmo_space_id, gizmo_space));
        if !gizmo_dragging {
            ctx.data_mut(|d| d.remove_temp::<usize>(gizmo_drag_mode_id));
        }

    });
}

/// Resolve gizmo mode from egui keyboard input.
/// W → Translate (0), E → Rotate (1), R → Scale (2).
/// Command-modified keys are ignored so they don't conflict with shortcuts.
pub fn resolve_gizmo_mode(current: usize, ctx: &egui::Context) -> usize {
    let w = ctx.input(|i| i.key_pressed(egui::Key::W) && !i.modifiers.command);
    let e = ctx.input(|i| i.key_pressed(egui::Key::E) && !i.modifiers.command);
    let r = ctx.input(|i| i.key_pressed(egui::Key::R) && !i.modifiers.command);
    resolve_gizmo_mode_pure(current, w, e, r)
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
/// W → Translate (0), E → Rotate (1), R → Scale (2).
/// If multiple keys are pressed, W takes precedence, then E, then R.
pub fn resolve_gizmo_mode_pure(current: usize, w_pressed: bool, e_pressed: bool, r_pressed: bool) -> usize {
    if w_pressed { 0 }
    else if e_pressed { 1 }
    else if r_pressed { 2 }
    else { current }
}

#[cfg(test)]
#[path = "viewport_tests.rs"]
mod tests;
