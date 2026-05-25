use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::Vec3;

use crate::scene::{Transform, Name, MeshComponent, Material};
use crate::undo::{UndoHistory, EditorAction};

pub fn handle_undo_redo(
    ctx: &egui::Context,
    world: &mut EcsWorld,
    selected_entity: &std::cell::RefCell<Option<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    undo_history: &std::cell::RefCell<UndoHistory>,
) {
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z) && !i.modifiers.shift) {
        let action = undo_history.borrow_mut().undo().cloned();
        if let Some(action) = action {
            match action {
                EditorAction::AddEntity(entity) => {
                    let _ = world.despawn(entity);
                    if *selected_entity.borrow() == Some(entity) {
                        *selected_entity.borrow_mut() = None;
                    }
                }
                EditorAction::DeleteEntity { name, transform, mesh, material, metallic } => {
                    let e = world.spawn((Name(name), transform, MeshComponent(mesh), Material { base_color: Vec3::new(material.x, material.y, material.z), roughness: material.w, metallic }));
                    *selected_entity.borrow_mut() = Some(e);
                }
                EditorAction::RenameEntity { entity, old_name } => {
                    for (e, n) in world.query_mut::<(&Entity, &mut Name)>() {
                        if *e == entity {
                            n.0 = old_name;
                            break;
                        }
                    }
                }
                EditorAction::TransformEntity { entity, old_transform } => {
                    for (e, t) in world.query_mut::<(&Entity, &mut Transform)>() {
                        if *e == entity {
                            *t = old_transform;
                            break;
                        }
                    }
                }
            }
            dirty.set(true);
        }
    }

    if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z)) {
        let action = undo_history.borrow_mut().redo().cloned();
        if let Some(action) = action {
            match action {
                EditorAction::AddEntity(_entity) => {
                    // Re-adding is too fragile since the old entity ID is gone
                }
                EditorAction::DeleteEntity { name, transform, mesh, material, metallic } => {
                    let e = world.spawn((Name(name), transform, MeshComponent(mesh), Material { base_color: Vec3::new(material.x, material.y, material.z), roughness: material.w, metallic }));
                    *selected_entity.borrow_mut() = Some(e);
                }
                EditorAction::RenameEntity { entity, old_name } => {
                    for (e, n) in world.query_mut::<(&Entity, &mut Name)>() {
                        if *e == entity {
                            n.0 = old_name;
                            break;
                        }
                    }
                }
                EditorAction::TransformEntity { entity, old_transform } => {
                    for (e, t) in world.query_mut::<(&Entity, &mut Transform)>() {
                        if *e == entity {
                            *t = old_transform;
                            break;
                        }
                    }
                }
            }
            dirty.set(true);
        }
    }
}
