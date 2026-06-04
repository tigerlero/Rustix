use std::collections::HashMap;

use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4};
use rustix_render::{DirectionalLight, PointLight, SpotLight};

use crate::scene::{Transform, Name, MeshComponent, Material, Parent};
use crate::undo::{UndoHistory, EditorAction};
use rustix_physics::{RigidBody, Collider};

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
        ui.horizontal(|ui| {
            ui.heading("Hierarchy");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Delete").clicked() {
                    if let Some(sel) = *selected_entity.borrow() {
                        *pending_delete.borrow_mut() = Some(sel);
                    }
                }
                if ui.button("Copy").clicked() {
                    if let Some(sel) = *selected_entity.borrow() {
                        let snapshot = crate::scene::entity_to_scene_entity(world, sel);
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("copied_entity"), snapshot));
                    }
                }
                if ui.button("Paste").clicked() {
                    if let Some(copied) = ui.ctx().data(|d| d.get_temp::<crate::scene::SceneEntity>(egui::Id::new("copied_entity"))) {
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
                if ui.button("Duplicate").clicked() {
                    let sel = *selected_entity.borrow();
                    if let Some(sel) = sel {
                        let name = world.get::<&Name>(sel).ok().map(|n| n.0.clone()).unwrap_or_default();
                        let transform = world.get::<&Transform>(sel).ok().map(|r| (*r).clone()).unwrap_or_default();
                        let mesh = world.get::<&MeshComponent>(sel).ok().map(|r| (*r).clone());
                        let material = world.get::<&Material>(sel).ok().map(|r| (*r).clone());
                        let dirlight = world.get::<&DirectionalLight>(sel).ok().map(|r| (*r).clone());
                        let pointlight = world.get::<&PointLight>(sel).ok().map(|r| (*r).clone());
                        let spotlight = world.get::<&SpotLight>(sel).ok().map(|r| (*r).clone());
                        let audio = world.get::<&rustix_audio::AudioSource>(sel).ok().map(|r| (*r).clone());
                        let rigidbody = world.get::<&RigidBody>(sel).ok().map(|r| *r);
                        let collider = world.get::<&Collider>(sel).ok().map(|r| *r);
                        let audiolistener = world.get::<&rustix_audio::AudioListener>(sel).ok().map(|r| *r);
                        let camera = world.get::<&rustix_render::Camera>(sel).ok().map(|r| *r);

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
            });
        });
        ui.separator();
        let search_id = egui::Id::new("hierarchy_search");
        let mut search_filter = ctx.data(|d| d.get_temp::<String>(search_id).unwrap_or_default());
        ui.horizontal(|ui| {
            ui.add_sized(
                egui::vec2(ui.available_width(), 0.0),
                egui::TextEdit::singleline(&mut search_filter).hint_text("Search entities..."),
            );
        });
        ui.separator();
        let filter_lower = search_filter.to_lowercase();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let is_renaming = *renaming.borrow();
            let mut finish_rename = None;
            let sel = *selected_entity.borrow();

            let mut children: HashMap<hecs::Entity, Vec<hecs::Entity>> = HashMap::new();
            let mut roots: Vec<hecs::Entity> = Vec::new();
            let mut query_count = 0;
            let mut all_entities: Vec<hecs::Entity> = Vec::new();
            for (eid, _name) in world.query::<(Entity, &Name)>().iter() {
                query_count += 1;
                let entity = eid;
                all_entities.push(entity);
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
            ui.label(format!("Entities: {query_count}"));

            fn is_descendant(
                children: &HashMap<hecs::Entity, Vec<hecs::Entity>>,
                ancestor: hecs::Entity,
                descendant: hecs::Entity,
            ) -> bool {
                if let Some(kids) = children.get(&ancestor) {
                    for &child in kids {
                        if child == descendant || is_descendant(children, child, descendant) {
                            return true;
                        }
                    }
                }
                false
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
                undo_history: &std::cell::RefCell<UndoHistory>,
                dirty: &std::cell::Cell<bool>,
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
                    let drag_id = egui::Id::new(("hier_drag", entity.to_bits().get()));
                    let drop_frame = egui::Frame::none().inner_margin(2.0);
                    let (_drop_inner, dropped) = ui.dnd_drop_zone(drop_frame, |ui| {
                        let _drag = ui.dnd_drag_source(drag_id, entity, |ui| {
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
                                    if ui.button("Copy").clicked() {
                                        let snapshot = crate::scene::entity_to_scene_entity(world, entity);
                                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("copied_entity"), snapshot));
                                        ui.close();
                                    }
                                    if ui.button("Paste").clicked() {
                                        if let Some(copied) = ui.ctx().data(|d| d.get_temp::<crate::scene::SceneEntity>(egui::Id::new("copied_entity"))) {
                                            let mut pasted = copied.clone();
                                            pasted.name = format!("{} (Pasted)", pasted.name);
                                            pasted.position[0] += 1.0;
                                            let new_entity = crate::scene::spawn_entity(world, &pasted);
                                            let snapshot = crate::scene::entity_to_scene_entity(world, new_entity);
                                            undo_history.borrow_mut().push(EditorAction::AddEntity { entity: new_entity, snapshot });
                                            *selected_entity.borrow_mut() = Some(new_entity);
                                            dirty.set(true);
                                        }
                                        ui.close();
                                    }
                                    if ui.button("Duplicate").clicked() {
                                        let name = world.get::<&Name>(entity).ok().map(|n| n.0.clone()).unwrap_or_default();
                                        let transform = world.get::<&Transform>(entity).ok().map(|r| (*r).clone()).unwrap_or_default();
                                        let mesh = world.get::<&MeshComponent>(entity).ok().map(|r| (*r).clone());
                                        let material = world.get::<&Material>(entity).ok().map(|r| (*r).clone());
                                        let dirlight = world.get::<&DirectionalLight>(entity).ok().map(|r| (*r).clone());
                                        let pointlight = world.get::<&PointLight>(entity).ok().map(|r| (*r).clone());
                                        let spotlight = world.get::<&SpotLight>(entity).ok().map(|r| (*r).clone());
                                        let audio = world.get::<&rustix_audio::AudioSource>(entity).ok().map(|r| (*r).clone());
                                        let rigidbody = world.get::<&RigidBody>(entity).ok().map(|r| *r);
                                        let collider = world.get::<&Collider>(entity).ok().map(|r| *r);
                                        let audiolistener = world.get::<&rustix_audio::AudioListener>(entity).ok().map(|r| *r);
                                        let camera = world.get::<&rustix_render::Camera>(entity).ok().map(|r| *r);

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
                                        ui.close();
                                    }
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
                        });
                    });
                    if let Some(dropped_entity) = dropped {
                        let dropped = *dropped_entity;
                        if dropped != entity && !is_descendant(children, entity, dropped) {
                            let old_parent = world.get::<&Parent>(dropped).ok().and_then(|p| p.0);
                            let _ = world.insert(dropped, (Parent(Some(entity)),));
                            undo_history.borrow_mut().push(EditorAction::ParentChanged {
                                entity: dropped,
                                old_parent,
                                new_parent: Some(entity),
                            });
                            dirty.set(true);
                        }
                    }
                }

                if let Some(kids) = children.get(&entity) {
                    for &child in kids {
                        render_entity(ui, child, world, depth + 1, is_renaming, sel, selected_entity, renaming, rename_buffer, pending_delete, children, finish_rename, undo_history, dirty);
                    }
                }
            }

            if filter_lower.is_empty() {
                for &root in &roots {
                    render_entity(ui, root, world, 0, is_renaming, sel, selected_entity, renaming, rename_buffer, pending_delete, &children, &mut finish_rename, undo_history, dirty);
                }
            } else {
                for &entity in &all_entities {
                    let name = world.get::<&Name>(entity).map(|n| n.0.clone()).unwrap_or_else(|_| "Unnamed".into());
                    if name.to_lowercase().contains(&filter_lower) {
                        render_entity(ui, entity, world, 0, is_renaming, sel, selected_entity, renaming, rename_buffer, pending_delete, &children, &mut finish_rename, undo_history, dirty);
                    }
                }
            }

            if let Some(entity) = finish_rename {
                let new_name = rename_buffer.borrow().clone();
                for (e, n) in world.query_mut::<(Entity, &mut Name)>() {
                    if e == entity {
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
        ctx.data_mut(|d| d.insert_temp(search_id, search_filter));
        if let Some(entity) = pending_delete.borrow_mut().take() {
            let snapshot = crate::scene::entity_to_scene_entity(world, entity);
            undo_history.borrow_mut().push(EditorAction::DeleteEntity { entity, snapshot });
            let _ = world.despawn(entity);
            dirty.set(true);
            if *selected_entity.borrow() == Some(entity) {
                *selected_entity.borrow_mut() = None;
            }
        }
        ui.add_space(4.0);
        if ui.button("Create Empty").clicked() {
            let count = world.query::<(&Name,)>().iter().filter(|(n,)| n.0.starts_with("Empty")).count() as u32;
            let e = world.spawn((
                Name(format!("Empty {}", count + 1)),
                Transform::default(),
            ));
            let snapshot = crate::scene::entity_to_scene_entity(world, e);
            undo_history.borrow_mut().push(EditorAction::AddEntity { entity: e, snapshot });
            *selected_entity.borrow_mut() = Some(e);
            dirty.set(true);
        }
        ui.menu_button("Create 3D Object", |ui| {
            let mut spawn = |name: &str, mesh: &str, color: Vec3| {
                let count = world.query::<(&Name,)>().iter().filter(|(n,)| n.0.starts_with(name)).count() as u32;
                let e = world.spawn((
                    Name(format!("{} {}", name, count + 1)),
                    Transform { position: Vec3::new(0.0, 0.5 + count as f32 * 0.5, 0.0), ..Default::default() },
                    MeshComponent(mesh.into()),
                    Material { base_color: color, roughness: 0.5, metallic: 0.0 },
                ));
                tracing::info!("spawned {} entity {:?}", name, e);
                let snapshot = crate::scene::entity_to_scene_entity(world, e);
                undo_history.borrow_mut().push(EditorAction::AddEntity { entity: e, snapshot });
                *selected_entity.borrow_mut() = Some(e);
                dirty.set(true);
            };
            if ui.button("Cube").clicked() {
                spawn("Cube", "Cube", Vec3::new(0.7, 0.7, 0.7));
                ui.close();
            }
            if ui.button("Sphere").clicked() {
                spawn("Sphere", "Sphere", Vec3::new(0.7, 0.5, 0.4));
                ui.close();
            }
            if ui.button("Torus").clicked() {
                spawn("Torus", "Torus", Vec3::new(0.5, 0.7, 0.6));
                ui.close();
            }
            if ui.button("Capsule").clicked() {
                spawn("Capsule", "Capsule", Vec3::new(0.6, 0.6, 0.8));
                ui.close();
            }
            if ui.button("Icosphere").clicked() {
                spawn("Icosphere", "Icosphere", Vec3::new(0.5, 0.5, 0.7));
                ui.close();
            }
            if ui.button("Plane").clicked() {
                spawn("Plane", "Plane", Vec3::new(0.4, 0.7, 0.4));
                ui.close();
            }
            if ui.button("Terrain").clicked() {
                spawn("Terrain", "Terrain", Vec3::new(0.35, 0.6, 0.3));
                ui.close();
            }
        });
        ui.menu_button("Create Light", |ui| {
            if ui.button("Directional").clicked() {
                let e = world.spawn((Name("Directional Light".to_string()), Transform::default(), DirectionalLight::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(1.0, 0.95, 0.8), roughness: 0.3, metallic: 0.0 }));
                let snapshot = crate::scene::entity_to_scene_entity(world, e);
                undo_history.borrow_mut().push(EditorAction::AddEntity { entity: e, snapshot });
                *selected_entity.borrow_mut() = Some(e);
                dirty.set(true);
                ui.close();
            }
            if ui.button("Point").clicked() {
                let e = world.spawn((Name("Point Light".to_string()), Transform { position: Vec3::new(0.0, 3.0, 0.0), ..Default::default() }, PointLight::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(1.0, 0.9, 0.6), roughness: 0.3, metallic: 0.0 }));
                let snapshot = crate::scene::entity_to_scene_entity(world, e);
                undo_history.borrow_mut().push(EditorAction::AddEntity { entity: e, snapshot });
                *selected_entity.borrow_mut() = Some(e);
                dirty.set(true);
                ui.close();
            }
            if ui.button("Spot").clicked() {
                let e = world.spawn((Name("Spot Light".to_string()), Transform { position: Vec3::new(0.0, 3.0, 0.0), ..Default::default() }, SpotLight::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(1.0, 0.95, 0.7), roughness: 0.3, metallic: 0.0 }));
                let snapshot = crate::scene::entity_to_scene_entity(world, e);
                undo_history.borrow_mut().push(EditorAction::AddEntity { entity: e, snapshot });
                *selected_entity.borrow_mut() = Some(e);
                dirty.set(true);
                ui.close();
            }
        });
    });
}
