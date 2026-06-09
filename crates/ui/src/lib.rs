//! Rustix Engine UI Framework — Immediate mode HUD, menus, debug overlay.

pub mod text;
pub mod layout;

pub use text::Font;

use ash::vk;
use gpu_allocator::MemoryLocation;
use rustix_render::Renderer;
use rustix_render::RenderError;
use rustix_render::texture::GpuTexture;
use std::collections::HashMap;

pub use glam::Vec2;

// ── Keyboard Input ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UIKey {
    Backspace, Enter, Left, Right, Home, End, Delete, Escape,
}

// ── GPU Vertex ──

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct UIVertex {
    pos: [f32; 2],
    color: [u8; 4],
    uv: [f32; 2],
}

// ── Draw Commands ──

#[derive(Debug, Clone)]
pub enum DrawCommand {
    Rect { min: Vec2, max: Vec2, fill: [u8; 4] },
    Glyph { pos: Vec2, size: Vec2, uv_min: [f32; 2], uv_max: [f32; 2], color: [u8; 4] },
    Image { min: Vec2, max: Vec2, uv_min: [f32; 2], uv_max: [f32; 2], color: [u8; 4], view: vk::ImageView, sampler: vk::Sampler },
}

// ── Draw List ──

#[derive(Debug, Clone, Default)]
pub struct DrawList {
    commands: Vec<DrawCommand>,
}

impl DrawList {
    pub fn new() -> Self { Self { commands: Vec::new() } }
    pub fn push(&mut self, cmd: DrawCommand) { self.commands.push(cmd); }
    pub fn clear(&mut self) { self.commands.clear(); }
    pub fn commands(&self) -> &[DrawCommand] { &self.commands }
    pub fn len(&self) -> usize { self.commands.len() }
}

// ── Interaction State ──

#[derive(Debug, Clone, Copy, Default)]
pub struct Interaction {
    pub mouse_pos: Vec2,
    pub mouse_down: bool,
    pub hot: usize,
    pub active: usize,
    pub focused: usize,
}

// ── UI Context ──

pub struct UIContext {
    pub draw_list: DrawList,
    pub screen_size: Vec2,
    pub cursor: Vec2,
    pub interact: Interaction,
    pub glyph_atlas: Option<text::GlyphAtlas>,
    pub typed_chars: Vec<char>,
    pub keys_pressed: Vec<UIKey>,
    text_cursors: HashMap<usize, usize>,
    next_id: usize,
}

impl UIContext {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            draw_list: DrawList::new(),
            screen_size: Vec2::new(screen_width, screen_height),
            cursor: Vec2::ZERO,
            interact: Interaction::default(),
            glyph_atlas: None,
            typed_chars: Vec::new(),
            keys_pressed: Vec::new(),
            text_cursors: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn with_font(mut self, font_data: &[u8]) -> Self {
        match text::GlyphAtlas::new(font_data) {
            Ok(atlas) => { self.glyph_atlas = Some(atlas); }
            Err(e) => { tracing::warn!("failed to load UI font: {}", e); }
        }
        self
    }

    /// Load a font from an asset definition.
    pub fn with_font_asset(mut self, asset: &rustix_asset::font::FontAsset) -> Self {
        let font = Font::from_asset(asset);
        match font.build_atlas() {
            Ok(atlas) => { self.glyph_atlas = Some(atlas); }
            Err(e) => { tracing::warn!("failed to build atlas from font asset '{}': {}", asset.name, e); }
        }
        self
    }

    pub fn begin_frame(&mut self, w: f32, h: f32, mouse_pos: (f32, f32), mouse_down: bool) {
        self.screen_size = Vec2::new(w, h);
        self.draw_list.clear();
        self.cursor = Vec2::ZERO;
        self.interact.mouse_pos = Vec2::new(mouse_pos.0, mouse_pos.1);
        self.interact.mouse_down = mouse_down;
        self.interact.hot = 0;
        self.typed_chars.clear();
        self.keys_pressed.clear();
        self.next_id = 1;
    }

    pub fn end_frame(&mut self) {
        if !self.interact.mouse_down {
            self.interact.active = 0;
        }
    }

    pub fn feed_char(&mut self, ch: char) { self.typed_chars.push(ch); }
    pub fn feed_key(&mut self, key: UIKey) { self.keys_pressed.push(key); }

    fn next_id(&mut self) -> usize { let id = self.next_id; self.next_id += 1; id }

    // ── Drawing ──

    pub fn rect(&mut self, min: Vec2, max: Vec2, color: [u8; 4]) {
        self.draw_list.push(DrawCommand::Rect { min, max, fill: color });
    }

    pub fn text_glyph(&mut self, pos: Vec2, size: Vec2, uv_min: [f32; 2], uv_max: [f32; 2], color: [u8; 4]) {
        self.draw_list.push(DrawCommand::Glyph { pos, size, uv_min, uv_max, color });
    }

    pub fn image(&mut self, tex: &GpuTexture, pos: Vec2, size: Vec2, uv_min: [f32; 2], uv_max: [f32; 2], tint: [u8; 4]) {
        self.draw_list.push(DrawCommand::Image { min: pos, max: pos + size, uv_min, uv_max, color: tint, view: tex.view, sampler: tex.sampler });
    }

    // ── Layout ──

    pub fn center(&self, size: Vec2) -> Vec2 {
        (self.screen_size - size) * 0.5
    }

    pub fn set_cursor(&mut self, x: f32, y: f32) { self.cursor = Vec2::new(x, y); }

    /// Advance cursor after a widget.
    pub fn advance(&mut self, dy: f32) {
        self.cursor.y += dy;
        self.cursor.x = 0.0;
    }

    /// Layout children vertically, returns total height used.
    pub fn vstack(&mut self, x: f32, y: f32, _spacing: f32, children: impl FnOnce(&mut Self)) {
        self.cursor = Vec2::new(x, y);
        children(self);
    }

    // ── Interaction helpers ──

    fn is_hovered(&self, min: Vec2, max: Vec2) -> bool {
        let m = self.interact.mouse_pos;
        m.x >= min.x && m.x <= max.x && m.y >= min.y && m.y <= max.y
    }

    fn widget_interact(&mut self, id: usize, bounds: (Vec2, Vec2)) -> bool {
        let hovered = self.is_hovered(bounds.0, bounds.1);
        if hovered { self.interact.hot = id; }
        let active = self.interact.active == id;
        let clicked = hovered && active && !self.interact.mouse_down;
        if hovered && self.interact.mouse_down && self.interact.active == 0 {
            self.interact.active = id;
        }
        clicked
    }
}

// ── Widgets ──

/// Interactive button. Returns true on click.
pub fn button(ctx: &mut UIContext, text: &str, pos: Vec2, size: Vec2) -> bool {
    let id = ctx.next_id();
    let min = pos;
    let max = pos + size;
    let clicked = ctx.widget_interact(id, (min, max));
    let hovered = ctx.interact.hot == id;
    let active = ctx.interact.active == id;

    let color = if active { [100, 120, 180, 255] }
        else if hovered { [90, 100, 140, 255] }
        else { [70, 75, 95, 255] };

    ctx.rect(min, max, color);
    // Border
    let b = if active { [150, 170, 220, 255] } else if hovered { [120, 130, 180, 255] } else { [50, 55, 75, 255] };
    ctx.rect(min, Vec2::new(max.x, min.y + 1.0), b);
    ctx.rect(min, Vec2::new(min.x + 1.0, max.y), b);
    ctx.rect(Vec2::new(max.x - 1.0, min.y), max, b);
    ctx.rect(Vec2::new(min.x, max.y - 1.0), max, b);

    let _ = text;
    clicked
}

/// Horizontal slider. Returns the current value (0.0..1.0).
pub fn slider(ctx: &mut UIContext, value: &mut f32, min: f32, max: f32, pos: Vec2, width: f32, height: f32) {
    let id = ctx.next_id();
    let knob_w = 12.0;
    let track_min = pos;
    let track_max = pos + Vec2::new(width, height);

    // Track
    ctx.rect(track_min, track_max, [50, 55, 70, 255]);

    // Knob
    let t = (*value - min) / (max - min);
    let knob_x = pos.x + (width - knob_w) * t;
    let knob_min = Vec2::new(knob_x, pos.y);
    let knob_max = knob_min + Vec2::new(knob_w, height);
    let hovered = ctx.is_hovered(knob_min, knob_max);
    let active = ctx.interact.active == id;

    if hovered && ctx.interact.mouse_down && ctx.interact.active == 0 {
        ctx.interact.active = id;
    }
    if ctx.interact.active == id {
        let mx = ctx.interact.mouse_pos.x.clamp(pos.x, pos.x + width - knob_w);
        *value = min + (max - min) * ((mx - pos.x) / (width - knob_w));
    }

    let knob_color = if active || hovered { [150, 170, 220, 255] } else { [100, 120, 180, 255] };
    ctx.rect(knob_min, knob_max, knob_color);
}

/// Draw an image (sampled from a `GpuTexture`) with optional UVs and tint.
pub fn image_widget(ctx: &mut UIContext, tex: &GpuTexture, pos: Vec2, size: Vec2, uv_min: [f32; 2], uv_max: [f32; 2], tint: [u8; 4]) {
    ctx.image(tex, pos, size, uv_min, uv_max, tint);
}

/// Single-line text input. Returns `true` when Enter is pressed (submit).
pub fn text_input(ctx: &mut UIContext, buffer: &mut String, pos: Vec2, size: Vec2, font_size: f32) -> bool {
    let id = ctx.next_id();
    let min = pos;
    let max = pos + size;

    let hovered = ctx.is_hovered(min, max);
    if hovered && ctx.interact.mouse_down && ctx.interact.active == 0 {
        ctx.interact.active = id;
        ctx.interact.focused = id;
    }
    let focused = ctx.interact.focused == id;

    // Background
    let bg = if focused { [60, 65, 85, 255] } else { [50, 55, 70, 255] };
    ctx.rect(min, max, bg);
    // Border
    let b = if focused { [120, 150, 220, 255] } else { [50, 55, 75, 255] };
    ctx.rect(min, Vec2::new(max.x, min.y + 1.0), b);
    ctx.rect(min, Vec2::new(min.x + 1.0, max.y), b);
    ctx.rect(Vec2::new(max.x - 1.0, min.y), max, b);
    ctx.rect(Vec2::new(min.x, max.y - 1.0), max, b);

    let text_pos = pos + Vec2::new(4.0, size.y * 0.5 - font_size * 0.5);
    let mut submitted = false;

    if focused {
        let cursor_idx = ctx.text_cursors.get(&id).copied().unwrap_or(buffer.len());
        let mut cursor_idx = cursor_idx.min(buffer.len());

        // Insert typed characters
        for ch in std::mem::take(&mut ctx.typed_chars) {
            buffer.insert(cursor_idx, ch);
            cursor_idx += ch.len_utf8();
        }

        // Process special keys
        for key in std::mem::take(&mut ctx.keys_pressed) {
            match key {
                UIKey::Backspace if cursor_idx > 0 => {
                    let prev = buffer[..cursor_idx].chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                    buffer.drain((cursor_idx - prev)..cursor_idx);
                    cursor_idx -= prev;
                }
                UIKey::Delete if cursor_idx < buffer.len() => {
                    let ch = buffer[cursor_idx..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
                    buffer.drain(cursor_idx..(cursor_idx + ch));
                }
                UIKey::Left if cursor_idx > 0 => {
                    let prev = buffer[..cursor_idx].chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                    cursor_idx -= prev;
                }
                UIKey::Right if cursor_idx < buffer.len() => {
                    let ch = buffer[cursor_idx..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
                    cursor_idx += ch;
                }
                UIKey::Home => cursor_idx = 0,
                UIKey::End => cursor_idx = buffer.len(),
                UIKey::Enter => submitted = true,
                UIKey::Escape => ctx.interact.focused = 0,
                _ => {}
            }
        }

        // Clamp and store cursor
        cursor_idx = cursor_idx.min(buffer.len());
        ctx.text_cursors.insert(id, cursor_idx);

        // Compute cursor X position
        let cursor_x = {
            let mut x = text_pos.x;
            if let Some(ref mut atlas) = ctx.glyph_atlas {
                let px = font_size as u32;
                for ch in buffer[..cursor_idx].chars() {
                    x += atlas.get_or_rasterize(ch, px).advance;
                }
            } else {
                x += cursor_idx as f32 * font_size * 0.5;
            }
            x
        };

        // Draw cursor line
        let cursor_y1 = pos.y + 4.0;
        let cursor_y2 = pos.y + size.y - 4.0;
        ctx.rect(Vec2::new(cursor_x, cursor_y1), Vec2::new(cursor_x + 1.0, cursor_y2), [200, 210, 240, 255]);
    }

    // Draw text
    label(ctx, buffer, text_pos, font_size, [220, 225, 240, 255]);
    submitted
}

/// Draw text using the glyph atlas. Falls back to a colored rect placeholder when no font is loaded.
pub fn label(ctx: &mut UIContext, text: &str, pos: Vec2, font_size: f32, color: [u8; 4]) {
    if ctx.glyph_atlas.is_none() {
        let w = text.len() as f32 * font_size * 0.5;
        let h = font_size * 1.3;
        ctx.rect(pos, pos + Vec2::new(w, h), color);
        return;
    }
    let mut atlas = ctx.glyph_atlas.take().unwrap();
    let px = font_size as u32;
    let mut cursor_x = pos.x;
    for ch in text.chars() {
        let glyph = atlas.get_or_rasterize(ch, px);
        let x = cursor_x + glyph.bearing_x;
        let y = pos.y + glyph.bearing_y;
        ctx.draw_list.push(DrawCommand::Glyph {
            pos: Vec2::new(x, y),
            size: Vec2::new(glyph.width as f32, glyph.height as f32),
            uv_min: glyph.uv_min,
            uv_max: glyph.uv_max,
            color,
        });
        cursor_x += glyph.advance;
    }
    ctx.glyph_atlas = Some(atlas);
}

// ── Vulkan Renderer ──

pub struct UIRenderer {
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    vertex_buffer: rustix_render::memory::GpuBuffer,
    index_buffer: rustix_render::memory::GpuBuffer,
    // These two handles have no Rust-level reads but must outlive `desc_set` in Vulkan.
    // Destroying the pool or layout while the set is still in use is undefined behavior.
    #[allow(dead_code)]
    desc_set_layout: vk::DescriptorSetLayout,
    #[allow(dead_code)]
    desc_pool: vk::DescriptorPool,
    desc_set: vk::DescriptorSet,
    atlas_texture: Option<GpuTexture>,
}

impl UIRenderer {
    pub fn new(renderer: &Renderer) -> Result<Self, RenderError> {
        let device = renderer.device();
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT).offset(0).size(16);

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_set_layout = unsafe {
            device.logical().create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("ui desc layout: {e}")))?
        };

        let pipeline_layout = unsafe {
            device.logical().create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .push_constant_ranges(&[push_range])
                    .set_layouts(&[desc_set_layout]),
                None,
            ).map_err(|e| RenderError::PipelineCreation(format!("ui pl: {e}")))?
        };

        let vs = rustix_render::shader::ShaderModule::from_glsl(device.logical(), r#"#version 460
layout(push_constant) uniform PC { vec2 screen_size; uint tex_idx; } pc;
layout(location=0) in vec2 aPos;
layout(location=1) in vec4 aColor;
layout(location=2) in vec2 aUV;
layout(location=0) out vec4 vColor;
layout(location=1) out vec2 vUV;
layout(location=2) flat out uint vTexIdx;
void main() {
    vColor = aColor;
    vUV = aUV;
    vTexIdx = pc.tex_idx;
    gl_Position = vec4(2.0*aPos.x/pc.screen_size.x-1.0, 1.0-2.0*aPos.y/pc.screen_size.y, 0.0, 1.0);
}
"#, vk::ShaderStageFlags::VERTEX)?;

        let fs = rustix_render::shader::ShaderModule::from_glsl(device.logical(), r#"#version 460
layout(set=0, binding=0) uniform sampler2D atlas;
layout(set=0, binding=1) uniform sampler2D image_tex;
layout(location=0) in vec4 vColor;
layout(location=1) in vec2 vUV;
layout(location=2) flat in uint vTexIdx;
layout(location=0) out vec4 outC;
void main() {
    if (vTexIdx == 0u) {
        float mask = texture(atlas, vUV).r;
        outC = vec4(vColor.rgb, vColor.a * mask);
    } else {
        outC = texture(image_tex, vUV) * vColor;
    }
}
"#, vk::ShaderStageFlags::FRAGMENT)?;

        let stages = [vs.stage_create_info(), fs.stage_create_info()];
        let stride = 20u32; // 8 + 4 + 8
        let vbs=[vk::VertexInputBindingDescription::default().binding(0).stride(stride).input_rate(vk::VertexInputRate::VERTEX)];
        let vas=[
            vk::VertexInputAttributeDescription::default().binding(0).location(0).format(vk::Format::R32G32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(0).location(1).format(vk::Format::R8G8B8A8_UNORM).offset(8),
            vk::VertexInputAttributeDescription::default().binding(0).location(2).format(vk::Format::R32G32_SFLOAT).offset(12),
        ];
        let vi=vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&vbs).vertex_attribute_descriptions(&vas);
        let ia=vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps=[vk::Viewport::default()]; let scs=[vk::Rect2D::default()];
        let vp=vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs=vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::CLOCKWISE).line_width(1.0);
        let ms=vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds=vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(false).depth_write_enable(false);
        let ba=[vk::PipelineColorBlendAttachmentState::default().blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA).dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD).src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA).alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let cb=vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns=[vk::DynamicState::VIEWPORT,vk::DynamicState::SCISSOR];
        let dy=vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);
        let cf=[vk::Format::B8G8R8A8_SRGB];
        let mut dr=vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cf);
        let ci=vk::GraphicsPipelineCreateInfo::default().stages(&stages)
            .vertex_input_state(&vi).input_assembly_state(&ia).viewport_state(&vp)
            .rasterization_state(&rs).multisample_state(&ms).depth_stencil_state(&ds)
            .color_blend_state(&cb).dynamic_state(&dy).layout(pipeline_layout)
            .base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1).push_next(&mut dr);
        let pipeline=unsafe{device.logical().create_graphics_pipelines(device.pipeline_cache(),&[ci],None)
            .map_err(|(_,e)|RenderError::PipelineCreation(format!("ui pipe: {e}")))?.remove(0)};

        let pool_sizes = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 2 },
        ];
        let desc_pool = unsafe {
            device.logical().create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default().pool_sizes(&pool_sizes).max_sets(1), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("ui desc pool: {e}")))?
        };
        let desc_set = unsafe {
            device.logical().allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default().descriptor_pool(desc_pool).set_layouts(&[desc_set_layout]),
            ).map_err(|e| RenderError::DeviceCreation(format!("ui desc set: {e}")))?.remove(0)
        };

        let vb=renderer.create_buffer("ui_vb",1024*1024,vk::BufferUsageFlags::VERTEX_BUFFER,MemoryLocation::CpuToGpu)?;
        let ib=renderer.create_buffer("ui_ib",1024*1024,vk::BufferUsageFlags::INDEX_BUFFER,MemoryLocation::CpuToGpu)?;
        tracing::info!("UI renderer initialized");
        Ok(Self{pipeline,pipeline_layout,vertex_buffer:vb,index_buffer:ib,desc_set_layout,desc_pool,desc_set,atlas_texture:None})
    }

    pub fn update_atlas(&mut self, atlas: &text::GlyphAtlas, renderer: &Renderer) -> Result<(), RenderError> {
        if self.atlas_texture.is_none() {
            self.atlas_texture = Some(renderer.create_texture(atlas.width, atlas.height, &atlas.texture)?);
        } else if atlas.dirty {
            let tex = self.atlas_texture.as_ref().unwrap();
            renderer.update_texture_pixels(tex, atlas.width, atlas.height, &atlas.texture)?;
        }
        Ok(())
    }

    pub fn render(&mut self, cmd: vk::CommandBuffer, renderer: &Renderer, draw_list: &DrawList, mut atlas: Option<&mut text::GlyphAtlas>) {
        if draw_list.commands().is_empty() { return; }

        if let Some(a) = atlas.as_mut() {
            if let Err(e) = self.update_atlas(*a, renderer) {
                tracing::warn!("UI atlas update failed: {e}");
            }
            a.dirty = false;
        }

        let sw=renderer.swapchain.lock();
        let s=[sw.extent().width as f32, sw.extent().height as f32];
        let ca=vk::RenderingAttachmentInfoKHR::default().image_view(sw.current_image_view())
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD).store_op(vk::AttachmentStoreOp::STORE);
        let cas=[ca];
        let ri=vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D{offset:vk::Offset2D{x:0,y:0},extent:sw.extent()})
            .layer_count(1).color_attachments(&cas);
        let (w,h)=(sw.extent().width as f32, sw.extent().height as f32);
        drop(sw);
        unsafe{
            let dr=ash::khr::dynamic_rendering::Device::new(&renderer.instance.inner(),&renderer.device().logical());
            dr.cmd_begin_rendering(cmd,&ri);
            renderer.device().logical().cmd_set_viewport(cmd,0,&[vk::Viewport{x:0.0,y:h,width:w,height:-h,min_depth:0.0,max_depth:1.0}]);
            renderer.device().logical().cmd_bind_pipeline(cmd,vk::PipelineBindPoint::GRAPHICS,self.pipeline);
        }

        // Write atlas to binding 0 once.
        if let Some(ref tex) = self.atlas_texture {
            let atlas_info = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(tex.view)
                .sampler(tex.sampler);
            let writes = [vk::WriteDescriptorSet::default()
                .dst_set(self.desc_set)
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&atlas_info))];
            unsafe {
                renderer.device().logical().update_descriptor_sets(&writes, &[]);
            }
        }

        let white_uv = atlas.as_ref().map(|a| a.white_uv()).unwrap_or([0.0, 0.0]);
        let mut verts: Vec<UIVertex> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        let mut current_tex_idx: u32 = 0;
        let mut current_image: Option<(vk::ImageView, vk::Sampler)> = None;

        let pl = self.pipeline_layout;
        let vb = self.vertex_buffer.buffer;
        let ib = self.index_buffer.buffer;

        for dc in draw_list.commands() {
            let need_flush = match dc {
                DrawCommand::Rect { .. } | DrawCommand::Glyph { .. } => current_tex_idx != 0,
                DrawCommand::Image { view, sampler, .. } => {
                    let key = (*view, *sampler);
                    current_tex_idx != 1 || current_image != Some(key)
                }
            };
            if need_flush {
                if !verts.is_empty() {
                    self.vertex_buffer.write(bytemuck::cast_slice(&verts));
                    self.index_buffer.write(bytemuck::cast_slice(&indices));
                    unsafe {
                        renderer.device().logical().cmd_bind_vertex_buffers(cmd, 0, &[vb], &[0u64]);
                        renderer.device().logical().cmd_bind_index_buffer(cmd, ib, 0, vk::IndexType::UINT16);
                        renderer.device().logical().cmd_draw_indexed(cmd, indices.len() as u32, 1, 0, 0, 0);
                    }
                    verts.clear();
                    indices.clear();
                }
                match dc {
                    DrawCommand::Rect { .. } | DrawCommand::Glyph { .. } => {
                        current_tex_idx = 0;
                        current_image = None;
                        let mut pc = [0u8; 16];
                        pc[0..8].copy_from_slice(bytemuck::bytes_of(&s));
                        pc[8..12].copy_from_slice(bytemuck::bytes_of(&0u32));
                        unsafe {
                            renderer.device().logical().cmd_push_constants(cmd, pl,
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, &pc);
                        }
                    }
                    DrawCommand::Image { view, sampler, .. } => {
                        current_tex_idx = 1;
                        current_image = Some((*view, *sampler));
                        let img_info = vk::DescriptorImageInfo::default()
                            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                            .image_view(*view)
                            .sampler(*sampler);
                        let writes = [vk::WriteDescriptorSet::default()
                            .dst_set(self.desc_set)
                            .dst_binding(1)
                            .dst_array_element(0)
                            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                            .image_info(std::slice::from_ref(&img_info))];
                        unsafe {
                            renderer.device().logical().update_descriptor_sets(&writes, &[]);
                            renderer.device().logical().cmd_bind_descriptor_sets(
                                cmd, vk::PipelineBindPoint::GRAPHICS, pl, 0, &[self.desc_set], &[]
                            );
                        }
                        let mut pc = [0u8; 16];
                        pc[0..8].copy_from_slice(bytemuck::bytes_of(&s));
                        pc[8..12].copy_from_slice(bytemuck::bytes_of(&1u32));
                        unsafe {
                            renderer.device().logical().cmd_push_constants(cmd, pl,
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, &pc);
                        }
                    }
                }
            }
            match dc {
                DrawCommand::Rect { min, max, fill } => {
                    let base = verts.len() as u16;
                    verts.push(UIVertex{pos:[min.x,min.y],color:*fill,uv:white_uv});
                    verts.push(UIVertex{pos:[max.x,min.y],color:*fill,uv:white_uv});
                    verts.push(UIVertex{pos:[max.x,max.y],color:*fill,uv:white_uv});
                    verts.push(UIVertex{pos:[min.x,max.y],color:*fill,uv:white_uv});
                    indices.extend_from_slice(&[base,base+1,base+2,base,base+2,base+3]);
                }
                DrawCommand::Glyph { pos, size, uv_min, uv_max, color } => {
                    let base = verts.len() as u16;
                    let min = *pos;
                    let max = *pos + *size;
                    verts.push(UIVertex{pos:[min.x,min.y],color:*color,uv:[uv_min[0],uv_min[1]]});
                    verts.push(UIVertex{pos:[max.x,min.y],color:*color,uv:[uv_max[0],uv_min[1]]});
                    verts.push(UIVertex{pos:[max.x,max.y],color:*color,uv:[uv_max[0],uv_max[1]]});
                    verts.push(UIVertex{pos:[min.x,max.y],color:*color,uv:[uv_min[0],uv_max[1]]});
                    indices.extend_from_slice(&[base,base+1,base+2,base,base+2,base+3]);
                }
                DrawCommand::Image { min, max, uv_min, uv_max, color, .. } => {
                    let base = verts.len() as u16;
                    verts.push(UIVertex{pos:[min.x,min.y],color:*color,uv:[uv_min[0],uv_min[1]]});
                    verts.push(UIVertex{pos:[max.x,min.y],color:*color,uv:[uv_max[0],uv_min[1]]});
                    verts.push(UIVertex{pos:[max.x,max.y],color:*color,uv:[uv_max[0],uv_max[1]]});
                    verts.push(UIVertex{pos:[min.x,max.y],color:*color,uv:[uv_min[0],uv_max[1]]});
                    indices.extend_from_slice(&[base,base+1,base+2,base,base+2,base+3]);
                }
            }
        }
        if !verts.is_empty() {
            self.vertex_buffer.write(bytemuck::cast_slice(&verts));
            self.index_buffer.write(bytemuck::cast_slice(&indices));
            unsafe {
                renderer.device().logical().cmd_bind_vertex_buffers(cmd, 0, &[vb], &[0u64]);
                renderer.device().logical().cmd_bind_index_buffer(cmd, ib, 0, vk::IndexType::UINT16);
                renderer.device().logical().cmd_draw_indexed(cmd, indices.len() as u32, 1, 0, 0, 0);
            }
        }
        unsafe{
            let dr=ash::khr::dynamic_rendering::Device::new(&renderer.instance.inner(),&renderer.device().logical());
            dr.cmd_end_rendering(cmd);
        }
    }
}

#[cfg(test)]
pub mod lib_tests;
