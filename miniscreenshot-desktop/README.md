# miniscreenshot-desktop

[![Crates.io](https://img.shields.io/crates/v/miniscreenshot-desktop.svg)](https://crates.io/crates/miniscreenshot-desktop)
[![License](https://img.shields.io/crates/l/miniscreenshot-desktop.svg)](#license)

"Just take a screenshot" umbrella crate of the
[miniscreenshot](https://github.com/sunsided/miniscreenshot) workspace. A single
`take()` call picks the best available backend for the current desktop.

## Usage

```toml
[dependencies]
miniscreenshot-desktop = "0.3"
```

```rust
use miniscreenshot_desktop::take;

let shot = take().expect("screenshot");
shot.save("desktop.png").unwrap();
```

## Backend selection

Backends are tried in this order:

| Backend | Condition | Notes |
|---------|-----------|-------|
| Wayland ([`miniscreenshot-wayland`](https://crates.io/crates/miniscreenshot-wayland)) | `$WAYLAND_DISPLAY` set | Requires a wlroots-based compositor (`zwlr_screencopy_manager_v1`) |
| X11 ([`miniscreenshot-x11`](https://crates.io/crates/miniscreenshot-x11)) | `$DISPLAY` set, not XWayland | Uses MIT-SHM when available |
| Portal ([`miniscreenshot-portal`](https://crates.io/crates/miniscreenshot-portal)) | Always tried last | Works on GNOME, KDE, Flatpak, Snap; may show a confirmation dialog |

## License

Licensed under any of the European Union Public Licence 1.2, the MIT License,
or the Apache License 2.0, at your option (`EUPL-1.2 OR MIT OR Apache-2.0`).
