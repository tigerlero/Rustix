use std::any::TypeId;
use std::future::Future;
use std::pin::Pin;

use crate::handle::Asset;

/// Result of importing an asset from raw data.
pub type ImportResult<T> = Result<T, String>;

/// A trait for importing external data into engine assets.
pub trait Importer: Send + Sync {
    type Asset: Asset;

    /// Human-readable name for the importer.
    fn name(&self) -> &'static str;

    /// File extensions this importer supports.
    fn extensions(&self) -> &[&'static str];

    /// Import asset from bytes. Returns the parsed asset or an error.
    fn import<'a>(&self, bytes: &'a [u8], hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>>;
}

/// A registry of importers for different file types.
#[derive(Default)]
pub struct ImporterRegistry {
    // Simplified - stores just extension -> (TypeId, name) mapping
    entries: std::collections::HashMap<String, (TypeId, &'static str)>,
}

impl ImporterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Asset>(&mut self, ext: &'static str, type_id: TypeId, name: &'static str) {
        self.entries.insert(ext.to_string(), (type_id, name));
    }

    pub fn find_for_extension(&self, ext: &str) -> Option<(TypeId, &'static str)> {
        self.entries.get(ext).copied()
    }
}

/// Trait for assets that can be serialized to/from RON or JSON.
pub trait SerializableAsset: Asset + for<'de> serde::Deserialize<'de> + serde::Serialize {}

/// Import a RON-formatted asset.
pub fn import_ron<T: SerializableAsset>(bytes: &[u8]) -> ImportResult<T> {
    let s = std::str::from_utf8(bytes).map_err(|e| format!("invalid utf-8: {e}"))?;
    ron::from_str(s).map_err(|e| format!("RON parse error: {e}"))
}

/// Import a JSON-formatted asset.
pub fn import_json<T: SerializableAsset>(bytes: &[u8]) -> ImportResult<T> {
    let s = std::str::from_utf8(bytes).map_err(|e| format!("invalid utf-8: {e}"))?;
    serde_json::from_str(s).map_err(|e| format!("JSON parse error: {e}"))
}

/// Export a serializable asset to RON format.
pub fn export_ron<T: serde::Serialize>(asset: &T) -> Result<String, String> {
    ron::ser::to_string_pretty(asset, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("RON serialize error: {e}"))
}

/// Export a serializable asset to JSON format.
pub fn export_json<T: serde::Serialize>(asset: &T) -> Result<String, String> {
    serde_json::to_string_pretty(asset).map_err(|e| format!("JSON serialize error: {e}"))
}