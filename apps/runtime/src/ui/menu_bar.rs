use std::path::Path;

use rustix_core::ecs::EcsWorld;
use rustix_platform::input::InputManager;

use crate::camera::{EditorCamera, CameraMode};
use crate::project::{AppScreen, ConfirmTarget, ProjectType, ProjectInfo, write_project_file};
use crate::scene::world_to_scene;
use crate::sprite_editor;

#[allow(clippy::too_many_arguments)]
pub fn show_menu_bar(
    ctx: &egui::Context,
    cam: &mut EditorCamera,
    _input: &InputManager,
    target: &winit::event_loop::ActiveEventLoop,
    screen: &mut AppScreen,
    ww: &u32,
    wh: &u32,
    fps: &u64,
    open_project: &std::cell::RefCell<Option<String>>,
    new_project: &std::cell::RefCell<Option<String>>,
    project_name: &str,
    current_project: &mut Option<ProjectInfo>,
    project_dir: &mut Option<String>,
    world: &mut EcsWorld,
    dirty: &std::cell::Cell<bool>,
    show_confirm: &std::cell::Cell<bool>,
    confirm_target: &std::cell::Cell<ConfirmTarget>,
    show_settings: &std::cell::Cell<bool>,
    sprite_editor: &mut sprite_editor::SpriteEditor,
    pending_mesh_load: &std::cell::RefCell<Option<String>>,
) {
    egui::Panel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            let star = if dirty.get() { " *" } else { "" };
            ui.label(egui::RichText::new(format!("{project_name}{star}")).strong());
            ui.label(egui::RichText::new("\u{2014} Rustix Editor").weak());
            ui.separator();
            ui.separator();
            ui.menu_button("File", |ui| {
                if ui.button("New Project").clicked() {
                    ui.close();
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Create New Project")
                        .pick_folder()
                    {
                        *new_project.borrow_mut() = Some(path.to_string_lossy().to_string());
                    }
                }
                if ui.button("Open Project\u{2026}").clicked() {
                    ui.close();
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Open Project")
                        .pick_folder()
                    {
                        *open_project.borrow_mut() = Some(path.to_string_lossy().to_string());
                    }
                }
                ui.separator();
                if ui.button("Save").clicked() {
                    ui.close();
                    if let Some(ref mut proj) = current_project {
                        proj.settings.resolution_width = *ww;
                        proj.settings.resolution_height = *wh;
                        proj.scene = world_to_scene(world);
                        if let Some(ref dir) = project_dir {
                            let _ = write_project_file(Path::new(dir), proj);
                        }
                    }
                    dirty.set(false);
                }
                if ui.button("Save As\u{2026}").clicked() {
                    ui.close();
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Save Project As")
                        .pick_folder()
                    {
                        let dir = Path::new(&path);
                        if let Some(ref mut proj) = current_project {
                            proj.settings.resolution_width = *ww;
                            proj.settings.resolution_height = *wh;
                            proj.scene = world_to_scene(world);
                            let _ = write_project_file(dir, proj);
                            *project_dir = Some(path.to_string_lossy().to_string());
                            *open_project.borrow_mut() = Some(path.to_string_lossy().to_string());
                            dirty.set(false);
                        }
                    }
                }
                ui.separator();
                if ui.button("Load GLB\u{2026}").clicked() {
                    ui.close();
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Import GLB Mesh")
                        .add_filter("GLB", &["glb"])
                        .pick_file()
                    {
                        pending_mesh_load.replace(Some(path.to_string_lossy().to_string()));
                    }
                }
                if ui.button("Project Settings\u{2026}").clicked() {
                    show_settings.set(true);
                    ui.close();
                }
                ui.separator();
                if ui.button("Back to Project Hub").clicked() {
                    if dirty.get() {
                        show_confirm.set(true);
                        confirm_target.set(ConfirmTarget::BackToHub);
                    } else {
                        *screen = AppScreen::Startup;
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    if dirty.get() {
                        show_confirm.set(true);
                        confirm_target.set(ConfirmTarget::Exit);
                    } else {
                        target.exit();
                    }
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Preferences").clicked() { ui.close(); }
            });
            ui.menu_button("Assets", |ui| {
                if ui.button("Import New Asset\u{2026}").clicked() { ui.close(); }
                ui.separator();
                if ui.button("Sprite Editor").clicked() {
                    sprite_editor.set_visible(true);
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About Rustix").clicked() { ui.close(); }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ptype = current_project.as_ref().map(|p| match p.settings.project_type {
                    ProjectType::Dim3 => "3D",
                    ProjectType::Dim2 => "2D",
                }).unwrap_or("");
                if !ptype.is_empty() {
                    ui.label(egui::RichText::new(ptype).color(egui::Color32::from_rgb(120, 240, 200)).weak());
                }
                ui.label(format!("FPS: {fps}"));
                ui.separator();
                if ui.selectable_label(cam.follow_target, "Follow").clicked() {
                    cam.follow_target = !cam.follow_target;
                }
                let orbit_selected = cam.mode == CameraMode::Orbit;
                if ui.selectable_label(orbit_selected, "Orbit").clicked() && !orbit_selected {
                    cam.mode = CameraMode::Orbit;
                }
                if ui.selectable_label(!orbit_selected, "1stP").clicked() && orbit_selected {
                    cam.mode = CameraMode::FirstPerson;
                }
                ui.label(egui::RichText::new("Cam:").weak());
            });
        });
    });
}
