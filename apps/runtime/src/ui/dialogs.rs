use winit::event_loop::ActiveEventLoop;

use crate::project::{AppScreen, ConfirmTarget, ProjectType, ProjectInfo};
use crate::sprite_editor;

pub fn show_dialogs(
    ctx: &egui::Context,
    screen: &mut AppScreen,
    target: &ActiveEventLoop,
    current_project: &mut Option<ProjectInfo>,
    dirty: &std::cell::Cell<bool>,
    show_confirm: &std::cell::Cell<bool>,
    confirm_target: &std::cell::Cell<ConfirmTarget>,
    show_settings: &std::cell::Cell<bool>,
    sprite_editor: &mut sprite_editor::SpriteEditor,
) {
    if show_settings.get() {
        if let Some(ref mut proj) = current_project {
            let mut settings = proj.settings.clone();
            egui::Window::new("Project Settings")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Resolution");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut settings.resolution_width).prefix("Width: ").range(320..=7680));
                        ui.add(egui::DragValue::new(&mut settings.resolution_height).prefix("Height: ").range(240..=4320));
                    });
                    ui.add_space(8.0);
                    ui.add(egui::Checkbox::new(&mut settings.enable_vsync, "Enable V-Sync"));
                    ui.add_space(8.0);
                    ui.add(egui::DragValue::new(&mut settings.target_fps).prefix("Target FPS: ").range(30..=480));
                    ui.add_space(8.0);
                    let mut is_3d = settings.project_type == ProjectType::Dim3;
                    ui.horizontal(|ui| {
                        ui.label("Project type:");
                        if ui.selectable_label(is_3d, "3D").clicked() { is_3d = true; }
                        if ui.selectable_label(!is_3d, "2D").clicked() { is_3d = false; }
                    });
                    settings.project_type = if is_3d { ProjectType::Dim3 } else { ProjectType::Dim2 };
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            if proj.settings != settings {
                                proj.settings = settings;
                                dirty.set(true);
                            }
                            show_settings.set(false);
                        }
                        if ui.button("Cancel").clicked() {
                            show_settings.set(false);
                        }
                    });
                });
        } else {
            show_settings.set(false);
        }
    }

    if show_confirm.get() {
        egui::Window::new("Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("You have unsaved changes. Discard them?");
                ui.horizontal(|ui| {
                    if ui.button("Discard Changes").clicked() {
                        let target_action = confirm_target.get();
                        show_confirm.set(false);
                        confirm_target.set(ConfirmTarget::None);
                        dirty.set(false);
                        match target_action {
                            ConfirmTarget::BackToHub => *screen = AppScreen::Startup,
                            ConfirmTarget::Exit => target.exit(),
                            ConfirmTarget::None => {}
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        show_confirm.set(false);
                        confirm_target.set(ConfirmTarget::None);
                    }
                });
            });
    }

    if sprite_editor.is_visible() {
        sprite_editor.show(ctx);
    }
}
