//! Prefab system for Rustix.
//!
//! Allows saving entity hierarchies as reusable prefabs and instantiating them
//! in the scene with override tracking.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;
use crate::scene::{SceneData, SceneEntity, Transform, Name, MeshComponent, Material, Parent};

const PREFAB_EXT: &str = "rustixprefab";
const PREFAB_DIR: &str = "prefabs";

/// A prefab is a saved hierarchy of entities that can be instantiated multiple times.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Prefab {
    pub name: String,
    pub entities: Vec<SceneEntity>,
}

impl Prefab {
    /// Load a prefab from a file path.
    pub fn load(path: &Path) -> Option<Self> {
        let json = std::fs::read_to_string(path).ok()?;
        let mut prefab: Prefab = serde_json::from_str(&json).ok()?;
        prefab.name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Prefab")
            .to_string();
        Some(prefab)
    }

    /// Save the prefab to a file path.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialize prefab: {e}"))?;
        std::fs::write(path, json)
            .map_err(|e| format!("write prefab: {e}"))
    }

    /// Create a prefab from selected entities and their children.
    pub fn from_selection(
        world: &EcsWorld,
        selected: &[hecs::Entity],
    ) -> Option<Self> {
        if selected.is_empty() {
            return None;
        }

        // Collect all entities in the selected subtrees
        let mut collected = Vec::new();
        let mut to_visit = selected.to_vec();
        let mut visited = std::collections::HashSet::new();

        while let Some(entity) = to_visit.pop() {
            if !visited.insert(entity) {
                continue;
            }
            collected.push(entity);
            // Find children by scanning all Parent components
            for (child, parent) in world.query::<(hecs::Entity, &Parent)>().iter() {
                if parent.0 == Some(entity) {
                    to_visit.push(child);
                }
            }
        }

        // Build entity -> idx mapping for the collected set
        let mut entity_to_idx = HashMap::new();
        for (idx, &e) in collected.iter().enumerate() {
            entity_to_idx.insert(e, idx);
        }

        // Find root entities (those whose parent is NOT in the collected set)
        let mut roots = Vec::new();
        for &e in &collected {
            let parent_in_set = world.get::<&Parent>(e)
                .ok()
                .and_then(|p| p.0)
                .map(|pe| entity_to_idx.contains_key(&pe))
                .unwrap_or(false);
            if !parent_in_set {
                roots.push(e);
            }
        }
        if roots.is_empty() {
            return None;
        }

        // Serialize: roots first, then children (breadth-first ordering)
        let mut ordered = Vec::new();
        let mut queue = roots.clone();
        let mut seen = std::collections::HashSet::new();
        while let Some(e) = queue.pop() {
            if !seen.insert(e) {
                continue;
            }
            ordered.push(e);
            // Enqueue children
            for (child, parent) in world.query::<(hecs::Entity, &Parent)>().iter() {
                if parent.0 == Some(e) && collected.contains(&child) {
                    queue.push(child);
                }
            }
        }

        // Convert to SceneEntity with parent_idx relative to this prefab
        let mut entities = Vec::new();
        for e in ordered {
            let mut scene_ent = crate::scene::entity_to_scene_entity(world, e);
            // Remap parent_idx to prefab-local index
            scene_ent.parent_idx = world.get::<&Parent>(e)
                .ok()
                .and_then(|p| p.0)
                .and_then(|pe| entity_to_idx.get(&pe).copied());
            entities.push(scene_ent);
        }

        Some(Prefab {
            name: "Prefab".to_string(),
            entities,
        })
    }

    /// Instantiate this prefab into the world at the given position.
    /// Returns the root entity handles.
    pub fn instantiate(
        &self,
        world: &mut EcsWorld,
        position: Vec3,
    ) -> Vec<hecs::Entity> {
        let mut idx_to_entity: Vec<hecs::Entity> = Vec::with_capacity(self.entities.len());
        let mut root_entities = Vec::new();

        for (idx, e) in self.entities.iter().enumerate() {
            let is_root = e.parent_idx.is_none();
            let mut scene_ent = e.clone();

            if is_root {
                // Offset root positions by the spawn position
                scene_ent.position = [
                    e.position[0] + position.x,
                    e.position[1] + position.y,
                    e.position[2] + position.z,
                ];
            }

            let entity = crate::scene::spawn_entity(world, &scene_ent);

            // Insert optional components that spawn_entity doesn't handle
            if let Some(ref dl) = e.dirlight {
                let _ = world.insert(entity, (*dl,));
            }
            if let Some(ref pl) = e.pointlight {
                let _ = world.insert(entity, (*pl,));
            }
            if let Some(ref sl) = e.spotlight {
                let _ = world.insert(entity, (*sl,));
            }
            if let Some(ref script) = e.script {
                let _ = world.insert(entity, (script.clone(),));
            }
            if let Some(ref rb) = e.rigidbody {
                let _ = world.insert(entity, (*rb,));
            }
            if let Some(ref col) = e.collider {
                let _ = world.insert(entity, (*col,));
            }
            if let Some(ref al) = e.audiolistener {
                let _ = world.insert(entity, (*al,));
            }
            if let Some(ref cam) = e.camera {
                let _ = world.insert(entity, (*cam,));
            }
            if let Some(ref skel) = e.skeleton {
                let _ = world.insert(entity, (skel.clone(),));
            }
            if let Some(ref terrain) = e.terrain {
                let _ = world.insert(entity, (terrain.clone(),));
            }

            // Track prefab instance
            let _ = world.insert(entity, (PrefabInstance {
                prefab_path: String::new(), // filled below
                entity_idx: idx,
                overrides: Vec::new(),
            },));

            idx_to_entity.push(entity);
            if is_root {
                root_entities.push(entity);
            }
        }

        // Fix up parent relationships
        for (idx, e) in self.entities.iter().enumerate() {
            if let Some(parent_idx) = e.parent_idx {
                if let Some(&parent_entity) = idx_to_entity.get(parent_idx) {
                    let child_entity = idx_to_entity[idx];
                    let _ = world.insert(child_entity, (Parent(Some(parent_entity)),));
                }
            }
        }

        root_entities
    }
}

/// Component attached to entities that were spawned from a prefab.
/// Tracks overrides relative to the original prefab.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrefabInstance {
    pub prefab_path: String,
    pub entity_idx: usize,
    pub overrides: Vec<OverrideEntry>,
}

/// A single overridden property on a prefab instance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum OverrideEntry {
    Name(String),
    Position([f32; 3]),
    Rotation([f32; 3]),
    Scale([f32; 3]),
    Mesh(String),
    Material(Material),
    Enabled(bool),
}

/// Prefab editor state.
#[derive(Clone, Default)]
pub struct PrefabEditor {
    pub show: bool,
    pub selected_prefab: Option<String>,
    pub new_prefab_name: String,
    pub instantiate_at: Vec3,
}

/// Scan a project directory for available prefab files.
pub fn list_prefabs(project_dir: &Path) -> Vec<(String, PathBuf)> {
    let prefabs_dir = project_dir.join(PREFAB_DIR);
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&prefabs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some(PREFAB_EXT) {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unnamed")
                    .to_string();
                result.push((name, path));
            }
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Ensure the prefabs directory exists in a project.
pub fn ensure_prefabs_dir(project_dir: &Path) -> PathBuf {
    let dir = project_dir.join(PREFAB_DIR);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Compute overrides for a prefab instance entity by comparing with the original prefab.
pub fn compute_overrides(
    world: &EcsWorld,
    entity: hecs::Entity,
    prefab: &Prefab,
) -> Vec<OverrideEntry> {
    let mut overrides = Vec::new();
    let instance = match world.get::<&PrefabInstance>(entity).ok() {
        Some(i) => (*i).clone(),
        None => return overrides,
    };
    let original = match prefab.entities.get(instance.entity_idx) {
        Some(e) => e,
        None => return overrides,
    };

    if let Ok(name) = world.get::<&Name>(entity) {
        if name.0 != original.name {
            overrides.push(OverrideEntry::Name(name.0.clone()));
        }
    }
    if let Ok(t) = world.get::<&Transform>(entity) {
        let pos: [f32; 3] = t.position.into();
        if pos != original.position {
            overrides.push(OverrideEntry::Position(pos));
        }
        let rot: [f32; 3] = t.rotation.into();
        if rot != original.rotation {
            overrides.push(OverrideEntry::Rotation(rot));
        }
        let scl: [f32; 3] = t.scale.into();
        if scl != original.scale {
            overrides.push(OverrideEntry::Scale(scl));
        }
    }
    if let Ok(mesh) = world.get::<&MeshComponent>(entity) {
        let mesh_name = mesh.0.clone();
        if original.mesh.as_ref() != Some(&mesh_name) {
            overrides.push(OverrideEntry::Mesh(mesh_name));
        }
    }
    if let Ok(mat) = world.get::<&Material>(entity) {
        let mat_clone = (*mat).clone();
        if original.material.as_ref() != Some(&mat_clone) {
            overrides.push(OverrideEntry::Material(mat_clone));
        }
    }

    overrides
}

/// Apply stored overrides to a prefab instance entity.
pub fn apply_overrides(world: &mut EcsWorld, entity: hecs::Entity, overrides: &[OverrideEntry]) {
    for ov in overrides {
        match ov {
            OverrideEntry::Name(v) => {
                if let Ok(mut name) = world.get::<&mut Name>(entity) {
                    name.0 = v.clone();
                }
            }
            OverrideEntry::Position(v) => {
                if let Ok(mut t) = world.get::<&mut Transform>(entity) {
                    t.position = (*v).into();
                }
            }
            OverrideEntry::Rotation(v) => {
                if let Ok(mut t) = world.get::<&mut Transform>(entity) {
                    t.rotation = (*v).into();
                }
            }
            OverrideEntry::Scale(v) => {
                if let Ok(mut t) = world.get::<&mut Transform>(entity) {
                    t.scale = (*v).into();
                }
            }
            OverrideEntry::Mesh(v) => {
                let _ = world.insert(entity, (MeshComponent(v.clone()),));
            }
            OverrideEntry::Material(v) => {
                let _ = world.insert(entity, (v.clone(),));
            }
            OverrideEntry::Enabled(_) => {}
        }
    }
}
