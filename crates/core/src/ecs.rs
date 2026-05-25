pub use hecs::{
    self, Entity, World as EcsWorld, View,
    Query, With, Without,
    DynamicBundle, EntityBuilder, CommandBuffer,
    Component,
};

/// A stage label identifies when a system should execute during the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StageLabel {
    First,
    BeforeFixedUpdate,
    FixedUpdate,
    AfterFixedUpdate,
    BeforeUpdate,
    Update,
    AfterUpdate,
    BeforeRender,
    Render,
    AfterRender,
    Last,
}

impl StageLabel {
    pub fn order(&self) -> u32 {
        match self {
            StageLabel::First => 0,
            StageLabel::BeforeFixedUpdate => 1,
            StageLabel::FixedUpdate => 2,
            StageLabel::AfterFixedUpdate => 3,
            StageLabel::BeforeUpdate => 4,
            StageLabel::Update => 5,
            StageLabel::AfterUpdate => 6,
            StageLabel::BeforeRender => 7,
            StageLabel::Render => 8,
            StageLabel::AfterRender => 9,
            StageLabel::Last => 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SystemId(pub u64);

pub struct BoxedSystem {
    pub id: SystemId,
    pub name: &'static str,
    pub stage: StageLabel,
    pub func: Box<dyn FnMut(&mut EcsWorld) + Send>,
}

impl BoxedSystem {
    pub fn new(
        id: SystemId,
        name: &'static str,
        stage: StageLabel,
        func: impl FnMut(&mut EcsWorld) + Send + 'static,
    ) -> Self {
        Self {
            id,
            name,
            stage,
            func: Box::new(func),
        }
    }

    pub fn run(&mut self, world: &mut EcsWorld) {
        (self.func)(world);
    }
}

pub struct Schedule {
    stages: std::collections::HashMap<StageLabel, Vec<BoxedSystem>>,
    order: Vec<StageLabel>,
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            stages: std::collections::HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn add_system(&mut self, system: BoxedSystem) {
        let stage = system.stage;
        if !self.stages.contains_key(&stage) {
            self.order.push(stage);
            self.order.sort_by_key(|s| s.order());
        }
        self.stages.entry(stage).or_default().push(system);
    }

    pub fn run_stage(&mut self, world: &mut EcsWorld, stage: &StageLabel) {
        if let Some(systems) = self.stages.get_mut(stage) {
            for system in systems.iter_mut() {
                system.run(world);
            }
        }
    }

    pub fn run_all(&mut self, world: &mut EcsWorld) {
        let stages = self.order.clone();
        for stage in &stages {
            self.run_stage(world, stage);
        }
    }

    pub fn stages(&self) -> &[StageLabel] {
        &self.order
    }
}

pub struct SystemIdGenerator {
    next_id: u64,
}

impl SystemIdGenerator {
    pub const fn new() -> Self {
        Self { next_id: 0 }
    }

    pub fn generate(&mut self) -> SystemId {
        let id = SystemId(self.next_id);
        self.next_id += 1;
        id
    }
}
