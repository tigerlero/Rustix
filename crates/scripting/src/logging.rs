//! Logging bridge from scripts to Rust tracing.

/// Log a message at info level from a script.
pub fn script_log_info(message: &str) {
    tracing::info!(target: "script", "{}", message);
}

/// Log a message at warn level from a script.
pub fn script_log_warn(message: &str) {
    tracing::warn!(target: "script", "{}", message);
}

/// Log a message at error level from a script.
pub fn script_log_error(message: &str) {
    tracing::error!(target: "script", "{}", message);
}

/// Log a message at debug level from a script.
pub fn script_log_debug(message: &str) {
    tracing::debug!(target: "script", "{}", message);
}
