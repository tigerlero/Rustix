use rustix_core::ecs::EcsWorld;
use rustix_core::math::{Vec3, Mat4, Quat, EulerRot};
use rustix_render::{DirectionalLight, PointLight, SpotLight};
use rustix_scripting::ScriptComponent;
use rustix_physics::{RigidBody, Collider};
use rustix_animation::Skeleton;
use rustix_audio::AudioSource;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE }
    }
}

#[derive(Debug, Clone)]
pub struct Name(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct MeshComponent(pub String);

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Material {
    pub base_color: Vec3,
    #[serde(default = "default_alpha")]
    pub alpha: f32,
    pub roughness: f32,
    pub metallic: f32,
    pub ao: f32,
    pub emissive: f32,
}

fn default_alpha() -> f32 { 1.0 }

#[derive(Debug, Clone, PartialEq)]
pub struct Parent(pub Option<hecs::Entity>);

/// Tag component that identifies which scene an entity belongs to.
/// Used for additive scene loading and selective unloading.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneTag(pub String);

/// Metadata for a scene that is currently loaded in the world.
#[derive(Debug, Clone)]
pub struct LoadedScene {
    pub name: String,
    pub path: String,
    pub entity_count: usize,
}

/// A distance-based trigger for streaming scenes in/out.
#[derive(Debug, Clone)]
pub struct StreamingZone {
    pub name: String,
    pub center: Vec3,
    pub radius: f32,
    pub scene_path: String,
    pub loaded: bool,
}

/// Manages additive scene loading, unloading, and streaming.
#[derive(Debug, Clone, Default)]
pub struct SceneManager {
    pub loaded_scenes: Vec<LoadedScene>,
    pub streaming_zones: Vec<StreamingZone>,
    pub show_manager: bool,
}

impl SceneManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_loaded(&self, name: &str) -> bool {
        self.loaded_scenes.iter().any(|s| s.name == name)
    }

    pub fn register(&mut self, name: String, path: String, entity_count: usize) {
        if let Some(existing) = self.loaded_scenes.iter_mut().find(|s| s.name == name) {
            existing.entity_count = entity_count;
        } else {
            self.loaded_scenes.push(LoadedScene { name, path, entity_count });
        }
    }

    pub fn unregister(&mut self, name: &str) {
        self.loaded_scenes.retain(|s| s.name != name);
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneEntity {
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    #[serde(default)]
    pub mesh: Option<String>,
    #[serde(default)]
    pub dirlight: Option<DirectionalLight>,
    #[serde(default)]
    pub pointlight: Option<PointLight>,
    #[serde(default)]
    pub spotlight: Option<SpotLight>,
    #[serde(default)]
    pub material: Option<Material>,
    #[serde(default)]
    pub script: Option<ScriptComponent>,
    #[serde(default)]
    pub rigidbody: Option<RigidBody>,
    #[serde(default)]
    pub collider: Option<Collider>,
    #[serde(default)]
    pub audiolistener: Option<rustix_audio::AudioListener>,
    #[serde(default)]
    pub camera: Option<rustix_render::Camera>,
    #[serde(default)]
    pub skeleton: Option<Skeleton>,
    #[serde(default)]
    pub terrain: Option<crate::terrain::Terrain>,
    #[serde(default)]
    pub audio_source: Option<AudioSource>,
    #[serde(default)]
    pub scene_tag: Option<String>,
    pub parent_idx: Option<usize>,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SceneData {
    pub entities: Vec<SceneEntity>,
}

pub fn world_transform(world: &EcsWorld, entity: hecs::Entity) -> Mat4 {
    let mut matrix = Mat4::IDENTITY;
    let mut current = Some(entity);
    let mut depth = 0;
    while let Some(e) = current {
        if depth > 64 { break; }
        depth += 1;
        if let Ok(t) = world.get::<&Transform>(e) {
            let rot = Quat::from_euler(EulerRot::XYZ, t.rotation.x, t.rotation.y, t.rotation.z);
            let local = Mat4::from_scale_rotation_translation(t.scale, rot, t.position);
            matrix = local * matrix;
        }
        current = world.get::<&Parent>(e).ok().and_then(|p| p.0);
    }
    matrix
}

pub fn entity_to_scene_entity(world: &EcsWorld, entity: hecs::Entity) -> SceneEntity {
    let name = world.get::<&Name>(entity).ok().map(|r| r.0.clone()).unwrap_or_default();
    let t = world.get::<&Transform>(entity).ok().map(|r| (*r).clone()).unwrap_or_default();
    let mesh = world.get::<&MeshComponent>(entity).ok().map(|r| r.0.clone());
    let material = world.get::<&Material>(entity).ok().map(|r| (*r).clone());
    let dirlight = world.get::<&DirectionalLight>(entity).ok().map(|r| *r);
    let pointlight = world.get::<&PointLight>(entity).ok().map(|r| *r);
    let spotlight = world.get::<&SpotLight>(entity).ok().map(|r| *r);
    let script = world.get::<&ScriptComponent>(entity).ok().map(|r| (*r).clone());
    let rigidbody = world.get::<&RigidBody>(entity).ok().map(|r| *r);
    let collider = world.get::<&Collider>(entity).ok().map(|r| *r);
    let audiolistener = world.get::<&rustix_audio::AudioListener>(entity).ok().map(|r| *r);
    let camera = world.get::<&rustix_render::Camera>(entity).ok().map(|r| *r);
    let skeleton = world.get::<&Skeleton>(entity).ok().map(|r| (*r).clone());
    let terrain = world.get::<&crate::terrain::Terrain>(entity).ok().map(|r| (*r).clone());
    let audio_source = world.get::<&AudioSource>(entity).ok().map(|r| *r);
    let scene_tag = world.get::<&SceneTag>(entity).ok().map(|r| r.0.clone());
    SceneEntity {
        name,
        position: t.position.into(),
        rotation: t.rotation.into(),
        scale: t.scale.into(),
        mesh,
        dirlight,
        pointlight,
        spotlight,
        material,
        script,
        rigidbody,
        collider,
        audiolistener,
        camera,
        skeleton,
        terrain,
        audio_source,
        scene_tag,
        parent_idx: None,
    }
}

pub fn spawn_entity(world: &mut EcsWorld, e: &SceneEntity) -> hecs::Entity {
    let entity = world.spawn((
        Name(e.name.clone()),
        Transform {
            position: e.position.into(),
            rotation: e.rotation.into(),
            scale: e.scale.into(),
        },
        MeshComponent(e.mesh.clone().unwrap_or_else(|| "Cube".into())),
        e.material.clone().unwrap_or(Material {
            base_color: Vec3::new(0.7, 0.7, 0.7),
            alpha: 1.0,
            roughness: 0.5,
            metallic: 0.0,
            ao: 1.0,
            emissive: 0.0,
        }),
    ));
    if let Some(ref dl) = e.dirlight {
        let _ = world.insert(entity, (*dl,));
    }
    if let Some(ref pl) = e.pointlight {
        let _ = world.insert(entity, (*pl,));
    }
    if let Some(ref sl) = e.spotlight {
        let _ = world.insert(entity, (*sl,));
    }
    if let Some(ref sc) = e.script {
        let _ = world.insert(entity, (sc.clone(),));
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
    if let Some(ref src) = e.audio_source {
        let _ = world.insert(entity, (*src,));
    }
    if let Some(ref tag) = e.scene_tag {
        let _ = world.insert(entity, (SceneTag(tag.clone()),));
    }
    entity
}

pub fn world_to_scene(world: &EcsWorld) -> SceneData {
    let mut entities = Vec::new();
    let mut entity_to_idx = std::collections::HashMap::new();
    for (idx, (entity, _name, _t)) in world.query::<(hecs::Entity, &Name, &Transform)>().iter().enumerate() {
        entity_to_idx.insert(entity, idx);
    }
    tracing::debug!("world_to_scene: found {} entities", entity_to_idx.len());
    for (entity, name, t) in world.query::<(hecs::Entity, &Name, &Transform)>().iter() {
        let dirlight = world.get::<&DirectionalLight>(entity).ok().map(|r| *r);
        let pointlight = world.get::<&PointLight>(entity).ok().map(|r| *r);
        let spotlight = world.get::<&SpotLight>(entity).ok().map(|r| *r);
        let mesh = world.get::<&MeshComponent>(entity).ok().map(|r| r.0.clone());
        let material = world.get::<&Material>(entity).ok().map(|r| (*r).clone());
        let script = world.get::<&ScriptComponent>(entity).ok().map(|r| (*r).clone());
        let rigidbody = world.get::<&RigidBody>(entity).ok().map(|r| *r);
        let collider = world.get::<&Collider>(entity).ok().map(|r| *r);
        let audiolistener = world.get::<&rustix_audio::AudioListener>(entity).ok().map(|r| *r);
        let camera = world.get::<&rustix_render::Camera>(entity).ok().map(|r| *r);
        let skeleton = world.get::<&Skeleton>(entity).ok().map(|r| (*r).clone());
        let terrain = world.get::<&crate::terrain::Terrain>(entity).ok().map(|r| (*r).clone());
        let audio_source = world.get::<&AudioSource>(entity).ok().map(|r| *r);
        let scene_tag = world.get::<&SceneTag>(entity).ok().map(|r| r.0.clone());
        let parent_idx = world.get::<&Parent>(entity).ok()
            .and_then(|p| p.0.and_then(|pe| entity_to_idx.get(&pe).copied()));
        entities.push(SceneEntity {
            name: name.0.clone(),
            position: t.position.into(),
            rotation: t.rotation.into(),
            scale: t.scale.into(),
            mesh,
            dirlight,
            pointlight,
            spotlight,
            material,
            script,
            rigidbody,
            collider,
            audiolistener,
            camera,
            skeleton,
            terrain,
            audio_source,
            scene_tag,
            parent_idx,
        });
    }
    tracing::debug!("world_to_scene: serialized {} entities", entities.len());
    SceneData { entities }
}

pub fn save_scene(path: &std::path::Path, data: &SceneData) -> Option<()> {
    let json = match serde_json::to_string_pretty(data) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("failed to serialize scene: {}", e);
            return None;
        }
    };
    if let Err(e) = std::fs::write(path, &json) {
        tracing::error!("failed to write scene file {}: {}", path.display(), e);
        return None;
    }
    tracing::info!("saved scene to {} with {} entities", path.display(), data.entities.len());
    Some(())
}

pub fn load_scene(path: &std::path::Path) -> Option<SceneData> {
    let json = match std::fs::read_to_string(path) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("failed to read scene file {}: {}", path.display(), e);
            return None;
        }
    };
    let data: SceneData = match serde_json::from_str(&json) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("failed to parse scene file {}: {}", path.display(), e);
            return None;
        }
    };
    tracing::info!("loaded scene from {} with {} entities", path.display(), data.entities.len());
    Some(data)
}

pub fn scene_to_world(world: &mut EcsWorld, data: &SceneData) {
    world.clear();
    tracing::debug!("scene_to_world: loading {} entities", data.entities.len());
    let mut idx_to_entity = Vec::with_capacity(data.entities.len());
    for e in &data.entities {
        let mat = e.material.clone().unwrap_or(Material {
            base_color: Vec3::new(0.7, 0.7, 0.7),
            alpha: 1.0,
            roughness: 0.5,
            metallic: 0.0,
            ao: 1.0,
            emissive: 0.0,
        });
        let entity = world.spawn((
            Name(e.name.clone()),
            Transform {
                position: e.position.into(),
                rotation: e.rotation.into(),
                scale: e.scale.into(),
            },
            MeshComponent(e.mesh.clone().unwrap_or_else(|| "Cube".into())),
            mat,
        ));
        if let Some(ref dl) = e.dirlight {
            let _ = world.insert(entity, (*dl,));
        }
        if let Some(ref pl) = e.pointlight {
            let _ = world.insert(entity, (*pl,));
        }
        if let Some(ref sl) = e.spotlight {
            let _ = world.insert(entity, (*sl,));
        }
        if let Some(ref sc) = e.script {
            let _ = world.insert(entity, (sc.clone(),));
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
        if let Some(ref src) = e.audio_source {
            let _ = world.insert(entity, (*src,));
        }
        if let Some(ref tag) = e.scene_tag {
            let _ = world.insert(entity, (SceneTag(tag.clone()),));
        }
        idx_to_entity.push(entity);
    }
    for (idx, e) in data.entities.iter().enumerate() {
        if let Some(parent_idx) = e.parent_idx {
            if let Some(&parent_entity) = idx_to_entity.get(parent_idx) {
                let child_entity = idx_to_entity[idx];
                let _ = world.insert(child_entity, (Parent(Some(parent_entity)),));
            }
        }
    }
    let spawned = world.query::<(&Name,)>().iter().count();
    tracing::debug!("scene_to_world: spawned {} entities in world", spawned);
}

/// Additively load a `SceneData` into an existing world without clearing it.
/// Every spawned entity gets a `SceneTag` with the provided `scene_name`.
/// Returns the list of spawned entity handles.
pub fn merge_scene_into_world(world: &mut EcsWorld, data: &SceneData, scene_name: &str) -> Vec<hecs::Entity> {
    tracing::debug!("merge_scene_into_world: loading {} entities from '{}'", data.entities.len(), scene_name);
    let mut idx_to_entity = Vec::with_capacity(data.entities.len());
    for e in &data.entities {
        let mat = e.material.clone().unwrap_or(Material {
            base_color: Vec3::new(0.7, 0.7, 0.7),
            alpha: 1.0,
            roughness: 0.5,
            metallic: 0.0,
            ao: 1.0,
            emissive: 0.0,
        });
        let entity = world.spawn((
            Name(e.name.clone()),
            Transform {
                position: e.position.into(),
                rotation: e.rotation.into(),
                scale: e.scale.into(),
            },
            MeshComponent(e.mesh.clone().unwrap_or_else(|| "Cube".into())),
            mat,
            SceneTag(scene_name.to_string()),
        ));
        if let Some(ref dl) = e.dirlight {
            let _ = world.insert(entity, (*dl,));
        }
        if let Some(ref pl) = e.pointlight {
            let _ = world.insert(entity, (*pl,));
        }
        if let Some(ref sl) = e.spotlight {
            let _ = world.insert(entity, (*sl,));
        }
        if let Some(ref sc) = e.script {
            let _ = world.insert(entity, (sc.clone(),));
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
        if let Some(ref src) = e.audio_source {
            let _ = world.insert(entity, (*src,));
        }
        idx_to_entity.push(entity);
    }
    for (idx, e) in data.entities.iter().enumerate() {
        if let Some(parent_idx) = e.parent_idx {
            if let Some(&parent_entity) = idx_to_entity.get(parent_idx) {
                let child_entity = idx_to_entity[idx];
                let _ = world.insert(child_entity, (Parent(Some(parent_entity)),));
            }
        }
    }
    tracing::debug!("merge_scene_into_world: spawned {} entities from '{}'", idx_to_entity.len(), scene_name);
    idx_to_entity
}

/// Despawn every entity that carries the given `SceneTag` name, including
/// any children that reference them as parents.
pub fn unload_scene(world: &mut EcsWorld, scene_name: &str) -> usize {
    let mut to_remove: Vec<hecs::Entity> = Vec::new();
    for (entity, tag) in world.query::<(hecs::Entity, &SceneTag)>().iter() {
        if tag.0 == scene_name {
            to_remove.push(entity);
        }
    }
    // Also remove children whose parent is being removed.
    let remove_set: std::collections::HashSet<hecs::Entity> = to_remove.iter().copied().collect();
    let mut additional: Vec<hecs::Entity> = Vec::new();
    for (entity, parent) in world.query::<(hecs::Entity, &Parent)>().iter() {
        if let Some(pe) = parent.0 {
            if remove_set.contains(&pe) && !remove_set.contains(&entity) {
                additional.push(entity);
            }
        }
    }
    to_remove.extend(additional);

    let count = to_remove.len();
    for entity in to_remove {
        let _ = world.despawn(entity);
    }
    tracing::debug!("unload_scene: removed {} entities for '{}'", count, scene_name);
    count
}

/// Extract a `SceneData` from the world containing only entities that match
/// the given `SceneTag`. Entities without a tag are omitted.
pub fn world_to_scene_by_tag(world: &EcsWorld, scene_name: &str) -> SceneData {
    let mut entities = Vec::new();
    let mut entity_to_idx = std::collections::HashMap::new();
    for (idx, (entity, _name, _t, tag)) in world.query::<(hecs::Entity, &Name, &Transform, &SceneTag)>().iter().enumerate() {
        if tag.0 == scene_name {
            entity_to_idx.insert(entity, idx);
        }
    }
    for (entity, name, t) in world.query::<(hecs::Entity, &Name, &Transform)>().iter() {
        let tag = match world.get::<&SceneTag>(entity) {
            Ok(t) if t.0 == scene_name => true,
            _ => false,
        };
        if !tag { continue; }
        let mesh = world.get::<&MeshComponent>(entity).ok().map(|r| r.0.clone());
        let material = world.get::<&Material>(entity).ok().map(|r| (*r).clone());
        let dirlight = world.get::<&DirectionalLight>(entity).ok().map(|r| *r);
        let pointlight = world.get::<&PointLight>(entity).ok().map(|r| *r);
        let spotlight = world.get::<&SpotLight>(entity).ok().map(|r| *r);
        let script = world.get::<&ScriptComponent>(entity).ok().map(|r| (*r).clone());
        let rigidbody = world.get::<&RigidBody>(entity).ok().map(|r| *r);
        let collider = world.get::<&Collider>(entity).ok().map(|r| *r);
        let audiolistener = world.get::<&rustix_audio::AudioListener>(entity).ok().map(|r| *r);
        let camera = world.get::<&rustix_render::Camera>(entity).ok().map(|r| *r);
        let skeleton = world.get::<&Skeleton>(entity).ok().map(|r| (*r).clone());
        let terrain = world.get::<&crate::terrain::Terrain>(entity).ok().map(|r| (*r).clone());
        let audio_source = world.get::<&AudioSource>(entity).ok().map(|r| *r);
        let scene_tag = world.get::<&SceneTag>(entity).ok().map(|r| r.0.clone());
        let parent_idx = world.get::<&Parent>(entity).ok()
            .and_then(|p| p.0.and_then(|pe| entity_to_idx.get(&pe).copied()));
        entities.push(SceneEntity {
            name: name.0.clone(),
            position: t.position.into(),
            rotation: t.rotation.into(),
            scale: t.scale.into(),
            mesh,
            dirlight,
            pointlight,
            spotlight,
            material,
            script,
            rigidbody,
            collider,
            audiolistener,
            camera,
            skeleton,
            terrain,
            audio_source,
            scene_tag,
            parent_idx,
        });
    }
    SceneData { entities }
}

/// Evaluate streaming zones against a viewer position (e.g. the camera or player).
/// Returns a list of `(scene_path, should_be_loaded)` changes that the caller
/// should act on.
pub fn evaluate_streaming(
    zones: &mut [StreamingZone],
    viewer_pos: Vec3,
) -> Vec<(String, bool)> {
    let mut changes = Vec::new();
    for zone in zones.iter_mut() {
        let dist = (zone.center - viewer_pos).length();
        let should_load = dist <= zone.radius;
        if should_load && !zone.loaded {
            zone.loaded = true;
            changes.push((zone.scene_path.clone(), true));
        } else if !should_load && zone.loaded {
            zone.loaded = false;
            changes.push((zone.scene_path.clone(), false));
        }
    }
    changes
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "multi_scene_tests.rs"]
mod multi_scene_tests;
