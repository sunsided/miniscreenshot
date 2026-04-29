//! Portal async MCP server — serves the XDG Portal screenshot tool over HTTP.
//!
//! This example uses the async (`CaptureAsync`) path with the portal backend.
//!
//! Run with:
//! ```text
//! cargo run --example portal_async_mcp_server
//! ```
//!
//! Connect an MCP client to `http://127.0.0.1:8731/mcp`.

use miniscreenshot_mcp::AsyncScreenshotServer;
use miniscreenshot_portal::PortalCapture;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("miniscreenshot_mcp=info".parse()?),
        )
        .init();

    let capture = PortalCapture::connect_async().await;
    let server = AsyncScreenshotServer::new(capture);
    info!("starting async MCP server on 127.0.0.1:8731");
    server.serve().await?;

    Ok(())
}
