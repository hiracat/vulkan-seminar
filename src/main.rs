use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalSize, Size},
    event_loop::EventLoop,
    window::{Window, WindowAttributes},
};
static WINDOW_WIDTH: u32 = 800;
static WINDOW_HEIGHT: u32 = 600;

fn main() {
    let mut app = App::new();
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut app).unwrap();
}

struct App {
    window: Option<Window>,
}
impl App {
    fn new() -> App {
        App { window: None }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_title("Thanks for Coming to my Seminar")
                    .with_resizable(false)
                    .with_inner_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT)),
            )
            .ok()
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::RedrawRequested => {}
            winit::event::WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => (),
        }
    }
}
