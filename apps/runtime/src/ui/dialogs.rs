use winit::event_loop::ActiveEventLoop;

use crate::project::{AppScreen, ConfirmTarget};
use crate::sprite_editor;

pub fn show_dialogs(
    ctx: &egui::Context,
    screen: &mut AppScreen,
    target: &ActiveEventLoop,
    dirty: &std::cell::Cell<bool>,
    show_confirm: &std::cell::Cell<bool>,
    confirm_target: &std::cell::Cell<ConfirmTarget>,
    sprite_editor: &mut sprite_editor::SpriteEditor,
) {
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
