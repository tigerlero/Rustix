use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;

use crate::log_capture::LogCaptureLayer;
use crate::LogBuffer;

/// A `tracing` layer that writes each log record as a JSON line to a file.
///
/// # Example
///
/// ```rust,no_run
/// use rustix_core::diagnostics::JsonFileLayer;
/// use tracing_subscriber::layer::SubscriberExt;
///
/// let json_layer = JsonFileLayer::new("logs/app.jsonl", 10 * 1024 * 1024, 3).unwrap();
/// let subscriber = tracing_subscriber::registry().with(json_layer);
/// tracing::subscriber::set_global_default(subscriber).ok();
///
/// tracing::info!(user = "alice", "login succeeded");
/// // logs/app.jsonl now contains one JSON line per event.
/// ```
pub struct JsonFileLayer {
    file: Mutex<std::fs::File>,
    path: std::path::PathBuf,
    max_size_bytes: u64,
    max_backups: usize,
    current_size: std::sync::atomic::AtomicU64,
}

impl JsonFileLayer {
    /// Open (create / append) the given path for JSON log output.
    /// `max_size_bytes`: rotate when file exceeds this size (0 = never rotate).
    /// `max_backups`: number of backup files to keep.
    pub fn new(
        path: impl AsRef<std::path::Path>,
        max_size_bytes: u64,
        max_backups: usize,
    ) -> Result<Self, std::io::Error> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let current_size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            file: Mutex::new(file),
            path: path.as_ref().to_path_buf(),
            max_size_bytes,
            max_backups,
            current_size: std::sync::atomic::AtomicU64::new(current_size),
        })
    }

    /// Rotate the current file: close it, rename to `path.N`, and reopen a
    /// fresh file.  If `max_backups` is reached the oldest backup is deleted.
    pub fn rotate(&self, path: &std::path::Path, max_backups: usize) -> Result<(), std::io::Error> {
        let mut file = self.file.lock().unwrap();
        file.flush()?;
        drop(file);

        // Shift backups: N-1 -> N, N-2 -> N-1, ...
        for i in (1..max_backups).rev() {
            let src = path.with_extension(format!("jsonl.{}", i - 1));
            let dst = path.with_extension(format!("jsonl.{}", i));
            if src.exists() {
                std::fs::rename(&src, &dst)?;
            }
        }

        // Rotate current -> .0
        let backup0 = path.with_extension("jsonl.0");
        std::fs::rename(path, &backup0)?;

        // Reopen fresh file
        let new_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        *self.file.lock().unwrap() = new_file;
        Ok(())
    }
}

impl<S> Layer<S> for JsonFileLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut buf = String::new();

        // Build a JSON object manually so we don't need extra features.
        let meta = event.metadata();
        let level = meta.level().as_str();
        let target = meta.target();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        buf.push('{');
        buf.push_str(&format!(r#""timestamp":{timestamp},"#));
        buf.push_str(&format!(r#""level":"{level}","#));
        buf.push_str(&format!(r#""target":"{target}","#));

        // Write the message + fields
        let mut visitor = JsonVisitor::new(&mut buf);
        event.record(&mut visitor);

        // If no message field, write an empty one
        if !visitor.has_message {
            buf.push_str(r#""message":"""#);
        }

        // Span context (current span + parents)
        let mut spans = Vec::new();
        if let Some(span) = ctx.lookup_current() {
            spans.push(span.name().to_string());
            let mut current = span.parent();
            while let Some(s) = current {
                spans.push(s.name().to_string());
                current = s.parent();
            }
        }
        if !spans.is_empty() {
            spans.reverse();
            let span_str = spans.join(">");
            buf.push_str(&format!(r#","spans":"{span_str}""#));
        }

        buf.push_str("}\n");

        let bytes = buf.as_bytes();
        if let Ok(mut f) = self.file.lock() {
            let _ = f.write_all(bytes);
        }

        if self.max_size_bytes > 0 {
            let new_size = self.current_size.fetch_add(bytes.len() as u64, std::sync::atomic::Ordering::Relaxed) + bytes.len() as u64;
            if new_size >= self.max_size_bytes {
                let _ = self.rotate(&self.path, self.max_backups);
                let fresh_size = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
                self.current_size.store(fresh_size, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}

struct JsonVisitor<'a> {
    buf: &'a mut String,
    first: bool,
    has_message: bool,
}

impl<'a> JsonVisitor<'a> {
    fn new(buf: &'a mut String) -> Self {
        Self {
            buf,
            first: true,
            has_message: false,
        }
    }

    fn write_key_value(&mut self, key: &str, value: &str) {
        if !self.first {
            self.buf.push(',');
        }
        self.first = false;
        let safe = value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
        self.buf.push_str(&format!(r#""{key}":"{safe}""#));
    }
}

impl<'a> tracing::field::Visit for JsonVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        let key = if name == "message" {
            self.has_message = true;
            "message"
        } else {
            name
        };
        let val = format!("{:?}", value);
        self.write_key_value(key, &val);
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if !self.first {
            self.buf.push(',');
        }
        self.first = false;
        self.buf.push_str(&format!(r#""{}":{}"#, field.name(), value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if !self.first {
            self.buf.push(',');
        }
        self.first = false;
        self.buf.push_str(&format!(r#""{}":{}"#, field.name(), value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if !self.first {
            self.buf.push(',');
        }
        self.first = false;
        self.buf.push_str(&format!(r#""{}":{}"#, field.name(), value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        if !self.first {
            self.buf.push(',');
        }
        self.first = false;
        self.buf.push_str(&format!(r#""{}":{}"#, field.name(), value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let name = field.name();
        let key = if name == "message" {
            self.has_message = true;
            "message"
        } else {
            name
        };
        self.write_key_value(key, value);
    }
}

// ------------------------------------------------------------------

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
    /// Enable JSON console logging (for production).
    pub json: bool,
    /// Path to a JSON Lines log file.  `None` disables file output.
    pub json_file_path: Option<PathBuf>,
    /// Max file size in MB before rotating the JSON log.  0 = no rotation.
    pub json_max_size_mb: u32,
    /// Number of backup JSON log files to keep.
    pub json_max_backups: usize,
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
            json_file_path: None,
            json_max_size_mb: 10,
            json_max_backups: 3,
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

    let fmt_layer = tracing_subscriber::fmt()
        .with_thread_ids(config.thread_ids)
        .with_target(config.targets)
        .with_env_filter(env_filter);

    if config.json {
        let subscriber = fmt_layer
            .json()
            .finish();

        let _ = tracing::subscriber::set_global_default(subscriber);
    } else if let Some(path) = &config.json_file_path {
        let max_size = (config.json_max_size_mb as u64) * 1024 * 1024;
        match JsonFileLayer::new(path, max_size, config.json_max_backups) {
            Ok(json_layer) => {
                let subscriber = fmt_layer.finish().with(json_layer);
                if let Some(buf) = capture {
                    let subscriber = subscriber.with(LogCaptureLayer::new(buf));
                    let _ = tracing::subscriber::set_global_default(subscriber);
                } else {
                    let _ = tracing::subscriber::set_global_default(subscriber);
                }
            }
            Err(e) => {
                let subscriber = fmt_layer.finish();
                let _ = tracing::subscriber::set_global_default(subscriber);
                tracing::warn!("failed to open JSON log file {path:?}: {e}");
            }
        }
    } else if let Some(buf) = capture {
        let subscriber = fmt_layer
            .finish()
            .with(LogCaptureLayer::new(buf));
        let _ = tracing::subscriber::set_global_default(subscriber);
    } else {
        let subscriber = fmt_layer.finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn temp_jsonl(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("rustix_jsonlog_test_{}_{}", std::process::id(), name));
        p
    }

    #[test]
    fn json_file_layer_creates_file() {
        let path = temp_jsonl("create");
        let _ = std::fs::remove_file(&path);
        let layer = JsonFileLayer::new(&path, 0, 0).unwrap();
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn json_file_layer_appends_lines() {
        let path = temp_jsonl("append");
        let _ = std::fs::remove_file(&path);
        let layer = JsonFileLayer::new(&path, 0, 0).unwrap();

        // Write two events via the tracing subscriber
        use tracing_subscriber::layer::SubscriberExt;

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(score = 42, "player scored");
            tracing::warn!(target = "test", "something happened");
        });

        let mut contents = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();

        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2, "expected 2 JSON lines, got {lines:?}");

        // Check first line contains expected fields
        assert!(lines[0].contains("\"timestamp\":"));
        assert!(lines[0].contains("\"level\":\"INFO\""));
        assert!(lines[0].contains("\"message\":\"player scored\""));
        assert!(lines[0].contains("\"score\":42"));

        // Check second line
        assert!(lines[1].contains("\"level\":\"WARN\""));
        assert!(lines[1].contains("\"message\":\"something happened\""));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn json_file_layer_rotation() {
        let path = temp_jsonl("rotate");
        let _ = std::fs::remove_file(&path);
        let backup = path.with_extension("jsonl.0");
        let _ = std::fs::remove_file(&backup);

        let layer = JsonFileLayer::new(&path, 0, 3).unwrap();
        std::fs::write(&path, "old data\n").unwrap();

        layer.rotate(&path, 3).unwrap();

        assert!(backup.exists(), "backup file should exist after rotation");
        let backup_contents = std::fs::read_to_string(&backup).unwrap();
        assert_eq!(backup_contents, "old data\n");

        // Fresh file should be empty
        let fresh = std::fs::read_to_string(&path).unwrap();
        assert!(fresh.is_empty());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);
    }

    #[test]
    fn json_file_layer_escape_quotes() {
        let path = temp_jsonl("escape");
        let _ = std::fs::remove_file(&path);
        let layer = JsonFileLayer::new(&path, 0, 0).unwrap();

        use tracing_subscriber::layer::SubscriberExt;
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(name = "he said \"hello\"", "message with \"quotes\"");
        });

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains(r#"\"hello\""#));
        assert!(contents.contains(r#"\"quotes\""#));
        // Verify the unescaped version does NOT appear
        assert!(!contents.contains("message with \"hello\""));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn json_file_layer_escaped_newlines() {
        let path = temp_jsonl("newline");
        let _ = std::fs::remove_file(&path);
        let layer = JsonFileLayer::new(&path, 0, 0).unwrap();

        use tracing_subscriber::layer::SubscriberExt;
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("line1\nline2");
        });

        let contents = std::fs::read_to_string(&path).unwrap();
        // Should be one single line
        assert_eq!(contents.lines().count(), 1);
        assert!(contents.contains(r#"\n"#));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn json_file_layer_auto_rotate_on_size() {
        let path = temp_jsonl("autorotate");
        let backup = path.with_extension("jsonl.0");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);

        // Set a tiny max size so a single log line triggers rotation
        let layer = JsonFileLayer::new(&path, 1, 2).unwrap();

        use tracing_subscriber::layer::SubscriberExt;
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            // This line will exceed 1 byte and trigger auto-rotation
            tracing::info!("this is a log message that is definitely more than one byte long");
        });

        // The original file should have been rotated to backup
        assert!(backup.exists(), "backup should exist after auto-rotation");
        // Fresh file should exist (may or may not be empty depending on timing)
        assert!(path.exists(), "fresh file should exist after auto-rotation");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);
    }
}
