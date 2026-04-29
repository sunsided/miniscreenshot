//! Skia screenshot helper for the miniscreenshot ecosystem.
//!
//! This crate re-exports `skia-safe` to avoid version conflicts, and provides
//! a helper for extracting [`Screenshot`]s from Skia surfaces.
//!
//! # Usage
//!
//! ```toml
//! [dependencies]
//! miniscreenshot-skia = "0.1"
//! ```
//!
//! Then access the re-exported crate:
//!
//! ```rust,ignore
//! use miniscreenshot_skia::skia_safe;
//! ```

pub use miniscreenshot::{Capture, CaptureError, Screenshot};

/// Re-export of `skia-safe`.
///
/// Use this instead of a direct `skia-safe` dependency to avoid version
/// mismatches.
pub use skia_safe;

/// Borrowed view over a Skia [`skia_safe::Surface`] that implements [`Capture`].
///
/// The surface is borrowed mutably because `read_pixels` requires a mutable
/// reference. This means you cannot hold `SkiaCapture` across multiple frames
/// without re-constructing it — an acceptable trade-off.
pub struct SkiaCapture<'a> {
    surface: &'a mut skia_safe::Surface,
}

impl<'a> SkiaCapture<'a> {
    /// Create a new capture helper from a mutable Skia surface reference.
    pub fn new(surface: &'a mut skia_safe::Surface) -> Self {
        Self { surface }
    }
}

impl Capture for SkiaCapture<'_> {
    type Error = CaptureError;

    fn capture(&mut self) -> Result<Screenshot, CaptureError> {
        Ok(capture(self.surface))
    }
}

/// Read RGBA8 pixel data from a `skia_safe::Surface` and return a
/// [`Screenshot`].
///
/// # Example
///
/// ```rust,ignore
/// use miniscreenshot_skia::capture;
///
/// let screenshot = capture(&mut surface);
/// screenshot.save("output.png").unwrap();
/// ```
pub fn capture(surface: &mut skia_safe::Surface) -> Screenshot {
    let info = surface.image_info();
    let width = info.width() as u32;
    let height = info.height() as u32;
    let row_bytes = width as usize * 4;
    let mut pixels = vec![0u8; row_bytes * height as usize];

    let dst_info = skia_safe::ImageInfo::new(
        skia_safe::ISize::new(width as i32, height as i32),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        None,
    );
    surface.read_pixels(
        &dst_info,
        &mut pixels,
        row_bytes,
        skia_safe::IPoint::new(0, 0),
    );
    Screenshot::from_rgba(width, height, pixels)
}
