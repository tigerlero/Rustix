# Rustix Engine — Coding Standards

This document defines the conventions and requirements for all code in the Rustix workspace. Following these standards keeps the codebase consistent, reviewable, and maintainable as it scales across 17+ crates.

---

## 1. Naming Conventions

### 1.1 General Rust conventions

| Item | Convention | Example |
|------|------------|---------|
| Types (structs, enums, traits) | `PascalCase` | `GraphicsPipeline`, `CameraMode` |
| Functions, methods, variables | `snake_case` | `init_scene_resources`, `mesh_count` |
| Constants, statics | `SCREAMING_SNAKE_CASE` | `MAX_VIEWPORTS`, `UBO_SCENE_SIZE` |
| Generic parameters | Single uppercase letter, or `PascalCase` | `T`, `K`, `V`, `Input` |
| Modules, files | `snake_case` | `bindless/ops.rs`, `scene_tests.rs` |
| Feature flags | `snake_case` | `profiling`, `audio-playback` |

### 1.2 Project-specific prefixes

- **Crate prefixes**: `rustix-core`, `rustix-render`, `rustix-platform`, etc. Use the full crate name in `Cargo.toml`, but in code use the short module paths.
- **File naming for tests**: Extract inline `#[cfg(test)]` blocks into files named `<module>_tests.rs`. Declare them with `#[path = "<module>_tests.rs"]`.
- **Sub-module directories**: When splitting a large file (500+ lines) into submodules, create a directory named after the parent module (e.g., `bindless.rs` → `bindless/ops.rs`).

### 1.3 Avoid

- Hungarian notation (`szName`, `nCount`)
- Single-letter variable names except in trivial closures (`|e|`, `|x|`) or mathematical formulas
- `foo`, `bar`, `baz`, `tmp`, `test1` in production code

---

## 2. Error Handling Patterns

### 2.1 Custom error types

Every crate that can fail in domain-specific ways must define a central error enum.

```rust
// crates/render/src/lib.rs
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Vulkan error: {0}")]
    Vulkan(#[from] ash::vk::Result),
    #[error("GPU allocation failed: {0}")]
    Allocation(String),
    #[error("shader compilation failed: {0}")]
    ShaderCompile(String),
    #[error("invalid extent: {0}x{1}")]
    InvalidExtent(u32, u32),
}
```

- Use `thiserror` for boilerplate reduction.
- Implement `From<UnderlyingError>` for all wrapped error types.
- Do NOT use `anyhow` in library crates (only in binaries / `main.rs`).

### 2.2 The `?` operator

Prefer `?` over manual `match` error propagation:

```rust
// GOOD
let buf = renderer.create_buffer(name, size, usage, location)?;

// BAD
let buf = match renderer.create_buffer(name, size, usage, location) {
    Ok(b) => b,
    Err(e) => return Err(e),
};
```

### 2.3 Fatal vs. recoverable errors

| Fatal (panic / exit) | Recoverable (return `Result` / log) |
|----------------------|-------------------------------------|
| Programmer bug (null pointer where struct guaranteed) | GPU allocation failure (try smaller size) |
| Assertion violation in internal logic | Shader compile failure (fallback to builtin) |
| `unreachable!()` branches reached | File read failure (user-facing dialog) |

Use `tracing::error!` for recoverable errors that should be visible but not crash the process. Use `expect()` or `panic!` only when the program is in an unrecoverable invalid state that indicates a bug.

### 2.4 `unsafe` blocks

Every `unsafe` block must have a `// SAFETY:` comment explaining why the operation is sound:

```rust
// SAFETY: `device` is a valid `*const ash::Device` kept alive by `Renderer`.
unsafe { (*self.device).free_descriptor_sets(self.pool, &[set]) };
```

- Keep `unsafe` blocks as small as possible.
- Encapsulate unsafe logic in safe wrapper functions where feasible.
- Never expose raw Vulkan pointers in public APIs unless absolutely necessary.

---

## 3. Testing Requirements

### 3.1 Minimum coverage rules

| Crate type | Test requirement |
|------------|----------------|
| `crates/core` | Every public API must have at least one unit test |
| `crates/render` | Test resource creation, pipeline compilation, and memory math (GPU tests can be behind `--features test-gpu`) |
| `crates/platform` | Mock-based tests for input state machines |
| `apps/runtime` | UI logic extracted into pure functions tested in `_tests.rs`; integration tests for scene serialization |

### 3.2 Test file organization

```
crates/core/src/
  memory.rs           // main module
  memory_tests.rs     // extracted tests
crates/render/src/
  graph.rs            // main module
  graph_tests.rs      // extracted tests
```

In the source file:

```rust
#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
```

### 3.3 Test naming

```rust
#[test]
fn allocate_texture_slot_returns_index() { }

#[test]
fn free_texture_slot_reuses_index() { }

#[test]
#[should_panic(expected = "out of slots")]
fn alloc_texture_exhausted_panics() { }
```

- Start with the action: `fn <action>_<condition>_<expected_outcome>()`.
- For bug regression tests, prefix with `regression_` and reference the issue.

### 3.4 Assertions

Prefer `assert_eq!` / `assert_ne!` with messages over bare `assert!`:

```rust
assert_eq!(slot, 0, "first allocation should return slot 0");
```

For floating-point comparisons, use an epsilon:

```rust
assert!((result - expected).abs() < 1e-6, "result {} != expected {}", result, expected);
```

---

## 4. Documentation Requirements

### 4.1 Doc comments

Every `pub` item must have a doc comment:

```rust
/// Allocate a texture slot and write the image descriptor.
/// Returns the slot index (e.g., for push constants).
pub fn alloc_texture(&self, view: vk::ImageView, image_layout: vk::ImageLayout) -> u32 { }
```

- First sentence is a one-line summary.
- Following paragraphs explain parameters, return values, and behavior.
- Use `/// # Examples` for non-trivial functions.
- Use `/// # Safety` for `unsafe` functions.

### 4.2 Inline comments

Use inline comments sparingly — prefer self-documenting code. Comment **why**, not **what**:

```rust
// GOOD: explains the business reason
// Clamp distance to avoid near-plane clipping when orbiting close to objects.
self.distance = self.distance.max(2.0);

// BAD: restates the obvious code
// Set distance to the maximum of distance and 2.0
self.distance = self.distance.max(2.0);
```

### 4.3 Module-level documentation

Each `lib.rs` or major module should have a top-level doc comment explaining the module's responsibility:

```rust
//! Vulkan rendering backend for Rustix.
//!
//! This crate wraps the Vulkan API (via `ash`) and provides:
//! - Device initialization and swapchain management
//! - Descriptor heaps, pipeline caches, and shader hot-reload
//! - Frame graph for declarative render passes
```

### 4.4 README files

Each crate should have a `README.md` with:
- One-line description
- Dependency list (internal + external)
- How to run tests for this crate specifically (`cargo test -p rustix-core`)

---

## 5. Code Review Checklist

Before submitting a PR or merging a branch, verify:

### 5.1 Correctness

- [ ] The change addresses the root cause, not just symptoms.
- [ ] `unsafe` blocks have `// SAFETY:` comments.
- [ ] Error paths are handled (no silent `unwrap()` in library code).
- [ ] Resource leaks checked: every `vkCreate*` has a corresponding `vkDestroy*` or `Drop` impl.

### 5.2 Style

- [ ] `cargo fmt` applied (no manual formatting debates).
- [ ] `cargo clippy` warnings resolved (or explicitly allowed with a comment).
- [ ] No dead code, unused imports, or commented-out blocks left behind.
- [ ] Naming follows Section 1 conventions.

### 5.3 Testing

- [ ] New logic has unit tests.
- [ ] Existing tests still pass (`cargo test --workspace`).
- [ ] For bug fixes: a regression test is included if feasible.

### 5.4 Performance & Resources

- [ ] No unnecessary allocations in hot paths (use pools / arenas).
- [ ] GPU resources (pipelines, descriptor sets) are cached, not recreated per frame.
- [ ] Lock contention reviewed: prefer `Mutex` over `RwLock` for short critical sections; avoid locks in render loop if possible.

### 5.5 Documentation

- [ ] Public API items have doc comments.
- [ ] `ROADMAP.md` and `FEATURES.md` updated if completing a milestone.
- [ ] `CHANGELOG.md` entry added for user-visible changes.

### 5.6 Module structure

- [ ] Files under 500 lines when reasonable; large files split into logical submodules.
- [ ] Tests extracted into `_tests.rs` files, not inline.
- [ ] No circular dependencies between crates (check with `cargo tree --edges normal`).

---

## 6. Commit Message Format

```
<crate>: <short summary>

<body — explain what and why, not how>

Refs: #<issue-number>
```

Example:
```
render: fix bindless descriptor slot reuse

The free_texture_slots vector was not being re-sorted after allocation,
causing O(n) scans instead of O(1) pops. Switch to a VecDeque for
FIFO reuse of recently freed slots.

Refs: #42
```

---

## 7. Refactoring Guidelines

When splitting large files or extracting tests:

1. **Make fields `pub(crate)`** if submodules need access, not `pub` unless part of public API.
2. **Preserve `Drop` behavior** — ensure resources are still cleaned up after moving methods.
3. **Re-export at module root** — if splitting `bindless.rs` into `bindless/ops.rs`, re-export types in `bindless.rs` so callers don't need to update paths.
4. **Compile after every move** — check `cargo check` after each file change, not just at the end.
5. **Run tests after extraction** — `cargo test -p <crate>` to verify test paths still resolve.

---

*Last updated: 2026-06-06*
