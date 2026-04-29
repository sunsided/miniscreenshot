//! Screenshot integration for the [`minifb`] windowing library.
//!
//! This crate lets you capture a [`Screenshot`] from any raw 0RGB pixel
//! buffer produced by minifb, and re-exports the entire `minifb`
//! crate so that downstream consumers can rely on a single version.
//!
//! # Re-export
//!
//! ```rust
//! // Use the bundled minifb — avoids version conflicts.
//! use miniscreenshot_minifb::minifb;
//! ```

/// Re-export of the `minifb` crate.
///
/// Depending on `miniscreenshot-minifb` instead of `minifb` directly
/// guarantees version compatibility across the workspace.
pub use minifb;

pub use miniscreenshot::{Screenshot, ScreenshotProvider};

/// Convert a minifb pixel buffer into a [`Screenshot`].
///
/// Minifb stores pixels as native-endian `u32` values in **0RGB8888**
/// format: the most-significant byte is unused (zero), followed by 8-bit R, G, B.
///
/// # Arguments
///
/// * `pixels` – Pixel buffer as passed to `Window::update_with_buffer()`.
///   Length must equal `width × height`.
/// * `width`  — Window width in pixels.
/// * `height` — Window height in pixels.
///
/// # Panics
///
/// Panics if `pixels.len() != width as usize * height as usize`.
pub fn screenshot_from_minifb(pixels: &[u32], width: u32, height: u32) -> Screenshot {
    assert_eq!(
        pixels.len(),
        width as usize * height as usize,
        "pixel count must equal width × height"
    );

    // 0RGB8888 → RGBA8 (alpha forced to 255)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_red() {
        let pixels = vec![0x00FF0000u32; 4]; // 0RGB: 0=0, R=255, G=0, B=0
        let shot = screenshot_from_minifb(&pixels, 2, 2);
        assert_eq!(shot.width(), 2);
        assert_eq!(shot.height(), 2);
        for chunk in shot.data().chunks_exact(4) {
            assert_eq!(chunk, &[255, 0, 0, 255]);
        }
    }

    #[test]
    fn high_byte_ignored() {
        let pixels = vec![0xFF000000u32]; // high byte set but should be ignored
        let shot = screenshot_from_minifb(&pixels, 1, 1);
        assert_eq!(shot.data(), &[0, 0, 0, 255]); // black with full alpha
    }

    #[test]
    fn mixed_colors() {
        let pixels = vec![
            0x00FF0000u32, // red
            0x0000FF00u32, // green
            0x000000FFu32, // blue
            0x00FFFFFFu32, // white
        ];
        let shot = screenshot_from_minifb(&pixels, 2, 2);
        assert_eq!(shot.data().len(), 16);
        // red pixel
        assert_eq!(&shot.data()[0..4], &[255, 0, 0, 255]);
        // green pixel
        assert_eq!(&shot.data()[4..8], &[0, 255, 0, 255]);
        // blue pixel
        assert_eq!(&shot.data()[8..12], &[0, 0, 255, 255]);
        // white pixel
        assert_eq!(&shot.data()[12..16], &[255, 255, 255, 255]);
    }

    #[test]
    #[should_panic]
    fn wrong_size_panics() {
        screenshot_from_minifb(&[0u32; 5], 2, 2);
    }
}
