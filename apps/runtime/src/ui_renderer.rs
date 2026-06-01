use ash::vk::{self, Handle};
use std::collections::HashMap;
use std::cell::Cell;
use gpu_allocator::MemoryLocation;
use rustix_render::Renderer;
use rustix_render::RenderError;

pub struct EguiVulkanRenderer {
    device: *const ash::Device,
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    descriptor_set: vk::DescriptorSet,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    font_texture: Option<rustix_render::GpuTexture>,
    font_texture_size: (u32, u32),
    #[allow(dead_code)]
    sampler: vk::Sampler,
    vertex_buffer: rustix_render::memory::GpuBuffer,
    index_buffer: rustix_render::memory::GpuBuffer,
    // Keep old font textures alive until pending command buffers finish.
    // With 3 in-flight frames, a texture pushed here survives at least
    // 3 frames before being dropped (and having its GPU memory freed).
    old_font_textures: Vec<rustix_render::GpuTexture>,
    // Map of all active egui textures (including font atlas) by TextureId.
    textures: HashMap<egui::TextureId, rustix_render::GpuTexture>,
    // Cache of currently bound texture id to avoid redundant descriptor writes.
    bound_texture: Cell<Option<egui::TextureId>>,
}

impl EguiVulkanRenderer {
    pub fn new(renderer: &Renderer, swapchain_format: vk::Format) -> Result<Self, RenderError> {
        let device = renderer.device();

        let placeholder = vec![255u8; 4];
        let font_texture = renderer.create_texture(1, 1, &placeholder)?;
        let sampler = unsafe {
            device.logical().create_sampler(&vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR).min_filter(vk::Filter::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("samp: {e}")))?
        };

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1).descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = unsafe {
            device.logical().create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("dsl: {e}")))?
        };

        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX).offset(0).size(8);
        let pipeline_layout = unsafe {
            device.logical().create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default()
                .set_layouts(&[desc_layout]).push_constant_ranges(&[push_range]), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("pl: {e}")))?
        };

        let pool_sizes = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLED_IMAGE, descriptor_count: 1 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLER, descriptor_count: 1 },
        ];
        let desc_pool = unsafe {
            device.logical().create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default()
                .pool_sizes(&pool_sizes).max_sets(1), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("dp: {e}")))?
        };
        let desc_set = unsafe {
            let mut sets = device.logical().allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default().descriptor_pool(desc_pool).set_layouts(&[desc_layout]),
            ).map_err(|e| RenderError::PipelineCreation(format!("ds: {e}")))?;
            sets.remove(0)
        };

        let vs = rustix_render::shader::ShaderModule::from_glsl(device.logical(), r"#version 460
layout(push_constant) uniform PC { vec2 screen_size; } pc;
layout(location=0) in vec2 aPos; layout(location=1) in vec2 aUV; layout(location=2) in vec4 aCol;
layout(location=0) out vec2 vUV; layout(location=1) out vec4 vColor;
void main() { vUV=aUV; vColor=aCol; gl_Position=vec4(2.0*aPos.x/pc.screen_size.x-1.0,1.0-2.0*aPos.y/pc.screen_size.y,0.0,1.0); }
", vk::ShaderStageFlags::VERTEX)?;
        let fs = rustix_render::shader::ShaderModule::from_wgsl(device.logical(), r"
@group(0) @binding(0) var uTex: texture_2d<f32>;
@group(0) @binding(1) var uSamp: sampler;
@fragment
fn main(@location(0) uv: vec2<f32>, @location(1) color: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(uTex, uSamp, uv) * color;
}
", vk::ShaderStageFlags::FRAGMENT)?;

        let stages = [vs.stage_create_info(), fs.stage_create_info()];
        let v_stride = std::mem::size_of::<egui::epaint::Vertex>() as u32;
        let vbs = [vk::VertexInputBindingDescription::default().binding(0).stride(v_stride).input_rate(vk::VertexInputRate::VERTEX)];
        let vas = [
            vk::VertexInputAttributeDescription::default().binding(0).location(0).format(vk::Format::R32G32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(0).location(1).format(vk::Format::R32G32_SFLOAT).offset(8),
            vk::VertexInputAttributeDescription::default().binding(0).location(2).format(vk::Format::R8G8B8A8_UNORM).offset(16),
        ];
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&vbs).vertex_attribute_descriptions(&vas);
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps=[vk::Viewport::default()]; let scs=[vk::Rect2D::default()];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let raster = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::CLOCKWISE).line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(false).depth_write_enable(false);
        let ba=[vk::PipelineColorBlendAttachmentState::default().blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA).dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD).src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA).alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let color_blend=vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns=[vk::DynamicState::VIEWPORT,vk::DynamicState::SCISSOR];
        let dynamic=vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);
        let cf=[swapchain_format];
        let mut dr=vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cf);
        let ci=vk::GraphicsPipelineCreateInfo::default().stages(&stages)
            .vertex_input_state(&vertex_input).input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state).rasterization_state(&raster)
            .multisample_state(&ms).depth_stencil_state(&ds).color_blend_state(&color_blend)
            .dynamic_state(&dynamic).layout(pipeline_layout)
            .base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1)
            .push_next(&mut dr);
        let pipeline=unsafe{device.logical().create_graphics_pipelines(device.pipeline_cache(),&[ci],None)
            .map_err(|(_,e)|RenderError::PipelineCreation(format!("pipe: {e}")))?.remove(0)};

        // Triple-buffered: 3 x 4MB slots so each in-flight frame writes to its own region
        let vb=renderer.create_buffer("egui_vb", 4*1024*1024 * 3, vk::BufferUsageFlags::VERTEX_BUFFER, MemoryLocation::CpuToGpu)?;
        let ib=renderer.create_buffer("egui_ib", 4*1024*1024 * 3, vk::BufferUsageFlags::INDEX_BUFFER, MemoryLocation::CpuToGpu)?;

        // Initial descriptor writes (placeholder texture + sampler)
        let img_info=[vk::DescriptorImageInfo::default()
            .image_view(font_texture.view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let samp_info=[vk::DescriptorImageInfo::default().sampler(sampler)];
        let writes = [
            vk::WriteDescriptorSet::default().dst_set(desc_set).dst_binding(0)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&img_info),
            vk::WriteDescriptorSet::default().dst_set(desc_set).dst_binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLER).image_info(&samp_info),
        ];
        unsafe{device.logical().update_descriptor_sets(&writes,&[]);}

        tracing::info!("egui renderer ready (WGSL separate texture+sampler)");

        Ok(Self{device:device.logical() as *const ash::Device,pipeline,pipeline_layout,descriptor_set:desc_set,descriptor_pool:desc_pool,descriptor_set_layout:desc_layout,font_texture:Some(font_texture),font_texture_size:(1,1),sampler,vertex_buffer:vb,index_buffer:ib,old_font_textures:Vec::new(),textures:HashMap::new(),bound_texture:Cell::new(None)})
    }

    pub fn update_textures(&mut self, renderer: &Renderer, delta: &egui::TexturesDelta) {
        for id in &delta.free {
            tracing::trace!("texture free: {id:?}");
            if let Some(tex) = self.textures.remove(id) {
                self.old_font_textures.push(tex);
            }
            // If the freed texture was currently bound, force rebind to fallback
            if self.bound_texture.get() == Some(*id) {
                self.bound_texture.set(None);
            }
        }
        for (id, d) in &delta.set {
            let (w, h, pixels_rgba) = match &d.image {
                egui::ImageData::Color(img) => {
                    let w = img.size[0] as u32;
                    let h = img.size[1] as u32;
                    let px: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b(), c.a()]).collect();
                    (w, h, px)
                }
            };
            tracing::trace!("texture set: {id:?} {w}x{h} pos={:?} (existing: {}x{})", d.pos, self.font_texture_size.0, self.font_texture_size.1);

            // Skip empty texture data (egui may send zero-size delta)
            if w == 0 || h == 0 || pixels_rgba.is_empty() { continue; }

            if let Some([x, y]) = d.pos {
                // Partial update: patch existing texture subregion
                if let Some(existing) = self.textures.get(id) {
                    if let Err(e) = renderer.update_texture_subregion(existing, x as u32, y as u32, w, h, &pixels_rgba) {
                        tracing::error!("failed to update texture subregion {id:?} at ({x},{y}): {e}");
                    }
                } else {
                    tracing::warn!("partial texture update for unknown id {id:?}");
                }
            } else {
                // Full update: create or replace the texture for this id
                if let Ok(tex) = renderer.create_texture(w, h, &pixels_rgba) {
                    if let Some(old) = self.textures.insert(*id, tex) {
                        self.old_font_textures.push(old);
                    }
                    while self.old_font_textures.len() > 8 {
                        self.old_font_textures.remove(0);
                    }
                    // If this id was the last bound texture, force a rebind next frame
                    if self.bound_texture.get() == Some(*id) {
                        self.bound_texture.set(None);
                    }

                    // Update cached atlas size (used for logging/diagnostics)
                    self.font_texture_size = (w, h);
                } else {
                    tracing::error!("failed to create/replace egui texture {id:?} ({}x{})", w, h);
                }
            }
        }
    }

    pub fn draw_primitives(&self, cmd: vk::CommandBuffer, renderer: &Renderer, primitives: &[egui::ClippedPrimitive], pixels_per_point: f32, frame_index: usize) {
        if primitives.is_empty() { return; }
        if self.font_texture.is_none() {
            tracing::warn!("draw_primitives: no font atlas texture bound, skipping");
            return;
        }

        let sw = renderer.swapchain.lock();
        let phys_w = sw.extent().width as f32;
        let phys_h = sw.extent().height as f32;

        if phys_w == 0.0 || phys_h == 0.0 || pixels_per_point <= 0.0 {
            return;
        }

        let logical_w = phys_w / pixels_per_point;
        let logical_h = phys_h / pixels_per_point;
        let s = [logical_w, logical_h];
        let ca = vk::RenderingAttachmentInfoKHR::default().image_view(sw.current_image_view())
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD).store_op(vk::AttachmentStoreOp::STORE);
        let cas = [ca];
        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D{offset:vk::Offset2D{x:0,y:0},extent:sw.extent()})
            .layer_count(1).color_attachments(&cas);
        drop(sw);

        unsafe{
            let dr=ash::khr::dynamic_rendering::Device::new(&renderer.instance.inner(),&renderer.device().logical());
            dr.cmd_begin_rendering(cmd,&ri);
            renderer.device().logical().cmd_set_viewport(cmd,0,&[vk::Viewport{x:0.0,y:phys_h,width:phys_w,height:-(phys_h),min_depth:0.0,max_depth:1.0}]);
            renderer.device().logical().cmd_bind_pipeline(cmd,vk::PipelineBindPoint::GRAPHICS,self.pipeline);
            renderer.device().logical().cmd_bind_descriptor_sets(cmd,vk::PipelineBindPoint::GRAPHICS,self.pipeline_layout,0,&[self.descriptor_set],&[]);
            renderer.device().logical().cmd_push_constants(cmd,self.pipeline_layout,vk::ShaderStageFlags::VERTEX,0,bytemuck::bytes_of(&s));
        }

        const SLOT_SIZE: u64 = 4 * 1024 * 1024;
        let slot = (frame_index % 3) as u64;
        let vb_base = slot * SLOT_SIZE;
        let ib_base = slot * SLOT_SIZE;
        let mut vb_off = 0u64;
        let mut ib_off = 0u64;
        for prim in primitives {
            let egui::ClippedPrimitive{clip_rect,primitive}=prim;
            if let egui::epaint::Primitive::Mesh(mesh)=primitive {
                if mesh.vertices.is_empty() || mesh.indices.is_empty() { continue; }

                // Bind the texture for this mesh if we have it; otherwise fall back to the font texture
                let wanted_id = Some(mesh.texture_id);
                if self.bound_texture.get() != wanted_id {
                    let tex_to_bind = self.textures.get(&mesh.texture_id).or_else(|| self.font_texture.as_ref());
                    if let Some(t) = tex_to_bind {
                        let img_info = [vk::DescriptorImageInfo::default()
                            .image_view(t.view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
                        let writes = [vk::WriteDescriptorSet::default()
                            .dst_set(self.descriptor_set).dst_binding(0)
                            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&img_info)];
                        unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
                        self.bound_texture.set(wanted_id);
                    }
                }

                let vb_bytes = bytemuck::cast_slice(&mesh.vertices);
                let ib_bytes = bytemuck::cast_slice(&mesh.indices);

                // Ensure we don't overflow the per-frame 4MB slot
                let vb_end = vb_off.saturating_add(vb_bytes.len() as u64);
                let ib_end = ib_off.saturating_add(ib_bytes.len() as u64);
                if vb_end > SLOT_SIZE || ib_end > SLOT_SIZE {
                    tracing::warn!("draw_primitives: vertex/index buffer slot overflow, dropping primitives");
                    break;
                }

                self.vertex_buffer.write_at(vb_bytes, vb_base + vb_off);
                self.index_buffer.write_at(ib_bytes, ib_base + ib_off);
                let _ = self.vertex_buffer.flush(vb_base + vb_off, vb_bytes.len() as u64);
                let _ = self.index_buffer.flush(ib_base + ib_off, ib_bytes.len() as u64);

                // Clamp scissor to valid range (avoid Vulkan validation errors)
                let sc_x = (clip_rect.min.x * pixels_per_point) as i32;
                let sc_y = (clip_rect.min.y * pixels_per_point) as i32;
                let sc_w = (clip_rect.width() * pixels_per_point).max(0.0) as u32;
                let sc_h = (clip_rect.height() * pixels_per_point).max(0.0) as u32;
                let sc = vk::Rect2D{
                    offset: vk::Offset2D{ x: sc_x.max(0), y: sc_y.max(0) },
                    extent: vk::Extent2D{ width: sc_w.min(phys_w as u32), height: sc_h.min(phys_h as u32) },
                };

                if sc.extent.width == 0 || sc.extent.height == 0 { continue; }

                unsafe{
                    renderer.device().logical().cmd_set_scissor(cmd,0,&[sc]);
                    renderer.device().logical().cmd_bind_vertex_buffers(cmd,0,&[self.vertex_buffer.buffer],&[vb_base + vb_off]);
                    renderer.device().logical().cmd_bind_index_buffer(cmd,self.index_buffer.buffer,ib_base + ib_off,vk::IndexType::UINT32);
                    renderer.device().logical().cmd_draw_indexed(cmd,mesh.indices.len() as u32,1,0,0,0);
                }
                vb_off += vb_bytes.len() as u64;
                ib_off += ib_bytes.len() as u64;
            }
        }

        unsafe{let dr=ash::khr::dynamic_rendering::Device::new(&renderer.instance.inner(),&renderer.device().logical());dr.cmd_end_rendering(cmd);}
    }
}

impl Drop for EguiVulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            if !self.device.is_null() {
                let dev = &*self.device;
                if !self.pipeline.is_null() {
                    dev.destroy_pipeline(self.pipeline, None);
                }
                if !self.pipeline_layout.is_null() {
                    dev.destroy_pipeline_layout(self.pipeline_layout, None);
                }
                if !self.descriptor_set_layout.is_null() {
                    dev.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
                }
                if !self.descriptor_pool.is_null() {
                    dev.destroy_descriptor_pool(self.descriptor_pool, None);
                }
                if !self.sampler.is_null() {
                    dev.destroy_sampler(self.sampler, None);
                }
            }
        }
        tracing::debug!("EguiVulkanRenderer dropped");
    }
}
