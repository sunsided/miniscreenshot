# miniscreenshot

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot.svg)](https://crates.io/crates/miniscreenshot)
[![License](https://img.shields.io/crates/l/miniscreenshot.svg)](#license)

Core crate of the [miniscreenshot](https://github.com/sunsided/miniscreenshot)
workspace: the `Screenshot` type, PNG / PPM / PGM encoding, and the `Capture` /
`CaptureAsync` / `MultiCapture` traits that every backend crate implements.

## Usage

```toml
[dependencies]
miniscreenshot = "0.3"
```

```rust
use miniscreenshot::Screenshot;

// Build from raw RGBA8 pixel data
let data = vec![255u8, 0, 0, 255]; // 1×1 red pixel
let shot = Screenshot::from_rgba(1, 1, data);

// Save — format inferred from extension (.png / .ppm / .pgm)
shot.save("screenshot.png").unwrap();

// Or let a "screenshot key" handler name the file for you:
// writes screenshots/screenshot-YYYYMMDD-HHMMSS-mmm.png, returns the path.
let path = shot.save_in_dir_timestamped("screenshots").unwrap();

// Or encode to bytes explicitly
let png_bytes: Vec<u8> = shot.encode_png().unwrap();
let ppm_bytes: Vec<u8> = shot.encode_ppm();   // lossless, trivial format
let pgm_bytes: Vec<u8> = shot.encode_pgm();   // grayscale
```

## Output formats

| Format | Method | Notes |
|--------|--------|-------|
| PNG | `encode_png()` / `save("file.png")` | Lossless, widely supported |
| PPM | `encode_ppm()` / `save("file.ppm")` | Binary P6, trivial to parse |
| PGM | `encode_pgm()` / `save("file.pgm")` | Binary P5 grayscale (BT.601 luma) |

## The `Capture` trait

All system-capture backend crates (`-wayland`, `-x11`, `-portal`) implement
`Capture`, making backends interchangeable:

```rust
use miniscreenshot::Capture;

fn take_and_save<C: Capture>(cap: &mut C)
where C::Error: std::fmt::Debug
{
    let shot = cap.capture().unwrap();
    shot.save("output.png").unwrap();
}
```

### Closures as `Capture`

A blanket implementation lets any `FnMut() -> Result<Screenshot, E>` act as a
`Capture`, so free functions like `miniscreenshot_wgpu::capture` work directly:

```rust
let mut cap = || miniscreenshot_wgpu::capture(&device, &queue, &texture);
take_and_save(&mut cap);
```

### `MultiCapture`

For backends with multiple outputs (monitors), the `MultiCapture` super-trait
provides `source_count()`, `capture_index()`, and `capture_all()`:

```rust
use miniscreenshot::{Capture, MultiCapture};

fn capture_all_screens<C: MultiCapture>(cap: &mut C)
where C::Error: std::fmt::Debug
{
    println!("{} screen(s) found", cap.source_count());
    let shots = cap.capture_all().unwrap();
    for (i, shot) in shots.iter().enumerate() {
        shot.save(&format!("screen_{i}.png")).unwrap();
    }
}
```

## Example

```bash
cargo run -p miniscreenshot --example core_scene_screenshot
```

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
