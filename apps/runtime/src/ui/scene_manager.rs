use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;

use crate::project::DockPosition;
use crate::scene::{SceneManager, merge_scene_into_world, unload_scene, load_scene, world_to_scene_by_tag, save_scene, StreamingZone};

#[allow(clippy::too_many_arguments)]
pub fn show_scene_manager(
    ctx: &egui::Context,
    world: &mut EcsWorld,
    scene_manager: &mut SceneManager,
    selected_entities: &std::cell::RefCell<Vec<hecs::Entity>>,
    undo_history: &std::cell::RefCell<crate::undo::UndoHistory>,
    dirty: &std::cell::Cell<bool>,
    dock: DockPosition,
    project_dir: &Option<String>,
) {
    if dock == DockPosition::Hidden || !scene_manager.show_manager {
        return;
    }

    let window = egui::Window::new("Scene Manager")
        .id(egui::Id::new("scene_manager_window"))
        .resizable(true)
        .default_size([280.0, 400.0]);

    match dock {
        DockPosition::Left => window.default_pos([4.0, 32.0]),
        DockPosition::Right => window.default_pos([ctx.screen_rect().width() - 284.0, 32.0]),
        DockPosition::Bottom => window.default_pos([4.0, ctx.screen_rect().height() - 404.0]),
        DockPosition::Floating | DockPosition::Hidden => window,
    }.show(ctx, |ui| {
        ui.label(egui::RichText::new("Loaded Scenes").strong());
        ui.separator();

        if scene_manager.loaded_scenes.is_empty() {
            ui.label("No additive scenes loaded.");
        } else {
            let mut to_unload: Option<String> = None;
            let mut to_save: Option<String> = None;
            for scene in &scene_manager.loaded_scenes {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&scene.name).size(13.0));
                    ui.label(egui::RichText::new(format!("{} ents", scene.entity_count)).weak().size(11.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Unload").clicked() {
                            to_unload = Some(scene.name.clone());
                        }
                        if ui.small_button("Save").clicked() {
                            to_save = Some(scene.name.clone());
                        }
                    });
                });
            }
            if let Some(name) = to_unload {
                let removed = unload_scene(world, &name);
                if removed > 0 {
                    scene_manager.unregister(&name);
                    selected_entities.borrow_mut().clear();
                    undo_history.borrow_mut().clear();
                    dirty.set(true);
                }
            }
            if let Some(name) = to_save {
                if let Some(ref dir) = project_dir {
                    let data = world_to_scene_by_tag(world, &name);
                    let path = std::path::Path::new(dir).join(format!("{}.rustixscene", name));
                    let _ = save_scene(&path, &data);
                }
            }
        }

        ui.add_space(8.0);
        ui.separator();
        if ui.button("Load Scene Additively…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Load Scene Additively")
                .add_filter("Rustix Scene", &["rustixscene"])
                .pick_file()
            {
                if let Some(data) = load_scene(&path) {
                    let name = path.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "scene".to_string());
                    let entities = merge_scene_into_world(world, &data, &name);
                    scene_manager.register(name, path.to_string_lossy().to_string(), entities.len());
                    dirty.set(true);
                }
            }
        }

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Streaming Zones").strong());
        ui.separator();

        let mut zone_to_remove: Option<usize> = None;
        for (i, zone) in scene_manager.streaming_zones.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&zone.name).size(12.0));
                    ui.label(if zone.loaded { "● Loaded" } else { "○ Unloaded" }.to_string());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").clicked() {
                            zone_to_remove = Some(i);
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Center:");
                    ui.label(format!("{:.1}, {:.1}, {:.1}", zone.center.x, zone.center.y, zone.center.z));
                });
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    ui.label(format!("{:.1}", zone.radius));
                });
                ui.horizontal(|ui| {
                    ui.label("Scene:");
                    ui.label(&zone.scene_path);
                });
            });
        }
        if let Some(idx) = zone_to_remove {
            scene_manager.streaming_zones.remove(idx);
        }

        ui.add_space(4.0);
        if ui.button("Add Streaming Zone…").clicked() {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("show_add_zone_dialog"), true));
        }

        show_add_zone_dialog(ctx, scene_manager);
    });
}

fn show_add_zone_dialog(ctx: &egui::Context, scene_manager: &mut SceneManager) {
    let show_id = egui::Id::new("show_add_zone_dialog");
    let mut show = ctx.data(|d| d.get_temp::<bool>(show_id)).unwrap_or(false);
    if !show { return; }

    egui::Window::new("Add Streaming Zone")
        .id(egui::Id::new("add_zone_window"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let name_id = egui::Id::new("zone_name");
            let path_id = egui::Id::new("zone_path");
            let cx_id = egui::Id::new("zone_cx");
            let cy_id = egui::Id::new("zone_cy");
            let cz_id = egui::Id::new("zone_cz");
            let radius_id = egui::Id::new("zone_radius");

            let mut name = ctx.data(|d| d.get_temp::<String>(name_id)).unwrap_or_else(|| "Zone".to_string());
            let mut path = ctx.data(|d| d.get_temp::<String>(path_id)).unwrap_or_default();
            let mut cx = ctx.data(|d| d.get_temp::<f32>(cx_id)).unwrap_or(0.0);
            let mut cy = ctx.data(|d| d.get_temp::<f32>(cy_id)).unwrap_or(0.0);
            let mut cz = ctx.data(|d| d.get_temp::<f32>(cz_id)).unwrap_or(0.0);
            let mut radius = ctx.data(|d| d.get_temp::<f32>(radius_id)).unwrap_or(50.0);

            ui.label("Zone Name:");
            ui.text_edit_singleline(&mut name);
            ui.add_space(4.0);
            ui.label("Scene Path (.rustixscene):");
            ui.text_edit_singleline(&mut path);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Center X");
                ui.add(egui::DragValue::new(&mut cx).speed(1.0));
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut cy).speed(1.0));
                ui.label("Z");
                ui.add(egui::DragValue::new(&mut cz).speed(1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Radius");
                ui.add(egui::DragValue::new(&mut radius).speed(1.0).clamp_range(0.1..=10000.0));
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    scene_manager.streaming_zones.push(StreamingZone {
                        name: name.clone(),
                        center: Vec3::new(cx, cy, cz),
                        radius,
                        scene_path: path.clone(),
                        loaded: false,
                    });
                    show = false;
                }
                if ui.button("Cancel").clicked() {
                    show = false;
                }
            });

            ctx.data_mut(|d| {
                d.insert_temp(name_id, name);
                d.insert_temp(path_id, path);
                d.insert_temp(cx_id, cx);
                d.insert_temp(cy_id, cy);
                d.insert_temp(cz_id, cz);
                d.insert_temp(radius_id, radius);
            });
        });
    ctx.data_mut(|d| d.insert_temp(show_id, show));
}
