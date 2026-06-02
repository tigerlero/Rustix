# Rustix Engine — Feature Progress & Completion

**Last updated:** 2026-06-02  
**Rust:** 1.95.0 stable  
**Total source:** ~22,300 lines across 16 crates + 1 binary  

---

## Overall Status

```
Core          ██████████████░░  85%  ECS, jobs, math, memory, config, diagnostics
Platform      ████████░░░░░░░░  55%  Windowing, input (keyboard+mouse), fullscreen
Renderer      ███████░░░░░░░░░  40%  Vulkan backend done, staging, PBR/lighting not started
Audio         ██████████░░░░  65%  Decoding, spatial, effects all implemented
Scripting     ████████░░░░░░  50%  Rhai engine, ECS bridge — needs hot-reload
AI            ███████░░░░░░░  45%  Behavior trees, A* pathfinding, navmesh
Editor UI     █████████████░░  85%  egui editor with full text rendering, panels, gizmos, offscreen viewport
Game UI       █████░░░░░░░░░  20%  Button/slider/label stubs, no text rendering
Asset System  ██████░░░░░░░░  30%  Handle system, glTF mesh import, hot-reload
Engine Facade ██████████████  85%  Plugin trait, AppBuilder, Schedule
Runtime       █████████████░░  75%  Editor UI, project mgmt, gizmos, undo/redo, offscreen viewport
Physics       ░░░░░░░░░░░░░░   0%  Rapier3D not integrated
Animation     ░░░░░░░░░░░░░░   0%  Not started
Networking    ░░░░░░░░░░░░░░   0%  Not started
Terrain       ░░░░░░░░░░░░░░   0%  Not started
World         ░░░░░░░░░░░░░░   0%  Not started
Editor Crate  ░░░░░░░░░░░░░░   0%  Not started

OVERALL       ██████████████████░░░░░░░░░░░░  35-40%
```

---

## 1. Core (`rustix-core`) — 85%

### Implemented
- ECS via `hecs` with custom `Schedule` + stage-based system ordering
- `Transform` component with full translation/rotation/scale chain
- `JobSystem` via rayon work-stealing pool
- `FrameAllocator` (bump) + `PoolAllocator` + `FrameMemory`
- `ThreadLocalArena` — per-thread bump allocator for zero-contention allocation
- `GpuStagingBuffer` — coherent mapped ring buffer with fence tracking for CPU→GPU uploads
- `Aligned<T>` (cache-line aligned, 64-byte)
- Math: `Aabb`, `Sphere`, `Frustum`, `Plane`, `Ray`, `Color`, lerp/smoothstep
- `EngineConfig` TOML loading with layered merge (default → project → user → CLI)
- `ConfigWatcher` — polling file watcher for runtime config reload with callback
- `DevToggles` + `HotkeyBindings` — atomic-bool toggles for dev/debug/profiling (F1/F2/F3)
- Structured logging via `tracing` with level/per-crate filtering
- `JsonFileLayer` — JSON Lines file logging with field escaping and rotation
- `LogCapture` — tracing subscriber → ring buffer (used by editor Console)
- `ComponentRegistry` — type-erased component storage via `TypeId` + vtable
- `CommandBuffer` — deferred world mutation (`Spawn`, `Despawn`, `InsertBundle`, `Remove`, etc.)
- `ChangeTracker` — dirty flags per component per tick
- `ComponentGroup` / `GroupRegistry` — named component sets for cache-optimal archetypes
- `WorldRegistry` — multi-world support (game, editor, preview) with entity mapping
- `TransformHierarchy` — BFS world matrix computation with cycle detection + topological ordering
- `TaskGraph` — DAG task dependency system with Kahn's sort + parallel execution
- `PriorityTaskSystem` — high/medium/low priority worker threads
- `SystemMonitor` — frame timing + per-system CPU cost tracking
- `MemoryTracker` — allocation tracking + high-water mark reporting
- `SoaStorage` — structure-of-arrays dense storage
- `ScriptComponent` (stub for scripting integration)

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| Thread affinity runtime | Low | `pthread_setaffinity_np` at pool start |
| Tracy profiling integration | Low | CPU/GPU zones per task |
| Log rotation in release | Low | File size limits, auto-rotation |
| Entity serialization | Medium | Save/load ECS world to disk format |

---

## 2. Platform (`rustix-platform`) — 55%

### Implemented
- `WindowHandle` wrapping `winit 0.30` — Wayland + X11
- `FullscreenMode` enum — exclusive (best video mode auto-select) + borderless + windowed
- `WindowHandle::set_fullscreen_mode()` / `toggle_fullscreen()` — runtime fullscreen toggle
- `InputManager` — keyboard (full keycode mapping), mouse (position/delta/scroll/buttons)
- Raw window handle access for Vulkan surface creation
- File dialog via `rfd` (native OS picker)

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| Gamepad input | Medium | `gilrs` integration, button/axis mapping |
| Input action system | High | Abstract bindings ("Jump" → Space/A button) in config |
| Text input (IME) | Medium | `winit` IME for Wayland text entry |
| Multi-window support | Medium | N viewports for editor (scene + debug + tools) |
| DPI-aware scaling | Medium | Handle `ScaleFactorChanged`, resize fonts |
| Cursor modes | Low | Normal/hidden/captured/raw-delta |
| Input recording/playback | Low | Demo/test automation |
| Touch input | Low | Surface/tablet support |

---

## 3. Renderer (`rustix-render`) — 35%

### Implemented
- Vulkan 1.3 via `ash 0.38` — instance, debug messenger, NVIDIA preference scoring
- Physical/logical device, queue families (graphics, present), pipeline cache
- Surface creation (Wayland, Xlib, Xcb)
- Triple-buffered swapchain, mailbox present mode, semaphore sync
- Dynamic rendering (no RenderPass objects)
- SPIR-V shader loading + built-in vertex/fragment shaders
- `GraphicsPipeline` + `GraphicsPipeline2D` with push constants
- `GpuMemoryAllocator` (via `gpu-allocator`), `GpuBuffer`, `StagingBufferPool`
- `FrameGraph` — resource/pass declaration, automatic barrier compilation
- Full texture creation + update pipeline (staging → transfer → shader-ready)
- `DepthBuffer`, `Framebuffer` (offscreen rendering)
- `GpuTexture` with sampler
- `draw_2d`, `draw_mesh`, `begin_scene_pass`/`end_scene_pass`
- Mesh: `Mesh` struct, `Vertex`, procedural generators (cube, sphere, plane, torus, cylinder)
- Components: `Camera`, `DirectionalLight`, `PointLight`, `SpotLight`, `MeshRenderer`, `Material`, `MaterialComponent`, `Sprite`, `SpriteRenderer`, `Parent`, `Children`, `Visible`, `CastShadows`

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| **PBR shading** | **HIGH** | Forward+ or deferred, metal-rough, IBL |
| **Directional light CSM** | **HIGH** | Cascaded shadow maps for sun light |
| Point/spot shadow maps | Medium | Cubemap array shadow rendering |
| HDR + tonemapping | Medium | Reinhard/ACES, exposure control |
| Frame graph runtime | Medium | Actual barrier execution, resource aliasing |
| Bindless descriptors | Medium | Global GPU descriptor heap |
| GPU-driven culling | Low | Compute shader frustum + occlusion culling |
| Pipeline variants | Low | Forward/deferred quality levels, specialization constants |
| Shader hot-reload | Low | Watch .glsl, recompile SPIR-V, rebuild pipelines |
| SPIR-V reflection | Low | Auto-detect bindings, push constants |
| Bloom | Low | Gaussian pyramid bloom |
| SSAO | Low | HBAO or equivalent |
| TAA | Low | Temporal anti-aliasing |
| SSR | Low | Screen-space reflections |
| Volumetric fog | Low | Ray marched fog |
| Skybox | Low | Cubemap or procedural atmosphere |
| Mesh shaders | Low | NVIDIA VK_NV_mesh_shader path |
| Timestamp queries | Low | GPU pass timing |
| RenderDoc trigger | Low | F12 capture |

---

## 4. Audio (`rustix-audio`) — 65%

### Implemented
- Multi-format decoding via `symphonia` (WAV, MP3, OGG/Vorbis, FLAC, AAC)
- Pure-Rust — no system audio libs at build time
- Hardware playback via `rodio` (feature-gated: `audio-playback`)
- `AudioEngine` — play/stop/pause/volume, looping, master volume
- `SoundInstance` — decoded sample access
- `StreamDecoder` — streaming for long files
- Spatial audio: `AudioListener`, `AudioSource`, distance attenuation, HRTF panning (ILD + ITD)
- `Compressor`, `Equalizer` (3-band), `Reverb` (Freeverb), `EffectChain`
- Unit tests for spatial, compressor, eq, reverb, effect chain
- Effect assets in `assets/sounds/` (click, beep, whoosh, thump)
- ECS components: `SoundPlayer`, `AudioSource`, `AudioListener`

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| Full ECS integration | Medium | Auto-play SoundPlayer, cleanup finished instances |
| Editor preview | Medium | Play button in Asset Browser → instant preview |
| Waveform visualization in-editor | Low | Real-time waveform in Console/Asset panel |
| Audio source gizmos | Low | 3D viewport indicators for positional audio |
| Audio bus/mixer graph | Low | Submix buses, groups, effects routing |
| WASAPI/CoreAudio backends | Low | Platform-native audio beyond ALSA |

---

## 5. Editor UI (egui in `apps/runtime`) — 80%

### Implemented
- Full egui-based editor with startup screen, editor panels, menu bar
- Complete text rendering via egui font atlas (NotoSans, NotoSansMono, NotoEmoji)
- Image widget (texture-backed via egui::Image)
- Text input (cursor, selection, copy/paste)
- Scrollable regions (clipped scrolling in hierarchy, inspector, console)
- Custom dark theme with `RichText` color/size modifiers
- Project Hub, Hierarchy, Inspector, Console, Asset Browser, Viewport panels
- Gizmo controls (translate handles for X/Y/Z axes)
- Undo/redo UI integration

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| Flexbox/grid layout | Medium | Flex/grid layout engine for game UI |
| CSS-like styling | Low | Color/font/border themes for game UI |
| Accessibility | Low | Screen reader API |

---

## 5b. Game UI Framework (`rustix-ui` crate) — 20%

### Implemented
- `UIContext` — immediate mode context
- `DrawList` — command list (rectangles, colored)
- `button` widget (hover/interaction state)
- `slider` widget
- `label` widget (placeholder — colored rect, no glyphs)
- Layout helpers: `vstack`, `center`

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| **Text rendering** | **HIGH** | Glyph atlas, font rasterization, kerning, wrapping |
| Image widget | Medium | Texture-backed image display |
| Text input | Medium | Cursor, selection, IME |
| Flexbox/grid layout | Medium | Flex/grid layout engine |
| Scrollable regions | Medium | Clipped scrolling containers |
| CSS-like styling | Low | Color/font/border themes |
| Accessibility | Low | Screen reader API |

---

## 6. Scripting (`rustix-scripting`) — 50%

### Implemented
- `ScriptEngine` — Rhai scripting engine initialization
- `Script` — source + config storage
- `ScriptInstance` — per-entity script runtime
- `ScriptApi` — host API registration (expose ECS, math, etc. to scripts)
- `ScriptLoader` — load `.rhai` files from assets
- `ScriptRegistry` — manage loaded scripts
- `ScriptError` enum
- `ScriptComponent` — attach scripts to entities

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| Hot-reload | Medium | Watch .rhai files, re-evaluate on change |
| Editor integration | Medium | Script editor panel, error display in Console |
| Full ECS API | Medium | Query ECS from Rhai, mutate components |
| Async scripting | Low | Non-blocking script execution |
| Script sandboxing | Low | Resource limits, recursion guards |

---

## 7. AI & Navigation (`rustix-ai`) — 45%

### Implemented
- `BehaviorTree` — `Status`, `Blackboard`, `BehaviorNode` trait
- Node types: `Action`, `Condition`, `Sequence`, `Selector`, `Invert`, `Repeat`
- `PathFinder` — A* grid pathfinding, A* graph pathfinding
- `NavMesh` — `NavTriangle` structure, `NavMesh` with adjacency

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| ECS integration | Medium | BehaviorTree component, AI system in Schedule |
| NavMesh baking | Medium | Generate navmesh from scene geometry |
| Agent movement | Medium | Steering, obstacle avoidance, path following |
| Editor gizmos | Low | Visualize navmesh, paths in scene view |
| State machine support | Low | HFSM in addition to behavior trees |
| Utility AI | Low | Utility theory scoring for decision making |

---

## 8. Asset System (`rustix-asset`) — 30%

### Implemented
- `Handle<T>` — 8-byte asset handle
- `AssetServer` — basic registry and load interface
- `HotReloadWatcher` — file watcher via `notify`
- `LoadState` enum — unloaded/loading/loaded/error
- `Importer` — RON/JSON import/export traits
- glTF 2.0 mesh + material import (via `gltf_loader.rs` in runtime)
- Asset file browser in console panel with type icons

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| **Texture loading** | **HIGH** | PNG/HDR/KTX2 → GPU upload pipeline |
| **Async loading** | **HIGH** | tokio IO on worker threads, GPU upload via transfer queue |
| Asset caching | Medium | Disk cache of cooked assets |
| Virtual file system | Medium | Mount points, path resolution, pack files |
| Texture compression | Medium | BC7/ASTC conversion at import time |
| Mesh optimization | Low | Vertex cache reordering, stripification |
| Asset dependency graph | Low | Material → texture hot-reload chain |
| Shader compilation pipeline | Medium | GLSL → SPIR-V at build time |
| glTF skeleton + animation | Medium | Skin, joints, animation clips |

---

## 9. Engine Facade (`rustix-engine`) — 85%

### Implemented
- `Plugin` trait — `build()` method for crate registration
- `AppBuilder` — plugin registration, system addition, config setup
- `Schedule` — stage-ordered system execution
- `App` struct — wraps ECS World, jobs, memory, config

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| Plugin lifecycle | Low | `on_start`, `on_stop`, `on_update` hooks |
| Plugin dependencies | Low | Declare plugin load order |
| Thread-local systems | Low | Systems that run per-thread (e.g., particle update) |

---

## 10. Runtime / Editor (`apps/runtime`) — 75%

### Implemented
- Custom egui Vulkan renderer (WGSL fragment shader, separate texture+sampler descriptor)
- Correct Y-down coordinate system (egui → Vulkan NDC match)
- Font atlas upload and texture update each frame
- Project Hub startup screen with branding, recent projects (disk-persisted), New/Open
- Editor layout: menu bar (File/Edit/Assets/Help + FPS), hierarchy, inspector, console, asset browser, scene view
- `EditorCamera` — orbit (RMB drag), first-person (WASDQE), follow-target toggle
- Project serialization: save/load `.rustixproj` with full scene data (entities, components, transforms, mesh refs)
- ECS entity tree → Hierarchy panel (create, delete, rename, context menu, drag reorder)
- Component editing → Inspector (Transform, DirectionalLight, PointLight, SpotLight, Name, MeshRenderer)
- Real log capture → Console panel (tracing subscriber → ring buffer → color-coded output)
- Asset file listing → Asset Browser (file tree, type icons, colored extensions)
- Window resize handling (swapchain + depth buffer recreation)
- Undo/redo system (Ctrl+Z/Ctrl+Shift+Z) with `UndoStack<SceneCommand>`
- Gizmos — translate handles (X/Y/Z axis arrows with `ray_intersect_triangle`)
- Per-entity mesh rendering with GLB import (tinygltf → mesh registry)
- Procedural mesh presets (cube, sphere, plane, torus, cylinder)
- 3D scene grid overlay with entity position dots
- Offscreen 3D scene rendering → Scene View panel (framebuffer → egui Image)
- Vulkan synchronization: per-frame semaphores, proper fence ordering, resource cleanup (Drop impls)
- Sprite editor with pixel manipulation (set_pixel, draw_line, fill_circle, 9-patch, etc.)
- Waveform visualizer for audio editing
- Bundled fonts (NotoSans, NotoSansMono, NotoEmoji) via `include_bytes!`

### What's needed to reach 100%
| Feature | Priority | What to build |
|---------|----------|---------------|
| **Entity selection** | **HIGH** | Raycast click in scene → highlight in hierarchy + inspector |
| Multiple viewports | Medium | Side-by-side scene + debug views |
| Docking / panel rearrangement | Medium | Drag panels to reorganize layout |
| Layout persistence | Medium | Save/restore panel positions per project |
| Material editing | Medium | PBR parameter sliders in Inspector |
| Scene view gizmos (rotate/scale) | Medium | Rotate and scale handles |
| Camera preview overlay | Low | PiP from other cameras |
| Profiler panel | Low | Frame time graph, memory usage |

---

## 11. Stub Crates — 0% each

These crates compile but their `src/lib.rs` is a single blank line. Zero implementation.

### What's needed to start
| Crate | Priority | First step |
|-------|----------|------------|
| `physics` | High | Add Rapier3D dep, create `PhysicsWorld` + ECS sync |
| `animation` | Medium | Skeleton loading, skinning shader, animation clip player |
| `networking` | Low | QUIC transport via `quinn`, ECS replication |
| `terrain` | Low | Chunk-based heightmap, LOD selection, GPU upload |
| `world` | Low | Spatial ECS partitioning, streaming, persistence |
| `editor` | Low | Gizmo math, scene graph editing helpers, overlay rendering |

---

## Priority Recommendations for Next Work

### High — Phase 1 blockers
1. **PBR rendering pipeline** — Without this, there's no "real" rendering
2. **Entity selection** (raycast) — Core editor interaction missing
3. **glTF full import + texture loading** — Needed to get user assets in-engine
4. **Physics integration** — Rapier3D is the biggest remaining ECS feature

### Medium — Phase 1 scope
5. **Input action system** — Gamepad + configurable key bindings
6. **Full audio ECS integration** — Spatial audio in 3D scene
7. **Hot-reload for scripts + shaders** — Iteration speed
8. **Async asset loading** — Non-blocking IO
9. **Rotate/scale gizmos** — Full transform editing

### Low — Post-Phase 1
10. Animation system, terrain, world streaming, networking, AI ECS integration
11. UI framework text rendering, editor docking, profiler panels
