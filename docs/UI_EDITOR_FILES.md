# Rustix Editor — UI File Reference

## Architecture Overview

The editor UI is implemented in `apps/runtime/src/ui/` (10 files) and runs as an egui
overlay on top of the Vulkan 3D scene. The app has two screens (`AppScreen`):

- **Startup** — A centered "Project Hub" dialog (new/open project)
- **Editor** — Full panel layout when a project is loaded

## Entry Point

### `apps/runtime/src/main.rs`
Creates the window, Vulkan renderer, egui context, event loop. The main loop:
1. Records the 3D scene command buffer
2. Runs egui via `egui_ctx.run()`, dispatching to `startup_screen` or `editor_screen`
3. Uploads egui font atlas textures (`egui_r.update_textures`)
4. Tessellates egui shapes and records draw commands (`egui_r.draw_primitives`)
5. Submits the frame via `renderer.end_frame()`

**Font loading** (lines 56-85): Iterates a hardcoded list of system font paths,
loads fonts into `egui::FontDefinitions`, and inserts them into the Proportional
and Monospace font families.

### `apps/runtime/src/ui_renderer.rs`
Custom Vulkan backend for egui. `EguiVulkanRenderer`:
- Holds the egui pipeline, descriptor set, font texture, vertex/index buffers
- `update_textures()` — Uploads font atlas delta to GPU; creates new texture + updates descriptor set when atlas resizes; defers old texture destruction by 3 frames
- `draw_primitives()` — For each `ClippedPrimitive::Mesh`, writes vertex/index data, sets scissor rect, binds pipeline/descriptors, draws indexed
- Uses dynamic rendering (no VkRenderPass), WGSL fragment shader with separate texture + sampler bindings

## Screen: Startup

### `apps/runtime/src/ui/startup.rs`
`startup_screen()` — Single `CentralPanel` with:
- "Rustix Engine Project Hub" heading
- Left column: recent projects list (clickable, with name + path + timestamp)
- Right column: New Project / Open Project buttons
- Modal dialog for project type selection (2D / 3D)
- Uses only ASCII text — unaffected by font coverage issues

## Screen: Editor

### `apps/runtime/src/ui/mod.rs`
Module re-exports. Makes `startup_screen` and `editor_screen` public.

### `apps/runtime/src/ui/editor.rs`
`editor_screen()` — Orchestrator that calls all panel functions:
1. `menu_bar::show_menu_bar()` — top panel
2. `hierarchy::show_hierarchy()` — left panel
3. `inspector::show_inspector()` — right panel
4. `console::show_console()` — bottom panel
5. `viewport::show_viewport()` — central panel
6. `dialogs::show_dialogs()` — overlay windows
7. `undo_redo::handle_undo_redo()` — keyboard shortcuts

### `apps/runtime/src/ui/menu_bar.rs`
`egui::Panel::top` with `egui::MenuBar`:
- File: New Project, Open Project…, Save, Save As…, Load GLB…, Project Settings…, Back to Project Hub, Exit
- Edit: Preferences (placeholder)
- Assets: Import New Asset…, Sprite Editor
- Help: About Rustix (placeholder)
- Right side: FPS, Orbit/1stP/Follow camera toggles, project type label
- Uses: `\u{2014}` (em dash), `\u{2026}` (ellipsis)

### `apps/runtime/src/ui/hierarchy.rs`
`egui::Panel::left` — Entity tree:
- Lists all entities from the ECS world
- Indented hierarchy with `\u{2514}` (└) prefix for children
- Click to select, double-click to rename, right-click context menu
- "Add Entity" button, "Create Light" submenu (Directional/Point/Spot)
- Delete entity processing with undo history capture

### `apps/runtime/src/ui/inspector.rs`
`egui::Panel::right` — Component editor:
- Shows selected entity's name, transform (position/rotation/scale as DragValues)
- Light components: Directional, Point, Spot (color, intensity, radius, angles)
- Material: base color, roughness, metallic
- Audio: Source (min/max distance, rolloff), Listener status
- Camera info: mode (Orbit/FirstPerson), center/position, distance, yaw/pitch
- Writes changes back to ECS world with undo history

### `apps/runtime/src/ui/console.rs`
`egui::Panel::bottom` — Two tabs:
- **Console**: Scrollable log output from `tracing` subscriber, color-coded by level
- **Asset Browser**: File listing of project directory with icons, audio preview (waveform + play/stop), file type tags
- Uses: `\u{25b6}` (play triangle), `\u{23f9}` (stop square)

### `apps/runtime/src/ui/viewport.rs`
`egui::CentralPanel` — 3D scene overlay:
- Projects 3D entity positions to screen-space via camera view-projection matrix
- Renders grid (XZ plane with major/minor lines)
- Entity dots with name labels
- Audio source distance visualization (circles with labels)
- Gizmo handles (translate/rotate/scale) with drag interaction
- HUD text: mode label + entity count
- Keyboard shortcuts: W/E/R for gizmo mode, F to focus, Home to reset camera
- Uses: `\u{1f50a}` (speaker emoji for audio sources)

### `apps/runtime/src/ui/dialogs.rs`
Overlay windows:
- **Project Settings**: Resolution, V-Sync, target FPS, project type (2D/3D)
- **Unsaved Changes**: Confirm dialog for discarding changes
- Delegates to `sprite_editor.show()` for the sprite editor window

### `apps/runtime/src/ui/undo_redo.rs`
Keyboard shortcut handlers (Ctrl+Z undo, Ctrl+Shift+Z redo):
- AddEntity: despawns on undo
- DeleteEntity: respawns with saved data on undo
- RenameEntity: restores old name
- TransformEntity: restores old transform

## Supporting Files

### `apps/runtime/src/project.rs`
Data types and file I/O for project management:
- `ProjectInfo`, `ProjectSettings`, `ProjectEntry`, `SceneData`
- `create_project_file()`, `load_project_file()`, `write_project_file()`
- Recent projects persistence (JSON in config dir)

### `apps/runtime/src/scene.rs`
ECS component types and scene serialization:
- `Transform`, `Name`, `MeshComponent`, `Material`, `Parent`
- `world_transform()` — computes world-space matrix from parent hierarchy
- `world_to_scene()` / `scene_to_world()` — serialize/deserialize ECS world

### `apps/runtime/src/undo.rs`
`UndoHistory` — Bounded action stack with undo/redo index:
- `EditorAction` enum: AddEntity, DeleteEntity, RenameEntity, TransformEntity

### `apps/runtime/src/sprite_editor.rs`
Pixel-art sprite editor window:
- Draw modes: Fill, Outline, Rect, Circle
- Color pickers for fill/outline
- Brush/outline size sliders
- 256x256 pixel grid with mouse painting

### `apps/runtime/src/waveform.rs`
Audio waveform visualization widget:
- Draws interleaved f32 samples as amplitude bars
- Stereo channel coloring, scroll, zoom
- Playhead cursor with triangle marker
- Time markers with labels

### `apps/runtime/src/camera.rs`
`EditorCamera` — Orbit/first-person camera:
- Mouse orbit, WASD movement, distance scroll
- Follow mode tracks selected entity

## Font Loading & Text Rendering

Font setup via `setup_fonts()` in `main.rs:30-78`:

Three fonts are bundled in `assets/fonts/` and compiled into the binary
via `include_bytes!`:

| File | Source | Family role |
|------|--------|-------------|
| `assets/fonts/NotoSans-Regular.ttf` | Noto Sans | Primary proportional |
| `assets/fonts/NotoSansMono-Regular.ttf` | Noto Sans Mono | Primary monospace |
| `assets/fonts/NotoEmoji-Regular.ttf` | Noto Sans Symbols 2 | Fallback for symbols/emoji |

Fallback chain:
```
Proportional: noto_sans → noto_emoji
Monospace:    noto_mono → noto_emoji
```

When a glyph is missing in Noto Sans, egui searches the next font in the
family, eventually reaching `noto_emoji` which covers all editor UI symbols:

| Character | Code Point | Used In |
|-----------|-----------|---------|
| └ | U+2514 | hierarchy.rs (tree connector) |
| — | U+2014 | menu_bar.rs (separator) |
| … | U+2026 | menu_bar.rs (ellipsis) |
| ▶ | U+25B6 | console.rs (play button) |
| ⏹ | U+23F9 | console.rs (stop button) |
| 🔊 | U+1F50A | viewport.rs (audio speaker icon) |
| → | U+2192 | (general arrow) |

The egui built-in fallback fonts (Ubuntu-Light, Hack) are also retained
from `FontDefinitions::default()` as an additional safety net.
