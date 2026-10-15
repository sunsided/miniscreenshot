# miniscreenshot-vello

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot-vello.svg)](https://crates.io/crates/miniscreenshot-vello)
[![License](https://img.shields.io/crates/l/miniscreenshot-vello.svg)](#license)

[`vello`](https://crates.io/crates/vello) screenshot helper for the
[miniscreenshot](https://github.com/sunsided/miniscreenshot) workspace. Re-exports
`vello` and the matching `wgpu` so your app and this crate always agree on the
versions.

## Usage

```toml
[dependencies]
miniscreenshot-vello = "0.3"
```

```rust
use miniscreenshot_vello::{capture, vello, wgpu};

// Render with vello into `texture` (created with STORAGE_BINDING | COPY_SRC) ...
let shot = capture(&device, &queue, &texture).expect("capture");
shot.save("screenshot.png").unwrap();
```

Vello renders on the GPU, so readback goes through a `wgpu` texture. `capture`
delegates to
[`miniscreenshot-wgpu`](https://crates.io/crates/miniscreenshot-wgpu) and
supports the same texture formats. This crate uses the `wgpu-29` track, which
is the wgpu version vello 0.9 is built on. `VelloCapture` wraps device, queue,
and texture and implements the `Capture` trait.

## Example

```bash
cargo run -p miniscreenshot-vello --example vello_scene_screenshot
```

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
