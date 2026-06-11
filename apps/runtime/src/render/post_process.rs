use ash::vk;
use rustix_render::Renderer;
use rustix_render::pipeline::PostProcessPipeline;

/// Post-process stack parameters.
#[derive(Debug, Clone, Copy)]
pub struct PostProcessParams {
    pub grain_intensity: f32,
    pub chromatic_aberration: f32,
    pub vignette_intensity: f32,
    pub vignette_smoothness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub gamma: f32,
    pub tint_shadows: [f32; 4],
    pub tint_midtones: [f32; 4],
    pub tint_highlights: [f32; 4],
}

impl Default for PostProcessParams {
    fn default() -> Self {
        Self {
            grain_intensity: 0.03,
            chromatic_aberration: 0.005,
            vignette_intensity: 1.5,
            vignette_smoothness: 0.8,
            contrast: 1.0,
            saturation: 1.0,
            gamma: 2.2,
            tint_shadows: [1.0, 1.0, 1.0, 1.0],
            tint_midtones: [1.0, 1.0, 1.0, 1.0],
            tint_highlights: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// GPU resources for the post-process stack.
pub struct PostProcessResources {
    pub pipeline: PostProcessPipeline,
    pub desc_set: vk::DescriptorSet,
    pub params: PostProcessParams,
}

impl PostProcessResources {
    pub fn new(renderer: &Renderer) -> Self {
        let device = renderer.device();
        let swapchain = renderer.swapchain.lock();
        let vs = rustix_render::shader::builtin::postprocess::postprocess_vertex_shader_override(device.logical())
            .expect("postprocess vertex shader");
        let fs = rustix_render::shader::builtin::postprocess::postprocess_fragment_shader_override(device.logical())
            .expect("postprocess fragment shader");
        let pipeline = PostProcessPipeline::create(device, &swapchain, &vs, &fs)
            .expect("postprocess pipeline");
        drop(swapchain);

        let desc_set = renderer.allocate_descriptor_set(pipeline.desc_layout)
            .expect("postprocess desc set");

        Self {
            pipeline,
            desc_set,
            params: PostProcessParams::default(),
        }
    }

    pub fn update_descriptor_set(&self, renderer: &Renderer, scene_view: vk::ImageView, sampler: vk::Sampler) {
        let scene_ii = [vk::DescriptorImageInfo::default()
            .image_view(scene_view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let samp_ii = [vk::DescriptorImageInfo::default().sampler(sampler)];
        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(self.desc_set).dst_binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&scene_ii),
            vk::WriteDescriptorSet::default()
                .dst_set(self.desc_set).dst_binding(2)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .image_info(&samp_ii),
        ];
        unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    }

    pub fn draw(&self, cmd: vk::CommandBuffer, renderer: &Renderer, extent: vk::Extent2D) {
        let pc_data: [f32; 20] = [
            self.params.grain_intensity,
            self.params.chromatic_aberration,
            self.params.vignette_intensity,
            self.params.vignette_smoothness,
            self.params.contrast,
            self.params.saturation,
            self.params.gamma,
            0.0, // pad
            self.params.tint_shadows[0], self.params.tint_shadows[1], self.params.tint_shadows[2], self.params.tint_shadows[3],
            self.params.tint_midtones[0], self.params.tint_midtones[1], self.params.tint_midtones[2], self.params.tint_midtones[3],
            self.params.tint_highlights[0], self.params.tint_highlights[1], self.params.tint_highlights[2], self.params.tint_highlights[3],
        ];
        unsafe {
            let device = renderer.device().logical();
            device.cmd_set_viewport(cmd, 0, &[vk::Viewport {
                x: 0.0, y: extent.height as f32,
                width: extent.width as f32, height: -(extent.height as f32),
                min_depth: 0.0, max_depth: 1.0,
            }]);
            device.cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }]);
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline.pipeline);
            device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline.layout, 0, &[self.desc_set], &[]);
            device.cmd_push_constants(cmd, self.pipeline.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc_data));
            device.cmd_draw(cmd, 3, 1, 0, 0);
        }
    }
}
