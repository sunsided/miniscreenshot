# miniscreenshot-wgpu

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot-wgpu.svg)](https://crates.io/crates/miniscreenshot-wgpu)
[![License](https://img.shields.io/crates/l/miniscreenshot-wgpu.svg)](#license)

[`wgpu`](https://crates.io/crates/wgpu) texture readback for the
[miniscreenshot](https://github.com/sunsided/miniscreenshot) workspace. Re-exports
`wgpu` as `miniscreenshot_wgpu::wgpu` so your app and this crate always agree on
the version.

## Usage

```toml
[dependencies]
miniscreenshot-wgpu = "0.3"
```

```rust
use miniscreenshot_wgpu::{wgpu, capture};

// `texture` must have been created with TextureUsages::COPY_SRC
let shot = capture(&device, &queue, &texture).unwrap();
shot.save("screenshot.png").unwrap();
```

> **Capturing a frame you present:** the swapchain/surface texture is acquired
> without `COPY_SRC` and cannot be captured directly. Render your scene into an
> offscreen texture created with `RENDER_ATTACHMENT | COPY_SRC`, `capture` that,
> then blit it to the surface to present. See the `wgpu_scene_screenshot`
> example. From an async render loop, wrap the (blocking) `capture` call in
> `tokio::task::spawn_blocking`.

## wgpu version tracks

Pick exactly one wgpu version via a feature. Enabling none or more than one is
a compile error.

| Feature | wgpu | |
|---------|------|-|
| `wgpu-28` | 28 | |
| `wgpu-29` | 29 | default |
| `wgpu-30` | 30 | |

```toml
miniscreenshot-wgpu = { version = "0.3", default-features = false, features = ["wgpu-30"] }
```

On the `wgpu-30` track, `WgpuCaptureError` has an extra `MapRangeFailed`
variant, because wgpu 30 made mapped-range access fallible.

## Serving a live frame: `WgpuFrameTarget`

`WgpuCapture` borrows the device, queue, and texture, so it can't be handed to a
long-lived consumer such as the
[`miniscreenshot-mcp`](https://crates.io/crates/miniscreenshot-mcp) server.
`WgpuFrameTarget` owns clones of the device and queue plus a swappable
"current frame" slot. It is `Clone + Send + 'static` and implements `Capture`.

```rust
use miniscreenshot_wgpu::WgpuFrameTarget;

let target = WgpuFrameTarget::new(device.clone(), queue.clone());
// Hand `target.clone()` to the consumer, then publish your render target:
target.set_frame(frame_texture.clone());
```

`set_frame` stores the texture *handle*, not a snapshot. For a persistent
render target, publish it once and again only when you recreate the texture
(e.g. on resize). A capture reads the texture's current contents.

## Example

```bash
cargo run -p miniscreenshot-wgpu --example wgpu_scene_screenshot
```

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
