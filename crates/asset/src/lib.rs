pub mod handle;
pub mod server;
pub mod importer;
pub mod load_state;
pub mod hot_reload;
pub mod mmap;
pub mod mesh;
pub mod texture;
pub mod material;
pub mod shader;
pub mod audio;
pub mod animation;
pub mod skeleton;
pub mod physics;
pub mod prefab;
pub mod region;
pub mod font;
pub mod decoder_pool;
pub mod loader;
pub mod streaming;
pub mod cache;
pub mod vfs;
pub mod texture_compress;
pub mod mesh_opt;
pub mod cook;
pub mod dependency_graph;

#[cfg(test)]
pub mod handle_tests;
#[cfg(test)]
pub mod cache_tests;
#[cfg(test)]
pub mod vfs_tests;
#[cfg(test)]
mod mesh_tests;
#[cfg(test)]
mod texture_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod shader_tests;
#[cfg(test)]
mod skeleton_tests;
#[cfg(test)]
mod animation_tests;
#[cfg(test)]
mod audio_tests;
#[cfg(test)]
mod physics_tests;
#[cfg(test)]
mod font_tests;
#[cfg(test)]
mod prefab_tests;
#[cfg(test)]
mod region_tests;
#[cfg(test)]
mod importer_tests;
#[cfg(test)]
mod load_state_tests;
#[cfg(test)]
mod streaming_tests;
#[cfg(test)]
mod cook_tests;
#[cfg(test)]
mod texture_compress_tests;
#[cfg(test)]
mod server_tests;
#[cfg(test)]
mod hot_reload_tests;
#[cfg(test)]
mod mesh_opt_tests;
#[cfg(test)]
mod decoder_pool_tests;
#[cfg(test)]
pub mod loader_tests;
#[cfg(test)]
pub mod dependency_graph_tests;
#[cfg(test)]
pub mod mmap_tests;

pub use handle::*;
pub use loader::*;
pub use streaming::*;
pub use cache::*;
pub use vfs::*;
pub use texture_compress::*;
pub use mesh_opt::*;
pub use cook::*;
pub use dependency_graph::*;
pub use server::*;
pub use importer::*;
pub use load_state::*;
pub use hot_reload::*;
pub use mmap::*;

// Re-export serialization functions for convenience
pub use importer::{import_ron, import_json, export_ron, export_json, SerializableAsset};
