# Rustix Engine

A modern, high-performance game engine built in Rust, targeting Linux (Wayland/X11) with Vulkan.

## Quick Start

```bash
# Build everything
cargo build

# Run the editor / runtime
cargo run -p rustix-runtime

# Run with optimizations
cargo run -p rustix-runtime --release

# Check compilation
cargo check

# Run all tests
cargo test

# Run core crate tests only
cargo test -p rustix-core --lib
```

## Requirements

- **Rust** 1.95+ stable
- **Vulkan** 1.3 capable GPU (NVIDIA recommended)
- **Linux** (Pop!_OS / Ubuntu with Wayland or X11)
- Vulkan drivers installed (`nvidia-driver-*` or `mesa-vulkan-drivers`)

Optional for validation:
- `vulkan-validationlayers` (debug output)
- `vulkan-sdk` / `glslang-tools` (shader compilation)

## Architecture

```
rustix/
├── crates/
│   ├── core/        # ECS, job system, math, memory, config, diagnostics (24 modules)
│   ├── platform/    # Window, input (winit + Wayland/X11)
│   ├── render/      # Vulkan renderer (ash 0.38)
│   ├── asset/       # Asset server (typed handles, path mapping, registry)
│   ├── audio/       # Audio system (partial)
│   ├── ai/          # AI navigation (partial)
│   ├── physics/     # Rapier 3D (stub)
│   ├── animation/   # Skeletal animation (stub)
│   ├── networking/  # MMO networking (stub)
│   ├── scripting/   # WASM scripting (stub)
│   ├── ui/          # Game UI (stub)
│   ├── terrain/     # Terrain system (stub)
│   ├── world/       # World streaming (stub)
│   └── editor/      # Editor support (stub)
├── apps/
│   └── runtime/     # Editor + runtime binary (egui, scene editing, project management)
├── docs/            # Architecture, roadmap, features, design philosophy
└── shaders/         # Vulkan GLSL shader source files
```

## Current Capabilities

### Rendering
- **Vulkan 1.3** renderer with dynamic rendering (no RenderPass)
- **GPU memory** via gpu-allocator (device-local + host-visible)
- **Staging buffer ring allocator** with fence tracking for async CPU→GPU uploads
- **Procedural geometry**: cubes, toruses, UV spheres, icospheres
- **Indexed rendering** with vertex buffers + push constants
- **Uniform buffers** with descriptor sets
- **Depth testing** (D32_SFLOAT) with back-face culling
- **Multi-object scenes** with orbiting camera
- **naga** runtime GLSL → SPIR-V compilation

### ECS & Core
- **hecs** archetypal ECS with query filters (`With`, `Without`)
- **Component registry** — type-erased storage via `TypeId` + vtable (size, align, clone, drop, default)
- **Dynamic bundles** — runtime component addition via `ComponentRegistry::insert_bundle`
- **Command buffers** — deferred world mutation (`Spawn`, `Despawn`, `InsertBundle`, `Remove`, etc.)
- **Change detection** — dirty flags per component per tick (`flag<T>()`, `is_changed<T>()`, `changed_entities::<T>()`)
- **Component groups** — named sets for cache-optimal archetype pre-warming
- **Multi-world support** — `WorldRegistry` for game / editor / preview worlds with entity mapping
- **Transform hierarchy** — BFS world matrix computation with cycle detection and topological ordering

### Jobs & Memory
- **Job system** — `rayon` work-stealing thread pool with configurable thread count and work queue depth
- **Task graph** — DAG dependency system with Kahn's topological sort and parallel frontier execution
- **Priority task system** — dedicated threads with high / medium / low priority queues
- **Frame allocator** — atomic bump allocator, O(1) reset per frame
- **Pool allocator** — fixed-size object reuse with chunk-based growth
- **Thread-local arenas** — per-thread `FrameAllocator` for zero-contention allocation
- **Cache-line aligned** allocations (64-byte alignment)

### Configuration & Diagnostics
- **TOML-based engine config** — layered: default → project → user → CLI overrides
- **Runtime config reload** — polling file watcher with callback-based reload (`ConfigWatcher`)
- **Hot-key toggles** — `DevToggles` (dev mode, debug render, profiling) with `F1/F2/F3` defaults
- **Structured logging** via `tracing` with console + JSON file output
- **JSON file logging** — `JsonFileLayer` writes JSON Lines per event with span context and field escaping
- **Log capture** — circular in-memory buffer for runtime log inspection
- **Per-crate log level filtering**

### Platform
- **Wayland native** support (primary target for Pop!_OS)
- **X11 fallback** (xcb backend)
- **Fullscreen exclusive** — picks best video mode; falls back to borderless
- **Borderless fullscreen windowed** — fills screen without video mode change
- **Input state** — current + previous frame for "just pressed" edge detection
- **File dialogs** — native picker via `rfd`

### Assets & Editor
- **Asset system** — typed handles + path mapping + hot-reload registry
- **GLTF loading** — mesh + material import
- **Editor UI** — egui-based with scene graph, inspector, viewport, project manager
- **Undo/redo** — command-based history system

## Documentation

- [Feature Breakdown](docs/FEATURES.md) — full checklist of implemented and planned features
- [Architecture](docs/ARCHITECTURE.md)
- [Design Philosophy](docs/DESIGN_PHILOSOPHY.md)
- [Development Roadmap](docs/ROADMAP.md)
- [Implementation Status](docs/IMPLEMENTATION_STATUS.md)
- [Subsystem Reference](docs/SUBSYSTEMS_REFERENCE.md)

## License

MIT — see [LICENSE](LICENSE)
