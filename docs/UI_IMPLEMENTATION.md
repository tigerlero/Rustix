# Rustix Engine — UI Implementation Plan

## Current State

- egui overlay renders text and panels correctly
- WGSL fragment shader with separate texture + sampler bindings
- Vulkan backend: dynamic rendering, custom pipeline, viewport/scissor management
- Startup screen (Project Hub) with recent projects + native file dialogs
- Editor panels: menu bar, hierarchy, inspector, console, scene view

## Step 1: Fix Font Rendering — DONE ✅

**Goal:** Text renders in egui panels.

**Approach:** Use WGSL with separate image + sampler bindings.

```
Binding 0: texture_2d<f32> → VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE
Binding 1: sampler         → VK_DESCRIPTOR_TYPE_SAMPLER
```

### 1.1 UI fragment shader (WGSL)
```wgsl
@group(0) @binding(0) var uTex: texture_2d<f32>;
@group(0) @binding(1) var uSamp: sampler;
@fragment
fn main(@location(0) uv: vec2<f32>, @location(1) color: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(uTex, uSamp, uv) * color;
}
```

### 1.2 Descriptor set layout
- binding 0: VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE, shaderStage: FRAGMENT
- binding 1: VK_DESCRIPTOR_TYPE_SAMPLER, shaderStage: FRAGMENT

### 1.3 Descriptor pool
- 1x SAMPLED_IMAGE, 1x SAMPLER

### 1.4 Vertex shader (GLSL, compiled at startup via glslang)
- Push constants for screen_size (logical pixels)
- Y-down coordinate convention matching egui and Vulkan framebuffer:
  `gl_Position = vec4(2.0*aPos.x/screen_size.x-1.0, 2.0*aPos.y/screen_size.y-1.0, 0.0, 1.0)`
- Viewport: standard (y:0, height:phys_h), no Y-flip needed
- Scissor: direct mapping from clip_rect (no Y-flip)

### 1.5 Font atlas upload
- Get `ColorImage` from `egui::TexturesDelta` after `end_frame()`
- Convert `Color32` pixels to RGBA8 byte array
- Upload via staging buffer → device-local image (via `renderer.create_texture`)
- Create VkImageView
- Write to descriptor set

### 1.6 Render flow
For each `ClippedPrimitive`:
1. Write vertices/indices to staging buffers
2. Set scissor to clip_rect (framebuffer coordinates, Y-down)
3. Bind pipeline + vertex/index buffers
4. Bind descriptor set (image + sampler)
5. Push constants (screen_size)
6. Draw indexed

---

## Step 2: Build crates/ui/ — Custom UI Framework

**Goal:** Immediate mode game UI (HUD, menus, widgets).

### 2.1 Canvas System
- UI canvases are separate from egui
- Canvases render after the 3D scene, before egui overlay
- Each canvas is a 2D drawing surface with its own coordinate system

### 2.2 Drawing Primitives
- [x] Rect (filled, outline, rounded corners)
- [ ] Text (single-line, multi-line, rich)
- [ ] Image (texture-backed sprites)
- [ ] Line (straight, dashed)

### 2.3 Layout System
- [x] Vertical/horizontal stacking (vstack)
- [x] Center helper
- [x] Cursor-based layout
- [ ] Anchoring (top-left, center, stretch)
- [ ] Box model (margin, padding, border)
- [ ] Grid layout

### 2.4 Widget Library
- [x] Button
- [x] Label (placeholder — colored rect, no glyph rendering)
- [ ] Slider (int, float)
- [ ] Text input
- [ ] Progress bar
- [ ] Panel (scrollable, dockable)
- [ ] Image widget

### 2.5 HUD System (future)
- Health bar, stamina bar, minimap, crosshair, chat, inventory, scoreboard

---

## Step 3: Editor Panels (apps/runtime/src/main.rs)

**Goal:** Editor UI via egui overlay.

### 3.0 Project Hub / Startup Screen — DONE ✅
- Unity-inspired "Project Hub" centered dialog
- Recent projects list (clickable, tracked)
- "New Project" button — opens native folder picker via `rfd`
- "Open Project…" button — opens native folder picker
- Empty state messaging when no recent projects
- Max 10 recent entries with deduplication

### 3.1 Menu Bar — DONE ✅
- File: New Project, Open Project, Back to Project Hub, Exit
- Edit: Preferences (placeholder)
- Assets: Import New Asset (placeholder)
- Help: About Rustix (placeholder)
- FPS counter (right-aligned)

### 3.2 Hierarchy Panel — DONE (layout, no data)
- egui SidePanel left
- "No scene loaded" placeholder
- No ECS integration yet

### 3.3 Inspector Panel — DONE (layout, no data)
- egui SidePanel right
- "No object selected" placeholder
- Shows camera position/distance from editor camera

### 3.4 Scene View — DONE (layout, no 3D content)
- egui CentralPanel
- "Scene View" label placeholder
- Vulkan clear color (dark gray) background
- EditorCamera struct (orbit controls with WASDQE + mouse)

### 3.5 Console / Asset Browser — DONE (layout, no data)
- egui TopBottomPanel bottom
- Tab bar: Console / Asset Browser
- Console shows hardcoded startup messages
- Asset Browser shows placeholder text

### 3.6 Future editor features
- [ ] ECS integration: entity tree in Hierarchy, component editing in Inspector
- [ ] Offscreen 3D scene rendering into Scene View
- [ ] Real log capture via tracing subscriber → Console ring buffer
- [ ] Asset file listing → Asset Browser
- [ ] Entity selection (click in scene or hierarchy)
- [ ] Gizmos (translate, rotate, scale)
- [ ] Undo/redo

---

## Step 4: Editor Application (apps/editor/)

**Goal:** Separate editor binary.

### 4.1 Multiple Viewports
- Split/screen layout, tabbed/dockable panels, layout persistence

### 4.2 Editor Mode
- Play-in-editor, entity selection, gizmos, undo/redo

---

## Dependency Map

```
apps/editor/ ──────┐
apps/runtime/ ─────┤
                   ├──► crates/editor/ ──┬──► crates/ui/ ──┬──► crates/render/
                   │                    │                  └──► crates/core/
                   │                    └──► crates/render/
                   └──► crates/render/
```

---

## Implementation Notes

### Coordinate System
egui and Vulkan framebuffer both use Y-down convention. The vertex shader uses
`2.0*aPos.y/screen_size.y - 1.0` to map directly to Vulkan NDC (also Y-down).
No Y-flip is needed in the viewport or scissor rect.

### File Dialogs
Native OS folder pickers via `rfd` (Rust File Dialog) crate. Works on Linux
(GTK/xdg-desktop-portal), Windows, and macOS. Modal blocking dialogs.

### Recent Projects
Stored in-memory `Vec<ProjectEntry>`. Each entry has name, path, and
last_opened timestamp. Max 10 entries. Deduplication by path.
Persistence (to disk) not yet implemented.
