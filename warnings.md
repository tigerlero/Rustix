# Rustix Warning & Error Tracker

> Generated from `cargo check --all` and `cargo test --all --no-run`.
> Last updated: 2026-06-09

## Table of Contents
1. [Test Compilation Errors (Blocking)](#test-compilation-errors-blocking)
2. [Warnings by Crate](#warnings-by-crate)
   - [rustix-core](#rustix-core)
   - [rustix-asset](#rustix-asset)
   - [rustix-render](#rustix-render)
   - [rustix-physics](#rustix-physics)
   - [rustix-terrain](#rustix-terrain)
   - [rustix-ui](#rustix-ui)
   - [rustix-runtime](#rustix-runtime)
   - [rustix-editor](#rustix-editor)
   - [rustix-world](#rustix-world)
   - [rustix-networking](#rustix-networking)
   - [rustix-platform](#rustix-platform)
   - [Other / Engine](#other--engine)
3. [Auto-Fixable Warnings](#auto-fixable-warnings)

---

## Test Compilation Errors (Blocking)

These prevent `cargo test --all` from compiling.

| # | Error | File | Line | Fix |
|---|-------|------|------|-----|
| 1 | `E0063` missing field `alpha` in `scene::Material` | `apps/runtime/src/scene_tests.rs` | 11 | Add `alpha: 1.0` to the `Material` struct literal |
| 2 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 47 | Add `use rustix_core::math::Vec3;` |
| 3 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 57 | Same as above |
| 4 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 67 | Same as above |
| 5 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 77 | Same as above |
| 6 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 85 | Same as above |
| 7 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 86 | Same as above |
| 8 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 97 | Same as above |
| 9 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 98 | Same as above |
| 10 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 108 | Same as above |
| 11 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 110 | Same as above |
| 12 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 115 | Same as above |
| 13 | `E0433` cannot find type `Vec3` in this scope | `apps/runtime/src/ui/viewport_tests.rs` | 117 | Same as above |

---

## Warnings by Crate

### rustix-core

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | unused imports: `HashMap` and `HashSet` | `crates/core/src/task_graph.rs` | 1 | Remove from `use std::collections::{...}` |
| 2 | unused import: `std::marker::PhantomData` | `crates/core/src/soa_storage.rs` | 3 | Remove import |
| 3 | ambiguous glob re-exports (`DynamicBundle`) | `crates/core/src/lib.rs` | 64 | Be explicit: `pub use ecs::{Entity, ...};` or rename |
| 4 | unused variable: `world` | `crates/core/src/component_groups.rs` | 82 | Prefix with `_` or remove |
| 5 | unused variable: `name` | `crates/core/src/task_graph.rs` | 154 | Prefix with `_` |
| 6 | unused variable: `dep` | `crates/core/src/task_graph.rs` | 128 | Prefix with `_` |
| 7 | unused variable: `dep` | `crates/core/src/task_graph.rs` | 195 | Prefix with `_` |
| 8 | unused variable: `name` | `crates/core/src/task_priority.rs` | 234 | Prefix with `_` |
| 9 | unused variable: `current` | `crates/core/src/system_monitor.rs` | 77 | Prefix with `_` |
| 10 | variable does not need to be mutable | `crates/core/src/gpu_staging.rs` | 104 | Remove `mut` |
| 11 | field `clone_raw` is never read | `crates/core/src/component_registry.rs` | 20 | Remove field or add `#[allow(dead_code)]` |
| 12 | field `start` is never read | `crates/core/src/gpu_staging.rs` | 41 | Remove field or add `#[allow(dead_code)]` |

### rustix-asset

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | unused import: `Path` | `crates/asset/src/vfs.rs` | 9 | Remove import |
| 2 | variant `R8G8B8A8_UNORM` should have an upper camel case name | `crates/asset/src/texture.rs` | 14 | Rename to `R8g8b8a8Unorm` |
| 3 | variant `R16G16B16A16_SFLOAT` should have an upper camel case name | `crates/asset/src/texture.rs` | 16 | Rename to `R16g16b16a16Sfloat` |
| 4 | variant `R32G32B32A32_SFLOAT` should have an upper camel case name | `crates/asset/src/texture.rs` | 18 | Rename to `R32g32b32a32Sfloat` |
| 5 | unused imports: `Asset` and `Handle` | `crates/asset/src/streaming.rs` | 12 | Remove imports |
| 6 | unused import: `std::sync::Arc` | `crates/asset/src/streaming.rs` | 14 | Remove import |
| 7 | variant `BC7_UNORM` should have an upper camel case name | `crates/asset/src/texture_compress.rs` | 14 | Rename to `Bc7Unorm` |
| 8 | variant `BC7_UNORM_SRGB` should have an upper camel case name | `crates/asset/src/texture_compress.rs` | 16 | Rename to `Bc7UnormSrgb` |
| 9 | variant `ASTC_4x4_UNORM` should have an upper camel case name | `crates/asset/src/texture_compress.rs` | 18 | Rename to `Astc4x4Unorm` |
| 10 | variant `ASTC_4x4_UNORM_SRGB` should have an upper camel case name | `crates/asset/src/texture_compress.rs` | 20 | Rename to `Astc4x4UnormSrgb` |
| 11 | variant `ASTC_6x6_UNORM` should have an upper camel case name | `crates/asset/src/texture_compress.rs` | 22 | Rename to `Astc6x6Unorm` |
| 12 | variant `ASTC_6x6_UNORM_SRGB` should have an upper camel case name | `crates/asset/src/texture_compress.rs` | 24 | Rename to `Astc6x6UnormSrgb` |
| 13 | variant `ASTC_8x8_UNORM` should have an upper camel case name | `crates/asset/src/texture_compress.rs` | 26 | Rename to `Astc8x8Unorm` |
| 14 | variant `ASTC_8x8_UNORM_SRGB` should have an upper camel case name | `crates/asset/src/texture_compress.rs` | 28 | Rename to `Astc8x8UnormSrgb` |
| 15 | unused import: `Ordering` | `crates/asset/src/cook.rs` | 14 | Remove import |
| 16 | unused imports | `crates/asset/src/cook.rs` | 15-19 | Remove unused imports |
| 17 | unused import: `MeshAsset` | `crates/asset/src/cook.rs` | 17 | Remove import |
| 18 | unused imports: `TextureAsset` and `export_rxtex` | `crates/asset/src/cook.rs` | 18 | Remove imports |
| 19 | unused import: `AnimationAsset` | `crates/asset/src/cook.rs` | 19 | Remove import |
| 20 | unused import: `SkeletonAsset` | `crates/asset/src/cook.rs` | 20 | Remove import |
| 21 | unused variable: `buffers` | `crates/asset/src/cook.rs` | 14 | Prefix with `_` |
| 22 | unused variable: `stage` | `crates/asset/src/cook.rs` | 14 | Prefix with `_` |
| 23 | associated function `from_asset` is never used | `crates/asset/src/material.rs` | 334 | Remove or add `#[allow(dead_code)]` |
| 24 | unused import: `Vec3` | `crates/asset/src/animation.rs` | 9 | Remove import |
| 25 | unused import: `Vec3` | `crates/asset/src/skeleton.rs` | 9 | Remove import |

### rustix-render

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | unused import: `crate::pipeline` | `crates/render/src/shader.rs` | 176 | Remove import |
| 2 | unused import: `crate::memory::GpuBuffer` | `crates/render/src/shader.rs` | 176 | Remove import |
| 3 | unused import: `crate::error::RenderError` | `crates/render/src/shader.rs` | 176 | Remove import |
| 4 | unused import: `naga::ShaderStage` | `crates/render/src/shader.rs` | 176 | Remove import |
| 5 | unused import: `Mat4` | `crates/render/src/gizmo.rs` | 8 | Remove import |
| 6 | unused variable: `stage` | `crates/render/src/shader_archive.rs` | 17 | Prefix with `_` |
| 7 | unused import: `std::io::Write` | `crates/render/build.rs` | 88 | Remove import |
| 8 | unused variable: `shadow_map` | `crates/render/src/renderer.rs` | 229 | Prefix with `_` |
| 9 | variable does not need to be mutable | `crates/render/src/renderer.rs` | 228 | Remove `mut` |
| 10 | variable does not need to be mutable | `crates/render/src/renderer.rs` | 229 | Remove `mut` |
| 11 | variable does not need to be mutable | `crates/render/src/renderer.rs` | 10 | Remove `mut` |
| 12 | value assigned to `bf` is never read | `crates/render/src/renderer.rs` | 10 | Remove assignment or use variable |
| 13 | unused variable: `stride` | `crates/render/src/renderer.rs` | 10 | Prefix with `_` |
| 14 | unused variable: `theta1` | `crates/render/src/renderer.rs` | 10 | Prefix with `_` |
| 15 | unused imports | `crates/render/src/renderer.rs` | 2 | Remove unused imports |
| 16 | unused imports | `crates/render/src/renderer.rs` | 10 | Remove unused imports |
| 17 | unused variable: `scene_pipeline` | `crates/render/src/renderer/draw.rs` | 5 | Prefix with `_` |
| 18 | unused imports | `crates/render/src/renderer/resource.rs` | 120 | Remove unused imports |
| 19 | unused imports | `crates/render/src/renderer/texture.rs` | 2 | Remove unused imports |
| 20 | unused import: `std::marker::PhantomData` | `crates/render/src/memory/staging.rs` | 156 | Remove import |
| 21 | unused imports | `crates/render/src/memory/ring.rs` | 22 | Remove unused imports |
| 22 | unused imports | `crates/render/src/graph.rs` | 143, 277 | Remove unused imports |
| 23 | field `device` is never read | `crates/render/src/descriptor_cache.rs` | 68 | Remove field or add `#[allow(dead_code)]` |
| 24 | unused imports | `crates/render/src/bindless.rs` | 197 | Remove unused imports |
| 25 | unused imports | `crates/render/src/spv_reflect.rs` | 47, 182 | Remove unused imports |
| 26 | unused imports | `crates/render/src/pipeline.rs` | 1397 | Remove unused imports |
| 27 | unused imports | `crates/render/src/profiler.rs` | 111 | Remove unused imports |
| 28 | call to `.clone()` on a reference in this situation does nothing | `apps/runtime/src/render/deferred_graph.rs` | 338 | Remove `.clone()` call |

### rustix-physics

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | unused imports | `crates/physics/src/rapier.rs` | 38, 96, 137, 632 | Remove unused imports |
| 2 | unused imports: `Collider` and `PhysicsWorld` | `apps/runtime/src/main.rs` | — | Remove imports |
| 3 | unused imports: `RigidBody` and `step_physics` | `apps/runtime/src/main.rs` | — | Remove imports |

### rustix-terrain

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | unused variable: `edge1` | `crates/terrain/src/chunk.rs` | 222 | Prefix with `_` |

### rustix-ui

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | variable does not need to be mutable | `crates/ui/src/layout.rs` | 196 | Remove `mut` |
| 2 | unused variable: `cursor_main` | `crates/ui/src/layout.rs` | 196 | Prefix with `_` |
| 3 | unused variable: `i` | `crates/ui/src/layout.rs` | 228 | Prefix with `_` |
| 4 | unused variable: `spacing` | `crates/ui/src/layout.rs` | 228 | Prefix with `_` |
| 5 | unused imports | `crates/ui/src/lib.rs` | 168, 381 | Remove unused imports |

### rustix-runtime

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | unused imports: `scene_to_world`, `world_to_scene`, `world_transform` | `apps/runtime/src/ui/menu_bar.rs` | — | Remove imports |
| 2 | unused imports: `AudioEngine` and `SoundInstance` | `apps/runtime/src/ui/editor.rs` | — | Remove imports |
| 3 | unused imports: `ConfirmTarget`, `ProjectInfo`, `load_recent_projects` | `apps/runtime/src/ui/editor.rs` | — | Remove imports |
| 4 | unused imports: `add_recent_project`, `create_project_file`, `load_project_file`, `write_project_file` | `apps/runtime/src/ui/startup.rs` | — | Remove imports |
| 5 | unused import: `crate::camera::EditorCamera` | `apps/runtime/src/ui/hierarchy.rs` | — | Remove import |
| 6 | unused import: `crate::camera::EditorCamera` | `apps/runtime/src/ui/inspector.rs` | — | Remove import |
| 7 | unused import: `crate::camera::EditorCamera` | `apps/runtime/src/ui/viewport/primary.rs` | — | Remove import |
| 8 | unused import: `crate::camera::EditorCamera` | `apps/runtime/src/ui/menu_bar.rs` | — | Remove import |
| 9 | unused import: `DirectionalLight` | `apps/runtime/src/main.rs` | — | Remove import |
| 10 | unused import: `rustix_core::math::Vec3` | `apps/runtime/src/main.rs` | — | Remove import |
| 11 | unused imports: `MAX_VIEWPORTS`, `ViewportManager`, `Viewport`, `show_viewports`, `viewport_texture_id` | `apps/runtime/src/main.rs` | — | Remove imports |
| 12 | unused imports: `Mat4` and `Vec4` | `apps/runtime/src/main.rs` | — | Remove imports |
| 13 | unused import: `hecs::Entity` | `apps/runtime/src/main.rs` | — | Remove import |
| 14 | unused imports: `Alpha` and `color_picker_hsva_2d` | `apps/runtime/src/ui/inspector.rs` | — | Remove imports |
| 15 | unused import: `Rect` | `apps/runtime/src/ui/inspector.rs` | — | Remove import |
| 16 | unused import: `rustix_platform::input::InputManager` | `apps/runtime/src/ui/viewport/primary.rs` | — | Remove import |
| 17 | unused import: `rustix_platform::window::WindowHandle` | `apps/runtime/src/ui/viewport/primary.rs` | — | Remove import |
| 18 | unused import: `rustix_render::graph::PassContext` | `apps/runtime/src/ui/viewport/primary.rs` | — | Remove import |
| 19 | unused import: `rustix_render::mesh::Mesh` | `apps/runtime/src/main.rs` | — | Remove import |
| 20 | unused variable: `alt_held` | `apps/runtime/src/ui/viewport/primary.rs` | — | Prefix with `_` |
| 21 | unused variable: `world` | `apps/runtime/src/ui/viewport/primary.rs` | — | Prefix with `_` |
| 22 | unused variable: `looping` | `apps/runtime/src/ui/viewport/primary.rs` | — | Prefix with `_` |
| 23 | unused variable: `ndc` | `apps/runtime/src/ui/viewport/primary.rs` | — | Prefix with `_` |
| 24 | unused variable: `fb` | `apps/runtime/src/ui/viewport/primary.rs` | — | Prefix with `_` |
| 25 | unused variable: `idx` | `apps/runtime/src/ui/viewport/primary.rs` | — | Prefix with `_` |
| 26 | unused variable: `device` | `apps/runtime/src/ui/viewport/primary.rs` | — | Prefix with `_` |
| 27 | unused variable: `volume` | `apps/runtime/src/main.rs` | — | Prefix with `_` |
| 28 | unused variable: `ubo` | `apps/runtime/src/main.rs` | — | Prefix with `_` |
| 29 | unused variable: `sw_w` | `apps/runtime/src/main.rs` | — | Prefix with `_` |
| 30 | unused variable: `sw_h` | `apps/runtime/src/main.rs` | — | Prefix with `_` |
| 31 | value assigned to `any_offscreen` is never read | `apps/runtime/src/main.rs` | — | Remove or use |
| 32 | value assigned to `used_hdr` is never read | `apps/runtime/src/main.rs` | — | Remove or use |
| 33 | value assigned to `pc_data2` is never read | `apps/runtime/src/main.rs` | — | Remove or use |
| 34 | value assigned to `app.bloom_fb_size` is never read | `apps/runtime/src/main.rs` | — | Remove or use |
| 35 | value assigned to `app.taa_fb_size` is never read | `apps/runtime/src/main.rs` | — | Remove or use |
| 36 | value assigned to `app.ssr_fb_size` is never read | `apps/runtime/src/main.rs` | — | Remove or use |
| 37 | value assigned to `app.ssao_fb_size` is never read | `apps/runtime/src/main.rs` | — | Remove or use |
| 38 | value assigned to `app.skybox_fb_size` is never read | `apps/runtime/src/main.rs` | — | Remove or use |
| 39 | value assigned to `app.oit_fb_size` is never read | `apps/runtime/src/main.rs` | — | Remove or use |
| 40 | value assigned to `app.fog_fb_size` is never read | `apps/runtime/src/main.rs` | — | Remove or use |
| 41 | use of deprecated method `egui::Ui::allocate_ui_at_rect` | `apps/runtime/src/ui/startup.rs` | — | Use `allocate_new_ui` instead |
| 42 | use of deprecated method `egui::DragValue::clamp_to_range` | `apps/runtime/src/ui/inspector.rs` | — | Use `clamp_existing_to_range` |
| 43 | use of deprecated method `egui::Context::style` | `apps/runtime/src/ui/editor.rs` | — | Use `global_style` instead |
| 44 | use of deprecated associated function `egui::Frame::none` | `apps/runtime/src/ui/startup.rs` | — | Use `Frame::NONE` or `Frame::new()` |
| 45 | use of deprecated method `egui::Panel::show` | `apps/runtime/src/ui/viewport/primary.rs` | — | Use `show_inside()` instead |
| 46 | use of deprecated method `egui::CentralPanel::show` | `apps/runtime/src/main.rs` | — | Use `show_inside()` instead |

### rustix-editor

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | unused imports | `crates/editor/src/undo.rs` | 3 | Remove imports |
| 2 | unused imports | `crates/editor/src/terrain_editor.rs` | 3 | Remove imports |
| 3 | unused imports | `crates/editor/src/lighting_editor.rs` | 3 | Remove imports |
| 4 | unused imports | `crates/editor/src/lib.rs` | 124 | Remove imports |
| 5 | unused imports | `crates/editor/src/inspector.rs` | 3, 4 | Remove imports |

### rustix-world

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | unused imports | `crates/world/src/serialization.rs` | 6 | Remove imports |

### rustix-networking

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | unused imports | `crates/networking/src/protocol.rs` | 9, 311 | Remove imports |

### rustix-platform

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | unused imports | `crates/platform/src/gamepad.rs` | 1 | Remove imports |
| 2 | unused imports: `GamepadAxis`, `GamepadButton`, `GamepadId` | `apps/runtime/src/main.rs` | — | Remove imports |

### Other / Engine

| # | Warning | File | Line | Suggested Fix |
|---|---------|------|------|---------------|
| 1 | static variable `SPIRV_*` should have an upper case name | `target/debug/build/rustix-render-*/out/shader_archive_gen.rs` | generated | Fix `sanitize_ident` in `crates/render/build.rs` to produce valid Rust identifiers |
| 2 | unexpected `cfg` condition value: `profiling` | `engine/src/app.rs` | 38 | Add to `Cargo.toml` `[lints.rust.unexpected_cfgs]` or use `#[allow(unexpected_cfgs)]` |
| 3 | type `CollisionEventCollector` is more private than the item `RapierPhysicsWorld::event_handler` | `crates/physics/src/rapier.rs` | — | Make `CollisionEventCollector` `pub` or reduce visibility of `event_handler` |
| 4 | hiding a lifetime that's elided elsewhere is confusing | `crates/render/src/spv_reflect.rs` | — | Add explicit lifetime annotations |

---

## Auto-Fixable Warnings

Many of the above can be fixed automatically with:

```bash
# Fix unused imports, unused variables, unnecessary mut, etc.
cargo fix --allow-dirty --allow-staged

# Per-crate fixes
cargo fix --lib -p rustix-core
cargo fix --lib -p rustix-asset
cargo fix --lib -p rustix-render
cargo fix --lib -p rustix-physics
cargo fix --lib -p rustix-terrain
cargo fix --lib -p rustix-ui
cargo fix --bin rustix-runtime -p rustix-runtime
```

**Note:** Auto-fix will not handle:
- Enum variant naming conventions (`R8G8B8A8_UNORM` → `R8g8b8a8Unorm`)
- Ambiguous glob re-exports
- Deprecated API usage
- Dead code fields (struct fields that are never read)
- Test compilation errors (must be fixed manually)

---

## Priority Fix Order

1. **Test compilation errors** (`scene_tests.rs`, `viewport_tests.rs`) — blocking CI
2. **Deprecated egui APIs** — may break on next egui upgrade
3. **Unused imports/variables in runtime** — clutter the editor build
4. **Enum variant naming** — affects public API consistency
5. **Shader archive generated identifiers** — affects release builds
