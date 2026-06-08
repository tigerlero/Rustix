pub mod manager;
pub mod primary;
pub mod secondary;
pub mod gizmo;

pub use manager::{Viewport, ViewportManager, MAX_VIEWPORTS, PRIMARY_VIEWPORT, viewport_texture_id};
pub use primary::show_viewport;
pub use secondary::show_secondary_viewport;

use rustix_core::ecs::EcsWorld;
use crate::camera::EditorCamera;
use crate::project::{AppScreen, CameraBookmark};
use crate::undo::UndoHistory;

/// Show all viewports managed by the ViewportManager.
/// Primary viewport (index 0) uses CentralPanel with full interaction.
/// Secondary viewports use floating egui::Window (view-only for MVP).
pub fn show_viewports(
    ctx: &egui::Context,
    manager: &mut ViewportManager,
    world: &mut EcsWorld,
    selected_entities: &std::cell::RefCell<Vec<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    undo_history: &std::cell::RefCell<UndoHistory>,
    bookmarks: &mut Vec<CameraBookmark>,
    is_playing: bool,
    screen: &mut AppScreen,
) {
    if let Some(vp) = manager.viewports.get_mut(PRIMARY_VIEWPORT) {
        if vp.open {
            show_viewport(ctx, &mut vp.camera, world, selected_entities, dirty, undo_history, bookmarks, is_playing, screen);
        }
    }
    for i in 1..manager.viewports.len() {
        let vp = &mut manager.viewports[i];
        if vp.open {
            show_secondary_viewport(ctx, vp, i);
        }
    }
}

#[cfg(test)]
#[path = "viewport_tests.rs"]
mod tests;
