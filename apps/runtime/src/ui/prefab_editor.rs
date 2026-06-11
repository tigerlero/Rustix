//! Prefab editor UI panel.

use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;

use crate::prefab::{self, Prefab, PrefabEditor, list_prefabs, ensure_prefabs_dir};
use crate::project::DockPosition;

/// Show the prefab editor as a dockable egui panel.
pub fn show_prefab_editor(
    ctx: &egui::Context,
    prefab_editor: &mut PrefabEditor,
    world: &mut EcsWorld,
    selected_entities: &std::cell::RefCell<Vec<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    project_dir: &Option<String>,
    dock: DockPosition,
) {
    if !prefab_editor.show {
        return;
    }

    let title = "Prefab Editor";
    match dock {
        DockPosition::Left => {
            egui::SidePanel::left("prefab_editor_panel")
                .resizable(true)
                .default_width(250.0)
                .show(ctx, |ui| { ui.label(egui::RichText::new(title).heading()); ui.separator(); prefab_ui(ui, prefab_editor, world, selected_entities, dirty, project_dir); });
        }
        DockPosition::Right => {
            egui::SidePanel::right("prefab_editor_panel")
                .resizable(true)
                .default_width(250.0)
                .show(ctx, |ui| { ui.label(egui::RichText::new(title).heading()); ui.separator(); prefab_ui(ui, prefab_editor, world, selected_entities, dirty, project_dir); });
        }
        DockPosition::Bottom => {
            egui::TopBottomPanel::bottom("prefab_editor_panel")
                .resizable(true)
                .default_height(200.0)
                .show(ctx, |ui| { ui.label(egui::RichText::new(title).heading()); ui.separator(); prefab_ui(ui, prefab_editor, world, selected_entities, dirty, project_dir); });
        }
        DockPosition::Floating | DockPosition::Hidden => {
            egui::Window::new(title)
                .id(egui::Id::new("prefab_editor_window"))
                .show(ctx, |ui| { prefab_ui(ui, prefab_editor, world, selected_entities, dirty, project_dir); });
        }
    }
}

fn prefab_ui(
    ui: &mut egui::Ui,
    prefab_editor: &mut PrefabEditor,
    world: &mut EcsWorld,
    selected_entities: &std::cell::RefCell<Vec<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    project_dir: &Option<String>,
) {
    // -- Create Prefab from Selection --
    ui.group(|ui| {
        ui.label("Create Prefab");
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut prefab_editor.new_prefab_name);
        });
        if ui.button("Save from Selection").clicked() {
            let selected: Vec<hecs::Entity> = selected_entities.borrow().clone();
            if let Some(mut prefab) = Prefab::from_selection(world, &selected) {
                let name = if prefab_editor.new_prefab_name.trim().is_empty() {
                    "NewPrefab".to_string()
                } else {
                    prefab_editor.new_prefab_name.trim().to_string()
                };
                prefab.name = name.clone();
                if let Some(ref dir) = project_dir {
                    let prefabs_dir = ensure_prefabs_dir(std::path::Path::new(dir));
                    let path = prefabs_dir.join(format!("{name}.rustixprefab"));
                    match prefab.save(&path) {
                        Ok(()) => {
                            tracing::info!("saved prefab '{}' to {}", name, path.display());
                            prefab_editor.selected_prefab = Some(name);
                            prefab_editor.new_prefab_name.clear();
                        }
                        Err(e) => tracing::error!("failed to save prefab: {e}"),
                    }
                }
            } else {
                tracing::warn!("no entities selected to create prefab");
            }
        }
    });

    ui.separator();

    // -- Instantiate Prefab --
    ui.group(|ui| {
        ui.label("Instantiate Prefab");
        ui.horizontal(|ui| {
            ui.label("Position:");
            let mut pos: [f32; 3] = prefab_editor.instantiate_at.into();
            ui.add(egui::DragValue::new(&mut pos[0]).prefix("X "));
            ui.add(egui::DragValue::new(&mut pos[1]).prefix("Y "));
            ui.add(egui::DragValue::new(&mut pos[2]).prefix("Z "));
            prefab_editor.instantiate_at = pos.into();
        });

        if let Some(ref dir) = project_dir {
            let prefabs = list_prefabs(std::path::Path::new(dir));
            if prefabs.is_empty() {
                ui.label("No prefabs found.");
            } else {
                egui::ComboBox::from_label("Prefab")
                    .selected_text(
                        prefab_editor.selected_prefab.as_deref().unwrap_or("Select..."),
                    )
                    .show_ui(ui, |ui| {
                        for (name, _) in &prefabs {
                            ui.selectable_value(
                                &mut prefab_editor.selected_prefab,
                                Some(name.clone()),
                                name,
                            );
                        }
                    });

                if ui.button("Instantiate").clicked() {
                    if let Some(ref selected_name) = prefab_editor.selected_prefab {
                        if let Some((_, path)) = prefabs.iter().find(|(n, _)| n == selected_name) {
                            if let Some(prefab) = Prefab::load(path) {
                                let roots = prefab.instantiate(world, prefab_editor.instantiate_at);
                                // Tag all spawned entities with the prefab path
                                for (idx, entity) in roots.iter().enumerate() {
                                    if let Ok(mut instance) = world.get::<&mut prefab::PrefabInstance>(*entity) {
                                        instance.prefab_path = selected_name.clone();
                                    }
                                }
                                // Select the first root
                                if let Some(&first) = roots.first() {
                                    *selected_entities.borrow_mut() = vec![first];
                                }
                                tracing::info!("instantiated prefab '{}' with {} root(s)", selected_name, roots.len());
                                dirty.set(true);
                            } else {
                                tracing::warn!("failed to load prefab '{}'", path.display());
                            }
                        }
                    }
                }
            }
        } else {
            ui.label("No project open.");
        }
    });

    ui.separator();

    // -- Override Tracking --
    ui.group(|ui| {
        ui.label("Overrides");
        let selected = selected_entities.borrow().clone();
        let mut found = false;
        for entity in selected {
            let instance_opt: Option<prefab::PrefabInstance> = world
                .get::<&prefab::PrefabInstance>(entity)
                .ok()
                .map(|r| (*r).clone());
            if let Some(instance) = instance_opt {
                found = true;
                ui.horizontal(|ui| {
                    ui.label(format!("Entity {} (idx {})", entity.id(), instance.entity_idx));
                    if let Ok(name) = world.get::<&crate::scene::Name>(entity) {
                        ui.label(format!("'{}'", name.0));
                    }
                });
                if instance.overrides.is_empty() {
                    ui.label("  No overrides.");
                } else {
                    for ov in &instance.overrides {
                        ui.label(format!("  - {:?}", ov));
                    }
                }

                if ui.button("Compute Overrides").clicked() {
                    if let Some(ref dir) = project_dir {
                        let prefabs = list_prefabs(std::path::Path::new(dir));
                        if let Some((_, path)) = prefabs.iter().find(|(n, _)| n == &instance.prefab_path) {
                            if let Some(prefab) = Prefab::load(path) {
                                let overrides = prefab::compute_overrides(world, entity, &prefab);
                                if let Ok(mut inst) = world.get::<&mut prefab::PrefabInstance>(entity) {
                                    inst.overrides = overrides.clone();
                                }
                                tracing::info!("computed {} override(s) for entity {}", overrides.len(), entity.id());
                            }
                        }
                    }
                }

                if ui.button("Apply Overrides").clicked() {
                    prefab::apply_overrides(world, entity, &instance.overrides);
                    tracing::info!("applied {} override(s) to entity {}", instance.overrides.len(), entity.id());
                    dirty.set(true);
                }
            }
        }
        if !found {
            ui.label("Select a prefab instance to view overrides.");
        }
    });

    ui.separator();

    // -- Revert to Prefab --
    ui.group(|ui| {
        ui.label("Revert");
        let selected = selected_entities.borrow().clone();
        let mut revert_targets = Vec::new();
        for entity in selected {
            if world.get::<&prefab::PrefabInstance>(entity).is_ok() {
                revert_targets.push(entity);
            }
        }
        if !revert_targets.is_empty() {
            if ui.button("Revert Selected to Prefab").clicked() {
                if let Some(ref dir) = project_dir {
                    let prefabs = list_prefabs(std::path::Path::new(dir));
                    for entity in revert_targets {
                        let instance_opt: Option<prefab::PrefabInstance> = world
                            .get::<&prefab::PrefabInstance>(entity)
                            .ok()
                            .map(|r| (*r).clone());
                        if let Some(instance) = instance_opt {
                            if let Some((_, path)) = prefabs.iter().find(|(n, _)| n == &instance.prefab_path) {
                                if let Some(prefab) = Prefab::load(path) {
                                    if let Some(original) = prefab.entities.get(instance.entity_idx) {
                                        // Revert transform
                                        if let Ok(mut t) = world.get::<&mut crate::scene::Transform>(entity) {
                                            t.position = original.position.into();
                                            t.rotation = original.rotation.into();
                                            t.scale = original.scale.into();
                                        }
                                        if let Ok(mut name) = world.get::<&mut crate::scene::Name>(entity) {
                                            name.0 = original.name.clone();
                                        }
                                        if let Some(ref mesh) = original.mesh {
                                            let _ = world.insert(entity, (crate::scene::MeshComponent(mesh.clone()),));
                                        }
                                        if let Some(ref mat) = original.material {
                                            let _ = world.insert(entity, (mat.clone(),));
                                        }
                                        // Clear overrides
                                        if let Ok(mut inst) = world.get::<&mut prefab::PrefabInstance>(entity) {
                                            inst.overrides.clear();
                                        }
                                        dirty.set(true);
                                        tracing::info!("reverted entity {} to prefab '{}'", entity.id(), instance.prefab_path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            ui.label("Select prefab instances to revert.");
        }
    });
}
