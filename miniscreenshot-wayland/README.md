# miniscreenshot-wayland

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot-wayland.svg)](https://crates.io/crates/miniscreenshot-wayland)
[![License](https://img.shields.io/crates/l/miniscreenshot-wayland.svg)](#license)

Wayland system screenshots via `wlr-screencopy-v1` for the
[miniscreenshot](https://github.com/sunsided/miniscreenshot) workspace.

## Usage

```toml
[dependencies]
miniscreenshot-wayland = "0.3"
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

`WaylandCapture` implements `Capture` and `MultiCapture` from
[`miniscreenshot`](https://crates.io/crates/miniscreenshot).

## Compositor requirements

Requires a Wayland compositor that implements `zwlr_screencopy_manager_v1`
(wlroots-based: Sway, Hyprland, weston, cage, labwc, …). GNOME-on-Wayland and
KWin do **not** implement this protocol and return
`WaylandCaptureError::NoScreencopyManager`. Use
[`miniscreenshot-portal`](https://crates.io/crates/miniscreenshot-portal) on
those compositors, or
[`miniscreenshot-desktop`](https://crates.io/crates/miniscreenshot-desktop) to
pick a backend automatically.

## Example

```bash
cargo run -p miniscreenshot-wayland --example wayland_scene_screenshot
```

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
