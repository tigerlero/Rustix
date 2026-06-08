//! Spatial partitioning: uniform grid for fast entity queries.
//!
//! A simple 3D spatial hash that stores entity handles in cells.

use hecs::Entity;
use rustix_core::math::Vec3;
use std::collections::{HashMap, HashSet};

/// A uniform-grid spatial hash for entity positions.
#[derive(Debug, Clone)]
pub struct SpatialHash {
    pub cell_size: f32,
    cells: HashMap<(i32, i32, i32), HashSet<Entity>>,
}

impl SpatialHash {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    fn cell(&self, pos: Vec3) -> (i32, i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
            (pos.z / self.cell_size).floor() as i32,
        )
    }

    /// Insert an entity at a position.
    pub fn insert(&mut self, entity: Entity, pos: Vec3) {
        let c = self.cell(pos);
        self.cells.entry(c).or_default().insert(entity);
    }

    /// Remove an entity from its current cell.
    pub fn remove(&mut self, entity: Entity, pos: Vec3) {
        let c = self.cell(pos);
        if let Some(set) = self.cells.get_mut(&c) {
            set.remove(&entity);
            if set.is_empty() {
                self.cells.remove(&c);
            }
        }
    }

    /// Update entity position (remove from old, insert to new).
    pub fn update(&mut self, entity: Entity, old_pos: Vec3, new_pos: Vec3) {
        let old_c = self.cell(old_pos);
        let new_c = self.cell(new_pos);
        if old_c != new_c {
            if let Some(set) = self.cells.get_mut(&old_c) {
                set.remove(&entity);
                if set.is_empty() {
                    self.cells.remove(&old_c);
                }
            }
            self.cells.entry(new_c).or_default().insert(entity);
        }
    }

    /// Query all entities in the cell containing `pos`.
    pub fn query_cell(&self, pos: Vec3) -> Vec<Entity> {
        let c = self.cell(pos);
        self.cells.get(&c).map(|s| s.iter().copied().collect()).unwrap_or_default()
    }

    /// Query all entities within `radius` of `pos`.
    pub fn query_sphere(&self, pos: Vec3, radius: f32) -> Vec<Entity> {
        let mut results = Vec::new();
        let r = radius / self.cell_size;
        let base = self.cell(pos);
        let range = r.ceil() as i32;

        for dx in -range..=range {
            for dy in -range..=range {
                for dz in -range..=range {
                    let c = (base.0 + dx, base.1 + dy, base.2 + dz);
                    if let Some(set) = self.cells.get(&c) {
                        for &entity in set {
                            results.push(entity);
                        }
                    }
                }
            }
        }
        results
    }
}
