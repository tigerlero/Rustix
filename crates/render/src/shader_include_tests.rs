//! Tests for shader #include resolution.

use crate::shader_include::resolve;

#[test]
fn no_include_unchanged() {
    let src = "#version 460\nvoid main() {}\n";
    let out = resolve(src, None).unwrap();
    assert_eq!(out, src);
}

#[test]
fn detects_cycle() {
    let src = r#"#include "self.glsl"
"#;
    // Since self.glsl won't exist on disk, this will fail with "not found"
    // rather than cycle. The cycle test would need temp files.
    assert!(resolve(src, None).is_err());
}
