#[cfg(test)]
use super::*;
use std::io::Read;

fn temp_jsonl(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("rustix_jsonlog_test_{}_{}", std::process::id(), name));
    p
}

#[test]
fn json_file_layer_creates_file() {
    let path = temp_jsonl("create");
    let _ = std::fs::remove_file(&path);
    let layer = JsonFileLayer::new(&path, 0, 0).unwrap();
    assert!(path.exists());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn json_file_layer_appends_lines() {
    let path = temp_jsonl("append");
    let _ = std::fs::remove_file(&path);
    let layer = JsonFileLayer::new(&path, 0, 0).unwrap();

    // Write two events via the tracing subscriber
    use tracing_subscriber::layer::SubscriberExt;

    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(score = 42, "player scored");
        tracing::warn!(target = "test", "something happened");
    });

    let mut contents = String::new();
    std::fs::File::open(&path)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();

    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 2, "expected 2 JSON lines, got {lines:?}");

    // Check first line contains expected fields
    assert!(lines[0].contains("\"timestamp\""));
    assert!(lines[0].contains("\"level\":\"INFO\""));
    assert!(lines[0].contains("\"message\":\"player scored\""));
    assert!(lines[0].contains("\"score\":42"));

    // Check second line
    assert!(lines[1].contains("\"level\":\"WARN\""));
    assert!(lines[1].contains("\"message\":\"something happened\""));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn json_file_layer_rotation() {
    let path = temp_jsonl("rotate");
    let _ = std::fs::remove_file(&path);
    let backup = path.with_extension("jsonl.0");
    let _ = std::fs::remove_file(&backup);

    let layer = JsonFileLayer::new(&path, 0, 3).unwrap();
    std::fs::write(&path, "old data\n").unwrap();

    layer.rotate(&path, 3).unwrap();

    assert!(backup.exists(), "backup file should exist after rotation");
    let backup_contents = std::fs::read_to_string(&backup).unwrap();
    assert_eq!(backup_contents, "old data\n");

    // Fresh file should be empty
    let fresh = std::fs::read_to_string(&path).unwrap();
    assert!(fresh.is_empty());

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&backup);
}

#[test]
fn json_file_layer_escape_quotes() {
    let path = temp_jsonl("escape");
    let _ = std::fs::remove_file(&path);
    let layer = JsonFileLayer::new(&path, 0, 0).unwrap();

    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(name = "he said \"hello\"", "message with \"quotes\"");
    });

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains(r#"\"hello\""#));
    assert!(contents.contains(r#"\"quotes\""#));
    // Verify the unescaped version does NOT appear
    assert!(!contents.contains("message with \"hello\""));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn json_file_layer_escaped_newlines() {
    let path = temp_jsonl("newline");
    let _ = std::fs::remove_file(&path);
    let layer = JsonFileLayer::new(&path, 0, 0).unwrap();

    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("line1\nline2");
    });

    let contents = std::fs::read_to_string(&path).unwrap();
    // Should be one single line
    assert_eq!(contents.lines().count(), 1);
    assert!(contents.contains(r#"\n"#));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn json_file_layer_auto_rotate_on_size() {
    let path = temp_jsonl("autorotate");
    let backup = path.with_extension("jsonl.0");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&backup);

    // Set a tiny max size so a single log line triggers rotation
    let layer = JsonFileLayer::new(&path, 1, 2).unwrap();

    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        // This line will exceed 1 byte and trigger auto-rotation
        tracing::info!("this is a log message that is definitely more than one byte long");
    });

    // The original file should have been rotated to backup
    assert!(backup.exists(), "backup should exist after auto-rotation");
    // Fresh file should exist (may or may not be empty depending on timing)
    assert!(path.exists(), "fresh file should exist after auto-rotation");

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&backup);
}
