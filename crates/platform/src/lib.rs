pub mod window;
pub mod input;

pub use window::*;
pub use input::*;

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
