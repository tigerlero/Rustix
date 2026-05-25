# Rustix Engine

A modern, high-performance game engine built in Rust.

## Quick Start

```bash
# Run the engine (debug mode)
cargo run

# Run with optimizations
cargo run --release

# Check compilation
cargo check

# Run tests
cargo test

# Run benchmarks
cargo bench
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
├── engine/          # Engine facade (App, Plugin, Schedule)
├── crates/
│   ├── core/        # ECS, job system, math, memory, config
│   ├── platform/    # Window, input (winit + Wayland/X11)
│   ├── render/      # Vulkan renderer (ash 0.38)
│   ├── asset/       # Asset server (handles, registry)
│   ├── physics/     # Rapier 3D (stub)
│   ├── audio/       # Audio system (stub)
│   ├── animation/   # Skeletal animation (stub)
│   ├── networking/  # MMO networking (stub)
│   ├── scripting/   # WASM scripting (stub)
│   ├── ui/          # Game UI (stub)
│   ├── ai/          # AI / navigation (stub)
│   ├── terrain/     # Terrain system (stub)
│   ├── world/       # World streaming (stub)
│   └── editor/      # Editor support (stub)
├── apps/
│   └── runtime/     # Game runtime binary
├── docs/            # Architecture, roadmap, features
└── shaders/         # Vulkan shader source files
```

## Current Capabilities

- **Vulkan 1.3** renderer with dynamic rendering (no RenderPass)
- **GPU memory** via gpu-allocator (device-local + host-visible)
- **Procedural geometry**: cubes, toruses, UV spheres, icospheres
- **Indexed rendering** with vertex buffers + push constants
- **Uniform buffers** with descriptor sets
- **Depth testing** (D32_SFLOAT) with back-face culling
- **Multi-object scenes** with orbiting camera
- **naga** runtime GLSL → SPIR-V compilation
- **Asset system** with typed handles + path mapping
- **Input system** (keyboard + mouse via winit)
- **Job system** (rayon work-stealing, 12 threads)
- **Frame allocator** (bump allocator, O(1) reset)

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Design Philosophy](docs/DESIGN_PHILOSOPHY.md)
- [Feature Breakdown](docs/FEATURES.md)
- [Development Roadmap](docs/ROADMAP.md)
- [Implementation Status](docs/IMPLEMENTATION_STATUS.md)
- [Subsystem Reference](docs/SUBSYSTEMS_REFERENCE.md)

## License

MIT — see [LICENSE](LICENSE)
