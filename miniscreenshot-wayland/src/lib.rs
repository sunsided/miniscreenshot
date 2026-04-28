//! Wayland screen capture using the **wlr-screencopy-v1** protocol.
//!
//! Supports wlroots-based compositors such as [Sway] and [Hyprland].
//! On compositors that do not implement `zwlr_screencopy_manager_v1` this
//! crate returns [`WaylandCaptureError::NoScreencopyManager`].
//!
//! Both `wayland-client` and `wayland-protocols-wlr` are re-exported so
//! downstream consumers need not worry about version conflicts.
//!
//! [Sway]: https://swaywm.org/
//! [Hyprland]: https://hyprland.org/
//!
//! # Example
//!
//! ```rust,no_run
//! use miniscreenshot_wayland::WaylandCapture;
//!
//! let mut capture = WaylandCapture::connect().expect("connect to Wayland");
//! let shot = capture.capture_output(0).expect("capture first output");
//! shot.save("screenshot.png").unwrap();
//! ```

pub use wayland_client;
pub use wayland_protocols_wlr;
pub use miniscreenshot::{Screenshot, ScreenshotProvider};

use std::os::unix::io::AsFd;
use memmap2::MmapMut;
use wayland_client::{
    protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool},
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1,
};

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum WaylandCaptureError {
    Connection(wayland_client::ConnectError),
    NoScreencopyManager,
    OutputNotFound(usize),
    CaptureFailed,
    Dispatch(wayland_client::DispatchError),
    Io(std::io::Error),
}

impl std::fmt::Display for WaylandCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connection(e)     => write!(f, "Wayland connection error: {e}"),
            Self::NoScreencopyManager => write!(f, "compositor lacks zwlr_screencopy_manager_v1"),
            Self::OutputNotFound(i) => write!(f, "output index {i} not found"),
            Self::CaptureFailed     => write!(f, "compositor reported capture failure"),
            Self::Dispatch(e)       => write!(f, "Wayland dispatch error: {e}"),
            Self::Io(e)             => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for WaylandCaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Connection(e) => Some(e),
            Self::Dispatch(e)   => Some(e),
            Self::Io(e)         => Some(e),
            _                   => None,
        }
    }
}

// ── Internal state ────────────────────────────────────────────────────────────

struct AppState {
    screencopy_manager: Option<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1>,
    shm:     Option<wl_shm::WlShm>,
    outputs: Vec<wl_output::WlOutput>,
    frame:   Option<FrameCapture>,
}

impl AppState {
    fn new() -> Self {
        Self { screencopy_manager: None, shm: None, outputs: Vec::new(), frame: None }
    }
}

struct FrameCapture {
    shm_format:  Option<wl_shm::Format>,
    width:       u32,
    height:      u32,
    stride:      u32,
    buffer_done: bool,
    ready:       bool,
    failed:      bool,
    mmap:        Option<MmapMut>,
    wl_buffer:   Option<wl_buffer::WlBuffer>,
}

impl FrameCapture {
    fn new() -> Self {
        Self {
            shm_format: None, width: 0, height: 0, stride: 0,
            buffer_done: false, ready: false, failed: false,
            mmap: None, wl_buffer: None,
        }
    }
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(state: &mut Self, registry: &wl_registry::WlRegistry,
             event: wl_registry::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "zwlr_screencopy_manager_v1" => {
                    state.screencopy_manager = Some(registry.bind(name, version.min(3), qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind(name, version.min(1), qh, ()));
                }
                "wl_output" => {
                    state.outputs.push(registry.bind(name, version.min(4), qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_shm::WlShm, ()> for AppState {
    fn event(_: &mut Self, _: &wl_shm::WlShm, _: wl_shm::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for AppState {
    fn event(_: &mut Self, _: &wl_shm_pool::WlShmPool, _: wl_shm_pool::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_buffer::WlBuffer, ()> for AppState {
    fn event(_: &mut Self, _: &wl_buffer::WlBuffer, _: wl_buffer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_output::WlOutput, ()> for AppState {
    fn event(_: &mut Self, _: &wl_output::WlOutput, _: wl_output::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ()> for AppState {
    fn event(_: &mut Self, _: &zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
             _: zwlr_screencopy_manager_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ()> for AppState {
    fn event(state: &mut Self, _: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
             event: zwlr_screencopy_frame_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        let frame = match state.frame.as_mut() { Some(f) => f, None => return };
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer { format, width, height, stride } => {
                if frame.shm_format.is_none() {
                    if let WEnum::Value(fmt) = format {
                        frame.shm_format = Some(fmt);
                        frame.width  = width;
                        frame.height = height;
                        frame.stride = stride;
                    }
                }
            }
            zwlr_screencopy_frame_v1::Event::BufferDone     => { frame.buffer_done = true; }
            zwlr_screencopy_frame_v1::Event::Ready { .. }   => { frame.ready  = true; }
            zwlr_screencopy_frame_v1::Event::Failed         => { frame.failed = true; }
            _ => {}
        }
    }
}

// ── SHM helper ────────────────────────────────────────────────────────────────

fn create_shm_file(size: usize) -> std::io::Result<std::fs::File> {
    create_shm_file_impl(size)
}

#[cfg(target_os = "linux")]
fn create_shm_file_impl(size: usize) -> std::io::Result<std::fs::File> {
    use std::os::unix::io::FromRawFd;
    let name = b"miniscreenshot\0";
    let fd = unsafe {
        libc::syscall(libc::SYS_memfd_create, name.as_ptr(), libc::MFD_CLOEXEC as libc::c_uint)
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: syscall returned a valid, owned file descriptor.
    let file = unsafe { std::fs::File::from_raw_fd(fd as libc::c_int) };
    file.set_len(size as u64)?;
    Ok(file)
}

#[cfg(not(target_os = "linux"))]
fn create_shm_file_impl(size: usize) -> std::io::Result<std::fs::File> {
    // Create a temp file, unlink immediately, keep fd open.
    let path = format!("/tmp/miniscreenshot_{}", std::process::id());
    let file = std::fs::OpenOptions::new().read(true).write(true).create(true).open(&path)?;
    let _ = std::fs::remove_file(&path);
    file.set_len(size as u64)?;
    Ok(file)
}

// ── WaylandCapture ────────────────────────────────────────────────────────────

/// A Wayland screen-capture session backed by `zwlr_screencopy_manager_v1`.
pub struct WaylandCapture {
    event_queue: EventQueue<AppState>,
    state:       AppState,
}

impl WaylandCapture {
    /// Connect to the Wayland compositor and bind required globals.
    pub fn connect() -> Result<Self, WaylandCaptureError> {
        let conn = Connection::connect_to_env().map_err(WaylandCaptureError::Connection)?;
        let mut event_queue = conn.new_event_queue::<AppState>();
        let qh   = event_queue.handle();
        let mut state = AppState::new();

        conn.display().get_registry(&qh, ());
        event_queue.roundtrip(&mut state).map_err(WaylandCaptureError::Dispatch)?;

        if state.screencopy_manager.is_none() {
            return Err(WaylandCaptureError::NoScreencopyManager);
        }
        Ok(Self { event_queue, state })
    }

    /// Number of outputs (monitors) available.
    pub fn output_count(&self) -> usize { self.state.outputs.len() }

    /// Capture the output at zero-based `output_index`.
    pub fn capture_output(&mut self, output_index: usize) -> Result<Screenshot, WaylandCaptureError> {
        let output = self.state.outputs.get(output_index)
            .ok_or(WaylandCaptureError::OutputNotFound(output_index))?.clone();

        let qh = self.event_queue.handle();
        let manager = self.state.screencopy_manager.as_ref().unwrap();

        self.state.frame = Some(FrameCapture::new());
        let frame = manager.capture_output(0, &output, &qh, ());

        // Phase 1: wait for buffer info + buffer_done.
        loop {
            self.event_queue.blocking_dispatch(&mut self.state)
                .map_err(WaylandCaptureError::Dispatch)?;
            match &self.state.frame {
                Some(f) if f.failed => {
                    frame.destroy();
                    self.state.frame = None;
                    return Err(WaylandCaptureError::CaptureFailed);
                }
                Some(f) if f.buffer_done => break,
                _ => {}
            }
        }

        // Allocate SHM.
        let fc = self.state.frame.as_mut().unwrap();
        let shm_format = fc.shm_format.ok_or(WaylandCaptureError::CaptureFailed)?;
        let (width, height, stride) = (fc.width, fc.height, fc.stride);
        let buf_size = (stride * height) as usize;

        let file = create_shm_file(buf_size).map_err(WaylandCaptureError::Io)?;
        // SAFETY: `file` is a valid, non-empty, writable anonymous file.
        let mmap = unsafe { MmapMut::map_mut(&file) }.map_err(WaylandCaptureError::Io)?;

        let shm  = self.state.shm.as_ref().unwrap();
        let pool = shm.create_pool(file.as_fd(), buf_size as i32, &qh, ());
        let wl_buf = pool.create_buffer(0, width as i32, height as i32, stride as i32, shm_format, &qh, ());
        pool.destroy();

        let fc = self.state.frame.as_mut().unwrap();
        fc.mmap      = Some(mmap);
        fc.wl_buffer = Some(wl_buf.clone());
        frame.copy(&wl_buf);

        // Phase 2: wait for ready or failed.
        loop {
            self.event_queue.blocking_dispatch(&mut self.state)
                .map_err(WaylandCaptureError::Dispatch)?;
            match &self.state.frame {
                Some(f) if f.failed => {
                    frame.destroy();
                    wl_buf.destroy();
                    self.state.frame = None;
                    return Err(WaylandCaptureError::CaptureFailed);
                }
                Some(f) if f.ready => break,
                _ => {}
            }
        }

        // Extract pixels.
        let fc   = self.state.frame.take().unwrap();
        frame.destroy();
        wl_buf.destroy();

        let mmap = fc.mmap.unwrap();
        convert_to_rgba(&mmap, width, height, stride, shm_format)
            .ok_or(WaylandCaptureError::CaptureFailed)
    }

    /// Capture all available outputs.
    pub fn capture_all(&mut self) -> Result<Vec<Screenshot>, WaylandCaptureError> {
        (0..self.output_count()).map(|i| self.capture_output(i)).collect()
    }
}

// ── Pixel conversion ──────────────────────────────────────────────────────────

fn convert_to_rgba(raw: &[u8], width: u32, height: u32, stride: u32, format: wl_shm::Format)
    -> Option<Screenshot>
{
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for row in 0..height as usize {
        let start = row * stride as usize;
        let end   = start + width as usize * 4;
        let row_data = raw.get(start..end)?;
        match format {
            wl_shm::Format::Argb8888 => {
                // Little-endian u32 layout: [B, G, R, A]
                for p in row_data.chunks_exact(4) {
                    rgba.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
                }
            }
            wl_shm::Format::Xrgb8888 => {
                // Little-endian u32 layout: [B, G, R, X]
                for p in row_data.chunks_exact(4) {
                    rgba.extend_from_slice(&[p[2], p[1], p[0], 255]);
                }
            }
            _ => return None,
        }
    }
    Some(Screenshot::from_rgba(width, height, rgba))
}
