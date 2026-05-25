use ash::vk;
use std::ffi::CStr;

use crate::RenderError;

pub struct VulkanInstance {
    pub entry: ash::Entry,
    pub inner: ash::Instance,
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    debug_utils_loader: Option<ash::ext::debug_utils::Instance>,
}

impl VulkanInstance {
    pub fn new(config: &crate::RenderConfig) -> Result<Self, RenderError> {
        let entry = unsafe {
            ash::Entry::load().map_err(|e| {
                RenderError::InstanceCreation(format!("failed to load Vulkan entry: {e}"))
            })?
        };

        let app_info = vk::ApplicationInfo::default()
            .application_name(c"Rustix Engine")
            .application_version(vk::make_api_version(0, 0, 1, 0))
            .engine_name(c"Rustix")
            .engine_version(vk::make_api_version(0, 0, 1, 0))
            .api_version(vk::API_VERSION_1_3);

        let mut extensions = vec![
            ash::khr::surface::NAME.as_ptr(),
            ash::khr::wayland_surface::NAME.as_ptr(),
            ash::khr::xlib_surface::NAME.as_ptr(),
            ash::khr::xcb_surface::NAME.as_ptr(),
        ];

        if config.enable_validation {
            extensions.push(ash::ext::debug_utils::NAME.as_ptr());
        }

        let mut validation_enabled = false;
        let layers: Vec<*const i8> = if config.enable_validation {
            let available = unsafe { entry.enumerate_instance_layer_properties() }
                .unwrap_or_default();
            let has_validation = available.iter().any(|layer| {
                let name = unsafe { CStr::from_ptr(layer.layer_name.as_ptr()) };
                name == c"VK_LAYER_KHRONOS_validation"
            });
            if has_validation {
                tracing::debug!("enabling Vulkan validation layers");
                validation_enabled = true;
                vec![c"VK_LAYER_KHRONOS_validation".as_ptr()]
            } else {
                tracing::warn!("VK_LAYER_KHRONOS_validation not found — running without validation");
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let mut create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layers);

        let mut debug_create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(vulkan_debug_callback));

        if validation_enabled {
            create_info = create_info.push_next(&mut debug_create_info);
        }

        let inner = unsafe {
            entry
                .create_instance(&create_info, None)
                .map_err(|e| RenderError::InstanceCreation(format!("vkCreateInstance: {e}")))?
        };

        let (debug_utils_loader, debug_messenger) = if validation_enabled {
            let loader = ash::ext::debug_utils::Instance::new(&entry, &inner);
            let messenger = unsafe {
                loader
                    .create_debug_utils_messenger(&debug_create_info, None)
                    .map_err(|e| {
                        RenderError::InstanceCreation(format!("DebugUtils messenger: {e}"))
                    })?
            };
            (Some(loader), Some(messenger))
        } else {
            (None, None)
        };

        tracing::info!("Vulkan instance created (API 1.3)");

        Ok(Self {
            entry,
            inner,
            debug_messenger,
            debug_utils_loader,
        })
    }

    pub fn entry(&self) -> &ash::Entry { &self.entry }
    pub fn inner(&self) -> &ash::Instance { &self.inner }
    pub fn surface_loader(&self) -> ash::khr::surface::Instance {
        ash::khr::surface::Instance::new(&self.entry, &self.inner)
    }
}

impl Drop for VulkanInstance {
    fn drop(&mut self) {
        unsafe {
            if let (Some(loader), Some(messenger)) =
                (&self.debug_utils_loader, &self.debug_messenger)
            {
                loader.destroy_debug_utils_messenger(*messenger, None);
            }
            self.inner.destroy_instance(None);
        }
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let type_str = match message_type {
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "GENERAL",
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "VALIDATION",
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "PERFORMANCE",
        _ => "UNKNOWN",
    };

    if let Some(data) = p_callback_data.as_ref() {
        if !data.p_message.is_null() {
            let msg = unsafe { CStr::from_ptr(data.p_message) };
            let msg_str = msg.to_string_lossy();
            match message_severity {
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => {
                    tracing::error!("[Vulkan][{type_str}] {msg_str}")
                }
                vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => {
                    tracing::warn!("[Vulkan][{type_str}] {msg_str}")
                }
                _ => tracing::debug!("[Vulkan][{type_str}] {msg_str}"),
            }
        }
    }

    vk::FALSE
}
