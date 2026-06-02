import re

with open("apps/runtime/src/ui_renderer.rs", "r") as f:
    content = f.read()

content = content.replace("        if self.font_texture.is_none() {\n            tracing::warn!(\"draw_primitives: no font atlas texture bound, skipping\");\n            return;\n        }", '''        if self.font_texture.is_none() {
            tracing::warn!("draw_primitives: no font atlas texture bound, skipping");
            return;
        }

        if self.last_frame_index.get() != frame_index {
            self.last_frame_index.set(frame_index);
            let slot = frame_index % 3;
            let pool = *self.descriptor_pools[slot].lock().unwrap();
            unsafe {
                renderer.device().logical().reset_descriptor_pool(pool, vk::DescriptorPoolResetFlags::empty()).unwrap();
            }
            self.bound_texture.set(None);
            self.bound_descriptor_set.set(vk::DescriptorSet::null());
        }''')

content = content.replace("renderer.device().logical().cmd_bind_descriptor_sets(cmd,vk::PipelineBindPoint::GRAPHICS,self.pipeline_layout,0,&[self.descriptor_set],&[]);", "")

# The allocation part
old_alloc = '''                    if let Some(v) = view {
                        let img_info = [vk::DescriptorImageInfo::default()
                            .image_view(v).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
                        let writes = [vk::WriteDescriptorSet::default()
                            .dst_set(self.descriptor_set).dst_binding(0)
                            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&img_info)];
                        unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
                        self.bound_texture.set(wanted_id);
                    }'''

new_alloc = '''                    if let Some(v) = view {
                        let slot = frame_index % 3;
                        let pool = *self.descriptor_pools[slot].lock().unwrap();
                        let desc_set = unsafe {
                            let mut sets = renderer.device().logical().allocate_descriptor_sets(
                                &vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&[self.descriptor_set_layout]),
                            ).unwrap();
                            sets.remove(0)
                        };

                        let img_info = [vk::DescriptorImageInfo::default()
                            .image_view(v).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
                        let samp_info=[vk::DescriptorImageInfo::default().sampler(self.sampler)];
                        let writes = [
                            vk::WriteDescriptorSet::default()
                                .dst_set(desc_set).dst_binding(0)
                                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&img_info),
                            vk::WriteDescriptorSet::default().dst_set(desc_set).dst_binding(1)
                                .descriptor_type(vk::DescriptorType::SAMPLER).image_info(&samp_info),
                        ];
                        unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
                        
                        self.bound_texture.set(wanted_id);
                        self.bound_descriptor_set.set(desc_set);

                        unsafe {
                            renderer.device().logical().cmd_bind_descriptor_sets(
                                cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline_layout,
                                0, &[desc_set], &[]
                            );
                        }
                    }'''

content = content.replace(old_alloc, new_alloc)

with open("apps/runtime/src/ui_renderer.rs", "w") as f:
    f.write(content)
