//! Virtual file system for asset path resolution.
//!
//! `Vfs` maps logical asset paths (e.g. `"textures/hero.png"`) to physical
//! locations by searching a stack of mounted directories and archives.
//! Mounts are checked in reverse insertion order (last mount wins), so
//! user/mods can override engine assets by mounting later.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A physical source for files: either a directory on disk or an archive.
#[derive(Debug, Clone)]
pub enum MountPoint {
    /// A real directory on the host file system.
    Directory(PathBuf),
    /// An in-memory archive (e.g. packed .pak / .zip).
    Archive {
        name: String,
        /// Map of virtual paths → byte offsets and lengths inside `data`.
        entries: HashMap<String, ArchiveEntry>,
        /// Flattened bytes of every file concatenated.
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ArchiveEntry {
    pub offset: usize,
    pub len: usize,
}

impl MountPoint {
    /// Create a directory mount.
    pub fn directory(path: impl Into<PathBuf>) -> Self {
        MountPoint::Directory(path.into())
    }

    /// Attempt to read a file at `virtual_path` from this mount.
    pub fn read(&self, virtual_path: &str) -> Option<Vec<u8>> {
        match self {
            MountPoint::Directory(dir) => {
                let full = dir.join(virtual_path);
                std::fs::read(&full).ok()
            }
            MountPoint::Archive { entries, data, .. } => {
                let entry = entries.get(virtual_path)?;
                Some(data[entry.offset..entry.offset + entry.len].to_vec())
            }
        }
    }

    /// List the immediate children of a directory inside this mount.
    ///
    /// Returns `None` if the mount does not support listing or the
    /// directory does not exist.
    pub fn list(&self, virtual_dir: &str) -> Option<Vec<String>> {
        match self {
            MountPoint::Directory(dir) => {
                let full = dir.join(virtual_dir);
                let entries = std::fs::read_dir(&full).ok()?;
                let names: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                    .collect();
                Some(names)
            }
            MountPoint::Archive { entries, .. } => {
                let prefix = if virtual_dir.ends_with('/') || virtual_dir.is_empty() {
                    virtual_dir.to_string()
                } else {
                    format!("{}/", virtual_dir)
                };
                let mut seen: Vec<String> = Vec::new();
                for key in entries.keys() {
                    if let Some(rem) = key.strip_prefix(&prefix) {
                        if let Some(first) = rem.split('/').next() {
                            let s = first.to_string();
                            if !seen.contains(&s) {
                                seen.push(s);
                            }
                        }
                    }
                }
                if seen.is_empty() {
                    None
                } else {
                    Some(seen)
                }
            }
        }
    }

    /// Return true if this mount contains `virtual_path`.
    pub fn exists(&self, virtual_path: &str) -> bool {
        match self {
            MountPoint::Directory(dir) => dir.join(virtual_path).exists(),
            MountPoint::Archive { entries, .. } => entries.contains_key(virtual_path),
        }
    }

    /// Return the physical path for a directory mount, or `None` for archives.
    pub fn physical_path(&self, virtual_path: &str) -> Option<PathBuf> {
        match self {
            MountPoint::Directory(dir) => Some(dir.join(virtual_path)),
            MountPoint::Archive { .. } => None,
        }
    }
}

/// Virtual file system: a stack of mount points with path resolution.
///
/// ```rust
/// let mut vfs = Vfs::new();
/// vfs.mount("core", MountPoint::directory("./assets/core"));
/// vfs.mount("dlc", MountPoint::directory("./assets/dlc"));
///
/// let bytes = vfs.read("textures/hero.png");
/// ```
#[derive(Debug, Default)]
pub struct Vfs {
    /// Name → mount.  Last mount is checked first.
    mounts: Vec<(String, MountPoint)>,
}

impl Vfs {
    pub fn new() -> Self {
        Self { mounts: Vec::new() }
    }

    /// Mount a filesystem root under `name`.
    ///
    /// Later mounts shadow earlier ones for the same virtual path.
    pub fn mount(&mut self, name: impl Into<String>, point: MountPoint) {
        self.mounts.push((name.into(), point));
    }

    /// Remove a mount by name.
    pub fn unmount(&mut self, name: &str) {
        self.mounts.retain(|(n, _)| n != name);
    }

    /// Read the contents of a file at the virtual path.
    ///
    /// Mounts are checked from last to first (reverse order).
    pub fn read(&self, virtual_path: impl AsRef<str>) -> Option<Vec<u8>> {
        let path = virtual_path.as_ref();
        for (_, mount) in self.mounts.iter().rev() {
            if let Some(data) = mount.read(path) {
                return Some(data);
            }
        }
        None
    }

    /// Read a file and, if it comes from a directory mount, also return the
    /// physical `PathBuf`.  Archives return `None` for the path.
    pub fn read_with_path(&self, virtual_path: impl AsRef<str>) -> Option<(Vec<u8>, Option<PathBuf>)> {
        let path = virtual_path.as_ref();
        for (_, mount) in self.mounts.iter().rev() {
            if let Some(data) = mount.read(path) {
                let phys = mount.physical_path(path);
                return Some((data, phys));
            }
        }
        None
    }

    /// Return true if the virtual path exists in any mount.
    pub fn exists(&self, virtual_path: impl AsRef<str>) -> bool {
        let path = virtual_path.as_ref();
        self.mounts.iter().rev().any(|(_, m)| m.exists(path))
    }

    /// Resolve a virtual path to a physical `PathBuf` from the first
    /// directory mount that contains it.
    ///
    /// Returns `None` if the path only exists in an archive.
    pub fn resolve(&self, virtual_path: impl AsRef<str>) -> Option<PathBuf> {
        let path = virtual_path.as_ref();
        for (_, mount) in self.mounts.iter().rev() {
            if mount.exists(path) {
                return mount.physical_path(path);
            }
        }
        None
    }

    /// List the immediate children of a virtual directory, merging results
    /// from all mounts.
    pub fn list(&self, virtual_dir: impl AsRef<str>) -> Vec<String> {
        let dir = virtual_dir.as_ref();
        let mut all = Vec::new();
        for (_, mount) in self.mounts.iter().rev() {
            if let Some(mut entries) = mount.list(dir) {
                all.append(&mut entries);
            }
        }
        all.sort();
        all.dedup();
        all
    }

    /// Iterator over mount names.
    pub fn mount_names(&self) -> impl Iterator<Item = &str> {
        self.mounts.iter().map(|(n, _)| n.as_str())
    }
}

// ── helpers ──

/// Build a simple in-memory archive mount from a list of (path, bytes).
pub fn build_archive(name: impl Into<String>, files: HashMap<String, Vec<u8>>) -> MountPoint {
    let mut data = Vec::new();
    let mut entries = HashMap::new();
    for (path, bytes) in files {
        let offset = data.len();
        let len = bytes.len();
        data.extend_from_slice(&bytes);
        entries.insert(path, ArchiveEntry { offset, len });
    }
    MountPoint::Archive {
        name: name.into(),
        entries,
        data,
    }
}
