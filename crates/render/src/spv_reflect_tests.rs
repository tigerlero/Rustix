//! Tests for SPIR-V reflection.

use crate::spv_reflect::{ReflectedResource, ShaderReflection};
use ash::vk;

fn sample_resource(name: &str, stage: vk::ShaderStageFlags) -> ReflectedResource {
    ReflectedResource {
        set: 0,
        binding: 0,
        descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
        count: 1,
        stage,
        name: Some(name.to_string()),
    }
}

#[test]
fn shader_reflection_default() {
    let sr = ShaderReflection::default();
    assert!(sr.resources.is_empty());
    assert_eq!(sr.push_constant_size, None);
}

#[test]
fn shader_reflection_merge_adds_resources() {
    let mut a = ShaderReflection::default();
    a.resources.push(ReflectedResource {
        set: 0,
        binding: 0,
        descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
        count: 1,
        stage: vk::ShaderStageFlags::VERTEX,
        name: Some("ubo".into()),
    });

    let mut b = ShaderReflection::default();
    b.resources.push(ReflectedResource {
        set: 0,
        binding: 1,
        descriptor_type: vk::DescriptorType::SAMPLED_IMAGE,
        count: 1,
        stage: vk::ShaderStageFlags::FRAGMENT,
        name: Some("tex".into()),
    });

    a.merge(&b, vk::ShaderStageFlags::FRAGMENT);
    assert_eq!(a.resources.len(), 2);
}

#[test]
fn shader_reflection_merge_combines_stage_flags() {
    let mut a = ShaderReflection::default();
    a.resources.push(ReflectedResource {
        set: 0,
        binding: 0,
        descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
        count: 1,
        stage: vk::ShaderStageFlags::VERTEX,
        name: Some("ubo".into()),
    });

    let mut b = ShaderReflection::default();
    b.resources.push(ReflectedResource {
        set: 0,
        binding: 0,
        descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
        count: 1,
        stage: vk::ShaderStageFlags::FRAGMENT,
        name: Some("ubo".into()),
    });

    a.merge(&b, vk::ShaderStageFlags::FRAGMENT);
    assert_eq!(a.resources.len(), 1);
    assert_eq!(a.resources[0].stage, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT);
}

#[test]
fn shader_reflection_bindings_by_set() {
    let mut sr = ShaderReflection::default();
    sr.resources.push(ReflectedResource {
        set: 0,
        binding: 0,
        descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
        count: 1,
        stage: vk::ShaderStageFlags::VERTEX,
        name: None,
    });
    sr.resources.push(ReflectedResource {
        set: 1,
        binding: 2,
        descriptor_type: vk::DescriptorType::SAMPLED_IMAGE,
        count: 4,
        stage: vk::ShaderStageFlags::FRAGMENT,
        name: None,
    });

    let by_set = sr.bindings_by_set();
    assert_eq!(by_set.len(), 2);
    assert_eq!(by_set[0].0, 0);
    assert_eq!(by_set[0].1.len(), 1);
    assert_eq!(by_set[0].1[0].binding, 0);
    assert_eq!(by_set[1].0, 1);
    assert_eq!(by_set[1].1.len(), 1);
    assert_eq!(by_set[1].1[0].descriptor_count, 4);
}

#[test]
fn shader_reflection_push_constant_range_none() {
    let sr = ShaderReflection::default();
    assert!(sr.push_constant_range(vk::ShaderStageFlags::VERTEX).is_none());
}

#[test]
fn shader_reflection_push_constant_range_some() {
    let mut sr = ShaderReflection::default();
    sr.push_constant_size = Some(16);
    sr.push_constant_offset = 4;
    let range = sr.push_constant_range(vk::ShaderStageFlags::VERTEX).unwrap();
    assert_eq!(range.size, 16);
    assert_eq!(range.offset, 4);
    assert_eq!(range.stage_flags, vk::ShaderStageFlags::VERTEX);
}
