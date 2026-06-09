use rustix_core::ecs::EcsWorld;
use rustix_platform::input::InputManager;
use rustix_audio::{AudioEngine, SoundInstance};

use std::path::Path;
use crate::project::{AppScreen, ConfirmTarget, ProjectInfo, write_project_file, EditorCameraState, LayoutState, ViewportLayout, DockPosition};
use crate::scene::world_to_scene;
use crate::undo::UndoHistory;
use crate::sprite_editor;
use crate::waveform;

use super::menu_bar;
use super::hierarchy;
use super::inspector;
use super::console;
use super::viewport::{self, ViewportManager};
use super::dialogs;
use super::undo_redo;

#[allow(clippy::too_many_arguments)]
pub fn editor_screen(
    ctx: &egui::Context,
    viewport_manager: &mut ViewportManager,
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
    selected_entities: &std::cell::RefCell<Vec<hecs::Entity>>,
    pending_delete: &std::cell::RefCell<Vec<hecs::Entity>>,
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
    let (hierarchy_dock, inspector_dock, console_dock) = current_project
        .as_ref()
        .and_then(|p| p.layout.as_ref())
        .map(|l| (l.hierarchy_dock, l.inspector_dock, l.console_dock))
        .unwrap_or((DockPosition::Left, DockPosition::Right, DockPosition::Bottom));

    menu_bar::show_menu_bar(ctx, viewport_manager, _input, target, screen, ww, wh, fps, open_project, new_project, project_name, current_project, project_dir, world, dirty, show_confirm, confirm_target, show_settings, sprite_editor, pending_mesh_load, _window);
    let is_playing = *screen == crate::project::AppScreen::PlayTest;
    if !is_playing {
        hierarchy::show_hierarchy(ctx, world, selected_entities, pending_delete, dirty, renaming, rename_buffer, undo_history, hierarchy_dock);
        let cam = viewport_manager.primary_camera_mut();
        inspector::show_inspector(ctx, cam, world, selected_entities, dirty, undo_history, inspector_dock);
        console::show_console(ctx, project_dir, audio_engine, audio_instance, waveform_viewer, console_dock);
    }
    {
        let bookmarks = current_project.as_mut().map(|p| &mut p.bookmarks);
        if let Some(bm) = bookmarks {
            viewport::show_viewports(ctx, viewport_manager, world, selected_entities, dirty, undo_history, bm, is_playing, screen);
        } else {
            let mut dummy = Vec::new();
            viewport::show_viewports(ctx, viewport_manager, world, selected_entities, dirty, undo_history, &mut dummy, is_playing, screen);
        }
    }
    if !is_playing {
        dialogs::show_dialogs(ctx, screen, target, dirty, show_confirm, confirm_target, sprite_editor);
        undo_redo::handle_undo_redo(ctx, world, selected_entities, dirty, undo_history);
    }
    if is_playing && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *screen = crate::project::AppScreen::Editor;
    }

    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
        if let Some(ref mut proj) = current_project {
            proj.settings.resolution_width = *ww;
            proj.settings.resolution_height = *wh;
            proj.scene = world_to_scene(world);
            let primary = viewport_manager.primary_camera();
            proj.editor_camera = Some(EditorCameraState {
                position: primary.position.into(),
                center: primary.center.into(),
                yaw: primary.yaw,
                pitch: primary.pitch,
                distance: primary.distance,
                mode: primary.mode,
                follow_target: primary.follow_target,
            });

            // Capture layout state from egui data
            let hierarchy_width = ctx.data(|d| d.get_temp::<f32>(egui::Id::new("hierarchy_width"))).unwrap_or(220.0);
            let inspector_width = ctx.data(|d| d.get_temp::<f32>(egui::Id::new("inspector_width"))).unwrap_or(260.0);
            let console_height = ctx.data(|d| d.get_temp::<f32>(egui::Id::new("console_height"))).unwrap_or(160.0);
            let mut viewport_layouts = Vec::new();
            for (i, vp) in viewport_manager.viewports.iter().enumerate() {
                let pos = if i > 0 {
                    ctx.data(|d| d.get_temp::<egui::Pos2>(egui::Id::new(format!("viewport_pos_{}", i))))
                        .map(|p| [p.x, p.y])
                } else {
                    None
                };
                let size = if i > 0 {
                    ctx.data(|d| d.get_temp::<egui::Vec2>(egui::Id::new(format!("viewport_size_{}", i))))
                        .map(|s| [s.x, s.y])
                } else {
                    None
                };
                viewport_layouts.push(ViewportLayout {
                    name: vp.name.clone(),
                    open: vp.open,
                    position: pos,
                    size,
                });
            }
            let (h_dock, i_dock, c_dock) = proj.layout.as_ref()
                .map(|l| (l.hierarchy_dock, l.inspector_dock, l.console_dock))
                .unwrap_or((DockPosition::Left, DockPosition::Right, DockPosition::Bottom));
            proj.layout = Some(LayoutState {
                hierarchy_width,
                inspector_width,
                console_height,
                viewports: viewport_layouts,
                hierarchy_dock: h_dock,
                inspector_dock: i_dock,
                console_dock: c_dock,
            });

            if let Some(ref dir) = project_dir {
                let _ = write_project_file(Path::new(dir), proj);
            }
        }
        dirty.set(false);
    }
}
