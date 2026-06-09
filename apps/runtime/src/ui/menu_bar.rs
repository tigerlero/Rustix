use std::path::Path;

use rustix_core::ecs::EcsWorld;
use rustix_platform::input::InputManager;

use crate::camera::CameraMode;
use crate::project::{AppScreen, ConfirmTarget, ProjectType, ProjectInfo, write_project_file, EditorCameraState, LayoutState};
use crate::scene::world_to_scene;
use crate::sprite_editor;
use crate::ui::viewport::{ViewportManager, MAX_VIEWPORTS};
use rustix_platform::window::{CursorMode, WindowHandle};

#[allow(clippy::too_many_arguments)]
#[allow(deprecated)]
pub fn show_menu_bar(
    ctx: &egui::Context,
    viewport_manager: &mut ViewportManager,
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
    window: &mut WindowHandle,
) {
    egui::Panel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            let star = if dirty.get() { " *" } else { "" };
            ui.label(egui::RichText::new(format!("{project_name}{star}")).strong());
            ui.label(egui::RichText::new("\u{2014} Rustix Editor").weak());
            if *screen == crate::project::AppScreen::PlayTest {
                ui.label(egui::RichText::new("▶ PLAYING").color(egui::Color32::from_rgb(255, 100, 100)).strong());
            }
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
                        let cam = viewport_manager.primary_camera();
                        proj.settings.resolution_width = *ww;
                        proj.settings.resolution_height = *wh;
                        proj.scene = world_to_scene(world);
                        proj.editor_camera = Some(EditorCameraState {
                            position: cam.position.into(),
                            center: cam.center.into(),
                            yaw: cam.yaw,
                            pitch: cam.pitch,
                            distance: cam.distance,
                            mode: cam.mode,
                            follow_target: cam.follow_target,
                        });
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
                            let cam = viewport_manager.primary_camera();
                            proj.settings.resolution_width = *ww;
                            proj.settings.resolution_height = *wh;
                            proj.scene = world_to_scene(world);
                            proj.editor_camera = Some(EditorCameraState {
                                position: cam.position.into(),
                                center: cam.center.into(),
                                yaw: cam.yaw,
                                pitch: cam.pitch,
                                distance: cam.distance,
                                mode: cam.mode,
                                follow_target: cam.follow_target,
                            });
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
            let prefs_id = egui::Id::new("show_preferences");
            let mut show_prefs = ctx.data(|d| d.get_temp::<bool>(prefs_id).unwrap_or(false));
            ui.menu_button("View", |ui| {
                let count = viewport_manager.viewports.len();
                if count < MAX_VIEWPORTS {
                    if ui.button("New Viewport").clicked() {
                        viewport_manager.add_viewport();
                        ui.close();
                    }
                }
                if count > 1 {
                    ui.separator();
                    for i in 1..count {
                        let label = format!("Close {}", viewport_manager.viewports[i].name);
                        if ui.button(&label).clicked() {
                            viewport_manager.remove_viewport(i);
                            ui.close();
                        }
                    }
                }
                ui.separator();

                // Panel Position controls
                if let Some(ref mut proj) = current_project {
                    let layout = proj.layout.get_or_insert_with(LayoutState::default);
                    ui.menu_button("Hierarchy Position", |ui| {
                        let positions = [
                            crate::project::DockPosition::Left,
                            crate::project::DockPosition::Right,
                            crate::project::DockPosition::Bottom,
                            crate::project::DockPosition::Floating,
                            crate::project::DockPosition::Hidden,
                        ];
                        for pos in positions {
                            if ui.selectable_label(layout.hierarchy_dock == pos, format!("{:?}", pos)).clicked() {
                                layout.hierarchy_dock = pos;
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Inspector Position", |ui| {
                        let positions = [
                            crate::project::DockPosition::Left,
                            crate::project::DockPosition::Right,
                            crate::project::DockPosition::Bottom,
                            crate::project::DockPosition::Floating,
                            crate::project::DockPosition::Hidden,
                        ];
                        for pos in positions {
                            if ui.selectable_label(layout.inspector_dock == pos, format!("{:?}", pos)).clicked() {
                                layout.inspector_dock = pos;
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Console Position", |ui| {
                        let positions = [
                            crate::project::DockPosition::Left,
                            crate::project::DockPosition::Right,
                            crate::project::DockPosition::Bottom,
                            crate::project::DockPosition::Floating,
                            crate::project::DockPosition::Hidden,
                        ];
                        for pos in positions {
                            if ui.selectable_label(layout.console_dock == pos, format!("{:?}", pos)).clicked() {
                                layout.console_dock = pos;
                                ui.close();
                            }
                        }
                    });
                }

                ui.separator();
                let current = window.cursor_mode();
                if ui.selectable_label(current == CursorMode::Normal, "Cursor: Normal").clicked() {
                    window.set_cursor_mode(CursorMode::Normal);
                    ui.close();
                }
                if ui.selectable_label(current == CursorMode::Hidden, "Cursor: Hidden").clicked() {
                    window.set_cursor_mode(CursorMode::Hidden);
                    ui.close();
                }
                if ui.selectable_label(current == CursorMode::Captured, "Cursor: Captured").clicked() {
                    window.set_cursor_mode(CursorMode::Captured);
                    ui.close();
                }
                if ui.selectable_label(current == CursorMode::RawDelta, "Cursor: Raw Delta").clicked() {
                    window.set_cursor_mode(CursorMode::RawDelta);
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Preferences").clicked() {
                    show_prefs = true;
                    ui.close();
                }
            });
            if show_prefs {
                egui::Window::new("Preferences")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label("Editor Preferences");
                        ui.add_space(8.0);
                        ui.label("(Editor-specific settings will appear here in future updates.)");
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                show_prefs = false;
                            }
                        });
                    });
            }
            ctx.data_mut(|d| d.insert_temp(prefs_id, show_prefs));

            ui.menu_button("Assets", |ui| {
                if ui.button("Import New Asset\u{2026}").clicked() {
                    ui.close();
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Import New Asset")
                        .add_filter("GLB", &["glb"])
                        .add_filter("glTF", &["gltf"])
                        .add_filter("OBJ", &["obj"])
                        .add_filter("FBX", &["fbx"])
                        .pick_file()
                    {
                        pending_mesh_load.replace(Some(path.to_string_lossy().to_string()));
                    }
                }
                ui.separator();
                if ui.button("Sprite Editor").clicked() {
                    sprite_editor.set_visible(true);
                    ui.close();
                }
            });

            let about_id = egui::Id::new("show_about");
            let mut show_about = ctx.data(|d| d.get_temp::<bool>(about_id).unwrap_or(false));
            ui.menu_button("Help", |ui| {
                if ui.button("About Rustix").clicked() {
                    show_about = true;
                    ui.close();
                }
            });
            if show_about {
                egui::Window::new("About Rustix")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.heading("Rustix Engine");
                        ui.label("Version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("A game engine built with Rust, Vulkan, and egui.");
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                show_about = false;
                            }
                        });
                    });
            }
            ctx.data_mut(|d| d.insert_temp(about_id, show_about));
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
                let cam = viewport_manager.primary_camera_mut();
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
