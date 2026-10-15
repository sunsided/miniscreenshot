# miniscreenshot-portal

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot-portal.svg)](https://crates.io/crates/miniscreenshot-portal)
[![License](https://img.shields.io/crates/l/miniscreenshot-portal.svg)](#license)

XDG Desktop Portal screenshots (via [`ashpd`](https://crates.io/crates/ashpd))
for the [miniscreenshot](https://github.com/sunsided/miniscreenshot) workspace.
Works on GNOME, KDE, and wlroots-based compositors, and inside Flatpak and Snap
sandboxes.

## Usage

```toml
[dependencies]
miniscreenshot-portal = "0.3"
```

Blocking (default):

```rust
use miniscreenshot_portal::PortalCapture;

let mut cap = PortalCapture::connect().expect("connect to portal");
let shot = cap.capture_interactive().expect("capture");
shot.save("screenshot.png").unwrap();
```

Async:

```toml
[dependencies]
miniscreenshot-portal = { version = "0.3", default-features = false, features = ["tokio"] }
```

```rust
use miniscreenshot_portal::PortalCapture;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cap = PortalCapture::connect_async().await;
    let shot = cap.capture_interactive_async().await?;
    shot.save("screenshot.png")?;
    Ok(())
}
```

## Features

Enabling a runtime (`tokio` or `async-io`) automatically enables the async API.

```toml
# Default: tokio runtime + blocking API + async API
miniscreenshot-portal = "0.3"

# Async-only with tokio (no blocking convenience methods)
miniscreenshot-portal = { version = "0.3", default-features = false, features = ["tokio"] }

# Async-only with async-io
miniscreenshot-portal = { version = "0.3", default-features = false, features = ["async-io"] }
```

The `tokio` and `async-io` runtime features are mutually exclusive. The
`blocking` API feature is independent. The `async` API feature is implied by
either runtime, but can also be enabled on its own if you bring your own
executor.

## Portal requirements

Requires a running desktop session with `$XDG_RUNTIME_DIR` and a portal
implementation (`xdg-desktop-portal` plus a backend such as
`xdg-desktop-portal-gnome`, `-kde`, `-wlr`, or `-gtk`). GNOME always shows a
confirmation dialog; KDE and wlroots may or may not, depending on backend
policy. Use this crate instead of
[`miniscreenshot-wayland`](https://crates.io/crates/miniscreenshot-wayland) on
GNOME or KWin.

## Examples

```bash
cargo run -p miniscreenshot-portal --example portal_scene_screenshot
cargo run -p miniscreenshot-portal --example portal_async_scene_screenshot --features async
```

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
