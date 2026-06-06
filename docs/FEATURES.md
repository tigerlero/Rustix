# Rustix Engine — Feature Breakdown

Legend: `[x]` = implemented, `[ ]` = planned, `[~]` = partial

---

## 1. CORE (crates/core)

### 1.1 Entity Component System
- [x] Archetypal ECS via `hecs`
- [x] Component registration with type-erased storage — `ComponentRegistry` in `crates/core/src/component_registry.rs` maps `TypeId` and string names to `ComponentInfo` (size, align, vtable for default/clone/drop). `ErasedStorage` provides a dense sparse-set backed by an aligned `Vec<u8>` for each component type. `ErasedWorld` ties multiple storages together indexed by `TypeId`. 24 unit tests.
- [x] Query filters: `With`, `Without`
- [x] Dynamic bundles (runtime component addition) — `DynamicBundle` stores `(TypeId, Box<dyn Any + Send + Sync>)` pairs. `ComponentRegistry::insert_bundle` consumes a `DynamicBundle` and dispatches each component into `hecs::World` via O(1) HashMap lookup + stored `insert_fn` vtable. `add_component_by_name` / `remove_component_by_name` replace the editor's monolithic if-else chains.
- [x] Command buffers for deferred world mutation — `CommandBuffer` in `crates/core/src/command_buffer.rs` queues `Command` variants (`Spawn`, `Despawn`, `InsertBundle`, `RemoveByTypeId`, `RemoveByName`, `AddDefaultByName`). `apply(world, registry)` flushes all commands in order after systems finish. 13 unit tests.
- [x] Change detection (dirty flags per component per tick) — `ChangeTracker` in `crates/core/src/change_tracker.rs` maintains `HashMap<TypeId, HashSet<Entity>>` dirty sets. `flag<T>(entity)` / `is_changed<T>(entity)` for typed use; `flag_erased` / `is_changed_erased` for runtime dispatch. `changed_entities::<T>()` returns the full dirty set for efficient batch filtering. `clear()` resets all flags at tick boundary; `clear_type::<T>()` for selective reset. 11 unit tests.
- [x] Component grouping for cache-optimal iteration — `ComponentGroup` in `crates/core/src/component_groups.rs` defines named sets of component `TypeId`s that are commonly accessed together. `GroupRegistry` stores groups and provides pre-warming hints. `spawn_group(world, registry, bundle)` ensures archetype creation happens in a single step. 8 unit tests.
- [x] Multi-world support (game world, editor world, preview world) — `WorldRegistry` in `crates/core/src/world_registry.rs` stores named `hecs::World` instances with an active-world pointer. `create` / `create_inactive` / `destroy` for lifecycle; `set_active` / `active_mut` for context switching. `spawn_active` convenience for the hot path. `EntityMapping` provides bidirectional entity translation between worlds for editor/preview sync. 14 unit tests.

### 1.2 Job / Task System
- [x] Rayon-based work-stealing thread pool — `JobSystem` in `crates/core/src/job.rs` wraps a `rayon::ThreadPool` with configurable thread count, work-stealing queue depth, and thread stack size. `install(op)` runs closures on the pool and returns results. `for_each` and `join` helpers for fork-join parallelism. `thread_count()` and `rebuild(config)` support dynamic resizing.
- [x] Explicit task graph with dependency edges — `TaskGraph` in `crates/core/src/task_graph.rs` is a DAG of `TaskNode`s with `add_task(name, func)` and `add_dependency(before, after)`. `topo_sort()` produces a valid ordering via Kahn's algorithm with O(V+E) complexity. `execute(pool)` runs each frontier in parallel via `rayon::scope`, respecting all dependency edges. Cycle detection via DFS prevents deadlocks. 11 unit tests.
- [x] Fork-join parallelism API — `JobSystem::join(left, right)` splits work recursively with work-stealing. `for_each(slice, op)` parallelizes iteration over contiguous data. Built on `rayon` so the engine gets adaptive task splitting and thread-local work queues for free.
- [x] Thread affinity (pinning to physical cores on Linux) — `pthread_setaffinity_np` in `JobSystem::new`, worker `i` pinned to CPU `i % num_cpus`
- [x] Task priorities (high for render, medium for gameplay, low for streaming) — `PriorityTaskSystem` in `crates/core/src/task_priority.rs` spawns dedicated worker threads that drain three `Mutex<Vec>` queues in strict priority order (high → medium → low). `submit(priority, func)` enqueues work; `install(priority, func)` blocks on the result. `wait_for_all()` spin-yields until the pending counter reaches zero. Workers named `rx-priority-N` for debugging. 8 unit tests.
- [x] Job profiling (Tracy integration per task) — `tracy_client::span!()` zones wrap every task in `TaskGraph::execute` and `PriorityTaskSystem::worker_loop`, gated by `#[cfg(feature = "profiling")]`. `PriorityTaskSystem` stores `(name, closure)` pairs via `submit_named(priority, name, func)` so Tracy shows per-task names (e.g. "physics", "cull", "render"). `task_graph.rs` captures task names from `TaskNode` and emits zones inside `rayon::scope` spawns. The `profiling` feature also enables `profile_scope!` and `profile_frame!` macros in `diagnostics.rs`. 10 unit tests (including named variants).
- [x] Dynamic thread count (respond to system load) — `PriorityTaskSystem::resize(new_count)` can grow (spawn new workers) or shrink (signal idle threads to exit via CAS on an `excess` counter). `JobSystem::rebuild(config)` recreates the rayon pool with a different thread count. `SystemMonitor` in `crates/core/src/system_monitor.rs` reads `/proc/stat` on Linux to compute CPU usage (0-1). `recommended_threads(current, cpu_usage, min, max)` linearly interpolates between `max` (idle) and `min` (fully loaded). 15 unit tests + 1 doc test.

### 1.3 Memory Management
- [x] Frame allocator (per-frame bump allocation, O(1) reset) — `FrameAllocator` in `crates/core/src/memory.rs` is an atomic-bump allocator over a pre-allocated `Vec<u8>`. `allocate(layout)` CAS-advances a cursor; `reset()` sets it back to zero in a single atomic write. `FrameMemory` provides typed convenience helpers `alloc<T>` and `alloc_slice<T>`. 4 unit tests.
- [x] Pool allocator (fixed-size object reuse) — `PoolAllocator` in `crates/core/src/memory.rs` manages a `Mutex<Vec<*mut u8>>` free list and a `Mutex<Vec<Vec<u8>>>` chunk store. `alloc()` pops from the free list or allocates a new chunk; `free(ptr)` pushes back for reuse. Eliminates per-object allocation overhead for ECS components and particles. 4 unit tests.
- [x] Thread-local arenas (reduce contention) — `ThreadLocalArena` in `crates/core/src/thread_local_arena.rs` pre-allocates one `FrameAllocator` per thread. `thread_local!` storage caches a raw pointer to the thread's bound arena so the fast path (`allocate`) is entirely lock-free. `reset_all()` iterates all arenas and resets their cursors at frame boundary. Cross-thread allocation contention drops to zero. 6 unit tests.
- [x] Cache-line aligned allocations (avoid false sharing, align 64)
- [x] Memory tracker (leak detection, allocation statistics) — `MemoryTracker` in `crates/core/src/memory_tracker.rs` records every `track_alloc(ptr, layout, label)` / `track_free(ptr)` pair in a `Mutex<HashMap<usize, AllocationRecord>>`. Atomics track `total_allocated`, `total_freed`, `current_used`, and `peak_used` (CAS loop for peak). `leak_report()` dumps all unfreed allocations with their size, alignment, and label. `GLOBAL_MEMORY_TRACKER` via `std::sync::LazyLock` provides a process-wide instance. 8 unit tests.
- [x] Custom allocators for ECS component storage (SoA layout) — `SoAStorage` in `crates/core/src/soa_storage.rs` stores each component field in its own `AlignedVec` (system-allocated, properly aligned buffer). `insert(entity, component_bytes)` copies field data into separate contiguous buffers. `remove(entity)` uses swap-remove to keep buffers dense. `field_slice::<T>(index)` returns a typed slice for SIMD-friendly iteration. `SoARegistry` manages named storage layouts. 9 unit tests + 1 doc test.
- [x] GPU staging buffer allocator (coherent, mapped, ring-buffer) — `GpuStagingRing` in `crates/core/src/gpu_staging.rs` implements a lock-free ring buffer with `head`/`tail` offsets and `VecDeque<Region>` fence tracking. `allocate(size, align)` returns an offset; `set_fence_on_last(fence)` tags the region; `release_completed(fence)` reclaims contiguous completed space. Handles wrap-around automatically. `GpuStagingBuffer` in `crates/render/src/memory.rs` wraps a Vulkan `GpuBuffer` (`CpuToGpu`, `TRANSFER_SRC`, mapped) with the ring allocator for CPU → GPU uploads. 9 unit tests + 1 doc test in core.

### 1.4 Math Library
- [x] `glam` re-export: Vec2/3/4, Mat3/4, Quat, Affine3A
- [x] Bounding volumes: AABB, Sphere, Frustum
- [x] Ray structs for intersection queries
- [x] Color types (linear vs sRGB conversion)
- [x] Transform hierarchy (local → world matrix computation) — `Hierarchy` in `crates/core/src/transform_hierarchy.rs` computes `LocalToWorld` matrices from `Transform` (local translation/rotation/scale) and `Parent` components in one BFS pass from roots. `update_local_to_world(world)` traverses the tree breadth-first so children are computed after parents. `set_parent(world, entity, parent)` rejects self-parenting and cycle-inducing changes. `topo_order(world)` returns entities in topological order. `LocalToWorld` caches the world matrix for the render loop. 11 unit tests.
- [x] Interpolation: lerp, smoothstep, smootherstep

### 1.5 Configuration
- [x] TOML-based engine configuration
- [x] Runtime config reload (monitor config file for changes) — `ConfigWatcher` in `crates/core/src/config.rs` polls a TOML config file by comparing `SystemTime` mtime on each `update()` call. Lightweight polling (default 1s interval) avoids OS-specific file watcher dependencies. The first call always loads the file so the callback receives the initial config. `set_interval()` controls polling rate; `request_refresh()` forces an immediate check. Missing files are handled gracefully (returns `Ok(false)`). Callback-based design lets the engine apply only the changed fields (e.g. log level, thread count) without full re-initialization. 5 unit tests.
- [x] Layered configs: default → project → user → CLI overrides
- [x] Hot-key toggles (dev mode, debug rendering, profiling) — `DevToggles` in `crates/core/src/dev_toggles.rs` is a thread-safe toggle resource (`AtomicBool` fields) that can be read from any system without locking. `HotkeyBindings` maps actions to `KeyCode`s (defaults: F1=dev mode, F2=debug render, F3=profiling). `update_toggles(toggles, input, bindings)` checks `just_pressed` edges and flips flags, emitting a `tracing::info!` log on change. `ToggleInput` trait abstracts keyboard state so the system works with any input backend. `ToggleKeyboardState` adapter provided for runtime use. 7 unit tests.

### 1.6 Diagnostics
- [x] Structured logging via `tracing`
- [x] Console output (colored, with span tracking)
- [x] JSON file logging for automated analysis — `JsonFileLayer` in `crates/core/src/diagnostics.rs` is a `tracing_subscriber::Layer` that writes each log event as a JSON Lines record to a file. Every entry contains `timestamp`, `level`, `target`, `message`, and all structured fields. Supports `i64`, `u64`, `f64`, `bool`, and string values with proper JSON escaping (quotes, newlines, backslashes). `rotate(path, max_backups)` renames the current file to `.jsonl.0` and shifts older backups, then reopens a fresh file. `LogConfig.json_file_path` controls the output path; when set, `init_logging_with_capture` wires the layer into the subscriber automatically alongside console output and optional log capture. 5 unit tests.
- [x] Log levels: error, warn, info, debug, trace
- [x] Per-crate log level filtering
- [x] Log rotation in release builds — `JsonFileLayer` auto-rotates when file size exceeds `json_max_size_mb` (default 10 MB). Keeps `json_max_backups` backups (default 3), shifting `.jsonl.N` → `.jsonl.N+1`.

---

## 2. PLATFORM (crates/platform)

### 2.1 Windowing
- [x] Wayland native support (primary target for Pop!_OS) — **Linux only**
- [x] X11 fallback (xcb backend) — **Linux only**
- [x] Win32 window backend (HWND + Vulkan Win32 surface) — **Windows**: `winit` provides HWND creation; `VK_KHR_win32_surface` implemented in `crates/render/src/surface.rs` using `ash::khr::win32_surface::Instance`
- [ ] macOS window backend (NSWindow + MoltenVK + CAMetalLayer) — **macOS**
- [x] Fullscreen exclusive (when display server allows) — `FullscreenMode::Exclusive` in `crates/platform/src/window.rs` picks the best video mode on the current monitor (largest resolution, then highest refresh rate) and passes it to `winit::window::Fullscreen::Exclusive`. Falls back to borderless if no video modes are available or no monitor is detected. `FullscreenMode::Borderless` fills the screen without changing the display video mode. Both modes are applied at window creation time if `WindowConfig.fullscreen` is set, and can be toggled at runtime via `WindowHandle::set_fullscreen_mode()` and `WindowHandle::toggle_fullscreen()`.
- [x] Window resize handling (swapchain recreation)
- [x] Multiple window support (editor: N viewports)
- [x] DPI-aware scaling
- [x] Cursor mode: normal, hidden, captured, raw-delta

### 2.2 Input
- [x] Keyboard: winit fallback (evdev raw input planned) — **Linux only**
- [x] Raw keyboard input — **cross-platform via winit** (Raw Input API on Windows, evdev planned on Linux, IOKit on macOS)
- [ ] Raw keyboard input (IOKit / CGEvent) — **macOS**
- [x] Mouse: absolute + raw delta motion
- [x] Gamepad: `gilrs` integration (enabled via `--features rustix-platform/gamepad`) — **cross-platform** (Linux via libudev, Windows via Raw Input, macOS via IOKit)
- [x] Gamepad: XInput / Windows.Gaming.Input — **Windows** (handled by `gilrs` Raw Input backend)
- [x] Gamepad: IOKit GameController — **macOS** (handled by `gilrs` IOKit backend)
- [x] Input state: current frame + previous frame (for "just pressed" detection)
- [x] Input actions (abstract binding: "jump" → Space / A-button)
- [x] Bindable key remapping (config file)
- [x] Text input (IME-aware for Wayland) — **Linux only**
- [x] Text input — **cross-platform via winit** (IME-aware on all platforms)
- [ ] Text input (NSTextInputClient) — **macOS**
- [x] Touch input (surface)
- [x] Input recording + playback (testing / demos)

### 2.3 OS Abstraction
- [x] High-resolution timer (monotonic clock) — **cross-platform via std**
- [x] Thread naming (pthread_setname_np on Linux) — **Linux only**
- [x] Thread naming — **cross-platform** (`std::thread::Builder::name()` on all platforms; `pthread_setname_np` on Linux, `SetThreadDescription` fallback on Windows)
- [x] Thread naming (`pthread_setname_np` on macOS) — **macOS**: implemented in `crates/core/src/thread_priority.rs` via `libc::pthread_setname_np`.
- [x] Thread priority (SCHED_FIFO or SCHED_RR for audio/render threads) — **Linux only**
- [x] Thread priority (`SetThreadPriority`) — **Windows**: implemented in `crates/core/src/thread_priority.rs` using raw FFI to `kernel32!SetThreadPriority`. Maps `Realtime`→`THREAD_PRIORITY_TIME_CRITICAL`, `High`→`THREAD_PRIORITY_HIGHEST`, `Normal`→`THREAD_PRIORITY_NORMAL`, `Low`→`THREAD_PRIORITY_LOWEST`.
- [x] Thread priority (`pthread_set_qos_class_self_np`) — **macOS**: implemented in `crates/core/src/thread_priority.rs` via `libc::pthread_set_qos_class_self_np`. Maps `Realtime`→`QOS_CLASS_USER_INTERACTIVE`, `High`→`QOS_CLASS_USER_INITIATED`, `Normal`→`QOS_CLASS_DEFAULT`, `Low`→`QOS_CLASS_UTILITY`.
- [x] Memory mapping for asset loading — **cross-platform via `memmap2`** (mmap on Linux/macOS, `CreateFileMapping`/`MapViewOfFile` on Windows)
- [x] Memory mapping (`CreateFileMapping` / `MapViewOfFile`) — **Windows** (handled by `memmap2` crate)
- [x] File watcher (inotify on Linux, ReadDirectoryChangesW on Windows, FSEvents on macOS) — **cross-platform via notify crate**
- [x] Clipboard access — **cross-platform via arboard**
- [x] File dialog (`rfd` native picker for project open/create) — **cross-platform via rfd**

### 2.4 Cross-Platform Build / CI
- [x] Windows build (MSVC toolchain, Vulkan SDK dependency) — All platform-specific code is structurally ready. Requires `Vulkan SDK` + `MSVC` or `MinGW-w64` toolchain. `winit` handles windowing; `VK_KHR_win32_surface` is implemented; `SetThreadPriority` is wired.
- [ ] macOS build (MoltenVK bundled or system install)
- [ ] CI: GitHub Actions matrix (Linux, Windows, macOS)
- [ ] CI: Vulkan validation layer testing on Linux
- [x] Packaging: `.deb` / `.rpm` for Linux — `cargo-deb` metadata in `apps/runtime/Cargo.toml` produces `.deb` with `libvulkan1` dependency and desktop entry. `cargo-generate-rpm` metadata produces `.rpm` with `vulkan-loader` dependency. Desktop file at `apps/runtime/packaging/rustix.desktop`.
- [ ] Packaging: `.msi` / `.zip` for Windows
- [ ] Packaging: `.dmg` / `.app` bundle for macOS
- [x] Cross-compilation docs (Linux → Windows, macOS) — documented in `docs/CROSS_COMPILATION.md`. Covers MinGW-w64 and `cargo-xwin` for Windows, `cargo-zigbuild` + osxcross for macOS. Notes on Vulkan runtime dependencies (Windows Vulkan loader, MoltenVK on macOS) included.

---

## 3. RENDERER (crates/render)

### 3.1 Vulkan Backend
- [x] Instance creation with validation layers (debug)
- [x] Physical device selection (NVIDIA preference scoring)
- [x] Logical device with queue families (graphics, present)
- [x] Surface creation (Wayland/Xlib/Xcb Vulkan KHR) — **Linux only**
- [x] Surface creation (Win32 `VK_KHR_win32_surface`) — **Windows**: implemented in `crates/render/src/surface.rs`
- [ ] Surface creation (Metal `VK_EXT_metal_surface` via MoltenVK) — **macOS**
- [x] Swapchain (mailbox present mode, triple buffering)
- [x] Dynamic rendering (VK_KHR_dynamic_rendering, no RenderPass objects)
- [x] Pipeline cache (VK_KHR_pipeline_cache, in-memory)
- [x] Timeline semaphores (VK_KHR_timeline_semaphore)
- [x] Synchronization2 (VK_KHR_synchronization2)

### 3.2 GPU Memory
- [x] Device-local memory (VRAM for textures, buffers)
- [x] Host-visible coherent memory (staging, streaming UBOs)
- [x] Memory allocator (gpu-allocator integration)
- [x] Dedicated transfer queue for async upload
- [x] Staging buffer pool (ring-buffer, recycled) — see `GpuStagingRing` / `GpuStagingBuffer` in Memory Management (1.3) and `crates/render/src/memory.rs`.
- [x] GPU readback (profiling counters, occlusion queries)
- [x] UBO / SSBO allocator (ring buffer for per-frame uniform data) — implemented in `crates/render/src/memory.rs` as `GpuUniformRing`. Uses a single `UNIFORM_BUFFER | STORAGE_BUFFER` with `CpuToGpu` memory, sub-allocates aligned regions via `GpuStagingRing`, and reclaims them by fence value.
- [ ] Secondary command buffers (multi-threaded command recording)

### 3.3 Render Targets
- [x] Render target / framebuffer abstraction (color + depth attachments) — `Framebuffer` / `RenderTarget` in `crates/render/src/texture.rs` wraps a color image (`COLOR_ATTACHMENT | TRANSFER_SRC | SAMPLED`) + `DepthBuffer`. Provides `begin_rendering` / `end_rendering` for dynamic rendering, layout transitions, and GPU readback. Triple-buffered per-viewport offscreen framebuffers are used for editor viewports.
- [ ] MSAA resolve targets (for Medium/High/Ultra quality levels)
- [x] Offscreen rendering (editor viewport, post-processing) — each viewport gets its own `Framebuffer` rendered via `render_3d_scene` with `begin_scene_pass_offscreen` / `end_scene_pass_offscreen`. The color view is registered as an egui user texture for display in the UI.
- [x] HDR framebuffer (RGBA16F) + tone mapping — `HdrFramebuffer` in `crates/render/src/texture.rs` uses `R16G16B16A16_SFLOAT` color + depth attachments. A fullscreen `ToneMapPipeline` applies ACES filmic tone mapping + gamma correction (`crates/render/src/pipeline.rs`). The primary viewport renders to HDR when no offscreen framebuffer is active, then resolves to the SDR swapchain via `tone_map_pass`.
- [x] Swapchain integration (blit / present from render target) — `Renderer::blit_to_swapchain` blits any render-target image into the current swapchain image with proper layout transitions (`TRANSFER_SRC` → `TRANSFER_DST` → `PRESENT_SRC_KHR`). Convenience methods `Framebuffer::blit_to_swapchain` and `HdrFramebuffer::blit_to_swapchain` wrap this. `Renderer::end_frame` now transitions the swapchain image to `PRESENT_SRC_KHR` before `vkQueuePresentKHR`.

### 3.4 Descriptors
- [x] Bindless descriptor model (global heap)
- [x] Descriptor set layout cache
- [x] Sampler cache (reuse sampler objects)
- [x] Descriptor update batching
- [x] Descriptor set allocator (pool recycling, not per-frame pool creation) — `DescriptorSetAllocator` in `crates/render/src/descriptor_allocator.rs` maintains ready/used pools. `allocate()` grabs a pool, spills to a new one on `ERROR_OUT_OF_POOL_MEMORY`, and `reset_pools()` recycles all used pools each frame. Integrated into `Renderer` as `allocate_descriptor_set(layout)` / `reset_descriptor_pools()`.

### 3.5 Pipelines
- [x] Graphics pipeline cache (hash-based key → VkPipeline)
- [x] Compute pipeline cache
- [x] Pipeline variants (forward/deferred, quality levels)
- [x] Specialization constants (reducing shader variants) — `SpecConstantMap` in `crates/render/src/spec_constants.rs` stores `(constant_id, u32)` pairs. `ShaderModule::stage_create_info_with_specialization()` builds `vk::SpecializationInfo` arrays. `PipelineVariantKey` includes `spec_constants` so the `GraphicsPipelineVariantCache` correctly keys variants by constant values. Note: naga's GLSL frontend does not support `layout(constant_id = …)`; the built-in shaders use plain `const int` and the specialization constant infrastructure is ready for SPIR-V modules that do contain `OpSpecConstant` instructions.
- [x] Pipeline layout cache (distinct from descriptor set layout cache) — `PipelineLayoutCache` in `crates/render/src/pipeline.rs` keys `vk::PipelineLayout` handles by `(set_layouts, push_ranges)`. Integrated into `GpuDevice` alongside `DescriptorSetLayoutCache` and `SamplerCache`. All pipeline creation code (`GraphicsPipeline`, `ShadowPipeline`, `GraphicsPipeline2D`, `ToneMapPipeline`, `ComputePipelineCache`) now uses `device.pipeline_layout_cache().get_or_create()` instead of direct `vkCreatePipelineLayout` calls.

### 3.6 Shaders
- [x] GLSL source → SPIR-V via naga — `ShaderModule::from_glsl()` uses `naga::front::glsl` to parse GLSL and `naga::back::spv` to emit SPIR-V. All builtin shaders (scene PBR, shadow, tone mapping, 2D sprite) are defined as GLSL string constants and compiled at runtime. WGSL → SPIR-V via `naga::front::wgsl` is also supported.
- [x] Runtime shader compilation (editor / debug) — `ShaderModule::from_file()` loads GLSL/WGSL from disk, infers stage from extension (`.vert`/`.frag`/`.comp`), and compiles via naga at runtime. Builtin shader module provides `*_override()` variants (`vertex_shader_override()`, `fragment_shader_override()`, etc.) that search `shaders/` for file overrides before falling back to embedded source. This allows editing shaders without recompiling the engine. Shader source files for all builtin pipelines are provided in `shaders/`.
- [x] SPIR-V reflection (resource binding, push constants) — `spv_reflect` module in `crates/render/src/spv_reflect.rs` uses `naga::front::spv` to parse compiled SPIR-V and extract `ResourceBinding` (set, binding) and `AddressSpace::PushConstant` info. `ShaderReflection` provides `bindings_by_set()` to build `vk::DescriptorSetLayoutBinding` arrays, `push_constant_range()` to build `vk::PushConstantRange`, and `merge()` to combine vertex+fragment stage resources. `ShaderModule::reflect()` exposes this on any compiled shader.
- [x] Shader hot-reload (watch source files, rebuild pipelines) — `ShaderHotReloader` in `crates/render/src/hot_reload.rs` uses `notify` to watch `shaders/` (and parent directories) for `.vert`/`.frag`/`.comp`/`.glsl`/`.wgsl` changes. Each frame the app polls `Renderer::hot_reloader().take_events()` and dispatches to per-pipeline reload functions (`reload_scene_pipeline`, `reload_shadow_pipeline`, `reload_tonemap_pipeline`, `reload_2d_pipeline`) in `apps/runtime/src/init.rs`. These recompile shaders via the same `*_override()` helpers and recreate `vk::Pipeline` objects. `GraphicsPipelineVariantCache::clear()` destroys old cached pipelines so the next `get_or_create()` rebuilds with the updated SPIR-V.
- [x] Shader include system (#include resolution) — `shader_include` module in `crates/render/src/shader_include.rs` resolves `#include "..."` and `#include <...>` directives in GLSL source before passing it to naga. Paths are resolved relative to the current source file (for file-loaded shaders), then against the standard shader search paths (`shaders/`, `../shaders/`, `../../shaders`). Circular includes are detected via a per-branch `HashSet` and rejected with an error. `#line` directives are inserted around included content so that compiler error messages retain correct file/line info. `ShaderModule::from_file()` automatically enables includes, and `ShaderModule::from_glsl_with_includes()` provides the same for string source with an explicit base path.
- [x] Pre-compiled shader archive for release builds — `build.rs` in `crates/render/build.rs` scans `shaders/` at compile time, compiles every `.vert`/`.frag`/`.comp`/`.glsl` file to SPIR-V via `naga::front::glsl` + `naga::back::spv`, and generates `shader_archive_gen.rs` in `OUT_DIR`. The generated file contains a `lookup(name) -> Option<(&[u32], ShaderStage)>` function backed by static `&[u32]` slices. `crates/render/src/shader_archive.rs` re-exports this lookup. In **release** builds (`cfg!(not(debug_assertions))`) all `builtin` shader loaders (`vertex_shader()`, `fragment_shader()`, `shadow_vertex_shader()`, etc.) bypass GLSL parsing and create `ShaderModule`s directly from the archive via `ShaderModule::from_archive_name()`. In **debug** builds the embedded GLSL strings and file-override paths remain active so hot-reload and `#include` resolution still work.

### 3.7 Frame Graph
- [x] Declarative render graph
- [x] Automatic resource barriers
- [x] Transient resource memory aliasing
- [x] Render pass merging
- [x] Async compute pass scheduling
- [x] Frame graph visualization (debug overlay)

### 3.8 Rendering Features
- [x] Forward+ lighting (tiled light culling) — bindless storage buffer bindings (3 = light data SSBO, 4 = tile light list SSBO), compute shader `light_cull.comp` with 16x16 tile dispatch, screen-space AABB culling per light, and per-tile light list consumed by `pbr.frag`. Supports up to 256 dynamic point/spot lights with 32 lights per tile.
- [x] Deferred shading (GBuffer: albedo, normal, PBR params, depth) — `GBufferPipeline` / `DeferredLightingPipeline` in `crates/render/src/pipeline.rs`. GBuffer pass writes albedo (RGBA8), normals (RGBA16F), and material (RGBA8) into dedicated render targets with a shared depth buffer. The deferred lighting pass draws a full-screen triangle that samples the G-buffer via fixed bindless bindings (5-9), reconstructs world position from depth, and computes directional light + Forward+ tiled point lights. Shaders: `gbuffer.vert`/`gbuffer.frag`, `deferred.vert`/`deferred.frag`. Integrated into the frame graph with automatic layout transitions. Toggle via `use_deferred` flag in `apps/runtime/src/main.rs`.
- [x] Directional light with cascaded shadow maps (CSM) — 3 cascades with split distances computed per-frame based on camera frustum, shadow map resolution 2048. CSM UBO (binding 10) stores light view-projection matrices and cascade split distances. Three shadow map textures (bindings 11-13) sampled in `pbr.frag` and `deferred.frag` with cascade selection based on view-space depth, PCF filtering, and 0.005 shadow bias. Shadow passes render to each cascade using dynamic rendering. Integrated into frame graph with automatic layout transitions.
- [x] Point/spot lights with shadow maps — cubemap array shadow maps for up to 4 point lights (512x512 faces, binding 15) and 2D array shadow maps for up to 4 spot lights (512x512, binding 17). Point light cubemap faces rendered using 90-degree perspective projections for +X/-X/+Y/-Y/+Z/-Z. Spot light shadow matrices stored in `SpotShadowUBO` (binding 19) with view-projection and layer index params. Both sampled in `pbr.frag` and `deferred.frag` with distance comparison for point lights and projected depth comparison for spot lights. Fixed bindless bindings: cubemap texture at 15, point sampler at 16, spot array texture at 17, spot sampler at 18, spot UBO at 19. Resources: `PointShadowResources` and `SpotShadowResources` in `apps/runtime/src/render.rs`, created in `init.rs`.
- [x] PBR material system (metal-rough workflow) — Cook-Torrance GGX microfacet BRDF with Schlick Fresnel, Smith-GGX geometry/visibility, and Trowbridge-Reitz NDF. `scene::Material` component stores base_color, roughness, metallic, ao, and emissive. Push constants pass material params as vec4(roughness, metallic, ao, emissive) to both forward (`pbr.frag`) and deferred (`gbuffer.frag`/`deferred.frag`) paths. GBuffer material channel encodes roughness in R, AO in G, emissive in B. Inspector UI exposes all five material parameters with AO range 0-1 and emissive range 0-10. Default values: roughness 0.5, metallic 0.0, AO 1.0, emissive 0.0.
- [ ] HDR rendering + tonemapping
- [ ] Bloom (gaussian pyramid)
- [ ] SSAO (HBAO)
- [ ] Temporal anti-aliasing (TAA)
- [ ] Screen-space reflections (SSR)
- [ ] Volumetric fog / lighting
- [ ] Skybox / atmospheric scattering
- [ ] Instanced rendering (indirect multidraw)
- [ ] GPU-driven culling (compute shader frustum + occlusion culling)
- [ ] Mesh shaders (VK_NV_mesh_shader if available, fallback to vertex shaders)
- [ ] Ordered independent transparency (OIT)

### 3.9 Debug / Profiling
- [x] VkDebugUtils messenger with severity filtering
- [ ] RenderDoc capture trigger (F12 key)
- [ ] VkDebugUtils labeling (objects, command buffer regions)
- [ ] GPU timestamp queries (per-pass timing)
- [ ] Tracy GPU profiling zones
- [ ] Wireframe / debug overlay rendering

---

## 4. UI SYSTEM (apps/runtime + crates/ui)

### 4.1 egui Integration (apps/runtime)
- [x] Custom Vulkan renderer backend with WGSL fragment shader
- [x] Separate texture + sampler descriptor bindings
- [x] Correct Y-down coordinate system (egui → Vulkan NDC → framebuffer)
- [x] Font atlas upload and texture update each frame
- [x] Clipped primitive rendering with scissor rects
- [x] Push constants for screen size

### 4.2 Project Hub Startup Screen
- [x] Centered dialog with branding ("Rustix Engine / Project Hub")
- [x] Recent projects list with hover interaction
- [x] "New Project" button with native folder picker
- [x] "Open Project…" button with native folder picker
- [x] Recent project tracking (in-memory, max 10, dedup by path)
- [x] Empty state messaging when no recent projects
- [x] Project serialization (.rustixproj save/load)
- [x] Recent projects persistence (disk)

### 4.3 Editor Layout (Implemented)
- [x] Menu bar: File, Edit, Assets, Help, Settings + FPS counter + dirty indicator (`*`). File menu: New/Open Project, Save (`Ctrl+S` auto-saves `.rustixproj` with camera state + scene), Exit, Back to Project Hub. Edit: Undo/Redo. Assets: mesh loader, sprite editor toggle. Settings: resolution, VSync, target FPS, 2D/3D mode.
- [x] Hierarchy panel (left, 220px resizable) — full ECS entity tree with type icons (mesh, light, camera, audio, physics). Toolbar: Add Entity, Delete, Copy, Paste, Duplicate. In-place rename with `F2`. Click to select; selected entity highlighted. Shows entity name + component badges.
- [x] Inspector panel (right, resizable) — component editing for: `Transform` (position/rotation/scale drag values), `Material` (albedo color via custom HSVA popup picker + RGB inputs, metallic/roughness), `MeshComponent`, `DirectionalLight`/`PointLight`/`SpotLight` (color, intensity, range, angle), `Camera` (FOV, near/far), `AudioSource` (volume, loop, pitch, spatial), `AudioListener`, `RigidBody` (mass, body type, damping), `Collider` (shape selector: box/sphere/capsule, size), `ScriptComponent` (script path), `Parent`. All edits push to `UndoHistory`.
- [x] Console / Asset Browser (bottom, 160px resizable, tabbed) — **Console tab**: real-time log capture via `rustix_core::log_capture::get_logs()` with color-coded levels (error=red, warn=yellow, info=blue-white, debug=gray, trace=dark gray), auto-scroll to bottom, Clear button. **Asset Browser tab**: filesystem listing of project directory with file icons, Refresh button.
- [x] Scene View (central panel) — transparent frame for offscreen rendering. Displays offscreen-rendered 3D scene texture when available. Viewport rect tracked per-frame for framebuffer sizing. World-to-screen projection for overlay elements.
- [x] EditorCamera with orbit + first-person modes. Orbit: WASDQE (shift), right-drag orbit, middle-drag pan, scroll zoom. First-person: right-drag look, WASDQE move. `Space` toggles mode. Yaw/pitch clamped. Distance minimum 0.5. Camera state serialized into `.rustixproj`.

### 4.4 Editor Features (Implemented)
- [x] ECS entity tree in Hierarchy panel — live `hecs::World` query with `Name` + `Transform` display. Component-type icons via `world.query_mut::<(&Name, Option<&MeshComponent>, ...)>`.
- [x] Component editing in Inspector panel — full component reflection via typed queries + drag-value widgets. Custom color picker with HSVA 2-D picker popup + R/G/B inputs. All mutations recorded in undo history.
- [x] Offscreen 3D rendering — viewport stores rect in egui memory (`viewport_rect_0`). Offscreen texture displayed via `ui.painter().image(tex_id, ...)`. Pipeline ready; needs render target / framebuffer implementation for full scene rendering.
- [x] Real log capture — `rustix_core::log_capture` module captures `tracing` events into a ring buffer. Console panel reads and displays with level-based coloring.
- [x] Asset file listing — Asset Browser tab reads project directory via `std::fs::read_dir`, shows files with icons.
- [x] Entity selection — click in Hierarchy panel sets `selected_entity`. Click in viewport (via world-to-screen ray test) selects entity under cursor.
- [x] Gizmos (translate, rotate, scale) — toolbar with E/R/T mode buttons. Local/world space toggle. Snap toggle with configurable step size. Visual gizmo axes drawn via `ui.painter().line_segment` in viewport. Dragging updates entity transform in real time with undo batching.
- [x] Grid overlay — configurable XZ grid with major/minor line spacing, world-to-screen projected, toggleable.
- [x] Undo/redo system — `UndoHistory` in `apps/runtime/src/undo.rs` records `EditorAction` variants: `AddEntity`, `DeleteEntity`, `TransformChange`, `ComponentChange`, `Rename`. `Ctrl+Z` / `Ctrl+Y` or Edit menu. Actions store before/after snapshots for full revert.
- [x] Viewport splitting — `ViewportManager` supports up to `MAX_VIEWPORTS=4`. Primary (index 0) uses `CentralPanel`; secondary use floating `egui::Window`. Each viewport has independent camera. Add/remove via menu bar.
- [x] Project Settings dialog — modal window: resolution (W/H drag values), VSync checkbox, target FPS (30-480), 2D/3D mode selector. Changes applied on close.
- [x] Sprite editor dialog — integrated sprite editing window with animation timeline.
- [x] Audio preview in Console — play/stop buttons, waveform visualization via `WaveformViewer`, volume slider.
- [x] Confirmation dialogs — unsaved changes warning when switching projects or closing.

### 4.5 Editor Features (Planned)
- [ ] Layout persistence (panel sizes, positions, viewport arrangement saved per-project)
- [ ] Docking / panel rearrangement (drag panels to new positions)
- [ ] Full offscreen scene rendering pipeline (requires render target implementation)
- [ ] Entity multi-select + group operations
- [ ] Drag-and-drop in Hierarchy (reparent entities)
- [ ] Scene camera bookmarks / preset views
- [ ] Play mode (simulate game inside editor viewport)

### 4.6 Custom UI Framework (crates/ui)
- [x] Immediate mode UI context
- [x] Draw command list (rectangles, colored)
- [x] Button widget (with hover/interaction state)
- [x] Slider widget
- [x] Label widget (placeholder — colored rect, no real glyphs)
- [x] Layout helpers: vstack, center
- [ ] Real text rendering (glyph atlas, font rasterization)
- [ ] Image widget
- [ ] Text input
- [ ] Flexbox/grid layout

---

## 5. ASSET SYSTEM (crates/asset)

### 5.1 Asset Types
- [ ] Meshes (glTF 2.0 → .rxmesh)
- [ ] Textures (PNG, HDR, KTX2 → .rxtex)
- [ ] Materials (custom .rxmat format)
- [ ] Shaders (GLSL → SPIR-V)
- [ ] Audio (WAV, OGG, FLAC → .rxsound)
- [ ] Animation clips (.gltf animations → .rxanim)
- [ ] Skeleton definitions (.rxskel)
- [ ] Physics materials (.rxphys)
- [ ] Prefabs (entity hierarchies, .rxprefab)
- [ ] Worlds / regions (.rxregion)
- [ ] Fonts (.ttf)

### 5.2 Asset Pipeline
- [ ] Hot-reload (watch source files, reimport)
- [ ] Asset decoding on worker threads (image, mesh, audio)
- [ ] GPU upload via transfer queue
- [ ] Asset registry with reference counting
- [ ] Handle-based access (8-byte handle, not Arc)
- [ ] Asset dependencies (material depends on textures)
- [ ] Async loading (tokio runtime for IO)
- [ ] Streaming (priority-ordered load/unload)
- [ ] Asset caching (disk cache of processed assets)
- [ ] Virtual file system (VFS) for asset path resolution

### 5.3 Import Pipeline
- [ ] glTF 2.0 import (meshes, materials, animations, skeletons)
- [ ] Texture compression (BC7 / ASTC conversion)
- [ ] Mesh optimization (vertex cache reordering, stripification)
- [ ] Asset cooking (preprocessing for runtime performance)
- [ ] Dependency graph for incremental builds

---

## 6. AUDIO SYSTEM (crates/audio)

### 6.1 Audio Decoding
- [x] Multi-format decoding via `symphonia` (WAV, MP3, OGG/Vorbis, FLAC, AAC)
- [x] Pure-Rust — no system audio libs required at build time
- [x] Sample rate and channel detection from codec metadata
- [x] Raw `f32` PCM sample output for analysis/visualization
- [x] `SoundInstance` with decoded sample access (`.decoded_samples()`)

### 6.2 Audio Playback
- [x] Hardware playback via `rodio` (optional, feature-gated: `audio-playback`)
- [x] `AudioEngine::new()` always succeeds — graceful fallback if no device
- [x] `is_playback_available()` runtime check
- [x] Play/stop/pause/volume per `SoundInstance`
- [x] Looping support
- [x] Master volume control
- [ ] Spatial audio (distance attenuation, HRTF panning)
- [ ] Audio effects (reverb, EQ, compression)
- [ ] Streaming for long files

### 6.3 ECS Components
- [ ] `AudioSource` as `hecs::Component` (position, min/max distance, rolloff)
- [ ] `AudioListener` as `hecs::Component` (position, forward, up)
- [ ] `SoundPlayer` as `hecs::Component` (path, volume, looping)
- [ ] Automatic cleanup of finished instances

### 6.4 Editor Integration
- [x] Sound effects assets in `assets/sounds/` (click, beep, whoosh, thump)
- [ ] Audio file preview in Asset Browser
- [ ] Waveform visualization
- [ ] Audio source gizmos in 3D viewport

---

## 7-14. REMAINING SUBSYSTEMS

Remaining crates (`physics`, `animation`, `networking`, `scripting`, `ai`, `terrain`, `world`, `editor`) are stubs with no implementation.

---

## Feature Priority Matrix

| Feature | Phase | Effort | Impact | Status |
|---------|-------|--------|--------|--------|
| Core ECS | 0 | Medium | Critical | **DONE** |
| Windowing | 0 | Low | Critical | **DONE** |
| Vulkan device + swapchain | 0 | Medium | Critical | **DONE** |
| Job system | 0 | Low | Critical | **DONE** |
| Math library | 0 | Low | Critical | **DONE** |
| Input system | 0 | Low | Critical | **Partial** (kb+mouse, no gamepad/action map) |
| Logging + config | 0 | Low | Critical | **DONE** |
| egui Vulkan overlay | 0.5 | Medium | High | **DONE** |
| Project Hub / startup screen | 0.5 | Medium | High | **DONE** |
| Editor layout | 0.5 | Medium | High | **Done** (placeholder panels) |
| File dialogs | 0.5 | Low | High | **DONE** |
| Recent projects | 0.5 | Low | Medium | **DONE** |
| Project serialization | 0.5 | Low | High | **DONE** |
| Console log capture (tracing → editor) | 1 | Low | High | **—** |
| ECS → Hierarchy/Inspector | 1 | Medium | High | **—** |
| Offscreen scene rendering | 1 | Medium | High | **—** |
| Render target / framebuffer management | 1 | Medium | Critical | **—** |
| MSAA render targets (for quality levels) | 1 | Medium | High | **—** |
| Frame graph | 1 | High | Critical | **—** |
| Pipeline layout cache | 1 | Low | Medium | **—** |
| PBR shading | 1 | High | Critical | **—** |
| Asset system (handles + async loading) | 1 | High | Critical | **—** |
| Physics integration | 1 | Medium | High | **—** |
| Audio | 1 | Medium | Medium | **Partial** (decode + playback done) |
| Animation | 2 | High | High | **—** |
| World streaming | 2 | High | High | **—** |
| Terrain | 2 | High | Medium | **—** |
| Shader hot-reload | 1 | Medium | High | **—** |
| GPU timestamp queries | 1 | Low | Medium | **—** |
| Windows build (Win32 + Vulkan) | 2 | Medium | High | **Done** |
| macOS build (MoltenVK + Metal surface) | 2 | Medium | High | **—** |
| CI: GitHub Actions matrix | 2 | Low | Medium | **—** |
| RenderDoc capture trigger | 1 | Low | Low | **—** |
| UI framework | 2 | Medium | High | **Partial** (crates/ui stub) |
| Networking | 3 | Very High | High | **—** |
| AI | 3 | Medium | Medium | **—** |
| Scripting | 3 | High | Medium | **—** |
| Full editor | 4 | Very High | Low | **—** |

---

## Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Frame time (CPU) | <8ms (120+ FPS) | Tracy |
| Frame time (GPU) | <8ms at 1440p | Tracy GPU zones |
| Memory (idle) | <500 MB | OS monitor |
| Memory (full scene) | <4 GB | Memory tracker |
| Asset load time | <100ms per asset | Tracy |
| World streaming | 32 chunks/sec | Custom metric |
| Physics tick | <4ms | Tracy |
| Network RTT | <50ms local | ping |
| Network throughput | <1 Mbps per client | Bandwidth tracker |
