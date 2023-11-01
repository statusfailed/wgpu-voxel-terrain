use std::fs;
use crate::state::*;

use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    window::Window,
};

pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let shader_source = fs::read_to_string("src/shader.wgsl").unwrap();
    let compute_shader_source = fs::read_to_string("src/compute.wgsl").unwrap();
    let mut state = State::new(&window, &shader_source, &compute_shader_source).await;

    // following wgpu-examples
    // TODO: not wasm32-friendly
    let mut frame_count: u32 = 0;
    let mut accum_time: f32 = 0.0;
    let mut last_frame_inst = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                // count frame times
                accum_time += last_frame_inst.elapsed().as_secs_f32();
                last_frame_inst = std::time::Instant::now();
                frame_count += 1;

                // reset and print every 100 frames
                if frame_count > 100 {
                    println!("{}ms", accum_time * 1000.0 / frame_count as f32);
                    frame_count = 0;
                    accum_time = 0.0;
                }

                state.update();
                match state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    // Exit if out of memory
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            }

            Event::MainEventsCleared => {
                window.request_redraw();
            }

            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => if !state.input(event) {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,

                    WindowEvent::CursorMoved { position, .. } => {
                        state.clear_color = wgpu::Color {
                            r: position.x / state.size.width as f64,
                            g: position.y / state.size.height as f64,
                            b: 0.3,
                            a: 1.0,
                        };
                    }

                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        state.resize(**new_inner_size);
                    }

                    _ => {}
                }
            },
            _ => {}
        }
    });
}
