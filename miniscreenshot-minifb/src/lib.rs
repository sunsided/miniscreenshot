//! Screenshot integration for the [`minifb`] windowing library.
//!
//! This crate provides [`MinifbCapture`], a borrowed view over a minifb
//! pixel buffer that implements [`ScreenshotProvider`].
//!
//! # Example
//!
//! ```rust
//! use miniscreenshot_minifb::{MinifbCapture, ScreenshotProvider};
//!
//! let buffer: Vec<u32> = vec![0u32; 800 * 600];
//! let mut capture = MinifbCapture::new(&buffer, 800, 600);
//! let shot = capture.take_screenshot().unwrap();
//! // shot.save("out.png")?;
//! ```

use std::fmt;

/// Re-export of the `minifb` crate.
///
/// Depending on `miniscreenshot-minifb` instead of `minifb` directly
/// guarantees version compatibility across the workspace.
pub use minifb;

pub use miniscreenshot::{Screenshot, ScreenshotProvider};

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

/// Borrowed view over a minifb pixel buffer that implements
/// [`ScreenshotProvider`].
pub struct MinifbCapture<'a> {
    pixels: &'a [u32],
    width: u32,
    height: u32,
    format: MinifbPixelFormat,
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

    fn decode(&self) -> Result<Vec<u8>, MinifbCaptureError> {
        let expected = self.width as usize * self.height as usize;
        if self.pixels.len() != expected {
            return Err(MinifbCaptureError::DimensionMismatch {
                expected,
                actual: self.pixels.len(),
            });
        }

        Ok(self
            .pixels
            .iter()
            .flat_map(|&p| match self.format {
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
            .collect())
    }
}

impl ScreenshotProvider for MinifbCapture<'_> {
    type Error = MinifbCaptureError;

    fn take_screenshot(&mut self) -> Result<Screenshot, Self::Error> {
        let rgba = self.decode()?;
        Ok(Screenshot::from_rgba(self.width, self.height, rgba))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_rgb_pure_red() {
        let pixels = vec![0x00FF0000u32; 4];
        let mut capture = MinifbCapture::new(&pixels, 2, 2);
        let shot = capture.take_screenshot().unwrap();
        assert_eq!(shot.width(), 2);
        assert_eq!(shot.height(), 2);
        for chunk in shot.data().chunks_exact(4) {
            assert_eq!(chunk, &[255, 0, 0, 255]);
        }
    }

    #[test]
    fn zero_rgb_high_byte_ignored() {
        let pixels = vec![0xFF000000u32];
        let mut capture = MinifbCapture::new(&pixels, 1, 1);
        let shot = capture.take_screenshot().unwrap();
        assert_eq!(shot.data(), &[0, 0, 0, 255]);
    }

    #[test]
    fn zero_rgb_mixed_colors() {
        let pixels = vec![0x00FF0000u32, 0x0000FF00u32, 0x000000FFu32, 0x00FFFFFFu32];
        let mut capture = MinifbCapture::new(&pixels, 2, 2);
        let shot = capture.take_screenshot().unwrap();
        assert_eq!(shot.data().len(), 16);
        assert_eq!(&shot.data()[0..4], &[255, 0, 0, 255]);
        assert_eq!(&shot.data()[4..8], &[0, 255, 0, 255]);
        assert_eq!(&shot.data()[8..12], &[0, 0, 255, 255]);
        assert_eq!(&shot.data()[12..16], &[255, 255, 255, 255]);
    }

    #[test]
    fn argb_preserves_alpha() {
        let pixels = vec![0x80FF0000u32, 0xFF00FF00u32, 0x400000FFu32, 0x00FFFFFFu32];
        let mut capture = MinifbCapture::with_format(&pixels, 2, 2, MinifbPixelFormat::Argb);
        let shot = capture.take_screenshot().unwrap();
        assert_eq!(&shot.data()[0..4], &[255, 0, 0, 128]);
        assert_eq!(&shot.data()[4..8], &[0, 255, 0, 255]);
        assert_eq!(&shot.data()[8..12], &[0, 0, 255, 64]);
        assert_eq!(&shot.data()[12..16], &[255, 255, 255, 0]);
    }

    #[test]
    fn dimension_mismatch_returns_error() {
        let pixels = [0u32; 5];
        let mut capture = MinifbCapture::new(&pixels, 2, 2);
        let result = capture.take_screenshot();
        assert!(matches!(
            result,
            Err(MinifbCaptureError::DimensionMismatch {
                expected: 4,
                actual: 5
            })
        ));
    }
}
