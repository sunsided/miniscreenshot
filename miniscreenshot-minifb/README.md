# miniscreenshot-minifb

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot-minifb.svg)](https://crates.io/crates/miniscreenshot-minifb)
[![License](https://img.shields.io/crates/l/miniscreenshot-minifb.svg)](#license)

[`minifb`](https://crates.io/crates/minifb) screenshot helper for the
[miniscreenshot](https://github.com/sunsided/miniscreenshot) workspace. Re-exports
`minifb` so your app and this crate always agree on the version. Handy for
quick prototyping windows.

## Usage

```toml
[dependencies]
miniscreenshot-minifb = "0.3"
```

```rust
use miniscreenshot_minifb::capture;

// minifb stores pixels as u32 in 0RGB8888 format
let pixels: &[u32] = /* buffer passed to Window::update_with_buffer() */ &[];
let shot = capture(pixels, width as u32, height as u32).unwrap();
shot.save("screenshot.png").unwrap();
```

Full window example:

```rust
use miniscreenshot_minifb::{capture, minifb};

fn main() {
    let (width, height) = (640, 480);
    let mut buffer = vec![0u32; width * height];

    for y in 0..height {
        for x in 0..width {
            buffer[y * width + x] =
                ((x as u32) << 16) | ((y as u32) << 8) | ((x ^ y) as u32 & 0xFF);
        }
    }

    let mut window =
        minifb::Window::new("Demo", width, height, minifb::WindowOptions::default())
            .expect("failed to create window");
    window.update_with_buffer(&buffer, width, height).unwrap();

    // Capture the displayed buffer as a Screenshot
    let shot = capture(&buffer, width as u32, height as u32).unwrap();
    shot.save("screenshot.png").unwrap();
}
```

Use `capture_with_format` for buffers in a different pixel layout.
`MinifbCapture` wraps a buffer and implements the `Capture` trait.

## Example

```bash
cargo run -p miniscreenshot-minifb --example minifb_scene_screenshot
```

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
