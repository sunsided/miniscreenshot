//! Vello screenshot helper for the miniscreenshot ecosystem.
//!
//! This crate re-exports `vello` to avoid version conflicts, and provides
//! a [`capture`] function for converting Vello render output into a [`Screenshot`].
//!
//! Vello itself is a GPU-accelerated vector renderer; pixel readback is
//! performed through a `wgpu` texture. This implementation delegates to
//! [`miniscreenshot_wgpu::capture`] and supports the same texture formats.
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
    /// The underlying wgpu capture failed.
    Wgpu(miniscreenshot_wgpu::WgpuCaptureError),
}

impl std::fmt::Display for VelloCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VelloCaptureError::Wgpu(e) => write!(f, "wgpu capture failed: {e}"),
        }
    }
}

impl std::error::Error for VelloCaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VelloCaptureError::Wgpu(e) => Some(e),
        }
    }
}

impl From<miniscreenshot_wgpu::WgpuCaptureError> for VelloCaptureError {
    fn from(e: miniscreenshot_wgpu::WgpuCaptureError) -> Self {
        VelloCaptureError::Wgpu(e)
    }
}

impl From<VelloCaptureError> for CaptureError {
    fn from(e: VelloCaptureError) -> Self {
        match e {
            VelloCaptureError::Wgpu(inner) => CaptureError::from(inner),
        }
    }
}

/// Borrowed view over a Vello-rendered texture that implements [`Capture`].
///
/// # Implementation
///
/// This delegates to [`miniscreenshot_wgpu::capture`] under the hood,
/// so it supports the same wgpu texture formats.
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
/// This function delegates to [`miniscreenshot_wgpu::capture`] and works for
/// any wgpu texture Vello renders into. See that function's documentation
/// for the list of supported texture formats.
///
/// # Errors
///
/// Returns [`VelloCaptureError::Wgpu`] if the underlying capture fails.
pub fn capture(
    device: &vello::wgpu::Device,
    queue: &vello::wgpu::Queue,
    texture: &vello::wgpu::Texture,
) -> Result<Screenshot, VelloCaptureError> {
    miniscreenshot_wgpu::capture(device, queue, texture).map_err(VelloCaptureError::Wgpu)
}
