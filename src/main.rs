use virt_texture::{
    setup::{VirtualTexturingPipelines, WgpuContext},
    vertex::FOUR_TRIANGLES,
};
use winit::platform::run_return::EventLoopExtRunReturn;

fn main() {
    let mut event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Virtual Texturing Demo")
        .build(&event_loop)
        .unwrap();

    let wgpu_ctx = pollster::block_on(WgpuContext::new(window));
    let mut pipelines = VirtualTexturingPipelines::new(&wgpu_ctx, &[]);

    event_loop.run_return(|event, _, control_flow| match event {
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
            pipelines.set_lod_bias(0.0, &mut command_encoder);
            pipelines.prepass(&mut command_encoder, &FOUR_TRIANGLES);
            let output = pipelines.debug_prepass_render(&mut command_encoder);

            wgpu_ctx.queue.submit(Some(command_encoder.finish()));
            output.present();
        }
        _ => (),
    });
}
