//! Multi-scene support: main world + UI scenes + additive sub-scenes.

use hecs::World;

/// A named scene containing its own ECS world.
pub struct Scene {
    pub name: String,
    pub world: World,
    pub active: bool,
    pub loaded: bool,
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("name", &self.name)
            .field("active", &self.active)
            .field("loaded", &self.loaded)
            .finish_non_exhaustive()
    }
}

impl Scene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            world: World::new(),
            active: true,
            loaded: true,
        }
    }
}

/// Manager for multiple additive scenes.
pub struct SceneManager {
    pub scenes: Vec<Scene>,
    pub main_scene: usize,
}

impl std::fmt::Debug for SceneManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneManager")
            .field("scenes", &self.scenes.len())
            .field("main_scene", &self.main_scene)
            .finish()
    }
}

impl SceneManager {
    pub fn new(main_scene_name: impl Into<String>) -> Self {
        Self {
            scenes: vec![Scene::new(main_scene_name)],
            main_scene: 0,
        }
    }

    pub fn main(&self) -> &Scene {
        &self.scenes[self.main_scene]
    }

    pub fn main_mut(&mut self) -> &mut Scene {
        &mut self.scenes[self.main_scene]
    }

    /// Additively load a sub-scene.
    pub fn load_scene(&mut self, name: impl Into<String>) -> usize {
        let idx = self.scenes.len();
        self.scenes.push(Scene::new(name));
        idx
    }

    pub fn unload_scene(&mut self, index: usize) {
        if let Some(scene) = self.scenes.get_mut(index) {
            scene.loaded = false;
            scene.active = false;
        }
    }

    pub fn set_active(&mut self, index: usize, active: bool) {
        if let Some(scene) = self.scenes.get_mut(index) {
            scene.active = active;
        }
    }

    pub fn active_scenes(&self) -> Vec<&Scene> {
        self.scenes.iter().filter(|s| s.active && s.loaded).collect()
    }

    pub fn active_scenes_mut(&mut self) -> Vec<&mut Scene> {
        self.scenes.iter_mut().filter(|s| s.active && s.loaded).collect()
    }
}
