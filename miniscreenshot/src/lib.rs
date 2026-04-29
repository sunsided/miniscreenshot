//! Core screenshot types for the miniscreenshot ecosystem.
//!
//! This crate provides the fundamental [`Screenshot`] type along with PNG,
//! PPM, and PGM encoding, and file-saving utilities. It also exposes the
//! [`Capture`], [`CaptureAsync`], and [`MultiCapture`] traits that driver
//! crates implement.
//!
//! # Quick start
//!
//! ```rust
//! use miniscreenshot::{Screenshot, ImageFormat};
//!
//! // Build from raw RGBA8 data
//! let data = vec![255u8, 0, 0, 255, 0, 255, 0, 255]; // 2×1 red|green pixels
//! let shot = Screenshot::from_rgba(2, 1, data);
//!
//! // Encode as PPM bytes
//! let ppm = shot.encode_ppm();
//! assert!(ppm.starts_with(b"P6\n"));
//!
//! // Save to disk — format inferred from extension
//! // shot.save("output.png").unwrap();
//! ```

use std::io::Write;
use std::path::Path;

// ── ImageFormat ─────────────────────────────────────────────────────────────

/// Supported image formats for encoding and saving screenshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// Portable Network Graphics — lossless, widely supported.
    Png,
    /// Portable Pixmap (P6 binary) — raw RGB, near-zero encoding overhead.
    Ppm,
    /// Portable Graymap (P5 binary) — luminance-weighted grayscale.
    Pgm,
}

impl ImageFormat {
    /// Detect the format from a file-name extension (case-insensitive).
    ///
    /// Returns `None` for unrecognised extensions.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(Self::Png),
            "ppm" => Some(Self::Ppm),
            "pgm" => Some(Self::Pgm),
            _ => None,
        }
    }
}

// ── Screenshot ───────────────────────────────────────────────────────────────

/// A captured screenshot backed by raw RGBA8 pixel data.
///
/// Pixels are stored in row-major order; each pixel is four consecutive bytes
/// `[R, G, B, A]` with values in `0..=255`.
#[derive(Debug, Clone)]
pub struct Screenshot {
    width: u32,
    height: u32,
    /// Raw RGBA8 data, row-major.
    data: Vec<u8>,
}

impl Screenshot {
    /// Create a screenshot from raw **RGBA8** pixel data.
    ///
    /// # Panics
    ///
    /// Panics if `data.len() != width * height * 4`.
    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> Self {
        assert_eq!(
            data.len(),
            (width as usize) * (height as usize) * 4,
            "RGBA8 data must be exactly width × height × 4 bytes"
        );
        Self {
            width,
            height,
            data,
        }
    }

    /// Create a screenshot from raw **RGB8** pixel data, promoting to RGBA8
    /// with full opacity (`alpha = 255`).
    ///
    /// # Panics
    ///
    /// Panics if `data.len() != width * height * 3`.
    pub fn from_rgb(width: u32, height: u32, data: &[u8]) -> Self {
        assert_eq!(
            data.len(),
            (width as usize) * (height as usize) * 3,
            "RGB8 data must be exactly width × height × 3 bytes"
        );
        let rgba: Vec<u8> = data
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255u8])
            .collect();
        Self {
            width,
            height,
            data: rgba,
        }
    }

    /// Width of the screenshot in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the screenshot in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Raw RGBA8 pixel data (row-major, 4 bytes per pixel).
    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Consume the screenshot and return the underlying RGBA8 pixel buffer.
    #[inline]
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    // ── Encoding ─────────────────────────────────────────────────────────────

    /// Encode the screenshot as a **PNG** image.
    pub fn encode_png(&self) -> Result<Vec<u8>, EncodeError> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|e| EncodeError::Png(e.to_string()))?;
            writer
                .write_image_data(&self.data)
                .map_err(|e| EncodeError::Png(e.to_string()))?;
        }
        Ok(buf)
    }

    /// Encode the screenshot as a **PPM** (Portable Pixmap, P6 binary) image.
    ///
    /// The alpha channel is discarded; only RGB values are written.
    pub fn encode_ppm(&self) -> Vec<u8> {
        let header = format!("P6\n{} {}\n255\n", self.width, self.height);
        let mut buf =
            Vec::with_capacity(header.len() + (self.width as usize) * (self.height as usize) * 3);
        buf.extend_from_slice(header.as_bytes());
        for pixel in self.data.chunks_exact(4) {
            buf.push(pixel[0]); // R
            buf.push(pixel[1]); // G
            buf.push(pixel[2]); // B
        }
        buf
    }

    /// Encode the screenshot as a **PGM** (Portable Graymap, P5 binary) image.
    ///
    /// Each pixel is converted to grayscale using the ITU-R BT.601 luminance
    /// coefficients: `Y = 0.299·R + 0.587·G + 0.114·B`.
    pub fn encode_pgm(&self) -> Vec<u8> {
        let header = format!("P5\n{} {}\n255\n", self.width, self.height);
        let mut buf =
            Vec::with_capacity(header.len() + (self.width as usize) * (self.height as usize));
        buf.extend_from_slice(header.as_bytes());
        for pixel in self.data.chunks_exact(4) {
            let gray =
                (0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32) as u8;
            buf.push(gray);
        }
        buf
    }

    /// Encode the screenshot in the given [`ImageFormat`].
    pub fn encode(&self, format: ImageFormat) -> Result<Vec<u8>, EncodeError> {
        match format {
            ImageFormat::Png => self.encode_png(),
            ImageFormat::Ppm => Ok(self.encode_ppm()),
            ImageFormat::Pgm => Ok(self.encode_pgm()),
        }
    }

    // ── Saving ───────────────────────────────────────────────────────────────

    /// Save the screenshot to `path`.
    ///
    /// The image format is inferred from the file extension (`.png`, `.ppm`,
    /// `.pgm`). When the extension is absent or unrecognised, PNG is used.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), SaveError> {
        let path = path.as_ref();
        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(ImageFormat::from_extension)
            .unwrap_or(ImageFormat::Png);
        self.save_as(path, format)
    }

    /// Save the screenshot to `path` in the explicitly chosen `format`.
    pub fn save_as<P: AsRef<Path>>(&self, path: P, format: ImageFormat) -> Result<(), SaveError> {
        let data = self.encode(format).map_err(SaveError::Encode)?;
        let mut file = std::fs::File::create(path).map_err(SaveError::Io)?;
        file.write_all(&data).map_err(SaveError::Io)?;
        Ok(())
    }
}

// ── Capture trait ────────────────────────────────────────────────────────────

/// A self-contained screenshot source (e.g. a system capture session).
///
/// Implemented by providers that already hold everything needed to produce a
/// screenshot on demand. Driver crates (`miniscreenshot-wayland`,
/// `miniscreenshot-x11`, `miniscreenshot-portal`, …) implement this trait so
/// they can be used interchangeably.
///
/// A blanket implementation is provided for `FnMut() -> Result<Screenshot, E>`,
/// so free functions like `miniscreenshot_wgpu::capture(&device, &queue,
/// &texture)` can be used as trait objects via a closure:
///
/// ```rust,ignore
/// let mut cap = || miniscreenshot_wgpu::capture(&device, &queue, &texture);
/// take_and_save(&mut cap);
/// ```
pub trait Capture {
    /// The error type returned when capture fails.
    type Error;

    /// Capture a screenshot from this source.
    fn capture(&mut self) -> Result<Screenshot, Self::Error>;
}

/// A blanket impl: any `FnMut` that returns `Result<Screenshot, E>` is a
/// `Capture`. This removes the need for wrapper structs when the per-call
/// state (device, queue, texture, surface, etc.) is captured in the closure
/// body.
impl<F, E> Capture for F
where
    F: FnMut() -> Result<Screenshot, E>,
{
    type Error = E;
    fn capture(&mut self) -> Result<Screenshot, E> {
        (self)()
    }
}

// ── CaptureAsync trait ──────────────────────────────────────────────────────

/// An async-capable source that can capture a screenshot.
///
/// This trait mirrors [`Capture`] but uses an `async fn` via return-position
/// `impl Trait` in trait (RPITIT), allowing driver crates such as
/// `miniscreenshot-portal` to expose natively async APIs without boxing futures.
pub trait CaptureAsync {
    /// The error type returned when capture fails.
    type Error;

    /// Capture a screenshot from this source.
    fn capture(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Screenshot, Self::Error>> + Send;
}

// ── MultiCapture trait ───────────────────────────────────────────────────────

/// A [`Capture`] source that can capture multiple outputs (screens, monitors).
///
/// Implemented by `X11Capture`, `WaylandCapture`, and `PortalCapture`
/// (which returns 1 for a single interactive session).
pub trait MultiCapture: Capture {
    /// Number of available capture sources.
    fn source_count(&self) -> usize;

    /// Capture the output at zero-based `index`.
    fn capture_index(&mut self, index: usize) -> Result<Screenshot, Self::Error>;

    /// Capture all available outputs.
    fn capture_all(&mut self) -> Result<Vec<Screenshot>, Self::Error> {
        (0..self.source_count())
            .map(|i| self.capture_index(i))
            .collect()
    }
}

// ── CaptureError ──────────────────────────────────────────────────────────────

/// A canonical error type shared by all `Capture` implementations.
///
/// Every driver crate maps its domain-specific error into this type via
/// `From<DomainError>` impls, so that `&mut dyn Capture<Error = CaptureError>`
/// works as a uniform interchange type.
///
/// Domain errors (`WaylandCaptureError`, `X11CaptureError`, …) remain public
/// for consumers who prefer rich, typed error matching on concrete methods.
#[derive(Debug)]
pub struct CaptureError {
    kind: CaptureErrorKind,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

/// High-level categories of capture failure.
///
/// `#[non_exhaustive]` so new variants can be added without a major-version bump.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureErrorKind {
    /// The backend could not connect or initialise (no Wayland display, no X server, etc.).
    Connect,
    /// The requested output / index / format is not supported by this backend.
    Unsupported,
    /// The user cancelled an interactive capture (e.g. portal dialog).
    Cancelled,
    /// The capture was attempted but the backend reported failure mid-flight.
    Backend,
    /// An I/O error occurred.
    Io,
    /// Pixel data could not be decoded or converted.
    Decode,
    /// Catch-all for anything else.
    Other,
}

impl CaptureError {
    /// Create a new capture error with the given kind and message.
    pub fn new(kind: CaptureErrorKind, msg: impl Into<String>) -> Self {
        Self {
            kind,
            message: msg.into(),
            source: None,
        }
    }

    /// Attach a chained [`source`](std::error::Error::source) to this error.
    pub fn with_source(mut self, e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        self.source = Some(e.into());
        self
    }

    /// The high-level category of this error.
    pub fn kind(&self) -> CaptureErrorKind {
        self.kind
    }
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind.as_str(), self.message)
    }
}

impl std::error::Error for CaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as _)
    }
}

impl CaptureErrorKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Connect => "Connection error",
            Self::Unsupported => "Unsupported operation",
            Self::Cancelled => "Cancelled by user",
            Self::Backend => "Backend failure",
            Self::Io => "I/O error",
            Self::Decode => "Decode error",
            Self::Other => "Other error",
        }
    }
}

/// Convenience type alias for dynamic dispatch over `Capture`.
pub type DynCapture = dyn Capture<Error = CaptureError>;

/// Convenience type alias for a boxed, Send-able `Capture`.
pub type BoxedCapture = Box<dyn Capture<Error = CaptureError> + Send>;

/// Convenience type alias for dynamic dispatch over `MultiCapture`.
pub type DynMultiCapture = dyn MultiCapture<Error = CaptureError>;

/// Convenience type alias for dynamic dispatch over `CaptureAsync`.
pub type DynCaptureAsync = dyn CaptureAsync<Error = CaptureError>;

/// Convenience type alias for a boxed, Send-able `CaptureAsync`.
pub type BoxedCaptureAsync = Box<dyn CaptureAsync<Error = CaptureError> + Send>;

// ── Error types ──────────────────────────────────────────────────────────────

/// An error that occurred while encoding a screenshot.
#[derive(Debug)]
pub enum EncodeError {
    /// The PNG encoder returned an error.
    Png(String),
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Png(msg) => write!(f, "PNG encoding error: {msg}"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// An error that occurred while saving a screenshot to disk.
#[derive(Debug)]
pub enum SaveError {
    /// An I/O error occurred while creating or writing the file.
    Io(std::io::Error),
    /// An encoding error occurred before the data could be written.
    Encode(EncodeError),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Encode(e) => write!(f, "Encoding error: {e}"),
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Encode(e) => Some(e),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal 2×2 RGBA image: red, green, blue, white.
    fn sample_2x2() -> Screenshot {
        #[rustfmt::skip]
        let data = vec![
            255,   0,   0, 255, // red
              0, 255,   0, 255, // green
              0,   0, 255, 255, // blue
            255, 255, 255, 255, // white
        ];
        Screenshot::from_rgba(2, 2, data)
    }

    #[test]
    fn from_rgba_dimensions() {
        let shot = sample_2x2();
        assert_eq!(shot.width(), 2);
        assert_eq!(shot.height(), 2);
        assert_eq!(shot.data().len(), 16);
    }

    #[test]
    fn from_rgb_promotes_alpha() {
        let rgb = vec![255u8, 0, 0, 0, 255, 0]; // 2×1 red|green
        let shot = Screenshot::from_rgb(2, 1, &rgb);
        assert_eq!(shot.data(), &[255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    #[should_panic]
    fn from_rgba_wrong_size_panics() {
        Screenshot::from_rgba(2, 2, vec![0u8; 10]);
    }

    // ── PPM ──────────────────────────────────────────────────────────────────

    #[test]
    fn encode_ppm_header() {
        let ppm = sample_2x2().encode_ppm();
        assert!(ppm.starts_with(b"P6\n2 2\n255\n"));
    }

    #[test]
    fn encode_ppm_body() {
        let shot = Screenshot::from_rgba(1, 1, vec![10, 20, 30, 255]);
        let ppm = shot.encode_ppm();
        // header "P6\n1 1\n255\n" then RGB bytes
        let header = b"P6\n1 1\n255\n";
        assert_eq!(&ppm[..header.len()], header);
        assert_eq!(&ppm[header.len()..], &[10, 20, 30]);
    }

    // ── PGM ──────────────────────────────────────────────────────────────────

    #[test]
    fn encode_pgm_header() {
        let pgm = sample_2x2().encode_pgm();
        assert!(pgm.starts_with(b"P5\n2 2\n255\n"));
    }

    #[test]
    fn encode_pgm_pure_white_is_255() {
        let shot = Screenshot::from_rgba(1, 1, vec![255, 255, 255, 255]);
        let pgm = shot.encode_pgm();
        let header = b"P5\n1 1\n255\n";
        assert_eq!(*pgm.last().unwrap(), 255u8);
        assert_eq!(pgm.len(), header.len() + 1);
    }

    #[test]
    fn encode_pgm_pure_black_is_0() {
        let shot = Screenshot::from_rgba(1, 1, vec![0, 0, 0, 255]);
        let pgm = shot.encode_pgm();
        assert_eq!(*pgm.last().unwrap(), 0u8);
    }

    // ── PNG ──────────────────────────────────────────────────────────────────

    #[test]
    fn encode_png_valid_magic() {
        let png = sample_2x2().encode_png().unwrap();
        // PNG files start with the 8-byte magic signature
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn encode_png_round_trip() {
        let original = sample_2x2();
        let png_bytes = original.encode_png().unwrap();

        // Decode with the `png` crate and compare pixel data
        let decoder = png::Decoder::new(png_bytes.as_slice());
        let mut reader = decoder.read_info().unwrap();
        let mut decoded = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut decoded).unwrap();
        decoded.truncate(info.buffer_size());

        assert_eq!(decoded, original.data());
    }

    // ── ImageFormat detection ────────────────────────────────────────────────

    #[test]
    fn format_from_extension() {
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("PNG"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("ppm"), Some(ImageFormat::Ppm));
        assert_eq!(ImageFormat::from_extension("pgm"), Some(ImageFormat::Pgm));
        assert_eq!(ImageFormat::from_extension("jpg"), None);
    }

    // ── Save ─────────────────────────────────────────────────────────────────

    #[test]
    fn save_png_creates_valid_file() {
        let shot = sample_2x2();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.png");
        shot.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn save_ppm_creates_valid_file() {
        let shot = sample_2x2();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.ppm");
        shot.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"P6\n"));
    }

    #[test]
    fn save_pgm_creates_valid_file() {
        let shot = sample_2x2();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.pgm");
        shot.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"P5\n"));
    }

    #[test]
    fn save_unknown_extension_defaults_to_png() {
        let shot = sample_2x2();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.xyz");
        shot.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        // PNG magic
        assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }
}
