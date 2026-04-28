//! Screenshot integration for the [`winit`] windowing library.
//!
//! `winit` is a cross-platform windowing abstraction; it does not manage pixel
//! buffers itself. This crate re-exports `winit` (ensuring a single shared
//! version across the workspace) and provides a thin [`WindowCapture`]
//! wrapper around any [`ScreenshotProvider`].
//!
//! [`WindowCapture`] itself is window-agnostic — it simply forwards to the
//! wrapped provider. The companion rendering crate (e.g.
//! [`miniscreenshot-softbuffer`] or [`miniscreenshot-wgpu`]) is responsible
//! for binding the provider to a specific `winit::window::Window` and
//! performing the actual pixel capture.
//!
//! # Re-export
//!
//! ```rust,no_run
//! // Use the bundled winit — avoids version conflicts.
//! use miniscreenshot_winit::winit;
//! ```

/// Re-export of the `winit` crate.
///
/// Depending on `miniscreenshot-winit` instead of `winit` directly guarantees
/// version compatibility across the workspace.
pub use winit;

pub use miniscreenshot::{Screenshot, ScreenshotProvider};

/// Thin wrapper around any [`ScreenshotProvider`].
///
/// `WindowCapture` does not itself reference a `winit::window::Window`; it
/// exists to provide a uniform capture API for applications that already own
/// a winit window and a rendering-backend provider (e.g. softbuffer- or
/// wgpu-backed). The wrapped provider is expected to know which window it
/// reads pixels from.
///
/// `P` can be any type that implements [`ScreenshotProvider`] — for example a
/// softbuffer-backed or wgpu-backed capture implementation.
///
/// # Example
///
/// ```rust,no_run
/// use miniscreenshot_winit::{WindowCapture, ScreenshotProvider};
///
/// struct MyBackend; // implements ScreenshotProvider
/// # use miniscreenshot::Screenshot;
/// # impl ScreenshotProvider for MyBackend {
/// #     type Error = ();
/// #     fn take_screenshot(&mut self) -> Result<Screenshot, ()> {
/// #         unimplemented!()
/// #     }
/// # }
///
/// let mut capture = WindowCapture::new(MyBackend);
/// // let shot = capture.capture().unwrap();
/// ```
pub struct WindowCapture<P> {
    provider: P,
}

impl<P: ScreenshotProvider> WindowCapture<P> {
    /// Wrap `provider` in a [`WindowCapture`].
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    /// Capture a screenshot using the underlying provider.
    pub fn capture(&mut self) -> Result<Screenshot, P::Error> {
        self.provider.take_screenshot()
    }

    /// Return a reference to the underlying provider.
    pub fn provider(&self) -> &P {
        &self.provider
    }

    /// Return a mutable reference to the underlying provider.
    pub fn provider_mut(&mut self) -> &mut P {
        &mut self.provider
    }

    /// Consume the wrapper and return the underlying provider.
    pub fn into_provider(self) -> P {
        self.provider
    }
}

impl<P: ScreenshotProvider> ScreenshotProvider for WindowCapture<P> {
    type Error = P::Error;

    fn take_screenshot(&mut self) -> Result<Screenshot, Self::Error> {
        self.provider.take_screenshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use miniscreenshot::Screenshot;

    /// Trivial provider that always returns a 1×1 red pixel.
    struct RedProvider;

    impl ScreenshotProvider for RedProvider {
        type Error = ();

        fn take_screenshot(&mut self) -> Result<Screenshot, ()> {
            Ok(Screenshot::from_rgba(1, 1, vec![255, 0, 0, 255]))
        }
    }

    #[test]
    fn window_capture_delegates_to_provider() {
        let mut cap = WindowCapture::new(RedProvider);
        let shot = cap.capture().unwrap();
        assert_eq!(shot.width(), 1);
        assert_eq!(shot.data(), &[255, 0, 0, 255]);
    }

    #[test]
    fn screenshot_provider_impl_for_window_capture() {
        let mut cap = WindowCapture::new(RedProvider);
        let shot = cap.take_screenshot().unwrap();
        assert_eq!(shot.height(), 1);
    }
}
