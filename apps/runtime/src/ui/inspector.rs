use rustix_core::ecs::{EcsWorld, Entity};
use rustix_render::{DirectionalLight, PointLight, SpotLight};
use rustix_audio::{AudioSource, AudioListener};

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
        world.query::<(&Entity, &Name)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, n)| n.0.clone())
    });
    let mut selected_transform: Option<Transform> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &Transform)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, t)| t.clone())
    });
    let mut selected_dirlight: Option<DirectionalLight> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &DirectionalLight)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, l)| l.clone())
    });
    let mut selected_pointlight: Option<PointLight> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &PointLight)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, l)| l.clone())
    });
    let mut selected_spotlight: Option<SpotLight> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &SpotLight)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, l)| l.clone())
    });
    let mut selected_material: Option<Material> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &Material)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, m)| m.clone())
    });
    let mut selected_audio_source: Option<AudioSource> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &AudioSource)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, a)| a.clone())
    });
    let mut selected_audio_listener: Option<AudioListener> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &AudioListener)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, a)| a.clone())
    });

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
                for (e, t) in world.query_mut::<(&Entity, &mut Transform)>() {
                    if *e == target_entity {
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
            if let Some(target_entity) = selected_entity_val {
                if let Some(ref dl) = selected_dirlight {
                    for (e, l) in world.query_mut::<(&Entity, &mut DirectionalLight)>() {
                        if *e == target_entity { *l = *dl; dirty.set(true); break; }
                    }
                }
                if let Some(ref pl) = selected_pointlight {
                    for (e, l) in world.query_mut::<(&Entity, &mut PointLight)>() {
                        if *e == target_entity { *l = *pl; dirty.set(true); break; }
                    }
                }
                if let Some(ref sl) = selected_spotlight {
                    for (e, l) in world.query_mut::<(&Entity, &mut SpotLight)>() {
                        if *e == target_entity { *l = *sl; dirty.set(true); break; }
                    }
                }
                if let Some(ref mat) = selected_material {
                    for (e, m) in world.query_mut::<(&Entity, &mut Material)>() {
                        if *e == target_entity { *m = mat.clone(); dirty.set(true); break; }
                    }
                }
            }
            
            if let Some(ref mut dl) = selected_dirlight {
                ui.separator();
                ui.label("Directional Light");
                ui.horizontal(|ui| { ui.label("Color"); ui.add(egui::DragValue::new(&mut dl.color.x).prefix("R:")); ui.add(egui::DragValue::new(&mut dl.color.y).prefix("G:")); ui.add(egui::DragValue::new(&mut dl.color.z).prefix("B:")); });
                ui.add(egui::DragValue::new(&mut dl.intensity).prefix("Intensity: ").speed(0.1));
            }
            if let Some(ref mut pl) = selected_pointlight {
                ui.separator();
                ui.label("Point Light");
                ui.horizontal(|ui| { ui.label("Color"); ui.add(egui::DragValue::new(&mut pl.color.x).prefix("R:")); ui.add(egui::DragValue::new(&mut pl.color.y).prefix("G:")); ui.add(egui::DragValue::new(&mut pl.color.z).prefix("B:")); });
                ui.add(egui::DragValue::new(&mut pl.intensity).prefix("Intensity: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut pl.radius).prefix("Radius: ").speed(0.1));
            }
            if let Some(ref mut sl) = selected_spotlight {
                ui.separator();
                ui.label("Spot Light");
                ui.horizontal(|ui| { ui.label("Color"); ui.add(egui::DragValue::new(&mut sl.color.x).prefix("R:")); ui.add(egui::DragValue::new(&mut sl.color.y).prefix("G:")); ui.add(egui::DragValue::new(&mut sl.color.z).prefix("B:")); });
                ui.add(egui::DragValue::new(&mut sl.intensity).prefix("Intensity: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut sl.inner_angle).prefix("Inner angle: ").speed(0.01));
                ui.add(egui::DragValue::new(&mut sl.outer_angle).prefix("Outer angle: ").speed(0.01));
                ui.add(egui::DragValue::new(&mut sl.radius).prefix("Radius: ").speed(0.1));
            }
            if let Some(ref mut mat) = selected_material {
                ui.separator();
                ui.label("Material");
                ui.horizontal(|ui| {
                    ui.label("Base");
                    ui.add(egui::DragValue::new(&mut mat.base_color.x).prefix("R:").speed(0.01));
                    ui.add(egui::DragValue::new(&mut mat.base_color.y).prefix("G:").speed(0.01));
                    ui.add(egui::DragValue::new(&mut mat.base_color.z).prefix("B:").speed(0.01));
                });
                ui.add(egui::DragValue::new(&mut mat.roughness).prefix("Roughness: ").speed(0.01).range(0.01..=1.0));
                ui.add(egui::DragValue::new(&mut mat.metallic).prefix("Metallic: ").speed(0.01).range(0.0..=1.0));
            }
            if let Some(ref mut aud) = selected_audio_source {
                ui.separator();
                ui.label("Audio Source");
                ui.add(egui::DragValue::new(&mut aud.min_distance).prefix("Min dist: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut aud.max_distance).prefix("Max dist: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut aud.rolloff).prefix("Rolloff: ").speed(0.1));
            }
            if let Some(ref _aud) = selected_audio_listener {
                ui.separator();
                ui.label("Audio Listener (active)");
            }

            if let Some(target_entity) = selected_entity_val {
                if let Some(ref aud_src) = selected_audio_source {
                    for (e, a) in world.query_mut::<(&Entity, &mut AudioSource)>() {
                        if *e == target_entity { *a = *aud_src; dirty.set(true); break; }
                    }
                }
                if let Some(ref aud_listener) = selected_audio_listener {
                    for (e, a) in world.query_mut::<(&Entity, &mut AudioListener)>() {
                        if *e == target_entity { *a = *aud_listener; dirty.set(true); break; }
                    }
                }
            }
        } else {
            ui.label("Select an object in the Hierarchy to inspect.");
            ui.add_space(10.0);
            ui.label(egui::RichText::new("No object selected").italics());
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
