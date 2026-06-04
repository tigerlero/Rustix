# Cross-Compilation Guide

This document covers cross-compiling the Rustix engine from a Linux host to Windows and macOS targets.

## Prerequisites

- Linux host with `rustup`, `cargo`, and `clang`/`lld` installed.
- For Windows targets: `mingw-w64` toolchain or `cargo-xwin`.
- For macOS targets: macOS SDK (see [osxcross](https://github.com/tpoechtrager/osxcross)) or `cargo-zigbuild`.

---

## Linux → Windows

### Option A: MinGW-w64 (system toolchain)

Install the MinGW-w64 cross-compiler:

```bash
# Debian / Ubuntu
sudo apt install mingw-w64

# Fedora
sudo dnf install mingw64-gcc
```

Add the target via `rustup`:

```bash
rustup target add x86_64-pc-windows-gnu
```

Build the runtime:

```bash
cargo build -p rustix-runtime --target x86_64-pc-windows-gnu --release
```

**Notes**
- The Windows Vulkan loader (`vulkan-1.dll`) is *not* statically linked; it must be present on the target machine (installed via the Vulkan SDK or GPU driver bundle).
- `winit` and `ash` (Win32 surface) are fully wired; no engine code changes are needed.
- Thread priority uses `SetThreadPriority` (already implemented in `crates/core/src/thread_priority.rs`).

### Option B: MSVC target via `cargo-xwin`

Install `cargo-xwin`:

```bash
cargo install cargo-xwin
rustup target add x86_64-pc-windows-msvc
```

Build:

```bash
cargo xwin build -p rustix-runtime --target x86_64-pc-windows-msvc --release
```

`cargo-xwin` downloads the Microsoft CRT and Windows SDK automatically. This produces a binary that uses the MSVC ABI and links against the official Windows import libraries.

---

## Linux → macOS

macOS cross-compilation requires the macOS SDK and a compatible linker. The easiest path is `cargo-zigbuild` with the macOS SDK.

### 1. Obtain the macOS SDK

Use [osxcross](https://github.com/tpoechtrager/osxcross) to extract the SDK from an Xcode `.xip` or download a free SDK package. You need:

- `MacOSX.sdk` (or `MacOSX15.sdk`, etc.)
- Environment variable `SDKROOT` pointing to the extracted SDK directory.

### 2. Install `cargo-zigbuild`

```bash
cargo install cargo-zigbuild
```

### 3. Build

```bash
export SDKROOT=/path/to/MacOSX.sdk
rustup target add aarch64-apple-darwin
cargo zigbuild -p rustix-runtime --target aarch64-apple-darwin --release
```

For Intel Macs use `x86_64-apple-darwin` instead.

**Notes**
- MoltenVK must be available on the target Mac (bundled with the Vulkan SDK or installed via `brew install molten-vk`).
- The engine surface creation code (`VK_MVK_macos_surface` / `VK_EXT_metal_surface`) is structurally ready; see `crates/render/src/surface.rs`.
- Thread priority uses `pthread_set_qos_class_self_np` (already implemented).
- macOS code signing and notarization are out of scope for this guide; refer to Apple documentation for distribution.

---

## Packaging Cross-Compiled Binaries

After cross-compilation, package the binaries using the target-native packaging tools:

- **Windows** (from Linux): Use `cargo-wix` (MSI) or `zip` the `.exe` + any runtime assets.
- **macOS** (from Linux): An `.app` bundle is just a directory structure; create it with a shell script, or transfer the binary to a Mac to run `create-dmg` / `appify`.

See `docs/FEATURES.md` for the current packaging status (`.deb`/`.rpm` for Linux, `.msi`/`.zip` for Windows, `.dmg`/`.app` for macOS).
