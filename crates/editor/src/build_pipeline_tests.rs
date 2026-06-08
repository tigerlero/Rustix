//! Tests for build pipeline state and configuration.

use std::path::PathBuf;
use crate::build_pipeline::*;

#[test]
fn build_target_variants() {
    assert_ne!(BuildTarget::Windows, BuildTarget::Linux);
    assert_ne!(BuildTarget::Linux, BuildTarget::MacOS);
    assert_ne!(BuildTarget::MacOS, BuildTarget::WebAssembly);
}

#[test]
fn build_profile_variants() {
    assert_ne!(BuildProfile::Debug, BuildProfile::Release);
    assert_ne!(BuildProfile::Release, BuildProfile::Shipping);
}

#[test]
fn build_config_default() {
    let cfg = BuildConfig::default();
    assert_eq!(cfg.target, BuildTarget::Linux);
    assert_eq!(cfg.profile, BuildProfile::Release);
    assert_eq!(cfg.output_dir, PathBuf::from("build"));
    assert!(cfg.cook_assets);
    assert!(cfg.compress_textures);
    assert!(cfg.strip_debug);
}

#[test]
fn build_config_clone() {
    let cfg = BuildConfig::default();
    let cloned = cfg.clone();
    assert_eq!(cfg, cloned);
}

#[test]
fn build_pipeline_new() {
    let cfg = BuildConfig::default();
    let bp = BuildPipeline::new(cfg);
    assert!(!bp.in_progress);
    assert_eq!(bp.progress_percent, 0.0);
    assert_eq!(bp.current_step, "");
    assert!(bp.logs.is_empty());
    assert!(bp.last_error.is_none());
}

#[test]
fn build_pipeline_default() {
    let bp: BuildPipeline = Default::default();
    assert!(!bp.in_progress);
}

#[test]
fn build_pipeline_start() {
    let mut bp = BuildPipeline::new(BuildConfig::default());
    bp.start();
    assert!(bp.in_progress);
    assert_eq!(bp.progress_percent, 0.0);
    assert_eq!(bp.last_error, None);
    assert!(bp.logs.is_empty());
}

#[test]
fn build_pipeline_set_step() {
    let mut bp = BuildPipeline::new(BuildConfig::default());
    bp.set_step("Cooking assets", 50.0);
    assert_eq!(bp.current_step, "Cooking assets");
    assert_eq!(bp.progress_percent, 50.0);
}

#[test]
fn build_pipeline_set_step_clamps() {
    let mut bp = BuildPipeline::new(BuildConfig::default());
    bp.set_step("Done", 150.0);
    assert_eq!(bp.progress_percent, 100.0);
    bp.set_step("Start", -10.0);
    assert_eq!(bp.progress_percent, 0.0);
}

#[test]
fn build_pipeline_log() {
    let mut bp = BuildPipeline::new(BuildConfig::default());
    bp.log("Step 1");
    bp.log("Step 2");
    assert_eq!(bp.logs.len(), 2);
    assert_eq!(bp.logs[0], "Step 1");
}

#[test]
fn build_pipeline_error() {
    let mut bp = BuildPipeline::new(BuildConfig::default());
    bp.start();
    bp.error("Something went wrong");
    assert_eq!(bp.last_error, Some("Something went wrong".to_string()));
    assert!(!bp.in_progress);
}

#[test]
fn build_pipeline_finish() {
    let mut bp = BuildPipeline::new(BuildConfig::default());
    bp.start();
    bp.finish();
    assert!(!bp.in_progress);
    assert_eq!(bp.progress_percent, 100.0);
    assert_eq!(bp.current_step, "Build complete");
}
