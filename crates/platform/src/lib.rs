pub mod window;
pub mod input;
pub mod gamepad;
pub mod actions;
pub mod recorder;
pub mod clipboard;

pub use window::*;
pub use input::*;
pub use gamepad::*;
pub use actions::*;
pub use recorder::*;
pub use clipboard::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("window creation failed: {0}")]
    WindowCreation(String),

    #[error("input initialization failed: {0}")]
    InputInit(String),

    #[error("surface creation failed: {0}")]
    SurfaceCreation(String),
}
