use rustix_core::ecs::EcsWorld;
use rustix_core::math::{Vec3, Mat4, Quat, EulerRot};
use rustix_render::{DirectionalLight, PointLight, SpotLight};
use rustix_scripting::ScriptComponent;

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

#[derive(Debug, Clone)]
pub struct MeshComponent(pub String);

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Material {
    pub base_color: Vec3,
    pub roughness: f32,
    pub metallic: f32,
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
            roughness: 0.5,
            metallic: 0.0,
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
    entity
}

pub fn world_to_scene(world: &EcsWorld) -> SceneData {
    let mut entities = Vec::new();
    let mut entity_to_idx = std::collections::HashMap::new();
    for (idx, (entity, _name, _t)) in world.query::<(&hecs::Entity, &Name, &Transform)>().iter().enumerate() {
        entity_to_idx.insert(*entity, idx);
    }
    for (entity, name, t) in world.query::<(&hecs::Entity, &Name, &Transform)>().iter() {
        let dirlight = world.get::<&DirectionalLight>(*entity).ok().map(|r| *r);
        let pointlight = world.get::<&PointLight>(*entity).ok().map(|r| *r);
        let spotlight = world.get::<&SpotLight>(*entity).ok().map(|r| *r);
        let mesh = world.get::<&MeshComponent>(*entity).ok().map(|r| r.0.clone());
        let material = world.get::<&Material>(*entity).ok().map(|r| (*r).clone());
        let script = world.get::<&ScriptComponent>(*entity).ok().map(|r| (*r).clone());
        let parent_idx = world.get::<&Parent>(*entity).ok()
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
            parent_idx,
        });
    }
    SceneData { entities }
}

pub fn scene_to_world(world: &mut EcsWorld, data: &SceneData) {
    world.clear();
    let mut idx_to_entity = Vec::with_capacity(data.entities.len());
    for e in &data.entities {
        let mat = e.material.clone().unwrap_or(Material {
            base_color: Vec3::new(0.7, 0.7, 0.7),
            roughness: 0.5,
            metallic: 0.0,
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
}
