//! Asset Browser: file tree, thumbnails, drag-drop into scene.

use std::path::PathBuf;

/// An entry in the asset browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetEntry {
    Folder {
        path: PathBuf,
        name: String,
        expanded: bool,
    },
    File {
        path: PathBuf,
        name: String,
        asset_type: String,
    },
}

impl AssetEntry {
    pub fn name(&self) -> &str {
        match self {
            AssetEntry::Folder { name, .. } => name,
            AssetEntry::File { name, .. } => name,
        }
    }

    pub fn path(&self) -> &PathBuf {
        match self {
            AssetEntry::Folder { path, .. } => path,
            AssetEntry::File { path, .. } => path,
        }
    }
}

/// Asset browser state.
#[derive(Debug, Clone, Default)]
pub struct AssetBrowserState {
    pub root: PathBuf,
    pub entries: Vec<AssetEntry>,
    pub selected: Option<PathBuf>,
    pub search_filter: String,
}

impl AssetBrowserState {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            ..Default::default()
        }
    }

    pub fn set_entries(&mut self, entries: Vec<AssetEntry>) {
        self.entries = entries;
    }

    pub fn filtered_entries(&self) -> Vec<&AssetEntry> {
        if self.search_filter.is_empty() {
            self.entries.iter().collect()
        } else {
            self.entries
                .iter()
                .filter(|e| e.name().to_lowercase().contains(&self.search_filter.to_lowercase()))
                .collect()
        }
    }

    pub fn select(&mut self, path: PathBuf) {
        self.selected = Some(path);
    }
}
