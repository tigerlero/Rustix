//! Build & export game functionality for Rustix.
//!
//! Provides a simple pipeline to compile the runtime in release mode,
//! package the project assets, and produce a standalone game folder.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of a build attempt.
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub success: bool,
    pub output_dir: Option<PathBuf>,
    pub message: String,
}

/// Build a standalone game from the current project.
///
/// Steps:
/// 1. Compile rustix-runtime in release mode.
/// 2. Create output directory.
/// 3. Copy the release binary.
/// 4. Copy the project directory (assets, scene, settings).
/// 5. Write a launch script.
pub fn build_game(project_dir: &Path) -> BuildResult {
    let project_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("game");

    let output_dir = PathBuf::from("build").join(project_name);

    // Step 1: Compile release binary
    let compile = Command::new("cargo")
        .args(["build", "--release", "-p", "rustix-runtime"])
        .current_dir(
            project_dir
                .ancestors()
                .find(|p| p.join("Cargo.toml").exists())
                .unwrap_or(Path::new(".")),
        )
        .output();

    match compile {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return BuildResult {
                success: false,
                output_dir: None,
                message: format!("Compilation failed:\n{}", stderr),
            };
        }
        Err(e) => {
            return BuildResult {
                success: false,
                output_dir: None,
                message: format!("Failed to run cargo: {}", e),
            };
        }
    }

    // Step 2: Create output directory
    if let Err(e) = std::fs::create_dir_all(&output_dir) {
        return BuildResult {
            success: false,
            output_dir: None,
            message: format!("Failed to create output dir: {}", e),
        };
    }

    // Step 3: Copy release binary
    let binary_name = "rustix-runtime";
    let source_binary = PathBuf::from("target/release").join(binary_name);
    let dest_binary = output_dir.join(binary_name);

    if let Err(e) = std::fs::copy(&source_binary, &dest_binary) {
        return BuildResult {
            success: false,
            output_dir: None,
            message: format!("Failed to copy binary: {}", e),
        };
    }

    // Step 4: Copy project directory
    let project_out = output_dir.join("project");
    if let Err(e) = copy_dir_all(project_dir, &project_out) {
        return BuildResult {
            success: false,
            output_dir: None,
            message: format!("Failed to copy project assets: {}", e),
        };
    }

    // Step 5: Write launch script
    let launch_script = format!(
        "#!/bin/bash\n# Auto-generated launch script for {}\ncd \"$(dirname \"$0\")\"\n./{} --project ./project\n",
        project_name, binary_name
    );
    let launch_path = output_dir.join("launch.sh");
    if let Err(e) = std::fs::write(&launch_path, launch_script) {
        return BuildResult {
            success: false,
            output_dir: None,
            message: format!("Failed to write launch script: {}", e),
        };
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&launch_path).unwrap().permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(&launch_path, perms);
    }

    BuildResult {
        success: true,
        output_dir: Some(output_dir.clone()),
        message: format!(
            "Build complete! Output: {}\nRun: ./build/{}/launch.sh",
            output_dir.display(),
            project_name
        ),
    }
}

/// Recursively copy a directory.
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.as_ref().join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
