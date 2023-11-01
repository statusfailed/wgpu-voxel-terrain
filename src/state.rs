use std::iter;
use crate::camera::*;
use crate::texture;
use crate::voxel;
use crate::compute;

use crate::constants::*;

// needed for create_buffer_init
use wgpu::util::DeviceExt;

use winit::{
    window::Window,
    event::WindowEvent,
};

pub struct State {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub clear_color: wgpu::Color,
    pub render_pipeline: wgpu::RenderPipeline,
    // camera
    pub camera: Camera,
    pub camera_controller: CameraController,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    // Depth buffering
    pub depth_texture: texture::Texture,
    pub compute_resources: compute::ComputeResources,
}

impl State {
    pub async fn new(window: &Window, shader_source: &str, compute_shader_source: &str) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };

        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }
        ).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                // We write the vertex buffer from the compute shader, so we need this feature
                features: wgpu::Features::VERTEX_WRITABLE_STORAGE,
                limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ).await.unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_supported_formats(&adapter)[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let modes = surface.get_supported_modes(&adapter);
        let clear_color = DEFAULT_CLEAR_COLOR;

        let camera = Camera::from_config(&config);
        let camera_controller = CameraController::new(0.1);
        let (camera_buffer, camera_bind_group_layout, camera_bind_group) =
            make_camera_bind_group(&device, &camera);

        let compute_resources = compute::ComputeResources::new(&device, &config, &compute_shader_source);

        let depth_texture = texture::Texture::depth(&device, &config, "depth_texture");
        let render_pipeline = make_render_pipeline(&device, &config, &shader_source, &camera_bind_group_layout);

        return Self {
            surface,
            device,
            queue,
            config,
            size,
            clear_color,
            render_pipeline,
            // Camera stuff
            camera,
            camera_controller,
            camera_buffer,
            camera_bind_group,
            // Depth buffering
            depth_texture,
            // Compute
            compute_resources,
        }
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        self.camera_controller.process_events(event)
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;

            // TODO: move this elsewhere?
            self.depth_texture = texture::Texture::depth(&self.device, &self.config, "depth_texture");

            // like recreating surface in Vulkan?
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn update(&mut self) {
        self.camera_controller.update_camera(&mut self.camera);
        let mat: [[f32; 4]; 4] = self.camera.build_view_projection_matrix().into();

        // write camera transformation matrix to uniform
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[mat]));
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // get somewhere to write
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        self.compute_resources.add_compute_pass(&mut encoder);

        add_render_pass(
            &mut encoder,
            &self.render_pipeline,
            &view,
            &self.camera_bind_group,
            &self.compute_resources.visible_buffer,
            &self.compute_resources.draw_indirect_buffer,
            &self.depth_texture,
            self.clear_color,
        );

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

// TODO: abstract this to create_buffer_and_layout or something?
fn make_camera_bind_group(device: &wgpu::Device, camera: &Camera) -> (wgpu::Buffer, wgpu::BindGroupLayout, wgpu::BindGroup) {
    let mat: [[f32; 4]; 4] = camera.build_view_projection_matrix().into();

    let buffer: wgpu::Buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Camera buffer"),
        contents: bytemuck::cast_slice(&[mat]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }
        ],
        label: Some("camera bind group layout"),
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }
        ],
        label: Some("Camera bind group")
    });

    return (buffer, layout, bind_group)
}

fn make_render_pipeline(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    shader_source: &str,
    camera_bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    // TODO: create vertex shader by reading in shader from a file?
    // Nice to have: reload from file...
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        //source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("render_pipeline_layout"),
        bind_group_layouts: &[camera_bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("render_pipeline"),
        layout: Some(&render_pipeline_layout),

        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[voxel::SparseVoxel::desc()],
        },

        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            //targets: &[Some(config.format.into())],
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent::REPLACE,
                    alpha: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        // NOTE: primitives are Ccw triangles!
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: texture::Texture::DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(), // also different
        multiview: None,
    })
}

fn add_render_pass(
    encoder: &mut wgpu::CommandEncoder,
    render_pipeline: &wgpu::RenderPipeline,
    view: &wgpu::TextureView,
    camera_bind_group: &wgpu::BindGroup,
    voxel_buffer: &wgpu::Buffer,
    draw_indirect_buffer: &wgpu::Buffer,
    depth_texture: &texture::Texture,
    clear_color: wgpu::Color,
) {
    encoder.push_debug_group("add_render_pass");
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: true,
                    },
                })
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0), // TODO: what's the 1.0?
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        // Use a pipeline
        render_pass.set_pipeline(render_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, voxel_buffer.slice(..));
        // 36 vertices, NUM_VOXELS instances, no instance data.
        //render_pass.draw(0..36, 0..num_instances);
        render_pass.draw_indirect(draw_indirect_buffer, 0);
    }
    encoder.pop_debug_group();
}
