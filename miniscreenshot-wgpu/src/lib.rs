//! Screenshot integration for the [`wgpu`] graphics API.
//!
//! This crate re-exports the `wgpu` crate (ensuring version consistency
//! across the workspace) and provides [`capture`], a synchronous utility for
//! reading a GPU texture back to CPU memory and converting it into a
//! [`Screenshot`].
//!
//! # Re-export
//!
//! ```rust,no_run
//! use miniscreenshot_wgpu::wgpu;
//! ```
//!
//! # Feature selection
//!
//! Enable exactly one compatibility feature to select the `wgpu` major
//! version:
//!
//! - `wgpu-28`
//! - `wgpu-29`
//!
//! # How it works
//!
//! 1. A staging `Buffer` is created with `COPY_DST | MAP_READ` usage.
//! 2. A `copy_texture_to_buffer` command is encoded and submitted.
//! 3. The device is polled to completion (blocking).
//! 4. The staging buffer is mapped, row padding is stripped, and the pixel
//!    data is converted to RGBA8 if necessary.

#[cfg(all(feature = "wgpu-28", feature = "wgpu-29"))]
compile_error!("features `wgpu-28` and `wgpu-29` are mutually exclusive; enable exactly one");
#[cfg(not(any(feature = "wgpu-28", feature = "wgpu-29")))]
compile_error!("one of `wgpu-28` or `wgpu-29` must be enabled for miniscreenshot-wgpu");

/// Re-export of the `wgpu` crate.
///
/// Depending on `miniscreenshot-wgpu` instead of `wgpu` directly guarantees
/// version compatibility across the workspace.
#[cfg(feature = "wgpu-28")]
pub use wgpu_28 as wgpu;
#[cfg(feature = "wgpu-29")]
pub use wgpu_29 as wgpu;

pub use miniscreenshot::{Capture, CaptureError, Screenshot};

use std::sync::{Arc, Mutex};

/// Errors that can occur while capturing a GPU texture.
#[derive(Debug)]
pub enum WgpuCaptureError {
    /// The texture format is not yet supported.
    ///
    /// Supported formats: `Rgba8Unorm`, `Rgba8UnormSrgb`, `Bgra8Unorm`,
    /// `Bgra8UnormSrgb`.
    UnsupportedFormat(wgpu::TextureFormat),

    /// The staging buffer could not be mapped.
    MapFailed(wgpu::BufferAsyncError),

    /// The device poll failed.
    PollFailed(wgpu::PollError),
}

impl std::fmt::Display for WgpuCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat(fmt) => {
                write!(f, "unsupported texture format for screenshot: {fmt:?}")
            }
            Self::MapFailed(e) => write!(f, "staging buffer map failed: {e}"),
            Self::PollFailed(e) => write!(f, "device poll failed: {e}"),
        }
    }
}

impl std::error::Error for WgpuCaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MapFailed(e) => Some(e),
            Self::PollFailed(e) => Some(e),
            _ => None,
        }
    }
}

impl From<WgpuCaptureError> for CaptureError {
    fn from(e: WgpuCaptureError) -> Self {
        match e {
            WgpuCaptureError::UnsupportedFormat(fmt) => CaptureError::new(
                miniscreenshot::CaptureErrorKind::Unsupported,
                format!("unsupported texture format: {fmt:?}"),
            )
            .with_source(WgpuCaptureError::UnsupportedFormat(fmt)),
            WgpuCaptureError::MapFailed(e) => CaptureError::new(
                miniscreenshot::CaptureErrorKind::Backend,
                format!("staging buffer map failed: {e}"),
            )
            .with_source(WgpuCaptureError::MapFailed(e)),
            WgpuCaptureError::PollFailed(e) => CaptureError::new(
                miniscreenshot::CaptureErrorKind::Backend,
                format!("device poll failed: {e}"),
            )
            .with_source(WgpuCaptureError::PollFailed(e)),
        }
    }
}

/// Borrowed view over a wgpu [`Texture`](wgpu::Texture) that implements [`Capture`].
///
/// # Example
///
/// ```rust,ignore
/// use miniscreenshot::Capture;
/// use miniscreenshot_wgpu::WgpuCapture;
///
/// let mut cap = WgpuCapture::new(&device, &queue, &texture);
/// let shot = cap.capture()?;
/// ```
pub struct WgpuCapture<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    texture: &'a wgpu::Texture,
}

impl<'a> WgpuCapture<'a> {
    /// Create a new capture helper.
    pub fn new(
        device: &'a wgpu::Device,
        queue: &'a wgpu::Queue,
        texture: &'a wgpu::Texture,
    ) -> Self {
        Self {
            device,
            queue,
            texture,
        }
    }
}

impl Capture for WgpuCapture<'_> {
    type Error = CaptureError;

    fn capture(&mut self) -> Result<Screenshot, CaptureError> {
        capture(self.device, self.queue, self.texture).map_err(CaptureError::from)
    }
}

/// Capture a screenshot from a wgpu [`Texture`](wgpu::Texture) synchronously.
///
/// The texture must have been created with [`wgpu::TextureUsages::COPY_SRC`].
///
/// # Capturing a frame you are presenting
///
/// You usually cannot capture the swapchain/surface texture directly: surface
/// textures are acquired without `COPY_SRC`, so `copy_texture_to_buffer`
/// rejects them. The standard pattern for an app or editor is to render the
/// scene into your own offscreen texture and present from there:
///
/// 1. Create an offscreen texture with `RENDER_ATTACHMENT | COPY_SRC` usage
///    and a supported format.
/// 2. Render your frame into that texture.
/// 3. Call [`capture`] on it to get a [`Screenshot`].
/// 4. Blit/draw the offscreen texture to the surface to present it.
///
/// The `wgpu_scene_screenshot` example shows the offscreen-texture setup.
///
/// # Supported texture formats
///
/// | Format | Behaviour |
/// |--------|-----------|
/// | `Rgba8Unorm` / `Rgba8UnormSrgb` | Used directly |
/// | `Bgra8Unorm` / `Bgra8UnormSrgb` | Channels reordered to RGBA |
///
/// All other formats return [`WgpuCaptureError::UnsupportedFormat`].
///
/// # Blocking behaviour
///
/// This function calls [`wgpu::Device::poll`] with
/// [`wgpu::PollType::wait_indefinitely`], which blocks the current thread
/// until the GPU work is complete. From an
/// async context (a tokio/async render loop) wrap the call in
/// `tokio::task::spawn_blocking` (or your runtime's equivalent) so the
/// executor thread is not stalled.
pub fn capture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Result<Screenshot, WgpuCaptureError> {
    let size = texture.size();
    let width = size.width;
    let height = size.height;
    let format = texture.format();

    // Determine whether channel swapping (BGRA → RGBA) is needed.
    let is_bgra = match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
        _ => return Err(WgpuCaptureError::UnsupportedFormat(format)),
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
        wgpu::TexelCopyBufferInfo {
            buffer: &staging_buffer,
            layout: wgpu::TexelCopyBufferLayout {
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
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(WgpuCaptureError::PollFailed)?;
    rx.recv()
        .expect("map_async callback channel closed unexpectedly")
        .map_err(WgpuCaptureError::MapFailed)?;

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

/// A `Send + 'static` capture source that always reads the **latest frame your
/// application published** — built for handing to a long-lived service such as
/// an embedded MCP screenshot server, so a coding agent can grab your game's
/// current frame (not the desktop) on demand.
///
/// Unlike [`WgpuCapture`], which borrows the device/queue/texture, this type
/// owns clones of the (cheaply cloneable) wgpu handles and an interior,
/// swappable "current frame" texture, so it satisfies `Capture + Send +
/// 'static`.
///
/// # Usage
///
/// The type is [`Clone`]; keep one clone in your render loop and hand another
/// to the server. Each clone shares the same published-frame slot.
///
/// 1. Render your frame into an offscreen texture created with
///    `RENDER_ATTACHMENT | COPY_SRC` and a supported format.
/// 2. After `queue.submit(...)`, call [`set_frame`](Self::set_frame) with that
///    texture. wgpu serializes GPU work on the queue, so a later capture reads
///    the last fully-submitted frame.
/// 3. Recreate the texture on resize and `set_frame` the new one.
///
/// ```rust,no_run
/// # use miniscreenshot_wgpu::{wgpu, WgpuFrameTarget};
/// # fn demo(device: wgpu::Device, queue: wgpu::Queue, frame: wgpu::Texture) {
/// let target = WgpuFrameTarget::new(device, queue);
/// // hand `target.clone()` to the MCP server, keep `target` in the loop:
/// target.set_frame(frame); // after queue.submit(...)
/// # }
/// ```
#[derive(Clone)]
pub struct WgpuFrameTarget {
    device: wgpu::Device,
    queue: wgpu::Queue,
    frame: Arc<Mutex<Option<wgpu::Texture>>>,
}

impl WgpuFrameTarget {
    /// Create a frame target from clones of your wgpu device and queue. No
    /// frame is published until the first [`set_frame`](Self::set_frame).
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            frame: Arc::new(Mutex::new(None)),
        }
    }

    /// Publish the latest fully-rendered frame. Call after `queue.submit(...)`
    /// for the texture you just rendered (and again whenever you recreate it,
    /// e.g. on resize). The texture must have `COPY_SRC` usage and a supported
    /// format (see [`capture`]).
    pub fn set_frame(&self, texture: wgpu::Texture) {
        *self.frame.lock().unwrap_or_else(|e| e.into_inner()) = Some(texture);
    }

    /// Drop the currently published frame (e.g. while the window is minimized).
    /// Captures then fail until the next [`set_frame`](Self::set_frame).
    pub fn clear(&self) {
        *self.frame.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Whether a frame has been published yet.
    pub fn has_frame(&self) -> bool {
        self.frame
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }
}

impl Capture for WgpuFrameTarget {
    type Error = CaptureError;

    fn capture(&mut self) -> Result<Screenshot, CaptureError> {
        // Clone the handle out and release the lock before the (blocking)
        // readback, so the render loop can keep publishing frames meanwhile.
        let texture = self
            .frame
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| {
                // Runtime state, not an unsupported operation: no frame published yet.
                CaptureError::new(
                    miniscreenshot::CaptureErrorKind::Other,
                    "no frame has been published yet (call WgpuFrameTarget::set_frame)",
                )
            })?;
        capture(&self.device, &self.queue, &texture).map_err(CaptureError::from)
    }
}

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
