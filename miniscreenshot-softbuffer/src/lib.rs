//! Screenshot integration for the [`softbuffer`] windowing library.
//!
//! This crate lets you capture a [`Screenshot`] from any raw XRGB pixel
//! buffer produced by softbuffer, and re-exports the entire `softbuffer`
//! crate so that downstream consumers can rely on a single version.
//!
//! # Re-export
//!
//! ```rust
//! // Use the bundled softbuffer — avoids version conflicts.
//! use miniscreenshot_softbuffer::softbuffer;
//! ```
//!
//! # Winit Integration
//!
//! Enable the `winit` feature to re-export `winit` alongside `softbuffer`.
//! This pairs naturally with `softbuffer::Surface<Rc<Window>, Rc<Window>>`.
//!
//! ```toml
//! [dependencies]
//! miniscreenshot-softbuffer = { version = "0.1", features = ["winit"] }
//! ```

/// Re-export of the `softbuffer` crate.
///
/// Depending on `miniscreenshot-softbuffer` instead of `softbuffer` directly
/// guarantees version compatibility across the workspace.
pub use softbuffer;

#[cfg(feature = "winit")]
/// Re-export of the `winit` crate.
///
/// Enables pairing `winit::window::Window` with `softbuffer::Surface`
/// without pulling in a conflicting winit version.
pub use winit;

pub use miniscreenshot::{Capture, CaptureError, Screenshot};

use std::fmt;

/// Pixel layout used by softbuffer pixel data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftbufferPixelFormat {
    /// XRGB8888 — high byte unused, alpha forced to 255.
    Xrgb,
    /// ARGB8888 — alpha packed in the high byte.
    Argb,
}

/// Error returned when capturing from a softbuffer pixel buffer fails.
#[derive(Debug)]
pub enum SoftbufferCaptureError {
    /// The pixel count does not match the declared dimensions.
    DimensionMismatch { expected: usize, actual: usize },
}

impl fmt::Display for SoftbufferCaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoftbufferCaptureError::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "dimension mismatch: expected {expected} pixels, got {actual}"
                )
            }
        }
    }
}

impl std::error::Error for SoftbufferCaptureError {}

impl From<SoftbufferCaptureError> for CaptureError {
    fn from(e: SoftbufferCaptureError) -> Self {
        match e {
            SoftbufferCaptureError::DimensionMismatch { expected, actual } => CaptureError::new(
                miniscreenshot::CaptureErrorKind::Unsupported,
                format!("dimension mismatch: expected {expected} pixels, got {actual}"),
            )
            .with_source(SoftbufferCaptureError::DimensionMismatch { expected, actual }),
        }
    }
}

/// Borrowed view over a softbuffer pixel buffer that implements [`Capture`].
pub struct SoftbufferCapture<'a> {
    pixels: &'a [u32],
    width: u32,
    height: u32,
    format: SoftbufferPixelFormat,
}

impl<'a> SoftbufferCapture<'a> {
    /// Create a new capture helper using the default XRGB8888 format.
    pub fn new(pixels: &'a [u32], width: u32, height: u32) -> Self {
        Self::with_format(pixels, width, height, SoftbufferPixelFormat::Xrgb)
    }

    /// Create a new capture helper with an explicit pixel format.
    pub fn with_format(
        pixels: &'a [u32],
        width: u32,
        height: u32,
        format: SoftbufferPixelFormat,
    ) -> Self {
        Self {
            pixels,
            width,
            height,
            format,
        }
    }
}

impl Capture for SoftbufferCapture<'_> {
    type Error = SoftbufferCaptureError;

    fn capture(&mut self) -> Result<Screenshot, SoftbufferCaptureError> {
        if self.pixels.len() != (self.width as usize) * (self.height as usize) {
            return Err(SoftbufferCaptureError::DimensionMismatch {
                expected: (self.width as usize) * (self.height as usize),
                actual: self.pixels.len(),
            });
        }
        let rgba: Vec<u8> = self
            .pixels
            .iter()
            .flat_map(|&p| match self.format {
                SoftbufferPixelFormat::Xrgb => {
                    let r = ((p >> 16) & 0xFF) as u8;
                    let g = ((p >> 8) & 0xFF) as u8;
                    let b = (p & 0xFF) as u8;
                    [r, g, b, 255u8]
                }
                SoftbufferPixelFormat::Argb => {
                    let a = ((p >> 24) & 0xFF) as u8;
                    let r = ((p >> 16) & 0xFF) as u8;
                    let g = ((p >> 8) & 0xFF) as u8;
                    let b = (p & 0xFF) as u8;
                    [r, g, b, a]
                }
            })
            .collect();
        Ok(Screenshot::from_rgba(self.width, self.height, rgba))
    }
}

/// Convert a softbuffer pixel buffer into a [`Screenshot`].
///
/// Softbuffer stores pixels as native-endian `u32` values in **XRGB8888**
/// format: the most-significant byte is unused, followed by 8-bit R, G, B.
///
/// # Arguments
///
/// * `pixels` – Pixel buffer as returned by `Buffer::deref()` / the slice
///   you read from softbuffer. Length must equal `width × height`.
/// * `width`  – Surface width in pixels.
/// * `height` – Surface height in pixels.
///
/// # Panics
///
/// Panics if `pixels.len() != width as usize * height as usize`.
pub fn capture(pixels: &[u32], width: u32, height: u32) -> Screenshot {
    assert_eq!(
        pixels.len(),
        width as usize * height as usize,
        "pixel count must equal width × height"
    );

    // XRGB8888 → RGBA8 (alpha forced to 255)
    let rgba: Vec<u8> = pixels
        .iter()
        .flat_map(|&p| {
            let r = ((p >> 16) & 0xFF) as u8;
            let g = ((p >> 8) & 0xFF) as u8;
            let b = (p & 0xFF) as u8;
            [r, g, b, 255u8]
        })
        .collect();

    Screenshot::from_rgba(width, height, rgba)
}

/// Convert a softbuffer pixel buffer stored in **ARGB8888** format into a
/// [`Screenshot`], preserving the alpha channel.
///
/// # Panics
///
/// Panics if `pixels.len() != width as usize * height as usize`.
pub fn capture_argb(pixels: &[u32], width: u32, height: u32) -> Screenshot {
    assert_eq!(
        pixels.len(),
        width as usize * height as usize,
        "pixel count must equal width × height"
    );

    // ARGB8888 → RGBA8
    let rgba: Vec<u8> = pixels
        .iter()
        .flat_map(|&p| {
            let a = ((p >> 24) & 0xFF) as u8;
            let r = ((p >> 16) & 0xFF) as u8;
            let g = ((p >> 8) & 0xFF) as u8;
            let b = (p & 0xFF) as u8;
            [r, g, b, a]
        })
        .collect();

    Screenshot::from_rgba(width, height, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xrgb_pure_red() {
        let pixels = vec![0x00FF0000u32; 4]; // XRGB: X=0, R=255, G=0, B=0
        let shot = capture(&pixels, 2, 2);
        assert_eq!(shot.width(), 2);
        assert_eq!(shot.height(), 2);
        // Every pixel should be [R=255, G=0, B=0, A=255]
        for chunk in shot.data().chunks_exact(4) {
            assert_eq!(chunk, &[255, 0, 0, 255]);
        }
    }

    #[test]
    fn xrgb_alpha_forced_to_255() {
        let pixels = vec![0xFF000000u32]; // X byte set but should be ignored
        let shot = capture(&pixels, 1, 1);
        assert_eq!(shot.data()[3], 255); // alpha is always 255
    }

    #[test]
    fn argb_preserves_alpha() {
        let pixels = vec![0x80FF0000u32]; // A=128, R=255, G=0, B=0
        let shot = capture_argb(&pixels, 1, 1);
        assert_eq!(shot.data(), &[255, 0, 0, 128]);
    }

    #[test]
    #[should_panic]
    fn xrgb_wrong_size_panics() {
        capture(&[0u32; 5], 2, 2);
    }

    #[test]
    fn struct_capture_returns_domain_error() {
        let pixels = [0u32; 5];
        let mut cap = SoftbufferCapture::new(&pixels, 2, 2);
        let err = cap.capture().unwrap_err();
        assert!(matches!(
            err,
            SoftbufferCaptureError::DimensionMismatch {
                expected: 4,
                actual: 5
            }
        ));
    }
}
