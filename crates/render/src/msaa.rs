//! MSAA resolve targets for quality level selection.

use ash::vk;

/// MSAA configuration for a render pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsaaSamples {
    Off,
    X2,
    X4,
    X8,
}

impl MsaaSamples {
    pub fn sample_count(&self) -> vk::SampleCountFlags {
        match self {
            MsaaSamples::Off => vk::SampleCountFlags::TYPE_1,
            MsaaSamples::X2 => vk::SampleCountFlags::TYPE_2,
            MsaaSamples::X4 => vk::SampleCountFlags::TYPE_4,
            MsaaSamples::X8 => vk::SampleCountFlags::TYPE_8,
        }
    }

    pub fn from_quality(quality: RenderQuality) -> Self {
        match quality {
            RenderQuality::Low => MsaaSamples::Off,
            RenderQuality::Medium => MsaaSamples::X2,
            RenderQuality::High => MsaaSamples::X4,
            RenderQuality::Ultra => MsaaSamples::X8,
        }
    }
}

/// Render quality presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderQuality {
    Low,
    Medium,
    High,
    Ultra,
}

/// An MSAA color attachment with automatic resolve.
#[derive(Debug)]
pub struct MsaaRenderTarget {
    pub color_image: vk::Image,
    pub color_view: vk::ImageView,
    pub resolve_image: Option<vk::Image>,
    pub resolve_view: Option<vk::ImageView>,
    pub samples: MsaaSamples,
    pub extent: vk::Extent2D,
}

impl MsaaRenderTarget {
    pub fn needs_resolve(&self) -> bool {
        self.samples != MsaaSamples::Off && self.resolve_image.is_some()
    }
}
