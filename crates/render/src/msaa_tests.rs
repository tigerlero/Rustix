//! Tests for MSAA configuration.

use crate::msaa::*;

#[test]
fn msaa_samples_from_quality() {
    assert_eq!(MsaaSamples::from_quality(RenderQuality::Low), MsaaSamples::Off);
    assert_eq!(MsaaSamples::from_quality(RenderQuality::Medium), MsaaSamples::X2);
    assert_eq!(MsaaSamples::from_quality(RenderQuality::High), MsaaSamples::X4);
    assert_eq!(MsaaSamples::from_quality(RenderQuality::Ultra), MsaaSamples::X8);
}

#[test]
fn msaa_samples_variants() {
    assert_ne!(MsaaSamples::Off, MsaaSamples::X2);
    assert_ne!(MsaaSamples::X2, MsaaSamples::X4);
    assert_ne!(MsaaSamples::X4, MsaaSamples::X8);
}

#[test]
fn render_quality_variants() {
    assert_ne!(RenderQuality::Low, RenderQuality::Medium);
    assert_ne!(RenderQuality::Medium, RenderQuality::High);
    assert_ne!(RenderQuality::High, RenderQuality::Ultra);
}
