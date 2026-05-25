use rustix_core::ecs::EcsWorld;
use rustix_platform::input::InputManager;
use rustix_audio::{AudioEngine, SoundInstance};

use crate::camera::EditorCamera;
use crate::project::{AppScreen, ConfirmTarget, ProjectInfo};
use crate::undo::UndoHistory;
use crate::sprite_editor;
use crate::waveform;

use super::menu_bar;
use super::hierarchy;
use super::inspector;
use super::console;
use super::viewport;
use super::dialogs;
use super::undo_redo;

#[allow(clippy::too_many_arguments)]
pub fn editor_screen(
    ctx: &egui::Context,
    cam: &mut EditorCamera,
    _window: &mut rustix_platform::window::WindowHandle,
    screen: &mut AppScreen,
    _input: &InputManager,
    target: &winit::event_loop::ActiveEventLoop,
    ww: &u32,
    wh: &u32,
    fps: &u64,
    open_project: &std::cell::RefCell<Option<String>>,
    new_project: &std::cell::RefCell<Option<String>>,
    project_name: &str,
    current_project: &mut Option<ProjectInfo>,
    project_dir: &mut Option<String>,
    world: &mut EcsWorld,
    selected_entity: &std::cell::RefCell<Option<hecs::Entity>>,
    pending_delete: &std::cell::RefCell<Option<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    show_confirm: &std::cell::Cell<bool>,
    confirm_target: &std::cell::Cell<ConfirmTarget>,
    show_settings: &std::cell::Cell<bool>,
    renaming: &std::cell::RefCell<Option<hecs::Entity>>,
    rename_buffer: &std::cell::RefCell<String>,
    undo_history: &std::cell::RefCell<UndoHistory>,
    sprite_editor: &mut sprite_editor::SpriteEditor,
    pending_mesh_load: &std::cell::RefCell<Option<String>>,
    audio_engine: &mut Option<AudioEngine>,
    audio_instance: &mut Option<SoundInstance>,
    waveform_viewer: &mut waveform::WaveformViewer,
) {
    menu_bar::show_menu_bar(ctx, cam, _input, target, screen, ww, wh, fps, open_project, new_project, project_name, current_project, project_dir, world, dirty, show_confirm, confirm_target, show_settings, sprite_editor, pending_mesh_load);
    hierarchy::show_hierarchy(ctx, world, selected_entity, pending_delete, dirty, renaming, rename_buffer, undo_history);
    inspector::show_inspector(ctx, cam, world, selected_entity, dirty, undo_history);
    console::show_console(ctx, project_dir, audio_engine, audio_instance, waveform_viewer);
    viewport::show_viewport(ctx, cam, world, selected_entity, dirty);
    dialogs::show_dialogs(ctx, screen, target, current_project, dirty, show_confirm, confirm_target, show_settings, sprite_editor);
    undo_redo::handle_undo_redo(ctx, world, selected_entity, dirty, undo_history);
}
