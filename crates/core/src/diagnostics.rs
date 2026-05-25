use std::str::FromStr;

use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::log_capture::LogCaptureLayer;
use crate::LogBuffer;

/// Log level configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn as_tracing_level(&self) -> tracing::Level {
        match self {
            LogLevel::Error => tracing::Level::ERROR,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Trace => tracing::Level::TRACE,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
}

impl FromStr for LogLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(LogLevel::Error),
            "warn" => Ok(LogLevel::Warn),
            "info" => Ok(LogLevel::Info),
            "debug" => Ok(LogLevel::Debug),
            "trace" => Ok(LogLevel::Trace),
            _ => Err(format!("unknown log level: {s}")),
        }
    }
}

/// Configuration for the logging/diagnostics system.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Global log level filter.
    pub level: LogLevel,
    /// Per-crate log level overrides (e.g., "rustix_render=debug").
    pub crate_filters: Vec<String>,
    /// Enable JSON logging (for production).
    pub json: bool,
    /// Include thread IDs in log output.
    pub thread_ids: bool,
    /// Include target module path.
    pub targets: bool,
    /// Enable Tracy integration.
    pub tracy_enabled: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            crate_filters: Vec::new(),
            json: false,
            thread_ids: true,
            targets: true,
            tracy_enabled: cfg!(feature = "profiling"),
        }
    }
}

/// Initialize the global tracing subscriber with the given configuration.
/// Should be called once at engine startup.
pub fn init_logging(config: &LogConfig) {
    init_logging_with_capture(config, None)
}

/// Initialize the global tracing subscriber with a log capture layer.
pub fn init_logging_with_capture(config: &LogConfig, capture: Option<std::sync::Arc<LogBuffer>>) {
    let filter_str = build_filter_string(config);
    let env_filter = EnvFilter::try_from(filter_str).unwrap_or_else(|_| EnvFilter::default());

    if let Some(buf) = capture {
        let subscriber = tracing_subscriber::fmt()
            .with_thread_ids(config.thread_ids)
            .with_target(config.targets)
            .with_env_filter(env_filter)
            .finish()
            .with(LogCaptureLayer::new(buf));

        #[allow(unused_must_use)]
        {
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| tracing::warn!("logging already initialized: {e}"))
                .ok();
        }
    } else {
        let subscriber = tracing_subscriber::fmt()
            .with_thread_ids(config.thread_ids)
            .with_target(config.targets)
            .with_env_filter(env_filter)
            .finish();

        #[allow(unused_must_use)]
        {
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| tracing::warn!("logging already initialized: {e}"))
                .ok();
        }
    }

    // Tracy client initialization
    #[cfg(feature = "profiling")]
    if config.tracy_enabled {
        tracy_client::Client::start();
        tracing::info!("Tracy profiling enabled");
    }

    tracing::info!(
        level = %config.level.as_str(),
        "logging initialized"
    );
}

/// Build an env-filter string from the log config.
fn build_filter_string(config: &LogConfig) -> String {
    let mut filters = vec![config.level.as_str().to_string()];
    filters.extend(config.crate_filters.clone());
    filters.join(",")
}

/// A simple scoped profiling guard.
/// When the profiling feature is enabled, it emits a Tracy zone.
/// Otherwise, it's a no-op.
#[macro_export]
macro_rules! profile_scope {
    ($name:literal $(, $color:expr)?) => {
        #[cfg(feature = "profiling")]
        let _tracy_guard = tracy_client::span!($name);
        #[cfg(not(feature = "profiling"))]
        let _tracy_guard = ();
    };
}

/// Frame marker for profiler.
#[macro_export]
macro_rules! profile_frame {
    () => {
        #[cfg(feature = "profiling")]
        tracy_client::frame_marker!();
    };
}
