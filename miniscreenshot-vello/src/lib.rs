//! Vello screenshot helper for the miniscreenshot ecosystem.
//!
//! This crate re-exports `vello` (when the `vello` feature is enabled) to
//! avoid version conflicts, and provides a placeholder for converting Vello
//! render output into a [`Screenshot`].
//!
//! Vello itself is a GPU-accelerated vector renderer; pixel readback is
//! performed through a `wgpu` texture. Pair this crate with
//! `miniscreenshot-wgpu` for a full capture pipeline.
//!
//! # Enabling Vello
//!
//! ```toml
//! [dependencies]
//! miniscreenshot-vello = { version = "0.1", features = ["vello"] }
//! ```
//!
//! Then access the re-exported crate:
//!
//! ```rust,ignore
//! use miniscreenshot_vello::vello;
//! ```

pub use miniscreenshot::{Screenshot, ScreenshotProvider};

/// Re-export of `vello` when the `vello` feature is enabled.
///
/// Use this instead of a direct `vello` dependency to avoid version
/// mismatches.
#[cfg(feature = "vello")]
pub use vello;
