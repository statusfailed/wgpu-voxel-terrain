use crate::constants::*;
use crate::voxel;

use wgpu::util::DeviceExt;

pub struct ComputeResources {
    // Voxels, visible voxels, and atomic counter
    pub voxel_buffer: wgpu::Buffer,
    pub count_atomic: wgpu::Buffer,
    pub visible_buffer: wgpu::Buffer,
    pub draw_indirect_buffer: wgpu::Buffer,

    // Bind groups for all buffers
    pub voxel_bind_group: wgpu::BindGroup,
    pub voxel_bind_group_layout: wgpu::BindGroupLayout,

    pub compute_pipeline_1: wgpu::ComputePipeline,
    pub compute_pipeline_2: wgpu::ComputePipeline,
}

impl ComputeResources {
    /// Create all the stuff we need for compute, including:
    ///     * Voxel buffer
    ///     * BindGroupLayout and BindGroup for the Voxel Buffer
    ///     * Compute pipeline
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        compute_shader_source: &str,
    ) -> Self {
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            //source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
            source: wgpu::ShaderSource::Wgsl(compute_shader_source.into()),
        });

        let voxel_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("voxel_buffer"),
            size: (std::mem::size_of::<u32>() * NUM_VOXELS as usize) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let count_atomic = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("count_atomic"),
            size: std::mem::size_of::<u32>() as wgpu::BufferAddress,
            // need COPY_DST for clearing buffer
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let visible_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("visible_buffer"),
            size: (std::mem::size_of::<voxel::SparseVoxel>() * NUM_VOXELS as usize) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create the DrawIndirect struct in GPU memory for a draw_indirect call.
        // This is not the most efficient way to do this: we could just map the relevant parts of
        // this second buffer to the shader directly, but it's a bit clearer
        let draw_indirect_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("draw_indirect_buffer"),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::INDIRECT, // TODO
            contents: wgpu::util::DrawIndirect {
                // 36 vertices in a cube (6 faces, 2 triangles each)
                vertex_count: 36,
                // we'll fill this later using copy_buffer_to_buffer
                instance_count: 0,
                // no fancy offsets required here
                base_vertex: 0,
                base_instance: 0,
            }.as_bytes(),
        });


        ////////////////////////////////////////
        // Bind groups and layouts
        let voxel_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("voxel_bind_group_layout"),
            entries: &[
                // voxel buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        // TODO: what is the min binding size?
                        min_binding_size: None,
                    },
                    count: None,
                },

                // Count atomic
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },

                // visibility buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let voxel_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("voxel_bind_group"),
            layout: &voxel_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: voxel_buffer.as_entire_binding(),
                },

                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: count_atomic.as_entire_binding(),
                },

                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: visible_buffer.as_entire_binding(),
                },
            ],
        });

        let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compute"),
            bind_group_layouts: &[&voxel_bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline_1 = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute pipeline 2"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: "main",
        });

        let compute_pipeline_2 = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute pipeline 2"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: "compute_visible_voxels",
        });

        return Self {
            voxel_buffer,
            count_atomic,
            visible_buffer,
            draw_indirect_buffer,
            voxel_bind_group,
            voxel_bind_group_layout,
            compute_pipeline_1,
            compute_pipeline_2,
        }
    }

    /// Add the compute pass to a command encoder
    pub fn add_compute_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        // reset atomic counter
        encoder.clear_buffer(&self.count_atomic, 0, None); // None => whole buffer?

        // Create terrain
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
            });
            compute_pass.set_pipeline(&self.compute_pipeline_1);
            compute_pass.set_bind_group(0, &self.voxel_bind_group, &[]);
            compute_pass.dispatch_workgroups(CHUNK_SIZE/4, CHUNK_SIZE/4, CHUNK_SIZE/4);
        }

        // Compute visibility
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
            });
            compute_pass.set_pipeline(&self.compute_pipeline_2);
            compute_pass.set_bind_group(0, &self.voxel_bind_group, &[]);
            compute_pass.dispatch_workgroups(CHUNK_SIZE/4, CHUNK_SIZE/4, CHUNK_SIZE/4);
        }

        // Copy atomic counter buffer into the draw indirect buffer, ready for rendering.
        // This tells us how many voxels will be rendered: we don't know in advance, since we cull
        // invisible voxels.
        encoder.copy_buffer_to_buffer(
            // copy all of count_atomic
            &self.count_atomic, 0,
            // into the draw_indirect_buffer, at the correct position
            &self.draw_indirect_buffer, std::mem::size_of::<u32>() as wgpu::BufferAddress,
            // size of count_atomic (TODO: factor this information out somewhere? it's repeated!)
            std::mem::size_of::<u32>() as wgpu::BufferAddress,
        );

    }
}
