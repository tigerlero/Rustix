# Rustix Engine — Feature Breakdown

Legend: `[x]` = implemented, `[ ]` = planned, `[~]` = partial

---

## 1. CORE (crates/core)

### 1.1 Entity Component System
- [x] Archetypal ECS via `hecs`
- [ ] Component registration with type-erased storage
- [x] Query filters: `With`, `Without`
- [ ] Dynamic bundles (runtime component addition)
- [ ] Command buffers for deferred world mutation
- [ ] Change detection (dirty flags per component per tick)
- [ ] Component grouping for cache-optimal iteration
- [ ] Multi-world support (game world, editor world, preview world)

### 1.2 Job / Task System
- [x] Rayon-based work-stealing thread pool
- [ ] Explicit task graph with dependency edges
- [x] Fork-join parallelism API
- [~] Thread affinity (pinning to physical cores on Linux) — configured but not functional
- [ ] Task priorities (high for render, medium for gameplay, low for streaming)
- [ ] Job profiling (Tracy integration per task)
- [ ] Dynamic thread count (respond to system load)

### 1.3 Memory Management
- [x] Frame allocator (per-frame bump allocation, O(1) reset)
- [x] Pool allocator (fixed-size object reuse)
- [ ] Thread-local arenas (reduce contention)
- [x] Cache-line aligned allocations (avoid false sharing, align 64)
- [ ] Memory tracker (leak detection, allocation statistics)
- [ ] Custom allocators for ECS component storage (SoA layout)
- [ ] GPU staging buffer allocator (coherent, mapped, ring-buffer)

### 1.4 Math Library
- [x] `glam` re-export: Vec2/3/4, Mat3/4, Quat, Affine3A
- [x] Bounding volumes: AABB, Sphere, Frustum
- [x] Ray structs for intersection queries
- [x] Color types (linear vs sRGB conversion)
- [ ] Transform hierarchy (local → world matrix computation)
- [x] Interpolation: lerp, smoothstep, smootherstep

### 1.5 Configuration
- [x] TOML-based engine configuration
- [ ] Runtime config reload (monitor config file for changes)
- [x] Layered configs: default → project → user → CLI overrides
- [ ] Hot-key toggles (dev mode, debug rendering, profiling)

### 1.6 Diagnostics
- [x] Structured logging via `tracing`
- [x] Console output (colored, with span tracking)
- [ ] JSON file logging for automated analysis
- [x] Log levels: error, warn, info, debug, trace
- [x] Per-crate log level filtering
- [ ] Log rotation in release builds

---

## 2. PLATFORM (crates/platform)

### 2.1 Windowing
- [x] Wayland native support (primary target for Pop!_OS)
- [x] X11 fallback (xcb backend)
- [ ] Fullscreen exclusive (when display server allows)
- [ ] Borderless fullscreen windowed
- [ ] Window resize handling (swapchain recreation)
- [ ] Multiple window support (editor: N viewports)
- [ ] DPI-aware scaling
- [ ] Cursor mode: normal, hidden, captured, raw-delta

### 2.2 Input
- [~] Keyboard: winit fallback (evdev raw input planned)
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
- [ ] Staging buffer pool (ring-buffer, recycled)
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

## 6-14. REMAINING SUBSYSTEMS

All crates (`physics`, `audio`, `animation`, `networking`, `scripting`, `ai`, `terrain`, `world`, `editor`) are currently **empty stubs** — they compile with no implementation.

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
