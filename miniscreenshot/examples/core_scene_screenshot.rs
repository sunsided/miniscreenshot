use miniscreenshot::Screenshot;

fn main() {
    let size = 256u32;
    let mut data = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            data.push(x as u8);
            data.push(y as u8);
            data.push(((x ^ y) & 0xFF) as u8);
            data.push(255);
        }
    }

    let shot = Screenshot::from_rgba(size, size, data);
    let path = "core_screenshot.png";
    shot.save(path).expect("failed to save screenshot");
    println!("saved {path}");
}
