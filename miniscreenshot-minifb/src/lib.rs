//! Screenshot integration for the [`minifb`] windowing library.
//!
//! This crate provides a [`capture`] free function and [`MinifbCapture`],
//! a borrowed view over a minifb pixel buffer that implements [`Capture`].
//!
//! # Example
//!
//! ```rust
//! use miniscreenshot_minifb::capture;
//!
//! let buffer: Vec<u32> = vec![0u32; 800 * 600];
//! let shot = capture(&buffer, 800, 600).unwrap();
//! // shot.save("out.png")?;
//! ```

use std::fmt;

/// Re-export of the `minifb` crate.
///
/// Depending on `miniscreenshot-minifb` instead of `minifb` directly
/// guarantees version compatibility across the workspace.
pub use minifb;

pub use miniscreenshot::{Capture, CaptureError, Screenshot};

/// Pixel layout used by a minifb buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinifbPixelFormat {
    /// 0RGB8888 — minifb's default (high byte ignored, alpha forced to 255).
    ZeroRgb,
    /// ARGB8888 — when the caller packs alpha in the high byte themselves.
    Argb,
}

/// Error returned when the supplied buffer does not match the declared
/// dimensions.
#[derive(Debug)]
pub enum MinifbCaptureError {
    DimensionMismatch { expected: usize, actual: usize },
}

impl fmt::Display for MinifbCaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MinifbCaptureError::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "dimension mismatch: expected {expected} pixels, got {actual}"
                )
            }
        }
    }
}

impl std::error::Error for MinifbCaptureError {}

impl From<MinifbCaptureError> for CaptureError {
    fn from(e: MinifbCaptureError) -> Self {
        match e {
            MinifbCaptureError::DimensionMismatch { expected, actual } => CaptureError::new(
                miniscreenshot::CaptureErrorKind::Unsupported,
                format!("dimension mismatch: expected {expected} pixels, got {actual}"),
            )
            .with_source(MinifbCaptureError::DimensionMismatch { expected, actual }),
        }
    }
}

/// Borrowed view over a minifb pixel buffer that implements [`Capture`].
pub struct MinifbCapture<'a> {
    pixels: &'a [u32],
    width: u32,
    height: u32,
    format: MinifbPixelFormat,
}

/// Convert a minifb pixel buffer into a [`Screenshot`].
///
/// Minifb stores pixels as `u32` in **0RGB8888** format by default.
///
/// # Arguments
///
/// * `pixels` – Pixel buffer from `minifb`. Length must equal `width × height`.
/// * `width`  – Buffer width in pixels.
/// * `height` – Buffer height in pixels.
///
/// # Errors
///
/// Returns [`MinifbCaptureError::DimensionMismatch`] if the buffer length
/// does not match `width × height`.
pub fn capture(pixels: &[u32], width: u32, height: u32) -> Result<Screenshot, MinifbCaptureError> {
    capture_with_format(pixels, width, height, MinifbPixelFormat::ZeroRgb)
}

/// Convert a minifb pixel buffer into a [`Screenshot`] with an explicit format.
pub fn capture_with_format(
    pixels: &[u32],
    width: u32,
    height: u32,
    format: MinifbPixelFormat,
) -> Result<Screenshot, MinifbCaptureError> {
    let expected = width as usize * height as usize;
    if pixels.len() != expected {
        return Err(MinifbCaptureError::DimensionMismatch {
            expected,
            actual: pixels.len(),
        });
    }

    let rgba: Vec<u8> = pixels
        .iter()
        .flat_map(|&p| match format {
            MinifbPixelFormat::ZeroRgb => {
                let r = ((p >> 16) & 0xFF) as u8;
                let g = ((p >> 8) & 0xFF) as u8;
                let b = (p & 0xFF) as u8;
                [r, g, b, 255u8]
            }
            MinifbPixelFormat::Argb => {
                let a = ((p >> 24) & 0xFF) as u8;
                let r = ((p >> 16) & 0xFF) as u8;
                let g = ((p >> 8) & 0xFF) as u8;
                let b = (p & 0xFF) as u8;
                [r, g, b, a]
            }
        })
        .collect();

    Ok(Screenshot::from_rgba(width, height, rgba))
}

impl<'a> MinifbCapture<'a> {
    /// Create a new capture helper using the default 0RGB8888 format.
    pub fn new(pixels: &'a [u32], width: u32, height: u32) -> Self {
        Self::with_format(pixels, width, height, MinifbPixelFormat::ZeroRgb)
    }

    /// Create a new capture helper with an explicit pixel format.
    pub fn with_format(
        pixels: &'a [u32],
        width: u32,
        height: u32,
        format: MinifbPixelFormat,
    ) -> Self {
        Self {
            pixels,
            width,
            height,
            format,
        }
    }
}

impl Capture for MinifbCapture<'_> {
    type Error = CaptureError;

    fn capture(&mut self) -> Result<Screenshot, CaptureError> {
        capture_with_format(self.pixels, self.width, self.height, self.format)
            .map_err(CaptureError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_rgb_pure_red() {
        let pixels = vec![0x00FF0000u32; 4];
        let shot = capture(&pixels, 2, 2).unwrap();
        assert_eq!(shot.width(), 2);
        assert_eq!(shot.height(), 2);
        for chunk in shot.data().chunks_exact(4) {
            assert_eq!(chunk, &[255, 0, 0, 255]);
        }
    }

    #[test]
    fn zero_rgb_high_byte_ignored() {
        let pixels = vec![0xFF000000u32];
        let shot = capture(&pixels, 1, 1).unwrap();
        assert_eq!(shot.data(), &[0, 0, 0, 255]);
    }

    #[test]
    fn zero_rgb_mixed_colors() {
        let pixels = vec![0x00FF0000u32, 0x0000FF00u32, 0x000000FFu32, 0x00FFFFFFu32];
        let shot = capture(&pixels, 2, 2).unwrap();
        assert_eq!(shot.data().len(), 16);
        assert_eq!(&shot.data()[0..4], &[255, 0, 0, 255]);
        assert_eq!(&shot.data()[4..8], &[0, 255, 0, 255]);
        assert_eq!(&shot.data()[8..12], &[0, 0, 255, 255]);
        assert_eq!(&shot.data()[12..16], &[255, 255, 255, 255]);
    }

    #[test]
    fn argb_preserves_alpha() {
        let pixels = vec![0x80FF0000u32, 0xFF00FF00u32, 0x400000FFu32, 0x00FFFFFFu32];
        let shot = capture_with_format(&pixels, 2, 2, MinifbPixelFormat::Argb).unwrap();
        assert_eq!(&shot.data()[0..4], &[255, 0, 0, 128]);
        assert_eq!(&shot.data()[4..8], &[0, 255, 0, 255]);
        assert_eq!(&shot.data()[8..12], &[0, 0, 255, 64]);
        assert_eq!(&shot.data()[12..16], &[255, 255, 255, 0]);
    }

    #[test]
    fn dimension_mismatch_returns_error() {
        let pixels = [0u32; 5];
        let result = capture(&pixels, 2, 2);
        assert!(matches!(
            result,
            Err(MinifbCaptureError::DimensionMismatch {
                expected: 4,
                actual: 5
            })
        ));
    }

    #[test]
    fn struct_impls_capture() {
        let pixels = vec![0x00FF0000u32; 4];
        let mut cap = MinifbCapture::new(&pixels, 2, 2);
        let shot = cap.capture().unwrap();
        assert_eq!(shot.width(), 2);
        assert_eq!(shot.height(), 2);
    }
}
