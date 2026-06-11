use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use notify::Watcher;

/// Watches a project asset directory and reports which files changed.
///
/// Poll `take_events()` each frame; on change, re-import the affected asset(s)
/// and update the in-game registries.
pub struct AssetWatcher {
    _watcher: notify::RecommendedWatcher,
    events: Arc<Mutex<Vec<PathBuf>>>,
    enabled: bool,
}

impl AssetWatcher {
    /// Start watching `watch_dir` recursively.
    pub fn new(watch_dir: &Path) -> Option<Self> {
        if !watch_dir.exists() {
            return None;
        }
        let dir = watch_dir;

        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                use notify::EventKind;
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        let mut vec = events_clone.lock().unwrap();
                        for path in event.paths {
                            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                let ext = ext.to_lowercase();
                                if matches!(ext.as_str(),
                                    "glb" | "gltf" | "obj" | "fbx" |
                                    "png" | "jpg" | "jpeg" | "hdr" | "exr" | "tga" | "bmp" | "webp" | "ktx2" |
                                    "wav" | "ogg" | "mp3" | "flac" | "aac" | "m4a" |
                                    "wgsl" | "glsl" | "vert" | "frag" | "comp"
                                ) {
                                    if !vec.contains(&path) {
                                        vec.push(path);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }).ok()?;

        watcher.watch(dir, notify::RecursiveMode::Recursive).ok()?;

        Some(Self {
            _watcher: watcher,
            events,
            enabled: true,
        })
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Drain all pending asset-change events.
    pub fn take_events(&self) -> Vec<PathBuf> {
        if !self.enabled {
            return Vec::new();
        }
        std::mem::take(&mut self.events.lock().unwrap())
    }
}

/// Re-import a changed asset file and update the relevant registry.
///
/// * `path` — absolute or relative path to the changed asset file.
/// * `renderer` — required for GPU texture creation.
/// * `app` — mutable access to `AppState` registries.
pub fn reload_asset(path: &Path, renderer: &rustix_render::Renderer, app: &mut crate::app_state::AppState) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported")
        .to_string();

    match ext.as_str() {
        "glb" | "gltf" | "obj" | "fbx" => reload_mesh(path, &name, renderer, app),
        "png" | "jpg" | "jpeg" | "hdr" | "exr" | "tga" | "bmp" | "webp" | "ktx2" => {
            reload_texture(path, &name, renderer, app);
        }
        "wav" | "ogg" | "mp3" | "flac" | "aac" | "m4a" => {
            reload_audio(path, &name, app);
        }
        "wgsl" | "glsl" | "vert" | "frag" | "comp" => {
            tracing::info!("shader change detected: {path}", path = path.display());
            // Shader hot-reload is handled by the renderer's ShaderHotReloader.
        }
        _ => {}
    }
}

fn reload_mesh(path: &Path, name: &str, renderer: &rustix_render::Renderer, app: &mut crate::app_state::AppState) {
    if let Ok(data) = rustix_asset::mmap::MappedFile::open(path) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match crate::model_import::import_model(renderer, &data, name, &ext) {
            Ok(result) => {
                for w in &result.validation.warnings {
                    tracing::warn!("mesh hot-reload validation: {}", w);
                }
                app.meshes.insert(name.to_string(), result.mesh);
                tracing::info!("hot-reloaded mesh {name} from {path}", path = path.display());
            }
            Err(e) => tracing::error!("failed to hot-reload mesh from {path}: {e}", path = path.display()),
        }
    } else {
        tracing::error!("failed to read mesh file {path} for hot-reload", path = path.display());
    }
}

fn reload_texture(path: &Path, name: &str, renderer: &rustix_render::Renderer, app: &mut crate::app_state::AppState) {
    if let Ok(data) = rustix_asset::mmap::MappedFile::open(path) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_normal = name.to_lowercase().contains("normal") || name.to_lowercase().contains("nrm");
        let options = crate::texture_import::TextureImportOptions {
            compress: Some(rustix_asset::texture_compress::CompressedBlockFormat::Bc7Unorm),
            generate_mips: true,
            normal_map: is_normal,
            srgb: !is_normal,
        };
        match crate::texture_import::import_texture(renderer, &data, name, &ext, &options) {
            Ok(result) => {
                app.textures.insert(name.to_string(), result.texture);
                tracing::info!("hot-reloaded texture {name} from {path}", path = path.display());
            }
            Err(e) => tracing::error!("failed to hot-reload texture from {path}: {e}", path = path.display()),
        }
    } else {
        tracing::error!("failed to read texture file {path} for hot-reload", path = path.display());
    }
}

fn reload_audio(path: &Path, name: &str, app: &mut crate::app_state::AppState) {
    let is_music = name.to_lowercase().contains("music")
        || name.to_lowercase().contains("bgm")
        || name.to_lowercase().contains("theme");
    let options = crate::audio_import::AudioImportOptions {
        streaming: is_music,
        volume: 1.0,
        looping: is_music,
        spatial_blend: 0.0,
    };
    match crate::audio_import::import_audio(path, name, &options) {
        Ok(result) => {
            app.sounds.insert(name.to_string(), result);
            tracing::info!("hot-reloaded audio {name} from {path}", path = path.display());
        }
        Err(e) => tracing::error!("failed to hot-reload audio from {path}: {e}", path = path.display()),
    }
}
