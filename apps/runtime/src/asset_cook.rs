//! Asset cooking pipeline.
//!
//! Strips editor metadata from the project and packs runtime assets into a
//! platform-optimised `.pak` archive.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};

use crate::project::{ProjectInfo, ProjectSettings, ProjectType};
use crate::scene::SceneData;

/// Runtime-facing project data with all editor-only fields removed.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CookedProject {
    pub name: String,
    pub description: String,
    pub created: String,
    pub default_scene: String,
    pub scenes: Vec<String>,
    pub settings: ProjectSettings,
    pub scene: SceneData,
}

impl CookedProject {
    /// Strip editor metadata from a full `ProjectInfo`.
    pub fn from_project(info: &ProjectInfo) -> Self {
        Self {
            name: info.name.clone(),
            description: info.description.clone(),
            created: info.created.clone(),
            default_scene: info.default_scene.clone(),
            scenes: info.scenes.clone(),
            settings: info.settings.clone(),
            scene: info.scene.clone(),
        }
    }
}

/// File extensions that are considered editor-only and should be skipped during cooking.
const EDITOR_EXTENSIONS: &[&str] = &[
    // Thumbnails / caches
    "thumb",
    "cache",
    "tmp",
    "temp",
    // Editor settings
    "importsettings",
    "meta",
    // Backup files
    "bak",
    "backup",
    // IDE / OS files
    "ds_store",
];

/// File names that should never be packed.
const EDITOR_FILE_NAMES: &[&str] = &[
    ".recent_projects.json",
    "recent_projects.json",
];

/// Returns `true` if the given path should be excluded from the cooked archive.
fn is_editor_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    if EDITOR_FILE_NAMES.iter().any(|&excluded| name == excluded) {
        return true;
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        if EDITOR_EXTENSIONS.iter().any(|&e| ext_lower == e) {
            return true;
        }
    }

    // Skip hidden directories (e.g. .git, .idea)
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::Normal(n) if n.to_str().map_or(false, |s| s.starts_with('.'))))
    {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// .pak archive format
//
//   Header (25 bytes)
//       magic:     [u8; 9]  = b"RUSTIXPAK"
//       version:   u32 LE   = 1
//       reserved:  u32 LE   = 0
//       entries:   u32 LE
//
//   Entry table (N * entry_size)
//       path_len:  u32 LE
//       path:      [u8; path_len]  (UTF-8)
//       offset:    u64 LE  (from start of file)
//       size:      u64 LE
//
//   Data (contiguous blob, each entry points into this)
// ---------------------------------------------------------------------------

const PAK_MAGIC: &[u8] = b"RUSTIXPAK";
const PAK_VERSION: u32 = 1;

#[derive(Debug, Clone)]
struct PakEntry {
    path: String,
    offset: u64,
    size: u64,
}

/// Builder for `.pak` archives.
pub struct PakBuilder {
    files: Vec<(String, Vec<u8>)>,
}

impl PakBuilder {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Add a file from memory.
    pub fn add_data(&mut self, path: impl Into<String>, data: Vec<u8>) {
        self.files.push((path.into(), data));
    }

    /// Recursively add all files under `src_dir` that are not editor files.
    /// Each file is stored with a path relative to `src_dir`.
    pub fn add_dir(&mut self, src_dir: &Path) -> io::Result<()> {
        self.add_dir_inner(src_dir, src_dir)
    }

    fn add_dir_inner(&mut self, base: &Path, current: &Path) -> io::Result<()> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            if is_editor_file(&path) {
                continue;
            }
            if path.is_dir() {
                self.add_dir_inner(base, &path)?;
            } else {
                let rel = path.strip_prefix(base).unwrap_or(&path);
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let data = fs::read(&path)?;
                self.add_data(rel_str, data);
            }
        }
        Ok(())
    }

    /// Write the complete archive to `dest`.
    pub fn write(&self, dest: &Path) -> io::Result<()> {
        let mut file = fs::File::create(dest)?;

        // ---- header ----
        file.write_all(PAK_MAGIC)?;
        file.write_all(&PAK_VERSION.to_le_bytes())?;
        file.write_all(&[0u8; 4])?; // reserved
        file.write_all(&(self.files.len() as u32).to_le_bytes())?;

        // Reserve space for entry table
        let header_size = PAK_MAGIC.len() + 4 + 4 + 4; // 9 + 4 + 4 + 4 = 21
        let mut entry_offsets = Vec::with_capacity(self.files.len());

        for (path, data) in &self.files {
            let path_bytes = path.as_bytes();
            let path_len = path_bytes.len() as u32;
            let offset_pos = file.stream_position()?;
            file.write_all(&path_len.to_le_bytes())?;
            file.write_all(path_bytes)?;
            // offset placeholder
            let offset_placeholder = file.stream_position()?;
            file.write_all(&0u64.to_le_bytes())?;
            file.write_all(&(data.len() as u64).to_le_bytes())?;
            entry_offsets.push((offset_pos, offset_placeholder));
        }

        // Write data blobs and backfill offsets
        for ((_, data), (_, offset_placeholder)) in self.files.iter().zip(entry_offsets.iter()) {
            let data_offset = file.stream_position()?;
            file.write_all(data)?;
            // Backfill offset
            file.seek(io::SeekFrom::Start(*offset_placeholder))?;
            file.write_all(&data_offset.to_le_bytes())?;
            file.seek(io::SeekFrom::End(0))?;
        }

        Ok(())
    }
}

/// Read a `.pak` archive created by `PakBuilder`.
pub struct PakArchive {
    entries: HashMap<String, PakEntry>,
    data: Vec<u8>,
}

impl PakArchive {
    /// Load an archive from disk.
    pub fn load(path: &Path) -> io::Result<Self> {
        let mut file = fs::File::open(path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Self::from_bytes(buf)
    }

    /// Parse archive from an in-memory byte vector.
    pub fn from_bytes(buf: Vec<u8>) -> io::Result<Self> {
        if buf.len() < 21 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "archive too small"));
        }
        if &buf[0..9] != PAK_MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad pak magic"));
        }
        let version = u32::from_le_bytes([buf[9], buf[10], buf[11], buf[12]]);
        if version != PAK_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported pak version {version}"),
            ));
        }
        let entry_count = u32::from_le_bytes([buf[17], buf[18], buf[19], buf[20]]) as usize;

        let mut entries = HashMap::with_capacity(entry_count);
        let mut cursor: usize = 21;

        for _ in 0..entry_count {
            if cursor + 4 > buf.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated entry table"));
            }
            let path_len = u32::from_le_bytes([buf[cursor], buf[cursor + 1], buf[cursor + 2], buf[cursor + 3]]) as usize;
            cursor += 4;

            if cursor + path_len + 16 > buf.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated entry table"));
            }
            let path = String::from_utf8_lossy(&buf[cursor..cursor + path_len]).to_string();
            cursor += path_len;

            let offset = u64::from_le_bytes([
                buf[cursor], buf[cursor + 1], buf[cursor + 2], buf[cursor + 3],
                buf[cursor + 4], buf[cursor + 5], buf[cursor + 6], buf[cursor + 7],
            ]);
            cursor += 8;

            let size = u64::from_le_bytes([
                buf[cursor], buf[cursor + 1], buf[cursor + 2], buf[cursor + 3],
                buf[cursor + 4], buf[cursor + 5], buf[cursor + 6], buf[cursor + 7],
            ]);
            cursor += 8;

            entries.insert(path.clone(), PakEntry { path, offset, size });
        }

        Ok(Self { entries, data: buf })
    }

    /// Retrieve a file by its archive path (e.g. "textures/hero.png").
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        let entry = self.entries.get(path)?;
        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        self.data.get(start..end)
    }

    /// True if the archive contains the given path.
    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    /// Iterate over all stored paths.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// Cooking
// ---------------------------------------------------------------------------

/// Result of a cook attempt.
#[derive(Debug, Clone)]
pub struct CookResult {
    pub success: bool,
    pub message: String,
    pub cooked_project_path: Option<PathBuf>,
    pub pak_path: Option<PathBuf>,
}

/// Cook a project directory into a runtime-ready bundle.
///
/// * `project_dir` — source project directory (contains `project.rustixproj`, assets, etc.)
/// * `output_dir`  — directory to write `game.json` and `assets.pak` into.
pub fn cook_project(project_dir: &Path, output_dir: &Path) -> CookResult {
    // 1. Load source project
    let project_info = match crate::project::load_project_file(project_dir) {
        Some(p) => p,
        None => {
            return CookResult {
                success: false,
                message: "Failed to load project.rustixproj".to_string(),
                cooked_project_path: None,
                pak_path: None,
            };
        }
    };

    // 2. Strip editor metadata
    let cooked = CookedProject::from_project(&project_info);
    let cooked_json = match serde_json::to_string_pretty(&cooked) {
        Ok(j) => j,
        Err(e) => {
            return CookResult {
                success: false,
                message: format!("Failed to serialize cooked project: {e}"),
                cooked_project_path: None,
                pak_path: None,
            };
        }
    };

    // 3. Ensure output directory
    if let Err(e) = fs::create_dir_all(output_dir) {
        return CookResult {
            success: false,
            message: format!("Failed to create output directory: {e}"),
            cooked_project_path: None,
            pak_path: None,
        };
    }

    // 4. Write cooked project JSON
    let game_json_path = output_dir.join("game.json");
    if let Err(e) = fs::write(&game_json_path, cooked_json) {
        return CookResult {
            success: false,
            message: format!("Failed to write game.json: {e}"),
            cooked_project_path: None,
            pak_path: None,
        };
    }

    // 5. Build asset pak (skip the project file itself)
    let pak_path = output_dir.join("assets.pak");
    let mut builder = PakBuilder::new();

    // Add all files from project dir except editor files and the project file
    for entry in match fs::read_dir(project_dir) {
        Ok(r) => r,
        Err(e) => {
            return CookResult {
                success: false,
                message: format!("Failed to read project directory: {e}"),
                cooked_project_path: Some(game_json_path),
                pak_path: None,
            };
        }
    } {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Skipping directory entry: {e}");
                continue;
            }
        };
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if name == crate::project::PROJECT_FILE {
            continue; // don't pack the raw project file
        }
        if is_editor_file(&path) {
            continue;
        }

        if path.is_dir() {
            if let Err(e) = builder.add_dir(&path) {
                tracing::warn!("Failed to add directory {} to pak: {e}", path.display());
            }
        } else {
            let rel = name;
            match fs::read(&path) {
                Ok(data) => builder.add_data(rel, data),
                Err(e) => tracing::warn!("Failed to read {} for pak: {e}", path.display()),
            }
        }
    }

    if let Err(e) = builder.write(&pak_path) {
        return CookResult {
            success: false,
            message: format!("Failed to write assets.pak: {e}"),
            cooked_project_path: Some(game_json_path),
            pak_path: None,
        };
    }

    let file_count = PakArchive::load(&pak_path)
        .map(|a| a.entries.len())
        .unwrap_or(0);

    CookResult {
        success: true,
        message: format!(
            "Cooked project '{}' into {} ({} assets)",
            cooked.name,
            output_dir.display(),
            file_count
        ),
        cooked_project_path: Some(game_json_path),
        pak_path: Some(pak_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pak_roundtrip() {
        let mut builder = PakBuilder::new();
        builder.add_data("hello.txt", b"world".to_vec());
        builder.add_data("nested/file.bin", b"\x01\x02\x03".to_vec());

        let tmp = std::env::temp_dir().join("rustix_test.pak");
        builder.write(&tmp).unwrap();

        let archive = PakArchive::load(&tmp).unwrap();
        assert!(archive.contains("hello.txt"));
        assert!(archive.contains("nested/file.bin"));
        assert_eq!(archive.get("hello.txt").unwrap(), b"world");
        assert_eq!(archive.get("nested/file.bin").unwrap(), b"\x01\x02\x03");

        let _ = std::fs::remove_file(&tmp);
    }
}
