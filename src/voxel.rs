/// This is the integration point between compute and render: we produce a number of voxels to be
/// rendered.
/// Later, we'll add coordinates into the voxel data (because it'll be sparse)
/// NOTE: we'll never actually instantiate this type in the CPU!

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Voxel {
    //pub value: [u32; 3], // vec3 (coordinates)
    pub value: u32, // vec3 (coordinates)
}

// TODO: a generic function/macro(?) to produce desc from an arbitrary struct.
impl Voxel {
    pub fn attr<'a>() -> &'a [wgpu::VertexAttribute] {
        &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Uint32,
            },
        ]
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: Self::attr(),
        }
    }
}

// SparseVoxels are basically just indexes; this is the format passed to the vertex shader for
// rendering: we don't want to render the entire 3D array of voxels; it's huge!
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SparseVoxel {
    pub index: u32,
    pub neighbours: u32, // 6 bit packed field
}

impl SparseVoxel {
    pub fn attr<'a>() -> &'a [wgpu::VertexAttribute] {
        &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Uint32,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<u32>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Uint32,
            },
        ]
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: Self::attr(),
        }
    }
}
