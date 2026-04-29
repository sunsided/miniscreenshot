use miniscreenshot_wgpu::capture_texture;
use miniscreenshot_wgpu::wgpu;

const SHADER: &str = r#"
@vertex
fn vs(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.5),
        vec2<f32>(-0.5, -0.5),
        vec2<f32>(0.5, -0.5),
    );
    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}

@fragment
fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.5, 0.2, 1.0);
}
"#;

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
        label: Some("render_target"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("triangle_shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("triangle_pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: "vs",
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader_module,
            entry_point: "fs",
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(&pipeline);
        rpass.draw(0..3, 0..1);
    }

    queue.submit(std::iter::once(encoder.finish()));

    let shot = capture_texture(&device, &queue, &texture).expect("failed to capture texture");
    let path = "wgpu_screenshot.png";
    shot.save(path).expect("failed to save screenshot");
    println!("saved {path}");
}
