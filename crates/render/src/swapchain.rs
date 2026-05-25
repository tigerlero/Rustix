use ash::vk;

use crate::device::GpuDevice;
use crate::instance::VulkanInstance;
use crate::RenderError;

pub struct Swapchain {
    swapchain: Option<vk::SwapchainKHR>,
    loader: Option<ash::khr::swapchain::Device>,
    surface: Option<vk::SurfaceKHR>,
    surface_loader: Option<ash::khr::surface::Instance>,
    extent: vk::Extent2D,
    format: vk::Format,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    current_image_index: usize,
    image_count: u32,
}

impl Swapchain {
    pub fn new() -> Self {
        Self {
            swapchain: None,
            loader: None,
            surface: None,
            surface_loader: None,
            extent: vk::Extent2D { width: 1, height: 1 },
            format: vk::Format::B8G8R8A8_UNORM,
            images: Vec::new(),
            image_views: Vec::new(),
            image_available_semaphores: Vec::new(),
            render_finished_semaphores: Vec::new(),
            current_image_index: 0,
            image_count: 3,
        }
    }

    pub fn init(
        &mut self,
        instance: &VulkanInstance,
        device: &GpuDevice,
        surface: vk::SurfaceKHR,
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        self.surface = Some(surface);
        self.surface_loader = Some(instance.surface_loader());
        self.create_swapchain(instance, device, width, height)?;
        self.create_sync_objects(device)?;
        Ok(())
    }

    fn create_swapchain(
        &mut self,
        instance: &VulkanInstance,
        device: &GpuDevice,
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        let surface = self.surface.ok_or_else(|| {
            RenderError::SwapchainCreation("no surface".into())
        })?;

        let surface_loader = self.surface_loader.as_ref().unwrap();

        if let (Some(old), Some(ref old_loader)) = (self.swapchain.take(), &self.loader) {
            unsafe { old_loader.destroy_swapchain(old, None); }
        }

        let loader =
            ash::khr::swapchain::Device::new(instance.inner(), device.logical());

        let capabilities = unsafe {
            surface_loader.get_physical_device_surface_capabilities(
                device.physical(),
                surface,
            )
            .map_err(|e| RenderError::SwapchainCreation(format!("caps: {e}")))?
        };

        let formats = unsafe {
            surface_loader.get_physical_device_surface_formats(
                device.physical(),
                surface,
            )
            .map_err(|e| RenderError::SwapchainCreation(format!("formats: {e}")))?
        };

        let present_modes = unsafe {
            surface_loader.get_physical_device_surface_present_modes(
                device.physical(),
                surface,
            )
            .map_err(|e| RenderError::SwapchainCreation(format!("present modes: {e}")))?
        };

        let (format, _color_space) = choose_surface_format(&formats);
        let present_mode = choose_present_mode(&present_modes);
        let extent = choose_extent(&capabilities, width, height);

        let image_count = capabilities.min_image_count.max(3).min(
            if capabilities.max_image_count > 0 {
                capabilities.max_image_count
            } else {
                3
            },
        );
        let image_count = image_count.max(3);

        let queue_family_indices = device.queue_families().unique_indices();

        let mut create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(format)
            .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true);

        if queue_family_indices.len() > 1 {
            create_info = create_info
                .image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&queue_family_indices);
        } else {
            create_info = create_info.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
        }

        // Destroy old image views and sync objects
        self.destroy_views_and_sync(device);

        let swapchain = unsafe {
            loader
                .create_swapchain(&create_info, None)
                .map_err(|e| RenderError::SwapchainCreation(format!("create: {e}")))?
        };

        let images = unsafe {
            loader
                .get_swapchain_images(swapchain)
                .map_err(|e| RenderError::SwapchainCreation(format!("get images: {e}")))?
        };

        let image_views: Vec<vk::ImageView> = images
            .iter()
            .map(|&image| {
                let view_info = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                unsafe {
                    device.logical().create_image_view(&view_info, None)
                        .expect("failed to create image view")
                }
            })
            .collect();

        self.loader = Some(loader);
        self.swapchain = Some(swapchain);
        self.format = format;
        self.extent = extent;
        self.images = images;
        self.image_views = image_views;
        self.image_count = image_count;

        tracing::info!(
            image_count = self.images.len(),
            format = ?format,
            present_mode = ?present_mode,
            extent = ?extent,
            "swapchain created"
        );

        Ok(())
    }

    fn destroy_views_and_sync(&self, device: &GpuDevice) {
        unsafe {
            for &view in &self.image_views {
                device.logical().destroy_image_view(view, None);
            }
            for &sem in &self.image_available_semaphores {
                device.logical().destroy_semaphore(sem, None);
            }
            for &sem in &self.render_finished_semaphores {
                device.logical().destroy_semaphore(sem, None);
            }
        }
    }

    fn create_sync_objects(&mut self, device: &GpuDevice) -> Result<(), RenderError> {
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let num_frames = self.image_count as usize;

        self.image_available_semaphores = (0..num_frames)
            .map(|_| unsafe {
                device.logical().create_semaphore(&semaphore_info, None)
                    .expect("failed to create semaphore")
            })
            .collect();
        self.render_finished_semaphores = (0..num_frames)
            .map(|_| unsafe {
                device.logical().create_semaphore(&semaphore_info, None)
                    .expect("failed to create semaphore")
            })
            .collect();
        Ok(())
    }

    pub fn acquire_next_image(&mut self, _device: &GpuDevice, frame_index: usize) -> Result<bool, RenderError> {
        let sem_idx = frame_index % self.image_count as usize;
        let semaphore = self.image_available_semaphores[sem_idx];
        let result = unsafe {
            self.loader.as_ref().unwrap().acquire_next_image(
                self.swapchain.unwrap(),
                u64::MAX,
                semaphore,
                vk::Fence::null(),
            )
        };
        match result {
            Ok((index, _suboptimal)) => {
                self.current_image_index = index as usize;
                Ok(true)
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(false),
            Err(e) => Err(RenderError::Vulkan(e)),
        }
    }

    pub fn present(
        &mut self,
        device: &GpuDevice,
        wait_semaphores: &[vk::Semaphore],
    ) -> Result<bool, RenderError> {
        let swapchain = self.swapchain.unwrap();
        let image_index = self.current_image_index as u32;

        let swapchains = [swapchain];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        let result = unsafe {
            self.loader
                .as_ref()
                .unwrap()
                .queue_present(device.present_queue(), &present_info)
        };
        match result {
            Ok(true) | Ok(false) | Err(vk::Result::SUBOPTIMAL_KHR) => Ok(true),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(false),
            Err(e) => Err(RenderError::Vulkan(e)),
        }
    }

    pub fn recreate(
        &mut self,
        instance: &VulkanInstance,
        device: &GpuDevice,
    ) -> Result<(), RenderError> {
        unsafe { device.logical().device_wait_idle()?; }
        let size = self.extent;
        let _ = device;
        self.create_swapchain(instance, device, size.width, size.height)
    }

    pub fn extent(&self) -> vk::Extent2D { self.extent }
    pub fn format(&self) -> vk::Format { self.format }
    pub const fn image_count(&self) -> u32 { self.image_count }
    pub fn current_image_view(&self) -> vk::ImageView {
        self.image_views[self.current_image_index]
    }
    pub fn image_available_semaphore(&self, index: usize) -> vk::Semaphore {
        self.image_available_semaphores[index % self.image_available_semaphores.len()]
    }
    pub fn render_finished_semaphore(&self, index: usize) -> vk::Semaphore {
        self.render_finished_semaphores[index % self.render_finished_semaphores.len()]
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
    }
}

impl Swapchain {
    pub fn destroy(&mut self, device: &GpuDevice) {
        unsafe { device.logical().device_wait_idle().ok(); }
        self.destroy_views_and_sync(device);
        if let (Some(swapchain), Some(ref loader)) = (self.swapchain.take(), &self.loader) {
            unsafe { loader.destroy_swapchain(swapchain, None); }
        }
        self.loader = None;
        self.swapchain = None;
        self.images.clear();
    }
}

fn choose_surface_format(formats: &[vk::SurfaceFormatKHR]) -> (vk::Format, vk::ColorSpaceKHR) {
    for fmt in formats {
        if fmt.format == vk::Format::B8G8R8A8_SRGB
            && fmt.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        {
            return (fmt.format, fmt.color_space);
        }
    }
    for fmt in formats {
        if fmt.format == vk::Format::B8G8R8A8_UNORM {
            return (fmt.format, fmt.color_space);
        }
    }
    (formats[0].format, formats[0].color_space)
}

fn choose_present_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    if modes.contains(&vk::PresentModeKHR::MAILBOX) {
        return vk::PresentModeKHR::MAILBOX;
    }
    if modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
        return vk::PresentModeKHR::IMMEDIATE;
    }
    vk::PresentModeKHR::FIFO
}

fn choose_extent(
    capabilities: &vk::SurfaceCapabilitiesKHR,
    width: u32,
    height: u32,
) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::MAX {
        capabilities.current_extent
    } else {
        vk::Extent2D {
            width: width.clamp(
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
            ),
            height: height.clamp(
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
            ),
        }
    }
}
