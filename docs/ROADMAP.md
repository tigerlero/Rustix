# Rustix Engine — Development Roadmap

## Phase 0: Foundation — COMPLETE ✅

**Goal:** Bootstrap the engine skeleton — window opens, Vulkan initializes, triangle renders.  
**Status:** All Phase 0 goals achieved. Workspace compiles cleanly with `cargo check`.

### What was built

- [x] 17-crate Cargo workspace with Cargo.toml and `.cargo/config.toml`
- [x] `rustix-core`: ECS (hecs) + job system (rayon) + math (glam + bounds/ray/color) + memory (frame/pool allocators) + logging (tracing) + config (TOML + layered merge)
- [x] `rustix-platform`: Window (winit 0.30, Wayland + X11) + input (keyboard, mouse, gamepad stubs, winit event forwarding)
- [x] `rustix-render`: Vulkan 1.3 (ash 0.38) — instance, device (NVIDIA preference), surface (Wayland/Xlib/Xcb), swapchain (triple-buffered, mailbox), dynamic rendering pipeline, built-in SPIR-V shaders, texture creation
- [x] `rustix-engine`: Plugin trait + AppBuilder + Schedule with stage-based ordering + App struct
- [x] `rustix-runtime`: Binary entry point — game loop (120Hz fixed update + variable render), Vulkan triangle render
- [x] 13 stub crates: asset, physics, audio, animation, networking, scripting, ui, ai, terrain, world, editor

### Deferred from original Phase 0 plan
- [ ] RenderDoc capture trigger (F12) — UI layer needed first
- [ ] Tracy profiling markers (CPU zones active, GPU zones need query infrastructure)
- [ ] Full plugin lifecycle integration (plugins register via builder but not auto-loaded by App)
- [ ] Fullscreen mode — winit 0.30 `ActiveEventLoop` migration required

---

## Phase 0.5: Editor UI — COMPLETE ✅

**Goal:** egui-based editor overlay with project management and editor panels.
**Status:** All Phase 0.5 goals achieved.

### Completed
- [x] Custom egui Vulkan renderer (WGSL fragment shader, separate texture+sampler)
- [x] Correct Y-down coordinate system (no flips needed — egui matches Vulkan NDC)
- [x] Font rendering via font atlas upload + descriptor update
- [x] Native file dialogs via `rfd` for New/Open Project
- [x] Project Hub startup screen with branding and recent projects list
- [x] Editor screen: menu bar, hierarchy, inspector, console, scene view panels
- [x] EditorCamera with orbit + first-person modes, follow-target toggle
- [x] Recent project tracking (persisted to disk via `dirs`)
- [x] Project switching between Hub and Editor
- [x] Project serialization — save/load `.rustixproj` with full scene data
- [x] ECS entity tree → Hierarchy panel with rename, delete, create, context menus
- [x] Component editing → Inspector panel (Transform, DirectionalLight, PointLight, SpotLight)
- [x] Real log capture → Console panel (tracing subscriber → ring buffer, color-coded)
- [x] Asset file listing → Asset Browser (with type icons and coloring)
- [x] Window resize handling (swapchain + depth buffer recreation)
- [x] Undo/redo system (Ctrl+Z/Ctrl+Shift+Z)
- [x] Gizmos (translate handles for X/Y/Z axes)
- [x] Per-entity mesh rendering (mesh registry, GLB import, procedural presets)
- [x] 3D scene grid overlay with entity position dots in scene view
- [x] Vulkan synchronization fixes (per-frame semaphores, proper fence ordering)
- [x] Resource cleanup (Drop impls for Renderer, Swapchain, GpuBuffer, ShaderModule)

### Deferred to Phase 1
- [x] Offscreen 3D rendering → Scene View panel (triple-buffered per-viewport framebuffers, HDR path)
- [x] Multiple viewport support (up to 4, independent cameras)
- [ ] Docking / panel rearrangement

---

## Phase 1: Core Rendering + Asset Pipeline (Weeks 5–12)

**Goal:** Full PBR renderer with asset loading, physics, audio, and input.

### Weeks 5–6: Frame Graph & GPU Memory
- [x] Frame graph architecture (pass declaration, resource tracking, automatic barriers, transient aliasing, pass merging)
- [x] Automatic barrier insertion
- [x] GPU memory manager (gpu-allocator integration)
- [x] Staging buffer pool (ring-buffer for upload)
- [x] Transient resource management (per-frame render targets)
- [x] Bindless descriptor manager (global descriptor heap)
- [ ] Pipeline cache persistence (disk-backed)

### Weeks 7–8: PBR Rendering
- [x] Forward+ renderer (tiled light culling compute shader, 256 lights / 32 per tile)
- [x] Deferred shading pipeline (GBuffer pass + lighting pass)
- [x] PBR material system (metal-rough workflow, Cook-Torrance GGX)
- [x] Directional light with cascaded shadow maps (CSM, 3 cascades @ 2048)
- [x] Point/spot lights with shadow maps (cubemap array + 2D array)
- [x] HDR rendering + ACES filmic tone mapping
- [x] Mesh rendering: vertex buffers, index buffers, procedural presets
- [~] glTF 2.0 mesh loading (GLB positions/normals + material import working; tangents/UVs/animations pending)

### Weeks 9–10: Asset Pipeline
- [ ] Asset registry with handle system (8-byte `Handle<T>`)
- [ ] Asset importer framework (per-type plugin)
- [~] glTF 2.0 full import (meshes, materials partial; textures, animations, skeletons pending)
- [ ] Texture loading (PNG, HDR) with BC7 compression
- [ ] Async asset loading (tokio runtime)
- [ ] GPU upload via transfer queue
- [ ] Asset dependency tracking (material → textures)
- [x] Hot-reload watcher (notify crate) for debug mode
- [x] Shader compilation pipeline (GLSL → SPIR-V via naga)
- [x] Shader hot-reload (file watch → recompile → pipeline rebuild)

### Weeks 11–12: Physics + Audio + Input
- [~] Physics integration (RigidBody + Collider ECS components exist; Rapier3D solver not wired)
- [~] Audio integration (symphonia decode + rodio playback done; 3D spatial + effects pending)
- [x] Input system: keyboard, mouse (absolute + raw delta), gamepad via gilrs

---

## Phase 2: World Building + Animation (Weeks 13–20)

**Goal:** Open world terrain, animation system, UI framework, world streaming.

- [ ] Terrain system (heightmap + LOD + collision)
- [ ] Animation system (skeleton + skinning + blend trees + state machine)
- [ ] World streaming (chunks + persistence + LOD transitions)
- [ ] UI framework (immediate mode + widget library)

---

## Phase 3: Multiplayer + AI (Weeks 21–30)

**Goal:** MMO networking, AI system, scripting, performance optimization.

- [ ] QUIC transport (quinn) + authoritative server + replication
- [ ] AI system (navmesh + pathfinding + behavior trees)
- [ ] WASM scripting (wasmtime) + hot-reload + host API
- [ ] Performance: GPU-driven culling, mesh LOD, texture streaming

---

## Phase 4: Editor + Polish (Weeks 31–40)

**Goal:** Game editor, shipping-quality rendering, comprehensive testing.

- [ ] Editor (scene view + entity tree + inspector + gizmos)
- [ ] Rendering polish (TAA + SSR + bloom + SSAO + volumetric fog)
- [ ] Testing (integration tests + benchmarks + memory analysis)

---

## Phase 5: Production & Scale (Weeks 41–52)

**Goal:** Ship a tech demo, prepare for production use.

- [ ] Large world validation (64km²+)
- [ ] Network stress testing (100+ simulated clients)
- [ ] Windows platform support
- [ ] Documentation + v0.1.0 release

---

## Key Milestones

| Milestone | Phase | Status | Deliverable |
|-----------|-------|--------|-------------|
| Window + Triangle | 0 | ✅ **DONE** | Window opens, Vulkan triangle render pipeline ready |
| egui Overlay | 0.5 | ✅ **DONE** | Text renders, panels layout, file dialogs work |
| Project Hub | 0.5 | ✅ **DONE** | Startup screen with recent projects + New/Open |
| Editor Layout | 0.5 | ✅ **DONE** | Menu bar, hierarchy, inspector, console, scene view |
| Project Persistence | 0.5 | ✅ **DONE** | .rustixproj save/load with scene data + mesh refs |
| Scene → Hierarchy | 0.5 | ✅ **DONE** | ECS entities shown in hierarchy panel with interactions |
| Inspector + Gizmos | 0.5 | ✅ **DONE** | Component editing + translate gizmos |
| Camera Modes | 0.5 | ✅ **DONE** | Orbit + 1st person + follow-target toggle |
| Per-Entity Meshes | 0.5 | ✅ **DONE** | GLB import, mesh registry, procedural presets |
| Window Resize | 0.5 | ✅ **DONE** | Swapchain + depth buffer recreation |
| Vulkan Sync | 0.5 | ✅ **DONE** | Per-frame semaphores, fence ordering, resource cleanup |
| Frame Graph | 1 | ✅ **DONE** | Declarative render passes with auto barriers |
| Forward+ / Deferred | 1 | ✅ **DONE** | Tiled light culling + GBuffer + lighting |
| PBR + Shadows | 1 | ✅ **DONE** | Cook-Torrance BRDF, CSM, point/spot shadows |
| HDR + Tonemap | 1 | ✅ **DONE** | ACES filmic tone mapping |
| glTF Model Viewer | 1 | ~ **PARTIAL** | Import and render GLB meshes with materials |
| Physics + Audio | 1 | ~ **PARTIAL** | ECS components exist; solver/spatial audio pending |
| Open World Terrain | 2 | — | Walkable terrain that streams in/out |
| Animating Character | 2 | — | Character with animation state machine |
| Two Players Connected | 3 | — | Peer-to-peer or server multiplayer |
| AI NPC Patrol | 3 | — | NPCs navigating and behaving |
| Editor MVP | 4 | — | Scene editing with entity tree + inspector |
| Tech Demo | 5 | — | Playable tech demo |

---

## Timeline Summary

```
Phase 0: Foundation           ✅ COMPLETE
Phase 0.5: Editor UI          ✅ COMPLETE
Phase 1: Core Render + Assets  ████████████░░░░
Phase 2: World + Animation     ████████░░░░░░░░
Phase 3: Multiplayer + AI      ██████████░░░░░░░░
Phase 4: Editor + Polish       ████████████░░░░░░░░
Phase 5: Production & Scale    ████████████████░░░░
```
