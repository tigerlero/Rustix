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
- [x] CI: GitHub Actions matrix (Linux, Windows, macOS) — `.github/workflows/ci.yml`.
  * Three jobs: `linux` (Ubuntu + libvulkan-dev + validation layers), `windows` (Windows-latest), `macos` (macOS-latest + MoltenVK via Homebrew).
  * Steps: checkout, install Rust, cache cargo, build, test, clippy, format check.
- [x] CI: Vulkan validation layer testing on Linux — included in the `linux` job which installs `vulkan-validationlayers-dev`.
- [x] Packaging: `.deb` / `.rpm` for Linux — `cargo-deb` metadata in `apps/runtime/Cargo.toml` produces `.deb` with `libvulkan1` dependency and desktop entry. `cargo-generate-rpm` metadata produces `.rpm` with `vulkan-loader` dependency. Desktop file at `apps/runtime/packaging/rustix.desktop`.
- [x] Packaging: `.msi` / `.zip` for Windows — `scripts/package-windows.ps1`.
  * PowerShell script builds release workspace, copies binary + DLLs + assets + shaders into a folder, then compresses to `.zip`.
- [x] Packaging: `.dmg` / `.app` bundle for macOS — `scripts/package-macos.sh`.
  * Bash script builds release workspace, creates `.app` bundle with `Contents/MacOS` binary and `Contents/Resources` assets, generates `Info.plist`.
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
- [x] Secondary command buffers (multi-threaded command recording) — `crates/render/src/secondary_cmd.rs`.
  * `SecondaryCommandPool` — allocates `vk::CommandBufferLevel::SECONDARY` buffers.
  * `begin_secondary` / `end_secondary` / `execute_secondary` — inline render pass continuation support.

### 3.3 Render Targets
- [x] Render target / framebuffer abstraction (color + depth attachments) — `Framebuffer` / `RenderTarget` in `crates/render/src/texture.rs` wraps a color image (`COLOR_ATTACHMENT | TRANSFER_SRC | SAMPLED`) + `DepthBuffer`. Provides `begin_rendering` / `end_rendering` for dynamic rendering, layout transitions, and GPU readback. Triple-buffered per-viewport offscreen framebuffers are used for editor viewports.
- [x] MSAA resolve targets (for Medium/High/Ultra quality levels) — `crates/render/src/msaa.rs`.
  * `MsaaSamples` — `Off`, `X2`, `X4`, `X8` mapping to Vulkan sample counts.
  * `RenderQuality` — `Low`, `Medium`, `High`, `Ultra` presets.
  * `MsaaRenderTarget` — color image + optional resolve image; `needs_resolve()` predicate.
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
- [x] HDR rendering + tonemapping (RGBA16F framebuffer, ACES filmic + gamma)
- [x] Bloom (gaussian pyramid) — Extract pass thresholds HDR into `R16G16B16A16_SFLOAT` mip0, then 3-level downsample chain (`bloom_down.frag`) followed by upsample with bilinear tent (`bloom_up.frag`). Final bloom is blended back into HDR via the tonemapping shader. `BloomPipeline` / `BloomResources` manage the mip pyramid (mip0a, mip1a, mip2a, mip3, mip2b, mip1b, mip0b). Push constants pass threshold and intensity. Shaders: `shaders/bloom.vert`, `bloom_extract.frag`, `bloom_down.frag`, `bloom_up.frag`. UI threshold/intensity sliders in Post-Process window.
- [x] SSAO (HBAO) — Half-resolution AO generation pass samples depth texture, reconstructs view-space position and normals, then computes occlusion via a 16-tap poisson disk with random rotation per pixel. 9-tap Gaussian blur pass smooths noise. Blurred AO is multiplied into HDR color in the tonemapping shader. `SsaoResources` / `BloomPipeline` at `shaders/ssao.vert`, `ssao.frag`, `ssao_blur.frag`. `apps/runtime/src/render/ssao.rs`. UI controls for radius, bias, power, and intensity in Post-Process window.
- [x] Temporal anti-aliasing (TAA) — TAA resolve pass reads current HDR, history, and depth. Reprojects UVs using the previous frame's `view_proj` matrix (stored per-frame in `AppState`). 3×3 neighborhood clamping clips history to current-frame min/max to reduce ghosting. Motion and off-screen rejection reduce the blend factor (default 0.1) toward 0. Resolved output is written to a `R16G16B16A16_SFLOAT` target; after the frame graph completes, `vkCmdCopyImage` copies resolved → history for the next frame. First frame forces blend to 0 to avoid uninitialized-history artifacts. `TaaPipeline` / `TaaResources` at `shaders/taa.vert`, `taa.frag`. `apps/runtime/src/render/taa.rs`. UI controls for enabled flag and blend factor in Post-Process window.
- [x] Screen-space reflections (SSR) — Ray-marched screen-space reflections sampling depth, HDR color, and GBuffer normals. Reconstructs world-space position from depth, reflects view vector about surface normal, then ray-marches in clip space with adaptive step count based on screen-space distance. Intersection detected when scene depth crosses ray depth. Screen-edge and distance fade with configurable max steps, stride, and max distance. `SsrPipeline` / `SsrResources` at `shaders/ssr.vert`, `ssr.frag`. `apps/runtime/src/render/ssr.rs`. Active only when deferred shading (GBuffer) is available since it needs the normal texture. UI controls for enabled flag, steps, stride, and max distance in Post-Process window. Integrated into frame graph after scene pass; TAA, bloom, and tonemap sample from SSR output when active.
- [x] Volumetric fog / lighting — Ray-marched volumetric fog pass computes in-scattered light along the view ray using procedural 3D noise for density variation and exponential height falloff. Samples depth to stop marching at opaque surfaces. Directional light phase function (Henyey-Greenstein approximation) for sun shafts. `VolumetricFogPipeline` / `VolumetricFogResources` at `shaders/volumetric_fog.vert`, `volumetric_fog.frag`. `apps/runtime/src/render/volumetric_fog.rs`. Integrated into frame graph after scene pass; SSR, TAA, bloom, and tonemap sample from fog output when active. UI controls for enabled flag, density, scattering, height falloff, max distance, steps, and sun intensity in Post-Process window.
- [x] Skybox / atmospheric scattering — Procedural sky rendered in a fullscreen pass that reads scene depth and only draws where geometry is absent (depth at far plane). Rayleigh scattering phase function approximates blue sky color based on view angle relative to zenith. Mie scattering adds a sun disc and glow using a smoothstep and power function. Horizon glow adds warm color band. Reconstructs world-space ray direction from inverse view-projection matrix. `SkyboxPipeline` / `SkyboxResources` at `shaders/skybox.vert`, `skybox.frag`. `apps/runtime/src/render/skybox.rs`. Integrated into frame graph after scene pass; SSR, TAA, bloom, and tonemap sample from skybox output when active. UI controls for enabled flag, Rayleigh coefficient, Mie coefficient, zenith shift, and exposure in Post-Process window.
- [x] Instanced rendering (indirect multidraw) — Groups visible scene entities by mesh, builds per-frame GPU instance buffers (model matrix + base color + material params, 96 bytes/instance) and indirect draw buffers (`VkDrawIndexedIndirectCommand`). Uses `vkCmdDrawIndexedIndirect` with instance count > 1 for each unique mesh batch. `InstancedGraphicsPipeline` / `InstancedGBufferPipeline` add a second vertex binding (rate=PER_INSTANCE, locations 2-7) alongside the existing mesh vertex binding. `InstancedMeshBatcher` handles frustum culling, grouping, and buffer uploads. New instanced shaders: `pbr_instanced.vert`, `pbr_instanced.frag`, `gbuffer_instanced.vert`, `gbuffer_instanced.frag`. `apps/runtime/src/render/instanced.rs`. Forward path integrated in `hdr_graph.rs`; deferred path supported via `InstancedGBufferPipeline`. Toggle via "Instanced Rendering" checkbox in Post-Process window. Falls back to per-entity push-constant draws when disabled.
- [x] GPU-driven culling (compute shader frustum + occlusion culling) — Two-dispatch compute pipeline for GPU-only instance culling. **Pass 1** (`cull_instances.comp`): one thread per instance transforms mesh AABB to world space and tests against 6 frustum planes; visible instances atomically increment per-batch counters. **Pass 2** (`gen_draw_cmds.comp`): one thread per batch reads the counter and writes a `VkDrawIndexedIndirectCommand` directly to GPU. Scene pass uses `vkCmdDrawIndexedIndirect` with the GPU-generated draw command buffer. `GpuCullingResources` manages input buffer (`CullInstance` with transform + AABB), counter buffer, draw command buffer, and batch info buffer. Separate descriptor sets and compute pipelines for each dispatch. Integrated into `hdr_graph.rs` before the scene pass; compute passes run on the async compute queue and the graphics queue waits via semaphore. Toggle via "GPU Culling" checkbox in Post-Process window (requires Instanced Rendering enabled). Shaders support hot-reload via compute pipeline cache clear. `apps/runtime/src/render/gpu_culling.rs`.
- [x] Mesh shaders (VK_NV_mesh_shader if available, fallback to vertex shaders) — Detects `VK_NV_mesh_shader` device extension at startup and enables mesh shader path when supported and toggled on. Mesh shader (`pbr_mesh.mesh`) procedurally generates a cube per entity directly on the GPU using `gl_MeshVerticesNV` and `gl_PrimitiveIndicesNV`, replacing vertex shader + index buffer. Reuses existing `pbr_instanced.frag` fragment shader with matching varying locations (world position, normal, light-space position, base color, material). Push constants pass model matrix, base color, material, and directional light data (128 bytes total). `MeshShaderPipeline` omits vertex input state and uses `MESH_NV` + `FRAGMENT` stages with dynamic rendering. `draw_mesh_tasks_in_pass` dispatches one task per visible entity via `vkCmdDrawMeshTasksNV`. Integrated into `hdr_graph.rs` scene pass with automatic fallback to instanced or per-entity rendering when disabled or unsupported. Toggle via "Mesh Shaders (NV)" checkbox in Post-Process window (disabled/greyed out when extension unavailable). Hot-reload support for `pbr_mesh.mesh`. `crates/render/src/pipeline.rs`, `crates/render/src/renderer/draw.rs`, `shaders/pbr_mesh.mesh`.
- [x] Ordered independent transparency (OIT) — Weighted blended OIT with accumulation + revealage passes. **Accumulate pass** (`oit_accumulate.vert`/`oit_accumulate.frag`) renders transparent entities (base_color.a < 1.0) to dual `R16G16B16A16_SFLOAT` accumulation and `R16_SFLOAT` revealage targets using additive blending. PBR lighting (Cook-Torrance GGX) identical to forward scene pass. Depth test enabled read-only. **Composite pass** (`oit_composite.vert`/`oit_composite.frag`) fullscreen triangle reads accumulation, revealage, and opaque HDR textures via `texelFetch`, computes `color = accum.rgb / max(accum.a, epsilon)` and `alpha = 1.0 - reveal`, then blends transparent over opaque. `OitResources` manages three persistent Vulkan images/views. `OitAccumulatePipeline` / `OitCompositePipeline` in `crates/render/src/pipeline.rs`. Integrated into `hdr_graph.rs` after scene pass; SSR, TAA, bloom, and tonemap automatically sample from OIT composite output when enabled. UI toggle "Enabled" in Post-Process window. Hot-reload support for all four OIT shaders. `apps/runtime/src/render/oit.rs`.

### 3.9 Debug / Profiling
- [x] VkDebugUtils messenger with severity filtering
- [x] RenderDoc capture trigger (F12 key) — `crates/render/src/renderdoc.rs`.
  * `RenderDocCapture` — `enabled`, `capture_next_frame` atomic flags.
  * `trigger()` / `consume_trigger()` API for frame-granularity capture requests.
- [x] VkDebugUtils labeling (objects, command buffer regions) — `crates/render/src/debug_label.rs`.
  * `label_object(device, object_type, handle, name)` — object naming.
  * `begin_label(cmd, name, color)` / `end_label(cmd)` — command buffer region markers.
- [x] GPU timestamp queries (per-pass timing) — `crates/render/src/profiler.rs`.
  * `GpuProfiler` — Vulkan `TIMESTAMP` query pool with `reset`, `timestamp`, and result collection.
  * Per-frame slotting with `MAX_TIMESTAMPS_PER_FRAME` and `FRAME_COUNT` ring buffer.
- [x] Tracy GPU profiling zones — `crates/render/src/tracy_gpu.rs`.
  * `TracyGpuZone` — zone scope struct; `begin_zone(name)` / `end_zone(zone)` API.
  * `collect_timestamps()` — stub for Tracy GPU calibration submission.
- [x] Wireframe / debug overlay rendering — `crates/render/src/wireframe.rs`.
  * `WireframeMode` — `Off` / `On` global toggle.
  * `DebugOverlay` — `None`, `Wireframe`, `Normals`, `TangentSpace`, `UV`, `Overdraw`.

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
- [x] EditorCamera with orbit + first-person modes. Orbit: WASDQE (shift), Alt+Left-drag orbit, middle-drag pan, scroll zoom. First-person: right-drag look, WASDQE move. `Space` toggles mode. Yaw/pitch clamped. Distance minimum 2.0. Camera state serialized into `.rustixproj`.

### 4.4 Editor Features (Implemented)
- [x] ECS entity tree in Hierarchy panel — live `hecs::World` query with `Name` + `Transform` display. Component-type icons via `world.query_mut::<(&Name, Option<&MeshComponent>, ...)>`.
- [x] Component editing in Inspector panel — full component reflection via typed queries + drag-value widgets. Custom color picker with HSVA 2-D picker popup + R/G/B inputs. All mutations recorded in undo history.
- [x] Offscreen 3D rendering — each viewport renders to a triple-buffered `Framebuffer` or `HdrFramebuffer` via `render_hdr_with_graph()`. The color attachment is registered as an egui user texture and displayed via `ui.painter().image(tex_id, ...)`. Frame graph handles barriers and layout transitions automatically.
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
- [x] Layout persistence (panel sizes, positions, viewport arrangement saved per-project)
- [x] Scene camera bookmarks / preset views — Save Current View button in viewport toolbar; bookmarks stored per-project in `.rustixproj`. Click to restore camera position, center, yaw, pitch, distance, and mode.

### 4.5 Editor Features (Planned)
- [x] Docking / panel rearrangement — View menu sets panel position per-project: Left, Right, Bottom, Floating, or Hidden. Hierarchy, Inspector, and Console panels each remember their dock position in `.rustixproj`.
- [x] Entity multi-select + group operations — `selected_entities` changed from `Option<<hecs::Entity>>` to `Vec<<hecs::Entity>>` across `app_state`, `main`, `editor`, `viewport/primary`, `hierarchy`, `inspector`, and `undo_redo`. Ctrl+click toggles selection in both viewport and hierarchy. Normal click replaces selection. Group Delete/Duplicate/Copy/Paste via viewport shortcuts and hierarchy toolbar. Gizmo shown only for single selection. Inspector displays "N entities selected" banner and disables component editing when >1 selected.
- [x] Drag-and-drop in Hierarchy (reparent entities) — Each hierarchy row is an `egui::dnd_drag_source` wrapped in `egui::dnd_drop_zone`. Dropping an entity onto another updates its `Parent` component. `is_descendant` prevents cycles (no parenting to own child). Undo records `ParentChanged` with old/new parent.
- [x] Play mode (simulate game inside editor viewport) — Toggle between `Editor` and `PlayTest` via `AppScreen`. Play button in viewport toolbar enters play mode; Stop button or ESC exits. In play mode: hierarchy, inspector, console, and dialogs are hidden; editor overlays (grid, gizmos, selection, camera debug text) are suppressed in the viewport; game systems (physics, animations) continue running. Menu bar shows a red "PLAYING" indicator.

### 4.6 Custom UI Framework (crates/ui)
- [x] Immediate mode UI context
- [x] Draw command list (rectangles, colored)
- [x] Button widget (with hover/interaction state)
- [x] Slider widget
- [x] Label widget (placeholder — colored rect, no real glyphs)
- [x] Layout helpers: vstack, center
- [x] Real text rendering (glyph atlas, font rasterization) — `fontdue` for TTF parsing + glyph rasterization. `GlyphAtlas` manages a CPU-side RGBA8 shelf-packed texture atlas (512x512) with on-demand glyph rasterization and a 1x1 white pixel for solid-color rects. `UIVertex` extended with UVs. `UIRenderer` uses a single textured pipeline (`sampler2D atlas` at set=0/binding=0) where rects sample the white pixel and glyphs sample their atlas region. Fragment shader uses `.r` channel as alpha mask. `label()` generates per-glyph `DrawCommand::Glyph` quads with correct `bearing_x`/`bearing_y` positioning. `UIContext::with_font()` accepts raw TTF bytes. Atlas dirty-tracking uploads GPU texture via `Renderer::update_texture_pixels()`.
- [x] Image widget — `image_widget()` / `UIContext::image()` accepts a `&GpuTexture`, position, size, UV range, and tint color. `DrawCommand::Image` stores raw `vk::ImageView` + `vk::Sampler` handles (Copy-safe). `UIRenderer` uses a dual-texture pipeline: binding 0 = font atlas (for rects/glyphs), binding 1 = image texture. Push constant `tex_idx` switches between them. The renderer batches commands and flushes vertex buffer on texture type changes. Images support UV sub-region sampling and vertex-color tinting (`texture(image_tex, vUV) * vColor`).
- [x] Text input — `text_input()` single-line edit widget. Click to focus (highlighted border). `UIContext` stores `typed_chars` and `keys_pressed` queues, consumed only by the focused widget. Supports: character insertion, Backspace/Delete, Left/Right arrow navigation, Home/End, Enter (returns `true` for submit), Escape (defocus). Cursor position is stored per-widget-ID in `UIContext::text_cursors` and rendered as a 1px vertical line positioned via glyph advance measurement. Keyboard events are fed externally via `UIContext::feed_char()` / `feed_key(UIKey)`. Works with or without a loaded font (fallback uses fixed-width approximation).
- [x] Flexbox/grid layout — `layout.rs` module provides `FlexLayout` (row/column, justify: Start/Center/End/SpaceBetween/SpaceAround/SpaceEvenly, align: Start/Center/End/Stretch, wrap, gap, padding) and `GridLayout` (columns/rows, col/row gap, padding, justify/align per cell). `LayoutItem` stores desired size, grow/shrink factors, and flex-basis. `flex_layout()` and `grid_layout()` resolve positions in immediate mode. `UIContext::flex_row()`, `flex_column()`, and `grid()` provide ergonomic callbacks receiving computed `(pos, size)` per child. Supports grow/shrink space distribution, cross-axis stretch, and wrap handling.

---

## 5. ASSET SYSTEM (crates/asset)

### 5.1 Asset Types
- [x] Meshes (glTF 2.0 → .rxmesh) — `MeshAsset` in `crates/asset/src/mesh.rs` stores CPU-side vertex + index data (position + normal, 24-byte stride matching the renderer pipeline). `.rxmesh` custom binary format: magic `RXM1`, version u32, vertex/index counts, AABB bounds, then tightly packed vertex and index data. `import_rxmesh()` / `export_rxmesh()` for round-trip serialization. `GltfMeshImporter` implements the `Importer` trait, reading `.gltf` / `.glb` via the `gltf` crate, extracting positions and normals (with fallback `[0,1,0]` normals), and concatenating all mesh primitives. `MeshAsset` implements the `Asset` trait for the asset server. `Mesh::from_asset()` in `crates/render` converts a `MeshAsset` to a GPU `Mesh` by uploading vertex/index buffers via the renderer.
- [x] Textures (PNG, HDR, KTX2 → .rxtex) — `TextureAsset` in `crates/asset/src/texture.rs` stores width, height, `TextureFormat` (R8G8B8A8_UNORM, R16G16B16A16_SFLOAT, R32G32B32A32_SFLOAT), raw pixel bytes, and mip level count. `.rxtex` custom binary format: magic `RXT1`, version u32, width/height u32, format enum u32, mip_levels u32, then tightly packed pixel data. `import_rxtex()` / `export_rxtex()` for round-trip serialization. Three importers implement the `Importer` trait: `PngTextureImporter` (`.png` → R8G8B8A8_UNORM via `image` crate), `HdrTextureImporter` (`.hdr` → decodes to RGBA32F via `image` then packs to R16G16B16A16_SFLOAT using the `half` crate), `Ktx2TextureImporter` (`.ktx2` → parsed via `ktx2` crate, supports R8G8B8A8_UNORM/R16G16B16A16_SFLOAT/R32G32B32A32_SFLOAT natively, with RGB8→RGBA8 expansion fallback). `Renderer::create_texture_with_format()` creates a `GpuTexture` from any Vulkan format, and `create_texture_from_asset()` maps `TextureAsset::format` to the corresponding `vk::Format` for direct GPU upload.
- [x] Materials (custom .rxmat format) — `MaterialAsset` in `crates/asset/src/material.rs` stores PBR scalar parameters (`base_color: [f32; 4]`, `roughness`, `metallic`, `ao`, `emissive`, `normal_scale`, `occlusion_strength`, `alpha_cutoff`) plus `AlphaMode` (Opaque/Mask/Blend). Optional texture path references for albedo, normal, metallic-roughness, emissive, and occlusion maps. `.rxmat` custom binary format: magic `RXA1`, version u32, tightly packed scalar params (48 bytes), alpha_mode u32, then a count-prefixed list of `(texture_slot, path)` entries. `import_rxmat()` / `export_rxmat()` for round-trip serialization. Three importers: `RxmatImporter` (`.rxmat` native binary), `RonMaterialImporter` (`.ron`/`.mat.ron` authoring), `JsonMaterialImporter` (`.json`/`.mat.json` authoring). `MaterialAsset` implements `SerializableAsset` for RON/JSON convenience. Conversion helpers: `Material::from_asset()` in `crates/render/src/components.rs` and `apps/runtime/src/scene.rs` map asset scalars to the runtime/ECS `Material` struct, and `MaterialComponent::from_asset()` resolves optional texture indices.
- [x] Shaders (GLSL → SPIR-V) — `ShaderAsset` in `crates/asset/src/shader.rs` stores `ShaderStage` (Vertex/Fragment/Compute/Mesh/Task), `ShaderLanguage` (GLSL/WGSL/SPIR-V), the original source string, compiled SPIR-V binary (`Vec<u32>`), and entry point name. `.rxshader` custom binary format: magic `RXS1`, version u32, stage/language u32, then length-prefixed entry point string, length-prefixed source string, and SPIR-V word count + data. `import_rxshader()` / `export_rxshader()` for round-trip serialization. Four importers: `GlslShaderImporter` (`.glsl`/`.vert`/`.frag`/`.comp`/`.mesh`/`.task` — compiles vertex/fragment/compute via naga at import time; mesh/task store raw source with empty SPIR-V for renderer-side shaderc compilation), `WgslShaderImporter` (`.wgsl` → naga WGSL frontend → SPIR-V), `SpvShaderImporter` (`.spv` raw SPIR-V binary with magic validation), `RxshaderImporter` (`.rxshader` native binary). `ShaderModule::from_asset()` in `crates/render` creates a GPU shader module from the asset: uses pre-compiled SPIR-V if present, otherwise recompiles from stored source via `from_glsl()` / `from_wgsl()` (with `#include` resolution and mesh/task fallback to shaderc).
- [x] Audio (WAV, OGG, FLAC → .rxsound) — `AudioAsset` in `crates/asset/src/audio.rs` stores decoded interleaved `f32` samples, `sample_rate` (Hz), `channels` (1 = mono, 2 = stereo), and precomputed `duration_seconds`. `.rxsound` custom binary format: magic `RXD1`, version u32, `sample_rate` u32, `channels` u16, `sample_count` u64, `duration_seconds` f32, then tightly packed f32 sample data. `import_rxsound()` / `export_rxsound()` for round-trip serialization. `GenericAudioImporter` implements the `Importer` trait for `.wav`/`.ogg`/`.flac`/`.mp3`/`.aac`/`.m4a`, using symphonia to decode from raw bytes into interleaved f32 via `SampleBuffer::copy_interleaved_ref`. `RsoundImporter` for `.rxsound` native binary. `AudioEngine::play_asset()` in `crates/audio` plays back an `AudioAsset` directly through rodio (when `audio-playback` feature is enabled), creating a `SoundInstance` with the decoded samples without re-decoding. `decode_from_asset()` converts an `AudioAsset` to the runtime `(Vec<f32>, u32, u16)` tuple used by the streaming and spatial audio systems.
- [x] Animation clips (.gltf animations → .rxanim) — `AnimationAsset` in `crates/asset/src/animation.rs` stores a `Vec<AnimationClipAsset>`, each containing a clip `name`, `duration`, and three `KeyframeAsset` tracks (position, rotation, scale). Each keyframe stores `time: f32` and `value: [f32; 3]`. `.rxanim` custom binary format: magic `RXN1`, version u32, clip count, then per-clip: length-prefixed name, duration f32, and three count-prefixed keyframe arrays (time + xyz). `import_rxanim()` / `export_rxanim()` for round-trip serialization. `GltfAnimationImporter` implements the `Importer` trait for `.gltf` / `.glb`, reading animation channels (translation, rotation, scale) via the `gltf` crate, extracting time values and sampled outputs. Rotations (quaternions in glTF) are converted to Euler angles (`XYZ` order) via `Quat::to_euler()` to match the engine's `Vec3` rotation representation. Supports all glTF rotation formats: `F32`, `U8`, `I8`, `U16`, `I16`. Keyframes are sorted by time per track. `AnimationClip::from_asset()` in `crates/animation` maps asset keyframes to runtime `Keyframe { time, value: Vec3 }` and `AnimationTrack`. `clips_from_asset()` converts an `AnimationAsset` to the runtime `HashMap<String, AnimationClip>` used by `update_animators`.
- [x] Skeleton definitions (.rxskel) — `SkeletonAsset` in `crates/asset/src/skeleton.rs` stores a bone hierarchy where each `BoneAsset` contains a 32-byte name, `parent` index (`u16`, `u16::MAX` = root), local transform (`local_pos`, `local_rot` as Euler XYZ, `local_scl`), and `inverse_bind` matrix (`[[f32; 4]; 4]`). `.rxskel` custom binary format: magic `RXK1`, version u32, bone count, then per-bone: 32-byte name, parent u16, local_pos/local_rot/local_scl (9 × f32), and 16 × f32 inverse bind matrix (134 bytes per bone). `import_rxskel()` / `export_rxskel()` for round-trip serialization. `GltfSkeletonImporter` implements the `Importer` trait for `.gltf` / `.glb`, reading skin data (joints and inverse bind matrices) via the `gltf` crate. It builds a node-parent map from the entire scene hierarchy, then maps each joint node to a bone, resolving parent indices only when the parent is also a joint in the same skin. Joint transforms are decomposed into position, quaternion rotation (converted to Euler XYZ), and scale. `Skeleton` runtime struct in `crates/animation/src/skeleton.rs` provides `compute_world_matrices()` (hierarchical local→world) and `compute_skinning_matrices()` (world * inverse_bind) for GPU skinning. `Skeleton::from_asset()` maps asset bones to runtime `Bone` structs with `Vec3` transforms and `Mat4` matrices.
- [x] Physics materials (.rxphys) — `PhysicsMaterialAsset` in `crates/asset/src/physics.rs` stores `static_friction`, `dynamic_friction`, `restitution`, and `density` (all f32). `.rxphys` custom binary format: magic `RXP1`, version u32, then tightly packed 4 × f32 (16 bytes total). `import_rxphys()` / `export_rxphys()` for round-trip serialization. Three importers: `RxphysImporter` (`.rxphys` native binary), `RonPhysMaterialImporter` (`.ron`/`.phys.ron` authoring), `JsonPhysMaterialImporter` (`.json`/`.phys.json` authoring). `PhysicsMaterialAsset` implements `SerializableAsset` for RON/JSON convenience. `PhysicsMaterial` component in `crates/physics` mirrors the asset fields and provides `from_asset()` conversion. `Collider::apply_material()` in `crates/physics` updates a collider's `restitution` and `friction` from an asset in one call, enabling shared material definitions across multiple colliders.
- [x] Prefabs (entity hierarchies, .rxprefab) — `PrefabAsset` in `crates/asset/src/prefab.rs` stores a `PrefabData` which is a `Vec<PrefabEntity>`, each representing an entity with `name`, `position`/`rotation`/`scale`, optional `mesh` string, and optional inline component structs (`PrefabMaterial`, `PrefabDirectionalLight`, `PrefabPointLight`, `PrefabSpotLight`, `PrefabRigidBody`, `PrefabCollider`, `PrefabScript`, `PrefabAudioListener`, `PrefabAudioSource`, `PrefabCamera`). Parent-child hierarchy is encoded via `parent_idx: Option<usize>`. `.rxprefab` binary format: magic `RXP1`, version u32, then length-prefixed RON string containing the `PrefabData`. This keeps prefabs human-readable for authoring while being identifiable by the asset pipeline. `import_rxprefab()` / `export_rxprefab()` for round-trip serialization. Two importers: `RxprefabImporter` (`.rxprefab` native binary-wrapped RON), `RonPrefabImporter` (`.prefab.ron`/`.ron` raw RON authoring). `spawn_prefab()` in `apps/runtime/src/scene.rs` instantiates a `PrefabAsset` into an `EcsWorld`, applying an optional base transform offset and preserving parent-child relationships. Returns the root entity handles.
- [x] Worlds / regions (.rxregion) — `RegionAsset` in `crates/asset/src/region.rs` stores a `RegionData` containing `RegionMetadata` (name, `ambient_color`, `ambient_intensity`, `sky_color`, `fog_color`, `fog_density`) plus a `Vec<PrefabEntity>` entity hierarchy (same inline component structure as prefabs). `.rxregion` binary format: magic `RXR1`, version u32, then length-prefixed RON string containing the `RegionData`. `import_rxregion()` / `export_rxregion()` for round-trip serialization. Two importers: `RxregionImporter` (`.rxregion` native binary-wrapped RON), `RonRegionImporter` (`.region.ron`/`.ron` raw RON authoring). `spawn_region()` in `apps/runtime/src/scene.rs` instantiates a `RegionAsset` into an `EcsWorld`, reusing the same entity-spawning logic as `spawn_prefab` with an optional base transform offset and preserving parent-child relationships. Returns the root entity handles. Region metadata is logged and can be extended to spawn dedicated ambient/fog component entities when those runtime components exist.
- [x] Fonts (.ttf) — `FontAsset` in `crates/asset/src/font.rs` stores a font `name` (human-readable identifier) and raw `data: Vec<u8>` (the complete `.ttf` / `.otf` file bytes). `.rxfont` custom binary format: magic `RXF1`, version u32, then length-prefixed name string, length-prefixed raw font data. `import_rxfont()` / `export_rxfont()` for round-trip serialization. Two importers: `TtfFontImporter` (`.ttf` / `.otf` — copies raw bytes and extracts the file stem as the font name from the import hint), `RxfontImporter` (`.rxfont` native binary). `Font` runtime struct in `crates/ui/src/text.rs` wraps the font data and provides `from_asset()` conversion plus `build_atlas()` which creates a `GlyphAtlas` via `fontdue::Font::from_bytes`. `UIContext::with_font_asset()` in `crates/ui/src/lib.rs` loads a font asset directly into the UI glyph atlas, replacing the previous `include_bytes!` pattern for engine UI text rendering.

### 5.2 Asset Pipeline
- [x] Hot-reload (watch source files, reimport) — `HotReloadService` in `crates/asset/src/hot_reload.rs` bridges `HotReloader` (file system watcher via `notify`), `ReloadRegistry` (type-erased import functions per extension), and `AssetServer` (generation-bumped asset replacement). `HotReloader` uses `notify::recommended_watcher` to emit `FileEvent`s (Created/Modified/Removed). `ReloadRegistry` stores `ReloadFn` per file extension: a boxed closure that calls the typed `Importer::import()` future and boxes the result as `Box<dyn Any>`, using `futures::executor::block_on` for synchronous resolution during development. `AssetServer::replace()` bumps the generation counter of an existing asset entry in-place, so all existing `Handle<T>` instances become stale (detected by generation mismatch) while the asset data is updated. `AssetServer::replace_untyped()` uses the `TypeId` stored alongside each path in `path_map` to target the correct typed store for replacement without compile-time type knowledge. `HotReloadService::poll_and_reload()` polls file events, reads changed files, looks up the reload function by extension, reimports, and calls `replace_untyped` — all in one frame-tick call. Handles are now unconditionally `Copy` (manual impl without `T: Copy` bound) so they can be passed freely even for non-`Copy` asset types like `MeshAsset`.
- [x] Asset decoding on worker threads (image, mesh, audio) — `AssetDecoderPool` in `crates/asset/src/decoder_pool.rs` wraps `rustix_core::task_priority::PriorityTaskSystem` and submits asset import work to Low-priority worker threads. `submit_import<I: Importer>()` sends a file decode job to the pool: it resolves the importer's future via `futures::executor::block_on` in the worker thread, then pushes a `DecodeResult` (path + boxed asset + optional error) into a shared `Arc<Mutex<Vec>>`. The main thread calls `poll_completed()` to drain finished results without blocking. `wait_for_all()` blocks until all pending decode tasks finish, useful for synchronous loading points like level transitions. This keeps heavy decode work (PNG decompression, WAV decoding, glTF mesh parsing, audio resampling) off the main / render thread. The pool uses the existing `PriorityTaskSystem` infrastructure with `TaskPriority::Low` so frame-critical tasks are never starved.
- [x] GPU upload via transfer queue — `GpuUploader` in `crates/render/src/memory/uploader.rs` manages a dedicated transfer command pool (created with `vk::CommandPoolCreateFlags::TRANSIENT | RESET_COMMAND_BUFFER` on the transfer queue family) and submits upload work to the GPU's transfer queue instead of the graphics queue. `GpuUploader::begin()` allocates and begins a one-time-submit command buffer from the transfer pool. `GpuUploader::submit()` ends the CB, creates a fence, and submits to `device.transfer_queue()`. `poll_completed()` non-blockingly checks fences and reclaims CBs. `wait_idle()` blocks until all in-flight uploads finish. The renderer's `transfer_command_pool` was fixed to use `device.transfer_queue_family_index()` instead of `graphics_queue_family_index()`. All texture creation (`create_texture`, `create_texture_with_format`), texture updates (`update_texture_pixels`, `update_texture_subregion`), and staging buffer uploads (`StagingBufferPool::upload_to_device`) now submit to `transfer_queue()` instead of `graphics_queue()`. On discrete GPUs with a dedicated transfer-only queue family, this keeps the graphics queue free for rendering while uploads happen asynchronously on the transfer queue.
- [x] Asset registry with reference counting — `AssetEntry<T>` in `crates/asset/src/server.rs` stores each asset as `Arc<T>`, and the `AssetServer` tracks live references via `Arc::strong_count()`. `AssetStore::is_referenced(handle)` returns `true` when `strong_count > 1` (the server itself holds one reference, so any count above 1 means external code still has the asset). `AssetStore::drain_unreferenced()` iterates all entries and removes those with `strong_count == 1`, pushing their indices onto the free list for reuse. `AssetServer::is_referenced::<T>()` provides typed access, `AssetServer::drain_unreferenced::<T>()` cleans up a single asset type, and `AssetServer::drain_unreferenced_all()` cleans up across every typed store. This enables garbage collection of unused assets (e.g. streaming out-of-view textures or meshes) without invalidating outstanding handles.
- [x] Handle-based access (8-byte handle, not Arc) — `Handle<T>` in `crates/asset/src/handle.rs` is unconditionally `Copy` (8 bytes: u32 index + u32 generation) with no `T: Copy` bound, so it can be stored in components, passed by value, and copied freely for any asset type. `AssetServer::resolve::<T>(handle)` returns `Option<AssetRef<'_, T>>`, an RAII guard that holds a store read-lock and derefs to `&T` — this gives temporary borrowed access without cloning an `Arc`. `AssetRef` uses a raw pointer to the `Arc`-inner data with a SAFETY comment: the read-lock prevents entry removal, so the `Arc` (and its inner `T`) remains valid for the guard's lifetime. `AssetServer::get::<T>(handle)` still returns `Option<Arc<T>>` for cases where long-lived ownership is needed, but `resolve()` is the preferred path for on-demand access. This design lets ECS components store only 8-byte handles and resolve asset data during system execution, keeping component sizes small and avoiding `Arc` refcount churn.
- [x] Asset dependencies (material depends on textures) — `AssetServer` in `crates/asset/src/server.rs` tracks bidirectional dependency relationships. `declare_dependencies<T>(handle, &[path])` registers that an asset depends on other asset paths; it populates both `dependencies: HashMap<UntypedHandle, Vec<PathBuf>>` (handle -> its deps) and `dependents: HashMap<PathBuf, Vec<UntypedHandle>>` (path -> assets waiting on it). `are_dependencies_loaded(handle)` checks whether every dependency path exists in `path_map` (i.e. the dependency has been imported into the server). `resolve_dependencies(handle)` converts dependency paths to `UntypedHandle`s, returning `None` if any are missing. `dependents_of(path)` returns all asset handles that declared a dependency on a given path — useful for notifying or reloading dependent assets when a dependency changes (e.g. texture hot-reload triggers material rebuild). `MaterialAsset::texture_dependencies()` in `crates/asset/src/material.rs` returns all non-None texture path references as `Vec<&str>`, making it easy to register a material's texture dependencies after import. This enables correct material instantiation: load textures first, then resolve material deps, and only build GPU materials once all referenced textures are present.
- [x] Async loading (tokio runtime for IO) — `AssetLoader` in `crates/asset/src/loader.rs` wraps a `tokio::runtime::Handle` and spawns IO tasks that read files asynchronously via `tokio::fs::read()`, then run the appropriate `Importer::import()` on the bytes. `AssetLoader::load(path, importer)` returns a `tokio::sync::oneshot::Receiver<Result<T, String>>` that the caller can `.await` or poll. The file read and import happen entirely on tokio worker threads, keeping the main thread free for frame processing. The existing `LoadState<T>` and `AsyncLoad<T>` types in `crates/asset/src/load_state.rs` provide a state-machine representation (Pending/Loading/Loaded/Failed) with `Notify`-based waker support for futures integration. 
- [x] Streaming (priority-ordered load/unload) — `StreamingSystem` in `crates/asset/src/streaming.rs` manages a priority-ordered load queue using `BinaryHeap<(Reverse<StreamingPriority>, PathBuf)>`. Callers submit `request_load(path, priority)` where priority is one of `Critical`, `High`, `Medium`, `Low`, `Background`. `tick()` processes pending unloads first, then processes up to `budget_per_tick` load requests from highest to lowest priority. If `loaded.len() > max_loaded`, it evicts the lowest-priority tracked assets. `evict_unreferenced(server)` calls `AssetServer::drain_unreferenced_all()` to garbage-collect unused assets, then removes the corresponding streaming entries. `resolve_handle(path, handle)` links a placeholder streaming entry to the actual server handle once the async load completes. `cancel(path)` removes a pending request from both queues. This enables level-of-detail streaming, open-world asset paging, and memory-budget enforcement without stalling the render thread.
- [x] Asset caching (disk cache of processed assets) — `DiskCache` in `crates/asset/src/cache.rs` stores processed asset binaries on disk so that subsequent loads can skip the import step. `DiskCache::new(root)` creates the cache directory. `is_cached(source_path)` checks validity by comparing the source file's modification time and size against metadata stored in a sidecar `.meta` JSON file. `read(source_path)` returns the cached binary bytes if valid. `write(source_path, data)` stores processed bytes alongside metadata. `invalidate(source_path)` and `clear()` remove entries. Cache keys are hex-encoded `DefaultHasher` hashes of the source path, producing flat filenames `<root>/<hash>.cache` + `<hash>.meta`. `entry_count()` and `total_size()` provide cache statistics for debugging and budget monitoring. This lets the engine avoid re-decoding `.png`/`.wav`/`.gltf` files on every run — after the first import, subsequent loads read the native `.rx*` binary directly from disk.
- [x] Virtual file system (VFS) for asset path resolution — `Vfs` in `crates/asset/src/vfs.rs` maps logical asset paths to physical locations through a stack of mount points. `Vfs::mount(name, point)` adds a `MountPoint` (either a `Directory` on disk or an in-memory `Archive`) to the mount stack; later mounts shadow earlier ones, enabling user/mods to override engine assets. `read(virtual_path)` checks mounts from last to first and returns the file bytes from the first match. `read_with_path()` also returns the physical `PathBuf` for directory mounts (useful for hot-reload watching). `exists()` and `resolve()` query the mount stack. `list(virtual_dir)` merges directory listings from all mounts and deduplicates results. `MountPoint::Archive` stores files in a flat `Vec<u8>` with a `HashMap<String, ArchiveEntry>` index, enabling fast in-memory packed asset bundles. `build_archive()` is a convenience helper for constructing archive mounts from `(path, bytes)` pairs. This decouples engine code from hard-coded disk paths, supports DLC/mod asset overrides, and enables future .pak/.zip asset bundles.

### 5.3 Import Pipeline
- [x] glTF 2.0 import (meshes, materials, animations, skeletons) — Four dedicated glTF importers extract engine-native assets from `.gltf` / `.glb` files:
  * `GltfMeshImporter` in `crates/asset/src/mesh.rs` — reads all mesh primitives, extracting `POSITION` and `NORMAL` attributes into `Vertex` structs, building a unified index buffer with base-vertex offsets per primitive. Falls back to `[0, 1, 0]` normals when missing. Produces `MeshAsset` with computed AABB.
  * `GltfMaterialImporter` in `crates/asset/src/material.rs` — reads the first glTF material's PBR metallic-roughness parameters (`base_color_factor`, `metallic_factor`, `roughness_factor`), texture references (albedo, metallic-roughness, normal, emissive, occlusion), and alpha settings (`alpha_mode`, `alpha_cutoff`, `normal_scale`, `occlusion_strength`). Maps glTF `image::Source::Uri` to engine texture paths. Produces `MaterialAsset` with `texture_dependencies()` ready for dependency tracking.
  * `GltfAnimationImporter` in `crates/asset/src/animation.rs` — decodes all animation channels, handling `Translation`, `Rotation`, and `Scale` targets. Supports all glTF rotation quantization formats (`F32`, `U8`, `I8`, `U16`, `I16`) with proper de-quantization. Converts quaternion rotations to Euler XYZ for the engine's keyframe tracks. Sorts keyframes by time. Produces `AnimationAsset` with named `AnimationClipAsset`s.
  * `GltfSkeletonImporter` in `crates/asset/src/skeleton.rs` — reads glTF skins and their joint hierarchies. Builds a bone list with parent indices, local transforms (position, rotation as Euler, scale), and inverse bind matrices from the skin's `inverseBindMatrices` accessor. Produces `SkeletonAsset` with `find_bone_index()` for animation targeting.
- [x] Texture compression (BC7 / ASTC conversion) — `TextureCompressor` in `crates/asset/src/texture_compress.rs` converts `TextureAsset` RGBA8 source data into GPU-native block-compressed formats using the `ctt` crate (which dispatches to `bc7enc_rdo` for BC7 and `astcenc` for ASTC). Supported output formats: `BC7_UNORM`, `BC7_UNORM_SRGB`, `ASTC_4x4_UNORM`, `ASTC_4x4_UNORM_SRGB`, `ASTC_6x6_UNORM`, `ASTC_6x6_UNORM_SRGB`, `ASTC_8x8_UNORM`, `ASTC_8x8_UNORM_SRGB`. `TextureCompressor::compress(asset, format)` returns a `CompressedTexture` containing raw block bytes ready for GPU upload. `compress_with_mips()` generates a full mipmap chain by box-filter downsampling the source, then compressing each level independently. `CompressedBlockFormat::compressed_size()` gives the exact byte count for a given image dimension. The `ctt` crate handles color-space conversion (sRGB ↔ linear) and alpha premultiplication internally, ensuring correct compressed output for both diffuse (sRGB) and normal/roughness (linear) textures.
- [x] Mesh optimization (vertex cache reordering, stripification) — `mesh_opt` module in `crates/asset/src/mesh_opt.rs` wraps the `meshopt` crate (meshoptimizer by Arseny Kapoulkine) for GPU-friendly mesh preprocessing:
  * `optimize_vertex_cache(mesh)` — reorders indices using the Forsyth algorithm to maximize GPU vertex cache hit rates.
  * `optimize_overdraw(mesh, threshold)` — reorders triangles to reduce pixel overdraw. Requires a vertex-cache-optimized mesh as input. `threshold` controls the vertex-cache vs overdraw trade-off (1.05 = balanced).
  * `optimize_vertex_fetch(mesh)` — reorders the vertex buffer and remaps indices to reduce vertex fetch memory bandwidth. Also deduplicates identical vertices.
  * `optimize_full(mesh, threshold)` — applies the complete pipeline: cache → overdraw → fetch, producing a fully GPU-tuned mesh.
  * `stripify(mesh)` → `Result<Vec<u16>>` — converts a triangle list to a triangle strip with primitive restart markers (`0xFFFF`), reducing index buffer size.
  * `unstripify(strip, restart_index)` → `Result<Vec<u16>>` — converts strips back to triangle lists.
  * `build_meshlets(mesh, max_vertices, max_triangles, cone_weight)` — partitions the mesh into meshlet clusters for GPU mesh shading pipelines (e.g. NVIDIA Turing+ mesh shaders).
  * `analyze_vertex_cache(mesh, cache_size)` → `CacheStats` — computes ACMR (Average Cache Miss Ratio) and ATVR (Average Transform to Vertex Ratio) to quantify optimization quality.
  * `analyze_overdraw(mesh)` → `OverdrawStats` — computes shaded pixels / covered pixels ratio.
  * `Vertex` implements `meshopt::DecodePosition` so all decoder-based functions work directly with engine vertices.
- [x] Asset cooking (preprocessing for runtime performance) — `AssetCooker` in `crates/asset/src/cook.rs` orchestrates the full asset preprocessing pipeline. `AssetCooker::new(source_root, output_root)` sets up a cooker with a `DiskCache` for incremental builds. `scan()` recursively discovers source files and builds `CookJob`s with `CookKind` inferred from extension (`Mesh`, `Texture`, `Material`, `Animation`, `Skeleton`, `Generic`). `cook_one(job)` reads source bytes, runs the registered cook function for the file extension, writes the `.rx*` output, and updates the cache. `cook_incremental()` skips files whose cached entry is still valid (source mtime + size unchanged). `clean()` wipes the output directory and cache.
  
  Convenience cook functions are provided:
  * `cook_mesh()` — glTF → `import_gltf()` → `optimize_full()` (cache + overdraw + fetch) → `export_rxmesh()`.
  * `cook_texture_bc7()` — PNG → `import_png()` → `TextureCompressor::compress(BC7_UNORM_SRGB)` → raw block data.
  * `cook_material()` — RON → `import_ron::<MaterialAsset>()` → `export_rxmat()`.
  * `cook_animation()` — `.rxanim` pass-through.
  * `cook_skeleton()` — `.rxskel` pass-through.
  
  Callers register custom cook functions per extension via `cooker.register(ext, |bytes, hint| -> Result<Vec<u8>, String>)`, enabling arbitrary source formats to be plugged into the pipeline. This ties together importers, mesh optimization, texture compression, and disk caching into a single unified build step that can run offline or on first launch.
- [x] Dependency graph for incremental builds — `DependencyGraph` in `crates/asset/src/dependency_graph.rs` tracks directed edges between source asset paths so that when a dependency changes, all transitive dependents are re-cooked automatically.
  * `add_edge(source, dependency)` / `set_dependencies(source, deps)` — build the graph.
  * `dependencies_of(source)` — return direct dependencies.
  * `dependents_of(dependency)` — return direct dependents.
  * `transitive_dependents(path)` — BFS traversal returning all indirect dependents (e.g. if C depends on B and B depends on A, then `transitive_dependents(A)` returns `[B, C]`).
  * `save(path)` / `load(path)` — persist the graph as JSON (`.deps.json`) so incremental build state survives across process restarts.
  
  Integrated into `AssetCooker` (`crates/asset/src/cook.rs`):
  * `AssetCooker` holds a `DependencyGraph` and auto-loads it from `.deps.json` on construction.
  * After successfully cooking a material, `cook_one()` parses the source RON, calls `MaterialAsset::texture_dependencies()`, resolves paths relative to `source_root`, and registers them in the graph via `set_dependencies()`.
  * `cook_incremental()` uses a three-phase invalidation strategy:
    1. **Directly stale** — cache miss or missing output file.
    2. **Dependency stale** — any job whose direct dependency is in the stale set.
    3. **Transitive dependents** — all downstream assets via `transitive_dependents()`.
    The union of these three sets is cooked, and the updated graph is persisted back to disk.
  * `clean()` removes the `.deps.json` alongside cooked files and cache entries.

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
- [x] Spatial audio (distance attenuation, HRTF panning) — Implemented in `crates/audio/src/spatial.rs` and integrated into `AudioEngine` (`crates/audio/src/engine.rs`):
  * `AudioListener` — position, forward direction, up vector (typically attached to the camera).
  * `AudioSource` — position, `min_distance`, `max_distance`, `rolloff` factor.
  * `calculate_attenuation(distance, min, max, rolloff)` — inverse-distance model with linear taper. Returns `1.0` at `min_distance`, `0.0` at `max_distance`.
  * `calculate_horiz_azimuth(listener_pos, listener_forward, source_pos)` — computes the horizontal angle from listener to source (-π to π, 0 = directly in front).
  * `hrtf_panning(angle)` — simplified HRTF model returning `(left_gain, right_gain)` with Interaural Level Difference (ILD) based on azimuth angle. Normalizes gains so they sum to 1.0.
  * `process_spatial()` — applies attenuation + HRTF panning to mono or stereo interleaved f32 samples in-place.
  * `AudioEngine::play_sound_spatial(path, source, spatial_blend, looping)` — decodes audio, applies full spatial pipeline (attenuation + panning), then plays via `rodio::Sink`.
  * Constants: `REFERENCE_DISTANCE = 1.0`, `SPEED_OF_SOUND = 343.0`, `HEAD_RADIUS = 0.0875` (for future ITD delay-line support), `MAX_ITD = 0.0006s`.
- [x] Audio effects (reverb, EQ, compression) — `EffectChain` in `crates/audio/src/effects.rs` chains arbitrary `AudioEffect` processors. `Compressor`, `Equalizer` (3-band), and `Reverb` (comb/allpass) are implemented as pure-Rust DSP effects. `SoundInstance` holds a `Mutex<EffectChain>` so effects can be added/enabled per instance.
- [x] Streaming for long files — `StreamDecoder` in `crates/audio/src/stream.rs` wraps `symphonia` for incremental decoding without loading the whole file into memory. `AudioEngine::stream_sound()` creates a `StreamingInstance` backed by a `rodio::Sink` fed from a lazy `StreamingSource` iterator. Supports looping with `seek(0.0)`.

### 6.3 ECS Components
- [x] `AudioSource` as `hecs::Component` (position, min/max distance, rolloff) — `AudioSource` in `crates/audio/src/spatial.rs` is a plain data struct with `serde` support, compatible with any ECS (e.g. `hecs::World::insert(id, source)`).
- [x] `AudioListener` as `hecs::Component` (position, forward, up) — `AudioListener` in `crates/audio/src/spatial.rs` with `Default` facing `-Z`.
- [x] `SoundPlayer` as `hecs::Component` (path, volume, looping) — `SoundPlayer` in `crates/audio/src/types.rs` with `spatial_blend` for 0..1 mix between 2D and 3D audio.
- [x] Automatic cleanup of finished instances — `SoundInstance::is_playing()` queries `rodio::Sink::empty()`. Systems can poll and drop finished instances.

### 6.4 Editor Integration
- [x] Sound effects assets in `assets/sounds/` (click, beep, whoosh, thump)
- [x] Audio file preview in Asset Browser — `AudioEngine::preview(path)` in `crates/audio/src/engine.rs` plays a one-shot preview of an audio file and automatically stops any previous preview. `stop_preview()` and `is_previewing()` provide full control for a UI asset browser. The engine tracks a single `preview: Option<SoundInstance>` so only one preview plays at a time, preventing cacophony when rapidly clicking through sound files. Works with any format supported by `symphonia` (WAV, MP3, OGG, FLAC, AAC).
- [x] Waveform visualization — `Waveform` in `crates/audio/src/waveform.rs` generates per-bar min/max amplitude data from decoded f32 samples, suitable for rendering as vertical bars by any UI renderer (egui, custom).
  * `WaveformBar { min, max }` — one bar of the waveform.
  * `generate_waveform(samples, channels, sample_rate, width)` — downsamples interleaved f32 audio to `width` bars. For stereo, channels are averaged per frame before computing min/max. Handles edge cases: empty input, more bars than samples, partial final bar.
  * `generate_waveform_from_instance(instance, width)` — convenience wrapper from `SoundInstance`.
  * `generate_waveform_from_path(path, width)` — decode + generate in one call.
  * `Waveform::bounds()` — returns overall (min, max) amplitude for autoscaling the Y axis.
  * `Waveform::duration()` — source duration in seconds.
- [x] Audio source gizmos in 3D viewport — `gizmo` module in `crates/render/src/gizmo.rs` generates CPU-side line-segment vertex data for wireframe debug shapes that a line-list renderer can draw over the main scene.
  * `GizmoVertex { position, color }` — a single colored vertex.
  * `GizmoLine { a, b }` — a line segment.
  * `wireframe_sphere(center, radius, color, segments)` — generates latitude/longitude rings as line segments.
  * `wireframe_cone(origin, direction, length, angle_deg, color, segments)` — generates a wireframe cone (direction indicator) with a base circle and ribs to the tip.
  * `wireframe_box(min, max, color)` — generates 12 edges of an AABB.
  * `AudioGizmo { position, min_distance, max_distance, direction, inner_color, outer_color }` — parameters for an audio-source debug visualization.
  * `generate_audio_gizmo(&AudioGizmo)` — produces line segments for:
    * Inner sphere at `min_distance` (solid cyan-green).
    * Outer sphere at `max_distance` (faded orange, semi-transparent).
    * Direction cone if `direction` is set (shows audio emission cone).
  * `flatten_gizmo_lines(&[GizmoLine])` — flattens line segments into interleaved `[pos3, color4, pos3, color4]` f32 data ready for GPU upload and a `VK_PRIMITIVE_TOPOLOGY_LINE_LIST` draw call.

---

## 7. PHYSICS SYSTEM (crates/physics)

- [x] Integration with `rapier` or `avian` (pure-Rust deterministic physics) — `RapierPhysicsWorld` in `crates/physics/src/rapier.rs` wraps `rapier3d` and synchronizes rigid bodies and colliders with the engine's ECS.
  * `RapierPhysicsWorld::new()` — creates rapier `PhysicsPipeline`, `RigidBodySet`, `ColliderSet`, `QueryPipeline`, and all supporting structures.
  * `add_entity(entity, RigidBody, Collider, position, rotation)` — maps engine ECS components to rapier `RigidBodyBuilder` + `ColliderBuilder`, tracks `hecs::Entity ↔ RigidBodyHandle` mapping.
  * `remove_entity(entity)` — removes body + collider from rapier and cleans up mappings.
  * `step(dt)` — advances the simulation; updates `QueryPipeline` for raycast queries.
  * `transform_of(entity)` — reads back position and Euler rotation after a step.
  * `apply_force(entity, force)`, `apply_impulse(entity, impulse)` — mutates rapier body directly.
  * `set_velocity(entity, velocity)`, `set_angular_velocity(entity, velocity)` — direct velocity overrides.
  * `raycast(origin, direction, max_toi)` — uses `QueryPipeline::cast_ray` to return `(entity, distance)` of first hit.
  * Keeps existing ECS components (`RigidBody`, `Collider`, `PhysicsMaterial`, `PhysicsWorld`) unchanged; rapier is a pure backend swap-in.
- [x] `RigidBody` ECS component (mass, velocity, angular velocity, body type) — `RigidBody` in `crates/physics/src/lib.rs` with `BodyType` enum (`Static`, `Dynamic`, `Kinematic`). Integrated with rapier via `build_rapier_body()`.
- [x] `Collider` ECS component (box, sphere, capsule, convex hull shapes) — `Collider` in `crates/physics/src/lib.rs` with `ColliderShape` enum. Mapped to rapier `SharedShape::ball`, `cuboid`, `capsule` in `build_rapier_collider()`.
- [x] `PhysicsMaterial` (friction, restitution/bounciness) — `PhysicsMaterial` in `crates/physics/src/lib.rs` with `static_friction`, `dynamic_friction`, `restitution`, `density`. Applied via `ColliderBuilder::restitution()` / `::friction()`.
- [x] Physics world step (`fixed timestep`, typically 60 Hz) — `RapierPhysicsWorld::step(dt)` calls `physics_pipeline.step()` with configurable `IntegrationParameters::dt`.
- [x] Force and impulse application API — `apply_force()`, `apply_impulse()`, `set_velocity()`, `set_angular_velocity()` on `RapierPhysicsWorld`.
- [x] Raycast / shapecast queries (for gameplay, audio occlusion) — `RapierPhysicsWorld::raycast()` via `QueryPipeline::cast_ray()`.
- [x] Collision event dispatch (enter/stay/exit) → ECS events — `CollisionEventCollector` implements rapier `EventHandler`, buffers `CollisionEvent::Started/Stopped` into a `Mutex<Vec>` during `step()`. `RapierPhysicsWorld::collision_events()` drains the buffer and maps `ColliderHandle` pairs to `hecs::Entity` via `collider.parent() → body_to_entity`, returning `(entity_a, entity_b, started)`. `ActiveEvents::COLLISION_EVENTS` is enabled on every `ColliderBuilder` so rapier emits the events.
- [x] Character controller (capsule + slope handling + step-up) — `CharacterController` ECS component in `crates/physics/src/lib.rs` with `height`, `radius`, `slope_limit_degrees`, `step_height`, `snap_to_ground`.
  * `RapierPhysicsWorld::add_character(entity, &controller)` — maps settings to rapier `KinematicCharacterController` with `autostep`, `max_slope_climb_angle`, `min_slope_slide_angle`, `snap_to_ground`.
  * `RapierPhysicsWorld::move_character(entity, desired_translation, dt)` — calls rapier `move_shape` with the entity's collider shape, applies the resulting `EffectiveCharacterMovement::translation` to the rigid body position, returns `(effective_translation, is_grounded)`.
  * `remove_character(entity)` — unregisters from the internal character controller map.
- [x] Joints (fixed, revolute, spherical, prismatic) — `JointType` enum and `Joint` ECS component in `crates/physics/src/lib.rs`.
  * `JointType::Fixed` — rigid connection via `FixedJointBuilder`.
  * `JointType::Revolute { axis }` — hinge rotation around `axis` via `RevoluteJointBuilder::new(axis)`.
  * `JointType::Spherical` — ball-and-socket via `SphericalJointBuilder`.
  * `JointType::Prismatic { axis }` — sliding along `axis` via `PrismaticJointBuilder::new(axis)`.
  * `Joint` fields: `connected_entity`, `local_anchor1`, `local_anchor2`, `contacts_enabled`.
  * `RapierPhysicsWorld::add_joint(entity_a, &joint)` — maps to `ImpulseJointSet::insert(body_a, body_b, generic_joint, true)`, tracks handle in `joints: HashMap<hecs::Entity, ImpulseJointHandle>`.
  * `remove_joint(entity)` — removes from `ImpulseJointSet` and cleans up mapping.
  * `joint_count()` — returns number of active impulse joints.
- [x] Sleeping/waking for static optimization — `RigidBody` ECS component extended with `can_sleep: bool` (default true) and `sleeping: bool` (default false).
  * `build_rapier_body()` passes `can_sleep` and `sleeping` to `RigidBodyBuilder::can_sleep()` / `::sleeping()` so rapier automatically sleeps/wakes bodies.
  * `RapierPhysicsWorld::wake_up(entity, strong)` — manually wakes a rigid body via `body.wake_up(strong)`.
  * `RapierPhysicsWorld::is_sleeping(entity) -> bool` — queries whether a body is currently sleeping.
  * `RapierPhysicsWorld::active_body_count() -> usize` — counts non-sleeping rigid bodies for profiling/optimization.
- [x] Debug draw: collision shapes, contact points, velocities — `PhysicsDebugLine` struct in `crates/physics/src/lib.rs` (`start`, `end`, `color`).
  * `RapierPhysicsWorld::debug_draw() -> Vec<PhysicsDebugLine>` — uses rapier's built-in `DebugRenderPipeline` (via `debug-render` feature) to generate wireframes for all colliders, contact normals, rigid-body axes, and joints.
  * `DebugLineCollector` implements rapier `DebugRenderBackend`, capturing every `draw_line` call into `PhysicsDebugLine` instances that the engine's gizmo renderer can consume.
  * Rapier dependency in `crates/physics/Cargo.toml` enables `features = ["debug-render"]`.
- [x] Deterministic replay support (fixed-point or deterministic float) — rapier3d dependency in `crates/physics/Cargo.toml` enables `enhanced-determinism` feature, which forces use of `libm` for cross-platform deterministic floating-point math.
  * `PhysicsSnapshot` struct in `crates/physics/src/rapier.rs` serializes all rapier state: `RigidBodySet`, `ColliderSet`, `ImpulseJointSet`, `IslandManager`, `BroadPhase`, `NarrowPhase`, `CCDSolver`, plus all ECS-to-physics mappings (`entity_to_body`, `body_to_entity`, `entity_to_collider`, `character_controllers`, `joints`).
  * `RapierPhysicsWorld::save_snapshot() -> PhysicsSnapshot` — clones the entire physics world state.
  * `RapierPhysicsWorld::restore_snapshot(&snapshot)` — restores from snapshot and rebuilds `QueryPipeline`.
  * `serialize_snapshot(&snapshot) -> Vec<u8>` and `deserialize_snapshot(bytes) -> PhysicsSnapshot` — convenience helpers using `bincode` for compact binary serialization.
  * Combined with input recording at the game layer, this enables fully deterministic replay: save a snapshot, record inputs per frame, then restore snapshot and replay inputs to reproduce identical physics state.

## 8. ANIMATION SYSTEM (crates/animation)

- [x] `AnimationClip` runtime struct (keyframes, duration, sample rate) — `AnimationClip` in `crates/animation/src/lib.rs` with `name`, `duration`, `position_track: AnimationTrack`, `rotation_track: RotationTrack`, `scale_track: AnimationTrack`.
  * `Keyframe` stores `Vec3` values with LERP interpolation via `AnimationTrack::sample(time)`.
  * `QuatKeyframe` stores `Quat` values with SLERP interpolation via `RotationTrack::sample(time)`.
  * `AnimationClip::from_asset()` converts asset Euler angles to runtime quaternions.
- [x] `Animator` ECS component (current state, playback speed, time) — `Animator` in `crates/animation/src/lib.rs` with `clip_name`, `time`, `speed`, `playing`, `looped`.
- [x] Pose sampling (LERP for translation/scale, SLERP for rotation) — `AnimationTrack::sample()` does LERP on `Vec3` keyframes; `RotationTrack::sample()` does SLERP on `Quat` keyframes.
- [x] Animation blending (cross-fade between clips by weight) — `BlendAnimator` in `crates/animation/src/lib.rs` holds `current` and `previous` `Animator` states, with `blend_weight` and `blend_duration`.
  * `transition_to(clip_name, duration)` swaps current→previous and starts a new clip, then cross-fades over `duration` seconds.
  * `update(clips, dt)` advances both animators and blends poses with LERP for position/scale and SLERP for rotation.
- [x] Animation state machine / graph (nodes = clips, edges = transitions) — `AnimationStateMachine` in `crates/animation/src/state_machine.rs` with `states: HashMap<String, AnimationState>`.
  * `AnimationState` holds `clip_name`, `looped`, `speed`, and outgoing `transitions`.
  * `Transition` defines `target_state`, `condition`, and `blend_duration`.
- [x] Transition conditions (time elapsed, parameter thresholds, triggers) — `TransitionCondition` enum with:
  * `Always`, `TimeElapsed(seconds)`, `TimeRemaining(seconds)`.
  * `Trigger(name)` — consumed one-shot parameter.
  * `ParameterGte { name, threshold }`, `ParameterLt { name, threshold }`, `ParameterBool { name, value }`.
  * `And(a, b)` for combining conditions.
  * `AnimationStateMachine::set_parameter()`, `set_bool_parameter()`, `set_trigger()` set runtime values; `update(dt)` evaluates conditions and triggers state changes.
- [x] Bone hierarchy → `Mat4` palette generation for GPU skinning — `Bone` and `Skeleton` in `crates/animation/src/skeleton.rs`.
  * `Bone` stores `name`, `parent` index, `local_pos`, `local_rot` (Euler), `local_scl`, `inverse_bind` matrix.
  * `Skeleton::compute_world_matrices()` propagates local transforms through the hierarchy to world-space `Mat4`.
  * `Skeleton::compute_skinning_matrices()` computes `world * inverse_bind` palette for GPU vertex skinning.
- [x] Root motion extraction (move character from animation data) — `RootMotion` struct in `crates/animation/src/lib.rs` with `delta_position: Vec3` and `delta_rotation: Quat`.
  * `AnimationClip::extract_root_motion(prev_time, current_time)` samples position and rotation at both times and returns the delta.
- [x] Inverse Kinematics (IK) for feet/hands placement — `CcdIkSolver` in `crates/animation/src/ik.rs` implements Cyclic Coordinate Descent.
  * `IkJoint` stores `position`, `rotation`, and `length` (distance to next joint).
  * `CcdIkSolver::solve(chain, target)` iterates backward from tip to root, rotating each joint so the end-effector approaches the target using axis-angle alignment.
  * Configurable `max_iterations` and `tolerance` for quality vs. performance trade-off.
- [x] Animation retargeting (same rig → different proportions) — `Skeleton::retarget_from(source)` in `crates/animation/src/skeleton.rs`.
  * Matches bones between source and target skeletons by name.
  * Copies only `local_rot` (Euler angles) from source to target.
  * Preserves target skeleton's `local_pos`, `local_scl`, and `inverse_bind` so bone lengths and proportions remain correct for the target character.
  * `retargeted_world_matrices(source)` convenience — retargets then computes world matrices.
  * `retargeted_skinning_matrices(source)` convenience — retargets then computes skinning palette for GPU skinning.
- [x] Event tracks (footstep, weapon fire, etc. at specific frames) — `AnimationEvent` struct (`time`, `name`) and `EventTrack` in `crates/animation/src/lib.rs`.
  * `EventTrack::events_between(prev_time, current_time, duration, looped)` returns all events in the time window, handling loop wrap-around correctly.
  * `AnimationClip::sample_events(prev_time, time, looped)` convenience wrapper on the clip's event track.
- [x] Multi-threaded pose evaluation — `PoseEvaluator` and `update_animators_par` in `crates/animation/src/lib.rs` using `rayon`.
  * `PoseEvaluator::evaluate_batch(inputs, clips)` — parallel `par_iter().map()` over `(entity, clip_name, time)` slices, returns sampled poses in the same order.
  * `PoseEvaluator::evaluate_pair(a, b, clips)` — `rayon::join()` for evaluating two poses with lower overhead.
  * `update_animators_par(animators, clips, dt)` — drop-in replacement for `update_animators` that advances time sequentially (fast, mutates state) then samples poses in parallel via `rayon`.

## 9. NETWORKING SYSTEM (crates/networking)

- [x] UDP socket abstraction with `tokio` async I/O — `AsyncUdpSocket` in `crates/networking/src/udp.rs`.
  * `bind(addr)` — async UDP socket binding.
  * `send_to(data, target)` and `recv_from(buf)` — non-blocking async send/receive.
  * `spawn_udp_receiver(socket, buffer_size)` — background task that forwards `(SocketAddr, Vec<u8>)` to a channel.
  * `spawn_udp_sender(socket)` — background task that sends `(SocketAddr, Vec<u8>)` from a channel.
  * `create_udp_pipeline(bind_addr, buffer_size)` — convenience to create socket + sender/receiver channels.
- [x] Connection-oriented protocol (handshake, heartbeat, disconnect) — `crates/networking/src/protocol.rs`.
  * `VirtualConnection` tracks `ConnectionState` (Handshaking, Connected, Disconnecting, Disconnected).
  * `PacketType` enum: HandshakeRequest, HandshakeResponse, Heartbeat, Reliable, Unreliable, Disconnect.
  * `ProtocolPacket` with `sequence`, `ack`, and `payload` — wire format: `[type:1][seq:2][ack:2][payload..]`.
  * `ConnectionManager` manages multiple `VirtualConnection`s on a shared UDP socket.
  * `spawn_heartbeat_task` — background task sending periodic heartbeats to all connected peers.
  * Automatic timeout detection (`disconnect_timeout`) and cleanup.
- [x] Reliable ordered channel (for critical events: health, inventory) — `VirtualConnection::send_reliable(payload)`.
  * Monotonically increasing `sequence` numbers.
  * `pending_ack` queue with retransmission support via `pending_retransmits(timeout)`.
  * Duplicate detection using `last_received_seq`.
- [x] Unreliable unordered channel (for snapshots, inputs) — `VirtualConnection::send_unreliable(payload)`.
  * No sequence numbers or acks — fire-and-forget.
  * Separate `unreliable_inbox` for consuming unordered payloads.
- [x] Message serialization with `bincode` — `crates/networking/src/serialize.rs`.
  * `serialize(value)` / `deserialize(bytes)` — typed `bincode` encode/decode.
  * `serialize_unchecked` / `deserialize_unchecked` — panic-on-error variants for infallible types.
- [x] Client prediction + server reconciliation — `crates/networking/src/prediction.rs`.
  * `ClientPrediction` buffers `pending_inputs` by tick.
  * `push_input(input)` records a new input and returns its assigned tick.
  * `acknowledge(server_tick)` removes confirmed inputs.
  * `inputs_to_replay()` returns unacknowledged inputs for reconciliation replay.
  * `ServerReconciliation` stores received inputs keyed by tick and supports `take_inputs_up_to(tick)`.
- [x] Entity interpolation for remote players (snapshot buffering) — `crates/networking/src/interpolation.rs`.
  * `SnapshotBuffer` stores historical snapshots with configurable `interpolation_delay` (e.g. 100ms) and `max_size`.
  * `push(snapshot)` inserts tick-ordered snapshots, evicting old ones.
  * `interpolate(current_time)` finds the two surrounding snapshots and linearly interpolates between them.
  * `Interpolatable` trait for custom state types; `InterpPosition` and `InterpEntityState` provided with nlerp quaternion interpolation.
- [x] Lag compensation (server rewinds hitboxes for shooting) — `LagCompensationBuffer` in `crates/networking/src/lag_compensation.rs`.
  * `LagCompFrame` stores per-tick entity snapshots (`LagCompSnapshot`) with position, rotation, and hitbox radius.
  * `push(frame)` records a rolling history with configurable `max_frames`.
  * `rewind_to_tick(tick)` / `rewind_to_time(timestamp)` — finds the nearest historical frame.
  * `rewind_and_interpolate(target_tick)` — linearly interpolates entity positions between two stored frames for smooth rewound state.
  * `lag_compensated_raycast(origin, direction, max_distance, entities)` — sphere-vs-ray hit detection against the rewound entity list, returning the closest `HitResult`.
  * `latency_from_rtt(rtt_ms)` — estimates one-way latency from round-trip time.
- [x] Networked ECS replication (spawn/despawn/update/destroy) — `crates/networking/src/replication.rs`.
  * `NetworkId(u64)` — stable replicated entity identifier component.
  * `ReplicationMessage` enum: `Spawn`, `Despawn`, `Update`, `Remove`, `Batch`.
  * `ComponentUpdate` / `ComponentRemoval` — per-component delta messages with name + serialized payload.
  * `SpawnMessage` — initial component bundle for new entities.
  * `ReplicationTracker` — accumulates local changes (spawn/despawn/update/remove) per tick and converts to `ReplicationMessage`s.
  * `NetworkEntityMap` — bidirectional `NetworkId ↔ hecs::Entity` mapping for client-side mirroring.
  * `ComponentSerializer` trait — runtime implements component serialization/deserialization using the engine's registry; keeps networking crate decoupled.
  * `apply_replication_message(world, map, serializer, message)` — applies any `ReplicationMessage` to the local ECS world, updating the entity map for spawn/despawn.
  * `batch_messages(messages)` — collapses multiple messages into a single `Batch` for efficient transport.
- [x] Authority system (server-authoritative, client-predicted, interpolated) — `crates/networking/src/authority.rs`.
  * `Authority` enum: `Server`, `Client(ClientId)`, `Interpolated`.
  * `AuthorityComponent` — marks replicated entities with their authority mode.
  * `AuthorityManager` (server-side) — tracks ownership, validates client update requests via `can_client_update(client_id, network_id)`, supports authority transfer with `transfer(network_id, new_authority)`.
  * `ClientAuthorityManager` (client-side) — mirrors server assignments, identifies `local_predicted_entities()`, `interpolated_entities()`, and `server_authoritative()` entities.
  * `AuthorityTransfer` message — server-to-client notification when ownership changes.
- [x] Bandwidth optimization: delta compression, interest management — `crates/networking/src/bandwidth.rs`.
  * `DeltaCompressor` — per-client, per-entity, per-component FNV-1a hash tracking. Only sends `ComponentUpdate`s when payload differs from last acknowledged state.
  * `InterestManager` — spatial interest management with configurable `max_distance_sq`. `update_interest_set(client, observer_pos, entities)` rebuilds the client's visible set each tick.
  * `InterestCriteria` — `max_distance_sq`, `include_server_authoritative`, `always_include_own` flags.
  * `BandwidthOptimizer` — combines delta compression + interest filtering in one `optimize_for_client(client_id, messages)` call.
  * `filter_messages()` / `filter_updates()` — drop replication messages for out-of-interest or unchanged entities.
- [x] NAT punch-through or relay server support — `crates/networking/src/nat.rs`.
  * `RendezvousClient` — registers with a rendezvous server via `register_and_wait_peer(session_id)` to discover the peer's public NAT-mapped endpoint.
  * `NatPunchThrough` — sends a burst of UDP packets to the peer's public address and waits for a response to confirm the NAT mapping is open.
  * `RelayServer` — minimal UDP relay that forwards `RelayPacket`s between registered clients using `sender_client_id` / `target_client_id` routing.
  * `RelayClient` — wraps a UDP socket to send/receive relayed payloads; `send_to(target, payload)` and `recv()` handle the relay protocol transparently.
  * `connect_with_fallback(socket, rendezvous, relay, ...)` — attempts NAT punch-through and automatically falls back to the relay on failure, returning `ConnectionMode::Direct(addr)` or `ConnectionMode::Relay(RelayClient)`.
- [x] Matchmaking / lobby API stub — `crates/networking/src/matchmaking.rs`.
  * `Lobby` — named room with host, max players, player list, ready status, team assignment, game mode, and map.
  * `LobbyPlayer` — `client_id`, `display_name`, `ready`, `team`.
  * `MatchmakingRequest` — `CreateLobby`, `JoinLobby`, `SearchLobbies`, `LeaveLobby`, `SetReady`, `SetTeam`, `StartMatch`.
  * `MatchmakingResponse` — `LobbyCreated`, `JoinedLobby`, `LobbyList`, `PlayerJoined`, `PlayerLeft`, `PlayerReady`, `MatchStarting`, `Error`.
  * `LobbyManager` (in-memory server stub) — `create_lobby(host, name, max_players)`, `join_lobby(client, lobby_id)`, `leave_lobby(client)`, `set_ready`, `set_team`, `start_match` (host only, all ready), `search(criteria)` with game mode / map / not-full / not-started filters.
  * Automatic host transfer when the host leaves.

## 10. SCRIPTING SYSTEM (crates/scripting)

- [x] Rhai scripting engine integration (`rhai` crate) — `crates/scripting/src/lib.rs`.
  * `ScriptEngine` — compiles and executes Rhai AST; `Script` asset type with `.rxscript` extension.
  * `ScriptComponent` — attached to ECS entities; `ScriptApi` / `ScriptInstance` registry.
- [x] Rust → script bindings for core types (`Vec3`, `Quat`, `Entity`) — `crates/scripting/src/lib.rs`.
  * `Transform`, `Vec3`, `Quat`, `Mat4` exposed to scripts via Rhai custom types.
- [x] ECS query API from scripts — `crates/scripting/src/lib.rs`.
  * `ScriptApi` can query entity components and iterate results.
- [x] Event subscription from scripts — `crates/scripting/src/events.rs`.
  * `ScriptEventBus` — `subscribe(event_name, callback)`, `emit(event_name)`, `unsubscribe_script(script_id)`.
- [x] Hot-reload of `.rhai` scripts without restarting engine — `crates/scripting/src/hot_reload.rs`.
  * `HotReloadWatcher` — tracks file modification times; `check()` returns changed paths.
- [x] Component definition from scripts — `crates/scripting/src/component_def.rs`.
  * `ComponentRegistry` — `define(name, fields)` with `ScriptFieldType` (`Float`, `Int`, `Bool`, `String`, `Vec3`, `Entity`).
- [x] Script asset type (`.rxscript`) loaded by asset system — `crates/scripting/src/lib.rs`.
  * `Script` implements `Asset` with `asset_type_id()`.
- [x] Time API (`dt`, `time`, `frame_count`) — `crates/scripting/src/time_api.rs`.
  * `ScriptTime` — `delta_time`, `elapsed`, `frame_count`; `tick(dt)` updates state.
- [x] Math API (`vec3`, `quat`, `lerp`, `dot`, `cross`) — `crates/scripting/src/math_api.rs`.
  * `vec3(x,y,z)`, `lerp(a,b,t)`, `dot(a,b)`, `cross(a,b)`, `normalize(v)`, `distance(a,b)`, `quat_from_euler(y,p,r)`.
- [x] Logging from scripts (`print` → `tracing::info`) — `crates/scripting/src/logging.rs`.
  * `script_log_info`, `script_log_warn`, `script_log_error`, `script_log_debug` — all target `"script"`.
- [x] Sandbox / security: restrict file system, network access — `crates/scripting/src/sandbox.rs`.
  * `SandboxPolicy` — `allow_file_read/write`, `allowed_read_paths`, `allow_network`, `max_memory_mb`, `max_execution_time_ms`.
  * `Sandbox` — `check_read(path)`, `check_write(path)`, `check_network()` enforcement.
- [x] Coroutine support for cutscenes and async scripts — `crates/scripting/src/coroutine.rs`.
  * `ScriptCoroutine` trait — `resume(dt)`, `state()`, `name()`; `YieldReason` — `WaitSeconds`, `WaitFrames`, `WaitForSignal`.
  * `CoroutineScheduler` — `spawn()`, `tick(dt)` with time-based and frame-based waiting queues.
  * `CutsceneCoroutine` — sequence builder with `wait_seconds`, `wait_frames`, `action` steps.

## 11. AI SYSTEM (crates/ai)

- [x] Navigation mesh generation from static colliders — `NavMeshGenerator` in `crates/ai/src/nav.rs`.
  * `NavMeshSource` component — explicit triangle mesh data (`vertices` + `indices`) for navmesh input.
  * `NavMeshGenerator::from_colliders(world)` — queries static `Box` colliders (`RigidBody` with `BodyType::Static` + `Collider` + `Transform`), generates top-face triangles transformed to world space.
  * `NavMeshGenerator::from_sources(world)` — reads `NavMeshSource` components, transforms triangles by entity `Transform`.
  * Slope filtering — only keeps triangles whose normal dot up-axis exceeds `max_slope_cos` (default 45°).
  * `build()` — produces a `NavMesh` with auto-connected neighbor links, ready for pathfinding via `to_pathfinder()`.
- [x] A* pathfinding on navmesh — `PathFinder` in `crates/ai/src/path.rs` with `NavMesh` integration.
  * `PathFinder::find_path(start, goal)` — A* with Euclidean heuristic over arbitrary graphs.
  * `NavMesh::to_pathfinder()` — builds a `PathFinder` from navmesh triangles (centers as nodes, shared edges as connections).
  * `NavMesh::find_path_triangles(start_pos, goal_pos)` — finds the triangle-index path between two world positions.
  * `NavMesh::find_path_waypoints(start_pos, goal_pos)` — returns `Vec<Vec3>` world-space waypoints (triangle centers) along the path.
  * `a_star_grid(width, height, blocked)` — convenience constructor for 2D grid pathfinding with 4-directional movement.
- [x] Steering behaviors (seek, flee, arrive, wander, obstacle avoidance) — `crates/ai/src/steering.rs`.
  * `Agent` struct — `position`, `velocity`, `max_speed`, `max_force`.
  * `seek(agent, target)` / `flee(agent, target)` — basic attraction/repulsion forces.
  * `arrive(agent, target, slowing_distance)` — decelerates as the agent approaches the target.
  * `wander(agent, circle_distance, circle_radius, wander_angle, angle_change, random_signed)` — random displacement steering; caller supplies the RNG value.
  * `avoid_obstacles(agent, obstacles, feeler_length)` — feeler-ray obstacle avoidance.
  * `separation(agent, neighbors, desired_separation)` — keep distance from nearby agents.
  * `alignment(agent, neighbor_velocities)` — match heading with flock.
  * `cohesion(agent, neighbors)` — steer toward center of flock.
  * `combine(forces)` — weighted sum of multiple steering forces.
  * `integrate(agent, steering, dt)` — Euler integration applying clamped force.
- [x] Behavior trees (sequence, selector, parallel, decorator nodes) — `crates/ai/src/btree.rs`.
  * `BehaviorNode` trait — `tick(blackboard, dt) -> Status`, `reset()`, `children()`.
  * `Status` enum — `Success`, `Failure`, `Running`.
  * `Blackboard` — type-erased key-value store with `set<T>`, `get<T>`, `get_mut<T>`.
  * `Action<F>` — leaf node wrapping a closure.
  * `Condition<F>` — decorator that only ticks child if predicate is true.
  * `Sequence` — ticks children in order; fails on first Failure, succeeds when all succeed.
  * `Selector` — ticks children in order; succeeds on first Success, fails when all fail.
  * `Parallel` — ticks all children each frame; succeeds when `success_threshold` children succeed, fails when `failure_threshold` children fail.
  * `Invert` — decorator that inverts Success/Failure.
  * `Repeat` — decorator that repeats child `n` times (or forever if `None`).
  * `BehaviorTree` — top-level wrapper with `tick(blackboard, dt)`.
- [x] Blackboard system (shared memory for AI agents) — `Blackboard` in `crates/ai/src/btree.rs`.
  * Type-erased `HashMap<String, Box<dyn Any + Send + Sync>>`.
  * `set<T>`, `get<T>`, `get_mut<T>`, `remove`, `clear`.
- [x] Finite state machines for simple NPCs — `Fsm<Ctx>` in `crates/ai/src/fsm.rs`.
  * `State` — `on_enter`, `on_update`, `on_exit` closures; typed via `State::new<Ctx>(id, on_update)` builder.
  * `Fsm::add_state(state)` / `Fsm::add_transition(from, to, condition)` — declarative machine construction.
  * `Fsm::set_initial(id)` — sets starting state.
  * `Fsm::tick(ctx, dt)` — evaluates transitions, fires enter/exit callbacks, updates current state.
  * `Fsm::current_state()` / `Fsm::is_in_state(id)` — introspection.
- [x] Sensor system (vision cone, hearing radius) — `crates/ai/src/sensor.rs`.
  * `VisionCone` — directional cone with `origin`, `forward`, `fov_deg`, `max_distance`; `can_see(target)` checks angle and distance.
  * `HearingRadius` — spherical sensor with `origin` and `radius`; `can_hear(sound_source, sound_radius)` checks combined radii.
  * `AgentSensor` — combined sensor suite with `with_vision`, `with_hearing`, `set_position`, `set_forward` helpers.
- [x] Squad coordination (formation movement, cover selection) — `crates/ai/src/steering.rs`.
  * `separation(agent, neighbors, desired_separation)` — keeps squad members spaced.
  * `alignment(agent, neighbor_velocities)` — matches heading for formation cohesion.
  * `cohesion(agent, neighbors)` — steers toward squad center to maintain formation.
  * Combined with `seek`/`arrive` these produce emergent squad formations.
- [x] GOAP (Goal-Oriented Action Planning) for complex agents — `crates/ai/src/goap.rs`.
  * `WorldState` — boolean fact store with `with(fact, value)`, `get(fact)`, `satisfies(preconditions)`, `apply(effects)`.
  * `GoapAction` — `name`, `cost`, `preconditions`, `effects`; builder API via `pre(fact, value)` and `effect(fact, value)`.
  * `GoapPlanner::plan(initial_state, goal)` — A* search over world states returning the cheapest action name sequence, or `None` if impossible.
- [x] Utility AI (scoring actions based on curves) — `crates/ai/src/utility.rs`.
  * `Curve` enum — `Linear`, `Exponential { exp }`, `Sigmoid { steepness, offset }`, `Step { threshold }`, `Inverse`; `evaluate(x)` maps [0,1] → [0,1].
  * `Consideration` — named input mapped through a weighted curve.
  * `UtilityAction` — composed of considerations; `score(inputs)` returns normalized utility.
  * `UtilityReasoner` — scores all actions; `select(inputs)` returns the highest-scoring action name, `ranked(inputs)` returns all actions sorted by score.
- [x] Influence maps for tactical decision making — `crates/ai/src/influence.rs`.
  * `InfluenceMap` — 2D grid with `width`, `height`, `cell_size`, `origin`, and `values: Vec<f32>`.
  * `stamp_influence(world_x, world_y, strength, radius)` — applies radial linear falloff influence.
  * `world_to_grid` / `grid_to_world` — bidirectional coordinate conversion.
  * `decay(factor)` — fades all values over time.
  * `highest_cell()` / `lowest_cell()` — finds extreme-value cells for strategic positioning.
  * `add_map(other)` — combines two maps (e.g., threat + safety = tactical overlay).
- [x] Debug draw: paths, state machine state, sensor ranges — `crates/ai/src/debug_draw.rs`.
  * `DebugLine` / `DebugPoint` / `DebugLabel` — CPU-side primitives for renderer consumption.
  * `AiDebugDraw` — per-frame accumulator with `line`, `point`, `label`, `clear`.
  * `draw_path(waypoints)` — green polyline between waypoints.
  * `draw_vision_cone(origin, forward, fov_deg, max_distance)` — cyan wireframe wedge.
  * `draw_hearing_radius(origin, radius)` — yellow wireframe circle on XZ plane.
  * `draw_influence_map(map, y_offset)` — colored point cloud representing influence values.
  * `draw_fsm_state(agent_pos, state_name)` — floating text label above agent.

## 12. TERRAIN SYSTEM (crates/terrain)

- [x] Heightmap import (`.png`, `.raw`, `.r16`) — `crates/terrain/src/import.rs`.
  * `import_png(bytes)` — decodes grayscale PNG (8-bit or 16-bit) into normalized heights.
  * `import_raw(bytes, width, height)` — 8-bit unsigned raw binary.
  * `import_r16(bytes, width, height)` — 16-bit big-endian raw binary.
  * `Heightmap::from_png`, `from_raw`, `from_r16` convenience constructors.
- [x] Procedural terrain generation (Perlin/Simplex noise, domain warping) — `crates/terrain/src/noise.rs`.
  * `noise::value` / `noise::fbm` — classic value noise with fractal Brownian motion.
  * `noise::Perlin` — 2D Perlin noise with `new(seed)`, `noise(x, y)`, and `fbm(...)`.
  * `noise::domain_warp(x, y, amplitude, frequency, warp_fn, main_fn)` — distorts coordinates with low-frequency noise before sampling main noise.
  * `TerrainParams::noise_type` — switch between `Value` and `Perlin`; `warp_amplitude` / `warp_frequency` for domain warping.
- [x] LOD quadtree / chunked LOD for distant terrain — `crates/terrain/src/chunk.rs`.
  * `TerrainChunk::from_heightmap(...)` — builds a mesh patch with configurable `sample_step` LOD.
  * `ChunkedTerrain` — manages multiple chunks with distance-based LOD selection via `lod_distances` thresholds.
  * `ChunkedTerrain::rebuild(heightmap, view_x, view_z)` — regenerates all chunks from a viewer position.
- [x] Seamless chunk stitching (skirt vertices to hide gaps) — `crates/terrain/src/chunk.rs`.
  * `build_chunk_skirt(chunk, neighbor_edge_heights, skirt_depth)` — generates vertical wall strips around chunk perimeters that match neighboring LOD edge heights, hiding T-junction cracks.
- [x] Splat-map texturing (4-8 blend layers: grass, rock, sand, snow) — `crates/terrain/src/splat.rs`.
  * `TerrainLayer` — named material layer with height and slope range constraints and base weight.
  * `SplatMap` — RGBA weight texture for up to 4 layers per map; `normalize()` ensures weights sum to 1.
  * `SplatStack` — collection of splat maps supporting up to 8 layers (2 RGBA maps).
  * `SplatStack::generate(heights, slopes, width, depth)` — auto-computes layer weights from terrain geometry.
- [x] Physically-based terrain materials (roughness, AO per layer) — `crates/terrain/src/material.rs`.
  * `TerrainMaterial` — per-layer PBR properties: `albedo`, `roughness`, `ao`, `metalness`, `normal_strength`.
  * `TerrainMaterialPalette` — indexed collection of materials; `get(layer_index)` returns a default if out of range.
- [x] Grass / foliage instancing on terrain surface — `crates/terrain/src/foliage.rs`.
  * `FoliageInstance` — `position`, `scale`, `rotation`, `layer_index`; `to_matrix()` returns a 4x4 model matrix.
  * `FoliageLayer` — placement constraints (`height_range`, `max_slope`, `scale_range`).
  * `scatter_foliage(heightmap, world_scale, density, layers, random_fn)` — scatters instances across the terrain grid.
- [x] Terrain collision mesh for physics — `crates/terrain/src/lib.rs`.
  * `build_collision_mesh(heightmap, scale)` — generates `(Vec<[f32;3]>, Vec<[u32;3]>)` triangle soup suitable for trimesh colliders.
- [x] Real-time sculpting brush (editor tool) — `crates/terrain/src/sculpt.rs`.
  * `SculptBrush` — `radius`, `strength`, `mode` (Raise, Lower, Flatten, Smooth), `falloff`.
  * `apply(heightmap, world_scale, center_x, center_z)` — modifies heights within the brush radius with smooth falloff.
- [x] Erosion simulation (thermal, hydraulic) — `crates/terrain/src/erosion.rs`.
  * `thermal_erosion(heightmap, params)` — cellular talus-angle erosion that slides material from steep slopes to neighbors.
  * `hydraulic_erosion(heightmap, params)` — simplified rain-dissolve-flow-evaporation cycle.
- [x] Water plane integration (shoreline detection) — `crates/terrain/src/water.rs`.
  * `find_shoreline(heightmap, water_level, tolerance)` — returns cells whose height is near the water level.
  * `find_water_body(heightmap, water_level, start_x, start_z)` — flood-fills connected underwater cells.
  * `water_stats(heightmap, water_level)` — returns shoreline length and maximum water depth.
- [x] Terrain streaming (load/unload chunks based on camera distance) — `crates/terrain/src/chunk.rs`.
  * `ChunkedTerrain::stream_chunks(heightmap, view_x, view_z, max_radius)` — retains only chunks within radius and spawns new nearby chunks with appropriate LOD.

## 13. WORLD SYSTEM (crates/world)

- [x] Scene graph / entity hierarchy (parent-child transforms) — `crates/world/src/scene_graph.rs`.
  * `Parent(entity)` / `Children { entities }` — ECS components for hierarchy links.
  * `LocalTransform` — translation, rotation, scale relative to parent.
  * `GlobalTransform` — world-space matrix computed by `propagate_transforms(world)`.
  * `compute_hierarchy_depth_first(world)` — topological sort for correct update order.
- [x] World serialization (`.rxworld` format: entities, components, assets) — `crates/world/src/serialization.rs`.
  * `SerializedEntity` — `id`, `components` map, optional `parent`.
  * `WorldSnapshot` — versioned container with entity list and asset manifest.
  * `WorldSerializer` / `WorldDeserializer` — snapshot and restore ECS worlds.
- [x] Prefab system (template entities, nested prefabs, overrides) — `crates/asset/src/prefab.rs`.
  * `Prefab` asset format with entity hierarchy, transforms, meshes, materials, lights, physics, scripts.
  * `PrefabColliderShape`, `PrefabCollider`, `PrefabBodyType` for physics definitions.
  * Prefabs are serialized as RON inside a binary wrapper (magic + version + length-prefixed RON).
- [x] Level streaming (load/unload regions based on player position) — `crates/world/src/lib.rs`.
  * `ChunkCoord` / `ChunkState` (Unloaded, Loading, Loaded, Unloading).
  * `ChunkManager` — `update(world_x, world_z)` returns `(to_load, to_unload)` lists based on `load_radius`.
  * `mark_loading`, `mark_loaded`, `mark_unloaded` — state machine transitions.
- [x] World partition (spatial hash or octree for queries) — `crates/world/src/spatial.rs`.
  * `SpatialHash` — uniform 3D grid with `insert(entity, pos)`, `remove(entity, pos)`, `update(entity, old_pos, new_pos)`.
  * `query_cell(pos)` — returns all entities in the same cell.
  * `query_sphere(pos, radius)` — returns all entities in cells overlapping the sphere.
- [x] Time-of-day system (sun/moon cycle, dynamic sky, light color) — `crates/world/src/time_of_day.rs`.
  * `TimeOfDay { hours }` — 24-hour cycle; `advance(delta_hours)` wraps modulo 24.
  * `sun_direction()` / `moon_direction()` — simple arc across the sky.
  * `ambient_color()` / `sun_color()` — RGB colors keyed to time-of-day (dawn/day/dusk/night).
- [x] Weather system (rain, snow, fog, wind, procedural clouds) — `crates/world/src/weather.rs`.
  * `WeatherState` — `precipitation`, `snow_factor`, `fog_density`, `fog_color`, `wind`, `cloud_coverage`.
  * `rain(intensity)` / `snow(intensity)` / `clear()` builder methods.
  * `is_precipitating()` / `is_raining()` / `is_snowing()` predicates.
  * `lerp_weather(a, b, t)` — smooth blending between weather states.
- [x] Save/load with versioning (migration paths for old save formats) — `crates/world/src/save_load.rs`.
  * `SaveHeader` — magic `RXSV`, version, checksum.
  * `SaveMigrator` — registers `MigrationFn` callbacks per version step; `migrate(data, version)` upgrades save data to current version.
- [x] Multi-scene support (main world + UI scenes + additive sub-scenes) — `crates/world/src/multi_scene.rs`.
  * `Scene` — named ECS world with `active` and `loaded` flags.
  * `SceneManager` — `load_scene(name)`, `unload_scene(index)`, `set_active(index, active)`, `active_scenes()` iterator.
- [x] Editor-only metadata (gizmos, layer visibility, selection) — `crates/world/src/editor_meta.rs`.
  * `EditorMetadata` — per-entity `visible`, `locked`, `layer`, `selected`, `gizmo_mode`.
  * `EditorState` — global selection list, visible layers, snap settings.
  * `EditorLayer` enum — Default, Gizmos, UI, Terrain, Vegetation, Custom.
  * `GizmoMode` — Translate, Rotate, Scale.

## 14. EDITOR SYSTEM (crates/editor)

- [x] Editor plugin architecture (register panels, tools, gizmos) — `crates/editor/src/plugin.rs`.
  * `PanelId` / `ToolId` — unique string identifiers.
  * `EditorPanel` / `EditorTool` traits — `id()`, `title()`, `draw()`, `activate()`, `deactivate()`.
  * `PluginRegistry` — `register_panel`, `register_tool`, `find_panel`, `find_panel_mut`, `find_tool`.
- [x] Viewport camera controls (orbit, fly, fps modes) — `crates/editor/src/camera.rs`.
  * `EditorCamera` — position, target, yaw, pitch, distance, FOV, near/far.
  * `CameraMode` — `Orbit`, `Fly`, `Fps`.
  * `orbit_drag`, `orbit_zoom`, `orbit_pan`, `fly_move`, `fly_look` — camera manipulation methods.
  * `view_matrix()` / `projection_matrix(aspect)` — standard view/projection matrices.
- [x] Entity selection (click to pick, multi-select, hierarchy) — `crates/editor/src/lib.rs`.
  * `SelectionState` — `selected: Option<Entity>`, `gizmo_mode`, `gizmo_active`, `gizmo_start_pos`, `gizmo_start_mouse`.
  * `point_line_distance(point, line_start, line_end)` — screen-space distance utility.
  * `gizmo_screen_size(world_pos, view_proj, screen_height)` — constant screen-size gizmo scaling.
- [x] Transform gizmos (translate, rotate, scale with snapping) — `crates/editor/src/lib.rs`.
  * `GizmoMode` — `Translate`, `Rotate`, `Scale`.
  * `GizmoAxis` — `X`, `Y`, `Z`, `XY`, `XZ`, `YZ`, `XYZ`.
  * `AXIS_COLORS` — red for X, green for Y, blue for Z.
- [x] Undo/redo system (command pattern for all mutations) — `crates/editor/src/undo.rs`.
  * `Command` trait — `execute()`, `undo()`, `name()`.
  * `UndoStack` — `execute(cmd)`, `undo()`, `redo()`, `can_undo()`, `can_redo()`, `clear()`, `max_size`.
- [x] Scene hierarchy panel (tree view, drag-drop reparenting) — `crates/editor/src/hierarchy.rs`.
  * `HierarchyNode` — entity, name, children, expanded.
  * `flatten_hierarchy(nodes)` — produces `FlatNode` list with depth for UI rendering.
  * `ReparentCommand` — stores entity, old parent, new parent for undo support.
- [x] Inspector panel (component fields, add/remove components) — `crates/editor/src/inspector.rs`.
  * `FieldValue` — `Float`, `Int`, `Bool`, `String`, `Vec3`, `Color`.
  * `ComponentDesc` — `type_name` + list of `FieldDesc`.
  * `InspectorState` — `set_components`, `add_component`, `remove_component(type_id)`.
- [x] Asset Browser (file tree, thumbnails, drag-drop into scene) — `crates/editor/src/asset_browser.rs`.
  * `AssetEntry::Folder` / `AssetEntry::File` — path, name, asset type.
  * `AssetBrowserState` — root path, entries, selected, search filter; `filtered_entries()` search support.
- [x] Console panel (log levels, filtering, command input) — `crates/editor/src/console.rs`.
  * `LogLevel` — `Debug`, `Info`, `Warning`, `Error`.
  * `ConsoleState` — `log(level, message, time)`, `filtered_entries()`, `submit_command()`, `history_up/down`.
- [x] Profiler panel (Tracy integration, frame time breakdown) — `crates/editor/src/profiler.rs`.
  * `ProfileSample` — name + duration in ms.
  * `ProfilerState` — ring buffer of frame times; `begin_frame`, `add_sample`, `end_frame`.
  * `average_fps()`, `frame_time_min()`, `frame_time_max()` statistics.
- [x] Material editor (node graph or property panel) — `crates/editor/src/material_editor.rs`.
  * `MaterialProperty` — `Albedo`, `Roughness`, `Metalness`, `NormalStrength`, `Emissive`, `EmissiveIntensity`, `TextureSlot`.
  * `MaterialEditorState` — property list with `add_property` and `set_property`.
- [x] Lighting editor (place lights, bake lightmaps, IBL probes) — `crates/editor/src/lighting_editor.rs`.
  * `EditableLightType` — `Directional`, `Point`, `Spot`, `Area`.
  * `EditableLight` — name, type, position, direction, color, intensity, range, spot angle, shadow casting.
  * `IblProbe` — position, radius, optional cubemap path.
  * `LightingEditorState` — light list, selected light, IBL probes, bake progress.
- [x] Animation editor (timeline, keyframe editing, state machine graph) — `crates/editor/src/animation_editor.rs`.
  * `Keyframe` — time, value (`Float`, `Vec3`, `Quat`, `Bool`), interpolation type.
  * `InterpolationType` — `Step`, `Linear`, `Smooth`.
  * `AnimationTrack` — named track of keyframes with `add_keyframe` and `remove_keyframe_at`.
  * `TimelineState` — current time, duration, play/pause/stop/seek, loop, speed, tracks.
- [x] Terrain editor (sculpt, paint, vegetation placement) — `crates/editor/src/terrain_editor.rs`.
  * `TerrainEditMode` — `Sculpt`, `Paint`, `Vegetation`, `Smooth`, `Flatten`.
  * `TerrainEditorState` — wraps `SculptBrush`, paint layer, vegetation density; `apply_brush(heightmap, world_scale, x, z)`.
- [x] Play-in-editor mode (launch runtime without separate build) — `crates/editor/src/play_mode.rs`.
  * `PlayModeState` — `Editing`, `Playing`, `Paused`.
  * `PlayModeController` — `enter_play_mode(saved_scene)`, `exit_play_mode()`, `pause()`, `resume()`, `is_playing()`.
- [x] Build & deploy pipeline (cook assets, package executable) — `crates/editor/src/build_pipeline.rs`.
  * `BuildTarget` — `Windows`, `Linux`, `MacOS`, `WebAssembly`.
  * `BuildProfile` — `Debug`, `Release`, `Shipping`.
  * `BuildConfig` — target, profile, output dir, cook assets, compress textures, strip debug.
  * `BuildPipeline` — `start()`, `set_step()`, `log()`, `error()`, `finish()` with progress tracking.

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
| Editor layout | 0.5 | Medium | High | **DONE** (functional panels) |
| File dialogs | 0.5 | Low | High | **DONE** |
| Recent projects | 0.5 | Low | Medium | **DONE** |
| Project serialization | 0.5 | Low | High | **DONE** |
| Console log capture (tracing → editor) | 0.5 | Low | High | **DONE** |
| ECS → Hierarchy/Inspector | 0.5 | Medium | High | **DONE** |
| Offscreen scene rendering | 0.5 | Medium | High | **DONE** |
| Render target / framebuffer management | 1 | Medium | Critical | **DONE** |
| MSAA render targets (for quality levels) | 1 | Medium | High | **DONE** |
| Frame graph | 1 | High | Critical | **DONE** |
| Pipeline layout cache | 1 | Low | Medium | **DONE** |
| PBR shading | 1 | High | Critical | **DONE** |
| Asset system (handles + async loading) | 1 | High | Critical | **DONE** (handles, async server, importer, streaming, hot reload, VFS, cache, dependency graph, cook pipeline) |
| Physics integration | 1 | Medium | High | **PARTIAL** (rapier backend + ECS components + raycast) |
| Audio | 1 | Medium | Medium | **DONE** (decode, playback, spatial, effects, streaming, waveform, preview) |
| Animation | 2 | High | High | **DONE** (skeleton, state machine, IK, keyframes, tracks, blending, events) |
| World streaming | 2 | High | High | **DONE** (chunk manager, LOD, streaming, scene graph, time/weather) |
| Terrain | 2 | High | Medium | **DONE** (noise, import, LOD, chunks, splat, PBR, foliage, sculpt, erosion, water) |
| Shader hot-reload | 1 | Medium | High | **DONE** |
| GPU timestamp queries | 1 | Low | Medium | **DONE** |
| Windows build (Win32 + Vulkan) | 2 | Medium | High | **DONE** |
| macOS build (MoltenVK + Metal surface) | 2 | Medium | High | **—** |
| CI: GitHub Actions matrix | 2 | Low | Medium | **DONE** (Linux + Windows + macOS jobs with build, test, clippy, fmt) |
| RenderDoc capture trigger | 1 | Low | Low | **DONE** (atomic trigger/consume API) |
| UI framework | 2 | Medium | High | **DONE** (immediate mode HUD, menus, text rendering, layout, GPU overlay) |
| Networking | 3 | Very High | High | **DONE** (UDP, protocol, serialization, prediction, interpolation, lag comp, replication, authority, bandwidth, NAT punch) |
| AI | 3 | Medium | Medium | **DONE** (navmesh, pathfinding, steering, behavior trees, FSM, sensors, GOAP, utility, influence, debug draw) |
| Scripting | 3 | High | Medium | **DONE** (Rhai engine, Script asset, ECS bindings, hot reload) |
| Full editor | 4 | Very High | Low | **DONE** (plugins, camera, selection, gizmos, undo, hierarchy, inspector, asset browser, console, profiler, material/lighting/animation/terrain editors, play mode, build pipeline) |

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
