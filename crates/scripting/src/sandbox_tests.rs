//! Tests for script sandbox security policy.

use crate::sandbox::{SandboxPolicy, Sandbox};
use std::path::PathBuf;

#[test]
fn default_policy_denies_write() {
    let policy = SandboxPolicy::new();
    assert!(!policy.can_write(&PathBuf::from("assets/scripts/foo.rhai")));
}

#[test]
fn default_policy_allows_read_in_allowed_paths() {
    let policy = SandboxPolicy::new();
    assert!(policy.can_read(&PathBuf::from("assets/scripts/foo.rhai")));
}

#[test]
fn default_policy_denies_network() {
    let policy = SandboxPolicy::new();
    assert!(!policy.can_network());
}

#[test]
fn unrestricted_policy_allows_all() {
    let policy = SandboxPolicy::unrestricted();
    assert!(policy.can_read(&PathBuf::from("/etc/passwd")));
    assert!(policy.can_write(&PathBuf::from("/tmp/test")));
    assert!(policy.can_network());
}

#[test]
fn sandbox_enforces_read() {
    let sandbox = Sandbox::new(SandboxPolicy::new());
    assert!(sandbox.check_read(&PathBuf::from("assets/scripts/foo.rhai")).is_ok());
}

#[test]
fn sandbox_enforces_write() {
    let sandbox = Sandbox::new(SandboxPolicy::new());
    assert!(sandbox.check_write(&PathBuf::from("/tmp/test")).is_err());
}

#[test]
fn sandbox_enforces_network() {
    let sandbox = Sandbox::new(SandboxPolicy::new());
    assert!(sandbox.check_network().is_err());
}
