//! Disk cache for processed assets.
//!
//! After importing a source file (e.g. `.png`, `.wav`) into the engine's
//! native format, the resulting binary can be written to the disk cache.
//! On the next load the cache is checked first; if it is still valid
//! (source file has not changed), the importer is skipped and the cached
//! binary is read directly.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Metadata stored alongside a cached asset.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CacheMeta {
    /// Source file modification time at the moment the cache was written.
    source_modified: u64,
    /// Size of the source file in bytes.
    source_size: u64,
    /// Size of the cached data in bytes.
    cache_size: u64,
}

/// Disk-based cache for processed assets.
///
/// Cache layout:
/// ```text
/// <root>/<hash>.cache   — cached binary data
/// <root>/<hash>.meta    — JSON metadata (source mtime, sizes)
/// ```
///
/// The cache key is a hex-encoded hash of the source file's absolute path.
pub struct DiskCache {
    root: PathBuf,
}

impl DiskCache {
    /// Create (or open) a disk cache at `root`.
    pub fn new(root: impl AsRef<Path>) -> std::io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Cache root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Check whether a valid cached entry exists for `source_path`.
    ///
    /// A cache entry is considered valid when:
    /// * both the `.cache` and `.meta` files exist, and
    /// * the source file's current modification time and size match the
    ///   values recorded in the metadata.
    pub fn is_cached(&self, source_path: &Path) -> bool {
        let key = Self::key(source_path);
        let cache_path = self.root.join(format!("{key}.cache"));
        let meta_path = self.root.join(format!("{key}.meta"));

        if !cache_path.exists() || !meta_path.exists() {
            return false;
        }

        let meta: CacheMeta = match std::fs::read_to_string(&meta_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(m) => m,
            None => return false,
        };

        let (mtime, size) = match Self::source_info(source_path) {
            Some(v) => v,
            None => return false,
        };

        meta.source_modified == mtime && meta.source_size == size
    }

    /// Read cached binary data for `source_path`.
    ///
    /// Returns `None` if the cache is missing or invalid.
    pub fn read(&self, source_path: &Path) -> Option<Vec<u8>> {
        if !self.is_cached(source_path) {
            return None;
        }
        let key = Self::key(source_path);
        let cache_path = self.root.join(format!("{key}.cache"));
        std::fs::read(&cache_path).ok()
    }

    /// Write processed binary data to the cache for `source_path`.
    pub fn write(&self, source_path: &Path, data: &[u8]) -> std::io::Result<()> {
        let key = Self::key(source_path);
        let cache_path = self.root.join(format!("{key}.cache"));
        let meta_path = self.root.join(format!("{key}.meta"));

        std::fs::write(&cache_path, data)?;

        let (mtime, size) = Self::source_info(source_path).unwrap_or((0, 0));
        let meta = CacheMeta {
            source_modified: mtime,
            source_size: size,
            cache_size: data.len() as u64,
        };
        let meta_json = serde_json::to_string(&meta).unwrap_or_default();
        std::fs::write(&meta_path, meta_json)?;

        Ok(())
    }

    /// Remove the cached entry for `source_path`, if any.
    pub fn invalidate(&self, source_path: &Path) -> std::io::Result<()> {
        let key = Self::key(source_path);
        let cache_path = self.root.join(format!("{key}.cache"));
        let meta_path = self.root.join(format!("{key}.meta"));
        if cache_path.exists() {
            std::fs::remove_file(&cache_path)?;
        }
        if meta_path.exists() {
            std::fs::remove_file(&meta_path)?;
        }
        Ok(())
    }

    /// Remove every cached file in the cache root.
    pub fn clear(&self) -> std::io::Result<()> {
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                std::fs::remove_file(&path)?;
            }
        }
        Ok(())
    }

    /// Number of cache entries (pairs of `.cache` + `.meta` files).
    pub fn entry_count(&self) -> usize {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return 0;
        };
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |e| e == "cache"))
            .count()
    }

    /// Total size of all cached `.cache` files in bytes.
    pub fn total_size(&self) -> u64 {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return 0;
        };
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |e| e == "cache"))
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    }

    // ── helpers ──

    fn key(source_path: &Path) -> String {
        let mut hasher = DefaultHasher::new();
        source_path.to_string_lossy().hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn source_info(path: &Path) -> Option<(u64, u64)> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Some((mtime, meta.len()))
    }
}

impl std::fmt::Debug for DiskCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiskCache")
            .field("root", &self.root)
            .field("entries", &self.entry_count())
            .field("size_bytes", &self.total_size())
            .finish()
    }
}
