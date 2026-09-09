#[allow(clippy::all)]
#[allow(clippy::pedantic)]
use ffmpeg_next as ffmpeg;

use std::{process::exit, sync::Arc, thread};

use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, MouseButton, MouseScrollDelta::LineDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::{
    camera::AppCameraWrapper,
    config::{CONFIG, load_config},
    mosaic::Mosaic,
    renderer::Renderer,
    streamer::Streamer,
    util::{benchmark, set_panic_hook},
};

mod camera;
mod config;
mod mosaic;
mod renderer;
mod streamer;
mod types;
mod util;
mod vidcap;

struct InputState {
    cursor_pos: (f64, f64),
    is_clicked: bool,
    clicked_cursor_pos: Option<(f64, f64)>,
    is_playing: bool,
}

struct AppState {
    mosaic: Mosaic,
    streamer: Streamer,
    window: Arc<Window>,
    renderer: Renderer,
    camera: AppCameraWrapper,
    input: InputState,
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
        let (mut mosaic, mut renderer) = thread::scope(|s| {
            let mosaic_fut = s.spawn(|| Mosaic::create(&CONFIG.source_path, &CONFIG.tile_path));
            let renderer = benchmark("initialising GPU", || {
                pollster::block_on(Renderer::initialise(window.clone()))
            });
            let mosaic = mosaic_fut
                .join()
                .expect("Could not create mosaic")
                .expect("Could not create mosaic");
            (mosaic, renderer)
        });
        let mut streamer = benchmark("initialising streamer", || {
            Streamer::initialise(&renderer.device, &mut renderer.queue, &mosaic)
        });
        let camera = AppCameraWrapper::initialise(
            &renderer.device,
            renderer.queue.clone(),
            (mosaic.width() as f32, mosaic.height() as f32),
            (
                window.inner_size().width as f32,
                window.inner_size().height as f32,
            ),
        );
        renderer.bind_resources(
            &camera.buffer,
            streamer.mosaic_view.clone(),
            streamer.palette_view.clone(),
        );
        streamer.update(&mut renderer.queue, &mosaic.read_frame());

        if CONFIG.end_after_init {
            exit(0);
        }

        self.0 = Some(AppState {
            streamer,
            mosaic,
            window,
            renderer,
            camera,
            input: InputState {
                cursor_pos: (0., 0.),
                clicked_cursor_pos: None,
                is_playing: true,
                is_clicked: false,
            },
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            //NOTE: resize handler
            WindowEvent::Resized(size) => {
                self.state().camera.resize(size);
                self.state().renderer.resize(size)
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.state().input.cursor_pos = (position.x, position.y);
                if self.state().input.is_clicked {
                    if let Some((ox, oy)) = self.state().input.clicked_cursor_pos {
                        let (nx, ny) = self.state().input.cursor_pos;
                        let (dx, dy) = (nx - ox, ny - oy);
                        self.state().camera.pan((-dx as f32, -dy as f32));
                    }
                    self.state().input.clicked_cursor_pos = Some(self.state().input.cursor_pos);
                } else {
                    self.state().input.clicked_cursor_pos = None;
                }
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.state().input.is_clicked = state.is_pressed();
                if !state.is_pressed() {
                    self.state().input.clicked_cursor_pos = None;
                }
            }

            WindowEvent::MouseWheel {
                delta: LineDelta(_, y),
                ..
            } => {
                if y > 0.0 {
                    //NOTE: mouse wheel delta (4 matching python)
                    self.state().camera.zoom(4);
                } else if y < 0.0 {
                    self.state().camera.zoom(-4);
                }
            }

            //NOTE: keyboard handler
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        ..
                    },
                ..
            } => match key_code {
                KeyCode::KeyD => self.state().camera.pan((-10.0, 0.0)),
                KeyCode::KeyS => self.state().camera.pan((0.0, 10.0)),
                KeyCode::KeyA => self.state().camera.pan((10.0, 0.0)),
                KeyCode::KeyW => self.state().camera.pan((0.0, -10.0)),
                KeyCode::ArrowUp => self.state().camera.zoom(1),
                KeyCode::ArrowDown => self.state().camera.zoom(-1),
                _ => {}
            },

            //NOTE: Renderer
            WindowEvent::RedrawRequested => {
                // let frame_matches = self.state().mosaic.read_frame();
                let s = self.state();
                // s.streamer.update(&mut s.renderer.queue, &frame_matches);
                s.renderer.render();
                s.window.request_redraw();
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
    load_config();
    ffmpeg::init().expect("Could not initialise FFMPEG");
    let event_loop: EventLoop<()> = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App(None);
    event_loop
        .run_app(&mut app)
        .expect("Error running main loop")
}
