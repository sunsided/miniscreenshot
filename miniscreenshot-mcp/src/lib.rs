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

use miniscreenshot::{Capture, CaptureAsync, ImageFormat};
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
    /// Absolute (or caller-relative) path to write the screenshot to.
    /// Format is inferred from the extension (.png, .ppm, .pgm); defaults to PNG.
    pub path: String,
    /// Optional explicit format override.
    #[serde(default)]
    pub format: Option<Format>,
    /// If true, return the encoded image as an MCP ImageContent alongside
    /// the text confirmation. Default false (path-only, keeps payload small).
    #[serde(default)]
    pub include_image: bool,
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
        description = "Capture a screenshot of the host application/desktop and save it to a file path of your choosing."
    )]
    async fn screenshot(
        &self,
        Parameters(args): Parameters<ScreenshotArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (path_buf, format) =
            validate_output_path(&args.path, args.format, self.config.allowed_root.as_ref())?;

        let include_image = args.include_image;

        // Capture on spawn_blocking (sync capture may block)
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

        let dimensions = (screenshot.width(), screenshot.height());
        let png_bytes = if include_image {
            Some(screenshot.encode_png().map_err(|e| {
                McpError::internal_error(format!("failed to encode PNG: {e}"), None)
            })?)
        } else {
            None
        };

        // Save in spawn_blocking
        let file_size = tokio::task::spawn_blocking({
            let path_buf = path_buf.clone();
            move || {
                screenshot.save_as(&path_buf, format).map_err(|e| {
                    McpError::internal_error(format!("failed to save screenshot: {e}"), None)
                })?;
                std::fs::metadata(&path_buf).map(|m| m.len()).map_err(|e| {
                    McpError::internal_error(format!("cannot read saved file metadata: {e}"), None)
                })
            }
        })
        .await
        .map_err(|e| McpError::internal_error(format!("save task panicked: {e}"), None))??;

        let mut content = vec![Content::text(format!(
            "Screenshot saved to {} ({}x{}, {} bytes, {:?})",
            path_buf.display(),
            dimensions.0,
            dimensions.1,
            file_size,
            format,
        ))];

        if let Some(png) = png_bytes {
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png);
            content.push(Content::image(b64, "image/png"));
        }

        Ok(CallToolResult::success(content))
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
            .with_instructions("Screenshot capture server. Use the `screenshot` tool to capture and save a screenshot.".to_string())
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
        handle.join.await.map_err(|e| {
            ServerError::Transport(std::io::Error::other(
                e.to_string(),
            ))
        })?
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
        description = "Capture a screenshot of the host application/desktop and save it to a file path of your choosing."
    )]
    async fn screenshot(
        &self,
        Parameters(args): Parameters<ScreenshotArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (path_buf, format) =
            validate_output_path(&args.path, args.format, self.config.allowed_root.as_ref())?;

        let include_image = args.include_image;

        // Capture async (no spawn_blocking)
        let screenshot = {
            let mut guard = self.capture.lock().await;
            guard
                .capture()
                .await
                .map_err(|e| McpError::internal_error(format!("capture failed: {e}"), None))?
        };

        let dimensions = (screenshot.width(), screenshot.height());
        let png_bytes = if include_image {
            Some(screenshot.encode_png().map_err(|e| {
                McpError::internal_error(format!("failed to encode PNG: {e}"), None)
            })?)
        } else {
            None
        };

        // Save in spawn_blocking
        let file_size = tokio::task::spawn_blocking({
            let path_buf = path_buf.clone();
            move || {
                screenshot.save_as(&path_buf, format).map_err(|e| {
                    McpError::internal_error(format!("failed to save screenshot: {e}"), None)
                })?;
                std::fs::metadata(&path_buf).map(|m| m.len()).map_err(|e| {
                    McpError::internal_error(format!("cannot read saved file metadata: {e}"), None)
                })
            }
        })
        .await
        .map_err(|e| McpError::internal_error(format!("save task panicked: {e}"), None))??;

        let mut content = vec![Content::text(format!(
            "Screenshot saved to {} ({}x{}, {} bytes, {:?})",
            path_buf.display(),
            dimensions.0,
            dimensions.1,
            file_size,
            format,
        ))];

        if let Some(png) = png_bytes {
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png);
            content.push(Content::image(b64, "image/png"));
        }

        Ok(CallToolResult::success(content))
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
            .with_instructions("Screenshot capture server. Use the `screenshot` tool to capture and save a screenshot.".to_string())
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
        handle.join.await.map_err(|e| {
            ServerError::Transport(std::io::Error::other(
                e.to_string(),
            ))
        })?
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
}
