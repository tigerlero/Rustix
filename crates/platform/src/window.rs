use winit::dpi::{LogicalSize, Size};
use winit::event_loop::EventLoop;
use winit::window::{Fullscreen, Window as WinitWindow, WindowAttributes, WindowButtons};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::PlatformError;

/// How the window should enter fullscreen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullscreenMode {
    /// Windowed mode (no fullscreen).
    Windowed,
    /// Exclusive fullscreen: acquires the display and switches video mode.
    /// On Wayland this may degrade to borderless.
    Exclusive,
    /// Borderless fullscreen windowed: fills the screen without changing
    /// the display video mode.
    Borderless,
}

impl Default for FullscreenMode {
    fn default() -> Self {
        FullscreenMode::Windowed
    }
}

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub fullscreen_mode: FullscreenMode,
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
            fullscreen_mode: FullscreenMode::Windowed,
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

        let mut handle = Self {
            inner,
            config: config.clone(),
            should_close: false,
        };

        // Apply fullscreen if requested.
        if config.fullscreen {
            handle.apply_fullscreen_mode(config.fullscreen_mode);
        }

        Ok(handle)
    }

    fn apply_fullscreen_mode(&mut self, mode: FullscreenMode) {
        match mode {
            FullscreenMode::Windowed => {
                self.inner.set_fullscreen(None);
            }
            FullscreenMode::Borderless => {
                let monitor = self.inner.current_monitor();
                let fs = Fullscreen::Borderless(monitor);
                self.inner.set_fullscreen(Some(fs));
                tracing::info!("entered borderless fullscreen");
            }
            FullscreenMode::Exclusive => {
                // Try to pick the best video mode on the current monitor.
                if let Some(monitor) = self.inner.current_monitor() {
                    let video_mode = monitor
                        .video_modes()
                        .max_by(|a, b| {
                            let area_a = a.size().width * a.size().height;
                            let area_b = b.size().width * b.size().height;
                            let refresh_a = a.refresh_rate_millihertz();
                            let refresh_b = b.refresh_rate_millihertz();
                            // Prefer larger resolution, then higher refresh
                            area_a
                                .cmp(&area_b)
                                .then_with(|| refresh_a.cmp(&refresh_b))
                        });
                    if let Some(vm) = video_mode {
                        tracing::info!(
                            resolution = ?(vm.size()),
                            refresh = vm.refresh_rate_millihertz(),
                            "entered exclusive fullscreen"
                        );
                        let fs = Fullscreen::Exclusive(vm);
                        self.inner.set_fullscreen(Some(fs));
                    } else {
                        tracing::warn!("no video modes found; falling back to borderless fullscreen");
                        let fs = Fullscreen::Borderless(Some(monitor));
                        self.inner.set_fullscreen(Some(fs));
                    }
                } else {
                    tracing::warn!("no monitor detected; cannot enter fullscreen");
                }
            }
        }
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

    /// Returns the window's current scale factor (DPI / 96).
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    /// Set the fullscreen mode at runtime.
    pub fn set_fullscreen_mode(&mut self, mode: FullscreenMode) {
        self.config.fullscreen_mode = mode;
        self.config.fullscreen = mode != FullscreenMode::Windowed;
        self.apply_fullscreen_mode(mode);
    }

    /// Query the current fullscreen mode.
    pub fn fullscreen_mode(&self) -> FullscreenMode {
        self.config.fullscreen_mode
    }

    /// Whether the window is currently in any fullscreen mode.
    pub fn is_fullscreen(&self) -> bool {
        self.inner.fullscreen().is_some()
    }

    /// Toggle between windowed and the configured fullscreen mode.
    /// Returns the new mode.
    pub fn toggle_fullscreen(&mut self) -> FullscreenMode {
        if self.is_fullscreen() {
            self.set_fullscreen_mode(FullscreenMode::Windowed);
            FullscreenMode::Windowed
        } else {
            let mode = self.config.fullscreen_mode;
            if mode == FullscreenMode::Windowed {
                self.set_fullscreen_mode(FullscreenMode::Exclusive);
                FullscreenMode::Exclusive
            } else {
                self.set_fullscreen_mode(mode);
                mode
            }
        }
    }
}

unsafe impl Send for WindowHandle {}
unsafe impl Sync for WindowHandle {}
