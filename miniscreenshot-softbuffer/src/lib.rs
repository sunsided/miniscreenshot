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
//! # Example
//!
//! ```rust,no_run
//! use miniscreenshot_softbuffer::screenshot_from_xrgb;
//!
//! // Obtain XRGB pixels from a softbuffer Buffer somehow:
//! let pixels: Vec<u32> = vec![0x00FF0000u32; 4]; // 2×2 solid red
//! let shot = screenshot_from_xrgb(&pixels, 2, 2);
//! shot.save("screenshot.png").unwrap();
//! ```

/// Re-export of the `softbuffer` crate.
///
/// Depending on `miniscreenshot-softbuffer` instead of `softbuffer` directly
/// guarantees version compatibility across the workspace.
pub use softbuffer;

pub use miniscreenshot::{Screenshot, ScreenshotProvider};

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
pub fn screenshot_from_xrgb(pixels: &[u32], width: u32, height: u32) -> Screenshot {
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
pub fn screenshot_from_argb(pixels: &[u32], width: u32, height: u32) -> Screenshot {
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
        let shot = screenshot_from_xrgb(&pixels, 2, 2);
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
        let shot = screenshot_from_xrgb(&pixels, 1, 1);
        assert_eq!(shot.data()[3], 255); // alpha is always 255
    }

    #[test]
    fn argb_preserves_alpha() {
        let pixels = vec![0x80FF0000u32]; // A=128, R=255, G=0, B=0
        let shot = screenshot_from_argb(&pixels, 1, 1);
        assert_eq!(shot.data(), &[255, 0, 0, 128]);
    }

    #[test]
    #[should_panic]
    fn xrgb_wrong_size_panics() {
        screenshot_from_xrgb(&[0u32; 5], 2, 2);
    }
}
