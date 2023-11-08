use std::{f32, sync::Arc};

use wgpu::util::DeviceExt;

use crate::{pipelines::Pipelines, textures::Textures};

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
            ..Default::default()
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

pub struct VirtualTexturingContext {
    pub wgpu_context: Arc<WgpuContext>,
    pub textures: Arc<Textures>,
    pub pipelines: Pipelines,
}

impl VirtualTexturingContext {
    /// Set the level of detail bias for the following passes.
    ///
    /// The level of detail is used during the prepass to determine which mip level to use for each
    /// texture page.
    pub fn set_lod_bias(&mut self, lod_bias: f32, command_encoder: &mut wgpu::CommandEncoder) {
        let lod_bias = f32::log2(Pipelines::PREPASS_RENDER_RATIO) + lod_bias;
        let lod_bias_stg =
            self.wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("lod bias stg"),
                    contents: bytemuck::cast_slice(&[lod_bias]),
                    usage: wgpu::BufferUsages::COPY_SRC,
                });
        command_encoder.copy_buffer_to_buffer(
            &lod_bias_stg,
            0,
            &self.pipelines.lod_bias_buffer,
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
            .textures
            .prepass_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let prepass_depth_view = self
            .textures
            .prepass_depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let vertex_buffer =
            self.wgpu_context
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
                     store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &prepass_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                     store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(&self.pipelines.prepass_pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_bind_group(0, &self.pipelines.lod_bias_bind_group, &[]);
        render_pass.draw(0..vertices.len() as u32, 0..1);
        drop(render_pass);

        self.pipelines.vertices = Some((vertex_buffer, vertices.len() as u32));
    }

    pub fn render(&self, command_encoder: &mut wgpu::CommandEncoder) -> wgpu::SurfaceTexture {
        let output = self.wgpu_context.surface.get_current_texture().unwrap();
        let view = &output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = &self
            .pipelines
            .render_depth_texture
            .create_view(&Default::default());

        let (vertices, vertex_len) = self.pipelines.vertices.as_ref().unwrap();

        let mut render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                     store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipelines.render_pipeline);
        render_pass.set_vertex_buffer(0, vertices.slice(..));
        render_pass.draw(0..*vertex_len, 0..1);

        output
    }

    #[cfg(debug_assertions)]
    pub fn debug_prepass_render(
        &self,
        command_encoder: &mut wgpu::CommandEncoder,
    ) -> wgpu::SurfaceTexture {
        let output = self.wgpu_context.surface.get_current_texture().unwrap();
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let texture_bind_group =
            self.wgpu_context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("debug prepass texture bind group"),
                    layout: &self
                        .pipelines
                        .debug_prepass_pipeline
                        .get_bind_group_layout(0),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &self
                                .textures
                                .prepass_texture
                                .create_view(&Default::default()),
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
                     store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipelines.debug_prepass_pipeline);
        render_pass.set_bind_group(0, &texture_bind_group, &[]);
        render_pass.draw(0..3, 0..1);

        output
    }
}
