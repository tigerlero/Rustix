# Font/Text Rendering Investigation

## Architecture Overview

The app uses **egui 0.34** for all editor UI text rendering. egui renders via a custom Vulkan backend (`EguiVulkanRenderer`). There is no CSS — all styling is in Rust code.

---

## Font Assets

| File | Purpose |
|---|---|
| `assets/fonts/NotoSans-Regular.ttf` | Primary proportional UI font |
| `assets/fonts/NotoSansMono-Regular.ttf` | Monospace font |
| `assets/fonts/NotoEmoji-Regular.ttf` | Emoji/symbol fallback |

---

## Font Initialization (Single Point)

**File:** `apps/runtime/src/main.rs:41-73`

```
setup_fonts(&egui_ctx)  // called ONCE at line 101, before event loop
```

- Embeds all 3 fonts via `include_bytes!` at compile time
- Inserts into `egui::FontDefinitions` as `"noto_sans"`, `"noto_mono"`, `"noto_emoji"`
- Configures Proportional family: `noto_sans` → `noto_emoji`
- Configures Monospace family: `noto_mono` → `noto_emoji`
- Calls `ctx.set_fonts(fonts)` to apply

**Key finding: Fonts are configured exactly once. They are never re-configured or re-applied when switching screens.**

---

## Font Atlas GPU Texture Pipeline

**File:** `apps/runtime/src/ui_renderer.rs`

### Creation (line 26-144)
- Starts with a 1x1 white placeholder texture (`font_texture_size = (1,1)`)
- Creates Vulkan pipeline: vertex shader (GLSL) + fragment shader (WGSL)
- Fragment shader: `textureSample(uTex, uSamp, uv) * color` — samples font atlas, multiplies by vertex color
- Descriptor set binds the font atlas texture + sampler

### Texture Updates (line 147-195)
- `update_textures()` is called every frame from `main.rs:542`
- Extracts `egui::ImageData::Color` pixel data, converts to RGBA bytes
- If same size as existing texture: updates pixels in-place
- If different size: creates new texture, defers old texture destruction (up to 3 kept for triple-buffering), updates descriptor set
- Logs: `--- update_textures: {free} free, {set} set ---`

### Drawing (line 197-258)
- Called from `main.rs:545` after tessellation
- Applies `pixels_per_point` DPI scaling to clip rectangles
- Uploads vertex/index data to GPU, issues draw calls

---

## Main Loop Flow (Per Frame)

**File:** `apps/runtime/src/main.rs:494-547`

```
raw_input = egui_state.take_egui_input(window)
out = egui_ctx.run(raw_input, |ctx| {
    match screen {
        Startup => startup_screen(ctx, ...),
        Editor  => editor_screen(ctx, ...),
    }
})
if open_project.is_some() { screen = AppScreen::Editor }  // line 516-528
egui_r.update_textures(&renderer, &out.textures_delta)    // line 542
clipped = egui_ctx.tessellate(out.shapes, out.pixels_per_point)  // line 543
egui_r.draw_primitives(cmd, &renderer, &clipped, out.pixels_per_point)  // line 545
```

**Key finding: The screen transition happens AFTER `egui_ctx.run()` but BEFORE `update_textures` and `draw_primitives`. This means the first frame after opening a project has Startup's UI baked into the tessellation, but the next frame will render the Editor UI.**

---

## Screen Transition on Project Open

**File:** `apps/runtime/src/main.rs:516-528`

```rust
if let Some(path) = open_project.borrow_mut().take() {
    // ... load project ...
    screen = AppScreen::Editor;  // switch happens here
}
```

This runs AFTER `egui_ctx.run()` completes. The transition is a simple enum swap. No font re-initialization, no texture invalidation, no style changes.

---

## Text Rendering in Startup Screen (Works)

**File:** `apps/runtime/src/ui/startup.rs`

| Line | Text | Font/Size |
|---|---|---|
| 40 | "Rustix" | `RichText::new(...).size(28.0).strong()` |
| 42 | "Engine" | `RichText::new(...).size(14.0)` |
| 44 | "Project Hub" | `RichText::new(...).size(12.0)` |
| 57 | "RECENT" | `RichText::new(...).size(10.0)` |
| 68-71 | Empty state text | 12.0 / 11.0 |
| 81 | Project name | `FontId::proportional(13.0)` |
| 82 | Project path | `FontId::proportional(10.0)` |
| 91-104 | `ui.painter().text(...)` | Direct painter calls with FontId |
| 123 | "New Project" button | `RichText::new(...).size(14.0)` |
| 135 | "Open Project..." button | `RichText::new(...).size(14.0)` |
| 172-184 | Dialog buttons | `RichText::new(...).size(16.0)` |

Color palette: `text_primary = rgb(220,220,228)`, `text_secondary = rgb(140,140,155)`, `accent = rgb(72,120,240)`

---

## Text Rendering in Editor Screen (Broken?)

**File:** `apps/runtime/src/ui/editor.rs` → calls all sub-panels

### Menu Bar (`ui/menu_bar.rs`)
- Line 37: `RichText::new(project_name).strong()`
- Line 38: `RichText::new("— Rustix Editor").weak()`
- Line 146: Project type badge (colored)
- Line 160: `RichText::new("Cam:").weak()`

### Hierarchy (`ui/hierarchy.rs`)
- Line 22: `ui.heading("Hierarchy")`
- Line 83: `RichText::new("└").weak()` — box-drawing character (U+2514)
- Line 87: `RichText::new(&name).color(Color32::WHITE)` — entity names

### Inspector (`ui/inspector.rs`)
- Line 45: `ui.heading("Inspector")`
- Line 48: `RichText::new(name).strong()`
- Line 160: `RichText::new("No object selected").italics()`

### Viewport (`ui/viewport.rs`)
- Line 105: `FontId::proportional(11.0)` — entity name labels
- Line 140: `FontId::proportional(10.0)` — audio max distance
- Line 149: `FontId::proportional(10.0)` — audio min distance
- Line 158: `FontId::proportional(12.0)` — audio emoji icon (🔊)
- Line 297: `FontId::proportional(11.0)` — HUD mode label

### Console (`ui/console.rs`)
- Line 43: `RichText` for log entries (colored by level)
- Line 79: `RichText::new(...).size(12.0)` — asset browser files

---

## Potential Root Causes for Fonts Breaking After Project Open

### 1. Font Atlas Texture Stale/Corrupted After Screen Switch
- The font atlas is a single GPU texture managed by `EguiVulkanRenderer`
- egui lazily rasterizes glyphs into the atlas as needed
- When switching from Startup→Editor, completely different text is rendered
- If egui needs to expand the atlas for new glyphs (e.g., hierarchy panel glyphs), it triggers a texture resize
- The resize path (`ui_renderer.rs:169-189`) creates a new texture, defers old one, and updates the descriptor set
- **If the descriptor set update fails or the new texture isn't properly initialized, all text would render as the 1x1 white placeholder**

### 2. `pixels_per_point` DPI Scaling Mismatch
- `pixels_per_point` comes from egui's output (`out.pixels_per_point`)
- It's used in both `tessellate()` and `draw_primitives()`
- In `draw_primitives()` (`ui_renderer.rs:203-204`): `logical_w = phys_w / pixels_per_point`
- If `pixels_per_point` is incorrect (e.g., 0.0 or very large), the push constant `screen_size` would be wrong, causing the vertex shader to miscalculate positions
- **This could make text appear at wrong positions or be invisible (clipped)**

### 3. Tessellation Happens With Wrong UI State
- The screen switch (`screen = AppScreen::Editor`) happens at line 526, AFTER `egui_ctx.run()` at line 503
- So the first frame after opening: tessellation contains Startup screen shapes
- The SECOND frame renders Editor screen correctly
- But the font atlas upload (`update_textures`) happens after tessellation
- **If the font atlas needs to grow for Editor's new glyphs, the growth happens on frame N+1, but tessellation on frame N already assumed those glyphs exist**

### 4. Clip Rect Overflow From Different Panel Layouts
- Editor has multiple panels (menu bar, hierarchy, inspector, console, viewport)
- Each panel constrains egui's available area
- Clip rects in `draw_primitives()` are scaled by `pixels_per_point`
- If panel layout causes clip rects to be very small or zero-sized, text primitives get clipped
- **The `max(0.0) as u32` in `ui_renderer.rs:239-240` would produce 0-extent scissors, making text invisible**

### 5. Missing `request_repaint()` After Screen Transition
- egui optimizes by not repainting if no input events
- After switching screens, the UI changes dramatically
- If egui doesn't know it needs to repaint, the old frame persists
- **The Editor panels might not trigger enough input events to force a full repaint**

---

## All Files Containing Font/Text Code

| File | Lines | Category |
|---|---|---|
| `assets/fonts/NotoSans-Regular.ttf` | — | Font asset |
| `assets/fonts/NotoSansMono-Regular.ttf` | — | Font asset |
| `assets/fonts/NotoEmoji-Regular.ttf` | — | Font asset |
| `apps/runtime/src/main.rs` | 30-74, 100-103, 503-547 | Font config, init, atlas upload |
| `apps/runtime/src/ui_renderer.rs` | 6-258 | Font atlas GPU texture, Vulkan pipeline |
| `apps/runtime/src/ui/startup.rs` | 12-18, 40-44, 57, 68-71, 81-104, 117, 123, 135, 151-154, 172, 184 | Text sizing/coloring |
| `apps/runtime/src/ui/viewport.rs` | 101-107, 136-160, 293-298 | FontId for labels/HUD |
| `apps/runtime/src/ui/console.rs` | 36-43, 79, 87, 97, 129 | RichText for logs/assets |
| `apps/runtime/src/ui/menu_bar.rs` | 37-38, 146, 160 | RichText for menu labels |
| `apps/runtime/src/ui/hierarchy.rs` | 22, 69, 83, 87-88 | Heading, tree connector, names |
| `apps/runtime/src/ui/inspector.rs` | 45, 48, 160 | Heading, entity name, empty state |
| `apps/runtime/src/waveform.rs` | 101-107 | FontId for time markers |
| `Cargo.toml` | 56-57 | egui dependency |
| `apps/runtime/Cargo.toml` | 15-16 | egui workspace dependency |

---

## No Theme/Style Configuration

There is **no** `egui::Style` or `egui::Visuals` customization anywhere. The app uses egui's default dark theme entirely. All visual theming is done ad-hoc through `RichText` color/size modifiers on individual widgets.

---

## Sprite Editor Text

**File:** `apps/runtime/src/sprite_editor.rs`

- Line 60: `format!("Sprite: {}x{}", ...)` — size label
- Line 63: `ui.label("Draw Mode:")`
- Line 66-69: Selectable labels: "Fill", "Outline", "Rect", "Circle"
- Line 73: `ui.label("Fill Color:")`
- Line 82: `ui.label("Outline Color:")`
- Line 91: `ui.label("Brush/Outline Size:")`
- Line 97-105: Buttons: "Clear", "Fill", "Checkerboard"
- Line 109: `ui.label("Pixel Editor:")`

All text uses egui defaults (no explicit FontId or RichText sizing).

---

## crates/ui/src/lib.rs — Unused Placeholder UI

**File:** `crates/ui/src/lib.rs:192-197`

```rust
pub fn label(ctx, text, pos, font_size, color) {
    let w = text.len() as f32 * font_size * 0.5;
    let h = font_size * 1.3;
    ctx.rect(pos, pos + Vec2::new(w, h), color);  // draws colored rect, NOT text
}
```

This is an early/alternative UI system. It does NOT render real text — it draws colored rectangles as placeholders. **Not used by the runtime editor.**

---

## Summary

The font setup is called once at startup and never re-applied. The font atlas is a single GPU texture that grows dynamically. The screen transition is a simple enum swap with no font re-initialization. The most likely causes of fonts breaking after project open are:

1. **Font atlas texture corruption/growth failure** during the screen switch
2. **DPI scaling (`pixels_per_point`) causing wrong screen_size push constant**
3. **Tessellation/rendering pipeline ordering issue** with the screen transition happening after `egui_ctx.run()`
4. **Clip rect/scissor issues** from the multi-panel editor layout

---

## Complete Text Rendering Locations

| File | Lines | Text Content |
|---|---|---|
| `ui/startup.rs` | 40 | "Rustix" (28pt bold) |
| `ui/startup.rs` | 42 | "Engine" (14pt) |
| `ui/startup.rs` | 44 | "Project Hub" (12pt) |
| `ui/startup.rs` | 57 | "RECENT" (10pt) |
| `ui/startup.rs` | 68-71 | Empty state text (11-12pt) |
| `ui/startup.rs` | 81-104 | Recent project list (FontId 10-13pt) |
| `ui/startup.rs` | 117 | "GET STARTED" (10pt) |
| `ui/startup.rs` | 123 | "New Project" (14pt) |
| `ui/startup.rs` | 135 | "Open Project..." (14pt) |
| `ui/startup.rs` | 151-154 | Help text (11pt) |
| `ui/startup.rs` | 168 | "Choose project type:" (default) |
| `ui/startup.rs` | 172, 184 | "3D Project" / "2D Project" (16pt) |
| `ui/startup.rs` | 197 | "Cancel" (default) |
| `ui/menu_bar.rs` | 37 | Project name (strong) |
| `ui/menu_bar.rs` | 38 | "— Rustix Editor" (weak) |
| `ui/menu_bar.rs` | 42-138 | Menu items (default sizing) |
| `ui/menu_bar.rs` | 146 | Project type badge (colored) |
| `ui/menu_bar.rs` | 148 | `format!("FPS: {fps}")` |
| `ui/menu_bar.rs` | 160 | "Cam:" (weak) |
| `ui/hierarchy.rs` | 22 | "Hierarchy" heading |
| `ui/hierarchy.rs` | 69 | Rename text input (WHITE) |
| `ui/hierarchy.rs` | 83 | "└" box-drawing (weak) |
| `ui/hierarchy.rs` | 87 | Entity names (WHITE) |
| `ui/hierarchy.rs` | 182-210 | "Add Entity", "Create Light" menu |
| `ui/inspector.rs` | 45 | "Inspector" heading |
| `ui/inspector.rs` | 48 | Entity name (strong) |
| `ui/inspector.rs` | 50-156 | Property labels (default) |
| `ui/inspector.rs` | 158 | "Select an object..." (default) |
| `ui/inspector.rs` | 160 | "No object selected" (italics) |
| `ui/inspector.rs` | 164-177 | Camera info labels (default) |
| `ui/console.rs` | 18-19 | Tab labels: "Console", "Asset Browser" |
| `ui/console.rs` | 43 | Log entries (colored by level) |
| `ui/console.rs` | 79 | Asset files (12pt, colored by type) |
| `ui/console.rs` | 87 | "Audio Preview" (strong) |
| `ui/console.rs` | 90, 97, 104 | Audio metadata (weak) |
| `ui/console.rs` | 117, 123 | "▶ Play" / "⏹ Stop" |
| `ui/console.rs` | 129 | "Audio engine not available" (weak) |
| `ui/console.rs` | 134 | "Open a project..." |
| `ui/viewport.rs` | 105 | Entity name labels (FontId 11pt) |
| `ui/viewport.rs` | 140, 149 | Audio distance labels (FontId 10pt) |
| `ui/viewport.rs` | 158 | Audio icon "🔊" (FontId 12pt) |
| `ui/viewport.rs` | 292-298 | HUD mode + entity count (FontId 11pt) |
| `ui/dialogs.rs` | 25-53 | Project settings labels (default) |
| `ui/dialogs.rs` | 67-83 | Unsaved changes dialog |
| `sprite_editor.rs` | 60-109 | Editor labels (default) |
| `waveform.rs` | 101-107 | Time marker labels (FontId 10pt) |
