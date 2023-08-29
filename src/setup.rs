use std::{f32, num::NonZeroU64};

use wgpu::util::DeviceExt;

pub struct WgpuContext {
    pub surface: wgpu::Surface,
    pub surface_format: wgpu::TextureFormat,
    window: winit::window::Window,
    window_size: winit::dpi::PhysicalSize<u32>,
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
        println!("Adapter features: {:?}", adapter.features());

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

pub struct VirtualTexturingPipelines<'a> {
    device: &'a wgpu::Device,
    surface: &'a wgpu::Surface,
    prepass_pipeline: wgpu::RenderPipeline,
    prepass_texture: wgpu::Texture,
    prepass_depth_texture: wgpu::Texture,
    render_pipeline: wgpu::RenderPipeline,
    render_depth_texture: wgpu::Texture,
    vertices: Option<(wgpu::Buffer, u32)>,
    lod_bias_buffer: wgpu::Buffer,
    lod_bias_bind_group: wgpu::BindGroup,
    page_table_texture: wgpu::Texture,
    #[cfg(debug_assertions)]
    debug_prepass_pipeline: wgpu::RenderPipeline,
}

impl<'a> VirtualTexturingPipelines<'a> {
    const PREPASS_RENDER_RATIO: f32 = 0.1;

    /// The bind group layouts for the render pipeline.
    pub fn new(
        context: &'a WgpuContext,
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        virtual_texture_page_side: u32,
    ) -> Self {
        let prepass_shader = context
            .device
            .create_shader_module(wgpu::include_wgsl!("prepass.wgsl"));
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
        let lod_bias_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("prepass lod bias buffer"),
            size: std::mem::size_of::<f32>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lod_bias_bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("prepass lod bias bind group"),
                layout: &lod_bias_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: lod_bias_buffer.as_entire_binding(),
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
                        module: &prepass_shader,
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
                        module: &prepass_shader,
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
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Uint,
                            },
                            count: None,
                        }],
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
                        module: &prepass_shader,
                        entry_point: "vs_debug_prepass",
                        buffers: &[],
                    },
                    primitive: pipeline_primitive_state,
                    depth_stencil: None,
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &prepass_shader,
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

        assert!(virtual_texture_page_side.is_power_of_two());
        let page_table_texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Page table texture"),
            size: wgpu::Extent3d {
                width: virtual_texture_page_side,
                height: virtual_texture_page_side,
                depth_or_array_layers: 1,
            },
            mip_level_count: f32::log2(virtual_texture_page_side as f32) as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        Self {
            device: &context.device,
            surface: &context.surface,
            vertices: None,
            prepass_pipeline,
            prepass_texture,
            prepass_depth_texture,
            render_pipeline,
            render_depth_texture,
            lod_bias_bind_group,
            lod_bias_buffer,
            page_table_texture,
            #[cfg(debug_assertions)]
            debug_prepass_pipeline,
        }
    }

    /// Set the level of detail bias for the following passes.
    ///
    /// The level of detail is used during the prepass to determine which mip level to use for each
    /// texture page.
    pub fn set_lod_bias(&mut self, lod_bias: f32, command_encoder: &mut wgpu::CommandEncoder) {
        let lod_bias = f32::log2(Self::PREPASS_RENDER_RATIO) + lod_bias;
        let lod_bias_stg = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("lod bias stg"),
                contents: bytemuck::cast_slice(&[lod_bias]),
                usage: wgpu::BufferUsages::COPY_SRC,
            });
        command_encoder.copy_buffer_to_buffer(
            &lod_bias_stg,
            0,
            &self.lod_bias_buffer,
            0,
            std::mem::size_of::<f32>() as wgpu::BufferAddress,
        );
    }

    pub fn prepass(
        &mut self,
        command_encoder: &mut wgpu::CommandEncoder,
        vertices: &[super::vertex::Vertex],
    ) {
        let prepass_view = self
            .prepass_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let prepass_depth_view = self
            .prepass_depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertex buffer"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

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
        render_pass.set_bind_group(0, &self.lod_bias_bind_group, &[]);
        render_pass.draw(0..vertices.len() as u32, 0..1);
        drop(render_pass);

        self.vertices = Some((vertex_buffer, vertices.len() as u32));
    }

    pub fn render(&self, command_encoder: &mut wgpu::CommandEncoder) -> wgpu::SurfaceTexture {
        let output = self.surface.get_current_texture().unwrap();
        let ref view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let ref depth_view = self.render_depth_texture.create_view(&Default::default());

        let (vertices, vertex_len) = self.vertices.as_ref().unwrap();

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
        render_pass.set_vertex_buffer(0, vertices.slice(..));
        render_pass.draw(0..*vertex_len, 0..1);

        output
    }

    #[cfg(debug_assertions)]
    pub fn debug_prepass_render(
        &self,
        command_encoder: &mut wgpu::CommandEncoder,
    ) -> wgpu::SurfaceTexture {
        let output = self.surface.get_current_texture().unwrap();
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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

        output
    }
}
