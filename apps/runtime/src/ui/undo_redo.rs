use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::Vec3;
use rustix_render::{DirectionalLight, PointLight, SpotLight};
use rustix_audio::AudioSource;

use crate::scene::{Transform, Name, MeshComponent, Material};
use crate::undo::{UndoHistory, EditorAction};

fn default_material() -> Material {
    Material { base_color: Vec3::new(0.7, 0.7, 0.7), roughness: 0.5, metallic: 0.0 }
}
fn default_audio_source() -> AudioSource {
    AudioSource { position: Vec3::ZERO, min_distance: 1.0, max_distance: 100.0, rolloff: 1.0 }
}

pub fn handle_undo_redo(
    ctx: &egui::Context,
    world: &mut EcsWorld,
    selected_entity: &std::cell::RefCell<Option<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    undo_history: &std::cell::RefCell<UndoHistory>,
) {
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z) && !i.modifiers.shift) {
        let (action, idx) = {
            let mut history = undo_history.borrow_mut();
            let action = history.undo().cloned();
            (action, history.index)
        };
        if let Some(action) = action {
            match action {
                EditorAction::AddEntity { entity, .. } => {
                    let _ = world.despawn(entity);
                    if *selected_entity.borrow() == Some(entity) {
                        *selected_entity.borrow_mut() = None;
                    }
                }
                EditorAction::DeleteEntity { snapshot, .. } => {
                    let e = crate::scene::spawn_entity(world, &snapshot);
                    *selected_entity.borrow_mut() = Some(e);
                    undo_history.borrow_mut().actions[idx] = EditorAction::DeleteEntity { entity: e, snapshot };
                }
                EditorAction::RenameEntity { entity, old_name } => {
                    for (e, n) in world.query_mut::<(Entity, &mut Name)>() {
                        if e == entity {
                            n.0 = old_name;
                            break;
                        }
                    }
                }
                EditorAction::TransformEntity { entity, old_transform } => {
                    for (e, t) in world.query_mut::<(Entity, &mut Transform)>() {
                        if e == entity {
                            *t = old_transform;
                            break;
                        }
                    }
                }
                EditorAction::DirectionalLightChanged { entity, old } => {
                    for (e, l) in world.query_mut::<(Entity, &mut DirectionalLight)>() {
                        if e == entity { *l = old; break; }
                    }
                }
                EditorAction::PointLightChanged { entity, old } => {
                    for (e, l) in world.query_mut::<(Entity, &mut PointLight)>() {
                        if e == entity { *l = old; break; }
                    }
                }
                EditorAction::SpotLightChanged { entity, old } => {
                    for (e, l) in world.query_mut::<(Entity, &mut SpotLight)>() {
                        if e == entity { *l = old; break; }
                    }
                }
                EditorAction::MaterialChanged { entity, old } => {
                    for (e, m) in world.query_mut::<(Entity, &mut Material)>() {
                        if e == entity { *m = old; break; }
                    }
                }
                EditorAction::AudioSourceChanged { entity, old } => {
                    for (e, a) in world.query_mut::<(Entity, &mut AudioSource)>() {
                        if e == entity { *a = old; break; }
                    }
                }
                EditorAction::ComponentAdded { entity, old_snapshot, .. } => {
                    let _ = world.despawn(entity);
                    let e = crate::scene::spawn_entity(world, &old_snapshot);
                    if *selected_entity.borrow() == Some(entity) {
                        *selected_entity.borrow_mut() = Some(e);
                    }
                }
                EditorAction::ComponentRemoved { entity, old_snapshot, .. } => {
                    let _ = world.despawn(entity);
                    let e = crate::scene::spawn_entity(world, &old_snapshot);
                    if *selected_entity.borrow() == Some(entity) {
                        *selected_entity.borrow_mut() = Some(e);
                    }
                }
            }
            dirty.set(true);
        }
    }

    if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z)) {
        let (action, idx) = {
            let mut history = undo_history.borrow_mut();
            let idx = history.index;
            let action = history.redo().cloned();
            (action, idx)
        };
        if let Some(action) = action {
            match action {
                EditorAction::AddEntity { snapshot, .. } => {
                    let e = crate::scene::spawn_entity(world, &snapshot);
                    *selected_entity.borrow_mut() = Some(e);
                    undo_history.borrow_mut().actions[idx] = EditorAction::AddEntity { entity: e, snapshot };
                }
                EditorAction::DeleteEntity { entity, .. } => {
                    let _ = world.despawn(entity);
                    if *selected_entity.borrow() == Some(entity) {
                        *selected_entity.borrow_mut() = None;
                    }
                }
                EditorAction::RenameEntity { entity, old_name } => {
                    for (e, n) in world.query_mut::<(Entity, &mut Name)>() {
                        if e == entity {
                            n.0 = old_name;
                            break;
                        }
                    }
                }
                EditorAction::TransformEntity { entity, old_transform } => {
                    for (e, t) in world.query_mut::<(Entity, &mut Transform)>() {
                        if e == entity {
                            *t = old_transform;
                            break;
                        }
                    }
                }
                EditorAction::DirectionalLightChanged { entity, old } => {
                    for (e, l) in world.query_mut::<(Entity, &mut DirectionalLight)>() {
                        if e == entity { *l = old; break; }
                    }
                }
                EditorAction::PointLightChanged { entity, old } => {
                    for (e, l) in world.query_mut::<(Entity, &mut PointLight)>() {
                        if e == entity { *l = old; break; }
                    }
                }
                EditorAction::SpotLightChanged { entity, old } => {
                    for (e, l) in world.query_mut::<(Entity, &mut SpotLight)>() {
                        if e == entity { *l = old; break; }
                    }
                }
                EditorAction::MaterialChanged { entity, old } => {
                    for (e, m) in world.query_mut::<(Entity, &mut Material)>() {
                        if e == entity { *m = old; break; }
                    }
                }
                EditorAction::AudioSourceChanged { entity, old } => {
                    for (e, a) in world.query_mut::<(Entity, &mut AudioSource)>() {
                        if e == entity { *a = old; break; }
                    }
                }
                EditorAction::ComponentAdded { entity, component, .. } => {
                    let comp = component.as_str();
                    if comp == "DirectionalLight" {
                        let _ = world.insert(entity, (DirectionalLight::default(),));
                    } else if comp == "PointLight" {
                        let _ = world.insert(entity, (PointLight::default(),));
                    } else if comp == "SpotLight" {
                        let _ = world.insert(entity, (SpotLight::default(),));
                    } else if comp == "Material" {
                        let _ = world.insert(entity, (default_material(),));
                    } else if comp == "MeshComponent" {
                        let _ = world.insert(entity, (MeshComponent("Cube".into()),));
                    } else if comp == "AudioSource" {
                        let _ = world.insert(entity, (default_audio_source(),));
                    }
                }
                EditorAction::ComponentRemoved { entity, component, .. } => {
                    let comp = component.as_str();
                    if comp == "DirectionalLight" {
                        let _ = world.remove_one::<DirectionalLight>(entity);
                    } else if comp == "PointLight" {
                        let _ = world.remove_one::<PointLight>(entity);
                    } else if comp == "SpotLight" {
                        let _ = world.remove_one::<SpotLight>(entity);
                    } else if comp == "Material" {
                        let _ = world.remove_one::<Material>(entity);
                    } else if comp == "MeshComponent" {
                        let _ = world.remove_one::<MeshComponent>(entity);
                    } else if comp == "AudioSource" {
                        let _ = world.remove_one::<AudioSource>(entity);
                    }
                }
            }
            dirty.set(true);
        }
    }
}
