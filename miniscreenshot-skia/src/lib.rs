//! Skia screenshot helper for the miniscreenshot ecosystem.
//!
//! This crate re-exports `skia-safe` (when the `skia` feature is enabled) to
//! avoid version conflicts, and provides a placeholder for extracting
//! [`Screenshot`]s from Skia surfaces.
//!
//! # Enabling Skia
//!
//! Add the crate with the `skia` feature flag:
//!
//! ```toml
//! [dependencies]
//! miniscreenshot-skia = { version = "0.1", features = ["skia"] }
//! ```
//!
//! Then access the re-exported crate:
//!
//! ```rust,ignore
//! use miniscreenshot_skia::skia_safe;
//! ```

pub use miniscreenshot::{Screenshot, ScreenshotProvider};

/// Re-export of `skia-safe` when the `skia` feature is enabled.
///
/// Use this instead of a direct `skia-safe` dependency to avoid version
/// mismatches.
#[cfg(feature = "skia")]
pub use skia_safe;

/// Read RGBA8 pixel data from a `skia_safe::Surface` and return a
/// [`Screenshot`].
///
/// This function is only available when the `skia` feature is enabled.
///
/// # Example
///
/// ```rust,ignore
/// use miniscreenshot_skia::screenshot_from_surface;
///
/// let screenshot = screenshot_from_surface(&mut surface);
/// screenshot.save("output.png").unwrap();
/// ```
#[cfg(feature = "skia")]
pub fn screenshot_from_surface(surface: &mut skia_safe::Surface) -> Screenshot {
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
    surface.read_pixels(&dst_info, &mut pixels, row_bytes, skia_safe::IPoint::new(0, 0));
    Screenshot::from_rgba(width, height, pixels)
}
