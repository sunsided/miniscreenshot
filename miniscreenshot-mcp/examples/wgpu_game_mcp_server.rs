//! wgpu game MCP server — serve **your game's current frame**, not the desktop.
//!
//! This is the use case the MCP integration is built for: while you develop a
//! wgpu game/editor, a coding agent connects to a server embedded in the
//! running process and grabs the frame you are rendering — not a screen capture.
//!
//! This example is a headless "fake game": it renders an animated frame into an
//! offscreen texture every ~16 ms and publishes it through a
//! [`WgpuFrameTarget`](miniscreenshot_wgpu::WgpuFrameTarget). An embedded
//! [`ScreenshotServer`] exposes the `screenshot` tool over HTTP, so each call
//! returns whatever the loop last rendered.
//!
//! Run with:
//! ```text
//! cargo run -p miniscreenshot-mcp --example wgpu_game_mcp_server
//! ```
//!
//! Then connect an MCP client to `http://127.0.0.1:8731/mcp` and call the
//! `screenshot` tool (omit `path` to get the current frame inline).

use std::time::Duration;

use miniscreenshot_mcp::ScreenshotServer;
use miniscreenshot_wgpu::{wgpu, WgpuFrameTarget};

const SIZE: u32 = 512;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // ---- wgpu init (headless; a real game already has its device/queue) ----
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .expect("failed to request adapter");
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        })
        .await
        .expect("failed to request device");

    // Persistent offscreen render target. COPY_SRC is required to capture it.
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("game_frame"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // The frame target is the bridge: one clone drives the render loop, the
    // other is handed to the server. Both share the same published-frame slot.
    let target = WgpuFrameTarget::new(device.clone(), queue.clone());

    // ---- render loop: animate a clear color, publish each finished frame ----
    std::thread::spawn({
        let (device, queue, target) = (device.clone(), queue.clone(), target.clone());
        move || {
            let mut frame = 0u32;
            loop {
                let t = f64::from(frame % 256) / 255.0;
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("frame"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: t,
                                    g: 0.2,
                                    b: 1.0 - t,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                }
                queue.submit(std::iter::once(encoder.finish()));

                // Publish the frame we just submitted. A capture reads this; the
                // queue serializes GPU work, so it observes a complete frame.
                target.set_frame(texture.clone());

                frame = frame.wrapping_add(1);
                std::thread::sleep(Duration::from_millis(16));
            }
        }
    });

    // ---- embedded MCP server: serves the game's current frame ----
    eprintln!("game running; MCP server on http://127.0.0.1:8731/mcp");
    ScreenshotServer::new(target).serve().await?;

    Ok(())
}
