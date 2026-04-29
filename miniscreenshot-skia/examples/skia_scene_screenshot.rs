use miniscreenshot_skia::screenshot_from_surface;
use miniscreenshot_skia::skia_safe::{surfaces, Color, ISize, Paint, PaintStyle, Rect};

fn main() {
    let size = ISize::new(512, 512);
    let mut surface = surfaces::raster_n32_premul(size).expect("failed to create raster surface");

    let canvas = surface.canvas();
    canvas.clear(Color::from_rgb(30, 30, 60));

    let mut paint = Paint::default();
    paint.set_color(Color::from_rgb(255, 140, 50));
    paint.set_style(PaintStyle::Fill);
    canvas.draw_circle((256, 256), 120.0, &paint);

    paint.set_color(Color::from_rgb(50, 200, 255));
    canvas.draw_rect(Rect::new(30.0, 30.0, 200.0, 200.0), &paint);

    let shot = screenshot_from_surface(&mut surface);
    let path = "skia_screenshot.png";
    shot.save(path).expect("failed to save screenshot");
    println!("saved {path}");
}
