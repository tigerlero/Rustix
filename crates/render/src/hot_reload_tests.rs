//! Tests for shader hot reload watcher.

use crate::hot_reload::ShaderHotReloader;

#[test]
fn shader_hot_reloader_new() {
    let reloader = ShaderHotReloader::new();
    // If no shader directories exist, creation should still succeed
    assert!(reloader.is_ok());
}

#[test]
fn shader_hot_reloader_take_events_empty() {
    let reloader = ShaderHotReloader::new().unwrap();
    let events = reloader.take_events();
    assert!(events.is_empty());
}
