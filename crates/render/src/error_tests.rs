//! Tests for RenderError.

use crate::error::RenderError;

#[test]
fn render_error_display_variants() {
    let e1 = RenderError::InstanceCreation("bad instance".into());
    assert_eq!(e1.to_string(), "instance: bad instance");

    let e2 = RenderError::DeviceCreation("no device".into());
    assert_eq!(e2.to_string(), "device: no device");

    let e3 = RenderError::SurfaceCreation("no surface".into());
    assert_eq!(e3.to_string(), "surface: no surface");

    let e4 = RenderError::SwapchainCreation("bad swapchain".into());
    assert_eq!(e4.to_string(), "swapchain: bad swapchain");

    let e5 = RenderError::ShaderCompile("bad shader".into());
    assert_eq!(e5.to_string(), "shader: bad shader");

    let e6 = RenderError::PipelineCreation("bad pipeline".into());
    assert_eq!(e6.to_string(), "pipeline: bad pipeline");
}

#[test]
fn render_error_debug() {
    let e = RenderError::InstanceCreation("test".into());
    let s = format!("{:?}", e);
    assert!(s.contains("InstanceCreation"));
}
