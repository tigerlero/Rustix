# Rustix Engine — Crash Log

Log of runtime crashes, their root causes, and fixes for future reference.

---

## Crash: SIGSEGV during egui renderer initialization (SamplerCache)

**Date:** 2026-06-03  
**Commit context:** After implementing pipeline variants (`GraphicsPipelineVariantCache`)

### Symptom
`cargo run -p rustix-runtime` crashed with a segmentation fault (SIGSEGV) inside `libnvidia-glcore.so`.  
`RUST_BACKTRACE=full` and `VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation` produced no helpful output — a hard driver-level crash.

### Reproduction
Run the application normally. The crash always occurred during `EguiVulkanRenderer::new`, specifically at:
```
#3  rustix_render::sampler_cache::SamplerCache::get_or_create
#4  rustix_render::renderer::Renderer::create_texture
#5  rustix_runtime::ui_renderer::EguiVulkanRenderer::new
```

### Root Cause
`GpuDevice` stored `ash::Device` by value. During `GpuDevice::new()`, `SamplerCache` was initialized with `&logical` (a raw pointer to the local variable). After `GpuDevice::new` returned, `logical` moved into the `GpuDevice` struct, then again into an `Arc<>` in `Renderer`. The raw pointer inside `SamplerCache` became dangling — pointing to a stale stack address.

When `create_texture()` later called `sampler_cache().get_or_create()`, the dereferenced `ash::Device` was garbage, causing a segfault inside the NVIDIA driver during `vkCreateSampler`.

### Fix
Changed `GpuDevice.logical` from `ash::Device` to `Box<ash::Device>` in `crates/render/src/device.rs`:

```rust
pub struct GpuDevice {
    logical: Box<ash::Device>,  // was: logical: ash::Device
    ...
}
```

This places the Vulkan device on the heap at a **stable address**. Cache constructors now receive `&*logical`, and `logical()` returns `self.logical.as_ref()`. The pointer remains valid regardless of how `GpuDevice` is moved.

### Files Changed
- `crates/render/src/device.rs` — `logical` field boxed, `logical()` accessor updated, cache construction updated

### Prevention Notes
Caches that store raw `*const ash::Device` pointers must receive a **heap-allocated / stable** address. If the underlying `ash::Device` ever moves (e.g., into a struct, then into an `Arc`), the raw pointer becomes dangling. Always box such handles when raw pointers to them will be stored.

---

## Template

**Date:** YYYY-MM-DD  
**Commit context:** ...

### Symptom
...

### Reproduction
...

### Root Cause
...

### Fix
...

### Files Changed
...

### Prevention Notes
...

---
