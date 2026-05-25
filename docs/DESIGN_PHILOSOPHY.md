# Rustix Engine — Design Philosophy

## Core Tenets

### 1. Performance is a Feature, Not an Afterthought

Every design decision is evaluated through the lens of performance. Not micro-optimization of individual operations, but architectural decisions that enable performance at scale.

**What this means in practice:**
- ECS is archetypal (cache-friendly contiguous memory), not sparse set
- Frame allocators for transient data (zero deallocation cost)
- Lock-free or low-lock data structures wherever possible
- Hot-path code is explicit about memory layout (`#[repr(C)]`, cache-line padding)
- Profile-first, optimize-second: never guess about bottlenecks
- `unsafe` is permitted only when:
  1. The safe alternative is measurably slower in the hot path
  2. We can prove correctness via invariants (documented)
  3. A safe abstraction isn't possible without language changes

### 2. Data-Oriented Design (DoD)

Inspired by Mike Acton's CppCon talks and the id Tech engine philosophy. Organize data for efficient processing, not developer convenience.

**Guidelines:**
- Structure of Arrays (SoA) over Array of Structures (AoS) for hot systems
- Component storage in `hecs` is already archetypal SoA
- Systems process components, not entities
- Avoid virtual dispatch in inner loops (use enums or monomorphization)
- Prefer iteration over random access
- Batch operations where possible

### 3. Minimal Unsafe, Safety Where It Counts

Rust's safety guarantees are not negotiable for the majority of code. `unsafe` is isolated to well-defined boundaries.

**Where unsafe is justified:**
- Raw Vulkan API calls (ash is inherently unsafe by design)
- Custom allocators (frame allocator, pool allocator) — with extensive tests
- SIMD intrinsics (only when `glam` doesn't suffice)
- FFI to system libraries (evdev, xkbcommon, ALSA)
- Never in gameplay code, never in ECS system code, never in asset loading

**Strategy:**
- Each `unsafe` block has a `// SAFETY:` comment explaining invariants
- `unsafe` functions have `# Safety` doc sections
- Clippy deny `unsafe` in crates that don't need it
- Miri testing for crates with unsafe code

### 4. Modularity Without Over-Engineering

The engine is modular (separate crates, plugin system), but not a microservice architecture. Overly abstracted systems add complexity without benefit.

**What we do:**
- Crates have clear dependencies (no circular deps)
- Plugin trait for opt-in subsystems
- Systems communicate through ECS components, not message passing
- The renderer is abstracted behind the frame graph, not a virtual interface

**What we don't do:**
- No trait objects for render backends (we use Vulkan, period)
- No dynamic plugin loading in initial phases
- No message bus between subsystems (ECS components + events are sufficient)

### 5. Linux-First, Cross-Platform Ready

Pop!_OS is the primary development target. Windows support comes later, but the architecture doesn't paint us into a corner.

**Linux-specific decisions:**
- Wayland primary → X11 fallback (via winit features)
- evdev for raw input (bypasses window system latency)
- Thread pinning (pthread_setaffinity_np on AMD)
- Vulkan only (no GL/D3D abstraction — no reason to support them)
- ALSA/pulse for audio

**Cross-platform hooks:**
- winit abstracts windowing
- Trauma layer between platform and engine core (future: D3D12/Metal via gfx-hal or custom)
- Audio abstraction (cpal handles backend differences)
- Only Vulkan for GPU (it runs on all desktop platforms: Linux, Windows, macOS via MoltenVK)

### 6. Async Where It Helps, Sync Where It Matters

Async (tokio) is used for IO-bound work: asset loading, networking, file streaming. Game simulation is synchronous and deterministic.

**Async boundary:**
- Asset loading pipeline: async IO on tokio runtime, sync processing on rayon
- Networking: async transport (quinn is async), sync simulation
- World streaming: async region loading, sync entity integration

**Why not async everything:**
- ECS systems are fundamentally synchronous (mutate world state in stages)
- Determinism requires ordered, predictable execution
- Async adds overhead to hot paths (allocations, state machines)
- Profile barrier: mixing sync and async is harder to optimize

### 7. Scalability from the Start

We build for MMORPG-scale, not just small games. This influences everything:

- Chunk-based world from day one (not retrofitting later)
- Networking with interest management (AOI grid), not broadcasting to all
- ECS designed for 100k+ entities
- Memory systems designed to minimize fragmentation over hours of gameplay
- Asset streaming, not loading everything at startup
- Profiling instrumentation built in (can't optimize what you can't measure)

### 8. Editor-Ready Architecture

The engine is designed to be editor-hosting from the start:

- App builder pattern allows multiple engine instances (editor + game view)
- Systems are inspectable (component reflection for property editing)
- Event system for undo/redo hooks
- Asset pipeline has import/export stages (editor modifies → reimport)
- Scene serialization built into ECS component model

### 9. Build for the Hardware

| Hardware | Strategy |
|----------|----------|
| AMD CPU (many cores) | Work-stealing job system, thread pinning, cache-line awareness |
| NVIDIA GPU | Explicit control, bindless descriptors, async compute, mesh shaders |
| NVMe SSD | Direct IO where possible, async readahead for streaming |
| 16+ GB RAM | Aggressive caching, large staging buffer pools |

### 10. Practical Over Trendy

We use proven technology, not the latest hype:

- `hecs` (mature, lightweight ECS) over `bevy_ecs` (heavy, bevy-tied)
- `ash` (raw Vulkan) over `wgpu` (high-level, less control)
- `glam` (standard, SIMD math) over `nalgebra` (heavy, academic)
- `rayon` (battle-tested parallelism) over `tokio` (for compute tasks)
- `rapier` (production physics) over custom (not worth the effort)
- `tracing` (structured, composable) over `log` (basic, unstructured)

### 11. Actual Crate Versions (Phase 0)

The following versions are locked and tested for Phase 0:

| Crate | Version | Role |
|-------|---------|------|
| `hecs` | 0.11 | Archetypal ECS (lightweight, ~10 deps) |
| `glam` | 0.29 | SIMD math (SSE2/AVX, const-compatible) |
| `rayon` | 1.10 | Work-stealing thread pool |
| `ash` | 0.38 + 1.3.281 | Raw Vulkan bindings |
| `winit` | 0.30.13 | Windowing + input events |
| `gpu-allocator` | 0.28 | GPU memory allocation |
| `tracing` | 0.1 | Structured logging |
| `parking_lot` | 0.12 | Fast synchronization primitives |
| `serde` | 1 | Serialization framework |
| `toml` | 0.8 | Configuration format |

### 12. API Migration Debt (Fall 2024 → Spring 2026)

Both ash and winit underwent significant breaking API changes between their 2024 and 2026 releases. The Rustix codebase absorbed these migrations during Phase 0:

- **ash 0.38+1.3.281**: Extension loaders relocated to `ash::khr`/`ash::ext` namespaces. Function access now requires `loader.fp().function_name_khr(...)`. Queue submission moved to `Device::queue_submit`. `PhysicalDeviceLimits` field names changed to snake_case without camelCase middle components.
- **winit 0.30.13**: `WindowBuilder` replaced by `WindowAttributes`. `Event` enum restructured — `RedrawRequested` moved to `WindowEvent`. `ActiveEventLoop` is the new window creation entry point (deprecating `EventLoop::create_window`). Raw window handle traits gated behind `rwh_06` feature flag.

These migrations consumed significant Phase 0 effort but ensure the engine builds on stable Rust 1.95 with current crate ecosystem.

## What We Don't Do

- No runtime reflection system (proc macros for component registration instead)
- No dynamic plugin loading in v1 (compile-time plugins only)
- No built-in networking authority model beyond client-server
- No engine-side entity scripting in Lua (WASM only, future)
- No baked-in game framework (this is an engine, not a game template)
- No prefab system beyond what ECS serialization provides

## Coding Standards

See `docs/CODING_STANDARDS.md` (TBD) for:
- Naming conventions
- Error handling patterns
- Testing requirements
- Documentation requirements
- Code review checklist
