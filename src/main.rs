use std::sync::Arc;

use virt_texture::{
    pipelines::Pipelines,
    setup::{VirtualTexturingContext, WgpuContext},
    textures::Textures,
    vertex::FOUR_TRIANGLES,
};
use winit::platform::run_return::EventLoopExtRunReturn;

fn main() {
    let mut event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Virtual Texturing Demo")
        .build(&event_loop)
        .unwrap();

    let wgpu_context = Arc::new(pollster::block_on(WgpuContext::new(window)));
    let textures = Arc::new(Textures::new(&wgpu_context, 2048));
    let pipelines = Pipelines::new(&wgpu_context, &textures, &[]);
    let mut context = VirtualTexturingContext {
        wgpu_context,
        textures,
        pipelines,
    };

    let mut command_encoder =
        context
            .wgpu_context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lod bias"),
            });
    context.set_lod_bias(0., &mut command_encoder);
    context
        .wgpu_context
        .queue
        .submit(Some(command_encoder.finish()));

    event_loop.run_return(|event, _, control_flow| match event {
        winit::event::Event::WindowEvent { event, .. } => match event {
            winit::event::WindowEvent::CloseRequested => {
                *control_flow = winit::event_loop::ControlFlow::Exit
            }
            _ => (),
        },
        winit::event::Event::RedrawRequested(_) => {
            println!("drawing");
            let mut command_encoder = context
                .wgpu_context
                .device
                .create_command_encoder(&Default::default());
            context.prepass(&mut command_encoder, &FOUR_TRIANGLES);
            // let output = context.debug_prepass_render(&mut command_encoder);

            // context
            //     .wgpu_context
            //     .queue
            //     .submit(Some(command_encoder.finish()));
            // output.present();
        }
        _ => (),
    });
}
