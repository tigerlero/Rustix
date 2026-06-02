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
- [~] Thread affinity (pinning to physical cores on Linux) — configured but not functional
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
- [ ] Log rotation in release builds

---

## 2. PLATFORM (crates/platform)

### 2.1 Windowing
- [x] Wayland native support (primary target for Pop!_OS)
- [x] X11 fallback (xcb backend)
- [x] Fullscreen exclusive (when display server allows) — `FullscreenMode::Exclusive` in `crates/platform/src/window.rs` picks the best video mode on the current monitor (largest resolution, then highest refresh rate) and passes it to `winit::window::Fullscreen::Exclusive`. Falls back to borderless if no video modes are available or no monitor is detected. `FullscreenMode::Borderless` fills the screen without changing the display video mode. Both modes are applied at window creation time if `WindowConfig.fullscreen` is set, and can be toggled at runtime via `WindowHandle::set_fullscreen_mode()` and `WindowHandle::toggle_fullscreen()`.
- [x] Window resize handling (swapchain recreation)
- [x] Multiple window support (editor: N viewports)
- [x] DPI-aware scaling
- [ ] Cursor mode: normal, hidden, captured, raw-delta

### 2.2 Input
- [x] Keyboard: winit fallback (evdev raw input planned)
- [x] Mouse: absolute + raw delta motion
- [ ] Gamepad: `gilrs` integration
- [x] Input state: current frame + previous frame (for "just pressed" detection)
- [ ] Input actions (abstract binding: "jump" → Space / A-button)
- [ ] Bindable key remapping (config file)
- [ ] Text input (IME-aware for Wayland)
- [ ] Touch input (surface)
- [ ] Input recording + playback (testing / demos)

### 2.3 OS Abstraction
- [x] High-resolution timer (monotonic clock)
- [ ] Thread naming (pthread_setname_np on Linux)
- [ ] Thread priority (SCHED_FIFO or SCHED_RR for audio/render threads)
- [ ] Memory mapping (mmap for asset loading)
- [ ] File watcher (inotify on Linux, ReadDirectoryChangesW on Windows)
- [ ] Clipboard access
- [x] File dialog (`rfd` native picker for project open/create)

---

## 3. RENDERER (crates/render)

### 3.1 Vulkan Backend
- [x] Instance creation with validation layers (debug)
- [x] Physical device selection (NVIDIA preference scoring)
- [x] Logical device with queue families (graphics, present)
- [x] Surface creation (Wayland/Xlib/Xcb Vulkan KHR)
- [x] Swapchain (mailbox present mode, triple buffering)
- [x] Dynamic rendering (VK_KHR_dynamic_rendering, no RenderPass objects)
- [x] Pipeline cache (VK_KHR_pipeline_cache, in-memory)
- [ ] Timeline semaphores (VK_KHR_timeline_semaphore)
- [ ] Synchronization2 (VK_KHR_synchronization2)

### 3.2 GPU Memory
- [ ] Device-local memory (VRAM for textures, buffers)
- [ ] Host-visible coherent memory (staging, streaming UBOs)
- [ ] Memory allocator (gpu-allocator integration)
- [ ] Dedicated transfer queue for async upload
- [x] Staging buffer pool (ring-buffer, recycled) — see `GpuStagingRing` / `GpuStagingBuffer` in Memory Management (1.3) and `crates/render/src/memory.rs`.
- [ ] GPU readback (profiling counters, occlusion queries)

### 3.3 Descriptors
- [ ] Bindless descriptor model (global heap)
- [ ] Descriptor set layout cache
- [ ] Sampler cache (reuse sampler objects)
- [ ] Descriptor update batching

### 3.4 Pipelines
- [x] Graphics pipeline cache (hash-based key → VkPipeline)
- [ ] Compute pipeline cache
- [ ] Pipeline variants (forward/deferred, quality levels)
- [ ] Specialization constants (reducing shader variants)

### 3.5 Shaders
- [~] GLSL source → SPIR-V via glslangValidator (SPIR-V module loading done, compilation not yet)
- [ ] Runtime shader compilation (editor / debug)
- [ ] SPIR-V reflection (resource binding, push constants)
- [ ] Shader hot-reload (watch source files, rebuild pipelines)
- [ ] Shader include system (#include resolution)
- [ ] Pre-compiled shader archive for release builds

### 3.6 Frame Graph
- [ ] Declarative render graph
- [ ] Automatic resource barriers
- [ ] Transient resource memory aliasing
- [ ] Render pass merging
- [ ] Async compute pass scheduling
- [ ] Frame graph visualization (debug overlay)

### 3.7 Rendering Features
- [ ] Forward+ lighting (tiled light culling)
- [ ] Deferred shading (GBuffer: albedo, normal, PBR params, depth)
- [ ] Directional light with cascaded shadow maps (CSM)
- [ ] Point/spot lights (shadow maps + cubemap arrays)
- [ ] PBR material system (metal-rough workflow)
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

### 3.8 Debug / Profiling
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
- [ ] Project serialization (.rustixproj save/load)
- [ ] Recent projects persistence (disk)

### 4.3 Editor Layout
- [x] Menu bar: File, Edit, Assets, Help menus + FPS counter
- [x] File → Back to Project Hub (screen switching)
- [x] Hierarchy panel (left side, placeholder)
- [x] Inspector panel (right side, placeholder)
- [x] Console / Asset Browser (bottom tabs, placeholder)
- [x] Scene View (central panel, clear color only)
- [x] EditorCamera with orbit controls (WASDQE + mouse drag)

### 4.4 Planned Editor Features
- [ ] ECS entity tree in Hierarchy panel
- [ ] Component editing in Inspector panel
- [ ] Offscreen 3D rendering into Scene View
- [ ] Real log capture via tracing subscriber → Console ring buffer
- [ ] Asset file listing → Asset Browser
- [ ] Entity selection (click in scene or hierarchy)
- [ ] Gizmos (translate, rotate, scale)
- [ ] Undo/redo system
- [ ] Docking / panel rearrangement
- [ ] Layout persistence

### 4.5 Custom UI Framework (crates/ui)
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
| Project serialization | 1 | Low | High | **—** |
| ECS → Hierarchy/Inspector | 1 | Medium | High | **—** |
| Offscreen scene rendering | 1 | Medium | High | **—** |
| Frame graph | 1 | High | Critical | **—** |
| PBR shading | 1 | High | Critical | **—** |
| Asset system | 1 | High | Critical | **—** |
| Physics | 1 | Medium | High | **—** |
| Audio | 1 | Medium | Medium | **—** |
| Animation | 2 | High | High | **—** |
| World streaming | 2 | High | High | **—** |
| Terrain | 2 | High | Medium | **—** |
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
