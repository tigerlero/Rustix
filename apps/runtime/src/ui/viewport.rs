use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4, Mat4, Quat, EulerRot};
use rustix_audio::AudioSource;

use crate::camera::EditorCamera;
use crate::scene::{Transform, Name, world_transform};

pub fn show_viewport(
    ctx: &egui::Context,
    cam: &mut EditorCamera,
    world: &mut EcsWorld,
    selected_entity: &std::cell::RefCell<Option<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let rect = ui.max_rect();
        
        let entity_count = world.query::<&Name>().iter().count();
        let mut clicked_entity = None;
        
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
        
        let gizmo_mode_id = egui::Id::new("gizmo_mode");
        let mut gizmo_mode = ctx.data(|d| d.get_temp::<usize>(gizmo_mode_id).unwrap_or(0));
        
        if ctx.input(|i| i.key_pressed(egui::Key::W) && !i.modifiers.command) { gizmo_mode = 0; }
        if ctx.input(|i| i.key_pressed(egui::Key::E) && !i.modifiers.command) { gizmo_mode = 1; }
        if ctx.input(|i| i.key_pressed(egui::Key::R) && !i.modifiers.command) { gizmo_mode = 2; }
        ctx.data_mut(|d| d.insert_temp(gizmo_mode_id, gizmo_mode));
        let gizmo_active_id = egui::Id::new("gizmo_active");
        let gizmo_drag_start_id = egui::Id::new("gizmo_drag_start");
        let gizmo_entity_pos_id = egui::Id::new("gizmo_entity_pos");
        let gizmo_entity_rot_id = egui::Id::new("gizmo_entity_rot");
        let gizmo_entity_scale_id = egui::Id::new("gizmo_entity_scale");
        let mut gizmo_active = ctx.data(|d| d.get_temp::<usize>(gizmo_active_id).unwrap_or(usize::MAX));
        let mut gizmo_drag_start = ctx.data(|d| d.get_temp::<egui::Vec2>(gizmo_drag_start_id).unwrap_or(egui::Vec2::ZERO));
        let mut gizmo_entity_pos = ctx.data(|d| d.get_temp::<Vec3>(gizmo_entity_pos_id).unwrap_or(Vec3::ZERO));
        let mut gizmo_entity_rot = ctx.data(|d| d.get_temp::<Vec3>(gizmo_entity_rot_id).unwrap_or(Vec3::ZERO));
        let mut gizmo_entity_scale = ctx.data(|d| d.get_temp::<Vec3>(gizmo_entity_scale_id).unwrap_or(Vec3::ONE));
        
        let mut deferred_new_pos: Option<Vec3> = None;
        let mut deferred_new_rot: Option<Vec3> = None;
        let mut deferred_new_scale: Option<Vec3> = None;
        
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
        
        for (entity, name, transform) in world.query::<(&Entity, &Name, &Transform)>().iter() {
            let is_selected = *selected_entity.borrow() == Some(*entity);
            
            if let Some(screen_pos) = world_to_screen(transform.position) {
                let entity_color = if is_selected {
                    egui::Color32::from_rgb(70, 150, 250)
                } else {
                    egui::Color32::from_rgb(200, 200, 220)
                };
                
                let ent_resp = ui.interact(
                    egui::Rect::from_center_size(screen_pos, egui::vec2(12.0, 12.0)),
                    ui.next_auto_id(),
                    egui::Sense::click()
                );
                if ent_resp.clicked() { clicked_entity = Some(*entity); }
                ui.painter().circle_filled(screen_pos, 4.0, entity_color);
                ui.painter().text(
                    egui::pos2(screen_pos.x + 8.0, screen_pos.y - 4.0),
                    egui::Align2::LEFT_CENTER,
                    &name.0,
                    egui::FontId::proportional(11.0),
                    egui::Color32::LIGHT_GRAY
                );

                if let Ok(aud) = world.get::<&AudioSource>(*entity) {
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
                                let wp = transform.position + Vec3::new(r_horiz * angle.cos(), h, r_horiz * angle.sin());
                                if let Some(sp) = world_to_screen(wp) {
                                    if let Some(lp) = last_point {
                                        let _alpha = if *radius == aud.min_distance { 80 } else { 25 };
                                        ui.painter().line_segment([lp, sp], egui::Stroke::new(1.0, aud_min_color));
                                    }
                                    last_point = Some(sp);
                                }
                            }
                        }
                    }

                    if let Some(label_pos) = world_to_screen(transform.position + Vec3::new(aud.max_distance, aud.max_distance * 0.3, 0.0)) {
                        ui.painter().text(
                            label_pos,
                            egui::Align2::LEFT_BOTTOM,
                            format!("max {:.0}m", aud.max_distance),
                            egui::FontId::proportional(10.0),
                            aud_label_color,
                        );
                    }
                    if let Some(label_pos) = world_to_screen(transform.position + Vec3::new(aud.min_distance, -aud.min_distance * 0.3, 0.0)) {
                        ui.painter().text(
                            label_pos,
                            egui::Align2::LEFT_TOP,
                            format!("min {:.0}m", aud.min_distance),
                            egui::FontId::proportional(10.0),
                            aud_label_color,
                        );
                    }

                    ui.painter().text(
                        egui::pos2(screen_pos.x + 8.0, screen_pos.y - 16.0),
                        egui::Align2::LEFT_CENTER,
                        "\u{1f50a}",
                        egui::FontId::proportional(12.0),
                        aud_label_color,
                    );
                }
                
                if is_selected {
                    let axis_colors = [
                        (Vec3::X, egui::Color32::from_rgb(220, 60, 60)),
                        (Vec3::Y, egui::Color32::from_rgb(60, 200, 60)),
                        (Vec3::Z, egui::Color32::from_rgb(60, 100, 220)),
                    ];
                    let gizmo_len = 60.0;
                    
                    for (axis_idx, (axis_dir, color)) in axis_colors.iter().enumerate() {
                        let tip_world = transform.position + *axis_dir * 2.0;
                        if let Some(tip_screen) = world_to_screen(tip_world) {
                            let dir_2d = (tip_screen - screen_pos).normalized();
                            let handle_screen = screen_pos + dir_2d * gizmo_len;
                            
                            ui.painter().line_segment(
                                [screen_pos, handle_screen],
                                egui::Stroke::new(2.0, *color),
                            );
                            
                            let handle_rect = egui::Rect::from_center_size(handle_screen, egui::vec2(14.0, 14.0));
                            let handle_resp = ui.interact(handle_rect, ui.next_auto_id(), egui::Sense::click_and_drag());
                            
                            let is_active = gizmo_active == axis_idx;
                            let handle_color = if is_active { egui::Color32::WHITE } else { *color };
                            ui.painter().circle_filled(handle_screen, 6.0, handle_color);
                            ui.painter().circle_stroke(handle_screen, 6.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                            
                            if handle_resp.drag_started() {
                                gizmo_active = axis_idx;
                                gizmo_drag_start = handle_resp.interact_pointer_pos().unwrap_or(handle_screen).to_vec2();
                                gizmo_entity_pos = transform.position;
                                gizmo_entity_rot = transform.rotation;
                                gizmo_entity_scale = transform.scale;
                            }
                        }
                    }
                    
                    if gizmo_active != usize::MAX {
                        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                            if ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                                let axis_dir = match gizmo_active {
                                    0 => Vec3::X, 1 => Vec3::Y, _ => Vec3::Z,
                                };
                                let drag_delta = pointer_pos.to_vec2() - gizmo_drag_start;
                                
                                match gizmo_mode {
                                    1 => {
                                        let angle = drag_delta.length() * 0.01;
                                        let sign = if drag_delta.x.abs() > drag_delta.y.abs() { drag_delta.x.signum() } else { -drag_delta.y.signum() };
                                        let s = sign * if gizmo_active == 0 { 1.0 } else if gizmo_active == 1 { 1.0 } else { 1.0 };
                                        let mut new_rot = gizmo_entity_rot;
                                        match gizmo_active { 0 => { new_rot.x += angle * s; } 1 => { new_rot.y += angle * s; } _ => { new_rot.z += angle * s; } }
                                        deferred_new_rot = Some(new_rot);
                                    }
                                    2 => {
                                        let scale_delta = drag_delta.length() * 0.01;
                                        let sign = if drag_delta.x.abs() > drag_delta.y.abs() { drag_delta.x.signum() } else { drag_delta.y.signum() };
                                        let mut new_scale = gizmo_entity_scale;
                                        let val = (new_scale.to_array()[gizmo_active] + scale_delta * sign).max(0.01);
                                        match gizmo_active { 0 => { new_scale.x = val; } 1 => { new_scale.y = val; } _ => { new_scale.z = val; } }
                                        deferred_new_scale = Some(new_scale);
                                    }
                                    _ => {
                                        if let (Some(tip), Some(base)) = (
                                            world_to_screen(gizmo_entity_pos + axis_dir),
                                            world_to_screen(gizmo_entity_pos),
                                        ) {
                                            let axis_2d = (tip - base).normalized();
                                            let along = drag_delta.dot(axis_2d) * cam.distance * 0.01;
                                            deferred_new_pos = Some(gizmo_entity_pos + axis_dir * along);
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
            for (e, t) in world.query_mut::<(&Entity, &mut Transform)>() {
                if *e == sel { t.position = new_pos; dirty.set(true); break; }
            }
        }
        if let (Some(sel), Some(new_rot)) = (*selected_entity.borrow(), deferred_new_rot) {
            for (e, t) in world.query_mut::<(&Entity, &mut Transform)>() {
                if *e == sel { t.rotation = new_rot; dirty.set(true); break; }
            }
        }
        if let (Some(sel), Some(new_scale)) = (*selected_entity.borrow(), deferred_new_scale) {
            for (e, t) in world.query_mut::<(&Entity, &mut Transform)>() {
                if *e == sel { t.scale = new_scale; dirty.set(true); break; }
            }
        }
        
        if let Some(e) = clicked_entity {
            *selected_entity.borrow_mut() = Some(e);
        } else if ui.interact(rect, ui.next_auto_id(), egui::Sense::click()).clicked() {
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
        
        ctx.data_mut(|d| d.insert_temp(gizmo_active_id, gizmo_active));
        ctx.data_mut(|d| d.insert_temp(gizmo_drag_start_id, gizmo_drag_start));
        ctx.data_mut(|d| d.insert_temp(gizmo_entity_pos_id, gizmo_entity_pos));
        ctx.data_mut(|d| d.insert_temp(gizmo_entity_rot_id, gizmo_entity_rot));
        ctx.data_mut(|d| d.insert_temp(gizmo_entity_scale_id, gizmo_entity_scale));
        
        let mode_label = match gizmo_mode { 1 => "Rotate", 2 => "Scale", _ => "Translate" };
        let hud = format!("[{mode_label}]  {} entities", entity_count);
        ui.painter().text(
            egui::pos2(rect.right() - 8.0, rect.bottom() - 8.0),
            egui::Align2::RIGHT_BOTTOM,
            hud,
            egui::FontId::proportional(11.0),
            egui::Color32::from_rgba_premultiplied(180, 200, 220, 160),
        );
    });
}
