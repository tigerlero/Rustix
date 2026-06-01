use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::Vec3;
use rustix_render::{DirectionalLight, PointLight, SpotLight};
use rustix_audio::{AudioSource, AudioListener};
use rustix_scripting::ScriptComponent;

use crate::camera::{EditorCamera, CameraMode};
use crate::scene::{Transform, Name, Material};
use crate::undo::{UndoHistory, EditorAction};

#[allow(clippy::too_many_arguments)]
pub fn show_inspector(
    ctx: &egui::Context,
    cam: &EditorCamera,
    world: &mut EcsWorld,
    selected_entity: &std::cell::RefCell<Option<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    undo_history: &std::cell::RefCell<UndoHistory>,
) {
    let selected_entity_val = *selected_entity.borrow();
    let selected_name: Option<String> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &Name)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, n)| n.0.clone())
    });
    let mut selected_transform: Option<Transform> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &Transform)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, t)| t.clone())
    });
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
    let mut selected_script: Option<ScriptComponent> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(Entity, &ScriptComponent)>().iter().find(|(e, _)| *e == sel_ent).map(|(_, s)| s.clone())
    });
    let old_script = selected_script.clone();

    egui::Panel::right("inspector").resizable(true).default_size(260.0).show(ctx, |ui| {
        ui.heading("Inspector");
        ui.separator();
        if let (Some(name), Some(transform)) = (selected_name.as_ref(), selected_transform.as_mut()) {
            ui.label(egui::RichText::new(name).strong());
            ui.separator();
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
                ui.horizontal(|ui| {
                    let (r, g, b) = (dl.color.x, dl.color.y, dl.color.z);
                    let size = egui::vec2(18.0, 18.0);
                    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
                    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8));
                    ui.add(egui::DragValue::new(&mut dl.color.x).prefix("R: ").speed(0.01).range(0.0..=1.0));
                    ui.add(egui::DragValue::new(&mut dl.color.y).prefix("G: ").speed(0.01).range(0.0..=1.0));
                    ui.add(egui::DragValue::new(&mut dl.color.z).prefix("B: ").speed(0.01).range(0.0..=1.0));
                });
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
                ui.horizontal(|ui| {
                    let (r, g, b) = (pl.color.x, pl.color.y, pl.color.z);
                    let size = egui::vec2(18.0, 18.0);
                    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
                    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8));
                    ui.add(egui::DragValue::new(&mut pl.color.x).prefix("R: ").speed(0.01).range(0.0..=1.0));
                    ui.add(egui::DragValue::new(&mut pl.color.y).prefix("G: ").speed(0.01).range(0.0..=1.0));
                    ui.add(egui::DragValue::new(&mut pl.color.z).prefix("B: ").speed(0.01).range(0.0..=1.0));
                });
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
                ui.horizontal(|ui| {
                    let (r, g, b) = (sl.color.x, sl.color.y, sl.color.z);
                    let size = egui::vec2(18.0, 18.0);
                    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
                    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8));
                    ui.add(egui::DragValue::new(&mut sl.color.x).prefix("R: ").speed(0.01).range(0.0..=1.0));
                    ui.add(egui::DragValue::new(&mut sl.color.y).prefix("G: ").speed(0.01).range(0.0..=1.0));
                    ui.add(egui::DragValue::new(&mut sl.color.z).prefix("B: ").speed(0.01).range(0.0..=1.0));
                });
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
                ui.horizontal(|ui| {
                    let (r, g, b) = (mat.base_color.x, mat.base_color.y, mat.base_color.z);
                    let size = egui::vec2(18.0, 18.0);
                    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
                    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8));
                    ui.add(egui::DragValue::new(&mut mat.base_color.x).prefix("R: ").speed(0.01).range(0.0..=1.0));
                    ui.add(egui::DragValue::new(&mut mat.base_color.y).prefix("G: ").speed(0.01).range(0.0..=1.0));
                    ui.add(egui::DragValue::new(&mut mat.base_color.z).prefix("B: ").speed(0.01).range(0.0..=1.0));
                });
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
            if let Some(ref _aud) = selected_audio_listener {
                ui.separator();
                ui.label("Audio Listener (active)");
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
            });
        }
        ui.separator();
        ui.add_space(5.0);
        ui.label("Camera:");
        match cam.mode {
            CameraMode::Orbit => {
                ui.label(format!("  Mode: Orbit{}", if cam.follow_target { " (following)" } else { "" }));
                ui.label(format!("  Center: ({:.2}, {:.2}, {:.2})",
                    cam.center.x, cam.center.y, cam.center.z));
                ui.label(format!("  Distance: {:.2}", cam.distance));
            }
            CameraMode::FirstPerson => {
                ui.label(format!("  Mode: 1st Person{}", if cam.follow_target { " (following)" } else { "" }));
                ui.label(format!("  Eye: ({:.2}, {:.2}, {:.2})",
                    cam.position.x, cam.position.y, cam.position.z));
                ui.label(format!("  Yaw: {:.2}  Pitch: {:.2}", cam.yaw, cam.pitch));
            }
        }
    });
}
