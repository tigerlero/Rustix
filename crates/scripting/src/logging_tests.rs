//! Tests for script logging bridge.

use crate::logging::*;

#[test]
fn script_log_info_does_not_panic() {
    script_log_info("hello from script");
}

#[test]
fn script_log_warn_does_not_panic() {
    script_log_warn("warning from script");
}

#[test]
fn script_log_error_does_not_panic() {
    script_log_error("error from script");
}

#[test]
fn script_log_debug_does_not_panic() {
    script_log_debug("debug from script");
}
