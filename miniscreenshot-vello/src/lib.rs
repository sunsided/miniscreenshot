//! Vello screenshot helper for the miniscreenshot ecosystem.
//!
//! This crate re-exports `vello` to avoid version conflicts, and provides
//! a placeholder for converting Vello render output into a [`Screenshot`].
//!
//! Vello itself is a GPU-accelerated vector renderer; pixel readback is
//! performed through a `wgpu` texture. Pair this crate with
//! `miniscreenshot-wgpu` for a full capture pipeline.
//!
//! # Usage
//!
//! ```toml
//! [dependencies]
//! miniscreenshot-vello = "0.2"
//! ```
//!
//! Then access the re-exported crate:
//!
//! ```rust,ignore
//! use miniscreenshot_vello::vello;
//! ```

pub use miniscreenshot::{Capture, CaptureError, Screenshot};

/// Re-export of `vello`.
///
/// Use this instead of a direct `vello` dependency to avoid version
/// mismatches.
pub use vello;

/// Error returned when capturing from a Vello-rendered texture fails.
#[derive(Debug)]
pub enum VelloCaptureError {
    /// The capture is not yet implemented.
    NotImplemented,
}

impl std::fmt::Display for VelloCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VelloCaptureError::NotImplemented => {
                write!(f, "Vello capture is not yet implemented")
            }
        }
    }
}

impl std::error::Error for VelloCaptureError {}

impl From<VelloCaptureError> for CaptureError {
    fn from(e: VelloCaptureError) -> Self {
        match e {
            VelloCaptureError::NotImplemented => CaptureError::new(
                miniscreenshot::CaptureErrorKind::Backend,
                "Vello capture is not yet implemented",
            )
            .with_source(VelloCaptureError::NotImplemented),
        }
    }
}

/// Borrowed view over a Vello-rendered texture that implements [`Capture`].
///
/// # Stub
///
/// This is a placeholder wrapper. Use `miniscreenshot-wgpu::capture` with the
/// texture produced by Vello for a full capture pipeline.
pub struct VelloCapture<'a> {
    device: &'a vello::wgpu::Device,
    queue: &'a vello::wgpu::Queue,
    texture: &'a vello::wgpu::Texture,
}

impl<'a> VelloCapture<'a> {
    /// Create a new capture helper.
    pub fn new(
        device: &'a vello::wgpu::Device,
        queue: &'a vello::wgpu::Queue,
        texture: &'a vello::wgpu::Texture,
    ) -> Self {
        Self {
            device,
            queue,
            texture,
        }
    }
}

impl Capture for VelloCapture<'_> {
    type Error = VelloCaptureError;

    fn capture(&mut self) -> Result<Screenshot, VelloCaptureError> {
        capture(self.device, self.queue, self.texture)
    }
}

/// Capture a screenshot from a Vello-rendered texture.
///
/// # Errors
///
/// Returns [`VelloCaptureError::NotImplemented`] as the Vello-specific
/// capture logic is not yet implemented. Use
/// `miniscreenshot-wgpu::capture` with the Vello output texture instead.
pub fn capture(
    _device: &vello::wgpu::Device,
    _queue: &vello::wgpu::Queue,
    _texture: &vello::wgpu::Texture,
) -> Result<Screenshot, VelloCaptureError> {
    // TODO: Implement Vello-specific capture logic.
    // For now, return an error — use `miniscreenshot_wgpu::capture` with the Vello output texture instead.
    Err(VelloCaptureError::NotImplemented)
}
