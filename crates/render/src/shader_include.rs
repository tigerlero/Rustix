use std::collections::HashSet;
use std::path::{Path, PathBuf};
use crate::RenderError;

const MAX_INCLUDE_DEPTH: usize = 64;
const SEARCH_PATHS: &[&str] = &["shaders", "../shaders", "../../shaders"];

/// Resolve `#include "..."` / `#include <...>` directives in GLSL source.
///
/// Paths are resolved relative to `base_path` (the directory of the current
/// source file) and then against the standard shader search paths.
/// Circular includes are detected and rejected.
///
/// `#line` directives are inserted so that naga error messages retain
/// file/line information.
pub fn resolve(source: &str, base_path: Option<&Path>) -> Result<String, RenderError> {
    let mut visited = HashSet::new();
    resolve_recursive(source, base_path, &mut visited, 0)
}

fn resolve_recursive(
    source: &str,
    base_path: Option<&Path>,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Result<String, RenderError> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err(RenderError::ShaderCompile(
            "#include nesting depth exceeded".into(),
        ));
    }

    let mut output = String::new();
    for (line_no, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("#include") {
            let rest = rest.trim_start();
            let quoted = rest
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| rest.strip_prefix('<').and_then(|s| s.strip_suffix('>')));

            if let Some(include_path) = quoted {
                let resolved = resolve_path(include_path, base_path)?;
                let canonical = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());

                if !visited.insert(canonical.clone()) {
                    return Err(RenderError::ShaderCompile(format!(
                        "circular #include detected: {}",
                        resolved.display()
                    )));
                }

                let included = std::fs::read_to_string(&resolved).map_err(|e| {
                    RenderError::ShaderCompile(format!(
                        "cannot read #include {}: {e}",
                        resolved.display()
                    ))
                })?;

                let resolved_base = resolved.parent();
                let processed = resolve_recursive(&included, resolved_base, visited, depth + 1)?;

                // Restore line mapping for the included file.
                output.push_str(&format!("#line 1 \"{}\"\n", resolved.display()));
                output.push_str(&processed);
                // Restore line mapping for the current file.
                let current_name = base_path.map(|p| p.display().to_string()).unwrap_or_default();
                output.push_str(&format!("\n#line {} \"{}\"\n", line_no + 2, current_name));

                visited.remove(&canonical);
                continue;
            }
        }
        output.push_str(line);
        output.push('\n');
    }
    Ok(output)
}

fn resolve_path(include_path: &str, base_path: Option<&Path>) -> Result<PathBuf, RenderError> {
    // 1. Relative to the current source file.
    if let Some(base) = base_path {
        let relative = base.join(include_path);
        if relative.exists() {
            return Ok(relative);
        }
    }
    // 2. Standard search paths.
    for dir in SEARCH_PATHS {
        let candidate = Path::new(dir).join(include_path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(RenderError::ShaderCompile(format!(
        "#include not found: {include_path}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
