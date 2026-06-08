use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::diagnostics::LogLevel;
use crate::job::JobSystemConfig;

/// Top-level engine configuration loaded from TOML.
/// Supports layered configs: default → project → user → CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Window configuration.
    #[serde(default)]
    pub window: WindowConfig,
    /// Rendering configuration.
    #[serde(default)]
    pub render: RenderConfig,
    /// Job system configuration.
    #[serde(default)]
    pub jobs: JobSystemConfig,
    /// Logging configuration.
    #[serde(default)]
    pub logging: LogConfigWrapper,
    /// Asset system configuration.
    #[serde(default)]
    pub assets: AssetConfig,
    /// Physics configuration.
    #[serde(default)]
    pub physics: PhysicsConfig,
    /// Audio configuration.
    #[serde(default)]
    pub audio: AudioConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            render: RenderConfig::default(),
            jobs: JobSystemConfig::default(),
            logging: LogConfigWrapper::default(),
            assets: AssetConfig::default(),
            physics: PhysicsConfig::default(),
            audio: AudioConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub vsync: bool,
    /// "wayland" | "x11" | "auto"
    pub backend: String,
    pub monitor: Option<usize>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Rustix Engine".into(),
            width: 1920,
            height: 1080,
            fullscreen: false,
            vsync: false,
            backend: "auto".into(),
            monitor: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    pub preferred_gpu: String,
    pub enable_validation: bool,
    pub frame_count: u32,
    pub shader_cache_path: PathBuf,
    pub pipeline_cache_path: PathBuf,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            preferred_gpu: "high_performance".into(),
            enable_validation: cfg!(debug_assertions),
            frame_count: 3,
            shader_cache_path: PathBuf::from("cache/shaders"),
            pipeline_cache_path: PathBuf::from("cache/pipelines"),
        }
    }
}

// Wrapper for LogConfig so we can deserialize it nicely
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfigWrapper {
    pub level: String,
    pub json: bool,
    #[serde(default)]
    pub crate_filters: Vec<String>,
}

impl Default for LogConfigWrapper {
    fn default() -> Self {
        Self {
            level: "info".into(),
            json: false,
            crate_filters: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetConfig {
    pub root: PathBuf,
    pub cache_path: PathBuf,
    pub hot_reload: bool,
    pub async_loading: bool,
    pub streaming_enabled: bool,
}

impl Default for AssetConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("assets"),
            cache_path: PathBuf::from("cache/assets"),
            hot_reload: cfg!(debug_assertions),
            async_loading: true,
            streaming_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsConfig {
    pub gravity: [f32; 3],
    pub substeps: u32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: [0.0, -9.81, 0.0],
            substeps: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub max_sources: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            buffer_size: 256,
            max_sources: 32,
        }
    }
}

impl EngineConfig {
    /// Load from a TOML file path.
    pub fn from_file(path: &std::path::Path) -> Result<Self, ConfigError> {
        let contents =
            std::fs::read_to_string(path).map_err(|e| ConfigError::Io(path.to_path_buf(), e))?;
        toml::from_str(&contents).map_err(|e| ConfigError::Parse(path.to_path_buf(), e))
    }

    /// Load from a TOML string.
    pub fn from_str(s: &str) -> Result<Self, ConfigError> {
        toml::from_str(s).map_err(|e| ConfigError::ParseString(e))
    }

    /// Merge another config on top of this one.
    /// Fields in `other` that are non-default override ours.
    pub fn merge(&mut self, _other: &EngineConfig) {
        // Simple field-by-field merge.
        // In a more sophisticated implementation, we'd use a deep merge.
        // For now, just take the other config's fields if they differ from default.
        let default = EngineConfig::default();

        if _other.window.title != default.window.title {
            self.window = _other.window.clone();
        }
        if _other.render.enable_validation != default.render.enable_validation {
            self.render = _other.render.clone();
        }
        if _other.jobs.thread_count != default.jobs.thread_count {
            self.jobs = _other.jobs.clone();
        }
        if _other.assets.root != default.assets.root {
            self.assets = _other.assets.clone();
        }
    }

    /// Get the log level from the config.
    pub fn log_level(&self) -> LogLevel {
        self.logging
            .level
            .parse()
            .unwrap_or(LogLevel::Info)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(PathBuf, std::io::Error),
    Parse(PathBuf, toml::de::Error),
    ParseString(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(path, e) => write!(f, "failed to read config '{path:?}': {e}"),
            ConfigError::Parse(path, e) => write!(f, "failed to parse config '{path:?}': {e}"),
            ConfigError::ParseString(e) => write!(f, "failed to parse config string: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Try to find and load the engine config from standard locations.
pub fn find_and_load_config() -> EngineConfig {
    let mut config = EngineConfig::default();

    // Look for config in order: project root, user config dir, XDG
    let candidates = [
        PathBuf::from("rustix.toml"),
        PathBuf::from("engine.toml"),
        dirs::config_dir()
            .map(|d| d.join("rustix").join("config.toml"))
            .unwrap_or_default(),
        dirs::config_dir()
            .map(|d| d.join("rustix").join("engine.toml"))
            .unwrap_or_default(),
    ];

    for path in &candidates {
        if path.exists() && path.is_file() {
            match EngineConfig::from_file(path) {
                Ok(loaded) => {
                    config.merge(&loaded);
                    tracing::info!(?path, "loaded config");
                }
                Err(e) => {
                    tracing::warn!(?path, error = %e, "failed to load config");
                }
            }
        }
    }

    config
}

// ------------------------------------------------------------------
// Runtime config reload
// ------------------------------------------------------------------

use std::time::{Duration, Instant, SystemTime};

/// Watches a TOML config file and invokes a callback when it changes.
///
/// Uses lightweight polling (file mtime) rather than OS-level file watching
/// so it works on every platform without extra dependencies.
///
/// # Example
///
/// ```rust,no_run
/// use std::time::Duration;
/// use rustix_core::config::{ConfigWatcher, EngineConfig};
///
/// let mut watcher = ConfigWatcher::new("engine.toml", |cfg: &EngineConfig| {
///     println!("config reloaded: {:?}", cfg.window.title);
/// });
/// watcher.set_interval(Duration::from_secs(2));
///
/// // Call once per frame / tick:
/// let _ = watcher.update();
/// ```
pub struct ConfigWatcher<F> {
    path: PathBuf,
    last_mtime: Option<SystemTime>,
    last_check: Instant,
    interval: Duration,
    on_change: F,
}

impl<F: FnMut(&EngineConfig)> ConfigWatcher<F> {
    /// Create a new watcher for `path`.  The first call to `update()` will
    /// always load the file so the callback receives the initial config.
    pub fn new(path: impl AsRef<std::path::Path>, on_change: F) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            last_mtime: None,
            last_check: Instant::now() - Duration::from_secs(3600), // force first check
            interval: Duration::from_secs(1),
            on_change,
        }
    }

    /// Set the polling interval (default: 1 second).
    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    /// Check whether the file has changed since the last call.
    ///
    /// Returns `Ok(true)` if the file was reloaded, `Ok(false)` if no change
    /// or not enough time has elapsed, `Err(ConfigError)` on I/O or parse
    /// failure.
    pub fn update(&mut self) -> Result<bool, ConfigError> {
        let now = Instant::now();
        if now.duration_since(self.last_check) < self.interval {
            return Ok(false);
        }
        self.last_check = now;

        let meta = match std::fs::metadata(&self.path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(ConfigError::Io(self.path.clone(), e)),
        };

        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        if self.last_mtime == Some(mtime) {
            return Ok(false);
        }

        let config = EngineConfig::from_file(&self.path)?;
        self.last_mtime = Some(mtime);
        (self.on_change)(&config);
        Ok(true)
    }

    /// Path being watched.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Force a reload on the next `update()` call, ignoring the interval.
    pub fn request_refresh(&mut self) {
        self.last_check = Instant::now() - self.interval - Duration::from_millis(1);
    }
}
