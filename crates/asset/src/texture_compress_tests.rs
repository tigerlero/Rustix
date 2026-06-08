//! Tests for texture compression types and calculations.

use crate::texture_compress::*;

#[test]
fn compressed_block_format_bc7_block_dims() {
    assert_eq!(CompressedBlockFormat::Bc7Unorm.block_dims(), (4, 4));
    assert_eq!(CompressedBlockFormat::Bc7UnormSrgb.block_dims(), (4, 4));
}

#[test]
fn compressed_block_format_astc_block_dims() {
    assert_eq!(CompressedBlockFormat::Astc4x4Unorm.block_dims(), (4, 4));
    assert_eq!(CompressedBlockFormat::Astc6x6Unorm.block_dims(), (6, 6));
    assert_eq!(CompressedBlockFormat::Astc8x8Unorm.block_dims(), (8, 8));
}

#[test]
fn compressed_block_format_block_size_bytes() {
    assert_eq!(CompressedBlockFormat::Bc7Unorm.block_size_bytes(), 16);
    assert_eq!(CompressedBlockFormat::Astc8x8UnormSrgb.block_size_bytes(), 16);
}

#[test]
fn compressed_block_format_is_srgb() {
    assert!(!CompressedBlockFormat::Bc7Unorm.is_srgb());
    assert!(CompressedBlockFormat::Bc7UnormSrgb.is_srgb());
    assert!(!CompressedBlockFormat::Astc4x4Unorm.is_srgb());
    assert!(CompressedBlockFormat::Astc4x4UnormSrgb.is_srgb());
}

#[test]
fn compressed_block_format_compressed_size() {
    let fmt = CompressedBlockFormat::Bc7Unorm;
    // 8x8 image = 2x2 blocks = 4 blocks * 16 bytes = 64 bytes
    assert_eq!(fmt.compressed_size(8, 8), 64);

    // 5x5 image = 2x2 blocks = 4 blocks * 16 bytes = 64 bytes
    assert_eq!(fmt.compressed_size(5, 5), 64);

    // 4x4 image = 1x1 blocks = 1 block * 16 bytes = 16 bytes
    assert_eq!(fmt.compressed_size(4, 4), 16);
}

#[test]
fn compressed_block_format_compressed_size_astc_8x8() {
    let fmt = CompressedBlockFormat::Astc8x8Unorm;
    // 16x16 image = 2x2 blocks = 4 blocks * 16 bytes = 64 bytes
    assert_eq!(fmt.compressed_size(16, 16), 64);
}

#[test]
fn compressed_texture_size_bytes() {
    let tex = CompressedTexture {
        width: 64,
        height: 64,
        format: CompressedBlockFormat::Bc7Unorm,
        data: vec![0u8; 1024],
        mip_levels: 1,
    };
    assert_eq!(tex.size_bytes(), 1024);
}
