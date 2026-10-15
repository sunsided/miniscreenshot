# miniscreenshot-skia

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot-skia.svg)](https://crates.io/crates/miniscreenshot-skia)
[![License](https://img.shields.io/crates/l/miniscreenshot-skia.svg)](#license)

[`skia-safe`](https://crates.io/crates/skia-safe) screenshot helper for the
[miniscreenshot](https://github.com/sunsided/miniscreenshot) workspace. Re-exports
`skia-safe` as `miniscreenshot_skia::skia_safe` so your app and this crate
always agree on the version.

## Usage

```toml
[dependencies]
miniscreenshot-skia = "0.3"
```

```rust
use miniscreenshot_skia::{capture, skia_safe::surfaces};

let mut surface = surfaces::raster_n32_premul((640, 480)).expect("surface");
// ... draw into surface.canvas() ...

let shot = capture(&mut surface).expect("capture");
shot.save("screenshot.png").unwrap();
```

`SkiaCapture` wraps a `&mut Surface` and implements the `Capture` trait from
[`miniscreenshot`](https://crates.io/crates/miniscreenshot).

## Example

```bash
cargo run -p miniscreenshot-skia --example skia_scene_screenshot
```

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
