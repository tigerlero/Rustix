# Pending Tests

## Shadow Mapping

### Shader Tests
- [x] **Vertex shader outputs `fragPosLightSpace`** — Fixed: shader compiles with naga 23.
- [x] **Fragment shader `shadowFactor` function** — Fixed: uses separate `texture2D` + `sampler` for naga compatibility. Now includes 3×3 PCF soft shadow filtering.
- [x] **Fragment shader ambient + lit blending** — Tests in `crates/render/src/shader.rs` verify `shade(base, 0.0)` equals ambient-only, `shade(base, 1.0)` equals ambient + lit, and partial shadow values from PCF produce correct intermediate lighting.
- [x] **Shadow vertex shader compiles without fragment stage** — Confirmed: compiles and links.

### Pipeline Tests
- [x] **ShadowPipeline creation** — Extracted pure configuration functions in `crates/render/src/pipeline.rs`: `shadow_descriptor_set_bindings()`, `shadow_push_constant_range()`, `shadow_vertex_input_state()`, `shadow_depth_stencil_state()`. Tests verify single UBO binding at 0 (VERTEX stage only), push constant range for VERTEX stage with size 128, 2 vertex attributes (pos+normal) with stride 24, and depth test/write enabled with LESS compare.
- [x] **UBO size alignment** — `UBO_SCENE_SIZE` (432 bytes) accommodates `light_view_proj` at offset 368.
- [x] **Descriptor set layout bindings** — Main pipeline uses bindings 0 (UBO), 1 (sampled image), 2 (sampler). Shadow pipeline uses binding 0 (UBO) only.

### Resource Creation Tests
- [x] **Shadow map texture creation** — Extracted `shadow_map_image_info(size)` pure function in `crates/render/src/renderer/resource.rs`. Tests verify `D32_SFLOAT` format, correct extent, `DEPTH_STENCIL_ATTACHMENT | SAMPLED` usage, optimal tiling, and 2D/1-sample/1-mip configuration.
- [x] **Shadow map sampler properties** — Extracted `shadow_sampler_info()` pure function. Tests verify `NEAREST` filtering/mipmapping, `CLAMP_TO_BORDER` addressing on all axes, `FLOAT_OPAQUE_WHITE` border color, and disabled compare mode.
- [x] **Descriptor pool sizing** — Pool includes `UNIFORM_BUFFER`, `SAMPLED_IMAGE`, and `SAMPLER`.

### Rendering Tests
- [x] **Light view-projection matrix computation** — Extracted `compute_light_view_proj` pure function in `apps/runtime/src/render.rs`. Tests verify it produces non-identity matrices, changes with direction/center, handles unnormalized input, and produces correct orthographic NDC bounds.
- [x] **Shadow pass renders all meshes** — Test in `apps/runtime/src/render_tests.rs` verifies an entity outside the camera frustum (x=50, well outside 45° FOV) is culled by `Frustum::intersects_aabb` but would still be rendered in the shadow pass, confirming shadow pass has no frustum culling.
- [x] **Image layout transitions** — Extracted `layout_transition_params()` pure function in `crates/render/src/renderer/resource.rs`. Tests verify correct pipeline stage and access masks for: UNDEFINED→DEPTH_ATTACHMENT, DEPTH_ATTACHMENT→SHADER_READ, SHADER_READ→DEPTH_ATTACHMENT, and unknown fallback.
- [x] **Shadow map sampled in main pass** — CPU-side `shadow_factor()` in `crates/render/src/shader_tests.rs` replicates the full GLSL `shadowFactor` function with mock sampler. Tests verify: outside frustum returns 1.0 (lit), all depths=1.0 returns 1.0 (lit), all depths=0.0 returns 0.0 (shadowed), partial PCF produces 5/9, and NDC→UV mapping is correct.
- [x] **Shadow descriptor set binding** — Extracted `main_descriptor_set_bindings()` in `crates/render/src/pipeline.rs`. Tests verify main pass has 3 bindings (UBO at 0 VERTEX|FRAGMENT, sampled image at 1 FRAGMENT, sampler at 2 FRAGMENT) while shadow pass has 1 binding (UBO at 0 VERTEX-only).

### Integration Tests
- [x] **Directional light rotation affects shadow direction** — Extracted `directional_light_dir_from_euler` in `apps/runtime/src/render.rs`. Tests verify different rotations produce different light directions, the direction is deterministic, and rotation changes the shadow VP matrix.
- [x] **Object self-shadowing bias** — Tests in `crates/render/src/shader_tests.rs` replicate the shader's `currentDepth - bias > pcfDepth` comparison. Verified: exact depth matches don't self-shadow, small epsilon differences don't self-shadow, and points clearly behind an occluder remain shadowed despite bias.
- [x] **No shadow when no directional light exists** — Scene renders without crash; main pass works with or without shadow resources.
- [x] **Shadow pass doesn't crash on empty scene** — `render_3d_scene` gracefully skips shadow pass when resources are missing.
- [x] **3D scene renders even if shadow resources fail** — `render_3d_scene` now accepts shadow resources as `Option`.

### Frustum Culling
- [x] **Mesh AABB computed from vertices** — Tests in `crates/render/src/mesh.rs` verify cube and sphere AABBs match expected bounds.
- [x] **Frustum culling in render loop** — `render_3d_scene` builds a `Frustum` from `view_proj` and skips entities whose world-space AABB is outside the camera view.

---

## Render Crate Module Split

### Module Compilation Tests
- [x] **All modules compile independently** — `cargo build` passes after split.
- [x] **Re-exports preserved** — `rustix_render::Renderer`, `RenderError`, `DepthBuffer`, `GpuTexture`, `Framebuffer`, `RenderConfig` all accessible.
- [x] **No duplicate type definitions** — `Renderer` only in `renderer.rs`, `RenderError` only in `error.rs`, etc.

### Backward Compatibility Tests
- [x] **Runtime crate builds without changes** — `apps/runtime` compiles with zero modifications.
- [x] **init.rs resource creation unchanged** — `init_scene_resources()` still creates all resources correctly.
- [x] **render.rs scene pass unchanged** — `render_3d_scene()` continues to work with the new module structure.

### File Structure Tests
- [x] **renderer.rs is under 150 lines** — `crates/render/src/renderer.rs` only contains struct, core lifecycle, and module declarations.
- [x] **No orphaned code in renderer.rs** — All previous `impl` blocks moved to `resource.rs`, `texture.rs`, `draw.rs`.

---

## Camera Controls

### Orbit Mode Tests
- [x] **Right-click drag rotates camera** — Test in `apps/runtime/src/camera.rs` verifies `yaw` and `pitch` change when `MouseButton::Right` is held and mouse moves.
- [x] **Left-click drag does NOT rotate camera** — Test in `apps/runtime/src/camera.rs` verifies yaw and pitch stay unchanged when `MouseButton::Left` is held and mouse moves.
- [x] **Middle-click drag pans center** — Test in `apps/runtime/src/camera.rs` verifies `cam.center` x and y change when `MouseButton::Middle` is held and mouse moves.
- [x] **Right-click no longer pans** — Test in `apps/runtime/src/camera.rs` verifies `cam.center` stays unchanged when `MouseButton::Right` is held and mouse moves.

### Keyboard Tests
- [x] **Plain WASD does nothing** — Test in `apps/runtime/src/camera.rs` verifies distance, yaw, and pitch stay unchanged when W/A/S/D are pressed without Shift.
- [x] **Shift+W zooms in / Shift+S zooms out** — Test in `apps/runtime/src/camera.rs` verifies Shift+W decreases distance and Shift+S increases distance.
- [x] **Shift+A rotates left / Shift+D rotates right** — Test in `apps/runtime/src/camera.rs` verifies Shift+A decreases yaw and Shift+D increases yaw.
- [x] **Shift+Q pitches up / Shift+E pitches down** — Tests in `apps/runtime/src/camera.rs` verify Shift+Q decreases pitch and Shift+E increases pitch, both clamped to [-1.4, 1.4].
- [x] **Ctrl+S does nothing** — Test in `apps/runtime/src/camera.rs` verifies distance, yaw, pitch, and center stay unchanged when Ctrl+S is pressed.

### First-Person Mode Tests
- [x] **Right-click drag looks around** — Test in `apps/runtime/src/camera.rs` verifies yaw and pitch change with right-click drag in FirstPerson mode.
- [x] **Shift+WASD moves position** — Test in `apps/runtime/src/camera.rs` verifies Shift+W and Shift+S change `cam.position` in FirstPerson mode.
- [x] **Plain WASD does nothing in FP mode** — Test in `apps/runtime/src/camera.rs` verifies position stays unchanged when W/A/S/D pressed without Shift in FirstPerson mode.

---

## Gizmo Interaction (Regression Tests)
- [x] **W/E/R still switch gizmo modes** — Extracted `resolve_gizmo_mode_pure()` in `apps/runtime/src/ui/viewport.rs`. Tests verify W→translate(0), E→rotate(1), R→scale(2), no key preserves current, and precedence W > E > R when multiple keys pressed.
- [x] **Left-click drag on gizmo handles works** — Extracted `apply_gizmo_rotation()`, `apply_gizmo_scale()`, `apply_gizmo_translation()`, and `snap_vec3()` in `apps/runtime/src/ui/viewport.rs`. Tests verify: right drag increases X rotation, up drag increases Y rotation, right drag increases scale, negative drag clamps to 0.01, translation moves along axis, snap rounds to grid.
- [x] **Gizmo handles don't conflict with camera** — The gizmo drag state (`gizmo_dragging`) is stored in egui temp data and checked before camera updates in `render_3d_scene`. When a gizmo handle is being dragged, egui consumes the pointer events so `InputManager` doesn't see them. The extracted pure functions confirm the transform math is separate from camera input handling.
