use std::collections::HashMap;

use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4};
use rustix_render::{DirectionalLight, PointLight, SpotLight};

use crate::scene::{Transform, Name, MeshComponent, Material, Parent};
use crate::undo::{UndoHistory, EditorAction};

#[allow(clippy::too_many_arguments)]
pub fn show_hierarchy(
    ctx: &egui::Context,
    world: &mut EcsWorld,
    selected_entity: &std::cell::RefCell<Option<hecs::Entity>>,
    pending_delete: &std::cell::RefCell<Option<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    renaming: &std::cell::RefCell<Option<hecs::Entity>>,
    rename_buffer: &std::cell::RefCell<String>,
    undo_history: &std::cell::RefCell<UndoHistory>,
) {
    egui::Panel::left("hierarchy").resizable(true).default_size(220.0).show(ctx, |ui| {
        ui.heading("Hierarchy");
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let is_renaming = *renaming.borrow();
            let mut finish_rename = None;
            let sel = *selected_entity.borrow();

            let mut children: HashMap<hecs::Entity, Vec<hecs::Entity>> = HashMap::new();
            let mut roots: Vec<hecs::Entity> = Vec::new();
            for (_e, _name) in world.query::<(&Entity, &Name)>().iter() {
                let entity = *_e;
                if let Ok(p) = world.get::<&Parent>(entity) {
                    if let Some(parent) = p.0 {
                        children.entry(parent).or_default().push(entity);
                    } else {
                        roots.push(entity);
                    }
                } else {
                    roots.push(entity);
                }
            }

            fn render_entity(
                ui: &mut egui::Ui,
                entity: hecs::Entity,
                world: &mut EcsWorld,
                depth: u32,
                is_renaming: Option<hecs::Entity>,
                sel: Option<hecs::Entity>,
                selected_entity: &std::cell::RefCell<Option<hecs::Entity>>,
                renaming: &std::cell::RefCell<Option<hecs::Entity>>,
                rename_buffer: &std::cell::RefCell<String>,
                pending_delete: &std::cell::RefCell<Option<hecs::Entity>>,
                children: &HashMap<hecs::Entity, Vec<hecs::Entity>>,
                finish_rename: &mut Option<hecs::Entity>,
            ) {
                let name = world.get::<&Name>(entity).map(|n| n.0.clone()).unwrap_or_else(|_| "Unnamed".into());
                let is_selected = sel == Some(entity);
                let indent = depth as f32 * 16.0;

                if Some(entity) == is_renaming {
                    let mut buf = rename_buffer.borrow_mut();
                    ui.horizontal(|ui| {
                        ui.add_space(indent);
                        let resp = ui.add_sized(
                            egui::vec2(ui.available_width(), 0.0),
                            egui::TextEdit::singleline(&mut *buf)
                                .text_color(egui::Color32::WHITE)
                                .desired_width(f32::INFINITY),
                        );
                        if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            *finish_rename = Some(entity);
                        }
                        if !resp.has_focus() {
                            ui.ctx().memory_mut(|mem| mem.request_focus(resp.id));
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.add_space(indent);
                        if depth > 0 {
                            ui.label(egui::RichText::new("\u{2514}").weak());
                        }
                        let resp = ui.add_sized(
                            egui::vec2(ui.available_width(), 0.0),
                            egui::Button::new(egui::RichText::new(&name).color(egui::Color32::WHITE))
                                .fill(if is_selected { egui::Color32::from_rgb(50, 90, 150) } else { egui::Color32::TRANSPARENT }),
                        );
                        if resp.clicked() {
                            *selected_entity.borrow_mut() = Some(entity);
                        }
                        if resp.double_clicked() {
                            *renaming.borrow_mut() = Some(entity);
                            *rename_buffer.borrow_mut() = name;
                        }
                        if resp.secondary_clicked() {
                            *selected_entity.borrow_mut() = Some(entity);
                        }
                        resp.context_menu(|ui| {
                            if ui.button("Rename").clicked() {
                                let n = world.get::<&Name>(entity).map(|n| n.0.clone()).unwrap_or_default();
                                *renaming.borrow_mut() = Some(entity);
                                *rename_buffer.borrow_mut() = n;
                                ui.close();
                            }
                            if ui.button("Delete").clicked() {
                                *pending_delete.borrow_mut() = Some(entity);
                                ui.close();
                            }
                            ui.separator();
                            if let Some(target) = sel {
                                if target != entity {
                                    if ui.button("Parent to Selected").clicked() {
                                        let _ = world.insert(entity, (Parent(Some(target)),));
                                        ui.close();
                                    }
                                }
                            }
                            if world.get::<&Parent>(entity).ok().and_then(|p| p.0).is_some() {
                                if ui.button("Unparent").clicked() {
                                    let _ = world.insert(entity, (Parent(None),));
                                    ui.close();
                                }
                            }
                        });
                    });
                }

                if let Some(kids) = children.get(&entity) {
                    for &child in kids {
                        render_entity(ui, child, world, depth + 1, is_renaming, sel, selected_entity, renaming, rename_buffer, pending_delete, children, finish_rename);
                    }
                }
            }

            for &root in &roots {
                render_entity(ui, root, world, 0, is_renaming, sel, selected_entity, renaming, rename_buffer, pending_delete, &children, &mut finish_rename);
            }

            if let Some(entity) = finish_rename {
                let new_name = rename_buffer.borrow().clone();
                for (e, n) in world.query_mut::<(&Entity, &mut Name)>() {
                    if *e == entity {
                        if n.0 != new_name {
                            undo_history.borrow_mut().push(EditorAction::RenameEntity { entity, old_name: n.0.clone() });
                            n.0 = new_name;
                            dirty.set(true);
                        }
                        break;
                    }
                }
                *renaming.borrow_mut() = None;
            }
        });
        if let Some(entity) = pending_delete.borrow_mut().take() {
            let mut name = String::new();
            let mut transform = Transform::default();
            let mut mesh = String::new();
            let mut mat = Vec4::new(0.7, 0.7, 0.7, 0.5);
            let mut metal = 0.0f32;
            for (e, n, t, m) in world.query_mut::<(&Entity, &Name, &Transform, &MeshComponent)>() {
                if *e == entity {
                    name = n.0.clone();
                    transform = t.clone();
                    mesh = m.0.clone();
                    if let Ok(mat_comp) = world.get::<&Material>(entity) {
                        mat = Vec4::new(mat_comp.base_color.x, mat_comp.base_color.y, mat_comp.base_color.z, mat_comp.roughness);
                        metal = mat_comp.metallic;
                    }
                    break;
                }
            }
            undo_history.borrow_mut().push(EditorAction::DeleteEntity { name, transform, mesh, material: mat, metallic: metal });
            let _ = world.despawn(entity);
            dirty.set(true);
            if *selected_entity.borrow() == Some(entity) {
                *selected_entity.borrow_mut() = None;
            }
        }
        ui.add_space(4.0);
        if ui.button("Add Entity").clicked() {
            let e = world.spawn((Name("New Entity".to_string()), Transform::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(0.7, 0.7, 0.7), roughness: 0.5, metallic: 0.0 }));
            undo_history.borrow_mut().push(EditorAction::AddEntity(e));
            *selected_entity.borrow_mut() = Some(e);
            dirty.set(true);
        }
        ui.menu_button("Create Light", |ui| {
            if ui.button("Directional").clicked() {
                let e = world.spawn((Name("Directional Light".to_string()), Transform::default(), DirectionalLight::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(1.0, 0.95, 0.8), roughness: 0.3, metallic: 0.0 }));
                undo_history.borrow_mut().push(EditorAction::AddEntity(e));
                *selected_entity.borrow_mut() = Some(e);
                dirty.set(true);
                ui.close();
            }
            if ui.button("Point").clicked() {
                let e = world.spawn((Name("Point Light".to_string()), Transform { position: Vec3::new(0.0, 3.0, 0.0), ..Default::default() }, PointLight::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(1.0, 0.9, 0.6), roughness: 0.3, metallic: 0.0 }));
                undo_history.borrow_mut().push(EditorAction::AddEntity(e));
                *selected_entity.borrow_mut() = Some(e);
                dirty.set(true);
                ui.close();
            }
            if ui.button("Spot").clicked() {
                let e = world.spawn((Name("Spot Light".to_string()), Transform { position: Vec3::new(0.0, 3.0, 0.0), ..Default::default() }, SpotLight::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(1.0, 0.95, 0.7), roughness: 0.3, metallic: 0.0 }));
                undo_history.borrow_mut().push(EditorAction::AddEntity(e));
                *selected_entity.borrow_mut() = Some(e);
                dirty.set(true);
                ui.close();
            }
        });
    });
}
