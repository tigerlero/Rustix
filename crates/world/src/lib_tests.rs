//! Tests for world chunk management.

use crate::{ChunkCoord, ChunkManager, ChunkState};

#[test]
fn chunk_coord_new() {
    let c = ChunkCoord::new(3, -2);
    assert_eq!(c.x, 3);
    assert_eq!(c.z, -2);
}

#[test]
fn chunk_coord_distance_sq() {
    let a = ChunkCoord::new(0, 0);
    let b = ChunkCoord::new(3, 4);
    assert_eq!(a.distance_sq(&b), 25);
}

#[test]
fn chunk_coord_neighbors() {
    let c = ChunkCoord::new(0, 0);
    let n = c.neighbors();
    assert_eq!(n[0], ChunkCoord::new(1, 0));
    assert_eq!(n[1], ChunkCoord::new(-1, 0));
    assert_eq!(n[2], ChunkCoord::new(0, 1));
    assert_eq!(n[3], ChunkCoord::new(0, -1));
}

#[test]
fn chunk_manager_new() {
    let mgr = ChunkManager::new(32.0, 3);
    assert_eq!(mgr.chunk_size, 32.0);
    assert_eq!(mgr.load_radius, 3);
    assert!(mgr.chunks.is_empty());
}

#[test]
fn chunk_manager_update_loads_chunks() {
    let mut mgr = ChunkManager::new(32.0, 1);
    let (to_load, _to_unload) = mgr.update(0.0, 0.0);
    assert!(!to_load.is_empty());
    // Center chunk (0,0) plus 4 neighbors within radius 1 = 5 chunks
    assert_eq!(to_load.len(), 5);
}

#[test]
fn chunk_manager_update_unloads_distant() {
    let mut mgr = ChunkManager::new(32.0, 3);
    let mut world = hecs::World::new();
    // First load center
    let (to_load, _) = mgr.update(0.0, 0.0);
    for coord in to_load {
        mgr.mark_loading(coord, world.spawn(()));
        mgr.mark_loaded(coord);
    }
    // Move far away
    let (_to_load, to_unload) = mgr.update(1000.0, 1000.0);
    assert!(!to_unload.is_empty());
}

#[test]
fn chunk_manager_mark_loading_and_loaded() {
    let mut mgr = ChunkManager::new(32.0, 1);
    let coord = ChunkCoord::new(0, 0);
    let entity = hecs::World::new().spawn(());
    mgr.mark_loading(coord, entity);
    assert_eq!(mgr.chunks[&coord].state, ChunkState::Loading);
    mgr.mark_loaded(coord);
    assert_eq!(mgr.chunks[&coord].state, ChunkState::Loaded);
}

#[test]
fn chunk_manager_mark_unloaded_removes() {
    let mut mgr = ChunkManager::new(32.0, 1);
    let coord = ChunkCoord::new(0, 0);
    let entity = hecs::World::new().spawn(());
    mgr.mark_loading(coord, entity);
    mgr.mark_unloaded(coord);
    assert!(!mgr.chunks.contains_key(&coord));
}

#[test]
fn chunk_manager_world_origin() {
    let mgr = ChunkManager::new(32.0, 1);
    let (x, z) = mgr.world_origin(ChunkCoord::new(2, -1));
    assert_eq!(x, 64.0);
    assert_eq!(z, -32.0);
}
