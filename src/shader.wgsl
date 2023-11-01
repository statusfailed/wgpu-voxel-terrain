let CHUNK_SIZE: u32 = 128u;
let N: i32 = 36;
let AMBIENT: f32 = 0.25;

// lol this is so dumb
let e = vec3<i32>(0, 0, 0);
let x = vec3<i32>(1, 0, 0);
let y = vec3<i32>(0, 1, 0);
let z = vec3<i32>(0, 0, 1);
let xy = vec3<i32>(1, 1, 0);
let xz = vec3<i32>(1, 0, 1);
let yz = vec3<i32>(0, 1, 1);
let xyz = vec3<i32>(1, 1, 1);

let nx = vec3<i32>(-1, 0, 0);
let ny = vec3<i32>(0, -1, 0);
let nz = vec3<i32>(0, 0, -1);

// TODO: why do we have to use var<private> here?
// https://stackoverflow.com/questions/73379152/how-can-i-declare-and-use-a-constant-array-in-a-wgsl-vertex-shader
var<private> TRI_VERTICES: array<vec3<i32>, N> = array<vec3<i32>, N>(
  // Front and back faces: NOTE: front faces should go opposite direction!
  e, y, xy,
  e, xy, x,
  z, xyz, yz,
  z, xz, xyz,
  // bottom and top
  e, x, xz,
  e, xz, z,
  y, xyz, xy,
  y, yz, xyz,
  // sides
  e, yz, y,
  e, z, yz,
  x, xyz, xy,
  x, xz, xyz,
);

// TODO: replace this with a length-N/6 array and divide index by 6.
var<private> TRI_NORMALS: array<vec3<i32>, N> = array<vec3<i32>, N>(
  // back
  nz, nz, nz,
  nz, nz, nz,
  // front
  z, z, z,
  z, z, z,
  // bottom
  ny, ny, ny,
  ny, ny, ny,
  // top
  y, y, y,
  y, y, y,
  // left side
  nx, nx, nx,
  nx, nx, nx,
  // right side
  x, x, x,
  x, x, x,
);

// A map from 3-bit input (so 8 entries) to 6-bit output (mask)
/*var<private> NEIGHBOUR_MASK: array<u32, N>*/

// Camera uniform
@group(0) @binding(0)
var<uniform> camera: mat4x4<f32>;

// Fixed light position
let light_pos: vec4<f32> = vec4<f32>(20.0, 8.0, 20.0, 1.0);

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) intensity: f32,
}

// the not_normal vector is (1 - normal),
// where 1 denotes the vector of only ones.
// On the name: it's literally the pointwise NOT, (if entries are boolean)
// TODO: unit_normal must be an axis-aligned vector; we could represent with
// just 3 bits indicating dimension.
// (This would require updating the TRI_NORMALS array)
fn not_normal(unit_normal: vec3<i32>) -> vec3<i32> {
  let ones = vec3<i32>(1, 1, 1);
  return ones - abs(unit_normal);
}

fn not_normals(unit_normal: vec3<i32>) -> array<vec3<i32>, 3> {
  let v = not_normal(unit_normal);
  return array<vec3<i32>, 3>(x * v, y * v, z * v);
}

fn moore_linear_index(v: vec3<i32>) -> u32 {
  return u32(v.x + 1) + 3u * u32(v.y + 1) + 9u * u32(v.z + 1);
}

fn to_corner(v: vec3<i32>) -> vec3<i32> {
	// map 0 → -1, 1 → +1
	return v * 2 - 1;
}

fn moore_neighbour_mask(v: vec3<i32>, n: vec3<i32>) -> u32 {
  let c = to_corner(v);
  let m = not_normals(n);
  let i = moore_linear_index(c - c * m[0]);
  let j = moore_linear_index(c - c * m[1]);
  let k = moore_linear_index(c - c * m[2]);
  return (1u << i) | (1u << j) | (1u << k);
}

// same as moore; we could pass a param here, but chunks don't subtract.
fn chunk_vector_index(i: u32) -> vec3<u32> {
  let s: u32 = CHUNK_SIZE * CHUNK_SIZE;
  let j: u32 = clamp(i, 0u, s * CHUNK_SIZE);

  let z: u32 = j / s;
  var y: u32 = j % s;
  let x: u32 = y % CHUNK_SIZE;
  y = y / CHUNK_SIZE;

  return vec3<u32>(x, y, z);
}

@vertex
fn vs_main(
  @builtin(vertex_index) in_vertex_index: u32,
  @builtin(instance_index) in_instance_index: u32,
  @location(0) voxel: u32,
  @location(1) neighbours: u32,
) -> VertexOutput {
  let translation = vec4<f32>(vec3<f32>(chunk_vector_index(voxel)), 1.0);

  let v = TRI_VERTICES[in_vertex_index];
  let n = TRI_NORMALS[in_vertex_index];
  let m = moore_neighbour_mask(v, n);
  let occlusion = f32(countOneBits(m & neighbours)) / 3.0;

  var frag_pos = vec4<f32>(vec3<f32>(v), 1.0) + translation; // + offset;
  let norm     = vec4<f32>(vec3<f32>(n), 1.0);
  let light_dir: vec4<f32> = normalize(light_pos - frag_pos);

  var out: VertexOutput;
  out.position = camera * frag_pos;
  out.intensity = max(dot(norm, light_dir), 0.0);
  out.intensity -= AMBIENT * occlusion; // TODO: uhhh how do I choose this >:D
  return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  return (AMBIENT + in.intensity) * vec4<f32>(0.3, 0.2, 0.1, 1.0);
}
