use miniscreenshot_portal::PortalCapture;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut capture = PortalCapture::connect_async()
        .await
        .expect("failed to connect to portal");

    println!("capturing screenshot interactively");
    let shot = capture
        .capture_interactive_async()
        .await
        .expect("failed to capture screenshot");

    let path = "portal_screenshot.png";
    shot.save(path).expect("failed to save screenshot");
    println!("saved {path}");

    Ok(())
}
