//! Editor plugin architecture: register panels, tools, and gizmos.

use std::any::Any;

/// A unique identifier for an editor panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanelId(pub &'static str);

/// A unique identifier for an editor tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToolId(pub &'static str);

/// Trait for editor panels that can be registered at runtime.
pub trait EditorPanel: Send + Sync {
    fn id(&self) -> PanelId;
    fn title(&self) -> &str;
    fn draw(&mut self, ctx: &mut dyn Any);
}

/// Trait for editor tools (select, move, brush, etc.).
pub trait EditorTool: Send + Sync {
    fn id(&self) -> ToolId;
    fn name(&self) -> &str;
    fn activate(&mut self);
    fn deactivate(&mut self);
}

/// Registry for editor extensions.
pub struct PluginRegistry {
    pub panels: Vec<Box<dyn EditorPanel>>,
    pub tools: Vec<Box<dyn EditorTool>>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self {
            panels: Vec::new(),
            tools: Vec::new(),
        }
    }
}

impl std::fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRegistry")
            .field("panels", &self.panels.len())
            .field("tools", &self.tools.len())
            .finish()
    }
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_panel(&mut self, panel: Box<dyn EditorPanel>) {
        self.panels.push(panel);
    }

    pub fn register_tool(&mut self, tool: Box<dyn EditorTool>) {
        self.tools.push(tool);
    }

    pub fn find_panel(&self, id: PanelId) -> Option<&dyn EditorPanel> {
        self.panels.iter().find(|p| p.id() == id).map(|p| p.as_ref())
    }

    pub fn find_panel_mut(&mut self, id: PanelId) -> Option<&mut (dyn EditorPanel + '_)> {
        for panel in &mut self.panels {
            if panel.id() == id {
                return Some(panel.as_mut());
            }
        }
        None
    }

    pub fn find_tool(&self, id: ToolId) -> Option<&dyn EditorTool> {
        self.tools.iter().find(|t| t.id() == id).map(|t| t.as_ref())
    }
}
