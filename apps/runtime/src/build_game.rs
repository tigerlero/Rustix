//! Build & export game functionality for Rustix.
//!
//! Compiles the runtime in release mode, cooks the project (strips editor
//! metadata and packs assets into a `.pak` archive), and produces a
//! standalone game folder.

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
/// 4. **Cook** the project: strip editor metadata and pack assets into `assets.pak`.
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

    // Step 4: Cook project — strip editor metadata and pack assets
    let cook_result = crate::asset_cook::cook_project(project_dir, &output_dir);
    if !cook_result.success {
        return BuildResult {
            success: false,
            output_dir: Some(output_dir.clone()),
            message: format!("Asset cooking failed: {}", cook_result.message),
        };
    }

    // Step 5: Write launch script
    let launch_script = format!(
        "#!/bin/bash\n# Auto-generated launch script for {}\ncd \"$(dirname \"$0\")\"\n./{} --project .\n",
        project_name, binary_name
    );
    let launch_path = output_dir.join("launch.sh");
    if let Err(e) = std::fs::write(&launch_path, launch_script) {
        return BuildResult {
            success: false,
            output_dir: Some(output_dir.clone()),
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
            "Build complete! Output: {}\n{}\nRun: ./build/{}/launch.sh",
            output_dir.display(),
            cook_result.message,
            project_name
        ),
    }
}
