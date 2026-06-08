//! Texture block compression: BC7 and ASTC conversion.
//!
//! `TextureCompressor` wraps the `ctt` crate to compress raw RGBA8 source
//! images into GPU-native block-compressed formats (BC7, ASTC).  The
//! resulting `CompressedTexture` stores the raw compressed block bytes
//! ready for direct GPU upload.

use crate::texture::{TextureAsset, TextureFormat};

/// Supported GPU block-compressed formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressedBlockFormat {
    /// BC7 (BPTC) RGBA, linear. 16 bytes per 4×4 block (~1 byte/pixel).
    Bc7Unorm,
    /// BC7 (BPTC) RGBA, sRGB. 16 bytes per 4×4 block.
    Bc7UnormSrgb,
    /// ASTC 4×4 LDR, linear. 16 bytes per 4×4 block.
    Astc4x4Unorm,
    /// ASTC 4×4 LDR, sRGB.
    Astc4x4UnormSrgb,
    /// ASTC 6×6 LDR, linear. 16 bytes per 6×6 block (~0.44 byte/pixel).
    Astc6x6Unorm,
    /// ASTC 6×6 LDR, sRGB.
    Astc6x6UnormSrgb,
    /// ASTC 8×8 LDR, linear. 16 bytes per 8×8 block (~0.25 byte/pixel).
    Astc8x8Unorm,
    /// ASTC 8×8 LDR, sRGB.
    Astc8x8UnormSrgb,
}

impl CompressedBlockFormat {
    /// Dimensions of one block in texels.
    pub fn block_dims(&self) -> (u32, u32) {
        match self {
            CompressedBlockFormat::Bc7Unorm
            | CompressedBlockFormat::Bc7UnormSrgb
            | CompressedBlockFormat::Astc4x4Unorm
            | CompressedBlockFormat::Astc4x4UnormSrgb => (4, 4),
            CompressedBlockFormat::Astc6x6Unorm
            | CompressedBlockFormat::Astc6x6UnormSrgb => (6, 6),
            CompressedBlockFormat::Astc8x8Unorm
            | CompressedBlockFormat::Astc8x8UnormSrgb => (8, 8),
        }
    }

    /// Size of one compressed block in bytes.
    pub fn block_size_bytes(&self) -> usize {
        16
    }

    /// True if the format is sRGB.
    pub fn is_srgb(&self) -> bool {
        matches!(
            self,
            CompressedBlockFormat::Bc7UnormSrgb
                | CompressedBlockFormat::Astc4x4UnormSrgb
                | CompressedBlockFormat::Astc6x6UnormSrgb
                | CompressedBlockFormat::Astc8x8UnormSrgb
        )
    }

    /// Total compressed data size for an image of the given dimensions.
    pub fn compressed_size(&self, width: u32, height: u32) -> usize {
        let (bx, by) = self.block_dims();
        let blocks_x = ((width + bx - 1) / bx) as usize;
        let blocks_y = ((height + by - 1) / by) as usize;
        blocks_x * blocks_y * self.block_size_bytes()
    }
}

/// Result of a compression operation.
#[derive(Debug, Clone)]
pub struct CompressedTexture {
    pub width: u32,
    pub height: u32,
    pub format: CompressedBlockFormat,
    pub data: Vec<u8>,
    pub mip_levels: u32,
}

impl CompressedTexture {
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }
}

/// Compressor that converts `TextureAsset` RGBA source data into block-compressed
/// GPU formats using the `ctt` encoder backends.
pub struct TextureCompressor;

impl TextureCompressor {
    /// Compress a `TextureAsset` to the requested block format.
    ///
    /// Only `R8G8B8A8_UNORM` source format is supported at this time.
    pub fn compress(
        asset: &TextureAsset,
        target: CompressedBlockFormat,
    ) -> Result<CompressedTexture, String> {
        if asset.format != TextureFormat::R8g8b8a8Unorm {
            return Err(format!(
                "texture compression: source format {:?} not supported, expected R8g8b8a8Unorm",
                asset.format
            ));
        }

        let data = Self::run_ctt(&asset.pixels, asset.width, asset.height, target)?;

        Ok(CompressedTexture {
            width: asset.width,
            height: asset.height,
            format: target,
            data,
            mip_levels: asset.mip_levels,
        })
    }

    /// Generate mipmaps for a source RGBA8 image and compress each level.
    ///
    /// Returns a `Vec<CompressedTexture>` from mip 0 (full size) down to 1×1.
    pub fn compress_with_mips(
        asset: &TextureAsset,
        target: CompressedBlockFormat,
    ) -> Result<Vec<CompressedTexture>, String> {
        let mut mips = Vec::new();
        let mut current = asset.clone();

        loop {
            let compressed = Self::compress(&current, target)?;
            let done = current.width == 1 && current.height == 1;
            mips.push(compressed);
            if done {
                break;
            }
            current = halve_rgba8(&current)?;
        }

        Ok(mips)
    }

    // ── internal ──

    fn run_ctt(
        rgba: &[u8],
        width: u32,
        height: u32,
        target: CompressedBlockFormat,
    ) -> Result<Vec<u8>, String> {
        use ctt::{
            AlphaMode, ColorSpace, Container, ConvertSettings, Image, PipelineOutput, Surface,
            TargetFormat, TextureKind,
        };

        let color_space = if target.is_srgb() {
            ColorSpace::Srgb
        } else {
            ColorSpace::Linear
        };

        let surface = Surface {
            data: rgba.to_vec(),
            width,
            height,
            depth: 1,
            stride: width * 4,
            slice_stride: 0,
            format: ctt::Format::R8G8B8A8_UNORM,
            color_space,
            alpha: AlphaMode::Straight,
        };

        let image = Image {
            surfaces: vec![vec![surface]],
            kind: TextureKind::Texture2D,
        };

        let (ctt_format, encoder) = match target {
            CompressedBlockFormat::Bc7Unorm => {
                (ctt::Format::BC7_UNORM_BLOCK, ctt::encoders::Encoder::Auto)
            }
            CompressedBlockFormat::Bc7UnormSrgb => {
                (ctt::Format::BC7_SRGB_BLOCK, ctt::encoders::Encoder::Auto)
            }
            CompressedBlockFormat::Astc4x4Unorm => {
                (ctt::Format::ASTC_4x4_UNORM_BLOCK, ctt::encoders::Encoder::Auto)
            }
            CompressedBlockFormat::Astc4x4UnormSrgb => {
                (ctt::Format::ASTC_4x4_SRGB_BLOCK, ctt::encoders::Encoder::Auto)
            }
            CompressedBlockFormat::Astc6x6Unorm => {
                (ctt::Format::ASTC_6x6_UNORM_BLOCK, ctt::encoders::Encoder::Auto)
            }
            CompressedBlockFormat::Astc6x6UnormSrgb => {
                (ctt::Format::ASTC_6x6_SRGB_BLOCK, ctt::encoders::Encoder::Auto)
            }
            CompressedBlockFormat::Astc8x8Unorm => {
                (ctt::Format::ASTC_8x8_UNORM_BLOCK, ctt::encoders::Encoder::Auto)
            }
            CompressedBlockFormat::Astc8x8UnormSrgb => {
                (ctt::Format::ASTC_8x8_SRGB_BLOCK, ctt::encoders::Encoder::Auto)
            }
        };

        let settings = ConvertSettings {
            format: Some(TargetFormat::Compressed {
                format: ctt_format,
                encoder,
            }),
            container: Container::Raw,
            ..Default::default()
        };

        let output = ctt::convert(image, settings)
            .map_err(|e| format!("ctt compress: {e}"))?;

        match output {
            PipelineOutput::Raw(image) => {
                let surface = &image.surfaces[0][0];
                Ok(surface.data.clone())
            }
            PipelineOutput::Encoded(_) => {
                Err("ctt returned encoded container, expected raw blocks".to_string())
            }
        }
    }
}

/// Halve an RGBA8 texture (box-filter downsample) for mip generation.
fn halve_rgba8(src: &TextureAsset) -> Result<TextureAsset, String> {
    let new_w = (src.width / 2).max(1);
    let new_h = (src.height / 2).max(1);
    let mut dst = vec![0u8; (new_w * new_h * 4) as usize];

    for y in 0..new_h {
        for x in 0..new_w {
            let sx = (x * 2).min(src.width - 1) as usize;
            let sy = (y * 2).min(src.height - 1) as usize;
            let sx2 = (sx + 1).min(src.width as usize - 1);
            let sy2 = (sy + 1).min(src.height as usize - 1);

            let stride = src.width as usize * 4;
            let samples = [
                &src.pixels[sy * stride + sx * 4..sy * stride + sx * 4 + 4],
                &src.pixels[sy * stride + sx2 * 4..sy * stride + sx2 * 4 + 4],
                &src.pixels[sy2 * stride + sx * 4..sy2 * stride + sx * 4 + 4],
                &src.pixels[sy2 * stride + sx2 * 4..sy2 * stride + sx2 * 4 + 4],
            ];

            let dst_off = (y * new_w + x) as usize * 4;
            for ch in 0..4 {
                let sum: u32 = samples.iter().map(|s| s[ch] as u32).sum();
                dst[dst_off + ch] = (sum / 4) as u8;
            }
        }
    }

    Ok(TextureAsset::new(new_w, new_h, TextureFormat::R8g8b8a8Unorm, dst))
}
