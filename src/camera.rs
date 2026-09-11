use wgpu::util::DeviceExt as _;
use winit::dpi::PhysicalSize;

use crate::{config::CONFIG, mosaic::Mosaic, types::Bb};

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
    res_limit: u64,
}

impl AppCamera {
    ///[xmin, ymin], [xmax, ymax]
    pub(crate) fn visible_world_bounds(&self) -> Bb {
        let zoom = self.zoom_factor();
        let half_viewport_world = (
            (self.viewport[0] * 0.5) / zoom,
            (self.viewport[1] * 0.5) / zoom,
        );
        let min = (
            (self.center[0] - half_viewport_world.0).floor() as i64,
            (self.center[1] - half_viewport_world.1).floor() as i64,
        );
        let max = (
            (self.center[0] + half_viewport_world.0).ceil() as i64,
            (self.center[1] + half_viewport_world.1).ceil() as i64,
        );
        (min, max)
    }

    fn zoom_factor(&self) -> f32 {
        2f32.powf(self.zoom_steps as f32 / self.steps_per_octave as f32)
    }
}

impl AppCameraWrapper {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: wgpu::Queue,
        mosaic_dims: (f32, f32),
        window_dims: (f32, f32),
        res_limit: u64,
    ) -> Self {
        let inner_camera = AppCamera {
            center: [mosaic_dims.0 / 2.0, mosaic_dims.1 / 2.0],
            viewport: [window_dims.0, window_dims.1],
            zoom_steps: 0,
            steps_per_octave: CONFIG.zoom_steps_per_octave.get() as u32,
        };
        AppCameraWrapper {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("saic_Camera Buffer"),
                contents: bytemuck::cast_slice(&[inner_camera]),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            }),
            inner: inner_camera,
            queue,
            res_limit,
        }
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
        self.inner.center[0] += x / self.inner.zoom_factor();
        self.inner.center[1] += y / self.inner.zoom_factor();
        self.sync();
    }

    pub(crate) fn set_zoom(&mut self, delta: i32) {
        self.inner.zoom_steps += delta;
        if self.get_onscreen_tile_size() > self.res_limit {
            self.inner.zoom_steps -= delta
        } else {
            self.sync();
        }
    }

    pub(crate) fn get_onscreen_tile_size(&self) -> u64 {
        let zs = self.inner.zoom_steps;
        let zo = self.inner.steps_per_octave as i32;
        2f64.powi((zs + zo - 1) / zo)
            .max(1.0)
            .min(self.res_limit as f64) as u64
    }

    ///is zoomed higher than 100%?
    pub(crate) fn is_zoomed_in(&self) -> bool {
        self.get_onscreen_tile_size() >= 2
    }

    ///get the bounding box in mosaic coords of what the camera is currently looking at
    ///
    ///None if camera is looking entirely outside the mosaic
    pub(crate) fn calc_visible_mosaic_bounding_box(&self, mosaic: &Mosaic) -> Option<Bb> {
        let (mmin, mmax) = (
            (0i64, 0i64),
            (mosaic.width() as i64, mosaic.height() as i64),
        );
        let (vmin, vmax) = self.inner.visible_world_bounds();
        let min = (vmin.0.max(mmin.0), vmin.1.max(mmin.1));
        let max = (vmax.0.min(mmax.0).max(0), vmax.1.min(mmax.1).max(0));
        if min.0 >= max.0 || min.1 >= max.1 {
            return None;
        };
        Some((min, max))
    }
}
