//! Tests for sampler cache key derivation.

use ash::vk;
use crate::sampler_cache::SamplerKey;

#[test]
fn sampler_key_from_info_basic() {
    let info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::NEAREST)
        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::REPEAT)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_w(vk::SamplerAddressMode::MIRROR_CLAMP_TO_EDGE)
        .mip_lod_bias(0.5)
        .anisotropy_enable(true)
        .max_anisotropy(4.0)
        .compare_enable(false)
        .compare_op(vk::CompareOp::LESS)
        .min_lod(0.0)
        .max_lod(10.0)
        .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
        .unnormalized_coordinates(false);

    let key = SamplerKey::from_info(&info);
    assert_eq!(key.mag_filter, vk::Filter::LINEAR);
    assert_eq!(key.min_filter, vk::Filter::NEAREST);
    assert_eq!(key.mipmap_mode, vk::SamplerMipmapMode::LINEAR);
    assert_eq!(key.address_mode_u, vk::SamplerAddressMode::REPEAT);
    assert_eq!(key.address_mode_v, vk::SamplerAddressMode::CLAMP_TO_EDGE);
    assert_eq!(key.address_mode_w, vk::SamplerAddressMode::MIRROR_CLAMP_TO_EDGE);
    assert_eq!(key.mip_lod_bias, 0.5f32.to_bits());
    assert!(key.anisotropy_enable);
    assert_eq!(key.max_anisotropy, 4.0f32.to_bits());
    assert!(!key.compare_enable);
    assert_eq!(key.compare_op, vk::CompareOp::LESS);
    assert_eq!(key.min_lod, 0.0f32.to_bits());
    assert_eq!(key.max_lod, 10.0f32.to_bits());
    assert_eq!(key.border_color, vk::BorderColor::INT_OPAQUE_BLACK);
    assert!(!key.unnormalized_coordinates);
}

#[test]
fn sampler_key_from_info_bool_fields() {
    let info = vk::SamplerCreateInfo::default()
        .anisotropy_enable(false)
        .compare_enable(true)
        .unnormalized_coordinates(true);

    let key = SamplerKey::from_info(&info);
    assert!(!key.anisotropy_enable);
    assert!(key.compare_enable);
    assert!(key.unnormalized_coordinates);
}

#[test]
fn sampler_key_hash_and_eq() {
    let info1 = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::NEAREST);

    let info2 = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::NEAREST);

    let info3 = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::NEAREST)
        .min_filter(vk::Filter::LINEAR);

    let key1 = SamplerKey::from_info(&info1);
    let key2 = SamplerKey::from_info(&info2);
    let key3 = SamplerKey::from_info(&info3);

    assert_eq!(key1, key2);
    assert_ne!(key1, key3);

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h1 = DefaultHasher::new();
    let mut h2 = DefaultHasher::new();
    key1.hash(&mut h1);
    key2.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}
