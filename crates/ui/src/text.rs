use std::collections::HashMap;

/// Information for a single rasterized glyph in the atlas.
pub struct GlyphInfo {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub width: u32,
    pub height: u32,
    pub advance: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
}

/// CPU-side glyph atlas backed by an RGBA8 texture.
/// Uses simple shelf packing. Glyphs are rasterized on-demand via `fontdue`.
pub struct GlyphAtlas {
    font: fontdue::Font,
    pub texture: Vec<u8>,
    pub width: u32,
    pub height: u32,
    glyphs: HashMap<(char, u32), GlyphInfo>,
    pack_x: u32,
    pack_y: u32,
    pack_row_height: u32,
    pub dirty: bool,
}

impl GlyphAtlas {
    pub fn new(font_data: &[u8]) -> Result<Self, String> {
        let font = fontdue::Font::from_bytes(font_data, fontdue::FontSettings::default())
            .map_err(|e| format!("fontdue load: {e:?}"))?;

        let width = 512;
        let height = 512;
        let mut texture = vec![0u8; (width * height * 4) as usize];
        // Reserve a 1x1 white pixel at (0,0) for solid-color rects.
        texture[0] = 255;
        texture[1] = 255;
        texture[2] = 255;
        texture[3] = 255;

        Ok(Self {
            font,
            texture,
            width,
            height,
            glyphs: HashMap::new(),
            pack_x: 1,
            pack_y: 0,
            pack_row_height: 0,
            dirty: false,
        })
    }

    /// UV coordinates for a sub-rect inside the atlas.
    fn uv_for(&self, x: u32, y: u32, w: u32, h: u32) -> ([f32; 2], [f32; 2]) {
        let min = [x as f32 / self.width as f32, y as f32 / self.height as f32];
        let max = [(x + w) as f32 / self.width as f32, (y + h) as f32 / self.height as f32];
        (min, max)
    }

    /// Center of the 1x1 white pixel reserved at the origin.
    pub fn white_uv(&self) -> [f32; 2] {
        [0.5 / self.width as f32, 0.5 / self.height as f32]
    }

    /// Rasterize a glyph at the requested pixel size (or return cached info).
    pub fn get_or_rasterize(&mut self, ch: char, px: u32) -> &GlyphInfo {
        let key = (ch, px);
        if self.glyphs.contains_key(&key) {
            return self.glyphs.get(&key).unwrap();
        }

        let (metrics, coverage) = self.font.rasterize(ch, px as f32);
        let gw = metrics.width as u32;
        let gh = metrics.height as u32;

        // Pack into atlas.
        let (x, y) = if self.pack_x + gw <= self.width {
            if gh > self.pack_row_height {
                self.pack_row_height = gh;
            }
            let pos = (self.pack_x, self.pack_y);
            self.pack_x += gw + 1; // 1px padding
            pos
        } else if self.pack_y + self.pack_row_height + gh <= self.height {
            self.pack_y += self.pack_row_height;
            self.pack_x = 0;
            self.pack_row_height = gh;
            let pos = (self.pack_x, self.pack_y);
            self.pack_x += gw + 1;
            pos
        } else {
            // Atlas overflow: emit a fallback 1x1 glyph so we don't panic.
            let info = GlyphInfo {
                uv_min: [0.0, 0.0],
                uv_max: [1.0 / self.width as f32, 1.0 / self.height as f32],
                width: 1,
                height: 1,
                advance: metrics.advance_width,
                bearing_x: 0.0,
                bearing_y: 0.0,
            };
            self.glyphs.insert(key, info);
            return self.glyphs.get(&key).unwrap();
        };

        // Blit coverage bitmap into RGBA8 atlas (coverage in R, G, B; alpha=255).
        for row in 0..gh {
            for col in 0..gw {
                let atlas_idx = (((y + row) * self.width + (x + col)) * 4) as usize;
                let cov_idx = (row * gw + col) as usize;
                let c = coverage[cov_idx];
                self.texture[atlas_idx] = c;
                self.texture[atlas_idx + 1] = c;
                self.texture[atlas_idx + 2] = c;
                self.texture[atlas_idx + 3] = 255;
            }
        }

        let (uv_min, uv_max) = self.uv_for(x, y, gw, gh);
        let info = GlyphInfo {
            uv_min,
            uv_max,
            width: gw,
            height: gh,
            advance: metrics.advance_width,
            bearing_x: metrics.bounds.xmin,
            bearing_y: metrics.bounds.ymin,
        };
        self.glyphs.insert(key, info);
        self.dirty = true;
        self.glyphs.get(&key).unwrap()
    }
}

// ── Runtime Font ──

/// Runtime font wrapper that can be created from a `FontAsset`.
///
/// Holds the raw TTF/OTF bytes so a `GlyphAtlas` can be constructed from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    pub name: String,
    pub data: Vec<u8>,
}

impl Font {
    pub fn from_asset(asset: &rustix_asset::font::FontAsset) -> Self {
        Self {
            name: asset.name.clone(),
            data: asset.data.clone(),
        }
    }

    /// Build a `GlyphAtlas` from this font's data.
    pub fn build_atlas(&self) -> Result<GlyphAtlas, String> {
        GlyphAtlas::new(&self.data)
    }
}
