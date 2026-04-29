//! Automatic desktop screenshot capture.
//!
//! This crate provides a single [`take`] function that attempts to capture a
//! screenshot using the best available backend for the current desktop
//! environment. The fallback order is:
//!
//! 1. **Wayland** — if `WAYLAND_DISPLAY` is set and the compositor supports
//!    `zwlr_screencopy_manager_v1` (wlroots-based: Sway, Hyprland, …).
//! 2. **X11** — if `DISPLAY` is set and we're not on XWayland.
//! 3. **Portal** — as a last resort, using the XDG Desktop Portal (works on
//!    GNOME, KDE, Flatpak, Snap).
//!
//! # Example
//!
//! ```rust,no_run
//! use miniscreenshot_desktop::take;
//!
//! let shot = take().expect("screenshot");
//! shot.save("desktop.png").unwrap();
//! ```
//!
//! # Backend details
//!
//! | Backend | Condition | Notes |
//! |---------|-----------|-------|
//! | Wayland | `$WAYLAND_DISPLAY` set | Requires wlroots-based compositor |
//! | X11 | `$DISPLAY` set, not XWayland | Uses MIT-SHM when available |
//! | Portal | Always available | May show a confirmation dialog |

use miniscreenshot::Capture;
use miniscreenshot::Screenshot;

/// Errors that can occur during automatic desktop screenshot capture.
#[derive(Debug)]
pub enum DesktopCaptureError {
    /// All backends failed.
    AllBackendsFailed(Vec<String>),
}

impl std::fmt::Display for DesktopCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllBackendsFailed(errors) => {
                write!(f, "all screenshot backends failed:")?;
                for err in errors {
                    write!(f, "\n  - {err}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for DesktopCaptureError {}

/// Attempt to capture a screenshot using the best available backend.
///
/// Tries backends in this order:
/// 1. Wayland (if `$WAYLAND_DISPLAY` is set and screencopy is available)
/// 2. X11 (if `$DISPLAY` is set and not XWayland)
/// 3. Portal (always available, may prompt user)
///
/// Returns the first successful capture, or an error with all failure reasons.
pub fn take() -> Result<Screenshot, DesktopCaptureError> {
    let mut errors = Vec::new();

    // Try Wayland first
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        match miniscreenshot_wayland::WaylandCapture::connect() {
            Ok(mut cap) => match cap.capture() {
                Ok(shot) => return Ok(shot),
                Err(e) => errors.push(format!("Wayland: {e}")),
            },
            Err(e) => errors.push(format!("Wayland (connect): {e}")),
        }
    }

    // Try X11
    if std::env::var("DISPLAY").is_ok() {
        match miniscreenshot_x11::X11Capture::connect() {
            Ok(mut cap) => match cap.capture() {
                Ok(shot) => return Ok(shot),
                Err(e) => errors.push(format!("X11: {e}")),
            },
            Err(e) => errors.push(format!("X11 (connect): {e}")),
        }
    }

    // Fall back to Portal
    let mut portal = miniscreenshot_portal::PortalCapture::connect();
    match portal.capture_interactive() {
        Ok(shot) => return Ok(shot),
        Err(e) => errors.push(format!("Portal: {e}")),
    }

    Err(DesktopCaptureError::AllBackendsFailed(errors))
}
