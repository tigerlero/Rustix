# Rustix Engine — UI & Editor Plan

## Overview

The engine UI system follows a layered approach. Unlike the original plan (separate `apps/editor` binary), the editor is embedded in `apps/runtime` and toggled via screen state (`AppScreen::ProjectHub` ↔ `AppScreen::Editor`). This keeps the engine and editor in the same process for fast iteration.

## Current Architecture

```
┌─────────────────────────────────────────────────────┐
│  apps/runtime  (editor + game runtime)                │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │Scene View│ │Inspector │ │Hierarchy │            │
│  │(central) │ │(right)   │ │(left)    │            │
│  └──────────┘ └──────────┘ └──────────┘            │
│  ┌──────────────────┐ ┌──────────┐                 │
│  │ Console / Asset  │ │ Menu Bar │                 │
│  │ Browser (bottom) │ │ (top)    │                 │
│  └──────────────────┘ └──────────┘                 │
├─────────────────────────────────────────────────────┤
│  crates/ui/  — Custom immediate mode game UI        │
│              (stub: button, slider, label, layout)  │
├─────────────────────────────────────────────────────┤
│  egui integration — Custom Vulkan backend           │
│  (EguiVulkanRenderer in apps/runtime/src/ui_renderer)│
├─────────────────────────────────────────────────────┤
│  crates/render/    — GPU resources, pipelines       │
│  crates/core/      — ECS, math, config              │
└─────────────────────────────────────────────────────┘
```

## Phase 0: egui Integration ✅ DONE

**Status:** Fully implemented. Custom Vulkan backend with WGSL fragment shader.

### Implemented
- [x] Custom `EguiVulkanRenderer` — separate texture + sampler bindings, WGSL fragment shader
- [x] Font atlas upload + per-frame texture updates
- [x] Clipped primitive rendering with scissor rects
- [x] Push constants for screen size
- [x] Y-down coordinate system correction (egui → Vulkan NDC)
- [x] Triple-buffered vertex/index buffers (4MB × 3, per-frame slotting)
- [x] winit event feeding (mouse, keyboard, scroll, text input)
- [x] Window resize + DPI scale forwarding

### Files
- `apps/runtime/src/ui_renderer.rs` — `EguiVulkanRenderer` (creation + render)
- `apps/runtime/src/main.rs` — event loop integration, `egui::Context` management
- `apps/runtime/src/init.rs` — `EguiVulkanRenderer::new()` call

---

## Phase 1: Project Hub ✅ DONE

**Status:** Fully implemented. First screen on startup.

### Implemented
- [x] Centered dialog with "Rustix Engine / Project Hub" branding
- [x] Recent projects list with hover interaction + path display
- [x] "New Project" button → native folder picker (`rfd::FileDialog`)
- [x] "Open Project…" button → native folder picker
- [x] Recent project tracking (in-memory `Vec`, max 10, dedup by path)
- [x] Empty state: "No recent projects" message
- [x] `.rustixproj` serialization (TOML: settings + scene + camera state)
- [x] Recent projects persistence to disk (`recent_projects.json`)

### Files
- `apps/runtime/src/ui/project_hub.rs` — `show_project_hub()`
- `apps/runtime/src/project.rs` — `ProjectInfo`, `write_project_file()`, `read_project_file()`

---

## Phase 2: Editor Layout ✅ DONE

**Status:** All panels functional, not placeholders.

### 2.1 Menu Bar
- [x] Project name + dirty indicator (`*`)
- [x] **File**: New/Open Project, Save (`Ctrl+S`), Exit, Back to Project Hub
- [x] **Edit**: Undo/Redo (`Ctrl+Z` / `Ctrl+Y`)
- [x] **Assets**: mesh loader file picker, sprite editor toggle
- [x] **Help**: About
- [x] FPS counter
- [x] **Settings** button → Project Settings modal

### 2.2 Hierarchy Panel (left, 220px, resizable)
- [x] Live ECS entity tree from `hecs::World` query
- [x] Component-type icons: mesh, light, camera, audio, physics
- [x] Toolbar: Add Entity, Delete, Copy, Paste, Duplicate
- [x] In-place rename (`F2` or double-click)
- [x] Click to select; selected entity highlighted
- [x] Delete confirmation

### 2.3 Inspector Panel (right, resizable)
- [x] **Transform**: position, rotation, scale (drag values)
- [x] **Material**: albedo via custom HSVA color picker + RGB inputs, metallic, roughness
- [x] **Lights**: color, intensity, range, spot angle
- [x] **Camera**: FOV, near/far planes
- [x] **Audio**: volume, loop, pitch, spatial toggle
- [x] **Physics**: mass, body type (dropdown), damping; collider shape (box/sphere/capsule) + size
- [x] **Script**: script path string
- [x] **Parent**: shows parent entity if any
- [x] All edits push `EditorAction` to `UndoHistory`

### 2.4 Console / Asset Browser (bottom, 160px, resizable, tabbed)
- [x] **Console**: real-time `tracing` log capture via `rustix_core::log_capture`. Color-coded levels (error=red, warn=yellow, info=blue-white, debug=gray, trace=dark). Auto-scroll to bottom, Clear button.
- [x] **Asset Browser**: filesystem listing of project directory with file icons, Refresh button.

### 2.5 Scene View (central panel)
- [x] Transparent `egui::Frame` for offscreen rendering
- [x] Displays offscreen texture via `ui.painter().image(tex_id, ...)` when available
- [x] Viewport rect tracked per-frame (`viewport_rect_0`) for framebuffer sizing
- [x] World-to-screen projection helper for overlay drawing

### 2.6 EditorCamera
- [x] **Orbit mode**: WASDQE (shift), right-drag orbit, middle-drag pan, scroll zoom
- [x] **First-person mode**: right-drag look, WASDQE move
- [x] `Space` toggles mode
- [x] Yaw/pitch clamped (-1.4 to 1.4 rad)
- [x] Distance min 0.5
- [x] Camera state serialized into `.rustixproj`

### Files
- `apps/runtime/src/ui/editor.rs` — `editor_screen()` orchestrates all panels
- `apps/runtime/src/ui/menu_bar.rs` — `show_menu_bar()`
- `apps/runtime/src/ui/hierarchy.rs` — `show_hierarchy()`
- `apps/runtime/src/ui/inspector.rs` — `show_inspector()` + custom color picker
- `apps/runtime/src/ui/console.rs` — `show_console()`
- `apps/runtime/src/ui/viewport.rs` — `show_viewport()`, `show_viewports()`, gizmos, grid
- `apps/runtime/src/ui/dialogs.rs` — Project Settings, confirmation dialogs
- `apps/runtime/src/ui/undo_redo.rs` — `handle_undo_redo()`
- `apps/runtime/src/camera.rs` — `EditorCamera` (orbit + first-person)
- `apps/runtime/src/undo.rs` — `UndoHistory`, `EditorAction` enum

---

## Phase 3: Editor Advanced Features ✅ PARTIAL

### Implemented
- [x] **Gizmos**: translate/rotate/scale toolbar (E/R/T buttons). Local/world space toggle. Snap toggle + step size. Visual axes drawn in viewport. Real-time transform update with undo.
- [x] **Grid overlay**: XZ plane, major/minor lines, world-to-screen projected, toggleable.
- [x] **Undo/redo**: full `UndoHistory` with `AddEntity`, `DeleteEntity`, `TransformChange`, `ComponentChange`, `Rename` actions. Snapshots before/after for revert.
- [x] **Viewport splitting**: `ViewportManager` supports up to 4 viewports. Primary uses `CentralPanel`; secondary use floating `egui::Window`. Independent cameras.
- [x] **Project Settings modal**: resolution, VSync, target FPS, 2D/3D mode.
- [x] **Sprite editor**: integrated window with animation timeline.
- [x] **Audio preview**: play/stop, waveform visualization, volume slider.
- [x] **Confirmation dialogs**: unsaved changes warning on project switch/close.

### Planned
- [ ] **Layout persistence** — save panel sizes, positions, viewport arrangement per-project
- [ ] **Docking** — drag panels to rearrange (requires `egui_dock` or custom)
- [ ] **Full offscreen scene rendering** — needs render target / framebuffer implementation
- [ ] **Entity multi-select** — shift/ctrl click for group operations
- [ ] **Hierarchy drag-and-drop** — reparent entities by dragging
- [ ] **Scene camera bookmarks** — save preset views
- [ ] **Play mode** — simulate game inside editor viewport

---

## Phase 4: Game UI Framework (crates/ui/) — STUB

**Status:** Minimal implementation. Not used by editor (editor uses egui).

### Implemented
- [x] Immediate mode UI context
- [x] Draw command list (colored rectangles)
- [x] Button widget (hover/interaction state)
- [x] Slider widget
- [x] Label widget (colored rect placeholder, no real glyphs)
- [x] Layout helpers: `vstack`, `center`

### Planned
- [ ] Real text rendering (glyph atlas, SDF or rasterized)
- [ ] Image widget
- [ ] Text input widget
- [ ] Flexbox/grid layout engine
- [ ] In-game HUD system (health bars, minimap, crosshair)
- [ ] Menu system (pause, settings, inventory)

**Rationale:** `crates/ui` is a long-term project for in-game UI. The editor uses egui exclusively. When in-game UI is needed, `crates/ui` will become a priority.

---

## Render Pipeline Integration

### Current Pass Order
```
1. Shadow map pass (if enabled)
2. Opaque geometry pass (3D scene)
3. Transparent geometry pass (future)
4. Offscreen scene pass (editor viewport texture — partial)
5. UI pass (egui overlay)
```

### egui Render Pass Details
- **Pipeline**: no depth test, alpha blending (`SrcAlpha` / `OneMinusSrcAlpha`), scissor enabled
- **Vertex format**: pos[2] + uv[2] + color[4] (egui default, matches `egui::epaint::Vertex`)
- **Textures**: font atlas + dynamic user textures (per-egui-texture-id descriptor sets)
- **Uniform**: push constant `vec2 screen_size` (not a UBO — 8 bytes via push constants)
- **Descriptor layout**: binding 0 = sampled image, binding 1 = sampler (separate descriptors for Vulkan 1.2+)
- **Per frame**: egui tessellates → write vertex/index data to CPU-mapped buffer → draw indexed with scissor rects

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `egui` | 0.31 | Immediate mode GUI framework |
| `egui-winit` | 0.31 | winit event integration |
| `rfd` | 0.15 | Native file dialogs (cross-platform) |

**Note:** No `egui-wgpu` dependency. The engine uses a **custom Vulkan backend** (`EguiVulkanRenderer`) that interfaces directly with `ash` + the engine's bindless descriptor system. This avoids a wgpu dependency and gives full control over descriptor management.

---

## Evolution Notes

### 2026-06-03: Boxed Device Fix
`EguiVulkanRenderer::new()` calls `renderer.create_texture()` which uses `device.sampler_cache().get_or_create()`. The `SamplerCache` stores `*const ash::Device`. When `GpuDevice` stored `ash::Device` by value, the cached pointer became dangling after struct moves. Fixed by boxing the device (`Box<ash::Device>`) for stable heap address.

See `docs/CRASH_LOG.md` for full incident report.
