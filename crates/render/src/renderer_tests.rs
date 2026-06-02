use super::*;
use ash::vk;

#[test]
fn shadow_map_image_format_is_d32_sfloat() {
    let info = resource::shadow_map_image_info(1024);
    assert_eq!(info.format, vk::Format::D32_SFLOAT, "shadow map format should be D32_SFLOAT");
}

#[test]
fn shadow_map_image_size_matches_input() {
    let info = resource::shadow_map_image_info(512);
    assert_eq!(info.extent.width, 512, "width should match input size");
    assert_eq!(info.extent.height, 512, "height should match input size");
    assert_eq!(info.extent.depth, 1, "depth should be 1 for 2D shadow map");
}

#[test]
fn shadow_map_image_has_depth_and_sampled_usage() {
    let info = resource::shadow_map_image_info(1024);
    assert!(info.usage.contains(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT),
        "shadow map must have DEPTH_STENCIL_ATTACHMENT usage");
    assert!(info.usage.contains(vk::ImageUsageFlags::SAMPLED),
        "shadow map must have SAMPLED usage for shader reads");
}

#[test]
fn shadow_map_image_is_optimal_tiled_2d() {
    let info = resource::shadow_map_image_info(1024);
    assert_eq!(info.image_type, vk::ImageType::TYPE_2D, "shadow map should be 2D");
    assert_eq!(info.tiling, vk::ImageTiling::OPTIMAL, "shadow map should use optimal tiling");
    assert_eq!(info.mip_levels, 1, "shadow map should have 1 mip level");
    assert_eq!(info.array_layers, 1, "shadow map should have 1 array layer");
    assert_eq!(info.samples, vk::SampleCountFlags::TYPE_1, "shadow map should use 1 sample");
}

#[test]
fn shadow_sampler_nearest_filtering() {
    let info = resource::shadow_sampler_info();
    assert_eq!(info.mag_filter, vk::Filter::NEAREST, "shadow sampler mag filter should be NEAREST");
    assert_eq!(info.min_filter, vk::Filter::NEAREST, "shadow sampler min filter should be NEAREST");
    assert_eq!(info.mipmap_mode, vk::SamplerMipmapMode::NEAREST, "shadow sampler mipmap mode should be NEAREST");
}

#[test]
fn shadow_sampler_clamp_to_border() {
    let info = resource::shadow_sampler_info();
    assert_eq!(info.address_mode_u, vk::SamplerAddressMode::CLAMP_TO_BORDER,
        "address_mode_u should be CLAMP_TO_BORDER");
    assert_eq!(info.address_mode_v, vk::SamplerAddressMode::CLAMP_TO_BORDER,
        "address_mode_v should be CLAMP_TO_BORDER");
    assert_eq!(info.address_mode_w, vk::SamplerAddressMode::CLAMP_TO_BORDER,
        "address_mode_w should be CLAMP_TO_BORDER");
}

#[test]
fn shadow_sampler_opaque_white_border() {
    let info = resource::shadow_sampler_info();
    assert_eq!(info.border_color, vk::BorderColor::FLOAT_OPAQUE_WHITE,
        "border color should be FLOAT_OPAQUE_WHITE (samples to 1.0 = no shadow outside)");
}

#[test]
fn shadow_sampler_no_compare() {
    let info = resource::shadow_sampler_info();
    assert_eq!(info.compare_enable, vk::FALSE, "compare should be disabled for standard shadow sampling");
}

#[test]
fn layout_transition_undefined_to_depth_attachment() {
    let (src_stage, dst_stage, src_mask, dst_mask) = resource::layout_transition_params(
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    );
    assert_eq!(src_stage, vk::PipelineStageFlags::TOP_OF_PIPE, "src should be TOP_OF_PIPE");
    assert_eq!(dst_stage, vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS, "dst should be EARLY_FRAGMENT_TESTS");
    assert_eq!(src_mask, vk::AccessFlags::empty(), "src access should be empty");
    assert_eq!(dst_mask, vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE, "dst access should be depth write");
}

#[test]
fn layout_transition_depth_attachment_to_shader_read() {
    let (src_stage, dst_stage, src_mask, dst_mask) = resource::layout_transition_params(
        vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
    assert_eq!(src_stage, vk::PipelineStageFlags::LATE_FRAGMENT_TESTS, "src should be LATE_FRAGMENT_TESTS");
    assert_eq!(dst_stage, vk::PipelineStageFlags::FRAGMENT_SHADER, "dst should be FRAGMENT_SHADER");
    assert_eq!(src_mask, vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE, "src access should be depth write");
    assert_eq!(dst_mask, vk::AccessFlags::SHADER_READ, "dst access should be shader read");
}

#[test]
fn layout_transition_shader_read_to_depth_attachment() {
    let (src_stage, dst_stage, src_mask, dst_mask) = resource::layout_transition_params(
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    );
    assert_eq!(src_stage, vk::PipelineStageFlags::FRAGMENT_SHADER, "src should be FRAGMENT_SHADER");
    assert_eq!(dst_stage, vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS, "dst should be EARLY_FRAGMENT_TESTS");
    assert_eq!(src_mask, vk::AccessFlags::SHADER_READ, "src access should be shader read");
    assert_eq!(dst_mask, vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE, "dst access should be depth write");
}

#[test]
fn layout_transition_unknown_fallback() {
    let (src_stage, dst_stage, src_mask, dst_mask) = resource::layout_transition_params(
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        vk::ImageLayout::PRESENT_SRC_KHR,
    );
    assert_eq!(src_stage, vk::PipelineStageFlags::ALL_COMMANDS, "unknown transition src should be ALL_COMMANDS");
    assert_eq!(dst_stage, vk::PipelineStageFlags::ALL_COMMANDS, "unknown transition dst should be ALL_COMMANDS");
    assert_eq!(src_mask, vk::AccessFlags::empty(), "unknown transition src access should be empty");
    assert_eq!(dst_mask, vk::AccessFlags::empty(), "unknown transition dst access should be empty");
}
