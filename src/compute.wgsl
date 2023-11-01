let CHUNK_SIZE: u32 = 128u;
let VOXEL_EMPTY: u32 = 0u;
let VOXEL_FULL: u32 = 1u;

fn linear_index(ix: vec3<u32>) -> u32 {
  return ix.x + ix.y * CHUNK_SIZE + ix.z * CHUNK_SIZE * CHUNK_SIZE;
}

// A dense array of voxel types
@group(0) @binding(0) var<storage, read_write> voxels: array<u32>;

// Some simple regularly-spaced mountainous terrain :^)
fn terrain_sin2d(pos: vec3<u32>) -> u32 {
  let yx: f32 = (1.0 + sin(f32(pos.x) / 4.0) / 2.0) * 8.0;
  let yz: f32 = (1.0 + sin(f32(pos.z) / 4.0) / 2.0) * 8.0;
  let y: f32 = (yx * yz) / 2.0;

  if(pos.y < u32(y)) {
    return VOXEL_FULL;
  } else {
    return VOXEL_EMPTY;
  }
}

// Compute one of the 8 monomials of a polynomial in 3 determinates.
// the lower 3 bits of 'mask' are interpreted as flags saying which of the 3
// variables of 'v' are to be multiplied.
// if 'mask' = 0, this function returns 1 (the empty product)
fn monomial(mask: u32, v: vec3<f32>) -> f32 {
  var accumulator: f32 = 1.0;
  for(var i: u32 = 0u; i < 3u; i++) {
    if( (mask & (1u << i)) != 0u) {
      accumulator *= sin(v[i]);
    }
  }
  return accumulator;
}

// given
//  pos (x, y, z)
//  coeffs a₁ .. a₈
// compute
//  a₁ + a₂·x + a₃·y + a₄·z + a₅·x·y + a₆·x·z + a₇·y·z + a₈·x·y·z
fn multi_sin(pos: vec3<f32>, freqs: array<f32, 8>, scale: array<f32, 8>) -> f32 {
  var accumulator = 0.0;
  accumulator += monomial(0u, pos * freqs[0u]) * scale[0u];
  accumulator += monomial(1u, pos * freqs[1u]) * scale[1u];
  accumulator += monomial(2u, pos * freqs[2u]) * scale[2u];
  accumulator += monomial(3u, pos * freqs[3u]) * scale[3u];
  accumulator += monomial(4u, pos * freqs[4u]) * scale[4u];
  accumulator += monomial(5u, pos * freqs[5u]) * scale[5u];
  accumulator += monomial(6u, pos * freqs[6u]) * scale[6u];
  accumulator += monomial(7u, pos * freqs[7u]) * scale[7u];
  return accumulator;
}

fn terrain_multisin(pos: vec3<u32>) -> u32 {
  let threshold: f32 = 5.0;

  let v: vec3<f32> = vec3<f32>(pos);
  let freqs = array<f32, 8>(0.1, 0.01, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1);
  let scale = array<f32, 8>(0.0, 0.1, 0.0, 0.1, 0.0, 0.1, 0.1, 0.1);
  let a: f32 = multi_sin(v, freqs, scale);
  let r: f32 = a * pow(threshold, 3.0);

  if (r > threshold) {
    return VOXEL_FULL;
  } else {
    return VOXEL_EMPTY;
  }
}

// Generate terrain using sin(x) * sin(z)
@compute
@workgroup_size(4u, 4u, 4u) // I think the product has to be < 256
fn main(
  @builtin(global_invocation_id) global_invocation_id: vec3<u32>
) {
  let i: u32 = linear_index(global_invocation_id);
  voxels[i] = terrain_sin2d(global_invocation_id) * terrain_multisin(global_invocation_id);
}

////////////////////////////////////////////////////////////////////////////////
// Neighbourhoods

struct MooreNeighbourhood {
  neighbours: array<u32, 27>,
  mask: u32,
}

// Linear index → Vector index
// e.g., 12 → (0, 0, 0)
fn moore_vector_index(i: u32) -> vec3<i32> {
  // like assert but silent :p
  let j: u32 = clamp(i, 0u, 27u);

  let z: u32 = j / 9u;
  var y: u32 = j % 9u; // temp
  let x: u32 = y % 3u;
  y = y / 3u;

  return vec3<i32>(vec3<u32>(x, y, z)) - 1;
}

// Linear index → Vector index
// e.g., ( 0,  0,  0) → 12
// e.g., (-1, -1, -1) →  0
fn moore_linear_index(v: vec3<i32>) -> u32 {
  return u32(v.x + 1) + 3u * u32(v.y + 1) + 9u * u32(v.z + 1);
}

// Read all voxels in a given moore neighbourhood
fn moore_neighbourhood(v: vec3<u32>) -> MooreNeighbourhood {
  var result = array<u32, 27>();
  var mask: u32 = 0u;

  for(var i = 0u; i < 27u; i++) {
    let offset: vec3<i32> = moore_vector_index(i);
    // true position of neighbour
    // TODO: validate???
    var u: vec3<u32> = vec3<u32>(vec3<i32>(v) + offset);
    let u_value = voxels[linear_index(u)];
    result[i] = u_value;
    mask |= (u32(u_value != VOXEL_EMPTY) << i);
  }

  return MooreNeighbourhood(result, mask);
}

fn moore_to_von_neumann(moore: MooreNeighbourhood) -> array<u32, 6> {
  // hardcoded the indices of the von neumann neighbourhood in the moore neighbourhood
  let n = moore.neighbours;
  return array<u32, 6>(
    // -x, x, -y, y, -z, z
    n[12u], n[14u], n[10u], n[16u], n[4u], n[22u]
  );
}

// Contains a 1 for each bit index of the von neumann neighbourhood
// (1 << 12) | (1 << 14) | (1 << 10) | (1 << 16) | (1 << 4) | (1 << 22);
let VON_NEUMANN_MASK: u32 = 4281360u;

////////////////////////////////////////////////////////////////////////////////
// Voxel culling and ambient occlusion
////////////////////////////////////////////////////////////////////////////////

struct SparseVoxel {
  // the voxel's linear index
  index: u32,

  // neighbourhood mask (6 bits, one for each von neumann neighbour)
  neighbours: u32
}

// Atomic counter lets us know the size of the resulting culled buffer, which we
// can then use with an indirect draw to cull vertices.
@group(0) @binding(1) var<storage, read_write> count: atomic<u32>;
@group(0) @binding(2) var<storage, read_write> visible_voxels: array<SparseVoxel>;

// Check if any (von neumann) neighbour is empty
fn has_empty_neighbour(moore: MooreNeighbourhood) -> bool {
  return (moore.mask & VON_NEUMANN_MASK) != VON_NEUMANN_MASK;
}

fn is_boundary(v: vec3<u32>) -> bool {
  // there is almost certainly a nicer way to write this but I'm bored of trying
  // to figure out what it is.
  return
    v.x == 0u || v.x == CHUNK_SIZE - 1u ||
    v.y == 0u || v.y == CHUNK_SIZE - 1u ||
    v.z == 0u || v.z == CHUNK_SIZE - 1u;
}

// is_visible if on_boundary or has_empty_neighbour
fn is_visible(v: vec3<u32>, neighbours: MooreNeighbourhood) -> bool {
  let i = linear_index(v);
  return (voxels[i] == VOXEL_FULL) && (is_boundary(v) || has_empty_neighbour(neighbours));
}

@compute
@workgroup_size(4u, 4u, 4u) // I think the product has to be < 256
fn compute_visible_voxels(
  @builtin(global_invocation_id) global_invocation_id: vec3<u32>
) {
  let pos: vec3<u32> = global_invocation_id;

  // start by writing down all indices, and reading neighbours
  let i: u32 = linear_index(pos);
  let neighbourhood: MooreNeighbourhood = moore_neighbourhood(pos);

  if(is_visible(pos, neighbourhood)) {
    let j: u32 = atomicAdd(&count, 1u);
    visible_voxels[j] = SparseVoxel(i, neighbourhood.mask);
  }
}
