# Rustix — Startup Crash Vector Analysis

Full analysis of all potential crash, panic, UB, and silent-failure vectors
during application startup (before the first frame renders).

---

## Severity Summary

| Severity | Count |
|----------|-------|
| **CRASH/PANIC** (expect/unwrap/panic) | **22** |
| **POTENTIAL UB** (unsafe code) | **7** |
| **SILENT FAILURE** (.ok() swallowing errors) | **13** |
| **LEAK** | **1** |
| **PERFORMANCE** | **1** |

---

## 1. CRASH / PANIC VECTORS

### 1.1 Event Loop & Window

| Line | Code | Trigger |
|------|------|---------|
| `main.rs:475` | `EventLoop::new().expect("event loop")` | No display server / winit backend missing |
| `main.rs:477` | `WindowHandle::new(...).expect("window")` | winit window creation fails |
| `main.rs:498-499` | `.get_mut(&FontFamily::*).unwrap()` x2 | Font family missing from egui defaults (API change) |
| `main.rs:516` | `Renderer::new(&rc).expect("renderer")` | No Vulkan driver, missing extensions, no GPU |
| `main.rs:517` | `renderer.init_surface(...).expect("surf")` | Surface creation fails (Wayland/X11 mismatch) |
| `main.rs:519` | `EguiVulkanRenderer::new(...).expect("egui")` | Shader compile, pipeline, or desc pool fails |
| `main.rs:626` | `.begin_command_buffer(...).unwrap()` | Command buffer begin fails (pool exhaustion) |

### 1.2 Renderer Internals

| Line | Code | Trigger |
|------|------|---------|
| `lib.rs:87` | `.create_fence(...).expect("fence")` x3 | Fence creation fails |
| `swapchain.rs:166` | `.create_image_view(...).expect(...)` | Driver fails image view creation |
| `swapchain.rs:211,217` | `.create_semaphore(...).expect(...)` x2 | Driver fails semaphore creation |
| `device.rs:152,154` | `queue_families.graphics.unwrap()` / `.present.unwrap()` | Raw unwrap on optional queue family |
| `shader.rs:29` | `CString::new("main").unwrap()` | Theoretical null byte in "main" |
| `shader.rs:50` | `panic!("unsupported stage")` | Non-VERTEX/FRAGMENT stage passed |
| `pipeline.rs:81` | `.remove(0)` after `create_graphics_pipelines` | Empty vec (theoretical, driver bug) |
| `swapchain.rs:328` | `formats[0].format` | Empty formats vec (broken driver) |

### 1.3 Memory / Buffer

| Line | Code | Trigger |
|------|------|---------|
| `memory.rs:142` | `.expect("unmapped GPU buffer")` in `write_at` | Writing to GpuOnly (non-mapped) buffer |
| `memory.rs:143` | `assert!(offset + data.len() <= size)` | Write past buffer bounds |
| `memory.rs:157-159` | Same for `read()` | Same |
| `memory.rs:219,228,237` | `.unwrap()` calls in StagingBufferPool | Called before `init()` |
| `memory.rs:273` | `.into_iter().next().unwrap()` | Empty command buffer allocation |

### 1.4 Index Out of Bounds

| Line | Code | Trigger |
|------|------|---------|
| `swapchain.rs:225` | `semaphores[sem_idx]` | Called before `create_sync_objects()` |
| `swapchain.rs:287` | `image_views[current_image_index]` | Index out of bounds (race or pre-init) |
| `gltf_loader.rs:51` | `normals[i.min(normals.len() - 1)]` | Underflow if `normals.len() == 0` |

---

## 2. POTENTIAL UB (Unsafe Code)

| File:Line | Issue | Risk |
|-----------|-------|------|
| `memory.rs:73` | `(*self.device).destroy_buffer(...)` in GpuBuffer::Drop — raw `*const ash::Device` could dangle if Arc<GpuDevice> is dropped first | Use-after-free |
| `shader.rs:17` | Same pattern in ShaderModule::Drop | Same |
| `lib.rs:484-487` | Same in GpuTexture::Drop | Same |
| `lib.rs:501-503` | Same in DepthBuffer::Drop | Same |
| `ui_renderer.rs:248-263` | Same in EguiVulkanRenderer::Drop | Same |
| `device.rs:239` | `&*(&name[..end] as *const [i8] as *const [u8])` transmutes `[i8]` to `[u8]` via pointer cast | Type-punning UB |
| `main.rs:830` | `scene_descriptor_set.unwrap_or_default()` — binds `VK_NULL_HANDLE` descriptor set when set is None | **Runtime UB / driver crash** |

---

## 3. SILENT FAILURES (.ok() swallowing errors)

| Line | What it hides |
|------|---------------|
| `main.rs:619` | Swapchain recreate fails after window resize |
| `main.rs:621` | Depth buffer creation fails on resize |
| `main.rs:624` | `begin_frame()` error silently skips frame |
| `main.rs:654` | 3D pipeline creation fails — no 3D scene rendered |
| `main.rs:655` | Descriptor pool allocation fails |
| `main.rs:661` | Descriptor set allocation fails |
| `main.rs:664` | Scene UBO allocation fails |
| `main.rs:680` | 2D descriptor set allocation fails |
| `main.rs:685` | 2D UBO allocation fails |
| `main.rs:694` | Quad vertex buffer allocation fails |
| `main.rs:711` | 2D texture creation fails |
| `main.rs:630` | Cube mesh GLB load fails — no default mesh |
| `main.rs:919` | `end_frame()` errors silently discarded |

---

## 4. PERFORMANCE

| Line | Issue |
|------|-------|
| `lib.rs:271` | `update_texture_pixels` allocates new staging buffer + one-time command buffer on every font atlas update |

---

## 5. LEAK

| Line | Issue |
|------|-------|
| `swapchain.rs:297-300` | Empty `Drop for Swapchain` — if dropped without `Renderer::Drop`, all Vulkan resources leak |

---

## 6. FORMAT COMPATIBILITY

| Line | Issue |
|------|-------|
| `pipeline.rs:70` | Hardcoded `D32_SFLOAT` depth format — may not be supported on mobile/embedded GPUs |
| `swapchain.rs:328` | Falls back to `formats[0]` when preferred SRGB/UNORM formats unavailable — panics if empty |

---

## RECOMMENDED FIX PRIORITY

1. **`main.rs:830`** — Null descriptor set binding (runtime UB, #1 risk)
2. **`.expect()` proliferation** — Convert 22 crash sites to proper error handling with user-visible error messages
3. **Raw pointer Drop impls** — Replace `*const ash::Device` with `Arc<ash::Device>` in GpuBuffer, ShaderModule, GpuTexture, DepthBuffer
4. **`.ok()` error logging** — Add `tracing::error!()` at all 13 silent-failure sites
5. **`device.rs:239`** — Fix `[i8]` to `[u8]` transmute UB
6. **`gltf_loader.rs:51`** — Guard against empty mesh (usize underflow)
7. **Depth format fallback** — Query supported depth formats, fall back to D24/D16
