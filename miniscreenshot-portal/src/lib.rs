//! XDG Desktop Portal screenshot capture via [`ashpd`].
//!
//! This crate wraps the `org.freedesktop.portal.Screenshot` interface,
//! providing both blocking and async APIs for system-level screenshot
//! capture that works on GNOME, KDE, wlroots-based compositors, and
//! inside Flatpak/Snap sandboxes.
//!
//! # Feature flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `tokio` (default) | Use tokio as the async runtime for ashpd |
//! | `async-std` | Use async-std as the async runtime for ashpd |
//! | `blocking` (default) | Enable blocking convenience methods via `pollster` |
//! | `async` | Enable `CaptureAsync` trait impl |
//!
//! Exactly one of `tokio` or `async-std` must be enabled.
//!
//! # Example (blocking)
//!
//! ```rust,no_run
//! use miniscreenshot_portal::PortalCapture;
//!
//! let mut cap = PortalCapture::connect();
//! let shot = cap.capture_interactive().expect("capture");
//! shot.save("portal_screenshot.png").unwrap();
//! ```
//!
//! # Example (async)
//!
//! ```rust,no_run
//! use miniscreenshot_portal::PortalCapture;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut cap = PortalCapture::connect_async().await;
//!     let shot = cap.capture_interactive_async().await?;
//!     shot.save("portal_screenshot.png")?;
//!     Ok(())
//! }
//! ```

#[cfg(all(feature = "tokio", feature = "async-std"))]
compile_error!("Enable exactly one of `tokio` or `async-std` on miniscreenshot-portal.");

#[cfg(not(any(feature = "tokio", feature = "async-std")))]
compile_error!("Enable one of `tokio` or `async-std` on miniscreenshot-portal.");

pub use ashpd;
#[cfg(feature = "async")]
pub use miniscreenshot::CaptureAsync;
pub use miniscreenshot::{Capture, CaptureError, MultiCapture, Screenshot};

use ashpd::desktop::screenshot::Screenshot as PortalScreenshot;
use std::fs::File;
use std::io::Read;

/// An error returned by [`PortalCapture`] operations.
#[derive(Debug)]
pub enum PortalCaptureError {
    /// The portal D-Bus call failed.
    Portal(ashpd::Error),
    /// The user cancelled the screenshot request (declined the dialog).
    PortalCancelled,
    /// Failed to decode the PNG file returned by the portal.
    DecodePng(String),
    /// The URI returned by the portal was not a `file://` scheme.
    UnsupportedScheme(String),
    /// An I/O error occurred.
    Io(std::io::Error),
}

impl std::fmt::Display for PortalCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Portal(e) => write!(f, "Portal error: {e}"),
            Self::PortalCancelled => write!(f, "Screenshot request was cancelled by the user"),
            Self::DecodePng(msg) => write!(f, "PNG decode error: {msg}"),
            Self::UnsupportedScheme(uri) => write!(f, "Unsupported URI scheme: {uri}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for PortalCaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Portal(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ashpd::Error> for PortalCaptureError {
    fn from(e: ashpd::Error) -> Self {
        match e {
            ashpd::Error::Response(response) => {
                if matches!(response, ashpd::desktop::ResponseError::Cancelled) {
                    Self::PortalCancelled
                } else {
                    Self::Portal(ashpd::Error::Response(response))
                }
            }
            other => Self::Portal(other),
        }
    }
}

impl From<PortalCaptureError> for CaptureError {
    fn from(e: PortalCaptureError) -> Self {
        use miniscreenshot::CaptureErrorKind;
        match e {
            PortalCaptureError::PortalCancelled => CaptureError::new(
                CaptureErrorKind::Cancelled,
                "screenshot request was cancelled by the user",
            ),
            PortalCaptureError::UnsupportedScheme(uri) => CaptureError::new(
                CaptureErrorKind::Unsupported,
                format!("unsupported URI scheme: {uri}"),
            ),
            PortalCaptureError::DecodePng(msg) => {
                CaptureError::new(CaptureErrorKind::Decode, format!("PNG decode error: {msg}"))
            }
            PortalCaptureError::Portal(err) => {
                CaptureError::new(CaptureErrorKind::Backend, format!("portal error: {err}"))
                    .with_source(PortalCaptureError::Portal(err))
            }
            PortalCaptureError::Io(err) => {
                CaptureError::new(CaptureErrorKind::Io, format!("I/O error: {err}"))
                    .with_source(PortalCaptureError::Io(err))
            }
        }
    }
}

// ── Blocking runtime bridge ──────────────────────────────────────────────────
//
// `pollster` cannot drive tokio-based futures because zbus (used by ashpd)
// requires an active I/O reactor. Route the blocking API through the
// matching runtime's own `block_on`.

#[cfg(all(feature = "blocking", feature = "tokio"))]
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build current-thread tokio runtime for blocking screenshot call");
    rt.block_on(fut)
}

#[cfg(all(feature = "blocking", feature = "async-std"))]
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    async_std::task::block_on(fut)
}

/// A portal-based screen-capture session.
///
/// Connects to the XDG Desktop Portal's screenshot interface. The
/// compositor may show a confirmation dialog depending on its policy
/// (GNOME always does; KDE and wlroots may or may not).
pub struct PortalCapture;

impl PortalCapture {
    /// Connect to the user session's portal. Does not prompt yet.
    ///
    /// This function is infallible; errors are reported on first capture
    /// when the portal proxy is actually created.
    pub async fn connect_async() -> Self {
        Self
    }

    /// Blocking variant of [`connect_async`].
    ///
    /// Internally builds a current-thread runtime (tokio or async-std,
    /// matching the selected feature). Not suitable for calling from
    /// inside an existing runtime — use `connect_async` there.
    #[cfg(feature = "blocking")]
    pub fn connect() -> Self {
        block_on(Self::connect_async())
    }

    /// Interactive capture — the compositor may show a confirmation UI.
    ///
    /// Returns a [`Screenshot`] on success.
    pub async fn capture_interactive_async(&mut self) -> Result<Screenshot, PortalCaptureError> {
        self.do_capture(true).await
    }

    /// Silent capture — requests `interactive: false`. The portal may
    /// still prompt depending on compositor policy and app permissions.
    pub async fn capture_silent_async(&mut self) -> Result<Screenshot, PortalCaptureError> {
        self.do_capture(false).await
    }

    /// Blocking variant of [`capture_interactive_async`].
    #[cfg(feature = "blocking")]
    pub fn capture_interactive(&mut self) -> Result<Screenshot, PortalCaptureError> {
        block_on(self.capture_interactive_async())
    }

    /// Blocking variant of [`capture_silent_async`].
    #[cfg(feature = "blocking")]
    pub fn capture_silent(&mut self) -> Result<Screenshot, PortalCaptureError> {
        block_on(self.capture_silent_async())
    }

    async fn do_capture(&mut self, interactive: bool) -> Result<Screenshot, PortalCaptureError> {
        let response = PortalScreenshot::request()
            .interactive(interactive)
            .modal(true)
            .send()
            .await?
            .response()?;

        let uri = response.uri();

        if uri.scheme() != "file" {
            return Err(PortalCaptureError::UnsupportedScheme(uri.to_string()));
        }

        let path = uri
            .to_file_path()
            .map_err(|()| PortalCaptureError::UnsupportedScheme(uri.to_string()))?;

        let mut file = File::open(&path).map_err(PortalCaptureError::Io)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).map_err(PortalCaptureError::Io)?;

        let decoder = png::Decoder::new(buf.as_slice());
        let mut reader = decoder
            .read_info()
            .map_err(|e| PortalCaptureError::DecodePng(e.to_string()))?;

        let mut img_data = vec![0u8; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut img_data)
            .map_err(|e| PortalCaptureError::DecodePng(e.to_string()))?;

        let width = info.width;
        let height = info.height;
        // The PNG reader may over-allocate `img_data` via `output_buffer_size()`.
        // Truncate to the actual bytes written to satisfy `Screenshot` length checks.
        img_data.truncate(info.buffer_size());

        let shot = match info.color_type {
            png::ColorType::Rgba => Screenshot::from_rgba(width, height, img_data),
            png::ColorType::Rgb => Screenshot::from_rgb(width, height, &img_data),
            other => {
                return Err(PortalCaptureError::DecodePng(format!(
                    "unsupported PNG color type: {other:?}"
                )))
            }
        };

        Ok(shot)
    }
}

#[cfg(feature = "blocking")]
impl Capture for PortalCapture {
    type Error = CaptureError;

    fn capture(&mut self) -> Result<Screenshot, CaptureError> {
        self.capture_interactive().map_err(CaptureError::from)
    }
}

#[cfg(feature = "blocking")]
impl MultiCapture for PortalCapture {
    fn source_count(&self) -> usize {
        1
    }

    fn capture_index(&mut self, index: usize) -> Result<Screenshot, CaptureError> {
        if index != 0 {
            return Err(CaptureError::new(
                miniscreenshot::CaptureErrorKind::Unsupported,
                "portal only supports a single capture (index 0)",
            ));
        }
        self.capture_interactive().map_err(CaptureError::from)
    }
}

#[cfg(feature = "async")]
impl CaptureAsync for PortalCapture {
    type Error = CaptureError;

    async fn capture(&mut self) -> Result<Screenshot, CaptureError> {
        self.capture_interactive_async()
            .await
            .map_err(CaptureError::from)
    }
}
