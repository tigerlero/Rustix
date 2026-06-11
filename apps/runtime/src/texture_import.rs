use rustix_render::Renderer;
use rustix_render::GpuTexture;

/// Options controlling texture import behavior.
#[derive(Debug, Clone)]
pub struct TextureImportOptions {
    /// Auto-compress to a GPU block format. HDR images are never compressed.
    pub compress: Option<rustix_asset::texture_compress::CompressedBlockFormat>,
    /// Generate mipmaps.
    pub generate_mips: bool,
    /// Treat as a normal map: reconstruct Z from XY and pack for better quality.
    pub normal_map: bool,
    /// Assume source is sRGB (affects compression color-space selection).
    pub srgb: bool,
}

impl Default for TextureImportOptions {
    fn default() -> Self {
        Self {
            compress: Some(rustix_asset::texture_compress::CompressedBlockFormat::Bc7Unorm),
            generate_mips: true,
            normal_map: false,
            srgb: true,
        }
    }
}

/// Result of importing a texture.
pub struct ImportedTexture {
    pub texture: GpuTexture,
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub compressed: bool,
    pub normal_map: bool,
}

/// Import a texture from raw file bytes.
///
/// Detects format from file extension (png, jpg, hdr, etc.) and applies the
/// requested processing pipeline (mip generation, compression, normal swizzle).
pub fn import_texture(
    renderer: &Renderer,
    data: &[u8],
    name: &str,
    ext: &str,
    options: &TextureImportOptions,
) -> Result<ImportedTexture, String> {
    let is_hdr = ext.eq_ignore_ascii_case("hdr") || ext.eq_ignore_ascii_case("exr");

    let img = image::load_from_memory(data)
        .map_err(|e| format!("image decode: {e}"))?;

    let (asset, is_float) = if is_hdr {
        let rgba32 = img.to_rgba32f();
        let (w, h) = (rgba32.width(), rgba32.height());
        let mut pixels = Vec::with_capacity((w * h * 8) as usize);
        use half::f16;
        for px in rgba32.pixels() {
            for ch in px.0 {
                pixels.extend_from_slice(&f16::from_f32(ch).to_le_bytes());
            }
        }
        let asset = rustix_asset::texture::TextureAsset::new(w, h, rustix_asset::texture::TextureFormat::R16g16b16a16Sfloat, pixels);
        (asset, true)
    } else {
        let rgba8 = img.to_rgba8();
        let (w, h) = (rgba8.width(), rgba8.height());
        let asset = rustix_asset::texture::TextureAsset::new(w, h, rustix_asset::texture::TextureFormat::R8g8b8a8Unorm, rgba8.into_raw());
        (asset, false)
    };

    // Normal map swizzle: reconstruct Z from XY and keep in RGB, set A=1
    let asset = if options.normal_map && !is_float {
        swizzle_normal_rgba8(&asset)?
    } else {
        asset
    };

    let mut effective_compress = options.compress;
    // HDR images skip compression (BC7/ASTC are LDR)
    if is_float {
        effective_compress = None;
    }
    // Normal maps prefer linear BC7 for quality
    if options.normal_map && effective_compress.is_some() {
        effective_compress = Some(rustix_asset::texture_compress::CompressedBlockFormat::Bc7Unorm);
    }
    // Adjust sRGB flag for compression
    if options.srgb && effective_compress.is_some() {
        use rustix_asset::texture_compress::CompressedBlockFormat::*;
        effective_compress = effective_compress.map(|f| match f {
            Bc7Unorm => Bc7UnormSrgb,
            Astc4x4Unorm => Astc4x4UnormSrgb,
            Astc6x6Unorm => Astc6x6UnormSrgb,
            Astc8x8Unorm => Astc8x8UnormSrgb,
            other => other,
        });
    }

    let (texture, mip_levels, compressed) = if let Some(target) = effective_compress {
        let compressed_mips = if options.generate_mips {
            rustix_asset::texture_compress::TextureCompressor::compress_with_mips(&asset, target)
                .map_err(|e| format!("compression: {e}"))?
        } else {
            vec![rustix_asset::texture_compress::TextureCompressor::compress(&asset, target)
                .map_err(|e| format!("compression: {e}"))?]
        };

        let vk_format = compressed_format_to_vk(target);
        let mips: Vec<&[u8]> = compressed_mips.iter().map(|m| m.data.as_slice()).collect();
        let tex = renderer.create_texture_compressed(asset.width, asset.height, vk_format, &mips)
            .map_err(|e| format!("gpu upload: {e}"))?;
        (tex, compressed_mips.len() as u32, true)
    } else {
        let vk_format = if is_float {
            ash::vk::Format::R16G16B16A16_SFLOAT
        } else if options.srgb {
            ash::vk::Format::R8G8B8A8_SRGB
        } else {
            ash::vk::Format::R8G8B8A8_UNORM
        };

        if options.generate_mips && !is_float {
            let mut mips = Vec::new();
            let mut current = asset.clone();
            loop {
                mips.push(current.clone());
                let done = current.width == 1 && current.height == 1;
                if done {
                    break;
                }
                current = halve_rgba8(&current)?;
            }
            let mip_refs: Vec<&[u8]> = mips.iter().map(|m| m.pixels.as_slice()).collect();
            let tex = renderer.create_texture_with_mips(asset.width, asset.height, vk_format, &mip_refs)
                .map_err(|e| format!("gpu upload: {e}"))?;
            (tex, mips.len() as u32, false)
        } else {
            let tex = renderer.create_texture_with_format(asset.width, asset.height, &asset.pixels, vk_format)
                .map_err(|e| format!("gpu upload: {e}"))?;
            (tex, 1, false)
        }
    };

    tracing::info!("imported texture {name}: {}x{} mips={} compressed={}",
        asset.width, asset.height, mip_levels, compressed);

    Ok(ImportedTexture {
        texture,
        width: asset.width,
        height: asset.height,
        mip_levels,
        compressed,
        normal_map: options.normal_map,
    })
}

/// Swizzle a normal map stored in RGBA8: reconstruct Z from XY and repack.
/// Assumes R=X, G=Y in [0,255] mapped to [-1,1].
fn swizzle_normal_rgba8(asset: &rustix_asset::texture::TextureAsset) -> Result<rustix_asset::texture::TextureAsset, String> {
    if asset.format != rustix_asset::texture::TextureFormat::R8g8b8a8Unorm {
        return Err("normal map swizzle only supported for RGBA8".into());
    }
    let mut out = asset.pixels.clone();
    let n = asset.width * asset.height;
    for i in 0..n {
        let off = (i as usize) * 4;
        let x = (out[off] as f32 / 255.0) * 2.0 - 1.0;
        let y = (out[off + 1] as f32 / 255.0) * 2.0 - 1.0;
        let z2 = 1.0 - x * x - y * y;
        let z = if z2 > 0.0 { z2.sqrt() } else { 0.0 };
        out[off + 2] = ((z * 0.5 + 0.5) * 255.0) as u8;
        out[off + 3] = 255;
    }
    Ok(rustix_asset::texture::TextureAsset::new(asset.width, asset.height, asset.format, out))
}

/// Box-filter downsample of an RGBA8 texture asset for mipmap generation.
fn halve_rgba8(src: &rustix_asset::texture::TextureAsset) -> Result<rustix_asset::texture::TextureAsset, String> {
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

    Ok(rustix_asset::texture::TextureAsset::new(new_w, new_h, src.format, dst))
}

fn compressed_format_to_vk(f: rustix_asset::texture_compress::CompressedBlockFormat) -> ash::vk::Format {
    use rustix_asset::texture_compress::CompressedBlockFormat::*;
    match f {
        Bc7Unorm => ash::vk::Format::BC7_UNORM_BLOCK,
        Bc7UnormSrgb => ash::vk::Format::BC7_SRGB_BLOCK,
        _ => ash::vk::Format::BC7_UNORM_BLOCK,
    }
}
