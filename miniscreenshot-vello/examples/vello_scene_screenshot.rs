use miniscreenshot_vello::vello;
use miniscreenshot_vello::vello::kurbo::{Affine, Rect};
use miniscreenshot_vello::vello::peniko::Color;
use miniscreenshot_vello::vello::peniko::Fill;
use miniscreenshot_vello::vello::{AaConfig, AaSupport, RenderParams};
use miniscreenshot_wgpu::capture_texture;
use miniscreenshot_wgpu::wgpu;

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .expect("failed to request adapter");

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        },
        None,
    ))
    .expect("failed to request device");

    let size = 512u32;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vello_render_target"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let mut renderer = vello::Renderer::new(
        &device,
        vello::RendererOptions {
            surface_format: Some(wgpu::TextureFormat::Rgba8Unorm),
            use_cpu: false,
            antialiasing_support: AaSupport::all(),
            num_init_threads: None,
        },
    )
    .expect("failed to create Vello renderer");

    let mut scene = vello::Scene::new();
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::rgb8(51, 77, 153),
        None,
        &Rect::new(0.0, 0.0, 512.0, 512.0),
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::rgb8(255, 128, 51),
        None,
        &Rect::new(100.0, 100.0, 300.0, 300.0),
    );
    scene.fill(
        Fill::NonZero,
        Affine::translate((50.0, 50.0)),
        Color::rgb8(51, 204, 128),
        None,
        &Rect::new(200.0, 200.0, 400.0, 400.0),
    );

    let render_params = RenderParams {
        base_color: Color::BLACK,
        width: size,
        height: size,
        antialiasing_method: AaConfig::Msaa16,
    };

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    renderer
        .render_to_texture(&device, &queue, &scene, &texture_view, &render_params)
        .expect("Vello render failed");

    let shot = capture_texture(&device, &queue, &texture).expect("failed to capture texture");
    let path = "vello_screenshot.png";
    shot.save(path).expect("failed to save screenshot");
    println!("saved {path}");
}
