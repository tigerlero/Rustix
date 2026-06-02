use super::*;
use ash::vk;

#[test]
fn shadow_descriptor_binding_is_ubo_at_binding_0() {
    let bindings = shadow_descriptor_set_bindings();
    assert_eq!(bindings.len(), 1, "shadow pass should have exactly 1 descriptor binding");
    let b = bindings[0];
    assert_eq!(b.binding, 0, "UBO should be at binding 0");
    assert_eq!(b.descriptor_type, vk::DescriptorType::UNIFORM_BUFFER, "should be UNIFORM_BUFFER");
    assert_eq!(b.descriptor_count, 1, "should have 1 descriptor");
    assert!(b.stage_flags == vk::ShaderStageFlags::VERTEX,
        "UBO should only be visible to VERTEX stage in shadow pass");
}

#[test]
fn shadow_push_constant_range_vertex_only() {
    let range = shadow_push_constant_range();
    assert_eq!(range.stage_flags, vk::ShaderStageFlags::VERTEX,
        "shadow push constants should only be for VERTEX stage");
    assert_eq!(range.offset, 0, "push constant offset should be 0");
    assert_eq!(range.size, PUSH_CONSTANT_SIZE, "push constant size should be PUSH_CONSTANT_SIZE (128)");
}

#[test]
fn shadow_vertex_input_has_two_attributes_stride_24() {
    let (vbs, va) = shadow_vertex_input_state();
    assert_eq!(vbs.len(), 1, "should have 1 binding");
    assert_eq!(vbs[0].stride, 24, "stride should be 24 bytes (pos+normal)");
    assert_eq!(vbs[0].input_rate, vk::VertexInputRate::VERTEX, "input rate should be VERTEX");

    assert_eq!(va.len(), 2, "should have 2 vertex attributes");

    // Position at location 0, offset 0
    assert_eq!(va[0].location, 0, "position should be location 0");
    assert_eq!(va[0].format, vk::Format::R32G32B32_SFLOAT, "position should be vec3 float");
    assert_eq!(va[0].offset, 0, "position should be at offset 0");

    // Normal at location 1, offset 12
    assert_eq!(va[1].location, 1, "normal should be location 1");
    assert_eq!(va[1].format, vk::Format::R32G32B32_SFLOAT, "normal should be vec3 float");
    assert_eq!(va[1].offset, 12, "normal should be at offset 12");
}

#[test]
fn shadow_depth_stencil_depth_only() {
    let ds = shadow_depth_stencil_state();
    assert_eq!(ds.depth_test_enable, vk::TRUE, "depth test should be enabled");
    assert_eq!(ds.depth_write_enable, vk::TRUE, "depth write should be enabled");
    assert_eq!(ds.depth_compare_op, vk::CompareOp::LESS, "depth compare should be LESS");
}

#[test]
fn main_descriptor_bindings_count_and_types() {
    let bindings = main_descriptor_set_bindings();
    assert_eq!(bindings.len(), 3, "main pass should have 3 descriptor bindings");

    // Binding 0: UBO, VERTEX | FRAGMENT
    assert_eq!(bindings[0].binding, 0, "UBO should be at binding 0");
    assert_eq!(bindings[0].descriptor_type, vk::DescriptorType::UNIFORM_BUFFER);
    assert!(bindings[0].stage_flags == (vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
        "UBO should be visible to both VERTEX and FRAGMENT stages");

    // Binding 1: Sampled image, FRAGMENT only
    assert_eq!(bindings[1].binding, 1, "shadow texture should be at binding 1");
    assert_eq!(bindings[1].descriptor_type, vk::DescriptorType::SAMPLED_IMAGE);
    assert!(bindings[1].stage_flags == vk::ShaderStageFlags::FRAGMENT,
        "sampled image should only be visible to FRAGMENT stage");

    // Binding 2: Sampler, FRAGMENT only
    assert_eq!(bindings[2].binding, 2, "shadow sampler should be at binding 2");
    assert_eq!(bindings[2].descriptor_type, vk::DescriptorType::SAMPLER);
    assert!(bindings[2].stage_flags == vk::ShaderStageFlags::FRAGMENT,
        "sampler should only be visible to FRAGMENT stage");
}

#[test]
fn shadow_vs_main_descriptor_binding_difference() {
    let shadow = shadow_descriptor_set_bindings();
    let main = main_descriptor_set_bindings();

    assert_eq!(shadow.len(), 1, "shadow pass should have 1 binding");
    assert_eq!(main.len(), 3, "main pass should have 3 bindings");

    // Both have UBO at binding 0, but shadow is VERTEX-only while main is VERTEX|FRAGMENT
    assert!(shadow[0].stage_flags == vk::ShaderStageFlags::VERTEX,
        "shadow UBO should be VERTEX-only");
    assert!(main[0].stage_flags == (vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
        "main UBO should be VERTEX|FRAGMENT");
}
