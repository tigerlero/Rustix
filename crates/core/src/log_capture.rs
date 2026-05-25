use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

use parking_lot::Mutex;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{layer::Context, Layer};

/// A single captured log entry.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: Level,
    pub target: String,
    pub message: String,
    pub timestamp: Instant,
}

/// Ring buffer for log capture.
pub struct LogBuffer {
    entries: Mutex<Vec<LogEntry>>,
    max_entries: usize,
}

impl LogBuffer {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Mutex::new(Vec::with_capacity(max_entries)), max_entries }
    }

    pub fn push(&self, entry: LogEntry) {
        let mut entries = self.entries.lock();
        if entries.len() >= self.max_entries {
            entries.remove(0);
        }
        entries.push(entry);
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries.lock().clone()
    }

    pub fn clear(&self) {
        self.entries.lock().clear();
    }
}

/// Capturing visitor to format event fields.
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, _field: &tracing::field::Field, value: &str) {
        self.message.push_str(value);
    }

    fn record_debug(&mut self, _field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.message.push_str(&format!("{:?}", value));
    }
}

/// Tracing layer that captures log entries to a ring buffer.
#[derive(Clone)]
pub struct LogCaptureLayer {
    buffer: Arc<LogBuffer>,
}

impl LogCaptureLayer {
    pub fn new(buffer: Arc<LogBuffer>) -> Self {
        Self { buffer }
    }
}

impl<S: Subscriber> Layer<S> for LogCaptureLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        
        // Skip tracing internal events
        if metadata.target().starts_with("tracing::") || metadata.target().starts_with("tracing_subscriber::") {
            return;
        }
        
        // Format the event message
        let mut visitor = MessageVisitor { message: String::new() };
        event.record(&mut visitor);
        let message = visitor.message;

        let entry = LogEntry {
            level: *metadata.level(),
            target: metadata.target().to_string(),
            message,
            timestamp: Instant::now(),
        };
        self.buffer.push(entry);
    }
}

/// Global log capture instance.
static LOG_CAPTURE: OnceLock<Arc<LogBuffer>> = OnceLock::new();

/// Initialize the global log capture buffer.
pub fn init_log_capture(max_entries: usize) -> Arc<LogBuffer> {
    LOG_CAPTURE.get_or_init(|| Arc::new(LogBuffer::new(max_entries))).clone()
}

/// Get the global log capture buffer.
pub fn log_buffer() -> Option<Arc<LogBuffer>> {
    LOG_CAPTURE.get().cloned()
}

/// Get all captured log entries.
pub fn get_logs() -> Vec<LogEntry> {
    log_buffer().map(|b| b.entries()).unwrap_or_default()
}

/// Clear the log buffer.
pub fn clear_logs() {
    if let Some(buf) = log_buffer() {
        buf.clear();
    }
}

impl std::fmt::Display for LogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level_str = match self.level {
            Level::ERROR => "ERROR",
            Level::WARN => "WARN",
            Level::INFO => "INFO",
            Level::DEBUG => "DEBUG",
            Level::TRACE => "TRACE",
        };
        write!(f, "[{}] {}: {}", level_str, self.target, self.message)
    }
}