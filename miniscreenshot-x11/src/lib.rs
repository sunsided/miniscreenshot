//! X11 screen capture using `XGetImage` with an optional **MIT-SHM** fast path.
//!
//! # Example
//!
//! ```rust,no_run
//! use miniscreenshot_x11::X11Capture;
//!
//! let mut capture = X11Capture::connect().expect("connect to X11");
//! let shot = capture.capture_screen(0).expect("capture first screen");
//! shot.save("screenshot.png").unwrap();
//! ```

pub use miniscreenshot::{Capture, MultiCapture, Screenshot};
pub use x11rb;

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::shm;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, Window};
use x11rb::rust_connection::RustConnection;

use x11rb::protocol::shm::ConnectionExt as _;

#[cfg(unix)]
use std::os::raw::c_void;

// ── SHM cleanup guard ────────────────────────────────────────────────────────

#[cfg(unix)]
struct ShmCleanup<'a> {
    conn: &'a RustConnection,
    seg_id: u32,
    shmid: i32,
    ptr: *mut c_void,
}

#[cfg(unix)]
impl Drop for ShmCleanup<'_> {
    fn drop(&mut self) {
        let _ = self.conn.shm_detach(self.seg_id);
        let _ = unsafe { libc::shmdt(self.ptr) };
        let _ = unsafe { libc::shmctl(self.shmid, libc::IPC_RMID, std::ptr::null_mut()) };
    }
}

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum X11CaptureError {
    Connect(x11rb::rust_connection::ConnectError),
    Connection(x11rb::errors::ConnectionError),
    Reply(x11rb::errors::ReplyError),
    ScreenNotFound(usize),
    UnsupportedVisual {
        depth: u8,
        bits_per_pixel: u8,
    },
    Io(std::io::Error),
    /// The X11 server is XWayland, which cannot capture the Wayland desktop.
    XWaylandDetected,
}

impl std::fmt::Display for X11CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(e) => write!(f, "X11 connection error: {e}"),
            Self::Connection(e) => write!(f, "X11 connection error: {e}"),
            Self::Reply(e) => write!(f, "X11 reply error: {e}"),
            Self::ScreenNotFound(i) => write!(f, "screen index {i} not found"),
            Self::UnsupportedVisual {
                depth,
                bits_per_pixel,
            } => {
                write!(f, "unsupported visual: depth={depth}, bpp={bits_per_pixel}")
            }
            Self::XWaylandDetected => write!(
                f,
                "XWayland detected — root window capture returns black. Use the Wayland or Portal crate instead."
            ),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for X11CaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Connect(e) => Some(e),
            Self::Connection(e) => Some(e),
            Self::Reply(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<x11rb::rust_connection::ConnectError> for X11CaptureError {
    fn from(e: x11rb::rust_connection::ConnectError) -> Self {
        Self::Connect(e)
    }
}

impl From<x11rb::errors::ConnectionError> for X11CaptureError {
    fn from(e: x11rb::errors::ConnectionError) -> Self {
        Self::Connection(e)
    }
}

impl From<x11rb::errors::ReplyError> for X11CaptureError {
    fn from(e: x11rb::errors::ReplyError) -> Self {
        Self::Reply(e)
    }
}

impl From<x11rb::errors::ReplyOrIdError> for X11CaptureError {
    fn from(e: x11rb::errors::ReplyOrIdError) -> Self {
        match e {
            x11rb::errors::ReplyOrIdError::ConnectionError(ce) => Self::Connection(ce),
            other => Self::Io(std::io::Error::other(format!("{other}"))),
        }
    }
}

impl From<std::io::Error> for X11CaptureError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ── Screen info ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct ScreenInfo {
    root: Window,
    width: u16,
    height: u16,
    depth: u8,
    bits_per_pixel: u8,
    bytes_per_line: u32,
}

impl ScreenInfo {
    fn root_drawable(&self) -> Window {
        self.root
    }
}

// ── X11Capture ────────────────────────────────────────────────────────────────

pub struct X11Capture {
    conn: RustConnection,
    screens: Vec<ScreenInfo>,
    shm_available: bool,
}

impl X11Capture {
    /// Connect to the X11 server and query screen information.
    pub fn connect() -> Result<Self, X11CaptureError> {
        let (conn, _screen_num) = x11rb::connect(None)?;
        let setup = conn.setup();

        // Detect XWayland — its root window is transparent and cannot capture
        // the Wayland desktop, so any screenshot will be black.
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            return Err(X11CaptureError::XWaylandDetected);
        }

        let mut screens = Vec::new();
        for screen in &setup.roots {
            let root = screen.root;
            let width = screen.width_in_pixels;
            let height = screen.height_in_pixels;
            let depth = screen.root_depth;

            let pixmap_format = setup
                .pixmap_formats
                .iter()
                .find(|pf| pf.depth == depth)
                .ok_or_else(|| {
                    X11CaptureError::Io(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "pixmap format not found for visual depth",
                    ))
                })?;

            let bits_per_pixel = pixmap_format.bits_per_pixel;
            let bytes_per_line = (width as u32 * bits_per_pixel as u32).div_ceil(32) * 4;

            screens.push(ScreenInfo {
                root,
                width,
                height,
                depth,
                bits_per_pixel,
                bytes_per_line,
            });
        }

        let shm_available = conn
            .extension_information(shm::X11_EXTENSION_NAME)?
            .is_some();

        Ok(Self {
            conn,
            screens,
            shm_available,
        })
    }

    /// Number of screens (monitors) available.
    pub fn screen_count(&self) -> usize {
        self.screens.len()
    }

    /// Capture the screen at zero-based `screen_index`.
    pub fn capture_screen(&mut self, screen_index: usize) -> Result<Screenshot, X11CaptureError> {
        let screen = self
            .screens
            .get(screen_index)
            .ok_or(X11CaptureError::ScreenNotFound(screen_index))?
            .clone();

        let width = screen.width as u32;
        let height = screen.height as u32;
        let depth = screen.depth;
        let bits_per_pixel = screen.bits_per_pixel;
        let bytes_per_line = screen.bytes_per_line;

        let raw: Vec<u8> = if self.shm_available {
            #[cfg(unix)]
            {
                self.shm_get_image(&screen, width, height, bytes_per_line)?
            }
            #[cfg(not(unix))]
            {
                self.get_image_fallback(&screen, width, height)?
            }
        } else {
            self.get_image_fallback(&screen, width, height)?
        };

        convert_to_rgba(&raw, width, height, depth, bits_per_pixel, bytes_per_line).ok_or(
            X11CaptureError::UnsupportedVisual {
                depth,
                bits_per_pixel,
            },
        )
    }

    #[cfg(unix)]
    fn shm_get_image(
        &mut self,
        screen: &ScreenInfo,
        width: u32,
        height: u32,
        bytes_per_line: u32,
    ) -> Result<Vec<u8>, X11CaptureError> {
        let seg_id: u32 = self.conn.generate_id()?;
        let size = (bytes_per_line * height) as usize;

        let shmid = unsafe { libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0o600) };
        if shmid < 0 {
            return Err(X11CaptureError::Io(std::io::Error::last_os_error()));
        }

        let ptr = unsafe { libc::shmat(shmid, std::ptr::null::<c_void>(), 0) };
        if ptr as isize == -1 {
            unsafe { libc::shmctl(shmid, libc::IPC_RMID, std::ptr::null_mut()) };
            return Err(X11CaptureError::Io(std::io::Error::last_os_error()));
        }

        if let Err(e) = self.conn.shm_attach(seg_id, shmid as u32, false)?.check() {
            unsafe { libc::shmdt(ptr) };
            unsafe { libc::shmctl(shmid, libc::IPC_RMID, std::ptr::null_mut()) };
            return Err(X11CaptureError::Reply(e));
        }

        let _guard = ShmCleanup {
            conn: &self.conn,
            seg_id,
            shmid,
            ptr,
        };

        self.conn
            .shm_get_image(
                screen.root_drawable(),
                0,
                0,
                width as u16,
                height as u16,
                !0u32,
                ImageFormat::Z_PIXMAP.into(),
                seg_id,
                0,
            )?
            .reply()?;

        let raw = unsafe { std::slice::from_raw_parts(ptr as *const u8, size).to_vec() };

        Ok(raw)
    }

    fn get_image_fallback(
        &mut self,
        screen: &ScreenInfo,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, X11CaptureError> {
        let reply = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                screen.root_drawable(),
                0,
                0,
                width as u16,
                height as u16,
                !0u32,
            )?
            .reply()?;

        Ok(reply.data)
    }

    /// Capture all available screens.
    pub fn capture_all(&mut self) -> Result<Vec<Screenshot>, X11CaptureError> {
        (0..self.screen_count())
            .map(|i| self.capture_screen(i))
            .collect()
    }
}

// ── Pixel conversion ──────────────────────────────────────────────────────────

fn convert_to_rgba(
    raw: &[u8],
    width: u32,
    height: u32,
    depth: u8,
    bits_per_pixel: u8,
    bytes_per_line: u32,
) -> Option<Screenshot> {
    match (depth, bits_per_pixel) {
        (24..=32, 32) => {
            let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
            for chunk in raw.chunks_exact(4) {
                rgba.extend_from_slice(&[chunk[2], chunk[1], chunk[0], 255]);
            }
            Some(Screenshot::from_rgba(width, height, rgba))
        }
        (24, 24) => {
            let row_stride = bytes_per_line as usize;
            let pixels_per_row = width as usize * 3;
            let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
            for row in 0..height as usize {
                let row_start = row * row_stride;
                let row_end = row_start + pixels_per_row;
                if row_end > raw.len() {
                    return None;
                }
                let row_data = &raw[row_start..row_end];
                for chunk in row_data.chunks_exact(3) {
                    rgba.extend_from_slice(&[chunk[2], chunk[1], chunk[0], 255]);
                }
            }
            Some(Screenshot::from_rgba(width, height, rgba))
        }
        _ => None,
    }
}

// ── Capture ────────────────────────────────────────────────────────────────

impl Capture for X11Capture {
    type Error = X11CaptureError;

    fn capture(&mut self) -> Result<Screenshot, Self::Error> {
        self.capture_screen(0)
    }
}

impl MultiCapture for X11Capture {
    fn source_count(&self) -> usize {
        self.screen_count()
    }

    fn capture_index(&mut self, index: usize) -> Result<Screenshot, Self::Error> {
        self.capture_screen(index)
    }
}
