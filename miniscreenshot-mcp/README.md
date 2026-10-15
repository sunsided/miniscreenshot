# miniscreenshot-mcp

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot-mcp.svg)](https://crates.io/crates/miniscreenshot-mcp)
[![License](https://img.shields.io/crates/l/miniscreenshot-mcp.svg)](#license)

MCP server for the [miniscreenshot](https://github.com/sunsided/miniscreenshot)
workspace. Serves any `Capture` (or `CaptureAsync`) as a `screenshot` tool over
the Model Context Protocol, using streamable HTTP, so a coding agent can look
at your running game or editor while you develop it.

## Usage

```toml
[dependencies]
miniscreenshot-mcp = "0.3"
```

```rust
use miniscreenshot_mcp::ScreenshotServer;

// `capture` can be any `Capture` — a closure over your wgpu frame, or a
// desktop / wayland / x11 / portal backend.
let capture = miniscreenshot_desktop::take;
let server = ScreenshotServer::new(capture);
server.serve().await?; // serves on http://127.0.0.1:8731/mcp
```

The server is meant to live **inside the long-running process** you want to
inspect; the agent connects to it over HTTP. That's why the transport is HTTP
rather than stdio: a stdio server would be a fresh subprocess that can't see
your already-running game.

## The `screenshot` tool

Call it with **no path** to just *see* the current frame inline:

```json
{ "max_dimension": 768 }
```

or with a `path` to also save it to disk:

```json
{ "path": "/tmp/screenshot.png", "include_image": false }
```

| Argument | Type | Default | Description |
|----------|------|---------|-------------|
| `path` | `string` | _(none)_ | Where to save the screenshot. **Omit to return the image inline only** — nothing is written to disk. |
| `format` | `"png" \| "ppm" \| "pgm"` | `null` | Explicit format override for the saved file (extension is inferred otherwise) |
| `include_image` | `bool` | `false` | Also return the image inline as base64 `ImageContent`. Forced on when `path` is omitted. |
| `max_dimension` | `int` | `1568` | Cap the inline image's longest side (aspect preserved) to keep the response small. `0` sends it full-resolution. The saved file is always full-resolution. |

## Capture your game's frame (wgpu)

To serve **your rendered frame** rather than the desktop, give the server a
[`WgpuFrameTarget`](https://docs.rs/miniscreenshot-wgpu) from
[`miniscreenshot-wgpu`](https://crates.io/crates/miniscreenshot-wgpu):

```rust
use miniscreenshot_mcp::ScreenshotServer;
use miniscreenshot_wgpu::WgpuFrameTarget;

// Render into an offscreen texture created with RENDER_ATTACHMENT | COPY_SRC.
let target = WgpuFrameTarget::new(device.clone(), queue.clone());

// Hand one clone to the server (runs on its own task)…
let server = ScreenshotServer::new(target.clone());
tokio::spawn(server.serve());

// …and publish your persistent offscreen texture once. A capture reads its
// *current* contents, so you only re-publish when you recreate it (on resize).
target.set_frame(frame_texture.clone());

loop {
    // render_scene(&mut encoder, &view); // draws into frame_texture in place
    queue.submit([encoder.finish()]);
}
```

wgpu serializes GPU work on the queue, so the readback observes a complete
frame.

## Custom configuration

```rust
use miniscreenshot_mcp::{ScreenshotServer, ServerConfig};

let config = ServerConfig {
    ip: "0.0.0.0".parse().unwrap(),
    port: 9000,
    path: "/mcp".into(),
    allowed_root: Some("/tmp/screenshots".into()),  // path traversal guard
    ..Default::default()
};

let server = ScreenshotServer::with_config(capture, config);
server.serve().await?;
```

## Async capture (`CaptureAsync`)

For async-native backends such as the portal:

```rust
use miniscreenshot_mcp::AsyncScreenshotServer;
use miniscreenshot_portal::PortalCapture;

let capture = PortalCapture::connect_async().await;
let server = AsyncScreenshotServer::new(capture);
server.serve().await?;
```

## Connecting a coding agent

Point your MCP-capable agent at the running server. With Claude Code:

```bash
claude mcp add --transport http screenshot http://127.0.0.1:8731/mcp
```

## Examples

```bash
cargo run -p miniscreenshot-mcp --example desktop_mcp_server
cargo run -p miniscreenshot-mcp --example portal_async_mcp_server
cargo run -p miniscreenshot-mcp --example wgpu_game_mcp_server
```

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
