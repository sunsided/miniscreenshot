//! Vello screenshot helper for the miniscreenshot ecosystem.
//!
//! This crate re-exports `vello` to avoid version conflicts, and provides
//! a placeholder for converting Vello render output into a [`Screenshot`].
//!
//! Vello itself is a GPU-accelerated vector renderer; pixel readback is
//! performed through a `wgpu` texture. Pair this crate with
//! `miniscreenshot-wgpu` for a full capture pipeline.
//!
//! # Usage
//!
//! ```toml
//! [dependencies]
//! miniscreenshot-vello = "0.1"
//! ```
//!
//! Then access the re-exported crate:
//!
//! ```rust,ignore
//! use miniscreenshot_vello::vello;
//! ```

pub use miniscreenshot::{Screenshot, ScreenshotProvider};

/// Re-export of `vello`.
///
/// Use this instead of a direct `vello` dependency to avoid version
/// mismatches.
pub use vello;
