use std::sync::Arc;

use virt_texture::{
    pipelines::Pipelines,
    setup::{VirtualTexturingContext, WgpuContext},
    textures::Textures,
    vertex::FOUR_TRIANGLES,
};
use winit::event::{Event, WindowEvent};

fn main() {
    let event_loop = winit::event_loop::EventLoop::new()
        .expect("the event loop creation to succeed since we are on the main thread");
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

    event_loop
        .run(|event, target| match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => target.exit(),
                WindowEvent::RedrawRequested => {
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
            },
            _ => (),
        })
        .unwrap();
}
