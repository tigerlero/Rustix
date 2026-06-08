use rustix_core::ecs::EcsWorld;
use rustix_core::math::{Vec3, Mat4, Quat, EulerRot};
use rustix_render::{DirectionalLight, PointLight, SpotLight};
use rustix_scripting::ScriptComponent;
use rustix_physics::{RigidBody, Collider};

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

impl Material {
    /// Build a runtime `Material` from an asset definition.
    pub fn from_asset(asset: &rustix_asset::material::MaterialAsset) -> Self {
        Self {
            base_color: Vec3::new(asset.base_color[0], asset.base_color[1], asset.base_color[2]),
            alpha: asset.base_color[3],
            roughness: asset.roughness,
            metallic: asset.metallic,
            ao: asset.ao,
            emissive: asset.emissive,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parent(pub Option<hecs::Entity>);

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
            parent_idx,
        });
    }
    tracing::debug!("world_to_scene: serialized {} entities", entities.len());
    SceneData { entities }
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

/// Spawn a slice of prefab entities into the world, returning root entities.
fn spawn_prefab_entities(
    world: &mut EcsWorld,
    entities: &[rustix_asset::prefab::PrefabEntity],
    base_position: Vec3,
    base_rotation: Vec3,
    base_scale: Vec3,
) -> Vec<hecs::Entity> {
    use rustix_asset::prefab::*;
    use rustix_render::Camera;
    use rustix_audio::{AudioListener, AudioSource};

    let mut idx_to_entity = Vec::with_capacity(entities.len());
    let mut roots = Vec::new();

    for entity_def in entities {
        let pos = Vec3::from(entity_def.position) + base_position;
        let rot = Vec3::from(entity_def.rotation) + base_rotation;
        let scl = Vec3::new(
            entity_def.scale[0] * base_scale.x,
            entity_def.scale[1] * base_scale.y,
            entity_def.scale[2] * base_scale.z,
        );

        let material = entity_def.material.as_ref().map(|m| Material {
            base_color: Vec3::new(m.base_color.x, m.base_color.y, m.base_color.z),
            alpha: m.alpha,
            roughness: m.roughness,
            metallic: m.metallic,
            ao: m.ao,
            emissive: m.emissive,
        }).unwrap_or(Material {
            base_color: Vec3::new(0.7, 0.7, 0.7),
            alpha: 1.0,
            roughness: 0.5,
            metallic: 0.0,
            ao: 1.0,
            emissive: 0.0,
        });

        let entity = world.spawn((
            Name(entity_def.name.clone()),
            Transform { position: pos, rotation: rot, scale: scl },
            MeshComponent(entity_def.mesh.clone().unwrap_or_else(|| "Cube".into())),
            material,
        ));

        if let Some(ref dl) = entity_def.dirlight {
            let _ = world.insert(entity, (DirectionalLight {
                color: Vec3::new(dl.color.x, dl.color.y, dl.color.z),
                intensity: dl.intensity,
            },));
        }
        if let Some(ref pl) = entity_def.pointlight {
            let _ = world.insert(entity, (PointLight {
                color: Vec3::new(pl.color.x, pl.color.y, pl.color.z),
                intensity: pl.intensity,
                radius: pl.radius,
            },));
        }
        if let Some(ref sl) = entity_def.spotlight {
            let _ = world.insert(entity, (SpotLight {
                color: Vec3::new(sl.color.x, sl.color.y, sl.color.z),
                intensity: sl.intensity,
                inner_angle: sl.inner_angle,
                outer_angle: sl.outer_angle,
                radius: sl.radius,
            },));
        }
        if let Some(ref sc) = entity_def.script {
            let _ = world.insert(entity, (ScriptComponent {
                source: sc.source.clone(),
                config: rustix_scripting::ScriptConfig { enabled: sc.config.enabled },
            },));
        }
        if let Some(ref rb) = entity_def.rigidbody {
            let _ = world.insert(entity, (RigidBody {
                body_type: match rb.body_type {
                    PrefabBodyType::Static => rustix_physics::BodyType::Static,
                    PrefabBodyType::Kinematic => rustix_physics::BodyType::Kinematic,
                    PrefabBodyType::Dynamic => rustix_physics::BodyType::Dynamic,
                },
                mass: rb.mass,
                velocity: Vec3::new(rb.velocity.x, rb.velocity.y, rb.velocity.z),
                angular_velocity: Vec3::new(rb.angular_velocity.x, rb.angular_velocity.y, rb.angular_velocity.z),
                gravity_scale: rb.gravity_scale,
                drag: rb.drag,
                angular_drag: rb.angular_drag,
                use_gravity: rb.use_gravity,
                can_sleep: rb.can_sleep,
                sleeping: rb.sleeping,
            },));
        }
        if let Some(ref col) = entity_def.collider {
            let shape = match col.shape {
                PrefabColliderShape::Sphere { radius } => rustix_physics::ColliderShape::Sphere { radius },
                PrefabColliderShape::Box { half_extents } => rustix_physics::ColliderShape::Box {
                    half_extents: Vec3::new(half_extents.x, half_extents.y, half_extents.z),
                },
                PrefabColliderShape::Capsule { radius, height } => rustix_physics::ColliderShape::Capsule { radius, height },
            };
            let _ = world.insert(entity, (Collider {
                shape,
                is_trigger: col.is_trigger,
                restitution: col.restitution,
                friction: col.friction,
            },));
        }
        if let Some(ref al) = entity_def.audiolistener {
            let _ = world.insert(entity, (AudioListener {
                position: Vec3::new(al.position.x, al.position.y, al.position.z),
                forward: Vec3::new(al.forward.x, al.forward.y, al.forward.z),
                up: Vec3::new(al.up.x, al.up.y, al.up.z),
            },));
        }
        if let Some(ref asrc) = entity_def.audiosource {
            let _ = world.insert(entity, (AudioSource {
                position: Vec3::new(asrc.position.x, asrc.position.y, asrc.position.z),
                min_distance: asrc.min_distance,
                max_distance: asrc.max_distance,
                rolloff: asrc.rolloff,
            },));
        }
        if let Some(ref cam) = entity_def.camera {
            let _ = world.insert(entity, (Camera {
                fov_degrees: cam.fov_degrees,
                near: cam.near,
                far: cam.far,
            },));
        }

        idx_to_entity.push(entity);
    }

    for (idx, entity_def) in entities.iter().enumerate() {
        if let Some(parent_idx) = entity_def.parent_idx {
            if let Some(&parent_entity) = idx_to_entity.get(parent_idx) {
                let child_entity = idx_to_entity[idx];
                let _ = world.insert(child_entity, (Parent(Some(parent_entity)),));
            }
        }
    }

    for (idx, entity_def) in entities.iter().enumerate() {
        if entity_def.parent_idx.is_none() {
            roots.push(idx_to_entity[idx]);
        }
    }

    roots
}

/// Spawn a prefab asset into the world, returning the spawned root entities.
///
/// The prefab's entities are created with an optional base transform offset.
/// Parent-child relationships within the prefab are preserved.
pub fn spawn_prefab(
    world: &mut EcsWorld,
    prefab: &rustix_asset::prefab::PrefabAsset,
    base_position: Vec3,
    base_rotation: Vec3,
    base_scale: Vec3,
) -> Vec<hecs::Entity> {
    let roots = spawn_prefab_entities(world, &prefab.data.entities, base_position, base_rotation, base_scale);
    tracing::debug!("spawn_prefab: spawned {} entities ({} roots)", prefab.data.entities.len(), roots.len());
    roots
}

/// Spawn a region / world asset into the world, returning the spawned root entities.
///
/// Region metadata (ambient color, fog, etc.) is applied as ECS components
/// on a dedicated metadata entity if the runtime has matching components.
pub fn spawn_region(
    world: &mut EcsWorld,
    region: &rustix_asset::region::RegionAsset,
    base_position: Vec3,
    base_rotation: Vec3,
    base_scale: Vec3,
) -> Vec<hecs::Entity> {
    let roots = spawn_prefab_entities(world, &region.data.entities, base_position, base_rotation, base_scale);
    tracing::debug!(
        "spawn_region: spawned {} entities ({} roots) for '{}'",
        region.data.entities.len(),
        roots.len(),
        region.data.metadata.name,
    );
    roots
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
