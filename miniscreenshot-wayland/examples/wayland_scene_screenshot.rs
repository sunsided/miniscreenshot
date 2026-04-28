#[cfg(target_os = "linux")]
use miniscreenshot_wayland::WaylandCapture;

#[cfg(target_os = "linux")]
fn main() {
    let mut capture = WaylandCapture::connect().expect("failed to connect to Wayland compositor");
    let count = capture.output_count();
    if count == 0 {
        eprintln!("no Wayland outputs available");
        return;
    }
    println!("capturing output 0 of {count}");
    let shot = capture.capture_output(0).expect("failed to capture output");
    let path = "wayland_screenshot.png";
    shot.save(path).expect("failed to save screenshot");
    println!("saved {path}");
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("Wayland capture is Linux-only");
}
