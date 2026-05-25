use ash::vk;

use crate::instance::VulkanInstance;
use crate::RenderError;

pub fn create_surface(
    instance: &VulkanInstance,
    raw_window_handle: raw_window_handle::RawWindowHandle,
    raw_display_handle: raw_window_handle::RawDisplayHandle,
) -> Result<vk::SurfaceKHR, RenderError> {
    let surface = match (raw_display_handle, raw_window_handle) {
        (
            raw_window_handle::RawDisplayHandle::Wayland(display),
            raw_window_handle::RawWindowHandle::Wayland(window),
        ) => {
            tracing::info!("creating Wayland Vulkan surface");
            let wayland_loader =
                ash::khr::wayland_surface::Instance::new(instance.entry(), instance.inner());
            let create_info = vk::WaylandSurfaceCreateInfoKHR::default()
                .display(display.display.as_ptr())
                .surface(window.surface.as_ptr());
            unsafe {
                wayland_loader
                    .create_wayland_surface(&create_info, None)
                    .map_err(|e| RenderError::SurfaceCreation(format!("Wayland: {e}")))?
            }
        }
        (
            raw_window_handle::RawDisplayHandle::Xlib(display),
            raw_window_handle::RawWindowHandle::Xlib(window),
        ) => {
            tracing::info!("creating Xlib Vulkan surface");
            let xlib_loader =
                ash::khr::xlib_surface::Instance::new(instance.entry(), instance.inner());
            let create_info = vk::XlibSurfaceCreateInfoKHR::default()
                .dpy(display.display.map(|p| p.as_ptr()).unwrap_or(std::ptr::null_mut()) as *mut vk::Display)
                .window(window.window);
            unsafe {
                xlib_loader
                    .create_xlib_surface(&create_info, None)
                    .map_err(|e| RenderError::SurfaceCreation(format!("Xlib: {e}")))?
            }
        }
        (
            raw_window_handle::RawDisplayHandle::Xcb(display),
            raw_window_handle::RawWindowHandle::Xcb(window),
        ) => {
            tracing::info!("creating XCB Vulkan surface");
            let xcb_loader =
                ash::khr::xcb_surface::Instance::new(instance.entry(), instance.inner());
            let create_info = vk::XcbSurfaceCreateInfoKHR::default()
                .connection(display.connection.map(|p| p.as_ptr()).unwrap_or(std::ptr::null_mut()))
                .window(window.window.into());
            unsafe {
                xcb_loader
                    .create_xcb_surface(&create_info, None)
                    .map_err(|e| RenderError::SurfaceCreation(format!("XCB: {e}")))?
            }
        }
        _ => {
            return Err(RenderError::SurfaceCreation(
                "unsupported windowing backend for Vulkan surface".into(),
            ));
        }
    };

    tracing::info!(?surface, "Vulkan surface created");
    Ok(surface)
}
