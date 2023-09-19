use std::num::NonZeroU64;

use crate::{setup::WgpuContext, textures::Textures};

pub struct Pipelines {
    pub prepass_pipeline: wgpu::RenderPipeline,
    pub render_pipeline: wgpu::RenderPipeline,
    pub render_depth_texture: wgpu::Texture,
    pub vertices: Option<(wgpu::Buffer, u32)>,
    pub lod_bias_buffer: wgpu::Buffer,
    pub lod_bias_bind_group: wgpu::BindGroup,
    #[cfg(debug_assertions)]
    pub debug_prepass_pipeline: wgpu::RenderPipeline,
}

impl Pipelines {
    pub const PREPASS_RENDER_RATIO: f32 = 0.1;

    pub fn new(
        context: &WgpuContext,
        textures: &Textures,
        bind_group_layouts: &[&wgpu::BindGroupLayout],
    ) -> Self {
        let prepass_shader = context
            .device
            .create_shader_module(wgpu::include_wgsl!("prepass.wgsl"));
        let shader = context
            .device
            .create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

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
                        format: textures.prepass_depth_texture.format(),
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
                            format: textures.prepass_texture.format(),
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

        Self {
            vertices: None,
            prepass_pipeline,
            render_pipeline,
            render_depth_texture,
            lod_bias_bind_group,
            lod_bias_buffer,
            #[cfg(debug_assertions)]
            debug_prepass_pipeline,
        }
    }
}
