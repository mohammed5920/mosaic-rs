use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::{Window, WindowId},
};

use crate::{benchmark, renderer::gpu::core::GpuState};

#[derive(Default)]
struct MosaicWindow {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
}

impl ApplicationHandler for MosaicWindow {
    //NOTE: Initialiser
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = benchmark("creating window", || {
            Arc::new(
                event_loop
                    .create_window(Window::default_attributes())
                    .unwrap(),
            )
        });
        self.window = Some(window.clone());
        self.gpu = Some(benchmark("initialising GPU", || {
            pollster::block_on(GpuState::new(window.clone()))
        }));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            //NOTE: resize handler
            WindowEvent::Resized(size) => {
                benchmark(format!("resizing to {size:?}").as_str(), || {
                    self.gpu
                        .as_mut()
                        .expect("gpu should be initialised")
                        .resize(size)
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
                // benchmark("rendering", || self.gpu.as_mut().expect("gpu should be initialised").render());
                self.gpu
                    .as_mut()
                    .expect("gpu should be initialised")
                    .render();
                self.window
                    .as_ref()
                    .expect("window should be initialised")
                    .request_redraw();
            }

            //NOTE: destructor
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            _ => {}
        }
    }
}

pub fn start_mosaic_loop() -> anyhow::Result<()> {
    let mut app = MosaicWindow::default();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    Ok(event_loop.run_app(&mut app)?)
}
