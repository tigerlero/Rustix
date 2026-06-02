# Pending Tests

## Shadow Mapping

### Shader Tests
- [x] **Vertex shader outputs `fragPosLightSpace`** — Fixed: shader compiles with naga 23.
- [x] **Fragment shader `shadowFactor` function** — Fixed: uses separate `texture2D` + `sampler` for naga compatibility.
- [x] **Fragment shader ambient + lit blending** — Test in `crates/render/src/shader.rs` verifies `shade(base, 0.0)` equals ambient-only and `shade(base, 1.0)` equals ambient + lit.
- [x] **Shadow vertex shader compiles without fragment stage** — Confirmed: compiles and links.

### Pipeline Tests
- [ ] **ShadowPipeline creation** — Verify `ShadowPipeline::create()` builds a valid Vulkan pipeline with no color attachments, depth-only rendering, and correct push constant ranges.
- [x] **UBO size alignment** — `UBO_SCENE_SIZE` (432 bytes) accommodates `light_view_proj` at offset 368.
- [x] **Descriptor set layout bindings** — Main pipeline uses bindings 0 (UBO), 1 (sampled image), 2 (sampler). Shadow pipeline uses binding 0 (UBO) only.

### Resource Creation Tests
- [ ] **Shadow map texture creation** — Verify `create_shadow_map(1024)` produces a valid `D32_SFLOAT` image with `DEPTH_STENCIL_ATTACHMENT | SAMPLED` usage.
- [ ] **Shadow map sampler properties** — Confirm sampler uses `NEAREST` filtering, `CLAMP_TO_BORDER`, and `FLOAT_OPAQUE_WHITE` border color.
- [x] **Descriptor pool sizing** — Pool includes `UNIFORM_BUFFER`, `SAMPLED_IMAGE`, and `SAMPLER`.

### Rendering Tests
- [ ] **Light view-projection matrix computation** — Verify orthographic projection bounds and light position calculation from directional light transform.
- [ ] **Shadow pass renders all meshes** — Confirm every entity with `MeshComponent` is drawn during the shadow pass.
- [ ] **Image layout transitions** — Test transitions: `UNDEFINED → DEPTH_ATTACHMENT → SHADER_READ_ONLY` across frames.
- [ ] **Shadow map sampled in main pass** — Verify the fragment shader receives valid shadow depth values and applies shadow factor correctly.
- [ ] **Shadow descriptor set binding** — Confirm shadow pass uses its own descriptor set (UBO only) while main pass uses the combined UBO+image+sampler set.

### Integration Tests
- [ ] **Directional light rotation affects shadow direction** — Rotate a `DirectionalLight` entity and verify shadow direction changes accordingly.
- [ ] **Object self-shadowing bias** — Place a plane at y=0 with a cube on it; verify the cube casts a shadow on the plane without excessive acne.
- [x] **No shadow when no directional light exists** — Scene renders without crash; main pass works with or without shadow resources.
- [x] **Shadow pass doesn't crash on empty scene** — `render_3d_scene` gracefully skips shadow pass when resources are missing.
- [x] **3D scene renders even if shadow resources fail** — `render_3d_scene` now accepts shadow resources as `Option`.

---

## Render Crate Module Split

### Module Compilation Tests
- [ ] **All modules compile independently** — Run `cargo check -p rustix-render` after split; no resolution errors.
- [ ] **Re-exports preserved** — Verify `rustix_render::RenderError`, `rustix_render::Renderer`, `rustix_render::DepthBuffer`, `rustix_render::GpuTexture`, `rustix_render::Framebuffer`, `rustix_render::RenderConfig` are all accessible from downstream crates.
- [ ] **No duplicate type definitions** — Confirm `RenderError` exists only in `error.rs`, `Renderer` only in `renderer.rs`, etc.

### Backward Compatibility Tests
- [ ] **Runtime crate builds without changes** — Verify `apps/runtime` compiles after the split with zero modifications.
- [ ] **init.rs resource creation unchanged** — Confirm `init_scene_resources()` still creates depth buffer, shadow map, and pipelines correctly.
- [ ] **render.rs scene pass unchanged** — Verify `render_3d_scene()` continues to work with the new module structure.

### File Structure Tests
- [ ] **lib.rs is under 30 lines** — Verify `crates/render/src/lib.rs` only contains module declarations and re-exports.
- [ ] **No orphaned code in lib.rs** — Confirm all previous `impl` blocks and struct definitions were moved out.

---

## Camera Controls

### Orbit Mode Tests
- [x] **Right-click drag rotates camera** — Test in `apps/runtime/src/camera.rs` verifies `yaw` and `pitch` change when `MouseButton::Right` is held and mouse moves.
- [x] **Left-click drag does NOT rotate camera** — Test in `apps/runtime/src/camera.rs` verifies yaw and pitch stay unchanged when `MouseButton::Left` is held and mouse moves.
- [x] **Middle-click drag pans center** — Test in `apps/runtime/src/camera.rs` verifies `cam.center` x and y change when `MouseButton::Middle` is held and mouse moves.
- [x] **Right-click no longer pans** — Test in `apps/runtime/src/camera.rs` verifies `cam.center` stays unchanged when `MouseButton::Right` is held and mouse moves.

### Keyboard Tests
- [ ] **Plain WASD does nothing** — Press `W`/`A`/`S`/`D` without Shift; verify camera does not move, zoom, or rotate.
- [ ] **Shift+W zooms in** — Hold Shift + `W`; verify `cam.distance` decreases.
- [ ] **Shift+S zooms out** — Hold Shift + `S`; verify `cam.distance` increases.
- [ ] **Shift+A rotates left** — Hold Shift + `A`; verify `cam.yaw` decreases.
- [ ] **Shift+D rotates right** — Hold Shift + `D`; verify `cam.yaw` increases.
- [ ] **Shift+Q pitches up** — Hold Shift + `Q`; verify `cam.pitch` decreases (clamped).
- [ ] **Shift+E pitches down** — Hold Shift + `E`; verify `cam.pitch` increases (clamped).
- [ ] **Ctrl+S does nothing** — Hold Ctrl + `S`; verify camera stays still.

### First-Person Mode Tests
- [ ] **Right-click drag looks around** — In first-person mode, right-click drag changes yaw/pitch.
- [ ] **Shift+WASD moves position** — Hold Shift + `W`/`A`/`S`/`D`; verify `cam.position` changes.
- [ ] **Plain WASD does nothing in FP mode** — Press `W` without Shift; verify position unchanged.

---

## Gizmo Interaction (Regression Tests)
- [ ] **W/E/R still switch gizmo modes** — Verify gizmo mode changes without Shift since these use `egui::Context::input()` not `InputManager`.
- [ ] **Left-click drag on gizmo handles works** — Select an object, drag a gizmo handle with left-click; verify transform updates.
- [ ] **Gizmo handles don't conflict with camera** — With an object selected, left-click drag on a gizmo handle rotates/scale/translates the object, NOT the camera.
