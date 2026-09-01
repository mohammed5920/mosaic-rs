use std::{fs, sync::Arc};

use winit::window::Window;

pub struct GpuState {
    ///the thing that winit presents, config lives in surface_config
    surface: wgpu::Surface<'static>,
    ///the gpu
    device: wgpu::Device,
    ///the command queue the gpu chews through
    queue: wgpu::Queue,
    ///stored config for the surface, since you cannot read it from the surface directly (?)
    surface_config: wgpu::SurfaceConfiguration,
    ///pipeline for the main mosaic shaders
    pipeline: wgpu::RenderPipeline,
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

        let surface = instance
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

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let vertex_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("saic_Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(
                fs::read_to_string("src/renderer/gpu/_vertex.wgsl")
                    .expect("Cannot read vertex shader")
                    .into(),
            ),
        });

        let fragment_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("saic_Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(
                fs::read_to_string("src/renderer/gpu/_fragment.wgsl")
                    .expect("Cannot read vertex shader")
                    .into(),
            ),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("saic_Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &vertex_shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        alpha: wgpu::BlendComponent::REPLACE,
                        color: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Self {
            surface,
            device,
            queue,
            surface_config: config,
            pipeline,
        }
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
    }

    //NOTE: wgpu renderer
    pub fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) => todo!(),
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
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
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
            pass.set_pipeline(&self.pipeline);
            // pass.set_bind_group(0, &render_config.mosaic_bind_group, &[]);
            pass.draw(0..4, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
    }
}
