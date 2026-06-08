//! Component definition from scripts.

use std::collections::HashMap;

/// A dynamically defined component schema.
#[derive(Debug, Clone, Default)]
pub struct ScriptComponentDef {
    pub name: String,
    pub fields: HashMap<String, ScriptFieldType>,
}

/// Types available for script-defined component fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptFieldType {
    Float,
    Int,
    Bool,
    String,
    Vec3,
    Entity,
}

/// Registry of script-defined component schemas.
#[derive(Debug, Default)]
pub struct ComponentRegistry {
    pub defs: HashMap<String, ScriptComponentDef>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn define(&mut self, name: impl Into<String>, fields: HashMap<String, ScriptFieldType>) {
        let name = name.into();
        self.defs.insert(name.clone(), ScriptComponentDef { name, fields });
    }

    pub fn get(&self, name: &str) -> Option<&ScriptComponentDef> {
        self.defs.get(name)
    }

    pub fn remove(&mut self, name: &str) {
        self.defs.remove(name);
    }
}
