use virt_texture::{
    setup::{VirtualTexturingPipelines, WgpuContext},
    vertex::FOUR_TRIANGLES,
};
use wgpu::util::DeviceExt;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Virtual Texturing Demo")
        .build(&event_loop)
        .unwrap();

    let wgpu_ctx = pollster::block_on(WgpuContext::new(window));
    let mut pipelines = VirtualTexturingPipelines::new(&wgpu_ctx, &[]);

    let vertex_buffer = wgpu_ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex buffer"),
            contents: bytemuck::cast_slice(&FOUR_TRIANGLES),
            usage: wgpu::BufferUsages::VERTEX,
        });

    let lod_bias_buffer = wgpu_ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lod bias buffer"),
            contents: bytemuck::cast_slice(&[0.0f32]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
    let lod_bias_bind_group = wgpu_ctx
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lod bias bind group"),
            layout: &pipelines.lod_bias_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &lod_bias_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

    event_loop.run(move |event, _, control_flow| match event {
        winit::event::Event::WindowEvent { event, .. } => match event {
            winit::event::WindowEvent::CloseRequested => {
                *control_flow = winit::event_loop::ControlFlow::Exit
            }
            _ => (),
        },
        winit::event::Event::RedrawRequested(_) => {
            println!("drawing");
            let mut command_encoder =
                wgpu_ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("prepass encoder"),
                    });
            pipelines.prepass(
                &mut command_encoder,
                &vertex_buffer,
                12,
                &lod_bias_bind_group,
            );

            let output = pipelines.debug_prepass_render(&wgpu_ctx, &mut command_encoder);
            wgpu_ctx
                .queue
                .submit(std::iter::once(command_encoder.finish()));
            output.present();
        }
        _ => (),
    });
}
