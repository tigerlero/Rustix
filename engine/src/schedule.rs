use std::collections::HashMap;

use rustix_core::ecs::{BoxedSystem, EcsWorld, StageLabel, SystemId, SystemIdGenerator};

/// Manages the ordered execution of systems across stages.
///
/// Systems are grouped into stages (FixedUpdate, Update, Render, etc.)
/// and executed in order. Within each stage, systems are also ordered.
pub struct Schedule {
    stages: HashMap<StageLabel, Vec<BoxedSystem>>,
    order: Vec<StageLabel>,
    id_gen: SystemIdGenerator,
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            stages: HashMap::new(),
            order: Vec::new(),
            id_gen: SystemIdGenerator::new(),
        }
    }

    /// Register a system to run at the given stage.
    pub fn add_system(
        &mut self,
        name: &'static str,
        stage: StageLabel,
        func: impl FnMut(&mut EcsWorld) + Send + 'static,
    ) -> SystemId {
        let id = self.id_gen.generate();
        let system = BoxedSystem::new(id, name, stage, func);
        self.add_boxed(system);
        id
    }

    /// Register a boxed system.
    pub fn add_boxed(&mut self, system: BoxedSystem) -> SystemId {
        let id = system.id;
        let stage = system.stage;

        // Ensure stage ordering
            if !self.stages.contains_key(&stage) {
                let pos = self.order.binary_search_by_key(&stage.order(), |s| s.order());
                let idx = match pos {
                    Ok(p) | Err(p) => p,
                };
                self.order.insert(idx, stage);
            }

        self.stages.entry(stage).or_default().push(system);
        id
    }

    /// Run all systems in a specific stage.
    pub fn run_stage(&mut self, world: &mut EcsWorld, stage: &StageLabel) {
        let Some(systems) = self.stages.get_mut(stage) else {
            return;
        };
        for system in systems.iter_mut() {
            system.run(world);
        }
    }

    /// Run all stages in order.
    pub fn run_all(&mut self, world: &mut EcsWorld) {
        let stages = self.order.clone();
        for stage in &stages {
            self.run_stage(world, stage);
        }
    }

    /// Run fixed-update stages. Called at a fixed timestep (e.g., 120 Hz).
    pub fn run_fixed_update(&mut self, world: &mut EcsWorld) {
        self.run_stage(world, &StageLabel::FixedUpdate);
        self.run_stage(world, &StageLabel::AfterFixedUpdate);
    }

    /// Run variable-update stages. Called every frame.
    pub fn run_update(&mut self, world: &mut EcsWorld) {
        self.run_stage(world, &StageLabel::First);
        self.run_stage(world, &StageLabel::BeforeUpdate);
        self.run_stage(world, &StageLabel::Update);
        self.run_stage(world, &StageLabel::AfterUpdate);
        self.run_stage(world, &StageLabel::BeforeRender);
    }

    /// Run render stage.
    pub fn run_render(&mut self, world: &mut EcsWorld) {
        self.run_stage(world, &StageLabel::Render);
        self.run_stage(world, &StageLabel::AfterRender);
        self.run_stage(world, &StageLabel::Last);
    }

    /// Returns all registered stage labels.
    pub fn stages(&self) -> &[StageLabel] {
        &self.order
    }

    /// Returns the number of registered systems.
    pub fn system_count(&self) -> usize {
        self.stages.values().map(|v| v.len()).sum()
    }
}
