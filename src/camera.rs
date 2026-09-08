use wgpu::util::DeviceExt as _;
use winit::dpi::PhysicalSize;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct AppCamera {
    ///mosaic space coordinate the camera looks at
    center: [f32; 2],
    zoom_steps: i32,
    steps_per_octave: u32,
    ///screen space dimensions
    viewport: [f32; 2],
}

pub(crate) struct AppCameraWrapper {
    pub(crate) buffer: wgpu::Buffer,
    inner: AppCamera,
    queue: wgpu::Queue,
}

impl AppCameraWrapper {
    pub(crate) fn initialise(
        device: &wgpu::Device,
        queue: wgpu::Queue,
        mosaic_dims: (f32, f32),
        window_dims: (f32, f32),
    ) -> Self {
        let inner_camera = AppCamera {
            center: [mosaic_dims.0 / 2.0, mosaic_dims.1 / 2.0],
            viewport: [window_dims.0, window_dims.1],
            zoom_steps: 0,
            steps_per_octave: 30,
        };
        AppCameraWrapper {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("saic_Camera Buffer"),
                contents: bytemuck::cast_slice(&[inner_camera]),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            }),
            inner: inner_camera,
            queue,
        }
    }

    fn zoom_factor(&self) -> f32 {
        2f32.powf(self.inner.zoom_steps as f32 / self.inner.steps_per_octave as f32)
    }

    fn sync(&self) {
        self.queue
            .write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.inner]));
    }

    pub(crate) fn resize(&mut self, new_dims: PhysicalSize<u32>) {
        self.inner.viewport = [new_dims.width as f32, new_dims.height as f32];
        self.sync();
    }

    pub(crate) fn pan(&mut self, deltas: (f32, f32)) {
        let (x, y) = deltas;
        self.inner.center[0] += x / self.zoom_factor();
        self.inner.center[1] += y / self.zoom_factor();
        self.sync();
    }

    pub(crate) fn zoom(&mut self, delta: i32) {
        self.inner.zoom_steps += delta;
        self.sync();
    }
}
