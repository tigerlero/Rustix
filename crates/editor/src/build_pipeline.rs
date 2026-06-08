//! Build & deploy pipeline: cook assets, package executable.

use std::path::PathBuf;

/// Target platform for builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTarget {
    Windows,
    Linux,
    MacOS,
    WebAssembly,
}

/// Build configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildConfig {
    pub target: BuildTarget,
    pub profile: BuildProfile,
    pub output_dir: PathBuf,
    pub cook_assets: bool,
    pub compress_textures: bool,
    pub strip_debug: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            target: BuildTarget::Linux,
            profile: BuildProfile::Release,
            output_dir: PathBuf::from("build"),
            cook_assets: true,
            compress_textures: true,
            strip_debug: true,
        }
    }
}

/// Optimization profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    Debug,
    Release,
    Shipping,
}

/// Build pipeline state and progress tracking.
#[derive(Debug, Clone, Default)]
pub struct BuildPipeline {
    pub config: BuildConfig,
    pub in_progress: bool,
    pub current_step: String,
    pub progress_percent: f32,
    pub logs: Vec<String>,
    pub last_error: Option<String>,
}

impl BuildPipeline {
    pub fn new(config: BuildConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    pub fn start(&mut self) {
        self.in_progress = true;
        self.progress_percent = 0.0;
        self.current_step = "Starting build...".to_string();
        self.logs.clear();
        self.last_error = None;
    }

    pub fn set_step(&mut self, step: impl Into<String>, progress: f32) {
        self.current_step = step.into();
        self.progress_percent = progress.clamp(0.0, 100.0);
    }

    pub fn log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.last_error = Some(message.into());
        self.in_progress = false;
    }

    pub fn finish(&mut self) {
        self.in_progress = false;
        self.progress_percent = 100.0;
        self.current_step = "Build complete".to_string();
    }
}
