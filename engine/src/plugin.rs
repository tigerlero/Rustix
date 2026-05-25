use rustix_core::ecs::EcsWorld;

/// A plugin registers systems, resources, and assets with the engine.
///
/// Plugins are the primary mechanism for extending the engine.
/// Each subsystem (render, physics, audio, etc.) is a plugin.
///
/// # Example
///
/// ```ignore
/// struct MyPlugin;
///
/// impl Plugin for MyPlugin {
///     fn name(&self) -> &'static str { "my_plugin" }
///     fn build(&self, app: &mut AppBuilder) {
///         app.add_system(my_system, StageLabel::Update)
///            .insert_resource(MyResource::new());
///     }
/// }
/// ```
pub trait Plugin: Send + Sync + 'static {
    /// Unique name for this plugin (used for debugging and ordering).
    fn name(&self) -> &'static str;

    /// Called during app building. Register systems, resources, and sub-plugins.
    fn build(&self, _app: &mut AppBuilder) {}

    /// Called when the plugin is loaded (after build, before first run).
    fn on_load(&self, _world: &mut EcsWorld) {}

    /// Called when the plugin is unloaded (during shutdown).
    fn on_unload(&self, _world: &mut EcsWorld) {}
}

/// Builder for constructing the application and registering plugins.
pub struct AppBuilder {
    /// Plugins that have been registered.
    pub(crate) plugins: Vec<Box<dyn Plugin>>,
}

impl AppBuilder {
    pub(crate) fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin.
    pub fn add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        let name = plugin.name();
        tracing::debug!(plugin = name, "registering plugin");
        plugin.build(self);
        self.plugins.push(Box::new(plugin));
        self
    }
}
