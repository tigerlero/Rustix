
use rustix_core::config::EngineConfig;
use rustix_core::diagnostics::{self, LogConfig};
use rustix_core::ecs::EcsWorld;
use rustix_core::job::JobSystem;
use rustix_core::memory::FrameMemory;

use crate::plugin::{AppBuilder, Plugin};
use crate::schedule::Schedule;

pub const FIXED_DT: f32 = 1.0 / 120.0;
pub const FRAME_ALLOCATOR_CAPACITY: usize = 64 * 1024 * 1024;

pub struct App {
    pub world: EcsWorld,
    pub schedule: Schedule,
    pub job_system: JobSystem,
    pub frame_memory: FrameMemory,
    pub config: EngineConfig,
    pub running: bool,
}

impl App {
    pub fn builder() -> AppBuilder {
        AppBuilder::new()
    }

    pub fn new(config: EngineConfig) -> Self {
        let log_config = LogConfig {
            level: config.log_level(),
            crate_filters: config.logging.crate_filters.clone(),
            json: config.logging.json,
            thread_ids: true,
            targets: true,
            tracy_enabled: cfg!(feature = "profiling"),
        };
        diagnostics::init_logging(&log_config);

        tracing::info!("Rustix Engine v{}", env!("CARGO_PKG_VERSION"));

        let job_system = JobSystem::new(&config.jobs)
            .expect("failed to initialize job system");
        tracing::info!(threads = job_system.thread_count(), "job system initialized");

        let frame_memory = FrameMemory::new(FRAME_ALLOCATOR_CAPACITY);

        Self {
            world: EcsWorld::new(),
            schedule: Schedule::new(),
            job_system,
            frame_memory,
            config,
            running: false,
        }
    }

    pub fn register_plugins(&mut self, builder: AppBuilder) {
        for plugin in builder.plugins {
            tracing::debug!(name = plugin.name(), "loading plugin");
            plugin.on_load(&mut self.world);
        }
    }
}
