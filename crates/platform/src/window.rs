use winit::dpi::{LogicalSize, Size};
use winit::event_loop::EventLoop;
use winit::window::{Window as WinitWindow, WindowAttributes, WindowButtons};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::PlatformError;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub resizable: bool,
    pub decorations: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Rustix Engine".into(),
            width: 1280,
            height: 720,
            fullscreen: false,
            resizable: true,
            decorations: true,
        }
    }
}

pub struct WindowHandle {
    inner: WinitWindow,
    config: WindowConfig,
    should_close: bool,
}

impl WindowHandle {
    #[allow(deprecated)]
    pub fn new(
        event_loop: &EventLoop<()>,
        config: &WindowConfig,
    ) -> Result<Self, PlatformError> {
        tracing::info!(
            title = %config.title,
            size = %format!("{}x{}", config.width, config.height),
            "creating window"
        );

        let attrs = WindowAttributes::default()
            .with_title(config.title.clone())
            .with_inner_size(Size::Logical(LogicalSize::new(
                f64::from(config.width),
                f64::from(config.height),
            )))
            .with_resizable(config.resizable)
            .with_decorations(config.decorations)
            .with_enabled_buttons(if config.decorations {
                WindowButtons::all()
            } else {
                WindowButtons::empty()
            });

        #[allow(deprecated)]
        let inner = event_loop
            .create_window(attrs)
            .map_err(|e| PlatformError::WindowCreation(format!("build: {e}")))?;

        tracing::info!(window_id = ?inner.id(), "window created");

        Ok(Self {
            inner,
            config: config.clone(),
            should_close: false,
        })
    }

    pub fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        self.inner
            .window_handle()
            .map(|h| h.as_raw())
            .unwrap_or(raw_window_handle::RawWindowHandle::Xlib(
                raw_window_handle::XlibWindowHandle::new(0),
            ))
    }

    pub fn raw_display_handle(&self) -> raw_window_handle::RawDisplayHandle {
        self.inner
            .display_handle()
            .map(|h| h.as_raw())
            .unwrap_or(raw_window_handle::RawDisplayHandle::Xlib(
                raw_window_handle::XlibDisplayHandle::new(None, 0),
            ))
    }

    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) {
        if let winit::event::WindowEvent::CloseRequested = event {
            self.should_close = true;
        }
    }

    pub fn should_close(&self) -> bool { self.should_close }

    pub fn request_redraw(&self) { self.inner.request_redraw(); }

    pub fn set_title(&self, title: &str) { self.inner.set_title(title); }

    pub fn physical_size(&self) -> (u32, u32) {
        let size = self.inner.inner_size();
        (size.width, size.height)
    }

    pub fn inner(&self) -> &WinitWindow { &self.inner }
    pub fn config(&self) -> &WindowConfig { &self.config }
}

unsafe impl Send for WindowHandle {}
unsafe impl Sync for WindowHandle {}
