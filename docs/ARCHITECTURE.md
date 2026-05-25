# Rustix Engine Architecture

**Status:** Phase 0 complete (2026-05-24). Core, platform, renderer, and engine facade implemented. 13 subsystem crates stubbed.

See `docs/IMPLEMENTATION_STATUS.md` for current build state and `docs/ROADMAP.md` for next steps.

## 1. High-Level Architectural Overview

```
                     ┌─────────────────────────────────────────────────────────┐
                     │                    GAME / EDITOR                        │
                     │         (apps/runtime, apps/editor)                     │
                     └────────────────────┬────────────────────────────────────┘
                                          │
                     ┌────────────────────▼────────────────────────────────────┐
                     │                   ENGINE FACADE                         │
                     │             (engine/ — public API)                      │
                     │    Re-exports, system registration, lifecycle           │
                     └────────────────────┬────────────────────────────────────┘
                                          │
    ┌─────────────────────────────────────┼─────────────────────────────────────┐
    │                  LAYER 1: CORE      │      LAYER 2: PLATFORM             │
    │  ┌──────────┐ ┌──────────┐         │  ┌──────────┐ ┌──────────┐         │
    │  │   ECS    │ │Job System│         │  │Window/Way│ │  Input   │         │
    │  │ (hecs+)  │ │ (rayon)  │         │  │(winit+x11)│ │(evdev,   │         │
    │  └──────────┘ └──────────┘         │  └──────────┘ │  libei)  │         │
    │  ┌──────────┐ ┌──────────┐         │  └──────────┘ └──────────┘         │
    │  │  Memory  │ │  Math    │         │  ┌──────────┐ ┌──────────┐         │
    │  │ (alloc)  │ │ (glam)   │         │  │  Time    │ │  OS      │         │
    │  └──────────┘ └──────────┘         │  │ (cross-  │ │ (mman,   │         │
    │  ┌──────────┐                      │  │  platform)│ │  syscall)│         │
    │  │  Config  │                      │  └──────────┘ └──────────┘         │
    │  │ (toml)   │                      │                                     │
    │  └──────────┘                      └─────────────────────────────────────┘
    │                                                                          │
    │                  LAYER 3: GRAPHICS / RENDERER                            │
    │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐      │
    │  │ Vulkan   │ │ Pipeline │ │ Descriptor│ │  Frame   │ │ Shader   │      │
    │  │ Device   │ │  Cache   │ │  Manager  │ │ Resource │ │ Compiler │      │
    │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘      │
    │  ┌──────────┐ ┌──────────┐ ┌──────────┐                                 │
    │  │ GPU      │ │  Mesh/   │ │  Async   │                                 │
    │  │ Memory   │ │  Mat     │ │  Upload  │                                 │
    │  └──────────┘ └──────────┘ └──────────┘                                 │
    └──────────────────────────────────────────────────────────────────────────┘
    ┌──────────────────────────────────────────────────────────────────────────┐
    │                  LAYER 4: CONTENT / ASSETS                               │
    │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐                    │
    │  │  Asset   │ │  Stream  │ │ Hot-Reload│ │  Import  │                    │
    │  │  Registry│ │  Engine  │ │  Watcher  │ │  Pipeline│                    │
    │  └──────────┘ └──────────┘ └──────────┘ └──────────┘                    │
    │  ┌──────────┐ ┌──────────┐                                              │
    │  │  World   │ │  Terrain │                                              │
    │  │  Stream  │ │  Chunks  │                                              │
    │  └──────────┘ └──────────┘                                              │
    └──────────────────────────────────────────────────────────────────────────┘
    ┌──────────────────────────────────────────────────────────────────────────┐
    │                LAYER 5: GAMEPLAY / SERVICES                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐      │
     │  │ Physics  │ │  Audio   │ │Animation │ │  AI/Nav  │ │Networking│      │
     │  │ (Rapier) │ │(cpal+)   │ │ (blend)  │ │(NavMesh) │ │ (tokio)  │      │
     │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘      │
     │  ┌──────────┐ ┌──────────┐                                              │
     │  │ Scripting│ │   UI     │                                              │
     │  │ (Rhai)   │ │ (egui)   │                                              │
     │  └──────────┘ └──────────┘                                              │
    └──────────────────────────────────────────────────────────────────────────┘
    ┌──────────────────────────────────────────────────────────────────────────┐
    │               LAYER 6: DIAGNOSTICS / PROFILING                          │
    │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐                    │
    │  │ tracing  │ │  Tracy   │ │ RenderDoc │ │ Metrics  │                    │
    │  │ (spans)  │ │ (profiler)│ │ (GPU)    │ │ (counters)│                   │
    │  └──────────┘ └──────────┘ └──────────┘ └──────────┘                    │
    └──────────────────────────────────────────────────────────────────────────┘
```

### 1.1 Layer Architecture (Why Layered?)

The engine is a **layered modular monolith** — not a microservices architecture. Each layer has clear dependencies (only downward). This gives:

- **Compile-time separation** — changing the renderer doesn't recompile gameplay code
- **Testability** — each layer can be unit-tested in isolation
- **Replaceability** — swap audio backends, physics engines, or renderer without touching game code
- **Clear dependency graph** — no circular deps between crates

Dependency direction: `Gameplay → Content → Graphics → Core + Platform`

All gameplay subsystems depend only on Content and Core, never directly on Graphics. The renderer is behind an abstraction barrier.

### 1.2 Crate Architecture (Workspace Layout)

```
rustix/                          # Workspace root
├── Cargo.toml                   # [workspace] with members
├── engine/                      # Facade crate: re-exports, App builder
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs               # Engine struct, AppBuilder, run()
│       ├── plugin.rs            # Plugin trait
│       ├── schedule.rs          # System scheduling stages
│       └── builder.rs           # App builder pattern
├── crates/
│   ├── core/                    # ECS, job system, memory, math, config
│   ├── platform/                # Windowing, input, OS abstractions
│   ├── render/                  # Vulkan renderer (ash)
│   ├── asset/                   # Asset pipeline, streaming, hot-reload
│   ├── physics/                 # Rapier integration
│   ├── audio/                   # Audio system (cpal/rodio)
│   ├── animation/               # Skeletal animation, blend trees
│   ├── networking/              # MMO networking (tokio + QUIC)
│   ├── scripting/               # WASM scripting host
│   ├── ui/                      # Immediate mode game UI
│   ├── ai/                      # Navigation meshes, behavior trees
│   ├── terrain/                 # Chunk-based terrain system
│   ├── world/                   # World streaming, LOD, chunks
│   └── editor/                  # Editor support crate (IMGUI overlay)
├── apps/
│   ├── runtime/                 # Game runtime binary
│   └── editor/                  # Editor binary (future)
├── shaders/
│   ├── vulkan/                  # GLSL source files
│   └── compiled/                # SPIR-V binaries (git-lfs)
├── assets/                      # Development assets
├── tools/
│   ├── shaderc/                 # Shader build tool (glsl → SPIR-V)
│   └── asset-pipeline/          # Asset preprocessing/import
├── benches/                     # Criterion benchmarks
├── docs/                        # Architecture, design, guides
└── scripts/                     # Build/CI scripts
```

### 1.3 Why Each Crate Exists

| Crate | Responsibility | Key Deps |
|-------|----------------|----------|
| `core` | ECS, job system, math, memory allocators, config, logging init | `hecs`, `glam`, `rayon`, `parking_lot`, `tracing`, `serde` |
| `platform` | Window creation, input (keyboard/mouse/gamepad), OS abstractions | `winit`, `xkbcommon`, `evdev` |
| `render` | Vulkan device, swapchain, pipelines, shaders, GPU memory, frame graph | `ash`, `gpu-allocator`, `spirv-reflect` |
| `asset` | Asset loading, format decoding, hot-reload, streaming, GPU upload | `serde`, `tokio`, `notify` |
| `physics` | Physics world, colliders, rigid bodies, queries | `rapier3d` |
| `audio` | Audio playback, streaming, 3D spatialization | `cpal`, `symphonium` |
| `animation` | Skeleton, skinning, animation clips, blend trees | `core` (math) |
| `networking` | Transport (QUIC/TCP), connection management, replication, RPC | `tokio`, `quinn` |
| `scripting` | Rhai scripting runtime, ECS integration, FFI bridge | `rhai` |
| `ui` | Immediate mode UI for game HUD | `egui` |
| `ai` | Navigation mesh, pathfinding, behavior trees | `core` (ECS) |
| `terrain` | Heightmap, chunk generation, LOD, voxel helpers | `render`, `core` |
| `world` | Chunk management, streaming regions, persistent state | `core`, `asset`, `terrain` |
| `editor` | Editor overlays, gizmos, scene graph editing | `render`, `ui`, `core` |

### 1.4 The Engine Facade (engine/ crate)

The `engine` crate is the **public API** users interact with. It re-exports key types and provides `AppBuilder`:

```rust
// engine/src/lib.rs

pub struct App {
    plugins: Vec<Box<dyn Plugin>>,
    schedule: Schedule,
    world: World,
}

impl App {
    pub fn new() -> Self { ... }
    pub fn add_plugin<P: Plugin>(mut self, plugin: P) -> Self { ... }
    pub fn run(self) -> ! { ... }
}
```

The facade pattern means:
- Internal crate changes don't break user code (re-exports are stable)
- `cargo build` only recompiles changed crates
- Users can opt-in to subsystems (plugin model)

---

## 2. Core Layer Design

### 2.1 ECS Architecture

**Why `hecs` over `bevy_ecs` or custom?**
- `hecs` is lightweight (~1/10th the deps of bevy_ecs), fast to compile
- No required rendering or scheduling logic baked in
- We need full control over system scheduling for our job system
- `bevy_ecs` pulls in the entire bevy ecosystem and adds ~50+ deps
- Custom ECS is not justified — `hecs` provides archetypal ECS with excellent cache behavior

**Architecture:**
- Archetypal ECS (entities grouped by component type → contiguous arrays → cache-friendly)
- No trait objects where avoidable — systems operate on component slices directly
- System scheduling is **our code** (not bevy's), giving full control over parallelism

```rust
// crates/core/src/ecs/mod.rs

pub use hecs::{
    Entity, World as EcsWorld, View, ViewMut,
    Query, QueryItem, QueryOneOf, With, Without,
    DynamicBundle, EntityBuilder,
};
```

**Why not custom ECS?**
- `hecs` is proven in production (used by `rusty_v8`, `kajiya`, etc.)
- Writing a competitive archetypal ECS is a multi-month effort
- We can extend `hecs` with a thin layer for system ordering and parallel scheduling

### 2.2 System Scheduling

Systems are organized into **stages** with **parallel groups** within each stage:

```
Stage Order:  FixedUpdate → PreUpdate → Update → PostUpdate → PreRender → Render
```

Within each stage, systems that read the same components are ordered;
systems that write disjoint components **can run in parallel** via rayon.

```rust
// Scheduled via a custom scheduler, not bevy's:

pub struct Schedule {
    stages: Vec<Stage>,
}

struct Stage {
    label: StageLabel,
    systems: Vec<SystemNode>,   // graph of systems within this stage
    // Systems grouped by parallelism: systems in same group run serially,
    // groups run in parallel via rayon
    parallel_groups: Vec<Vec<SystemNode>>,
}
```

**Threading model:**
- Each system takes `&World` (immutable ECS access) or `&mut World` (exclusive)
- Immutable systems run in parallel via rayon's `par_iter` on groups
- Mutable systems are pipelined (they run sequentially but overlap with render work)
- Render is pipelined with update: update frame N while GPU processes frame N-1 (triple buffering)

### 2.3 Job System

**Why rayon?**
- Proven, battle-tested work-stealing scheduler
- Excellent on AMD high-core-count CPUs (work stealing handles imbalance)
- `par_iter` provides automatic load balancing
- We wrap it in a thin `TaskGraph` for explicit dependency tracking when needed

```rust
// crates/core/src/job/mod.rs

pub struct JobSystem {
    pool: rayon::ThreadPool,
}

impl JobSystem {
    /// Dispatch parallel jobs with optional dependency tracking
    pub fn dispatch<T: Send + 'static>(
        &self,
        jobs: Vec<Job<T>>,
        deps: &[JobDependency],
    ) -> JobHandle<T> { ... }

    /// Fork-join pattern: run tasks in parallel, wait for all
    pub fn parallel_for<T, F>(&self, items: &[T], f: F)
    where
        T: Send + Sync,
        F: Fn(&T) + Send + Sync,
    { ... }

    /// Number of worker threads (typically num_cpus::get())
    pub fn thread_count(&self) -> usize { ... }
}
```

**AMD Optimization:**
- Default: `num_cpus::get()` threads (all logical cores)
- For Zen architectures: prefer 1 thread per physical core for compute-heavy workloads
- Configurable via `config.toml`:
  ```toml
  [jobs]
  thread_count = "physical"  # or integer
  affinity = true            # pin threads to cores (Linux only via libpthread)
  ```

### 2.4 Memory System

**Design principles:**
- Frame allocators for per-frame temporary data (zero-cost free)
- Pool allocators for fixed-size objects (components, entities)
- Custom arenas for GPU upload staging buffers
- Minimize heap fragmentation via early-size-class allocation

```rust
// crates/core/src/memory/mod.rs

pub struct FrameAllocator {
    slab: UnsafeCell<Vec<u8>>,
    cursor: AtomicUsize,
}

impl FrameAllocator {
    pub fn allocate(&self, layout: Layout) -> *mut u8 { ... }
}

// Reset at end of frame (O(1) — just reset cursor)
// Used for: transient per-frame allocations (render lists, etc.)
```

**Why frame allocators?**
- Eliminates allocation pressure during frame (zero syscalls after initial reserve)
- Perfect for ECS command buffers, render command lists, per-frame UI data
- 10-100x faster than general-purpose allocator for these use cases

**Cache-line alignment:**
- All hot data structures padded to 64 bytes (avoid false sharing on AMD)
- Hot structs: `#[repr(C, align(64))]`

### 2.5 Math Library

Use `glam` — it's SIMD-accelerated (SSE2/AVX on x86-64), const-compatible, and the de-facto standard Rust game math library.

```rust
// Re-exported from core
pub use glam::*;  // Vec3, Mat4, Quat, etc.
```

No custom math — `glam` is already optimized with explicit SIMD and is used by the entire Rust gamedev ecosystem.

---

## 3. Platform Layer

### 3.1 Windowing (winit)

**Why winit?**
- Mature Wayland + X11 support (crucial for Pop!_OS)
- Active maintenance by Rust gamedev community
- Handles raw window handle (HasRawWindowHandle) for Vulkan surface creation

```rust
// crates/platform/src/window.rs

pub struct Window {
    inner: winit::window::Window,
    event_loop: Option<winit::event_loop::EventLoop<()>>,
}

impl Window {
    pub fn new(config: &WindowConfig) -> Result<Self, PlatformError> {
        // Wayland preferred on Pop!_OS, fallback to X11
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title(&config.title)
            .with_inner_size(LogicalSize::new(config.width, config.height))
            .build(&event_loop)?;
        Ok(Self { inner: window, event_loop: Some(event_loop) })
    }

    pub fn raw_handle(&self) -> RawWindowHandle {
        self.inner.raw_window_handle()
    }

    pub fn event_loop(&mut self) -> EventLoop<()> {
        self.event_loop.take().unwrap()
    }
}
```

**Wayland vs X11 strategy:**
- Default to Wayland (Pop!_OS default)
- Detect via `WAYLAND_DISPLAY` env var
- Fallback to X11 if Wayland unavailable
- `winit` handles this transparently with `x11` and `wayland` features

### 3.2 Input System

**Why not winit events for input?**
- Winit events are tied to the window event loop (run at display refresh rate)
- We need raw input sampling at a fixed rate, decoupled from rendering
- Direct evdev access (on Linux) provides lower latency

```rust
// crates/platform/src/input.rs

pub struct InputManager {
    keyboard: KeyboardState,
    mouse: MouseState,
    gamepad: GamepadState,
    // Raw evdev fds for low-latency input
    evdev_handles: Vec<EvdevHandle>,
}

impl InputManager {
    /// Called once per fixed update tick (not per frame)
    pub fn poll(&mut self) {
        // Read evdev events if available
        // Else fallback to accumulated winit events
    }

    pub fn keyboard(&self) -> &KeyboardState { &self.keyboard }
    pub fn mouse(&self) -> &MouseState { &self.mouse }
}
```

**Input polling strategy:**
1. **Primary path**: evdev (Linux raw input) — direct from kernel, <1ms latency
2. **Fallback**: winit window events (works on all platforms)
3. **Gamepad**: `gilrs` crate for gamepad input

**Threading:**
- Input poll runs on dedicated thread (or in fixed update)
- Input state is `Send + Sync` behind RwLock for cross-thread access
- Accumulated events consumed each tick

---

## 4. Graphics / Renderer Layer

### 4.1 Vulkan Architecture (ash)

**Why `ash` over `vulkano`?**
- Zero overhead — raw Vulkan performance, no safety abstraction tax
- Full explicit control over device, memory, and pipelines
- `vulkano` adds overhead that prevents fine-grained optimization
- `ash` is what the Rust game engines that push Vulkan limits use (kajiya, etc.)

**Device Selection (NVIDIA optimization):**
```rust
// crates/render/src/device.rs

pub struct GpuDevice {
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    queue_families: QueueFamilies,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
}

impl GpuDevice {
    pub fn new(config: &RenderConfig) -> Result<Self, RenderError> {
        // 1. Create instance with validation layers in debug
        // 2. Enumerate physical devices
        // 3. Score devices: prefer NVIDIA discrete GPU
        //    - NVIDIA = preferred (most optimized path)
        //    - AMD = score slightly lower but still good
        //    - Intel integrated = lowest priority
        // 4. Select queue families:
        //    - Graphics queue (can also support compute)
        //    - Separate transfer queue (async upload)
        //    - Optional compute-only queue for async compute
        // 5. Enable device features:
        //    - VK_KHR_swapchain (required)
        //    - VK_KHR_dynamic_rendering (no render pass objects)
        //    - VK_EXT_descriptor_indexing (bindless descriptors)
        //    - VK_KHR_synchronization2 (modern sync)
        //    - VK_KHR_timeline_semaphore (GPU-CPU sync)
    }
}
```

**NVIDIA-specific tuning:**
- Prefer `VK_PRESENT_MODE_MAILBOX_KHR` (fastest, most responsive)
- Use `VK_KHR_dynamic_rendering` (eliminates render pass creation cost)
- Enable `VK_EXT_descriptor_indexing` for bindless — NVIDIA loves this
- Pipeline cache (`VK_KHR_pipeline_cache`) stored to disk for fast subsequent runs
- Frame pacing: use timeline semaphores for precise CPU-GPU sync
- Avoid `VK_PRESENT_MODE_FIFO_KHR` (turns on vsync, adds latency)

### 4.2 Frame Graph

A **Render Graph** (similar to The Forge, Granite, etc.):

```
FrameGraph {
    resources: Vec<ResourceNode>,
    passes: Vec<RenderPass>,
    edges: Vec<Edge>,  // resource dependencies
}
```

- Declarative: passes declare input/output resources
- Automatic barrier insertion (synchronization)
- Automatic memory management (transient resources reused)
- Parallel pass scheduling where possible (async compute overlap)

```rust
// crates/render/src/frame_graph/mod.rs

pub struct FrameGraphBuilder {
    resources: Vec<ResourceDesc>,
    passes: Vec<PassDesc>,
}

impl FrameGraphBuilder {
    pub fn add_pass(&mut self, pass: PassDesc) -> &mut Self { ... }
    pub fn add_resource(&mut self, resource: ResourceDesc) -> ResourceId { ... }
    pub fn build(self) -> FrameGraph { ... }

    pub fn compile(&mut self, device: &GpuDevice) -> CompiledFrameGraph {
        // 1. Dependency analysis
        // 2. Barrier insertion
        // 3. Memory aliasing for transient resources
        // 4. Render pass merging where possible
        // 5. Return compiled graph with concrete command buffers
    }
}
```

### 4.3 GPU Memory Hierarchy

```
GPU Memory Manager
├── Device Local (VRAM)          — HDR textures, swapchain images, depth buffers
├── Host Visible + Coherent      — Staging buffers, uniform buffers (streaming)
├── Host Visible + Cached        — Readback (profiling, debug)
└── Device Local + Host Visible  — BAR memory (NV only)
```

**Allocation strategy (NVIDIA):**
- Use `vk_mem` (`gpu-allocator` crate) for robust memory management
- Pre-allocate large chunks (256MB blocks) and sub-allocate
- Separate allocator categories:
  - `RenderTarget`: device-local, fast to free (per-frame transient)
  - `Texture`: device-local, long-lived
  - `Buffer_Upload`: host-visible, coherent, mapped persistently
  - `Buffer_Device`: device-local for GPU data (vertex, index, indirect)

### 4.4 Descriptor Management

**Bindless model (NVIDIA-optimized):**

```rust
// crates/render/src/descriptors.rs

pub struct DescriptorManager {
    // Single large descriptor set per type:
    // VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER — up to 4096 textures
    // VK_DESCRIPTOR_TYPE_STORAGE_BUFFER — up to 4096 buffers
    // VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER — up to 4096 UBOs
    sets: HashMap<DescriptorType, DescriptorSet>,
    // Free-list allocator per set type
    allocators: HashMap<DescriptorType, FreeListAllocator>,
}
```

- All materials share the same global descriptor heap (bindless)
- No per-material descriptor sets — just indices into the global heap
- Reduces driver overhead: no descriptor set binding between draw calls
- NVIDIA drivers handle bindless extremely well (hardware-accelerated pointer fetching)

### 4.5 Shader Compilation Pipeline

```
Source GLSL (shaders/vulkan/) 
    → glslangValidator (SPIR-V)
        → runtime: naga (optional validation) 
            → compiled SPIR-V cached on disk

OR in-editor:
    Source GLSL 
        → shaderc-rs (compile at startup / hot-reload)
            → SPIR-V binary in memory
```

**Strategy:**
- **Pre-compiled** path (release): compile to SPIR-V offline via build script
- **Runtime** path (debug): compile on-the-fly for hot-reload
- Cache compiled SPIR-V to disk (`~/.cache/rustix/shaders/`)
- Use `VK_KHR_pipeline_cache` for pipeline state caching (huge speedup on NVIDIA)

---

## 5. Content / Asset Layer

### 5.1 Asset Pipeline Architecture

```
                    ┌─────────────┐
                    │ Source Asset │ (.glb, .png, .wav, .glsl)
                    └──────┬──────┘
                           ▼
                    ┌─────────────┐
                    │  Importer   │ (per-type: gltf, image, audio, shader)
                    └──────┬──────┘
                           ▼
                    ┌─────────────┐
                    │  Converted  │ (engine-native format: .rxmesh, .rxtex)
                    └──────┬──────┘
                           ▼
                    ┌─────────────┐
                    │   Asset     │ (hot-reload watcher + reference counting)
                    │  Registry   │
                    └──────┬──────┘
                           ▼
                    ┌─────────────┐
                    │   Runtime   │ (GPU upload, audio decode, physics shapes)
                    │   Handle    │
                    └─────────────┘
```

### 5.2 Asset Identifiers & Handles

```rust
// crates/asset/src/handle.rs

pub struct Handle<T: Asset> {
    id: u64,           // 64-bit ID (generation + index for safety)
    marker: PhantomData<T>,
}

pub trait Asset: Send + Sync + 'static {
    type Loader: AssetLoader<Self>;
    fn asset_type() -> AssetType;
}
```

**Why handles, not Rc/Arc?**
- Handle is 8 bytes (small, copyable, cache-friendly)
- Indirection through registry avoids refcount atomic ops
- Enables streaming (unload/reload without invalidating references everywhere)
- The registry vends `Access<Asset>` (RwLock-protected) on demand

### 5.3 Hot-Reload

```rust
// crates/asset/src/hot_reload.rs

pub struct HotReloadWatcher {
    watcher: notify::RecommendedWatcher,
    // Thread-safe queue of changed files
    changed: crossbeam::channel::Receiver<PathBuf>,
}
```

- Uses `notify` crate for filesystem events
- Watches `assets/` directory recursively
- On change: invalidate cached asset, signal systems to re-import
- Shaders trigger re-compilation → pipeline cache rebuild
- Textures trigger re-upload to GPU (with staging buffer)
- Only in debug builds (disabled in release for perf)

### 5.4 Streaming Architecture

```rust
// crates/asset/src/stream.rs

pub struct StreamEngine {
    // Background tokio runtime for IO
    runtime: tokio::runtime::Runtime,
    // Priority queue of load requests
    queue: PriorityQueue<LoadRequest>,
    // Active streaming loads
    in_flight: Vec<StreamTask>,
}
```

**Streaming strategy:**
- **Priority-based**: near player → high priority, far → low priority
- **Background IO**: tokio thread pool reads from disk
- **Decode on worker threads**: asset decoding (image, mesh, audio) on rayon thread pool
- **GPU upload thread**: dedicated transfer queue submission
- **Tiered cache**: hot (VRAM) → warm (system RAM) → cold (disk)

---

## 6. Game Loop Design

### 6.1 Fixed-Update + Variable Render

```
Frame N:
    ┌─────────────────────────────────────────────────────┐
    │ INPUT POLL (evdev/winit)                            │
    └──────────────────────┬──────────────────────────────┘
                           ▼
    ┌─────────────────────────────────────────────────────┐
    │ FIXED UPDATE (120 Hz tick)                          │
    │ While accumulator >= fixed_dt:                      │
    │   → Physics (rapier)                                │
    │   → Animation (blend tree update)                   │
    │   → AI (behavior tree tick)                         │
    │   → Networking (send/receive state)                 │
    │   → Scripting (WASM tick)                           │
    │   → ECS systems (movement, gameplay)                │
    │   accumulator -= fixed_dt                           │
    └──────────────────────┬──────────────────────────────┘
                           ▼
    ┌─────────────────────────────────────────────────────┐
    │ VARIABLE UPDATE (every frame)                       │
    │ → Interpolation (visual smoothing)                  │
    │ → Camera update                                     │
    │ → World streaming (load/unload regions)             │
    │ → Audio listener update                             │
    └──────────────────────┬──────────────────────────────┘
                           ▼
    ┌─────────────────────────────────────────────────────┐
    │ RENDER (submitted to GPU, returns immediately)       │
    │ → Frustum culling (compute shader)                   │
    │ → Indirect draw command generation                   │
    │ → Frame graph compilation & submission               │
    │ → Present                                            │
    └──────────────────────────────────────────────────────┘
```

### 6.2 Triple Buffering

```
Frame N-1: GPU rendering (pipeline already submitted)
Frame N:   CPU update + render submission
Frame N+1: CPU update + render submission (while GPU on N)
```

- CPU is never blocked waiting for GPU (except when frame queue is full)
- Timeline semaphores for precise sync
- Frame resource ring buffer (per-frame data, 3 copies)

### 6.3 Frame Pacing (NVIDIA Optimization)

- `VK_PRESENT_MODE_MAILBOX_KHR`: tearing is acceptable (lowest latency)
- Optional: `VK_PRESENT_MODE_FIFO_RELAXED_KHR` if tearing is undesirable
- Frame time tracking: exponential moving average for adaptive quality
- If frame budget exceeded: drop quality (LOD, shadow resolution, post-process)
- If headroom: increase quality (MSAA samples, shadow cascades)

---

## 7. World Streaming & Open World

### 7.1 Chunk System

```
World
├── Region (1024x1024 units, persistent save unit)
│   ├── Chunk (128x128 units, streaming unit)
│   │   ├── Entity layer (ECS entities in this chunk)
│   │   ├── Terrain layer (heightmap, materials)
│   │   └── Navigation layer (local navmesh)
│   └── ...
└── ...
```

**Streaming trigger:**
- Distance from player: load radius (visible), keep radius (persistent)
- Async load: regions beyond keep radius get serialized to disk
- LOD: distant chunks use lower-detail render proxies

### 7.2 Persistent World State

- Regions saved as `.rxregion` files (custom binary format via serde + bincode)
- Entity serialization: component-based (only components with `Serialize`)
- World saves in background (tokio write tasks) — never block gameplay

---

## 8. Networking Layer

### 8.1 Architecture (Authoritative Server)

```
Game Server (authoritative)
    │
    ├── TCP (QUIC) — reliable: RPC, state sync, chat
    │
    └── UDP (QUIC) — unreliable: position updates, input, snapshots
```

**Why QUIC over raw UDP?**
- QUIC gives us TLS 1.3 encryption (mandatory for modern security)
- Stream multiplexing (separate reliable/unreliable streams)
- Connection migration (client IP changes don't disconnect)
- Built-in congestion control
- `quinn` crate provides Rust QUIC implementation

**Server architecture:**
- Multi-threaded: 1 IO thread + N worker threads (simulation)
- Deterministic tick rate (30-60 Hz server tick)
- Client sends inputs → server simulates → broadcasts state
- Snapshot compression: delta encoding + LZ4

### 8.2 Replication

```rust
// crates/networking/src/replication.rs

pub struct ReplicationManager {
    replicated_entities: Vec<Entity>,
    // Per-entity: replication mask (which components to sync)
    replication_masks: HashMap<Entity, ReplicationMask>,
}
```

- Only entities within `replication_distance` are sent to each client
- Changed components tracked via dirtiness bits
- Bandwidth budgeting: prioritize near entities, important state updates

---

## 9. Plugin System

The engine uses a **plugin-based architecture** (inspired by Bevy but simpler):

```rust
// engine/src/plugin.rs

pub trait Plugin: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn build(&self, app: &mut AppBuilder);
    fn on_load(&self, _world: &mut World) {}
    fn on_unload(&self, _world: &mut World) {}
}

// Example:
struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn name(&self) -> &'static str { "physics" }
    fn build(&self, app: &mut AppBuilder) {
        app.add_system(physics_system, Stage::FixedUpdate)
           .add_system(debug_physics_system, Stage::PostUpdate)
           .register_asset::<PhysicsMaterial>();
    }
}
```

**Plugin loading:**
- Compile-time: all plugins are known at link time (no dynamic loading for now)
- Future: WASM-based plugin loading for modding/scripting
- Each plugin registers systems, assets, and components

---

## 10. Diagnostics & Profiling

### 10.1 Logging (tracing)

- Use `tracing` crate for structured logging
- Subscribers:
  - Console (debug): human-readable, colored output
  - File (release): JSON structured logs for analysis
  - Tracy (optional): instrumented tracing spans

```rust
// crates/core/src/diagnostics.rs

pub fn init_logging(config: &LogConfig) {
    let subscriber = tracing_subscriber::fmt()
        .with_target(true)
        .with_thread_ids(true)
        .with_max_level(config.level)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();
}
```

### 10.2 Profiling (Tracy)

- Tracy client integration via `tracy-client` crate
- Frame markers, zone scopes, GPU zones (via timestamp queries)
- Custom counter plots (entity count, draw calls, VRAM used)

```rust
// Instrumentation macro:
#[macro_export]
macro_rules! profile_scope {
    ($name:literal) => {
        let _guard = tracy_client::span!($name);
    };
}
```

### 10.3 GPU Debugging (RenderDoc)

- RenderDoc capture trigger at runtime (e.g. press F12)
- Vulkan instance created with `VK_LAYER_LUNARG_api_dump` in debug mode
- Queue frame markers with `vkSetDebugUtilsObjectNameEXT`

---

## 11. Cross-Platform Abstraction

### 11.1 Current: Linux (Pop!_OS)

Full support for:
- Wayland windowing (winit + wayland feature)
- X11 fallback (winit + x11 feature)
- evdev raw input
- Vulkan via ash

### 11.2 Future: Windows

- winit with Windows backend
- DirectX 12 as secondary renderer (or Vulkan via MoltenVK)
- Win32 raw input
- Audio via WASAPI

---

## 12. Hot-Reload Architecture

```rust
// Engine hot-reload pipeline:

// 1. Asset change detected (notify)
// 2. Asset re-imported (converter thread)
// 3. Asset handle invalidated (registry)
// 4. Systems notified via event
// 5. GPU resources re-uploaded (transfer queue)

// Shader hot-reload:
// 1. GLSL source changed
// 2. Recompile to SPIR-V
// 3. Rebuild VkPipeline (with pipeline cache reuse)
// 4. Next draw call uses new pipeline
```

Only supported in debug builds. Release builds use compiled-in assets.

---

## 13. Build System

### 13.1 Cargo Workspace Configuration

```toml
# Cargo.toml (root)
[workspace]
resolver = "2"
members = [
    "engine",
    "crates/core",
    "crates/platform",
    "crates/render",
    "crates/asset",
    "crates/physics",
    "crates/audio",
    "crates/animation",
    "crates/networking",
    "crates/scripting",
    "crates/ui",
    "crates/ai",
    "crates/terrain",
    "crates/world",
    "crates/editor",
    "apps/runtime",
]
```

### 13.2 Linker Configuration

```toml
# .cargo/config.toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
# Or for lld:
# rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

### 13.3 Build Profile Tuning

```toml
[profile.dev]
opt-level = 1                     # Keeps debug info but compiles faster
debug = 1                         # Line tables only
incremental = true

[profile.release]
opt-level = 3
lto = "fat"                       # Full LTO for maximum optimization
codegen-units = 1                 # Single codegen unit for better inlining
strip = "symbols"                 # Remove debug symbols
debug = false
```

---

## 14. Dependency Graph Summary

```
apps/runtime
  └── engine
        ├── core          (ECS, jobs, math, memory)
        ├── platform      (window, input)
        ├── render        (Vulkan)
        ├── asset         (loading, streaming)
        ├── physics       (Rapier)
        ├── audio         (cpal)
        ├── animation
        ├── networking    (tokio + quinn)
        ├── scripting     (rhai)
        ├── ui            (egui)
        ├── ai
        ├── terrain
        └── world
```

No circular dependencies. Each crate has a single responsibility.
