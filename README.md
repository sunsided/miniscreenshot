# miniscreenshot

A pluggable, multi-crate Rust workspace for taking screenshots of windowed
applications or the entire desktop.

---

## Crate overview

| Crate | Description |
|-------|-------------|
| [`miniscreenshot`](https://crates.io/crates/miniscreenshot) | **Core** — `Screenshot` type, PNG / PPM / PGM encoding, `ScreenshotProvider` / `AsyncScreenshotProvider` traits |
| [`miniscreenshot-softbuffer`](https://crates.io/crates/miniscreenshot-softbuffer) | [`softbuffer`](https://crates.io/crates/softbuffer) integration + re-export. Enable the `winit` feature to re-export [`winit`](https://crates.io/crates/winit) alongside softbuffer. |
| [`miniscreenshot-wgpu`](https://crates.io/crates/miniscreenshot-wgpu) | [`wgpu`](https://crates.io/crates/wgpu) texture readback + re-export |
| [`miniscreenshot-wayland`](https://crates.io/crates/miniscreenshot-wayland) | Wayland `wlr-screencopy-v1` system capture + re-exports |
| [`miniscreenshot-x11`](https://crates.io/crates/miniscreenshot-x11) | X11 (XGetImage / MIT-SHM) system capture + re-exports |
| [`miniscreenshot-portal`](https://crates.io/crates/miniscreenshot-portal) | XDG Desktop Portal (ashpd) system capture; works on GNOME, KDE, wlroots, and inside Flatpak/Snap |
| [`miniscreenshot-skia`](https://crates.io/crates/miniscreenshot-skia) | [`skia-safe`](https://crates.io/crates/skia-safe) re-export + surface screenshot helper |
| [`miniscreenshot-vello`](https://crates.io/crates/miniscreenshot-vello) | [`vello`](https://crates.io/crates/vello) re-export + pixel readback support |
| [`miniscreenshot-minifb`](https://crates.io/crates/miniscreenshot-minifb) | [`minifb`](https://crates.io/crates/minifb) re-export + pixel buffer screenshot helper |

---

## Design goals

* **Pluggable** — each rendering backend is a separate crate. Applications depend
  only on what they use.
* **No version conflicts** — every driver crate re-exports its underlying
  library (e.g., `miniscreenshot_wgpu::wgpu`). Depending on a driver crate
  is sufficient; no separate `wgpu`/`winit`/… dependency required.
* **Low-friction output formats** — PNG (default), PPM and PGM are supported
  out of the box. Format is inferred from the file extension.
* **System screenshots** — Linux Wayland via `zwlr_screencopy_manager_v1`
  (wlroots-based compositors: Sway, Hyprland, …), X11 via `XGetImage` with
  an MIT-SHM fast path, and XDG Desktop Portal via `ashpd` (GNOME, KDE,
  Flatpak, Snap).

---

## Quick start

### Core crate

```toml
[dependencies]
miniscreenshot = "0.1"
```

```rust
use miniscreenshot::Screenshot;

// Build from raw RGBA8 pixel data
let data = vec![255u8, 0, 0, 255]; // 1×1 red pixel
let shot = Screenshot::from_rgba(1, 1, data);

// Save — format inferred from extension (.png / .ppm / .pgm)
shot.save("screenshot.png").unwrap();

// Or encode to bytes explicitly
let png_bytes: Vec<u8> = shot.encode_png().unwrap();
let ppm_bytes: Vec<u8> = shot.encode_ppm();   // lossless, trivial format
let pgm_bytes: Vec<u8> = shot.encode_pgm();   // grayscale
```

### softbuffer backend

```toml
[dependencies]
miniscreenshot-softbuffer = "0.1"
```

```rust
use miniscreenshot_softbuffer::{softbuffer, screenshot_from_xrgb};

// softbuffer stores pixels as u32 XRGB8888 values
let pixels: &[u32] = /* buffer.deref() from softbuffer */ &[];
let shot = screenshot_from_xrgb(pixels, width, height);
shot.save("screenshot.png").unwrap();
```

### softbuffer + winit pairing

When you want to create a `softbuffer::Surface` from a `winit::Window`, enable
the `winit` feature. This re-exports `winit` alongside `softbuffer` at the same
version, avoiding dependency conflicts.

```toml
[dependencies]
miniscreenshot-softbuffer = { version = "0.1", features = ["winit"] }
```

```rust
use miniscreenshot_softbuffer::winit::window::Window;
use miniscreenshot_softbuffer::softbuffer;
use std::rc::Rc;

// Rc<Window> implements the raw-handle traits softbuffer needs.
let window: Rc<Window> = /* create window */;
let ctx = softbuffer::Context::new(window.clone()).unwrap();
let surface = softbuffer::Surface::new(&ctx, window.clone()).unwrap();
```

See the `softbuffer_winit_scene_screenshot` example for a complete demo.

### wgpu backend

```toml
[dependencies]
miniscreenshot-wgpu = "0.1"
```

```rust
use miniscreenshot_wgpu::{wgpu, capture_texture};

// `texture` must have been created with TextureUsages::COPY_SRC
let shot = capture_texture(&device, &queue, &texture).unwrap();
shot.save("screenshot.png").unwrap();
```

### Wayland system screenshot

```toml
[dependencies]
miniscreenshot-wayland = "0.1"
```

```rust
use miniscreenshot_wayland::WaylandCapture;

let mut cap = WaylandCapture::connect().expect("connect to Wayland");
println!("{} output(s) found", cap.output_count());

// Capture first monitor
let shot = cap.capture_output(0).expect("capture");
shot.save("screenshot.png").unwrap();

// Or capture all monitors at once
let shots = cap.capture_all().expect("capture all");
```

> **Compositor requirements:** Requires a Wayland compositor that implements
> `zwlr_screencopy_manager_v1` (wlroots-based — Sway, Hyprland, weston, cage,
> labwc, …). GNOME-on-Wayland and KWin do **not** implement this protocol and
> will return `WaylandCaptureError::NoScreencopyManager`. Use
> `miniscreenshot-portal` instead on these compositors.

### X11 system screenshot

```toml
[dependencies]
miniscreenshot-x11 = "0.1"
```

```rust
use miniscreenshot_x11::X11Capture;

let mut cap = X11Capture::connect().expect("connect to X11");
println!("{} screen(s) found", cap.screen_count());

// Capture first screen
let shot = cap.capture_screen(0).expect("capture");
shot.save("screenshot.png").unwrap();

// Or capture all screens at once
let shots = cap.capture_all().expect("capture all");
```

> **Server requirements:** Requires a reachable X11 server (`$DISPLAY` set).
> Uses MIT-SHM when available for a fast-path capture; otherwise falls back to
> a plain `XGetImage` transfer over the wire.

### Screenshot portal (GNOME / KDE / Flatpak)

```toml
[dependencies]
miniscreenshot-portal = "0.1"
```

Blocking usage (default):

```rust
use miniscreenshot_portal::PortalCapture;

let mut cap = PortalCapture::connect().expect("connect to portal");
let shot = cap.capture_interactive().expect("capture");
shot.save("screenshot.png").unwrap();
```

Async usage:

```toml
[dependencies]
miniscreenshot-portal = { version = "0.1", default-features = false, features = ["tokio"] }
```

```rust
use miniscreenshot_portal::PortalCapture;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cap = PortalCapture::connect_async().await?;
    let shot = cap.capture_interactive_async().await?;
    shot.save("screenshot.png")?;
    Ok(())
}
```

> **Portal requirements:** Requires a running desktop session with
> `$XDG_RUNTIME_DIR` and a portal implementation (`xdg-desktop-portal` +
> a backend such as `xdg-desktop-portal-gnome`, `-kde`, `-wlr`, or `-gtk`).
> GNOME always shows a confirmation dialog; KDE and wlroots may or may not
> depending on backend policy. Works inside Flatpak and Snap sandboxes.
> Use this crate on GNOME or KWin instead of `miniscreenshot-wayland`.

### minifb (prototyping window)

```toml
[dependencies]
miniscreenshot-minifb = "0.1"
```

```rust
use miniscreenshot_minifb::{minifb, screenshot_from_minifb};

// minifb stores pixels as u32 in 0RGB8888 format
let pixels: &[u32] = /* buffer passed to Window::update_with_buffer() */ &[];
let shot = screenshot_from_minifb(pixels, width, height);
shot.save("screenshot.png").unwrap();
```

Full window example:

```rust
use miniscreenshot_minifb::minifb;
use miniscreenshot_minifb::screenshot_from_minifb;

fn main() {
    let (width, height) = (640, 480);
    let mut buffer = vec![0u32; width * height];

    // Fill buffer with content...
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            buffer[idx] = ((x as u32) << 16) | ((y as u32) << 8) | ((x ^ y) as u32 & 0xFF);
        }
    }

    let mut window = minifb::Window::new(
        "Demo",
        width,
        height,
        minifb::WindowOptions::default(),
    )
    .expect("failed to create window");

    window.update_with_buffer(&buffer, width, height).unwrap();

    // Capture the displayed buffer as a Screenshot
    let shot = screenshot_from_minifb(&buffer, width as u32, height as u32);
    shot.save("screenshot.png").unwrap();
    println!("saved screenshot");
}
```

### ScreenshotProvider trait

All driver crates implement (or wrap types that implement) the core
`ScreenshotProvider` trait, making backends interchangeable:

```rust
use miniscreenshot::ScreenshotProvider;

fn take_and_save<P: ScreenshotProvider>(provider: &mut P)
where P::Error: std::fmt::Debug
{
    let shot = provider.take_screenshot().unwrap();
    shot.save("output.png").unwrap();
}
```

---

## Optional features

```toml
# Winit (for softbuffer + winit integration)
miniscreenshot-softbuffer = { version = "0.1", features = ["winit"] }
```

### Async traits

The core `miniscreenshot` crate exposes the `AsyncScreenshotProvider` trait
with zero additional dependencies (the trait uses return-position
`impl Trait`). It is always available — no feature flag required:

```toml
miniscreenshot = "0.1"
```

### Portal features

`miniscreenshot-portal` exposes runtime and API-surface features. Enabling
a runtime (`tokio` or `async-std`) automatically enables the async API surface.

```toml
# Default: tokio runtime + blocking API + async API
miniscreenshot-portal = "0.1"

# Async-only with tokio (no blocking convenience methods)
miniscreenshot-portal = { version = "0.1", default-features = false, features = ["tokio"] }

# Async-only with async-std
miniscreenshot-portal = { version = "0.1", default-features = false, features = ["async-std"] }
```

The `tokio` and `async-std` runtime features are mutually exclusive. The
`blocking` API-surface feature is independent. The `async` API surface is
implied by whichever runtime you select, but can also be enabled standalone
if you want to provide your own executor.

---

## Output formats

| Format | Method | Notes |
|--------|--------|-------|
| PNG | `encode_png()` / `save("file.png")` | Lossless, widely supported |
| PPM | `encode_ppm()` / `save("file.ppm")` | Binary P6, trivial to parse |
| PGM | `encode_pgm()` / `save("file.pgm")` | Binary P5 grayscale (BT.601 luma) |

---

## Examples

Each crate ships with a self-contained `examples/<crate_short>_scene_screenshot.rs` that
renders a scene (or synthesises a buffer) and saves a PNG.

| Crate | Command | Headless? |
|-------|---------|-----------|
| `miniscreenshot` (core) | `cargo run -p miniscreenshot --example core_scene_screenshot` | Yes |
| `miniscreenshot-softbuffer` | `cargo run -p miniscreenshot-softbuffer --example softbuffer_scene_screenshot` | Yes |
| `miniscreenshot-softbuffer` (winit) | `cargo run -p miniscreenshot-softbuffer --example softbuffer_winit_scene_screenshot --features winit` | No (needs a display) |
| `miniscreenshot-wgpu` | `cargo run -p miniscreenshot-wgpu --example wgpu_scene_screenshot` | Yes |
| `miniscreenshot-wayland` | `cargo run -p miniscreenshot-wayland --example wayland_scene_screenshot` | No (needs wlroots-based Wayland compositor) |
| `miniscreenshot-x11` | `cargo run -p miniscreenshot-x11 --example x11_scene_screenshot` | No (needs `$DISPLAY` / X11 server) |
| `miniscreenshot-portal` | `cargo run -p miniscreenshot-portal --example portal_scene_screenshot` | No (needs desktop session with portal) |
| `miniscreenshot-portal` (async) | `cargo run -p miniscreenshot-portal --example portal_async_scene_screenshot --features async` | No (needs desktop session with portal) |
| `miniscreenshot-skia` | `cargo run -p miniscreenshot-skia --example skia_scene_screenshot` | Yes |
| `miniscreenshot-vello` | `cargo run -p miniscreenshot-vello --example vello_scene_screenshot` | Yes |
| `miniscreenshot-minifb` | `cargo run -p miniscreenshot-minifb --example minifb_scene_screenshot` | Yes |

Build all examples at once:

```bash
task examples:build
```

Build and run all headless examples:

```bash
task examples
```

---

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT) at your option.
