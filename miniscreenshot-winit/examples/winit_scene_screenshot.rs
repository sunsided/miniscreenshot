use miniscreenshot_softbuffer::screenshot_from_xrgb;
use miniscreenshot_softbuffer::softbuffer;
use miniscreenshot_winit::winit::application::ApplicationHandler;
use miniscreenshot_winit::winit::event::WindowEvent;
use miniscreenshot_winit::winit::event_loop::{ActiveEventLoop, EventLoop};
use miniscreenshot_winit::winit::window::Window;
use std::rc::Rc;

struct App {
    window: Option<Rc<Window>>,
    softbuffer_ctx: Option<softbuffer::Context<Rc<Window>>>,
    softbuffer_surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            softbuffer_ctx: None,
            softbuffer_surface: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Rc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_inner_size(winit::dpi::LogicalSize::new(512, 384))
                        .with_title("miniscreenshot-winit example"),
                )
                .expect("failed to create window"),
        );
        self.window = Some(window.clone());

        let ctx =
            softbuffer::Context::new(window.clone()).expect("failed to create softbuffer context");
        let surface =
            softbuffer::Surface::new(&ctx, window.clone()).expect("failed to create surface");
        self.softbuffer_ctx = Some(ctx);
        self.softbuffer_surface = Some(surface);

        window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let surface = self.softbuffer_surface.as_mut().unwrap();
                let window = self.window.as_ref().unwrap();
                let size = window.inner_size();
                let w = size.width;
                let h = size.height;

                if w == 0 || h == 0 {
                    return;
                }

                surface
                    .resize(w.try_into().unwrap(), h.try_into().unwrap())
                    .expect("failed to resize surface");

                let mut buffer = surface.buffer_mut().expect("failed to get buffer");
                let mut pixels = Vec::with_capacity((w * h) as usize);

                for y in 0..h {
                    for x in 0..w {
                        let r = x & 0xFF;
                        let g = y & 0xFF;
                        let b = (x ^ y) & 0xFF;
                        pixels.push((r << 16) | (g << 8) | b);
                    }
                }

                for (dest, &src) in buffer.iter_mut().zip(pixels.iter()) {
                    *dest = src;
                }
                let _ = buffer.present();

                let shot = screenshot_from_xrgb(&pixels, w, h);
                let path = "winit_screenshot.png";
                shot.save(path).expect("failed to save screenshot");
                println!("saved {path}");

                event_loop.exit();
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("event loop failed");
}
