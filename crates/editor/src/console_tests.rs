//! Tests for console state and log filtering.

use crate::console::{LogLevel, ConsoleEntry, ConsoleState};

#[test]
fn log_level_ordering() {
    assert!(LogLevel::Error > LogLevel::Warning);
    assert!(LogLevel::Warning > LogLevel::Info);
    assert!(LogLevel::Info > LogLevel::Debug);
}

#[test]
fn log_level_default() {
    assert_eq!(LogLevel::default(), LogLevel::Debug);
}

#[test]
fn console_state_new() {
    let cs = ConsoleState::new(100);
    assert_eq!(cs.max_entries, 100);
    assert!(cs.entries.is_empty());
    assert!(cs.history.is_empty());
    assert_eq!(cs.filter_level, LogLevel::Debug);
}

#[test]
fn console_state_default() {
    let cs: ConsoleState = Default::default();
    assert!(cs.entries.is_empty());
}

#[test]
fn console_log_adds_entry() {
    let mut cs = ConsoleState::new(100);
    cs.log(LogLevel::Info, "hello", 1.0);
    assert_eq!(cs.entries.len(), 1);
    assert_eq!(cs.entries[0].message, "hello");
    assert_eq!(cs.entries[0].level, LogLevel::Info);
}

#[test]
fn console_log_max_entries_eviction() {
    let mut cs = ConsoleState::new(2);
    cs.log(LogLevel::Info, "a", 1.0);
    cs.log(LogLevel::Info, "b", 2.0);
    cs.log(LogLevel::Info, "c", 3.0);
    assert_eq!(cs.entries.len(), 2);
    assert_eq!(cs.entries[0].message, "b");
    assert_eq!(cs.entries[1].message, "c");
}

#[test]
fn console_filtered_entries_by_level() {
    let mut cs = ConsoleState::new(100);
    cs.log(LogLevel::Debug, "dbg", 1.0);
    cs.log(LogLevel::Info, "inf", 2.0);
    cs.log(LogLevel::Warning, "wrn", 3.0);
    cs.filter_level = LogLevel::Warning;
    let filtered = cs.filtered_entries();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].message, "wrn");
}

#[test]
fn console_filtered_entries_by_text() {
    let mut cs = ConsoleState::new(100);
    cs.log(LogLevel::Info, "alpha", 1.0);
    cs.log(LogLevel::Info, "beta", 2.0);
    cs.filter_text = "alp".to_string();
    let filtered = cs.filtered_entries();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].message, "alpha");
}

#[test]
fn console_submit_command() {
    let mut cs = ConsoleState::new(100);
    cs.input_text = "  hello world  ".to_string();
    cs.submit_command();
    assert_eq!(cs.history.len(), 1);
    assert_eq!(cs.history[0], "hello world");
    assert!(cs.input_text.is_empty());
}

#[test]
fn console_submit_empty_command() {
    let mut cs = ConsoleState::new(100);
    cs.input_text = "   ".to_string();
    cs.submit_command();
    assert!(cs.history.is_empty());
}

#[test]
fn console_history_up_down() {
    let mut cs = ConsoleState::new(100);
    cs.history.push("first".to_string());
    cs.history.push("second".to_string());

    cs.history_up();
    assert_eq!(cs.input_text, "second");
    assert_eq!(cs.history_index, Some(1));

    cs.history_up();
    assert_eq!(cs.input_text, "first");
    assert_eq!(cs.history_index, Some(0));

    cs.history_down();
    assert_eq!(cs.input_text, "second");
    assert_eq!(cs.history_index, Some(1));
}
