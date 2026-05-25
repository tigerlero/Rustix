use rustix_core::ecs::EcsWorld;
use rustix_core::math::{Vec3, Mat4, Quat, EulerRot};
use rustix_render::{DirectionalLight, PointLight, SpotLight};

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

#[derive(Debug, Clone)]
pub struct Material {
    pub base_color: Vec3,
    pub roughness: f32,
    pub metallic: f32,
}

#[derive(Debug, Clone)]
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

pub fn world_to_scene(world: &EcsWorld) -> SceneData {
    let mut entities = Vec::new();
    for (entity, name, t) in world.query::<(&hecs::Entity, &Name, &Transform)>().iter() {
        let dirlight = world.get::<&DirectionalLight>(*entity).ok().map(|r| *r);
        let pointlight = world.get::<&PointLight>(*entity).ok().map(|r| *r);
        let spotlight = world.get::<&SpotLight>(*entity).ok().map(|r| *r);
        let mesh = world.get::<&MeshComponent>(*entity).ok().map(|r| r.0.clone());
        entities.push(SceneEntity {
            name: name.0.clone(),
            position: t.position.into(),
            rotation: t.rotation.into(),
            scale: t.scale.into(),
            mesh,
            dirlight,
            pointlight,
            spotlight,
        });
    }
    SceneData { entities }
}

pub fn scene_to_world(world: &mut EcsWorld, data: &SceneData) {
    world.clear();
    for e in &data.entities {
        let entity = world.spawn((
            Name(e.name.clone()),
            Transform {
                position: e.position.into(),
                rotation: e.rotation.into(),
                scale: e.scale.into(),
            },
            MeshComponent(e.mesh.clone().unwrap_or_else(|| "Cube".into())),
            Material { base_color: Vec3::new(0.7, 0.7, 0.7), roughness: 0.5, metallic: 0.0 },
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
    }
}
