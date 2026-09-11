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
    config::CONFIG,
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
                    .expect("could not create window"),
            )
        });
        let (mosaic, mut renderer) = thread::scope(|s| {
            let mosaic_fut = s.spawn(|| Mosaic::create(&CONFIG.source_path, &CONFIG.tile_path));
            let renderer = benchmark("initialising GPU", || {
                pollster::block_on(Renderer::new(window.clone()))
            });
            let mosaic = mosaic_fut
                .join()
                .expect("could not create mosaic")
                .expect("could not create mosaic");
            (mosaic, renderer)
        });

        let display_res = window
            .current_monitor()
            .expect("can't detect current screen size")
            .size();
        let res_limit = 2u64.pow(
            (display_res.width.min(display_res.height) as f64)
                .log2()
                .floor()
                .min(2048.0) as u32,
        );

        let streamer = benchmark("initialising streamer", || {
            Streamer::new(
                &renderer.device,
                &mut renderer.queue,
                &mosaic,
                display_res,
                mosaic.total_tile_frames(),
                (mosaic.total_tile_frames() as f64 / mosaic.tiles().len() as f64).ceil() as u64,
                res_limit,
            )
        });
        let camera = AppCameraWrapper::new(
            &renderer.device,
            renderer.queue.clone(),
            (mosaic.width() as f32, mosaic.height() as f32),
            (
                window.inner_size().width as f32,
                window.inner_size().height as f32,
            ),
            res_limit,
        );

        renderer.bind_resources(&camera.buffer, streamer.palette_view.clone(), &mosaic);
        renderer.update_mosaic_texture(&mosaic.dense_matches());

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
            WindowEvent::Resized(size) => {
                let s = self.state();
                s.camera.resize(size);
                s.streamer.update_visibility(&s.camera, &s.mosaic);
                s.renderer.resize(size)
            }

            WindowEvent::CursorMoved { position, .. } => {
                let s = self.state();
                s.input.cursor_pos = (position.x, position.y);
                if s.input.is_clicked {
                    if let Some((ox, oy)) = s.input.clicked_cursor_pos {
                        let (nx, ny) = s.input.cursor_pos;
                        let (dx, dy) = (nx - ox, ny - oy);

                        s.camera.pan((-dx as f32, -dy as f32));
                        s.streamer.update_visibility(&s.camera, &s.mosaic);
                    }
                    s.input.clicked_cursor_pos = Some(s.input.cursor_pos);
                } else {
                    s.input.clicked_cursor_pos = None;
                }
            }

            WindowEvent::MouseWheel {
                delta: LineDelta(_, y),
                ..
            } => {
                let s = self.state();
                let before_ts = s.camera.get_onscreen_tile_size();
                if y > 0.0 {
                    //NOTE: mouse wheel delta (4 matching python)
                    s.camera.set_zoom(4);
                } else if y < 0.0 {
                    s.camera.set_zoom(-4);
                }
                if s.camera.get_onscreen_tile_size() > before_ts {
                    s.streamer.on_lod_change();
                }
                s.streamer.update_visibility(&s.camera, &s.mosaic);
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        ..
                    },
                ..
            } => {
                let mut is_dirty = true;
                let s = self.state();
                let before_ts = s.camera.get_onscreen_tile_size();
                match key_code {
                    KeyCode::KeyD => s.camera.pan((-10.0, 0.0)),
                    KeyCode::KeyS => s.camera.pan((0.0, 10.0)),
                    KeyCode::KeyA => s.camera.pan((10.0, 0.0)),
                    KeyCode::KeyW => s.camera.pan((0.0, -10.0)),
                    KeyCode::ArrowUp => s.camera.set_zoom(1),
                    KeyCode::ArrowDown => s.camera.set_zoom(-1),
                    _ => is_dirty = false,
                }
                if is_dirty {
                    if s.camera.get_onscreen_tile_size() > before_ts {
                        s.streamer.on_lod_change();
                    }
                    s.streamer.update_visibility(&s.camera, &s.mosaic);
                }
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let s = self.state();
                s.input.is_clicked = state.is_pressed();
                if !state.is_pressed() {
                    s.input.clicked_cursor_pos = None;
                }
            }

            WindowEvent::RedrawRequested => {
                let s = self.state();
                s.streamer.check_refresh(s.camera.get_onscreen_tile_size());
                s.renderer.render();
                s.window.request_redraw();
            }

            WindowEvent::CloseRequested => {
                self.state().streamer.shutdown();
                event_loop.exit();
            }

            _ => {}
        }
    }
}

pub(crate) fn main() {
    set_panic_hook();
    ffmpeg::init().expect("could not initialise FFMPEG");
    let event_loop: EventLoop<()> = EventLoop::new().expect("could not initialise winit");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App(None);
    event_loop
        .run_app(&mut app)
        .expect("error running main loop")
}
