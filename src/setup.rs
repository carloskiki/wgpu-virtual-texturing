use std::num::NonZeroU64;

pub struct WgpuContext {
    pub surface: wgpu::Surface,
    pub surface_format: wgpu::TextureFormat,
    pub window: winit::window::Window,
    pub window_size: winit::dpi::PhysicalSize<u32>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl WgpuContext {
    pub async fn new(window: winit::window::Window) -> Self {
        let window_size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });
        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();
        let surface_format = surface
            .get_capabilities(&adapter)
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: window_size.width,
                height: window_size.height,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                view_formats: vec![],
            },
        );

        Self {
            surface,
            surface_format,
            window,
            window_size,
            device,
            queue,
        }
    }
}

pub struct VirtualTexturingPipelines {
    prepass_pipeline: wgpu::RenderPipeline,
    prepass_texture: wgpu::Texture,
    prepass_depth_texture: wgpu::Texture,
    render_pipeline: wgpu::RenderPipeline,
    render_depth_texture: wgpu::Texture,
    #[cfg(debug_assertions)]
    debug_prepass_pipeline: wgpu::RenderPipeline,
    pub lod_bias_bind_group_layout: wgpu::BindGroupLayout,
}

impl VirtualTexturingPipelines {
    /// The bind group layouts for the render pipeline.
    pub fn new(context: &WgpuContext, bind_group_layouts: &[&wgpu::BindGroupLayout]) -> Self {
        let shader = context
            .device
            .create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let prepass_texture_size = wgpu::Extent3d {
            width: context.window_size.width / 10,
            height: context.window_size.height / 10,
            depth_or_array_layers: 1,
        };

        let prepass_texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("prepass texture"),
            size: prepass_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Uint,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let prepass_depth_texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("prepass depth texture"),
            size: prepass_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let pipeline_primitive_state = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        };

        let lod_bias_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("prepass lod bias bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(std::mem::size_of::<f32>() as u64),
                        },
                        count: None,
                    }],
                });
        let prepass_bind_group_layouts: Vec<&wgpu::BindGroupLayout> =
            [&[&lod_bias_bind_group_layout], &bind_group_layouts[..]].concat();
        let prepass_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("prepass pipeline layout"),
                    push_constant_ranges: &[],
                    bind_group_layouts: &prepass_bind_group_layouts[..],
                });

        let prepass_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("prepass pipeline"),
                    layout: Some(&prepass_pipeline_layout),
                    primitive: pipeline_primitive_state,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_prepass",
                        buffers: &[super::vertex::Vertex::BUFFER_LAYOUT],
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: prepass_depth_texture.format(),
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "fs_prepass",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: prepass_texture.format(),
                            blend: None,
                            write_mask: wgpu::ColorWrites::COLOR,
                        })],
                    }),
                    multiview: None,
                });

        let render_depth_texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render depth texture"),
            size: wgpu::Extent3d {
                width: context.window_size.width,
                height: context.window_size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let render_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("render pipeline layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Render Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_render",
                        buffers: &[super::vertex::Vertex::BUFFER_LAYOUT],
                    },
                    primitive: pipeline_primitive_state,
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: render_depth_texture.format(),
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "fs_render",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: context.surface_format,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview: None,
                });

        #[cfg(debug_assertions)]
        let debug_prepass_pipeline = {
            let bind_group_layout =
                context
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("debug prepass texture bind group layout"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Uint,
                                },
                                count: None,
                            },
                        ],
                    });

            let pipeline_layout =
                context
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("debug prepass pipeline layout"),
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });

            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("debug prepass pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_debug_prepass",
                        buffers: &[],
                    },
                    primitive: pipeline_primitive_state,
                    depth_stencil: None,
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "fs_debug_prepass",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: context.surface_format,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview: None,
                })
        };

        Self {
            prepass_pipeline,
            prepass_texture,
            prepass_depth_texture,
            render_pipeline,
            render_depth_texture,
            lod_bias_bind_group_layout,
            #[cfg(debug_assertions)]
            debug_prepass_pipeline,
        }
    }

    pub fn prepass(
        &mut self,
        command_encoder: &mut wgpu::CommandEncoder,
        vertex_buffer: &wgpu::Buffer,
        vertex_count: usize,
        lod_bias_bind_group: &wgpu::BindGroup,
    ) {
        let prepass_view = self
            .prepass_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let prepass_depth_view = self
            .prepass_depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("prepass render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &prepass_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &prepass_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        render_pass.set_pipeline(&self.prepass_pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_bind_group(0, lod_bias_bind_group, &[]);
        render_pass.draw(0..vertex_count as u32, 0..1);
    }

    pub fn render(
        &self,
        command_encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::SurfaceTexture,
        vertex_buffer: &wgpu::Buffer,
        vertex_count: usize,
    ) {
        let ref view = surface
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let ref depth_view = self.render_depth_texture.create_view(&Default::default());

        let mut render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..vertex_count as u32, 0..1);
    }

    #[cfg(debug_assertions)]
    pub fn debug_prepass_render(
        &self,
        context: &WgpuContext,
        command_encoder: &mut wgpu::CommandEncoder,
    ) -> wgpu::SurfaceTexture {
        let output = context.surface.get_current_texture().unwrap();
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let texture_bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("debug prepass texture bind group"),
                layout: &self.debug_prepass_pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &self.prepass_texture.create_view(&Default::default()),
                    ),
                }],
            });

        let mut render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("debug prepass render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&self.debug_prepass_pipeline);
        render_pass.set_bind_group(0, &texture_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
        drop(render_pass);

        output
    }
}
