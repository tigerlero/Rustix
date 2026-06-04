use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use notify::Watcher;

/// Watches the `shaders/` directory and reports which files changed.
///
/// Create one per `Renderer`. Poll `take_events()` each frame; on change,
/// recompile the affected shader(s) and recreate the pipeline(s).
pub struct ShaderHotReloader {
    _watcher: notify::RecommendedWatcher,
    events: Arc<Mutex<Vec<PathBuf>>>,
}

impl ShaderHotReloader {
    /// Start watching `shaders/` under the current working directory.
    pub fn new() -> Result<Self, crate::RenderError> {
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                use notify::EventKind;
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        let mut vec = events_clone.lock().unwrap();
                        for path in event.paths {
                            if path.extension().map_or(false, |e| {
                                let ext = e.to_str().unwrap_or("").to_lowercase();
                                matches!(ext.as_str(), "glsl" | "vert" | "frag" | "comp" | "wgsl")
                            }) {
                                // Deduplicate
                                if !vec.contains(&path) {
                                    vec.push(path);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .map_err(|e| crate::RenderError::ShaderCompile(format!("watch: {e}")))?;

        let paths = ["shaders", "../shaders", "../../shaders"];
        for path in paths {
            let p = std::path::Path::new(path);
            if p.exists() {
                watcher
                    .watch(p, notify::RecursiveMode::NonRecursive)
                    .map_err(|e| crate::RenderError::ShaderCompile(format!("watch {}: {e}", p.display())))?;
            }
        }

        Ok(Self {
            _watcher: watcher,
            events,
        })
    }

    /// Drain all pending shader-change events.
    pub fn take_events(&self) -> Vec<PathBuf> {
        std::mem::take(&mut self.events.lock().unwrap())
    }
}
