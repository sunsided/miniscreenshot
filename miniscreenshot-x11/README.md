# miniscreenshot-x11

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot-x11.svg)](https://crates.io/crates/miniscreenshot-x11)
[![License](https://img.shields.io/crates/l/miniscreenshot-x11.svg)](#license)

X11 system screenshots (`XGetImage` with an MIT-SHM fast path) for the
[miniscreenshot](https://github.com/sunsided/miniscreenshot) workspace.

## Usage

```toml
[dependencies]
miniscreenshot-x11 = "0.3"
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

`X11Capture` implements `Capture` and `MultiCapture` from
[`miniscreenshot`](https://crates.io/crates/miniscreenshot).

## Server requirements

Requires a reachable X11 server (`$DISPLAY` set). Uses MIT-SHM when available
for a fast-path capture; otherwise falls back to a plain `XGetImage` transfer
over the wire.

## Example

```bash
cargo run -p miniscreenshot-x11 --example x11_scene_screenshot
```

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
