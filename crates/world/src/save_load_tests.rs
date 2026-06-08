//! Tests for save/load header and migration.

use crate::save_load::{SaveHeader, SaveMigrator};

#[test]
fn save_header_new() {
    let header = SaveHeader::new(1);
    assert_eq!(header.magic, SaveHeader::MAGIC);
    assert_eq!(header.version, 1);
    assert_eq!(header.checksum, 0);
}

#[test]
fn save_header_is_valid() {
    let header = SaveHeader::new(1);
    assert!(header.is_valid());
}

#[test]
fn save_header_invalid_magic() {
    let mut header = SaveHeader::new(1);
    header.magic = *b"XXXX";
    assert!(!header.is_valid());
}

#[test]
fn migrator_no_migration_needed() {
    let migrator = SaveMigrator::new(5);
    let mut data = serde_json::Value::Null;
    let result = migrator.migrate(&mut data, 5).unwrap();
    assert_eq!(result, 5);
}

#[test]
fn migrator_newer_version_fails() {
    let migrator = SaveMigrator::new(3);
    let mut data = serde_json::Value::Null;
    assert!(migrator.migrate(&mut data, 5).is_err());
}

#[test]
fn migrator_single_migration() {
    let mut migrator = SaveMigrator::new(2);
    migrator.register(1, |data| {
        data.as_object_mut().unwrap().insert("migrated".to_string(), serde_json::Value::Bool(true));
    });

    let mut data = serde_json::json!({"key": "value"});
    let result = migrator.migrate(&mut data, 1).unwrap();
    assert_eq!(result, 2);
    assert_eq!(data["migrated"], true);
}

#[test]
fn migrator_multi_step_migration() {
    let mut migrator = SaveMigrator::new(3);
    migrator.register(1, |data| {
        data.as_object_mut().unwrap().insert("step1".to_string(), serde_json::Value::Bool(true));
    });
    migrator.register(2, |data| {
        data.as_object_mut().unwrap().insert("step2".to_string(), serde_json::Value::Bool(true));
    });

    let mut data = serde_json::json!({});
    let result = migrator.migrate(&mut data, 1).unwrap();
    assert_eq!(result, 3);
    assert_eq!(data["step1"], true);
    assert_eq!(data["step2"], true);
}

#[test]
fn migrator_missing_path_fails() {
    let mut migrator = SaveMigrator::new(5);
    migrator.register(1, |_data| {});
    // No migration from 2 to 5 registered
    let mut data = serde_json::Value::Null;
    assert!(migrator.migrate(&mut data, 2).is_err());
}
