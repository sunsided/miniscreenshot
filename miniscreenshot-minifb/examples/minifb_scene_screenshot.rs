use miniscreenshot_minifb::{MinifbCapture, ScreenshotProvider};

fn main() {
    let size = 256u32;
    let mut pixels = vec![0u32; (size * size) as usize];

    for y in 0..size {
        for x in 0..size {
            let r = x;
            let g = y;
            let b = (x ^ y) & 0xFF;
            let idx = (y * size + x) as usize;
            pixels[idx] = (r << 16) | (g << 8) | b;
        }
    }

    let mut capture = MinifbCapture::new(&pixels, size, size);
    let shot = capture.take_screenshot().expect("screenshot failed");
    let path = "minifb_screenshot.png";
    shot.save(path).expect("failed to save screenshot");
    println!("saved {path}");
}
