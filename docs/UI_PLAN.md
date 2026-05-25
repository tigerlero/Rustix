# Rustix Engine — UI & Editor Plan

## Overview

The engine UI system follows a layered approach inspired by Unity Editor, Blender, and Unreal:

- **Layer 1: Immediate Mode Overlay** — `egui` integration for debug panels, editor tools
- **Layer 2: In-Game UI** — Custom immediate mode for HUD, menus, widgets
- **Layer 3: Editor Application** — Separate `apps/editor` binary with full IDE-like layout

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  apps/editor  (future)                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │Scene View│ │Inspector │ │Hierarchy │            │
│  └──────────┘ └──────────┘ └──────────┘            │
│  ┌──────────┐ ┌──────────┐                         │
│  │ Asset    │ │ Console  │                         │
│  │ Browser  │ └──────────┘                         │
│  └──────────┘                                       │
├─────────────────────────────────────────────────────┤
│  crates/ui/  — Immediate mode game UI framework     │
│  crates/editor/ — Editor-only panel implementations │
├─────────────────────────────────────────────────────┤
│  egui integration   — 3rd party immediate mode      │
│  (via egui + egui-wgpu or custom Vulkan backend)    │
├─────────────────────────────────────────────────────┤
│  crates/render/    — GPU resources, pipelines       │
│  crates/core/      — ECS, math, config              │
└─────────────────────────────────────────────────────┘
```

## Phase 0: egui Integration (current step)

**Goal:** Render `egui` UI panels on top of the 3D scene.

### Step 0.1: Add egui dependencies
```toml
egui = "0.30"
egui-winit = "0.30"
egui-wgpu = "0.30"   # or custom Vulkan backend
```

### Step 0.2: egui Render Pass
- Create a dedicated render pass for UI (after 3D scene pass)
- Use egui's tessellated mesh output
- Upload egui vertex/index data to GPU via staging buffers
- Create a UI pipeline (no depth test, alpha blending, scissor)
- Bind egui texture atlas as descriptor

### Step 0.3: egui-Winit Integration
- Feed winit events into egui (mouse, keyboard, scroll)
- egui handles input consumption (UI vs game input)
- Window resize, DPI scale forwarded to egui

### Step 0.4: First UI Panels
- **Performance overlay**: FPS, frame time, draw calls, entity count
- **Docking**: Main menu bar with View → toggle panels
- **Hierarchy panel** (empty): "No entities" placeholder
- **Inspector panel** (empty): "Select an entity" placeholder
- **Console panel**: Show last N log messages

### Step 0.5: Scene View (future phase)
- Render 3D scene to an offscreen texture
- Display the texture in an egui `Image` widget
- Handle mouse events for camera control in scene view
- Entity picking via raycast

## Phase 1: Game UI Framework (crates/ui/)

**Goal:** Custom immediate mode UI for in-game HUD, menus.

- Custom UI canvases (separate from egui)
- HUD elements: health bar, minimap, crosshair
- Menu system: pause menu, settings, inventory
- Text rendering: font atlas, glyph rasterization
- Layout system: flexbox-like anchoring
- Widget library: button, label, slider, text input, image

## Phase 2: Editor Panels (crates/editor/)

**Goal:** Full editor functionality.

### Hierarchy Panel
- Tree view of ECS entities
- Parent-child relationships (Transform hierarchy)
- Right-click context menu (create, delete, duplicate)
- Drag to reorder/reparent

### Inspector Panel
- Display components of selected entity
- Edit component fields in real-time
- Add/remove components
- Vec3/Rot/Scale floats with drag edit
- Color picker

### Scene View
- Flycam/walkcam controls
- Grid floor, world axes
- Entity selection (click to select)
- Gizmo overlay (translate, rotate, scale)
- Play-in-editor mode

### Asset Browser
- File system tree view
- Thumbnail previews
- Drag-and-drop from browser to scene
- Import pipeline integration

### Console
- Real-time log output
- Filter by level (error, warn, info, debug)
- Search/filter
- Command input (REPL)

## Phase 3: Editor Application (apps/editor/)

**Goal:** Separate editor binary with full IDE layout.

- Multiple viewport support (split screen)
- Dockable/tabbed panels
- Project settings
- Build pipeline
- Play/Stop/Pause controls
- Undo/Redo system

## Render Pipeline Integration

### Pass Order
```
1. Depth prepass (optional, future)
2. Opaque geometry pass (3D scene)
3. Transparent geometry pass (future)
4. Gizmo pass (editor)
5. UI pass (egui overlay)
```

### egui Render Pass Details
- Pipeline: no depth test, alpha blending, scissor enabled
- Vertex format: pos[2] + uv[2] + color[4] (egui's default)
- Texture: egui font atlas (single RGBA8 texture)
- Uniform: orthographic projection matrix (screen-space)
- Each frame: egui tessellates → upload mesh → draw indexed with scissor rects

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| egui | 0.30 | Immediate mode GUI framework |
| egui-winit | 0.30 | winit event integration |
| egui-wgpu | 0.30 | GPU render backend (or custom Vulkan) |

Alternatively, use `egui_glow` or a custom Vulkan backend for `egui` to avoid wgpu dependency.
