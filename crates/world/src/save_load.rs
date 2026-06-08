//! Save/load with versioning and migration paths.

use serde::{Serialize, Deserialize};

/// Versioned save file header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub checksum: u32,
}

impl SaveHeader {
    pub const MAGIC: [u8; 4] = *b"RXSV";

    pub fn new(version: u32) -> Self {
        Self {
            magic: Self::MAGIC,
            version,
            checksum: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == Self::MAGIC
    }
}

/// A migration function that upgrades save data from one version to the next.
pub type MigrationFn = fn(&mut serde_json::Value);

/// Registry of migration paths from old save versions.
pub struct SaveMigrator {
    pub current_version: u32,
    pub migrations: Vec<(u32, MigrationFn)>,
}

impl SaveMigrator {
    pub fn new(current_version: u32) -> Self {
        Self {
            current_version,
            migrations: Vec::new(),
        }
    }

    pub fn register(&mut self, from_version: u32, migration: MigrationFn) {
        self.migrations.push((from_version, migration));
        self.migrations.sort_by_key(|m| m.0);
    }

    /// Apply all necessary migrations to bring `data` up to current version.
    pub fn migrate(&self, data: &mut serde_json::Value, mut version: u32) -> Result<u32, String> {
        if version == self.current_version {
            return Ok(version);
        }
        if version > self.current_version {
            return Err(format!(
                "Save version {} is newer than engine version {}",
                version, self.current_version
            ));
        }

        for (from, migration) in &self.migrations {
            if version == *from {
                migration(data);
                version += 1;
                if version == self.current_version {
                    break;
                }
            }
        }

        if version != self.current_version {
            return Err(format!(
                "No migration path from version {} to {}",
                version, self.current_version
            ));
        }

        Ok(version)
    }
}
