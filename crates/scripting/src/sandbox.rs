//! Sandbox / security: restrict file system and network access.

use std::path::PathBuf;

/// Security policy for a script execution context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub allow_file_read: bool,
    pub allow_file_write: bool,
    pub allowed_read_paths: Vec<PathBuf>,
    pub allowed_write_paths: Vec<PathBuf>,
    pub allow_network: bool,
    pub max_memory_mb: usize,
    pub max_execution_time_ms: u64,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            allow_file_read: true,
            allow_file_write: false,
            allowed_read_paths: vec![PathBuf::from("assets/scripts")],
            allowed_write_paths: Vec::new(),
            allow_network: false,
            max_memory_mb: 64,
            max_execution_time_ms: 1000,
        }
    }
}

impl SandboxPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn unrestricted() -> Self {
        Self {
            allow_file_read: true,
            allow_file_write: true,
            allowed_read_paths: vec![],
            allowed_write_paths: vec![],
            allow_network: true,
            max_memory_mb: 256,
            max_execution_time_ms: 5000,
        }
    }

    pub fn can_read(&self, path: &PathBuf) -> bool {
        if !self.allow_file_read {
            return false;
        }
        if self.allowed_read_paths.is_empty() {
            return true;
        }
        self.allowed_read_paths.iter().any(|allowed| path.starts_with(allowed))
    }

    pub fn can_write(&self, path: &PathBuf) -> bool {
        if !self.allow_file_write {
            return false;
        }
        if self.allowed_write_paths.is_empty() {
            return true;
        }
        self.allowed_write_paths.iter().any(|allowed| path.starts_with(allowed))
    }

    pub fn can_network(&self) -> bool {
        self.allow_network
    }
}

/// Enforces sandbox policy at runtime.
#[derive(Debug, Clone, Default)]
pub struct Sandbox {
    pub policy: SandboxPolicy,
}

impl Sandbox {
    pub fn new(policy: SandboxPolicy) -> Self {
        Self { policy }
    }

    pub fn check_read(&self, path: &PathBuf) -> Result<(), String> {
        if self.policy.can_read(path) {
            Ok(())
        } else {
            Err(format!("Sandbox: read access denied for {:?}", path))
        }
    }

    pub fn check_write(&self, path: &PathBuf) -> Result<(), String> {
        if self.policy.can_write(path) {
            Ok(())
        } else {
            Err(format!("Sandbox: write access denied for {:?}", path))
        }
    }

    pub fn check_network(&self) -> Result<(), String> {
        if self.policy.can_network() {
            Ok(())
        } else {
            Err("Sandbox: network access denied".to_string())
        }
    }
}
