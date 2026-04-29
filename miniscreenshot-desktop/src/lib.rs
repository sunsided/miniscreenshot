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
//! | Portal | Always available | May show a confirmation dialog

use miniscreenshot::{Capture, CaptureError, MultiCapture, Screenshot};

/// Identifies which backend was selected by [`select_backend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Wayland screencopy (wlroots-based compositors).
    Wayland,
    /// X11 via MIT-SHM.
    X11,
    /// XDG Desktop Portal (GNOME, KDE, Flatpak, Snap).
    Portal,
}

/// Attempt to capture a screenshot using the best available backend.
///
/// Tries backends in this order:
/// 1. Wayland (if `$WAYLAND_DISPLAY` is set and screencopy is available)
/// 2. X11 (if `$DISPLAY` is set and not XWayland)
/// 3. Portal (always available, may prompt user)
///
/// Returns the first successful capture, or an error with all failure reasons.
pub fn take() -> Result<Screenshot, CaptureError> {
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

    Err(CaptureError::new(
        miniscreenshot::CaptureErrorKind::Backend,
        format!(
            "all screenshot backends failed:{}",
            errors
                .iter()
                .map(|e| format!("\n  - {e}"))
                .collect::<String>()
        ),
    ))
}

/// Select the best available backend for multi-monitor capture.
///
/// Probes backends in the same order as [`take`] (Wayland → X11 → Portal)
/// and returns a boxed [`DynMultiCapture`] so the caller can enumerate
/// monitors or call [`MultiCapture::capture_all`].
///
/// Returns [`BackendKind`] alongside the capture session so callers know
/// which backend was selected.
pub fn select_backend(
) -> Result<(BackendKind, Box<dyn MultiCapture<Error = CaptureError>>), CaptureError> {
    // Try Wayland first
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        if let Ok(cap) = miniscreenshot_wayland::WaylandCapture::connect() {
            let count = cap.source_count();
            if count > 0 {
                return Ok((BackendKind::Wayland, Box::new(cap)));
            }
        }
    }

    // Try X11
    if std::env::var("DISPLAY").is_ok() {
        if let Ok(cap) = miniscreenshot_x11::X11Capture::connect() {
            let count = cap.source_count();
            if count > 0 {
                return Ok((BackendKind::X11, Box::new(cap)));
            }
        }
    }

    // Fall back to Portal
    let portal = miniscreenshot_portal::PortalCapture::connect();
    Ok((BackendKind::Portal, Box::new(portal)))
}

/// Capture all available outputs using the best available backend.
///
/// This is a convenience wrapper around [`select_backend`] that returns
/// screenshots from every detected monitor.
pub fn take_all() -> Result<(BackendKind, Vec<Screenshot>), CaptureError> {
    let (kind, mut cap) = select_backend()?;
    let shots = cap.capture_all()?;
    Ok((kind, shots))
}
