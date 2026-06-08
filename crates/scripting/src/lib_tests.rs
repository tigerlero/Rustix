//! Tests for scripting events, math API, and component definitions.

use rustix_core::math::{Vec3, Quat};
use crate::events::*;
use crate::math_api::*;
use crate::component_def::*;
use crate::{ScriptId, Script, ScriptConfig, ScriptComponent, ScriptApi, ScriptLoader, ScriptRegistry};
use std::collections::HashMap;

// ---------- events.rs ----------

#[test]
fn event_bus_new_is_empty() {
    let bus = ScriptEventBus::new();
    assert!(bus.get_subscribers("test").is_empty());
    assert!(bus.emit("test").is_empty());
}

#[test]
fn event_bus_subscribe_and_emit() {
    let mut bus = ScriptEventBus::new();
    let cb = EventCallback { script_id: 1, function_name: "on_event".to_string() };
    bus.subscribe("damage", cb.clone());

    let emitted = bus.emit("damage");
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].script_id, 1);
    assert_eq!(emitted[0].function_name, "on_event");
}

#[test]
fn event_bus_multiple_subscribers() {
    let mut bus = ScriptEventBus::new();
    bus.subscribe("jump", EventCallback { script_id: 1, function_name: "a".to_string() });
    bus.subscribe("jump", EventCallback { script_id: 2, function_name: "b".to_string() });

    let emitted = bus.emit("jump");
    assert_eq!(emitted.len(), 2);
}

#[test]
fn event_bus_unsubscribe_script() {
    let mut bus = ScriptEventBus::new();
    bus.subscribe("move", EventCallback { script_id: 1, function_name: "a".to_string() });
    bus.subscribe("move", EventCallback { script_id: 2, function_name: "b".to_string() });
    bus.unsubscribe_script(1);

    let emitted = bus.emit("move");
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].script_id, 2);
}

#[test]
fn event_bus_different_events_isolated() {
    let mut bus = ScriptEventBus::new();
    bus.subscribe("a", EventCallback { script_id: 1, function_name: "x".to_string() });
    assert!(bus.emit("b").is_empty());
    assert_eq!(bus.emit("a").len(), 1);
}

// ---------- math_api.rs ----------

#[test]
fn math_vec3() {
    let v = vec3(1.0, 2.0, 3.0);
    assert_eq!(v, Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn math_lerp() {
    assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-4);
    assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < 1e-4);
    assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < 1e-4);
}

#[test]
fn math_dot() {
    let a = Vec3::X;
    let b = Vec3::Y;
    assert_eq!(dot(a, b), 0.0);
    assert_eq!(dot(a, a), 1.0);
}

#[test]
fn math_cross() {
    assert_eq!(cross(Vec3::X, Vec3::Y), Vec3::Z);
}

#[test]
fn math_normalize() {
    let v = Vec3::new(3.0, 0.0, 0.0);
    assert_eq!(normalize(v), Vec3::X);
}

#[test]
fn math_distance() {
    let a = Vec3::ZERO;
    let b = Vec3::X * 3.0;
    assert!((distance(a, b) - 3.0).abs() < 1e-4);
}

#[test]
fn math_quat_from_euler() {
    let q = quat_from_euler(0.0, 0.0, 0.0);
    assert!((q - Quat::IDENTITY).length() < 1e-4);
}

// ---------- component_def.rs ----------

#[test]
fn component_registry_new_empty() {
    let reg = ComponentRegistry::new();
    assert!(reg.get("health").is_none());
}

#[test]
fn component_registry_define_and_get() {
    let mut reg = ComponentRegistry::new();
    let mut fields = HashMap::new();
    fields.insert("hp".to_string(), ScriptFieldType::Float);
    fields.insert("max_hp".to_string(), ScriptFieldType::Float);
    reg.define("health", fields);

    let def = reg.get("health").unwrap();
    assert_eq!(def.name, "health");
    assert_eq!(def.fields.len(), 2);
    assert_eq!(def.fields["hp"], ScriptFieldType::Float);
}

#[test]
fn component_registry_remove() {
    let mut reg = ComponentRegistry::new();
    let mut fields = HashMap::new();
    fields.insert("x".to_string(), ScriptFieldType::Float);
    reg.define("pos", fields);
    assert!(reg.get("pos").is_some());
    reg.remove("pos");
    assert!(reg.get("pos").is_none());
}

#[test]
fn script_field_type_variants() {
    assert_ne!(ScriptFieldType::Float, ScriptFieldType::Int);
    assert_ne!(ScriptFieldType::Bool, ScriptFieldType::String);
    assert_eq!(ScriptFieldType::Vec3, ScriptFieldType::Vec3);
}

// ---------- lib.rs core types ----------

#[test]
fn script_id_equality() {
    assert_eq!(ScriptId(1), ScriptId(1));
    assert_ne!(ScriptId(1), ScriptId(2));
}

#[test]
fn script_default() {
    let script: Script = Default::default();
    assert!(script.source.is_empty());
    assert!(script.path.is_none());
}

#[test]
fn script_config_default() {
    let cfg = ScriptConfig::default();
    assert!(!cfg.enabled); // Default derive sets bool to false
}

#[test]
fn script_component_default() {
    let comp: ScriptComponent = Default::default();
    assert!(comp.source.is_empty());
    assert_eq!(comp.config, ScriptConfig::default());
}

#[test]
fn script_api_new() {
    let api = ScriptApi::new();
    assert!(api.instances.is_empty());
    assert_eq!(api.next_instance_id, 0);
}

#[test]
fn script_api_register_and_unregister() {
    let mut api = ScriptApi::new();
    let mut world = hecs::World::new();
    let entity = world.spawn(());
    let ast = rhai::AST::default();
    let id = api.register(entity, ast);
    assert_eq!(id, 0);
    assert_eq!(api.instances.len(), 1);

    api.unregister(id);
    assert!(api.instances.is_empty());
}

#[test]
fn script_loader_from_memory() {
    let script = ScriptLoader::load_from_memory("let x = 1;");
    assert_eq!(script.source, "let x = 1;");
    assert!(script.path.is_none());
}

#[test]
fn script_registry_new() {
    let reg = ScriptRegistry::new();
    assert!(reg.get(&ScriptId(0)).is_none());
}

#[test]
fn script_registry_register_and_get() {
    let mut reg = ScriptRegistry::new();
    let _script = ScriptLoader::load_from_memory("");
    let handle = rustix_asset::Handle::new(0, 0);
    reg.register(ScriptId(1), handle, std::path::PathBuf::from("test.rhai"));
    assert!(reg.get(&ScriptId(1)).is_some());
    let (id, _) = reg.get_by_path(std::path::Path::new("test.rhai")).unwrap();
    assert_eq!(id, ScriptId(1));
}
