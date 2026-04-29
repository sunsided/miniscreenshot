#[cfg(target_os = "linux")]
use miniscreenshot_portal::PortalCapture;

#[cfg(target_os = "linux")]
fn main() {
    let mut capture = PortalCapture::connect().expect("failed to connect to portal");
    println!("capturing screenshot interactively");
    let shot = capture
        .capture_interactive()
        .expect("failed to capture screenshot");
    let path = "portal_screenshot.png";
    shot.save(path).expect("failed to save screenshot");
    println!("saved {path}");
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("Portal capture is Linux-only");
}
