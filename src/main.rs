#[allow(clippy::all)]
#[allow(clippy::pedantic)]
use ffmpeg_next as ffmpeg;

use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::{Window, WindowId},
};

use crate::{
    mosaic::Mosaic,
    renderer::GpuState,
    streamer::Streamer,
    util::{benchmark, set_panic_hook},
};

mod mosaic;
mod renderer;
mod streamer;
mod util;
mod vidcap;

const SOURCE: &str = "test/source.jpg";
const TILES: &str = "test/vid_tiles/S2";

struct AppState {
    mosaic: Mosaic,
    streamer: Streamer,
    window: Arc<Window>,
    gpu: GpuState,
}

struct App(Option<AppState>);

impl App {
    fn state(&mut self) -> &mut AppState {
        self.0.as_mut().expect("app should be initialised")
    }
}

impl ApplicationHandler for App {
    //NOTE: Initialiser
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = benchmark("creating window", || {
            Arc::new(
                event_loop
                    .create_window(Window::default_attributes())
                    .unwrap(),
            )
        });
        let mut gpu = benchmark("initialising GPU", || {
            pollster::block_on(GpuState::initialise(window.clone(), false))
        });
        let mosaic = Mosaic::create(SOURCE, TILES).expect("Could not create mosaic");
        let streamer = benchmark("initialising streamer", || {
            Streamer::initialise(&gpu.device, &mut gpu.queue, &mosaic)
        });
        gpu.bind_streaming_resources(streamer.mosaic_view.clone(), streamer.palette_view.clone());

        self.0 = Some(AppState {
            mosaic,
            streamer,
            window,
            gpu,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            //NOTE: resize handler
            WindowEvent::Resized(size) => {
                benchmark(format!("resizing to {size:?}").as_str(), || {
                    self.state().gpu.resize(size)
                });
            }

            //NOTE: keyboard handler
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(_),
                        ..
                    },
                ..
            } => {}

            //NOTE: Renderer
            WindowEvent::RedrawRequested => {
                let frame_matches = self.state().mosaic.read_frame();
                let s = self.state();
                s.streamer.update(&mut s.gpu.queue, &frame_matches);
                self.state().gpu.render();
                self.state().window.request_redraw();
            }

            //NOTE: destructor
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            _ => {}
        }
    }
}

pub(crate) fn main() {
    set_panic_hook();
    ffmpeg::init().expect("Could not initialise FFMPEG");
    let event_loop: EventLoop<()> = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App(None);
    event_loop
        .run_app(&mut app)
        .expect("Error running main loop")
}
