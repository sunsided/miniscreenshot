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
//! miniscreenshot-vello = "0.2"
//! ```
//!
//! Then access the re-exported crate:
//!
//! ```rust,ignore
//! use miniscreenshot_vello::vello;
//! ```

pub use miniscreenshot::Screenshot;

/// Re-export of `vello`.
///
/// Use this instead of a direct `vello` dependency to avoid version
/// mismatches.
pub use vello;

/// Capture a screenshot from a Vello-rendered texture.
///
/// TODO: Implement Vello-specific capture logic. For now, use
/// `miniscreenshot-wgpu::capture` with the texture produced by Vello.
pub fn capture(
    _device: &vello::wgpu::Device,
    _queue: &vello::wgpu::Queue,
    _texture: &vello::wgpu::Texture,
) -> Screenshot {
    // TODO: Implement Vello-specific capture logic.
    // For now, return an empty 1×1 black screenshot.
    // Use `miniscreenshot_wgpu::capture` with the Vello output texture instead.
    Screenshot::from_rgba(1, 1, vec![0, 0, 0, 255])
}
