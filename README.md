# miniscreenshot

A pluggable, multi-crate Rust workspace for taking screenshots of windowed
applications or the entire desktop.

---

## Crate overview

| Crate | Description |
|-------|-------------|
| [`miniscreenshot`](miniscreenshot/) | **Core** — `Screenshot` type, PNG / PPM / PGM encoding, `ScreenshotProvider` trait |
| [`miniscreenshot-softbuffer`](miniscreenshot-softbuffer/) | [`softbuffer`](https://crates.io/crates/softbuffer) integration + re-export |
| [`miniscreenshot-winit`](miniscreenshot-winit/) | [`winit`](https://crates.io/crates/winit) integration + re-export |
| [`miniscreenshot-wgpu`](miniscreenshot-wgpu/) | [`wgpu`](https://crates.io/crates/wgpu) texture readback + re-export |
| [`miniscreenshot-wayland`](miniscreenshot-wayland/) | Wayland `wlr-screencopy-v1` system capture + re-exports |
| [`miniscreenshot-skia`](miniscreenshot-skia/) | [`skia-safe`](https://crates.io/crates/skia-safe) re-export (opt-in `skia` feature) |
| [`miniscreenshot-vello`](miniscreenshot-vello/) | [`vello`](https://crates.io/crates/vello) re-export (opt-in `vello` feature) |

---

## Design goals

* **Pluggable** — each rendering / windowing backend is a separate crate.
  Applications depend only on what they use.
* **No version conflicts** — every driver crate re-exports its underlying
  library (e.g., `miniscreenshot_wgpu::wgpu`).  Depending on a driver crate
  is sufficient; no separate `wgpu`/`winit`/… dependency required.
* **Low-friction output formats** — PNG (default), PPM and PGM are supported
  out of the box.  Format is inferred from the file extension.
* **System screenshots** — Linux Wayland via `zwlr_screencopy_manager_v1`
  (wlroots-based compositors: Sway, Hyprland, …).

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

## Optional / feature-gated backends

`skia-safe` and `vello` are large, complex dependencies.  Their driver crates
compile without them by default; enable the optional feature to unlock the
re-export and any helpers:

```toml
# Skia
miniscreenshot-skia = { version = "0.1", features = ["skia"] }

# Vello
miniscreenshot-vello = { version = "0.1", features = ["vello"] }
```

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
| `miniscreenshot-winit` | `cargo run -p miniscreenshot-winit --example winit_scene_screenshot` | No (needs a display) |
| `miniscreenshot-wgpu` | `cargo run -p miniscreenshot-wgpu --example wgpu_scene_screenshot` | Yes |
| `miniscreenshot-wayland` | `cargo run -p miniscreenshot-wayland --example wayland_scene_screenshot` | No (needs Wayland compositor) |
| `miniscreenshot-skia` | `cargo run -p miniscreenshot-skia --example skia_scene_screenshot --features skia` | Yes |
| `miniscreenshot-vello` | `cargo run -p miniscreenshot-vello --example vello_scene_screenshot --features vello` | Yes |

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
