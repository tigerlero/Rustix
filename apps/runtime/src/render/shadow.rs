use ash::vk;
use rustix_core::math::{Mat4, Vec3, Vec4};
use rustix_render::Renderer;
use rustix_render::memory::GpuBuffer;
use rustix_render::RenderError;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CsmUboData {
    pub light_view_proj: [Mat4; 3],
    pub cascade_splits: Vec4,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpotShadowUboData {
    pub view_proj: [Mat4; 4],
    pub params: [Vec4; 4], // xyz = position, w = layer index
}

/// Cascaded shadow map resources: 3 shadow maps + UBO for light matrices and splits.
pub struct CsmResources {
    pub shadow_maps: [rustix_render::GpuTexture; 3],
    pub ubo_buffer: GpuBuffer,
    pub ubo_data: CsmUboData,
    pub shadow_map_size: u32,
}

impl CsmResources {
    pub fn new(renderer: &Renderer, size: u32) -> Result<Self, RenderError> {
        let _device = renderer.device().logical();

        // Create 3 shadow maps
        let mut shadow_maps = Vec::with_capacity(3);
        for i in 0..3 {
            let sm = renderer.create_shadow_map(size)
                .map_err(|e| RenderError::DeviceCreation(format!("csm shadow map {i}: {e}")))?;
            shadow_maps.push(sm);
        }
        let shadow_maps: [rustix_render::GpuTexture; 3] = shadow_maps.try_into()
            .map_err(|_| RenderError::DeviceCreation("csm: failed to create 3 shadow maps".to_string()))?;

        // Create UBO buffer (256 bytes for alignment)
        let ubo_size = 256u64;
        let ubo_buffer = GpuBuffer::new(
            renderer.device(),
            &mut renderer.allocator.lock(),
            "csm_ubo",
            ubo_size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        // Write shadow map views to fixed bindless bindings 11-13
        let heap = renderer.bindless_heap();
        heap.write_fixed_sampled_image(11, shadow_maps[0].view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        heap.write_fixed_sampled_image(12, shadow_maps[1].view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        heap.write_fixed_sampled_image(13, shadow_maps[2].view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        // Use first shadow map's sampler for all cascades
        heap.write_fixed_sampler(14, shadow_maps[0].sampler);
        // Write UBO to binding 10
        heap.write_fixed_ubo(10, ubo_buffer.buffer, ubo_size);

        let ubo_data = CsmUboData {
            light_view_proj: [Mat4::IDENTITY; 3],
            cascade_splits: Vec4::new(10.0, 25.0, 60.0, 100.0),
        };

        Ok(Self { shadow_maps, ubo_buffer, ubo_data, shadow_map_size: size })
    }

    /// Compute cascade splits and light matrices for the current frame.
    /// `cam_view` and `cam_proj` are the camera's view and projection matrices.
    pub fn compute_cascades(&mut self, cam_view: &Mat4, cam_proj: &Mat4, light_dir: Vec3) {
        let near = 0.1f32;
        let far = 100.0f32;
        let lambda = 0.5f32;
        let num_cascades = 3usize;

        // Compute split distances using practical split scheme
        let mut splits = [0.0f32; 4];
        splits[0] = near;
        for i in 1..num_cascades {
            let t = i as f32 / num_cascades as f32;
            let log_split = near * (far / near).powf(t);
            let uniform_split = near + (far - near) * t;
            splits[i] = lambda * log_split + (1.0 - lambda) * uniform_split;
        }
        splits[num_cascades] = far;
        self.ubo_data.cascade_splits = Vec4::new(splits[1], splits[2], splits[3], 0.0);

        let inv_proj = cam_proj.inverse();
        let inv_view = cam_view.inverse();

        // Full frustum corners in view space (NDC z = 0 is near, z = 1 is far)
        let vs_corners = [
            inv_proj * Vec4::new(-1.0, -1.0, 0.0, 1.0),
            inv_proj * Vec4::new( 1.0, -1.0, 0.0, 1.0),
            inv_proj * Vec4::new( 1.0,  1.0, 0.0, 1.0),
            inv_proj * Vec4::new(-1.0,  1.0, 0.0, 1.0),
            inv_proj * Vec4::new(-1.0, -1.0, 1.0, 1.0),
            inv_proj * Vec4::new( 1.0, -1.0, 1.0, 1.0),
            inv_proj * Vec4::new( 1.0,  1.0, 1.0, 1.0),
            inv_proj * Vec4::new(-1.0,  1.0, 1.0, 1.0),
        ];

        for i in 0..num_cascades {
            let split_near = splits[i];
            let split_far = splits[i + 1];

            // Build sub-frustum corners in view space by scaling full-frustum corners
            let mut sub_vs_corners = [Vec3::ZERO; 8];
            for j in 0..4 {
                let orig = Vec3::new(vs_corners[j].x, vs_corners[j].y, vs_corners[j].z) / vs_corners[j].w;
                sub_vs_corners[j] = orig * (split_near / near);
            }
            for j in 4..8 {
                let orig = Vec3::new(vs_corners[j].x, vs_corners[j].y, vs_corners[j].z) / vs_corners[j].w;
                sub_vs_corners[j] = orig * (split_far / far);
            }

            // Transform to world space
            let mut ws_corners = [Vec3::ZERO; 8];
            for j in 0..8 {
                let ws = inv_view * Vec4::new(sub_vs_corners[j].x, sub_vs_corners[j].y, sub_vs_corners[j].z, 1.0);
                ws_corners[j] = Vec3::new(ws.x, ws.y, ws.z) / ws.w;
            }

            // Compute center and AABB in light space
            let center = ws_corners.iter().fold(Vec3::ZERO, |a, &b| a + b) / 8.0;
            let light_view = Mat4::look_at_rh(center, center - light_dir, Vec3::Y);

            let mut min_x = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            let mut min_z = f32::INFINITY;
            let mut max_z = f32::NEG_INFINITY;
            for &c in &ws_corners {
                let ls = light_view * Vec4::new(c.x, c.y, c.z, 1.0);
                let ls = Vec3::new(ls.x, ls.y, ls.z) / ls.w;
                min_x = min_x.min(ls.x);
                max_x = max_x.max(ls.x);
                min_y = min_y.min(ls.y);
                max_y = max_y.max(ls.y);
                min_z = min_z.min(ls.z);
                max_z = max_z.max(ls.z);
            }

            // Add padding to avoid edge clipping
            let padding = (max_x - min_x).max(max_y - min_y) * 0.05;
            min_x -= padding;
            max_x += padding;
            min_y -= padding;
            max_y += padding;
            // Increase Z range slightly
            let z_padding = (max_z - min_z) * 0.5;
            min_z -= z_padding;
            max_z += z_padding;

            let light_proj = Mat4::orthographic_rh_gl(min_x, max_x, min_y, max_y, min_z, max_z);
            self.ubo_data.light_view_proj[i] = light_proj * light_view;
        }
    }

    /// Upload current UBO data to GPU.
    pub fn upload_ubo(&self) {
        if let Some(ptr) = self.ubo_buffer.mapped_ptr {
            unsafe {
                let bytes = bytemuck::bytes_of(&self.ubo_data);
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            }
        }
    }
}

/// Point light shadow resources: cubemap array for up to 4 point lights.
pub struct PointShadowResources {
    pub cubemap: rustix_render::GpuTexture,
    pub face_views: Vec<vk::ImageView>,
    pub face_size: u32,
    pub max_lights: u32,
}

impl PointShadowResources {
    pub fn new(renderer: &Renderer, face_size: u32, max_lights: u32) -> Result<Self, RenderError> {
        let cubemap = renderer.create_cubemap_array_shadow(face_size, max_lights)
            .map_err(|e| RenderError::DeviceCreation(format!("point shadow cubemap: {e}")))?;
        let mut face_views = Vec::with_capacity((max_lights * 6) as usize);
        for layer in 0..max_lights * 6 {
            let view = renderer.create_layer_view(cubemap.image, vk::Format::D32_SFLOAT, layer)
                .map_err(|e| RenderError::DeviceCreation(format!("point shadow layer view {layer}: {e}")))?;
            face_views.push(view);
        }
        let heap = renderer.bindless_heap();
        heap.write_fixed_sampled_image(15, cubemap.view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        heap.write_fixed_sampler(16, cubemap.sampler);
        Ok(Self { cubemap, face_views, face_size, max_lights })
    }
}

/// Spot light shadow resources: 2D texture array for up to 4 spot lights.
pub struct SpotShadowResources {
    pub array: rustix_render::GpuTexture,
    pub layer_views: Vec<vk::ImageView>,
    pub ubo_buffer: GpuBuffer,
    pub ubo_data: SpotShadowUboData,
    pub size: u32,
    pub max_lights: u32,
}

impl SpotShadowResources {
    pub fn new(renderer: &Renderer, size: u32, max_lights: u32) -> Result<Self, RenderError> {
        let array = renderer.create_2d_array_shadow(size, max_lights)
            .map_err(|e| RenderError::DeviceCreation(format!("spot shadow array: {e}")))?;
        let mut layer_views = Vec::with_capacity(max_lights as usize);
        for layer in 0..max_lights {
            let view = renderer.create_layer_view(array.image, vk::Format::D32_SFLOAT, layer)
                .map_err(|e| RenderError::DeviceCreation(format!("spot shadow layer view {layer}: {e}")))?;
            layer_views.push(view);
        }
        let ubo_size = 256u64;
        let ubo_buffer = GpuBuffer::new(
            renderer.device(),
            &mut renderer.allocator.lock(),
            "spot_shadow_ubo",
            ubo_size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;
        let heap = renderer.bindless_heap();
        heap.write_fixed_sampled_image(17, array.view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        heap.write_fixed_sampler(18, array.sampler);
        heap.write_fixed_ubo(19, ubo_buffer.buffer, ubo_size);
        let ubo_data = SpotShadowUboData {
            view_proj: [Mat4::IDENTITY; 4],
            params: [Vec4::ZERO; 4],
        };
        Ok(Self { array, layer_views, ubo_buffer, ubo_data, size, max_lights })
    }

    pub fn upload_ubo(&self) {
        if let Some(ptr) = self.ubo_buffer.mapped_ptr {
            unsafe {
                let bytes = bytemuck::bytes_of(&self.ubo_data);
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            }
        }
    }
}
