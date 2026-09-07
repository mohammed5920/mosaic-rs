use std::{fs, sync::Arc};

use winit::window::Window;

pub(crate) struct GpuStreamingState {
    streaming_bind_group: wgpu::BindGroup,
}

pub(crate) struct GpuState {
    ///the gpu
    pub(crate) device: wgpu::Device,
    ///the command queue the gpu chews through
    pub(crate) queue: wgpu::Queue,

    ///the thing that winit presents, config lives in surface_config
    surface: wgpu::Surface<'static>,
    ///stored config for the surface, since you cannot read it from the surface directly (?)
    surface_config: wgpu::SurfaceConfiguration,
    ///pipeline for the main mosaic shaders
    pipeline: wgpu::RenderPipeline,

    streaming_resources: Option<GpuStreamingState>,
}

impl GpuState {
    //NOTE: wgpu initialiser
    pub(crate) async fn initialise(window: Arc<Window>, vsync: bool) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("cannot create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: Default::default(),
            })
            .await
            .expect("cannot request adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("cannot request device & queue");

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface not supported by adapter");
        config.present_mode = if vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        config.format = wgpu::TextureFormat::Rgba8Unorm;
        surface.configure(&device, &config);
        let surface_format = config.format;

        let vertex_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("saic_Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(
                fs::read_to_string("src/renderer/_vertex.wgsl")
                    .expect("cannot read vertex shader")
                    .into(),
            ),
        });

        let fragment_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("saic_Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(
                fs::read_to_string("src/renderer/_fragment.wgsl")
                    .expect("cannot read fragment shader")
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
                    blend: None,
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
            streaming_resources: None,
        }
    }

    pub(crate) fn bind_streaming_resources(
        &mut self,
        mosaic_view: wgpu::TextureView,
        palette_view: wgpu::TextureView,
    ) {
        self.streaming_resources = Some(GpuStreamingState {
            streaming_bind_group: self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("saic_Streaming Bind Group"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&mosaic_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&palette_view),
                    },
                ],
            }),
        })
    }

    pub(crate) fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
    }

    //NOTE: wgpu renderer
    pub(crate) fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) => return,
            something_else => {
                eprintln!("Recieved {something_else:?} instead of surface texture");
                return;
            }
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
                            a: 0.0,
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
            pass.set_bind_group(
                0,
                &self
                    .streaming_resources
                    .as_ref()
                    .expect("streaming should be initialised")
                    .streaming_bind_group,
                &[],
            );
            pass.draw(0..4, 0..1);
        }
        
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
    }
}
