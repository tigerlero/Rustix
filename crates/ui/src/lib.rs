//! Rustix Engine UI Framework — Immediate mode HUD, menus, debug overlay.

use ash::vk;
use gpu_allocator::MemoryLocation;
use rustix_render::Renderer;
use rustix_render::RenderError;

pub use glam::Vec2;

// ── GPU Vertex ──

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct UIVertex {
    pos: [f32; 2],
    color: [u8; 4],
}

// ── Draw Commands ──

#[derive(Debug, Clone)]
pub enum DrawCommand {
    Rect { min: Vec2, max: Vec2, fill: [u8; 4] },
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
}

// ── UI Context ──

pub struct UIContext {
    pub draw_list: DrawList,
    pub screen_size: Vec2,
    pub cursor: Vec2,
    pub interact: Interaction,
    next_id: usize,
}

impl UIContext {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            draw_list: DrawList::new(),
            screen_size: Vec2::new(screen_width, screen_height),
            cursor: Vec2::ZERO,
            interact: Interaction::default(),
            next_id: 1,
        }
    }

    pub fn begin_frame(&mut self, w: f32, h: f32, mouse_pos: (f32, f32), mouse_down: bool) {
        self.screen_size = Vec2::new(w, h);
        self.draw_list.clear();
        self.cursor = Vec2::ZERO;
        self.interact.mouse_pos = Vec2::new(mouse_pos.0, mouse_pos.1);
        self.interact.mouse_down = mouse_down;
        self.interact.hot = 0;
        self.next_id = 1;
    }

    pub fn end_frame(&mut self) {
        if !self.interact.mouse_down {
            self.interact.active = 0;
        }
    }

    fn next_id(&mut self) -> usize { let id = self.next_id; self.next_id += 1; id }

    // ── Drawing ──

    pub fn rect(&mut self, min: Vec2, max: Vec2, color: [u8; 4]) {
        self.draw_list.push(DrawCommand::Rect { min, max, fill: color });
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
    pub fn vstack(&mut self, x: f32, y: f32, spacing: f32, children: impl FnOnce(&mut Self)) {
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

/// Colored text placeholder (shows rect at position).
pub fn label(ctx: &mut UIContext, text: &str, pos: Vec2, font_size: f32, color: [u8; 4]) {
    let w = text.len() as f32 * font_size * 0.5;
    let h = font_size * 1.3;
    ctx.rect(pos, pos + Vec2::new(w, h), color);
}

// ── Vulkan Renderer ──

pub struct UIRenderer {
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    vertex_buffer: rustix_render::memory::GpuBuffer,
    index_buffer: rustix_render::memory::GpuBuffer,
}

impl UIRenderer {
    pub fn new(renderer: &Renderer) -> Result<Self, RenderError> {
        let device = renderer.device();
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX).offset(0).size(8);
        let pipeline_layout = unsafe {
            device.logical().create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default()
                .push_constant_ranges(&[push_range]).set_layouts(&[]), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("ui pl: {e}")))?
        };
        let vs = rustix_render::shader::ShaderModule::from_glsl(device.logical(), r"#version 460
layout(push_constant) uniform PC { vec2 screen_size; } pc;
layout(location=0) in vec2 aPos; layout(location=1) in vec4 aColor;
layout(location=0) out vec4 vColor;
void main() { vColor=aColor; gl_Position=vec4(2.0*aPos.x/pc.screen_size.x-1.0,1.0-2.0*aPos.y/pc.screen_size.y,0.0,1.0); }
", vk::ShaderStageFlags::VERTEX)?;
        let fs = rustix_render::shader::ShaderModule::from_glsl(device.logical(), r"#version 460
layout(location=0) in vec4 vColor; layout(location=0) out vec4 outC;
void main() { outC=vColor; }
", vk::ShaderStageFlags::FRAGMENT)?;
        let stages = [vs.stage_create_info(), fs.stage_create_info()];
        let stride = 12u32;
        let vbs=[vk::VertexInputBindingDescription::default().binding(0).stride(stride).input_rate(vk::VertexInputRate::VERTEX)];
        let vas=[
            vk::VertexInputAttributeDescription::default().binding(0).location(0).format(vk::Format::R32G32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(0).location(1).format(vk::Format::R8G8B8A8_UNORM).offset(8),
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
        let vb=renderer.create_buffer("ui_vb",1024*1024,vk::BufferUsageFlags::VERTEX_BUFFER,MemoryLocation::CpuToGpu)?;
        let ib=renderer.create_buffer("ui_ib",1024*1024,vk::BufferUsageFlags::INDEX_BUFFER,MemoryLocation::CpuToGpu)?;
        tracing::info!("UI renderer initialized");
        Ok(Self{pipeline,pipeline_layout,vertex_buffer:vb,index_buffer:ib})
    }

    pub fn render(&self, cmd: vk::CommandBuffer, renderer: &Renderer, draw_list: &DrawList) {
        if draw_list.commands().is_empty() { return; }
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
            renderer.device().logical().cmd_push_constants(cmd,self.pipeline_layout,vk::ShaderStageFlags::VERTEX,0,bytemuck::bytes_of(&s));
        }
        let mut verts: Vec<UIVertex> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        for cmd in draw_list.commands() {
            match cmd {
                DrawCommand::Rect { min, max, fill } => {
                    let base = verts.len() as u16;
                    verts.push(UIVertex{pos:[min.x,min.y],color:*fill});
                    verts.push(UIVertex{pos:[max.x,min.y],color:*fill});
                    verts.push(UIVertex{pos:[max.x,max.y],color:*fill});
                    verts.push(UIVertex{pos:[min.x,max.y],color:*fill});
                    indices.extend_from_slice(&[base,base+1,base+2,base,base+2,base+3]);
                }
            }
        }
        if verts.is_empty() { return; }
        self.vertex_buffer.write(bytemuck::cast_slice(&verts));
        self.index_buffer.write(bytemuck::cast_slice(&indices));
        unsafe{
            renderer.device().logical().cmd_bind_vertex_buffers(cmd,0,&[self.vertex_buffer.buffer],&[0u64]);
            renderer.device().logical().cmd_bind_index_buffer(cmd,self.index_buffer.buffer,0,vk::IndexType::UINT16);
            renderer.device().logical().cmd_draw_indexed(cmd,indices.len() as u32,1,0,0,0);
            let dr=ash::khr::dynamic_rendering::Device::new(&renderer.instance.inner(),&renderer.device().logical());
            dr.cmd_end_rendering(cmd);
        }
    }
}
