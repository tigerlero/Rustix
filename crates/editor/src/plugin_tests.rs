//! Tests for plugin registry.

use crate::plugin::*;

struct MockPanel;
impl EditorPanel for MockPanel {
    fn id(&self) -> PanelId { PanelId("mock.panel") }
    fn title(&self) -> &str { "Mock Panel" }
    fn draw(&mut self, _ctx: &mut dyn std::any::Any) {}
}

struct MockTool;
impl EditorTool for MockTool {
    fn id(&self) -> ToolId { ToolId("mock.tool") }
    fn name(&self) -> &str { "Mock Tool" }
    fn activate(&mut self) {}
    fn deactivate(&mut self) {}
}

#[test]
fn panel_id_equality() {
    assert_eq!(PanelId("a"), PanelId("a"));
    assert_ne!(PanelId("a"), PanelId("b"));
}

#[test]
fn tool_id_equality() {
    assert_eq!(ToolId("x"), ToolId("x"));
    assert_ne!(ToolId("x"), ToolId("y"));
}

#[test]
fn plugin_registry_new() {
    let reg = PluginRegistry::new();
    assert!(reg.panels.is_empty());
    assert!(reg.tools.is_empty());
}

#[test]
fn plugin_registry_default() {
    let reg: PluginRegistry = Default::default();
    assert!(reg.panels.is_empty());
}

#[test]
fn plugin_registry_register_panel() {
    let mut reg = PluginRegistry::new();
    reg.register_panel(Box::new(MockPanel));
    assert_eq!(reg.panels.len(), 1);
}

#[test]
fn plugin_registry_register_tool() {
    let mut reg = PluginRegistry::new();
    reg.register_tool(Box::new(MockTool));
    assert_eq!(reg.tools.len(), 1);
}

#[test]
fn plugin_registry_find_panel() {
    let mut reg = PluginRegistry::new();
    reg.register_panel(Box::new(MockPanel));
    let found = reg.find_panel(PanelId("mock.panel"));
    assert!(found.is_some());
    assert_eq!(found.unwrap().title(), "Mock Panel");
}

#[test]
fn plugin_registry_find_panel_missing() {
    let reg = PluginRegistry::new();
    assert!(reg.find_panel(PanelId("missing")).is_none());
}

#[test]
fn plugin_registry_find_panel_mut() {
    let mut reg = PluginRegistry::new();
    reg.register_panel(Box::new(MockPanel));
    let found = reg.find_panel_mut(PanelId("mock.panel"));
    assert!(found.is_some());
}

#[test]
fn plugin_registry_find_tool() {
    let mut reg = PluginRegistry::new();
    reg.register_tool(Box::new(MockTool));
    let found = reg.find_tool(ToolId("mock.tool"));
    assert!(found.is_some());
    assert_eq!(found.unwrap().name(), "Mock Tool");
}

#[test]
fn plugin_registry_find_tool_missing() {
    let reg = PluginRegistry::new();
    assert!(reg.find_tool(ToolId("missing")).is_none());
}
