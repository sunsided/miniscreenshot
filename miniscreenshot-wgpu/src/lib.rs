//! Screenshot integration for the [`wgpu`] graphics API.
//!
//! This crate re-exports the `wgpu` crate (ensuring version consistency
//! across the workspace) and provides [`capture_texture`], a synchronous
//! utility for reading a GPU texture back to CPU memory and converting it
//! into a [`Screenshot`].
//!
//! # Re-export
//!
//! ```rust,no_run
//! use miniscreenshot_wgpu::wgpu;
//! ```
//!
//! # How it works
//!
//! 1. A staging `Buffer` is created with `COPY_DST | MAP_READ` usage.
//! 2. A `copy_texture_to_buffer` command is encoded and submitted.
//! 3. The device is polled to completion (blocking).
//! 4. The staging buffer is mapped, row padding is stripped, and the pixel
//!    data is converted to RGBA8 if necessary.

/// Re-export of the `wgpu` crate.
///
/// Depending on `miniscreenshot-wgpu` instead of `wgpu` directly guarantees
/// version compatibility across the workspace.
pub use wgpu;

pub use miniscreenshot::{Capture, Screenshot};

// ── Public API ────────────────────────────────────────────────────────────────

/// Errors that can occur while capturing a GPU texture.
#[derive(Debug)]
pub enum CaptureError {
    /// The texture format is not yet supported.
    ///
    /// Supported formats: `Rgba8Unorm`, `Rgba8UnormSrgb`, `Bgra8Unorm`,
    /// `Bgra8UnormSrgb`.
    UnsupportedFormat(wgpu::TextureFormat),

    /// The staging buffer could not be mapped.
    MapFailed(wgpu::BufferAsyncError),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat(fmt) => {
                write!(f, "unsupported texture format for screenshot: {fmt:?}")
            }
            Self::MapFailed(e) => write!(f, "staging buffer map failed: {e}"),
        }
    }
}

impl std::error::Error for CaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MapFailed(e) => Some(e),
            _ => None,
        }
    }
}

/// Capture a screenshot from a wgpu [`Texture`](wgpu::Texture) synchronously.
///
/// The texture must have been created with [`wgpu::TextureUsages::COPY_SRC`].
///
/// # Supported texture formats
///
/// | Format | Behaviour |
/// |--------|-----------|
/// | `Rgba8Unorm` / `Rgba8UnormSrgb` | Used directly |
/// | `Bgra8Unorm` / `Bgra8UnormSrgb` | Channels reordered to RGBA |
///
/// All other formats return [`CaptureError::UnsupportedFormat`].
///
/// # Blocking behaviour
///
/// This function calls [`wgpu::Device::poll`] with [`wgpu::Maintain::Wait`],
/// which blocks the current thread until the GPU work is complete.
pub fn capture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Result<Screenshot, CaptureError> {
    let size = texture.size();
    let width = size.width;
    let height = size.height;
    let format = texture.format();

    // Determine whether channel swapping (BGRA → RGBA) is needed.
    let is_bgra = match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
        _ => return Err(CaptureError::UnsupportedFormat(format)),
    };

    let bytes_per_row = padded_bytes_per_row(width);
    let buffer_size = u64::from(bytes_per_row) * u64::from(height);

    // Create a staging buffer on the CPU side.
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("miniscreenshot_staging_buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Encode the GPU→CPU copy.
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("miniscreenshot_encoder"),
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        size,
    );
    queue.submit(std::iter::once(encoder.finish()));

    // Map the buffer and wait for completion.
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .expect("map_async callback channel closed unexpectedly")
        .map_err(CaptureError::MapFailed)?;

    // Strip row padding and optionally swap BGRA → RGBA.
    let mapped = buffer_slice.get_mapped_range();
    let raw: &[u8] = &mapped;
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for row_idx in 0..height as usize {
        let row_start = row_idx * bytes_per_row as usize;
        let row_end = row_start + width as usize * 4;
        let row = &raw[row_start..row_end];
        if is_bgra {
            for pixel in row.chunks_exact(4) {
                rgba.push(pixel[2]); // R  ← was B
                rgba.push(pixel[1]); // G
                rgba.push(pixel[0]); // B  ← was R
                rgba.push(pixel[3]); // A
            }
        } else {
            rgba.extend_from_slice(row);
        }
    }
    drop(mapped);
    staging_buffer.unmap();

    Ok(Screenshot::from_rgba(width, height, rgba))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Round `width * 4` (bytes per row in RGBA8) up to the next multiple of
/// [`wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`] (256 bytes).
fn padded_bytes_per_row(width: u32) -> u32 {
    let unpadded = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    unpadded.div_ceil(align) * align
}

#[cfg(test)]
mod tests {
    use super::padded_bytes_per_row;

    #[test]
    fn padding_aligns_to_256() {
        // 1 pixel → 4 bytes → padded to 256
        assert_eq!(padded_bytes_per_row(1), 256);
        // 64 pixels → 256 bytes → already aligned
        assert_eq!(padded_bytes_per_row(64), 256);
        // 65 pixels → 260 bytes → padded to 512
        assert_eq!(padded_bytes_per_row(65), 512);
        // 128 pixels → 512 bytes → already aligned
        assert_eq!(padded_bytes_per_row(128), 512);
    }
}
