# Rustix Dead Code Inventory

> Auto-generated from `#[allow(dead_code)]` annotations across the workspace (excluding `target/`).
> Each entry includes the file, line, item, and a suggested fix.

---

## Table of Contents

1. [rustix-physics](#rustix-physics)
2. [rustix-render](#rustix-render)
3. [rustix-ui](#rustix-ui)
4. [rustix-runtime](#rustix-runtime)
   - [scene.rs](#sceners)
   - [render subsystem](#render-subsystem)
   - [app_state.rs](#app_staters)
   - [ui/viewport/manager.rs](#uiviewportmanagerrs)
   - [undo.rs](#undors)
   - [sprite_editor.rs](#sprite_editorrs)
   - [ui_renderer.rs](#ui_rendererrs)

---

## rustix-physics

### `crates/physics/src/rapier.rs:137` ✅ FIXED

- **Item:** `fn gravity(&self) -> nalgebra::Vector3<f32>`
- **Action:** Removed unused private accessor. The `self.gravity` field is still required by `step()` and snapshot/restore logic.

### `crates/physics/src/rapier.rs:633` ✅ FIXED

- **Item:** `fn nalgebra_point_to_vec3(p: nalgebra::Point3<f32>) -> Vec3`
- **Action:** Removed unused private helper. The sibling `nalgebra_vec_to_vec3` is actively used in `transform_of` and `move_character`, so it remains.

---

## rustix-render

### `crates/render/src/memory/staging.rs:156` ✅ FIXED

- **Item:** `device: *const ash::Device` (struct field)
- **Action:** Removed the unused raw-pointer field from `GpuStagingBuffer`. The underlying `GpuBuffer` handles its own destruction, so the pointer was redundant.

### `crates/render/src/memory/ring.rs:22` ✅ FIXED

- **Item:** `device: *const ash::Device` (struct field)
- **Action:** Removed the unused raw-pointer field from `GpuUniformRing`. Same rationale as `GpuStagingBuffer` — the `GpuBuffer` handles destruction.

### `crates/render/src/graph.rs:140` ✅ FIXED

- **Item:** `struct TransientImage { ... }`
- **Action:** Removed the `#[allow(dead_code)]` annotation and pruned the unused fields (`offset`, `size`, `first_pass`, `last_pass`). Only `image` and `view` remain, which are actively used in `destroy_transient_resources`.

### `crates/render/src/profiler.rs:111` ✅ FIXED

- **Item:** `allocation: gpu_allocator::vulkan::Allocation` (struct field) — part of `GpuReadbackBuffer`
- **Action:** Removed the entire unused `GpuReadbackBuffer` struct and its impl block. It was `pub` but never referenced anywhere, and it lacked a `Drop` impl for proper memory cleanup. Also removed the now-unused `GpuMemoryAllocator` import.

---

## rustix-ui

### `crates/ui/src/lib.rs:381` ✅ FIXED

- **Item:** `desc_set_layout: vk::DescriptorSetLayout`
- **Action:** Added an explanatory comment clarifying that this handle (and `desc_pool`) have no Rust-level reads but must outlive the `desc_set` in Vulkan. Destroying the pool or layout while the set is still in use is undefined behavior, so the fields are intentionally retained.

### `crates/ui/src/lib.rs:383` ✅ FIXED

- **Item:** `desc_pool: vk::DescriptorPool`
- **Action:** Same as above — documented the Vulkan lifetime requirement alongside `desc_set_layout`.

---

## rustix-runtime

### `apps/runtime/src/scene.rs:41` ✅ FIXED

- **Item:** `Material::from_asset(asset: &MaterialAsset) -> Self`
- **Action:** Removed the unused method. It had no callers anywhere in the workspace.

### `apps/runtime/src/scene.rs:290` ✅ FIXED

- **Item:** `fn spawn_prefab_entities(world: &mut EcsWorld, ...)`
- **Action:** Removed along with `spawn_prefab` and `spawn_region`. All three had no external callers.

### `apps/runtime/src/scene.rs:446` ✅ FIXED

- **Item:** `pub fn spawn_prefab(world: &mut EcsWorld, ...)`
- **Action:** Removed. No callers anywhere in the workspace.

### `apps/runtime/src/scene.rs:463` ✅ FIXED

- **Item:** `pub fn spawn_region(world: &mut EcsWorld, ...)`
- **Action:** Removed. No callers anywhere in the workspace.

---

### render subsystem

#### `apps/runtime/src/render/instanced.rs:39`

- **Item:** `pub struct InstanceBuffer { ... }`
- **Note:** GPU buffer for per-instance transforms. Part of an instanced-rendering path that is not currently active.
- **Fix:** If instanced draw path is planned, keep and wire into `scene.rs` draw loop; otherwise delete `InstanceBuffer`, `IndirectDrawBuffer`, `MeshBatch`, and `InstancedMeshBatcher` together.

#### `apps/runtime/src/render/instanced.rs:76`

- **Item:** `InstanceBuffer::destroy(&mut self, device: &ash::Device)`
- **Note:** Manual Vulkan buffer destroy helper.
- **Fix:** Use `destroy()` in a `Drop` impl or resource cleanup routine. If the engine now uses `GpuBuffer`'s own drop logic, delete this method.

#### `apps/runtime/src/render/instanced.rs:83`

- **Item:** `pub struct IndirectDrawBuffer { ... }`
- **Note:** Buffer for `VkDrawIndexedIndirectCommand`. Unused.
- **Fix:** Remove if instanced rendering is not used; otherwise integrate into the culling/compute path.

#### `apps/runtime/src/render/instanced.rs:119`

- **Item:** `IndirectDrawBuffer::destroy(&mut self, device: &ash::Device)`
- **Note:** Same as `InstanceBuffer::destroy`.
- **Fix:** Same — adopt `GpuBuffer` drop or remove.

#### `apps/runtime/src/render/instanced.rs:126`

- **Item:** `pub struct MeshBatch { ... }`
- **Note:** Groups entities by mesh for batching. Unused.
- **Fix:** Remove along with the rest of the instanced-rendering subsystem if it is not being revived.

#### `apps/runtime/src/render/instanced.rs:135`

- **Item:** `pub struct InstancedMeshBatcher { ... }`
- **Note:** High-level batcher that builds instance & indirect buffers. Unused.
- **Fix:** Same — remove subsystem or integrate into draw loop.

#### `apps/runtime/src/render/instanced.rs:146`

- **Item:** `impl InstancedMeshBatcher { ... }`
- **Note:** Implementation block for the batcher.
- **Fix:** Same as above.

#### `apps/runtime/src/render/gpu_culling.rs:66`

- **Item:** `pub struct GpuCullingResources { ... }`
- **Note:** Compute pipeline and buffers for GPU-driven frustum culling. Not wired into the frame graph.
- **Fix:** Either integrate into the `deferred_graph.rs` / `hdr_graph.rs` render path (before the scene pass), or remove the module.

#### `apps/runtime/src/render/gpu_culling.rs:96`

- **Item:** `impl GpuCullingResources { ... }`
- **Note:** `new()` and update methods for the culling resources.
- **Fix:** Same — integrate or delete.

#### `apps/runtime/src/render/forward_plus.rs:7`

- **Item:** `pub struct ForwardPlusResources { ... }`
- **Note:** Tiled light-culling buffers. Unused; the engine currently uses a simpler uniform light array.
- **Fix:** If deferred + tiled forward is the target, wire this into the lighting pass; otherwise remove.

#### `apps/runtime/src/render/forward_plus.rs:17`

- **Item:** `impl ForwardPlusResources { ... }`
- **Note:** Constructor and constants for Forward+.
- **Fix:** Same as above.

#### `apps/runtime/src/render/gbuffer.rs:7`

- **Item:** `pub struct GBufferResources { ... }`
- **Note:** G-buffer images for deferred shading. The deferred pass is active but these resources may be allocated elsewhere.
- **Fix:** Verify whether `deferred_graph.rs` creates its own g-buffer attachments. If this struct is redundant, remove it; otherwise migrate deferred pass setup to use it.

#### `apps/runtime/src/render/gbuffer.rs:28`

- **Item:** `impl GBufferResources { ... }`
- **Note:** Constructor for g-buffer resources.
- **Fix:** Same — deduplicate or remove.

#### `apps/runtime/src/render/oit.rs:9`

- **Item:** `pub struct OitResources { ... }`
- **Note:** Order-independent transparency resources (accum / revealage / composite). Not used in current forward/deferred path.
- **Fix:** If OIT is on the roadmap, keep behind a feature flag; otherwise delete the module.

#### `apps/runtime/src/render/oit.rs:22`

- **Item:** `impl OitResources { ... }`
- **Note:** OIT resource constructor.
- **Fix:** Same as above.

#### `apps/runtime/src/render/lighting.rs:8`

- **Item:** `pub fn compute_light_view_proj(light_dir: Vec3, center: Vec3) -> Mat4`
- **Note:** Computes a shadow-map view-projection matrix. Likely superseded by CSM code in `deferred_graph.rs` or `shadow.rs`.
- **Fix:** If CSM already handles this, remove; otherwise replace the inline math in the shadow pass with this helper.

#### `apps/runtime/src/render/overlay.rs:9`

- **Item:** `pub fn render_2d_overlay(...)`
- **Note:** 2D overlay renderer (e.g., for debug UI or HUD). Not called from main loop.
- **Fix:** Wire it into the post-process chain in `main.rs` or remove if 2D overlay is now handled by egui exclusively.

---

### `apps/runtime/src/app_state.rs:18`

- **Item:** `pub struct AppState { ... }`
- **Note:** The entire struct is marked `#[allow(dead_code)]`. It is used heavily in `main.rs`, so the lint is likely triggered by fields that are written but never read.
- **Fix:** Remove the global `#[allow(dead_code)]` on the struct, then fix individual field lints (e.g., framebuffer-size fields, unused handles) by either reading them or deleting them.

### `apps/runtime/src/app_state.rs:164`

- **Item:** `impl AppState { ... }`
- **Note:** Same as above — the impl block carries the allow.
- **Fix:** Remove the attribute once the struct-level allow is gone.

---

### `apps/runtime/src/ui/viewport/manager.rs:13`

- **Item:** `pub struct Viewport { ... }`
- **Note:** Editor viewport abstraction with per-viewport camera. The runtime currently uses a single global camera.
- **Fix:** If multi-viewport editor layout is planned, keep and wire into `ViewportManager`; otherwise simplify to a single camera and remove this struct.

---

### `apps/runtime/src/undo.rs:68`

- **Item:** `pub fn can_undo(&self) -> bool`
- **Note:** Simple index check for undo availability.
- **Fix:** Wire into the editor UI (e.g., disable/enable Edit → Undo menu item) or remove.

### `apps/runtime/src/undo.rs:70`

- **Item:** `pub fn can_redo(&self) -> bool`
- **Note:** Simple index check for redo availability.
- **Fix:** Wire into the editor UI (e.g., disable/enable Edit → Redo menu item) or remove.

---

### `apps/runtime/src/sprite_editor.rs:16`

- **Item:** `pub struct SpriteEditor { ... }`
- **Note:** Sprite editor state. No UI or update code references it.
- **Fix:** If 2D sprite workflow is planned, keep behind a feature flag; otherwise remove the module.

### `apps/runtime/src/sprite_editor.rs:45`

- **Item:** `impl SpriteEditor { ... }`
- **Note:** Constructor and methods for the sprite editor.
- **Fix:** Same as above.

---

### `apps/runtime/src/ui_renderer.rs:19`

- **Item:** `sampler: vk::Sampler` (struct field)
- **Note:** Custom UI renderer stores a Vulkan sampler that is never bound.
- **Fix:** Either bind it when drawing the UI mesh, or remove the field if the UI texture is sampled via a bindless/default sampler.

---

## Recommended Cleanup Order

1. **rustix-render memory helpers** (`staging.rs`, `ring.rs`) — simplest; just remove or use the `device` field.
2. **rustix-ui descriptor fields** (`lib.rs:381`, `383`) — remove unused layout/pool or integrate into draw setup.
3. **rustix-runtime render stubs** (`instanced.rs`, `gpu_culling.rs`, `forward_plus.rs`, `oit.rs`, `overlay.rs`, `lighting.rs`) — remove whole modules if not on the near-term roadmap.
4. **rustix-runtime scene helpers** (`scene.rs:290`, `446`, `463`) — integrate into level load or remove.
5. **AppState struct allow** — remove blanket `#[allow(dead_code)]` and fix per-field warnings.
6. **Viewport, SpriteEditor, undo helpers** — wire into UI or remove.
