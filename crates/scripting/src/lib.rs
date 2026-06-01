//! Rustix scripting engine using Rhai
//!
//! Provides a lightweight, embeddable scripting system for game logic.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use hecs::Entity;
use serde::{Deserialize, Serialize};
use tracing::debug;

use rustix_asset::{Asset, Handle};
use rustix_core::components::Transform;
use rustix_core::math::{Mat4, Quat, Vec3};

/// Unique identifier for a script asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScriptId(pub u64);

/// A script asset containing Rhai code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Script {
    pub source: String,
    pub path: Option<PathBuf>,
}

impl Asset for Script {
    fn asset_type_id() -> rustix_asset::AssetTypeId {
        rustix_asset::AssetTypeId::from_crate_name("script")
    }
}

/// Configuration for a script behavior.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScriptConfig {
    pub enabled: bool,
}

/// Script component attached to entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptComponent {
    pub source: String,
    pub config: ScriptConfig,
}

impl Default for ScriptComponent {
    fn default() -> Self {
        Self {
            source: String::new(),
            config: ScriptConfig::default(),
        }
    }
}

/// Unique identifier for a running script instance.
pub type ScriptInstanceId = u64;

/// State of a script instance.
#[derive(Debug, Clone)]
pub struct ScriptInstance {
    pub id: ScriptInstanceId,
    pub entity: Entity,
    pub ast: rhai::AST,
}

/// Engine API functions available to scripts.
#[derive(Debug, Default)]
pub struct ScriptApi {
    pub instances: HashMap<ScriptInstanceId, ScriptInstance>,
    pub next_instance_id: ScriptInstanceId,
}

impl ScriptApi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, entity: Entity, ast: rhai::AST) -> ScriptInstanceId {
        let id = self.next_instance_id;
        self.next_instance_id = self.next_instance_id.wrapping_add(1);
        self.instances.insert(id, ScriptInstance { id, entity, ast });
        debug!(entity = ?entity, "Registered script instance");
        id
    }

    pub fn unregister(&mut self, id: ScriptInstanceId) {
        self.instances.remove(&id);
    }
}

/// The scripting engine managing script compilation and execution.
pub struct ScriptEngine {
    rhai_engine: rhai::Engine,
    api: ScriptApi,
}

impl std::fmt::Debug for ScriptEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptEngine")
            .field("rhai_engine", &"Engine")
            .field("instances", &self.api.instances.len())
            .finish()
    }
}

impl ScriptEngine {
    /// Create a new script engine.
    pub fn new() -> Self {
        let mut engine = rhai::Engine::new();

        // Register core types
        engine.register_type_with_name::<Vec3>("Vec3");
        engine.register_type_with_name::<Quat>("Quat");
        engine.register_type_with_name::<Mat4>("Mat4");

        // Register type constructors
        engine.register_fn("vec3", |x: f32, y: f32, z: f32| Vec3::new(x, y, z));
        engine.register_fn("quat", |x: f32, y: f32, z: f32, w: f32| Quat::from_xyzw(x, y, z, w));

        // Register math operations for Vec3
        engine.register_fn("length", Vec3::length);
        engine.register_fn("normalize", Vec3::normalize);
        engine.register_fn("dot", Vec3::dot);
        engine.register_fn("cross", Vec3::cross);
        engine.register_fn("distance", Vec3::distance);

        Self {
            rhai_engine: engine,
            api: ScriptApi::new(),
        }
    }

    /// Compile a script and return the AST.
    pub fn compile(&self, source: &str) -> Result<rhai::AST, ScriptError> {
        self.rhai_engine.compile(source).map_err(|e| ScriptError::CompileError(e.to_string()))
    }

    /// Create a new script instance for an entity.
    pub fn create_instance(&mut self, entity: Entity, source: &str, config: ScriptConfig) -> Result<ScriptInstanceId, ScriptError> {
        if !config.enabled {
            return Ok(0);
        }
        let ast = self.compile(source)?;
        Ok(self.api.register(entity, ast))
    }

    /// Run a script instance (called each tick).
    pub fn run_script(
        &mut self,
        id: ScriptInstanceId,
        world: &mut hecs::World,
    ) -> Result<(), ScriptError> {
        let instance = match self.api.instances.get(&id) {
            Some(inst) => inst.clone(),
            None => return Ok(()),
        };

        let mut scope = rhai::Scope::new();

        // Get entity's transform for script access
        if let Ok(t) = world.query_one::<&Transform>(instance.entity).get() {
            scope.push("translation", t.translation);
            scope.push("rotation", t.rotation);
            scope.push("scale", t.scale);
        }

        self.rhai_engine
            .run_ast_with_scope(&mut scope, &instance.ast)
            .map_err(|e| ScriptError::RuntimeError(e.to_string()))?;

        // Update entity from script modifications
        if let Ok(t) = world.query_one_mut::<&mut Transform>(instance.entity) {
            if let Some(trans) = scope.get_value::<Vec3>("translation") {
                t.translation = trans;
            }
            if let Some(rot) = scope.get_value::<Quat>("rotation") {
                t.rotation = rot;
            }
            if let Some(scl) = scope.get_value::<Vec3>("scale") {
                t.scale = scl;
            }
        }

        Ok(())
    }

    /// Register ECS API functions for script access.
    pub fn register_ecs_api(&mut self, _world: &mut hecs::World) {
        // Register functions that scripts can call:
        // - print(message) for debugging
        self.rhai_engine.register_fn("print", |s: &str| {
            println!("{}", s);
        });
        self.rhai_engine.register_fn("log", |s: &str| {
            tracing::info!("{}", s);
        });
    }

    /// Update all script instances (called each tick).
    pub fn update(&mut self, world: &mut hecs::World) {
        let instances: Vec<ScriptInstanceId> = self.api.instances.keys().copied().collect();
        for id in instances {
            let _ = self.run_script(id, world);
        }
    }

    /// Remove a script instance.
    pub fn remove_instance(&mut self, id: ScriptInstanceId) {
        self.api.unregister(id);
    }
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during script execution.
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    #[error("Failed to compile script: {0}")]
    CompileError(String),
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Script asset loader for the asset system.
pub struct ScriptLoader;

impl ScriptLoader {
    pub fn load(path: &Path) -> Result<Script, ScriptError> {
        let source = std::fs::read_to_string(path)?;
        Ok(Script {
            source,
            path: Some(path.to_path_buf()),
        })
    }

    pub fn load_from_memory(source: &str) -> Script {
        Script {
            source: source.to_string(),
            path: None,
        }
    }
}

/// Global script registry for tracking loaded scripts.
pub struct ScriptRegistry {
    scripts: HashMap<ScriptId, Handle<Script>>,
    paths: HashMap<PathBuf, ScriptId>,
}

impl ScriptRegistry {
    pub fn new() -> Self {
        Self {
            scripts: HashMap::new(),
            paths: HashMap::new(),
        }
    }

    pub fn register(&mut self, id: ScriptId, handle: Handle<Script>, path: PathBuf) {
        self.scripts.insert(id, handle);
        self.paths.insert(path, id);
    }

    pub fn get(&self, id: &ScriptId) -> Option<&Handle<Script>> {
        self.scripts.get(id)
    }

    pub fn get_by_path(&self, path: &Path) -> Option<(ScriptId, &Handle<Script>)> {
        self.paths.get(path)
            .and_then(|id| self.scripts.get(id).map(|h| (*id, h)))
    }
}

impl Default for ScriptRegistry {
    fn default() -> Self {
        Self::new()
    }
}