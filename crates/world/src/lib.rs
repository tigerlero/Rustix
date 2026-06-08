use std::collections::HashMap;

pub mod scene_graph;
pub mod spatial;
pub mod time_of_day;
pub mod weather;
pub mod serialization;
pub mod save_load;
pub mod multi_scene;
pub mod editor_meta;

#[cfg(test)]
pub mod time_of_day_tests;
#[cfg(test)]
pub mod weather_tests;
#[cfg(test)]
pub mod spatial_tests;
#[cfg(test)]
pub mod editor_meta_tests;
#[cfg(test)]
pub mod multi_scene_tests;
#[cfg(test)]
pub mod save_load_tests;
#[cfg(test)]
pub mod serialization_tests;
#[cfg(test)]
pub mod scene_graph_tests;
#[cfg(test)]
pub mod lib_tests;

pub use scene_graph::{Parent, Children, LocalTransform, GlobalTransform, propagate_transforms, compute_hierarchy_depth_first};
pub use spatial::SpatialHash;
pub use time_of_day::TimeOfDay;
pub use weather::{WeatherState, lerp_weather};
pub use serialization::{SerializedEntity, WorldSnapshot, WorldSerializer, WorldDeserializer};
pub use save_load::{SaveHeader, SaveMigrator, MigrationFn};
pub use multi_scene::{Scene, SceneManager};
pub use editor_meta::{EditorMetadata, EditorState, EditorLayer, GizmoMode};

/// Chunk coordinates in the world grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub fn new(x: i32, z: i32) -> Self { Self { x, z } }

    pub fn distance_sq(&self, other: &ChunkCoord) -> i32 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        dx * dx + dz * dz
    }

    pub fn neighbors(&self) -> [ChunkCoord; 4] {
        [
            ChunkCoord::new(self.x + 1, self.z),
            ChunkCoord::new(self.x - 1, self.z),
            ChunkCoord::new(self.x, self.z + 1),
            ChunkCoord::new(self.x, self.z - 1),
        ]
    }
}

/// State of a chunk in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkState {
    Unloaded,
    Loading,
    Loaded,
    Unloading,
}

/// A single chunk with its current state.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub coord: ChunkCoord,
    pub state: ChunkState,
    pub entity: Option<hecs::Entity>,
}

/// Manages which chunks are loaded around a center point.
#[derive(Debug, Clone)]
pub struct ChunkManager {
    pub chunk_size: f32,
    pub load_radius: i32,
    pub chunks: HashMap<ChunkCoord, Chunk>,
    pub center: ChunkCoord,
}

impl ChunkManager {
    pub fn new(chunk_size: f32, load_radius: i32) -> Self {
        Self {
            chunk_size,
            load_radius,
            chunks: HashMap::new(),
            center: ChunkCoord::new(0, 0),
        }
    }

    /// Update the center position and return lists of chunks to load/unload.
    pub fn update(&mut self, world_x: f32, world_z: f32) -> (Vec<ChunkCoord>, Vec<ChunkCoord>) {
        let new_center = ChunkCoord::new(
            (world_x / self.chunk_size).floor() as i32,
            (world_z / self.chunk_size).floor() as i32,
        );
        self.center = new_center;

        let radius_sq = self.load_radius * self.load_radius;
        let mut to_load = Vec::new();
        let mut to_unload = Vec::new();

        // Mark all current chunks for potential unload
        for (coord, chunk) in &self.chunks {
            if chunk.state == ChunkState::Loaded && coord.distance_sq(&new_center) > radius_sq {
                to_unload.push(*coord);
            }
        }

        // Find chunks that should be loaded
        for dx in -self.load_radius..=self.load_radius {
            for dz in -self.load_radius..=self.load_radius {
                if dx * dx + dz * dz > radius_sq {
                    continue;
                }
                let coord = ChunkCoord::new(new_center.x + dx, new_center.z + dz);
                match self.chunks.get(&coord) {
                    None | Some(Chunk { state: ChunkState::Unloaded, .. }) => {
                        to_load.push(coord);
                    }
                    _ => {}
                }
            }
        }

        (to_load, to_unload)
    }

    pub fn mark_loading(&mut self, coord: ChunkCoord, entity: hecs::Entity) {
        self.chunks.insert(coord, Chunk {
            coord,
            state: ChunkState::Loading,
            entity: Some(entity),
        });
    }

    pub fn mark_loaded(&mut self, coord: ChunkCoord) {
        if let Some(chunk) = self.chunks.get_mut(&coord) {
            chunk.state = ChunkState::Loaded;
        }
    }

    pub fn mark_unloaded(&mut self, coord: ChunkCoord) {
        self.chunks.remove(&coord);
    }

    pub fn world_origin(&self, coord: ChunkCoord) -> (f32, f32) {
        (coord.x as f32 * self.chunk_size, coord.z as f32 * self.chunk_size)
    }
}
