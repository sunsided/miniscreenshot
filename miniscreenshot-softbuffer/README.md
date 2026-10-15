# miniscreenshot-softbuffer

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot-softbuffer.svg)](https://crates.io/crates/miniscreenshot-softbuffer)
[![License](https://img.shields.io/crates/l/miniscreenshot-softbuffer.svg)](#license)

[`softbuffer`](https://crates.io/crates/softbuffer) screenshot helper for the
[miniscreenshot](https://github.com/sunsided/miniscreenshot) workspace. Re-exports
`softbuffer` (and optionally `winit`) so you don't need a separate dependency
that could drift out of version.

## Usage

```toml
[dependencies]
miniscreenshot-softbuffer = "0.3"
```

```rust
use miniscreenshot_softbuffer::{softbuffer, capture};

// softbuffer stores pixels as u32 XRGB8888 values
let pixels: &[u32] = /* buffer.deref() from softbuffer */ &[];
let shot = capture(pixels, width, height);
shot.save("screenshot.png").unwrap();
```

Use `capture_argb` instead if your buffer carries a meaningful alpha channel.

## softbuffer + winit

To create a `softbuffer::Surface` from a `winit::Window`, enable the `winit`
feature. It re-exports `winit` alongside `softbuffer` at a matching version.

```toml
[dependencies]
miniscreenshot-softbuffer = { version = "0.3", features = ["winit"] }
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

## Examples

```bash
# Headless
cargo run -p miniscreenshot-softbuffer --example softbuffer_scene_screenshot

# With a window (needs a display)
cargo run -p miniscreenshot-softbuffer --example softbuffer_winit_scene_screenshot --features winit
```

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
