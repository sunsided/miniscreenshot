//! Desktop MCP server — serves the full desktop screenshot tool over HTTP.
//!
//! Run with:
//! ```text
//! cargo run --example desktop_mcp_server
//! ```
//!
//! Connect an MCP client to `http://127.0.0.1:8731/mcp`.

use miniscreenshot_mcp::ScreenshotServer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("miniscreenshot_mcp=info".parse()?),
        )
        .init();

    let capture = miniscreenshot_desktop::take;
    let server = ScreenshotServer::new(capture);
    info!("starting MCP server on 127.0.0.1:8731");
    server.serve().await?;

    Ok(())
}
