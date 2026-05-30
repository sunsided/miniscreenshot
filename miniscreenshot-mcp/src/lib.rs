//! MCP server exposing a screenshot capture tool over the Model Context Protocol.
//!
//! This crate provides [`ScreenshotServer`] for synchronous [`Capture`] implementors
//! and [`AsyncScreenshotServer`] for async [`CaptureAsync`] implementors, both
//! serving over rmcp's streamable HTTP transport.
//!
//! # Quick start
//!
//! ```no_run
//! use miniscreenshot_mcp::ScreenshotServer;
//! use miniscreenshot::{Screenshot, CaptureError};
//!
//! // Create a capture closure with explicit error type
//! let mut capture = || -> Result<Screenshot, CaptureError> {
//!     unimplemented!()
//! };
//! let server = ScreenshotServer::new(capture);
//! // server.serve().await.unwrap();
//! ```

pub use rmcp;

use std::{
    fmt::Display,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
};

use miniscreenshot::{Capture, CaptureAsync, ImageFormat, Screenshot};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    ErrorData as McpError, RoleServer, ServerHandler,
};
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::Mutex;

// ── Constants ───────────────────────────────────────────────────────────────

/// Default port for the MCP HTTP server.
pub const DEFAULT_PORT: u16 = 8731;

// ── ServerError ─────────────────────────────────────────────────────────────

/// Errors that can occur while running the MCP server.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Failed to bind to the requested address.
    #[error("failed to bind to {addr}: {source}")]
    Bind {
        addr: SocketAddr,
        source: std::io::Error,
    },

    /// Transport-level error (e.g., connection dropped, I/O failure).
    #[error("transport error: {0}")]
    Transport(#[from] std::io::Error),
}

// ── ServeHandle ─────────────────────────────────────────────────────────────

/// A handle to a running server, returned by [`ScreenshotServer::serve_with_handle`]
/// and [`AsyncScreenshotServer::serve_with_handle`].
pub struct ServeHandle {
    /// The local address the server is bound to.
    pub local_addr: SocketAddr,
    /// Send on this to trigger server shutdown.
    pub shutdown: tokio::sync::oneshot::Sender<()>,
    /// The join handle for the server task. Awaits `Result<(), ServerError>`.
    pub join: tokio::task::JoinHandle<Result<(), ServerError>>,
}

// ── ServerConfig ────────────────────────────────────────────────────────────

/// Configuration for the MCP server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Bind address. Defaults to `127.0.0.1`.
    pub ip: IpAddr,
    /// Port. Defaults to [`DEFAULT_PORT`].
    pub port: u16,
    /// HTTP path for the MCP endpoint. Defaults to `"/mcp"`.
    pub path: String,
    /// Optional allow-list root: if `Some`, written files must be inside.
    pub allowed_root: Option<PathBuf>,
    /// Server name reported in MCP initialize.
    pub server_name: String,
    /// Server version reported in MCP initialize.
    pub server_version: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: DEFAULT_PORT,
            path: "/mcp".to_owned(),
            allowed_root: None,
            server_name: "miniscreenshot-mcp".to_owned(),
            server_version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

impl ServerConfig {
    fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.port)
    }
}

// ── Tool input types ────────────────────────────────────────────────────────

/// Explicit image format override for the screenshot tool.
#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Png,
    Ppm,
    Pgm,
}

impl Format {
    fn to_image_format(self) -> ImageFormat {
        match self {
            Format::Png => ImageFormat::Png,
            Format::Ppm => ImageFormat::Ppm,
            Format::Pgm => ImageFormat::Pgm,
        }
    }
}

/// Input arguments for the `screenshot` MCP tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ScreenshotArgs {
    /// Optional path to write the screenshot to. When omitted, the image is
    /// returned inline only and nothing is written to disk (handy for an agent
    /// that just wants to *see* the current frame). Format is inferred from the
    /// extension (.png, .ppm, .pgm); defaults to PNG.
    #[serde(default)]
    pub path: Option<String>,
    /// Optional explicit format override for the file written to `path`.
    #[serde(default)]
    pub format: Option<Format>,
    /// If true, also return the encoded image inline as an MCP ImageContent
    /// alongside the text confirmation. Forced on when `path` is omitted.
    /// Default false when `path` is set (path-only keeps the response small).
    #[serde(default)]
    pub include_image: bool,
    /// When an image is returned inline, downscale it so its longest side is at
    /// most this many pixels (aspect preserved). The file written to `path` is
    /// always full resolution. Defaults to [`DEFAULT_INLINE_MAX_DIM`]; set `0`
    /// to send the inline image at full resolution.
    #[serde(default)]
    pub max_dimension: Option<u32>,
}

// ── Path validation ─────────────────────────────────────────────────────────

/// Validates the output path before capture.
///
/// Rules:
/// 1. Reject empty paths.
/// 2. Canonicalize the parent directory; error if non-existent or not writable.
/// 3. If `allowed_root` is set, ensure the canonicalized parent is inside it.
/// 4. Reject if a file/directory/symlink already exists at `path`.
/// 5. Infer `ImageFormat` from extension (or from explicit `format` arg).
pub fn validate_output_path(
    path: &str,
    format_override: Option<Format>,
    allowed_root: Option<&PathBuf>,
) -> Result<(PathBuf, ImageFormat), McpError> {
    if path.is_empty() {
        return Err(McpError::invalid_params("path must not be empty", None));
    }

    let path_buf = PathBuf::from(path);

    let parent = path_buf
        .parent()
        .ok_or_else(|| McpError::invalid_params("path has no parent directory", None))?;

    let canonical_parent = parent.canonicalize().map_err(|e| {
        McpError::invalid_params(
            format!("parent directory does not exist or is not accessible: {e}"),
            None,
        )
    })?;

    if !canonical_parent.is_dir() {
        return Err(McpError::invalid_params(
            "parent path is not a directory",
            None,
        ));
    }

    // Writability probe
    check_writable(&canonical_parent)?;

    // allowed_root traversal guard
    if let Some(root) = allowed_root {
        let canonical_root = root.canonicalize().map_err(|e| {
            McpError::invalid_params(format!("allowed_root does not exist: {e}"), None)
        })?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(McpError::invalid_params(
                format!(
                    "path is outside the allowed root '{}'",
                    canonical_root.display()
                ),
                None,
            ));
        }
    }

    // Reject if already exists
    if path_buf
        .try_exists()
        .map_err(|e| McpError::invalid_params(format!("cannot check if path exists: {e}"), None))?
    {
        return Err(McpError::invalid_params(
            format!("file or directory already exists: {}", path_buf.display()),
            None,
        ));
    }

    // Infer format
    let format = if let Some(fmt) = format_override {
        fmt.to_image_format()
    } else {
        path_buf
            .extension()
            .and_then(|e| e.to_str())
            .and_then(ImageFormat::from_extension)
            .unwrap_or(ImageFormat::Png)
    };

    Ok((path_buf, format))
}

fn check_writable(dir: &Path) -> Result<(), McpError> {
    let probe = dir.join(".mcp_write_probe_tmp");
    if std::fs::File::create(&probe).is_err() {
        return Err(McpError::invalid_params(
            "parent directory is not writable",
            None,
        ));
    }
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

// ── Response building (shared by sync + async tool handlers) ─────────────────

/// Default longest-edge cap (in pixels) for inline images, sized to be
/// friendly to a token-budgeted model context. Override per call with
/// `max_dimension`; `0` disables downscaling.
pub const DEFAULT_INLINE_MAX_DIM: u32 = 1568;

/// Resolve the optional output target *before* capture, so a bad path fails
/// fast without taking a screenshot. Returns `None` for an inline-only request
/// (no `path` given).
fn resolve_target(
    args: &ScreenshotArgs,
    allowed_root: Option<&PathBuf>,
) -> Result<Option<(PathBuf, ImageFormat)>, McpError> {
    match args.path.as_deref() {
        Some(path) => Ok(Some(validate_output_path(path, args.format, allowed_root)?)),
        None => Ok(None),
    }
}

/// Encode the inline image (optionally downscaled) and, when `target` is set,
/// save the full-resolution screenshot to disk. Builds the tool response.
///
/// When `target` is `None` the image is always returned inline — that is the
/// only output for a path-less request.
async fn build_response(
    screenshot: Screenshot,
    target: Option<(PathBuf, ImageFormat)>,
    include_image: bool,
    max_dimension: Option<u32>,
) -> Result<CallToolResult, McpError> {
    let want_image = include_image || target.is_none();
    let (width, height) = (screenshot.width(), screenshot.height());

    // Inline copy is downscaled; the on-disk file stays full resolution.
    let inline_png =
        if want_image {
            let max = max_dimension.unwrap_or(DEFAULT_INLINE_MAX_DIM);
            let scaled = screenshot.downscale_to(max);
            Some(scaled.encode_png().map_err(|e| {
                McpError::internal_error(format!("failed to encode PNG: {e}"), None)
            })?)
        } else {
            None
        };

    let text = if let Some((path_buf, format)) = target {
        let (size, path_buf) = tokio::task::spawn_blocking(move || {
            screenshot.save_as(&path_buf, format).map_err(|e| {
                McpError::internal_error(format!("failed to save screenshot: {e}"), None)
            })?;
            std::fs::metadata(&path_buf)
                .map(|m| (m.len(), path_buf))
                .map_err(|e| {
                    McpError::internal_error(format!("cannot read saved file metadata: {e}"), None)
                })
        })
        .await
        .map_err(|e| McpError::internal_error(format!("save task panicked: {e}"), None))??;
        format!(
            "Screenshot saved to {} ({width}x{height}, {size} bytes, {format:?})",
            path_buf.display(),
        )
    } else {
        format!("Captured {width}x{height}; returned inline (not saved to disk).")
    };

    let mut content = vec![Content::text(text)];
    if let Some(png) = inline_png {
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png);
        content.push(Content::image(b64, "image/png"));
    }
    Ok(CallToolResult::success(content))
}

// ── Helper: bind + axum router construction ─────────────────────────────────

async fn bind_and_serve<S>(
    config: &ServerConfig,
    service_factory: impl Fn() -> Result<S, std::io::Error> + Send + Sync + 'static,
) -> Result<ServeHandle, ServerError>
where
    S: rmcp::Service<RoleServer> + Send + 'static,
{
    let addr = config.socket_addr();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| ServerError::Bind { addr, source: e })?;
    let local_addr = listener.local_addr().map_err(ServerError::Transport)?;

    let path = config.path.clone();
    let session_manager = Arc::new(LocalSessionManager::default());
    let http_config = StreamableHttpServerConfig::default();

    let service = StreamableHttpService::new(service_factory, session_manager, http_config);

    let router = axum::Router::new().nest_service(&path, service);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let join = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .map_err(ServerError::Transport)
    });

    Ok(ServeHandle {
        local_addr,
        shutdown: shutdown_tx,
        join,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// ScreenshotServer — synchronous Capture
// ═══════════════════════════════════════════════════════════════════════════

/// The internal service struct for sync capture.
struct SyncScreenshotService<C> {
    capture: Arc<Mutex<C>>,
    config: ServerConfig,
    tool_router: ToolRouter<Self>,
}

impl<C> Clone for SyncScreenshotService<C> {
    fn clone(&self) -> Self {
        Self {
            capture: Arc::clone(&self.capture),
            config: self.config.clone(),
            tool_router: self.tool_router.clone(),
        }
    }
}

#[tool_router]
impl<C> SyncScreenshotService<C>
where
    C: Capture + Send + 'static,
    C::Error: Display,
{
    #[tool(
        name = "screenshot",
        description = "Capture the host application's current frame (or the desktop). With `path`, saves the image there (format from the extension, or the `format` arg); without `path`, returns the image inline only and writes nothing. Use `max_dimension` to cap the inline image's longest side (default 1568px) so the response stays small."
    )]
    async fn screenshot(
        &self,
        Parameters(args): Parameters<ScreenshotArgs>,
    ) -> Result<CallToolResult, McpError> {
        let target = resolve_target(&args, self.config.allowed_root.as_ref())?;

        // Capture on spawn_blocking (sync capture may block).
        let screenshot = tokio::task::spawn_blocking({
            let capture = Arc::clone(&self.capture);
            move || {
                let mut guard = capture.blocking_lock();
                guard
                    .capture()
                    .map_err(|e| McpError::internal_error(format!("capture failed: {e}"), None))
            }
        })
        .await
        .map_err(|e| McpError::internal_error(format!("capture task panicked: {e}"), None))??;

        build_response(screenshot, target, args.include_image, args.max_dimension).await
    }
}

#[tool_handler]
impl<C> ServerHandler for SyncScreenshotService<C>
where
    C: Capture + Send + 'static,
    C::Error: Display,
{
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions("Screenshot capture server. Call the `screenshot` tool to capture the host application's current frame: pass a `path` to save it, or omit `path` to get the image back inline.".to_string())
    }
}

/// Screenshot MCP server for synchronous [`Capture`] implementors.
///
/// Tool invocations run the capture on `spawn_blocking` so the capture and
/// file save do not block the async runtime.
pub struct ScreenshotServer<C> {
    inner: SyncScreenshotService<C>,
}

impl<C, E> ScreenshotServer<C>
where
    C: Capture<Error = E> + Send + 'static,
    E: Display + Send + 'static,
{
    /// Wrap a synchronous Capture; tool invocations run on spawn_blocking.
    pub fn new(capture: C) -> Self {
        Self::with_config(capture, ServerConfig::default())
    }

    /// Wrap a Capture with custom configuration.
    pub fn with_config(capture: C, config: ServerConfig) -> Self {
        Self {
            inner: SyncScreenshotService {
                capture: Arc::new(Mutex::new(capture)),
                config,
                tool_router: SyncScreenshotService::tool_router(),
            },
        }
    }

    /// Bind and serve the MCP server. Returns when the server shuts down.
    pub async fn serve(self) -> Result<(), ServerError> {
        let handle = self.serve_with_handle_inner().await?;
        handle
            .join
            .await
            .map_err(|e| ServerError::Transport(std::io::Error::other(e.to_string())))?
    }

    /// Bind and return the local address + a JoinHandle for integration.
    pub async fn serve_with_handle(self) -> Result<ServeHandle, ServerError> {
        self.serve_with_handle_inner().await
    }

    async fn serve_with_handle_inner(self) -> Result<ServeHandle, ServerError> {
        let config = self.inner.config.clone();
        let inner = self.inner;
        bind_and_serve(&config, move || Ok(inner.clone())).await
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// AsyncScreenshotServer — CaptureAsync
// ═══════════════════════════════════════════════════════════════════════════

/// The internal service struct for async capture.
struct AsyncScreenshotService<C> {
    capture: Arc<Mutex<C>>,
    config: ServerConfig,
    tool_router: ToolRouter<Self>,
}

impl<C> Clone for AsyncScreenshotService<C> {
    fn clone(&self) -> Self {
        Self {
            capture: Arc::clone(&self.capture),
            config: self.config.clone(),
            tool_router: self.tool_router.clone(),
        }
    }
}

#[tool_router]
impl<C> AsyncScreenshotService<C>
where
    C: CaptureAsync + Send + 'static,
    C::Error: Display,
{
    #[tool(
        name = "screenshot",
        description = "Capture the host application's current frame (or the desktop). With `path`, saves the image there (format from the extension, or the `format` arg); without `path`, returns the image inline only and writes nothing. Use `max_dimension` to cap the inline image's longest side (default 1568px) so the response stays small."
    )]
    async fn screenshot(
        &self,
        Parameters(args): Parameters<ScreenshotArgs>,
    ) -> Result<CallToolResult, McpError> {
        let target = resolve_target(&args, self.config.allowed_root.as_ref())?;

        // Capture async (no spawn_blocking).
        let screenshot = {
            let mut guard = self.capture.lock().await;
            guard
                .capture()
                .await
                .map_err(|e| McpError::internal_error(format!("capture failed: {e}"), None))?
        };

        build_response(screenshot, target, args.include_image, args.max_dimension).await
    }
}

#[tool_handler]
impl<C> ServerHandler for AsyncScreenshotService<C>
where
    C: CaptureAsync + Send + 'static,
    C::Error: Display,
{
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions("Screenshot capture server. Call the `screenshot` tool to capture the host application's current frame: pass a `path` to save it, or omit `path` to get the image back inline.".to_string())
    }
}

/// Screenshot MCP server for async [`CaptureAsync`] implementors.
///
/// The capture is `.await`ed directly (no spawn_blocking), but the file save
/// still uses `spawn_blocking` to avoid stalling the runtime.
pub struct AsyncScreenshotServer<C> {
    inner: AsyncScreenshotService<C>,
}

impl<C, E> AsyncScreenshotServer<C>
where
    C: CaptureAsync<Error = E> + Send + 'static,
    E: Display + Send + 'static,
{
    /// Wrap an async Capture.
    pub fn new(capture: C) -> Self {
        Self::with_config(capture, ServerConfig::default())
    }

    /// Wrap an async Capture with custom configuration.
    pub fn with_config(capture: C, config: ServerConfig) -> Self {
        Self {
            inner: AsyncScreenshotService {
                capture: Arc::new(Mutex::new(capture)),
                config,
                tool_router: AsyncScreenshotService::tool_router(),
            },
        }
    }

    /// Bind and serve the MCP server. Returns when the server shuts down.
    pub async fn serve(self) -> Result<(), ServerError> {
        let handle = self.serve_with_handle_inner().await?;
        handle
            .join
            .await
            .map_err(|e| ServerError::Transport(std::io::Error::other(e.to_string())))?
    }

    /// Bind and return the local address + a JoinHandle for integration.
    pub async fn serve_with_handle(self) -> Result<ServeHandle, ServerError> {
        self.serve_with_handle_inner().await
    }

    async fn serve_with_handle_inner(self) -> Result<ServeHandle, ServerError> {
        let config = self.inner.config.clone();
        let inner = self.inner;
        bind_and_serve(&config, move || Ok(inner.clone())).await
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_empty_path() {
        let result = validate_output_path("", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_nonexistent_parent() {
        let result = validate_output_path("/nonexistent/dir/screenshot.png", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing.png");
        std::fs::File::create(&path).unwrap();

        let result = validate_output_path(path.to_str().unwrap(), None, None);
        assert!(result.is_err());
        let err = result.unwrap_err().message;
        assert!(err.contains("already exists"));
    }

    #[test]
    fn validate_accepts_happy_path_png() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.png");

        let (path_buf, format) = validate_output_path(path.to_str().unwrap(), None, None).unwrap();
        assert_eq!(path_buf, path);
        assert_eq!(format, ImageFormat::Png);
    }

    #[test]
    fn validate_infers_ppm_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.ppm");

        let (_, format) = validate_output_path(path.to_str().unwrap(), None, None).unwrap();
        assert_eq!(format, ImageFormat::Ppm);
    }

    #[test]
    fn validate_infers_pgm_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.pgm");

        let (_, format) = validate_output_path(path.to_str().unwrap(), None, None).unwrap();
        assert_eq!(format, ImageFormat::Pgm);
    }

    #[test]
    fn validate_defaults_to_png_for_unknown_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.xyz");

        let (_, format) = validate_output_path(path.to_str().unwrap(), None, None).unwrap();
        assert_eq!(format, ImageFormat::Png);
    }

    #[test]
    fn validate_format_override_wins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.png");

        let (_, format) =
            validate_output_path(path.to_str().unwrap(), Some(Format::Ppm), None).unwrap();
        assert_eq!(format, ImageFormat::Ppm);
    }

    #[test]
    fn validate_respects_allowed_root() {
        let root = tempfile::tempdir().unwrap();
        let inside = root.path().join("inside.png");
        let (_, format) = validate_output_path(
            inside.to_str().unwrap(),
            None,
            Some(&root.path().to_path_buf()),
        )
        .unwrap();
        assert_eq!(format, ImageFormat::Png);

        let outside_dir = tempfile::tempdir().unwrap();
        let outside = outside_dir.path().join("outside.png");
        let result = validate_output_path(
            outside.to_str().unwrap(),
            None,
            Some(&root.path().to_path_buf()),
        );
        assert!(result.is_err());
        let err = result.unwrap_err().message;
        assert!(err.contains("outside the allowed root"));
    }

    // ── resolve_target / build_response ──────────────────────────────────────

    fn args(path: Option<&str>, max_dimension: Option<u32>) -> ScreenshotArgs {
        ScreenshotArgs {
            path: path.map(str::to_owned),
            format: None,
            include_image: false,
            max_dimension,
        }
    }

    fn solid(width: u32, height: u32) -> Screenshot {
        Screenshot::from_rgba(width, height, vec![128u8; (width * height * 4) as usize])
    }

    #[test]
    fn resolve_target_none_when_no_path() {
        assert!(resolve_target(&args(None, None), None).unwrap().is_none());
    }

    #[test]
    fn resolve_target_some_when_path_set() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.png");
        let target = resolve_target(&args(Some(path.to_str().unwrap()), None), None).unwrap();
        let (buf, fmt) = target.unwrap();
        assert_eq!(buf, path);
        assert_eq!(fmt, ImageFormat::Png);
    }

    #[tokio::test]
    async fn build_response_saves_full_resolution_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.png");
        // include_image=false, but a tiny max_dimension must NOT shrink the file.
        build_response(
            solid(4, 2),
            Some((path.clone(), ImageFormat::Png)),
            false,
            Some(1),
        )
        .await
        .unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let info = reader.info();
        assert_eq!((info.width, info.height), (4, 2));
    }

    #[tokio::test]
    async fn build_response_without_path_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        build_response(solid(4, 2), None, false, Some(0))
            .await
            .unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
