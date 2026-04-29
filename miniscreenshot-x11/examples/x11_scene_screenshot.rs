#[cfg(unix)]
use miniscreenshot_x11::X11Capture;

#[cfg(unix)]
fn main() {
    let mut capture = X11Capture::connect().expect("failed to connect to X11 display");
    let count = capture.screen_count();
    if count == 0 {
        eprintln!("no X11 screens available");
        return;
    }
    println!("capturing screen 0 of {count}");
    let shot = capture.capture_screen(0).expect("failed to capture screen");
    let path = "x11_screenshot.png";
    shot.save(path).expect("failed to save screenshot");
    println!("saved {path}");
}

#[cfg(not(unix))]
fn main() {
    println!("X11 capture is not supported on this platform");
}
