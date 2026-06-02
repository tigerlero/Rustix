use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::Vec3;
use rustix_render::{DirectionalLight, PointLight, SpotLight, Camera};
use rustix_audio::{AudioSource, AudioListener};
use rustix_scripting::ScriptComponent;
use rustix_physics::{RigidBody, Collider, BodyType, ColliderShape};
use egui::color_picker::{color_picker_hsva_2d, Alpha};

use crate::camera::{EditorCamera, CameraMode};
use crate::scene::{Transform, Name, Material, MeshComponent, Parent};
use crate::undo::{UndoHistory, EditorAction};

/// Custom color picker button + popup that bypasses egui's built-in color_edit_button_srgb.
///
/// Displays the current R/G/B channel values as text labels **above** the color preview
/// button (outside the popup). Clicking the button opens a hue / saturation 2-D picker
/// inside a popup that does **not** contain any R/G/B text.
///
/// # Parameters
/// * `ui` — The egui `Ui` context to render into. Must be called inside a layout that
///   places widgets vertically so the labels appear above the button.
/// * `rgb` — Mutable 3-element array storing the sRGB channel values in the range
///   `0..=255`. Updated in-place when the user picks a new color.
/// * `popup_id` — Unique `egui::Id` used to track open/close state for this instance's
///   popup modal.
fn color_picker_button(ui: &mut egui::Ui, rgb: &mut [u8; 3], popup_id: egui::Id) {
    let color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);

    // Show R/G/B editable inputs above the color button (outside the popup modal).
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut rgb[0]).prefix("R: ").speed(1.0).range(0..=255));
        ui.add(egui::DragValue::new(&mut rgb[1]).prefix("G: ").speed(1.0).range(0..=255));
        ui.add(egui::DragValue::new(&mut rgb[2]).prefix("B: ").speed(1.0).range(0..=255));
    });

    let size = ui.spacing().interact_size;
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let draw_rect = rect.expand(visuals.expansion);
        ui.painter().rect_filled(draw_rect, visuals.corner_radius.at_most(2), color);
        ui.painter().rect_stroke(
            draw_rect,
            visuals.corner_radius.at_most(2),
            (1.0, visuals.bg_fill),
            egui::StrokeKind::Inside,
        );
    }

    egui::Popup::from_toggle_button_response(&response)
        .id(popup_id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(250.0);
                // Use the HSVA 2-D picker so no R/G/B text appears inside the popup.
                let mut hsva = egui::epaint::Hsva::from(color);
                color_picker_hsva_2d(ui, &mut hsva, Alpha::Opaque);
                let new_color = egui::Color32::from(hsva);
                rgb[0] = new_color[0];
                rgb[1] = new_color[1];
                rgb[2] = new_color[2];
            });
        });
}

#[allow(clippy::too_many_arguments)]
pub fn show_inspector(
    ctx: &egui::Context,
    cam: &mut EditorCamera,
    world: &mut EcsWorld,
    selected_entity: &std::cell::RefCell<Option<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    undo_history: &std::cell::RefCell<UndoHistory>,
) {
    let selected_entity_val = *selected_entity.borrow();
    let mut selected_name: Option<String> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &Name)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, n)| n.0.clone())
    });
    let old_name = selected_name.clone();
    let mut selected_transform: Option<Transform> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &Transform)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, t)| t.clone())
    });
    let mut selected_mesh: Option<MeshComponent> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &MeshComponent)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, m)| m.clone())
    });
    let old_mesh = selected_mesh.clone();
    let mut selected_dirlight: Option<DirectionalLight> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &DirectionalLight)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, l)| l.clone())
    });
    let old_dirlight = selected_dirlight.clone();
    let mut selected_pointlight: Option<PointLight> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &PointLight)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, l)| l.clone())
    });
    let old_pointlight = selected_pointlight.clone();
    let mut selected_spotlight: Option<SpotLight> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &SpotLight)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, l)| l.clone())
    });
    let old_spotlight = selected_spotlight.clone();
    let mut selected_material: Option<Material> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &Material)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, m)| m.clone())
    });
    let old_material = selected_material.clone();
    let mut selected_audio_source: Option<AudioSource> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &AudioSource)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, a)| a.clone())
    });
    let old_audio_source = selected_audio_source.clone();
    let mut selected_audio_listener: Option<AudioListener> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &AudioListener)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, a)| a.clone())
    });
    let old_audio_listener = selected_audio_listener.clone();
    let mut selected_camera: Option<Camera> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &Camera)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, c)| *c)
    });
    let old_camera = selected_camera;
    let mut selected_script: Option<ScriptComponent> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &ScriptComponent)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, s)| s.clone())
    });
    let old_script = selected_script.clone();
    let mut selected_rigidbody: Option<RigidBody> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &RigidBody)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, b)| *b)
    });
    let old_rigidbody = selected_rigidbody;
    let mut selected_collider: Option<Collider> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &Collider)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, c)| *c)
    });
    let old_collider = selected_collider;

    egui::Panel::right("inspector").resizable(true).default_size(260.0).show(ctx, |ui| {
        ui.heading("Inspector");
        ui.separator();
        if let (Some(name), Some(transform)) = (selected_name.as_mut(), selected_transform.as_mut()) {
            ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
            if let Some(entity) = selected_entity_val {
                ui.label(egui::RichText::new(format!("ID: {}", entity.to_bits().get())).weak().size(11.0));
            }
            ui.separator();
            if let Some(entity) = selected_entity_val {
                let current_parent = world.get::<&Parent>(entity).ok().and_then(|p| p.0);
                let mut children: Vec<(hecs::Entity, String)> = Vec::new();
                for (child_e, child_parent) in world.query::<(Entity, &Parent)>().iter() {
                    if let Some(pe) = child_parent.0 {
                        if pe == entity {
                            let child_name = world.get::<&Name>(child_e).map(|n| n.0.clone()).unwrap_or_else(|_| "Unnamed".to_string());
                            children.push((child_e, child_name));
                        }
                    }
                }
                if current_parent.is_some() || !children.is_empty() {
                    ui.label(egui::RichText::new("Hierarchy").strong());
                    if let Some(parent_entity) = current_parent {
                        let parent_name = world.get::<&Name>(parent_entity).map(|n| n.0.clone()).unwrap_or_else(|_| "Unnamed".to_string());
                        ui.label(egui::RichText::new(format!("Parent: {}", parent_name)).weak().size(12.0));
                    }
                    if !children.is_empty() {
                        ui.label(egui::RichText::new("Children:").weak().size(12.0));
                        for (_child_e, child_name) in &children {
                            ui.label(egui::RichText::new(format!("  - {}", child_name)).weak().size(11.0));
                        }
                    }
                    ui.separator();
                }
                // Reparent dropdown
                let mut all_entities: Vec<(hecs::Entity, String)> = Vec::new();
                for (e, n) in world.query::<(Entity, &Name)>().iter() {
                    if e != entity {
                        let mut is_descendant = false;
                        let mut check = e;
                        for _ in 0..64 {
                            if let Ok(p) = world.get::<&Parent>(check) {
                                if let Some(pe) = p.0 {
                                    if pe == entity { is_descendant = true; break; }
                                    check = pe;
                                } else { break; }
                            } else { break; }
                        }
                        if !is_descendant {
                            all_entities.push((e, n.0.clone()));
                        }
                    }
                }
                let mut selected_parent_idx: usize = 0;
                let mut parent_labels = vec!["None".to_string()];
                for (i, (e, name)) in all_entities.iter().enumerate() {
                    parent_labels.push(format!("{} ({})", name, e.to_bits().get()));
                    if current_parent == Some(*e) {
                        selected_parent_idx = i + 1;
                    }
                }
                let mut new_idx = selected_parent_idx;
                egui::ComboBox::from_label("Set Parent")
                    .selected_text(&parent_labels[new_idx])
                    .show_ui(ui, |ui| {
                        for (i, label) in parent_labels.iter().enumerate() {
                            ui.selectable_value(&mut new_idx, i, label);
                        }
                    });
                if new_idx != selected_parent_idx {
                    if new_idx == 0 {
                        let _ = world.insert(entity, (Parent(None),));
                    } else if let Some((new_parent, _)) = all_entities.get(new_idx - 1) {
                        let _ = world.insert(entity, (Parent(Some(*new_parent)),));
                    }
                    dirty.set(true);
                }
                ui.separator();
            }
            ui.label("Transform");
            ui.add(egui::DragValue::new(&mut transform.position.x).prefix("x: "));
            ui.add(egui::DragValue::new(&mut transform.position.y).prefix("y: "));
            ui.add(egui::DragValue::new(&mut transform.position.z).prefix("z: "));
            ui.horizontal(|ui| {
                ui.label("Rotation");
                ui.add(egui::DragValue::new(&mut transform.rotation.x).prefix("x: "));
                ui.add(egui::DragValue::new(&mut transform.rotation.y).prefix("y: "));
                ui.add(egui::DragValue::new(&mut transform.rotation.z).prefix("z: "));
            });
            ui.add(egui::DragValue::new(&mut transform.scale.x).prefix("Scale x: "));
            ui.add(egui::DragValue::new(&mut transform.scale.y).prefix("Scale y: "));
            ui.add(egui::DragValue::new(&mut transform.scale.z).prefix("Scale z: "));
            if let Some(target_entity) = selected_entity_val {
                for (e, t) in world.query_mut::<(Entity, &mut Transform)>() {
                    if e == target_entity {
                        let old = t.clone();
                        *t = transform.clone();
                        if old != *t {
                            undo_history.borrow_mut().push(EditorAction::TransformEntity { entity: target_entity, old_transform: old });
                            dirty.set(true);
                        }
                        break;
                    }
                }
            }
            if selected_entity_val.is_some() {
                if ui.button("Focus").clicked() {
                    if let Some(target_entity) = selected_entity_val {
                        if let Ok(t) = world.get::<&Transform>(target_entity) {
                            match cam.mode {
                                CameraMode::Orbit => {
                                    cam.center = t.position;
                                }
                                CameraMode::FirstPerson => {
                                    cam.position = t.position + Vec3::new(0.0, 1.6, 0.0);
                                }
                            }
                        }
                    }
                }
            }
            if let Some(ref mut mesh) = selected_mesh {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Mesh");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<MeshComponent>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "MeshComponent".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                let meshes = ["Cube", "Sphere", "Torus", "Capsule", "Icosphere", "Plane", "Terrain"];
                egui::ComboBox::from_label("")
                    .selected_text(&mesh.0)
                    .show_ui(ui, |ui| {
                        for m in meshes {
                            ui.selectable_value(&mut mesh.0, m.to_string(), m);
                        }
                    });
            }
            if let Some(ref mut dl) = selected_dirlight {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Directional Light");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<DirectionalLight>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "DirectionalLight".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                ui.label("Color");
                let dl_id = selected_entity_val.map(|e| e.to_bits().get()).unwrap_or(0);
                let mut dl_rgb = [
                    (dl.color.x.clamp(0.0, 1.0) * 255.0) as u8,
                    (dl.color.y.clamp(0.0, 1.0) * 255.0) as u8,
                    (dl.color.z.clamp(0.0, 1.0) * 255.0) as u8,
                ];
                color_picker_button(ui, &mut dl_rgb, egui::Id::new(("dl_color", dl_id)));
                dl.color = Vec3::new(dl_rgb[0] as f32 / 255.0, dl_rgb[1] as f32 / 255.0, dl_rgb[2] as f32 / 255.0);
                ui.add(egui::DragValue::new(&mut dl.intensity).prefix("Intensity: ").speed(0.1));
            }
            if let Some(ref mut pl) = selected_pointlight {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Point Light");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<PointLight>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "PointLight".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                ui.label("Color");
                let pl_id = selected_entity_val.map(|e| e.to_bits().get()).unwrap_or(0);
                let mut pl_rgb = [
                    (pl.color.x.clamp(0.0, 1.0) * 255.0) as u8,
                    (pl.color.y.clamp(0.0, 1.0) * 255.0) as u8,
                    (pl.color.z.clamp(0.0, 1.0) * 255.0) as u8,
                ];
                color_picker_button(ui, &mut pl_rgb, egui::Id::new(("pl_color", pl_id)));
                pl.color = Vec3::new(pl_rgb[0] as f32 / 255.0, pl_rgb[1] as f32 / 255.0, pl_rgb[2] as f32 / 255.0);
                ui.add(egui::DragValue::new(&mut pl.intensity).prefix("Intensity: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut pl.radius).prefix("Radius: ").speed(0.1));
            }
            if let Some(ref mut sl) = selected_spotlight {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Spot Light");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<SpotLight>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "SpotLight".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                ui.label("Color");
                let sl_id = selected_entity_val.map(|e| e.to_bits().get()).unwrap_or(0);
                let mut sl_rgb = [
                    (sl.color.x.clamp(0.0, 1.0) * 255.0) as u8,
                    (sl.color.y.clamp(0.0, 1.0) * 255.0) as u8,
                    (sl.color.z.clamp(0.0, 1.0) * 255.0) as u8,
                ];
                color_picker_button(ui, &mut sl_rgb, egui::Id::new(("sl_color", sl_id)));
                sl.color = Vec3::new(sl_rgb[0] as f32 / 255.0, sl_rgb[1] as f32 / 255.0, sl_rgb[2] as f32 / 255.0);
                ui.add(egui::DragValue::new(&mut sl.intensity).prefix("Intensity: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut sl.inner_angle).prefix("Inner angle: ").speed(0.01));
                ui.add(egui::DragValue::new(&mut sl.outer_angle).prefix("Outer angle: ").speed(0.01));
                ui.add(egui::DragValue::new(&mut sl.radius).prefix("Radius: ").speed(0.1));
            }
            if let Some(ref mut mat) = selected_material {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Material");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<Material>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "Material".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                ui.label("Base Color");
                let mat_id = selected_entity_val.map(|e| e.to_bits().get()).unwrap_or(0);
                let mut mat_rgb = [
                    (mat.base_color.x.clamp(0.0, 1.0) * 255.0) as u8,
                    (mat.base_color.y.clamp(0.0, 1.0) * 255.0) as u8,
                    (mat.base_color.z.clamp(0.0, 1.0) * 255.0) as u8,
                ];
                color_picker_button(ui, &mut mat_rgb, egui::Id::new(("mat_color", mat_id)));
                mat.base_color = Vec3::new(mat_rgb[0] as f32 / 255.0, mat_rgb[1] as f32 / 255.0, mat_rgb[2] as f32 / 255.0);
                ui.add(egui::DragValue::new(&mut mat.roughness).prefix("Roughness: ").speed(0.01).range(0.0..=1.0));
                ui.add(egui::DragValue::new(&mut mat.metallic).prefix("Metallic: ").speed(0.01).range(0.0..=1.0));
            }
            if let Some(ref mut aud) = selected_audio_source {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Audio Source");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<AudioSource>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "AudioSource".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                ui.add(egui::DragValue::new(&mut aud.min_distance).prefix("Min dist: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut aud.max_distance).prefix("Max dist: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut aud.rolloff).prefix("Rolloff: ").speed(0.1));
            }
            if let Some(ref mut al) = selected_audio_listener {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Audio Listener");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<AudioListener>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "AudioListener".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                ui.add(egui::DragValue::new(&mut al.position.x).prefix("Pos x: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut al.position.y).prefix("Pos y: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut al.position.z).prefix("Pos z: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut al.forward.x).prefix("Fwd x: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut al.forward.y).prefix("Fwd y: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut al.forward.z).prefix("Fwd z: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut al.up.x).prefix("Up x: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut al.up.y).prefix("Up y: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut al.up.z).prefix("Up z: ").speed(0.1));
            }
            if let Some(ref mut cam_comp) = selected_camera {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Camera");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<Camera>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "Camera".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                ui.add(egui::DragValue::new(&mut cam_comp.fov_degrees).prefix("FOV: ").speed(0.5).range(1.0..=179.0));
                ui.add(egui::DragValue::new(&mut cam_comp.near).prefix("Near: ").speed(0.01).range(0.001..=1000.0));
                ui.add(egui::DragValue::new(&mut cam_comp.far).prefix("Far: ").speed(0.1).range(0.1..=10000.0));
            }
            if let Some(ref mut sc) = selected_script {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Script");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<ScriptComponent>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "ScriptComponent".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                ui.checkbox(&mut sc.config.enabled, "Enabled");
                ui.label("Source:");
                ui.add_sized([ui.available_width(), 120.0], egui::TextEdit::multiline(&mut sc.source).code_editor());
            }
            if let Some(ref mut rb) = selected_rigidbody {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Rigid Body");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<RigidBody>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "RigidBody".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Type:");
                    let mut is_static = rb.body_type == BodyType::Static;
                    let mut is_kinematic = rb.body_type == BodyType::Kinematic;
                    let mut is_dynamic = rb.body_type == BodyType::Dynamic;
                    if ui.selectable_label(is_static, "Static").clicked() { rb.body_type = BodyType::Static; }
                    if ui.selectable_label(is_kinematic, "Kinematic").clicked() { rb.body_type = BodyType::Kinematic; }
                    if ui.selectable_label(is_dynamic, "Dynamic").clicked() { rb.body_type = BodyType::Dynamic; }
                });
                ui.add(egui::DragValue::new(&mut rb.mass).prefix("Mass: ").speed(0.1).range(0.001..=10000.0));
                ui.add(egui::DragValue::new(&mut rb.gravity_scale).prefix("Gravity scale: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut rb.drag).prefix("Drag: ").speed(0.01).range(0.0..=1.0));
                ui.add(egui::DragValue::new(&mut rb.angular_drag).prefix("Angular drag: ").speed(0.01).range(0.0..=1.0));
                ui.checkbox(&mut rb.use_gravity, "Use Gravity");
            }
            if let Some(ref mut col) = selected_collider {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Collider");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            if let Some(target) = selected_entity_val {
                                let snapshot = crate::scene::entity_to_scene_entity(world, target);
                                let _ = world.remove_one::<Collider>(target);
                                undo_history.borrow_mut().push(EditorAction::ComponentRemoved { entity: target, component: "Collider".into(), old_snapshot: snapshot });
                                dirty.set(true);
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Shape:");
                    let mut is_sphere = matches!(col.shape, ColliderShape::Sphere { .. });
                    let mut is_box = matches!(col.shape, ColliderShape::Box { .. });
                    let mut is_capsule = matches!(col.shape, ColliderShape::Capsule { .. });
                    if ui.selectable_label(is_sphere, "Sphere").clicked() { col.shape = ColliderShape::Sphere { radius: 0.5 }; }
                    if ui.selectable_label(is_box, "Box").clicked() { col.shape = ColliderShape::Box { half_extents: Vec3::splat(0.5) }; }
                    if ui.selectable_label(is_capsule, "Capsule").clicked() { col.shape = ColliderShape::Capsule { radius: 0.3, height: 1.0 }; }
                });
                match &mut col.shape {
                    ColliderShape::Sphere { radius } => {
                        ui.add(egui::DragValue::new(radius).prefix("Radius: ").speed(0.01).range(0.001..=1000.0));
                    }
                    ColliderShape::Box { half_extents } => {
                        ui.add(egui::DragValue::new(&mut half_extents.x).prefix("Half X: ").speed(0.01).range(0.001..=1000.0));
                        ui.add(egui::DragValue::new(&mut half_extents.y).prefix("Half Y: ").speed(0.01).range(0.001..=1000.0));
                        ui.add(egui::DragValue::new(&mut half_extents.z).prefix("Half Z: ").speed(0.01).range(0.001..=1000.0));
                    }
                    ColliderShape::Capsule { radius, height } => {
                        ui.add(egui::DragValue::new(radius).prefix("Radius: ").speed(0.01).range(0.001..=1000.0));
                        ui.add(egui::DragValue::new(height).prefix("Height: ").speed(0.01).range(0.001..=1000.0));
                    }
                }
                ui.add(egui::DragValue::new(&mut col.restitution).prefix("Restitution: ").speed(0.01).range(0.0..=1.0));
                ui.add(egui::DragValue::new(&mut col.friction).prefix("Friction: ").speed(0.01).range(0.0..=1.0));
                ui.checkbox(&mut col.is_trigger, "Is Trigger");
            }

            if let Some(target_entity) = selected_entity_val {
                if let (Some(ref new), Some(ref old)) = (selected_dirlight, old_dirlight) {
                    if new != old {
                        for (e, l) in world.query_mut::<(Entity, &mut DirectionalLight)>() {
                            if e == target_entity { *l = *new; dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::DirectionalLightChanged { entity: target_entity, old: *old });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (selected_pointlight, old_pointlight) {
                    if new != old {
                        for (e, l) in world.query_mut::<(Entity, &mut PointLight)>() {
                            if e == target_entity { *l = *new; dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::PointLightChanged { entity: target_entity, old: *old });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (selected_spotlight, old_spotlight) {
                    if new != old {
                        for (e, l) in world.query_mut::<(Entity, &mut SpotLight)>() {
                            if e == target_entity { *l = *new; dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::SpotLightChanged { entity: target_entity, old: *old });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (&selected_material, &old_material) {
                    if new != old {
                        for (e, m) in world.query_mut::<(Entity, &mut Material)>() {
                            if e == target_entity { *m = new.clone(); dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::MaterialChanged { entity: target_entity, old: old.clone() });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (selected_audio_source, old_audio_source) {
                    if new != old {
                        for (e, a) in world.query_mut::<(Entity, &mut AudioSource)>() {
                            if e == target_entity { *a = *new; dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::AudioSourceChanged { entity: target_entity, old: *old });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (selected_audio_listener, old_audio_listener) {
                    if new != old {
                        for (e, a) in world.query_mut::<(Entity, &mut AudioListener)>() {
                            if e == target_entity { *a = *new; dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::AudioListenerChanged { entity: target_entity, old: *old });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (&selected_script, &old_script) {
                    if new != old {
                        for (e, s) in world.query_mut::<(Entity, &mut ScriptComponent)>() {
                            if e == target_entity { *s = new.clone(); dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::ScriptComponentChanged { entity: target_entity, old: old.clone() });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (selected_rigidbody, old_rigidbody) {
                    if new != old {
                        for (e, b) in world.query_mut::<(Entity, &mut RigidBody)>() {
                            if e == target_entity { *b = *new; dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::RigidBodyChanged { entity: target_entity, old: *old });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (selected_collider, old_collider) {
                    if new != old {
                        for (e, c) in world.query_mut::<(Entity, &mut Collider)>() {
                            if e == target_entity { *c = *new; dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::ColliderChanged { entity: target_entity, old: *old });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (&selected_mesh, &old_mesh) {
                    if new != old {
                        for (e, m) in world.query_mut::<(Entity, &mut MeshComponent)>() {
                            if e == target_entity { m.0 = new.0.clone(); dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::MeshComponentChanged { entity: target_entity, old: old.clone() });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (&selected_name, &old_name) {
                    if new != old {
                        for (e, n) in world.query_mut::<(Entity, &mut Name)>() {
                            if e == target_entity { n.0 = new.clone(); dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::RenameEntity { entity: target_entity, old_name: old.clone() });
                    }
                }
                if let (Some(ref new), Some(ref old)) = (&selected_camera, &old_camera) {
                    if new != old {
                        for (e, c) in world.query_mut::<(Entity, &mut Camera)>() {
                            if e == target_entity { *c = *new; dirty.set(true); break; }
                        }
                        undo_history.borrow_mut().push(EditorAction::CameraChanged { entity: target_entity, old: *old });
                    }
                }
            }
        } else {
            ui.label("Select an object in the Hierarchy to inspect.");
            ui.add_space(10.0);
            ui.label(egui::RichText::new("No object selected").italics());
        }
        if let Some(target) = selected_entity_val {
            ui.separator();
            ui.menu_button("Add Component", |ui| {
                let has_dir = world.get::<&DirectionalLight>(target).is_ok();
                let has_pt = world.get::<&PointLight>(target).is_ok();
                let has_spot = world.get::<&SpotLight>(target).is_ok();
                let has_mat = world.get::<&Material>(target).is_ok();
                let has_audio = world.get::<&AudioSource>(target).is_ok();
                let has_rigidbody = world.get::<&RigidBody>(target).is_ok();
                let has_collider = world.get::<&Collider>(target).is_ok();
                let has_mesh = world.get::<&MeshComponent>(target).is_ok();
                let has_audiolistener = world.get::<&AudioListener>(target).is_ok();
                let has_camera = world.get::<&Camera>(target).is_ok();
                let has_script = world.get::<&ScriptComponent>(target).is_ok();
                if !has_dir && ui.button("Directional Light").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (DirectionalLight::default(),));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "DirectionalLight".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
                if !has_pt && ui.button("Point Light").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (PointLight::default(),));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "PointLight".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
                if !has_spot && ui.button("Spot Light").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (SpotLight::default(),));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "SpotLight".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
                if !has_mat && ui.button("Material").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (Material { base_color: Vec3::new(0.7, 0.7, 0.7), roughness: 0.5, metallic: 0.0 },));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "Material".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
                if !has_audio && ui.button("Audio Source").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (AudioSource { position: Vec3::ZERO, min_distance: 1.0, max_distance: 100.0, rolloff: 1.0 },));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "AudioSource".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
                if !has_script && ui.button("Script").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (ScriptComponent::default(),));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "ScriptComponent".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
                if !has_rigidbody && ui.button("Rigid Body").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (RigidBody::default(),));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "RigidBody".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
                if !has_collider && ui.button("Collider").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (Collider::default(),));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "Collider".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
                if !has_mesh && ui.button("Mesh").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (MeshComponent("Cube".into()),));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "MeshComponent".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
                if !has_audiolistener && ui.button("Audio Listener").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (AudioListener::default(),));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "AudioListener".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
                if !has_camera && ui.button("Camera").clicked() {
                    let snapshot = crate::scene::entity_to_scene_entity(world, target);
                    let _ = world.insert(target, (Camera::default(),));
                    undo_history.borrow_mut().push(EditorAction::ComponentAdded { entity: target, component: "Camera".into(), old_snapshot: snapshot });
                    dirty.set(true);
                    ui.close();
                }
            });
        }
        ui.separator();
        ui.add_space(5.0);
        ui.label(egui::RichText::new("Camera").strong());
        ui.horizontal(|ui| {
            ui.label("Mode:");
            if ui.selectable_label(cam.mode == CameraMode::Orbit, "Orbit").clicked() {
                cam.mode = CameraMode::Orbit;
            }
            if ui.selectable_label(cam.mode == CameraMode::FirstPerson, "First Person").clicked() {
                cam.mode = CameraMode::FirstPerson;
            }
        });
        ui.checkbox(&mut cam.follow_target, "Follow Target");
        let grid_toggle_id = egui::Id::new("viewport_show_grid");
        let mut show_grid = ctx.data(|d| d.get_temp::<bool>(grid_toggle_id).unwrap_or(true));
        ui.checkbox(&mut show_grid, "Show Grid");
        ctx.data_mut(|d| d.insert_temp(grid_toggle_id, show_grid));
        match cam.mode {
            CameraMode::Orbit => {
                ui.add(egui::DragValue::new(&mut cam.center.x).prefix("Center x: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut cam.center.y).prefix("Center y: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut cam.center.z).prefix("Center z: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut cam.distance).prefix("Distance: ").speed(0.1).range(0.1..=1000.0));
            }
            CameraMode::FirstPerson => {
                ui.add(egui::DragValue::new(&mut cam.position.x).prefix("Pos x: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut cam.position.y).prefix("Pos y: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut cam.position.z).prefix("Pos z: ").speed(0.1));
            }
        }
        ui.add(egui::DragValue::new(&mut cam.yaw).prefix("Yaw: ").speed(0.01));
        ui.add(egui::DragValue::new(&mut cam.pitch).prefix("Pitch: ").speed(0.01).range(-1.57..=1.57));
    });
}
