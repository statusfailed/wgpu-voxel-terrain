pub const CHUNK_SIZE: u32 = 128;

pub const NUM_VOXELS: u32 = CHUNK_SIZE.pow(3); // 8×8×8 = 512

pub const DEFAULT_CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.1,
    g: 0.2,
    b: 0.3,
    a: 1.0,
};

