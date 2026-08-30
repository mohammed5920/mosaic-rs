use std::sync::Arc;

use winit::window::Window;

pub struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    test: u64,
}

impl GpuState {
    //NOTE: wgpu initialiser
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface: wgpu::Surface<'_> = instance
            .create_surface(window.clone())
            .expect("Cannot create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: Default::default(),
            })
            .await
            .expect("Cannot request adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("Cannot request device & queue");

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("Surface not supported by adapter");
        config.present_mode = wgpu::PresentMode::FifoRelaxed;
        surface.configure(&device, &config);

        Self {
            surface,
            device,
            queue,
            config,
            test: 0,
        }
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.config.width = size.width.max(1);
        self.config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    //NOTE: wgpu renderer
    pub fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Timeout => todo!(),
            wgpu::CurrentSurfaceTexture::Occluded => todo!(),
            wgpu::CurrentSurfaceTexture::Outdated => todo!(),
            wgpu::CurrentSurfaceTexture::Lost => todo!(),
            wgpu::CurrentSurfaceTexture::Validation => todo!(),
        };
        let view = frame
            .texture
            .create_view(&wgpu::wgt::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            self.test += 1;
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: (self.test % 256) as f64 / 256.0,
                            g: (self.test % 512) as f64 / 256.0 / 2.0,
                            b: (self.test % 1024) as f64 / 256.0 / 4.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    resolve_target: None,
                    depth_slice: None,
                })],
                label: None,
                multiview_mask: None,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
    }
}
