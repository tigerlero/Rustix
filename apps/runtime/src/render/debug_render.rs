//! Debug render mode resources and execution.

use ash::vk;
use rustix_render::Renderer;
use rustix_render::pipeline::{DebugOverdrawPipeline, DebugLightComplexityPipeline};

pub struct DebugRenderResources {
    pub overdraw_pipeline: Option<DebugOverdrawPipeline>,
    pub overdraw_desc_set: vk::DescriptorSet,
    pub light_complexity_pipeline: Option<DebugLightComplexityPipeline>,
    pub light_complexity_desc_set: vk::DescriptorSet,
}

impl DebugRenderResources {
    pub fn new(renderer: &Renderer) -> Result<Self, rustix_render::RenderError> {
        let device = renderer.device();
        let swapchain = renderer.swapchain.lock();

        let overdraw = match (
            rustix_render::shader::builtin::debug::debug_vertex_shader_override(device.logical()),
            rustix_render::shader::builtin::debug::debug_overdraw_fragment_shader_override(device.logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match DebugOverdrawPipeline::create(device, &swapchain, &vs, &fs) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        tracing::error!("overdraw pipeline creation failed: {e}");
                        None
                    }
                }
            }
            (Err(e), _) => {
                tracing::error!("debug vertex shader compile failed: {e}");
                None
            }
            (_, Err(e)) => {
                tracing::error!("debug overdraw fragment shader compile failed: {e}");
                None
            }
        };

        let light_complexity = match (
            rustix_render::shader::builtin::debug::debug_vertex_shader_override(device.logical()),
            rustix_render::shader::builtin::debug::debug_light_complexity_fragment_shader_override(device.logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match DebugLightComplexityPipeline::create(device, &swapchain, &vs, &fs) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        tracing::error!("light complexity pipeline creation failed: {e}");
                        None
                    }
                }
            }
            (Err(e), _) => {
                tracing::error!("debug vertex shader compile failed: {e}");
                None
            }
            (_, Err(e)) => {
                tracing::error!("debug light complexity fragment shader compile failed: {e}");
                None
            }
        };

        let overdraw_desc_set = if let Some(ref p) = overdraw {
            renderer.allocate_descriptor_set(p.desc_layout).unwrap_or(vk::DescriptorSet::null())
        } else {
            vk::DescriptorSet::null()
        };

        let light_complexity_desc_set = if let Some(ref p) = light_complexity {
            renderer.allocate_descriptor_set(p.desc_layout).unwrap_or(vk::DescriptorSet::null())
        } else {
            vk::DescriptorSet::null()
        };

        drop(swapchain);

        Ok(Self {
            overdraw_pipeline: overdraw,
            overdraw_desc_set,
            light_complexity_pipeline: light_complexity,
            light_complexity_desc_set,
        })
    }

    pub fn update_overdraw_descriptor_set(
        &self,
        renderer: &Renderer,
        scene_view: vk::ImageView,
        sampler: vk::Sampler,
    ) {
        if self.overdraw_desc_set == vk::DescriptorSet::null() {
            return;
        }
        let scene_ii = [vk::DescriptorImageInfo::default()
            .image_view(scene_view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let sampler_ii = [vk::DescriptorImageInfo::default()
            .sampler(sampler)];
        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(self.overdraw_desc_set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&scene_ii),
            vk::WriteDescriptorSet::default()
                .dst_set(self.overdraw_desc_set)
                .dst_binding(2)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .image_info(&sampler_ii),
        ];
        unsafe {
            renderer.device().logical().update_descriptor_sets(&writes, &[]);
        }
    }

    pub fn update_light_complexity_descriptor_set(
        &self,
        renderer: &Renderer,
        scene_view: vk::ImageView,
        sampler: vk::Sampler,
    ) {
        if self.light_complexity_desc_set == vk::DescriptorSet::null() {
            return;
        }
        let scene_ii = [vk::DescriptorImageInfo::default()
            .image_view(scene_view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let sampler_ii = [vk::DescriptorImageInfo::default()
            .sampler(sampler)];
        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(self.light_complexity_desc_set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&scene_ii),
            vk::WriteDescriptorSet::default()
                .dst_set(self.light_complexity_desc_set)
                .dst_binding(2)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .image_info(&sampler_ii),
        ];
        unsafe {
            renderer.device().logical().update_descriptor_sets(&writes, &[]);
        }
    }

    pub fn draw_overdraw(
        &self,
        cmd: vk::CommandBuffer,
        renderer: &Renderer,
        scene_view: vk::ImageView,
        sampler: vk::Sampler,
    ) {
        let Some(ref pipeline) = self.overdraw_pipeline else { return };
        self.update_overdraw_descriptor_set(renderer, scene_view, sampler);
        unsafe {
            let device = renderer.device().logical();
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.layout,
                0,
                &[self.overdraw_desc_set],
                &[],
            );
            let color = [0.1f32, 0.0, 0.0, 1.0];
            device.cmd_push_constants(
                cmd,
                pipeline.layout,
                vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&color),
            );
            device.cmd_draw(cmd, 3, 1, 0, 0);
        }
    }

    pub fn draw_light_complexity(
        &self,
        cmd: vk::CommandBuffer,
        renderer: &Renderer,
        scene_view: vk::ImageView,
        sampler: vk::Sampler,
        avg_light_count: f32,
    ) {
        let Some(ref pipeline) = self.light_complexity_pipeline else { return };
        self.update_light_complexity_descriptor_set(renderer, scene_view, sampler);
        unsafe {
            let device = renderer.device().logical();
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.layout,
                0,
                &[self.light_complexity_desc_set],
                &[],
            );
            let params = [avg_light_count, 0.0f32, 0.0, 0.0];
            device.cmd_push_constants(
                cmd,
                pipeline.layout,
                vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&params),
            );
            device.cmd_draw(cmd, 3, 1, 0, 0);
        }
    }
}
