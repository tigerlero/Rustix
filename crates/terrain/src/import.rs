//! Heightmap importers for `.png`, `.raw`, and `.r16` files.

/// Import a heightmap from a `.png` grayscale image.
///
/// Returns a flat `Vec<f32>` of normalized heights [0, 1] and the
/// image dimensions `(width, height)`.
pub fn import_png(bytes: &[u8]) -> Result<(Vec<f32>, usize, usize), String> {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    let width = info.width as usize;
    let height = info.height as usize;
    let bit_depth = info.bit_depth;
    let color_type = info.color_type;

    if color_type != png::ColorType::Grayscale {
        return Err(format!("png: expected Grayscale, got {:?}", color_type));
    }

    let mut buf = vec![0u8; reader.output_buffer_size()];
    reader.next_frame(&mut buf).map_err(|e| e.to_string())?;

    let heights: Vec<f32> = match bit_depth {
        png::BitDepth::Eight => buf.into_iter().map(|v| v as f32 / 255.0).collect(),
        png::BitDepth::Sixteen => {
            let u16s: Vec<u16> = buf
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            u16s.into_iter().map(|v| v as f32 / 65535.0).collect()
        }
        _ => return Err(format!("png: unsupported bit depth {:?}", bit_depth)),
    };

    Ok((heights, width, height))
}

/// Import a heightmap from a raw binary file of 8-bit unsigned values.
///
/// `width` and `height` must be known in advance (raw files have no header).
pub fn import_raw(bytes: &[u8], width: usize, height: usize) -> Result<Vec<f32>, String> {
    let expected = width * height;
    if bytes.len() != expected {
        return Err(format!(
            "raw: expected {} bytes for {}x{}, got {}",
            expected,
            width,
            height,
            bytes.len()
        ));
    }
    Ok(bytes.iter().map(|&v| v as f32 / 255.0).collect())
}

/// Import a heightmap from a `.r16` file (16-bit unsigned raw, big-endian).
///
/// `width` and `height` must be known in advance.
pub fn import_r16(bytes: &[u8], width: usize, height: usize) -> Result<Vec<f32>, String> {
    let expected = width * height * 2;
    if bytes.len() != expected {
        return Err(format!(
            "r16: expected {} bytes for {}x{}, got {}",
            expected,
            width,
            height,
            bytes.len()
        ));
    }
    let heights: Vec<f32> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]) as f32 / 65535.0)
        .collect();
    Ok(heights)
}
