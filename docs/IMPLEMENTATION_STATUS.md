# Rustix Engine — Implementation Status

**Date:** 2026-05-24  
**Rust:** 1.95.0 stable  
**Status:** Phase 0 complete + Phase 0.5 (Editor UI) in progress

---

## What Compiles

```
$ cargo check
    Finished dev profile [optimized + debuginfo] target(s) in 0.76s
```

All 17 workspace crates compile without errors.

---

## Implemented Crates (4 of 17)

### `rustix-core` (crates/core) — COMPLETE

| Module | File | Status | What it provides |
|--------|------|--------|-----------------|
| ECS | `ecs.rs` | Done | `hecs` re-export + `StageLabel`, `BoxedSystem`, `Schedule`, `SystemIdGenerator` |
| Jobs | `job.rs` | Done | `JobSystem` (rayon thread pool) + `JobSystemConfig` |
| Math | `math.rs` | Done | `glam` re-export + `Aabb`, `Sphere`, `Frustum`, `Plane`, `Ray`, `Color`, `lerp`/`smoothstep` |
| Memory | `memory.rs` | Done | `FrameAllocator` (bump), `PoolAllocator`, `FrameMemory`, `Aligned<T>` (64-byte) |
| Diagnostics | `diagnostics.rs` | Done | `tracing` init + `LogConfig` + `profile_scope!`/`profile_frame!` macros |
| Config | `config.rs` | Done | `EngineConfig` (TOML), `WindowConfig`, `RenderConfig`, `JobSystemConfig`, layered merging |

**Deps:** `hecs 0.11`, `glam 0.29`, `rayon 1.10`, `tracing 0.1`, `tracing-subscriber 0.3`, `parking_lot 0.12`, `serde 1`, `toml 0.8`, `dirs 6`

### `rustix-platform` (crates/platform) — COMPLETE

| Module | File | Status | What it provides |
|--------|------|--------|-----------------|
| Window | `window.rs` | Done | `WindowHandle` wrapping `winit 0.30` `Window`, raw handles access |
| Input | `input.rs` | Done | `InputManager` + `KeyboardState`/`MouseState` + winit event forwarding |

**Features enabled:** Wayland, X11, rwh_06 (raw window handles)  
**Input coverage:** Keyboard (full keycode mapping), Mouse (position/delta/scroll/buttons), Gamepad (stub)

### `rustix-render` (crates/render) — COMPLETE

| Module | File | Status | What it provides |
|--------|------|--------|-----------------|
| Instance | `instance.rs` | Done | Vulkan instance + debug messenger + NVIDIA preference |
| Device | `device.rs` | Done | Physical device scoring, logical device, queue families, pipeline cache |
| Surface | `surface.rs` | Done | Wayland + Xlib + Xcb surface creation |
| Swapchain | `swapchain.rs` | Done | Triple-buffered swapchain, mailbox present, semaphore sync |
| Shader | `shader.rs` | Done | SPIR-V shader modules + built-in triangle vertex/fragment shaders |
| Pipeline | `pipeline.rs` | Done | Dynamic rendering pipeline (no RenderPass objects) |
| lib | `lib.rs` | Done | `Renderer` struct: init, begin/end frame, draw triangle, `create_texture` |

**Vulkan features enabled:** VK_KHR_dynamic_rendering, VK_KHR_swapchain  
**Device selection:** NVIDIA (0x10DE) = +500, AMD (0x1002) = +300, Intel (0x8086) = +100, discrete = +1000

### `rustix-engine` (engine/) — COMPLETE

| Module | File | Status | What it provides |
|--------|------|--------|-----------------|
| Plugin | `plugin.rs` | Done | `Plugin` trait + `AppBuilder` |
| Schedule | `schedule.rs` | Done | Stage-ordered system scheduling |
| App | `app.rs` | Done | `App` struct wrapping ECS world + jobs + memory + config |

### `rustix-runtime` (apps/runtime) — EDITOR UI IN PROGRESS

Binary entry point with egui-based editor overlay:

- [x] egui Vulkan renderer (custom backend, WGSL fragment shader, separate texture+sampler)
- [x] Font rendering with correct coordinate system (Y-down throughout)
- [x] Native file dialogs via `rfd` for project open/create
- [x] Recent project tracking (in-memory, max 10, deduplicated) - now persisted to disk via `dirs` crate
- [x] Startup screen: "Project Hub" with recent projects + New/Open buttons
- [x] Editor screen: menu bar, hierarchy, inspector, console, scene view panels
- [x] Project switching (Back to Project Hub from File menu)
- [x] ECS entity integration in hierarchy panel with `Transform` and `Name` components
- [ ] Offscreen 3D scene rendering in scene view panel
- [ ] Real log capture via tracing → console panel
- [ ] Asset browsing
- [ ] Project serialization (save/load `.rustixproj`) - find_project_dir added
- [ ] Window resize handling

---

## Stub Crates (13 of 17)

These compile but contain no implementation:
`asset`, `physics`, `audio`, `animation`, `networking`, `scripting`, `ui` (partial widgets), `ai`, `terrain`, `world`, `editor`

### `rustix-ui` (crates/ui) — PARTIAL
- Immediate mode UI context with draw list
- Basic widgets: `button`, `slider`, `label` (placeholder rect), `vstack`, `center`
- Text rendering not yet implemented (placeholder colored rects)

---

## API Migration Notes

### ash 0.38 Breaking Changes
- Extension loaders moved: `ash::extensions::khr::Swapchain` → `ash::khr::swapchain::Device`
- Function access via `fp()` struct: `loader.fp().destroy_surface_khr(...)`
- Function names have `_khr` suffix in fp struct fields
- `Queue::submit()` removed → use `Device::queue_submit(queue, ...)`
- `PhysicalDeviceLimits::max_image_dimension_2d` → `max_image_dimension2_d`
- `DebugUtilsMessengerCreateInfoEXT::user_callback()` → `pfn_user_callback(Some(...))`
- `DebugUtilsMessengerCallbackDataEXT::message_as_str()` → manual `CStr::from_ptr(data.p_message)`

### winit 0.30 Breaking Changes
- `WindowBuilder` removed → use `WindowAttributes::default().with_*(...)`
- `Event::RedrawRequested` → `WindowEvent::RedrawRequested` (moved to window event)
- `primary_monitor()` on `ActiveEventLoop` only (not `EventLoop`)
- `create_window()` deprecated on `EventLoop` (use `ActiveEventLoop`)
- Raw handle traits gated behind `rwh_06` feature flag
- `PhysicalKey::to_scancode()` removed → use `PhysicalKey::Code()`

### hecs 0.11
- `ViewMut`, `QueryItem`, `QueryOneOf`, `Components` removed from public API
- World resource pattern requires `query::<&mut T>()` iteration

---

## What Phase 0 Delivered

- [x] 17-crate Cargo workspace with strict dependency layering
- [x] ECS world (hecs) with system scheduling infrastructure
- [x] Job system (rayon) with configurable thread count
- [x] Frame allocator + pool allocator for cache-friendly memory
- [x] Math library (glam + custom AABB/Sphere/Frustum/Ray/Color)
- [x] Structured logging (tracing) with Tracy profiling hooks
- [x] Window creation (Wayland + X11 via winit 0.30)
- [x] Input manager (keyboard + mouse + gamepad stubs)
- [x] Vulkan 1.3 renderer (ash 0.38) with NVIDIA device preference
- [x] Dynamic rendering pipeline (no RenderPass)
- [x] Triple-buffered swapchain with mailbox present mode
- [x] 120Hz fixed update + variable render game loop
- [x] Plugin trait + AppBuilder pattern
- [x] TOML configuration with layered merging

## What Phase 0.5 (Editor UI) Delivered So Far

- [x] Custom egui Vulkan renderer (WGSL shader, separate image+sampler bindings)
- [x] Correct Y-down coordinate system for egui → Vulkan mapping
- [x] Native file dialogs via `rfd`
- [x] Project Hub startup screen with recent projects
- [x] Editor screen layout: menu bar, hierarchy, inspector, console, scene view
- [x] EditorCamera with orbit controls
- [x] Recent project tracking (in-memory)

---

## Next Phase: 1 (Core Rendering + Assets)

Priority order for next work:

1. **Project serialization** — Save `.rustixproj` on project create, load on open
2. **ECS → Hierarchy** — Show entities from `hecs::World` in the hierarchy panel
3. **Offscreen scene view** — Render 3D scene to framebuffer, display in egui `Image`
4. **Inspector panel** — Show/editable components for selected entity
5. **Console real logging** — tracing subscriber → ring buffer → console panel
6. **Asset browser** — List project assets in bottom panel
7. **Mesh + PBR renderer** — Real geometry instead of placeholder triangle
8. **Asset pipeline** — glTF loader, texture import, handle system
