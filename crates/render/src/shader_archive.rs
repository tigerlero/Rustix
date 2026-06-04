//! Pre-compiled shader archive for release builds.
//!
//! At compile time `build.rs` walks `shaders/` and compiles every `.vert`,
//! `.frag`, `.comp`, and `.glsl` file to SPIR-V via naga.  The resulting
//! binary is embedded in the crate via a generated Rust source file (in
//! `OUT_DIR`) that contains a `lookup(name) -> Option<(&[u32], ShaderStage)>`
//! function.
//!
//! In **release** builds the `builtin` module creates `ShaderModule`s from
//! this archive — no GLSL parsing happens at runtime.
//! In **debug** builds the archive is skipped and shaders are compiled from
//! source so that hot-reload and `#include` resolution still work.

pub use generated::lookup;

mod generated {
    use naga::ShaderStage;
    include!(concat!(env!("OUT_DIR"), "/shader_archive_gen.rs"));
}
