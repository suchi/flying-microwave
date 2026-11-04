// Draws textured quads from the sprite atlas (crates/app/src/sprite.rs).
//
// Each instance is a parallelogram in clip space, origin + s * u + t * v for
// s and t in [0, 1], drawn as a four-vertex triangle strip. The atlas holds
// premultiplied alpha, so the pipeline blends with PREMULTIPLIED_ALPHA_BLENDING.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
    @location(0) origin: vec2<f32>,
    @location(1) u: vec2<f32>,
    @location(2) v: vec2<f32>,
    @location(3) uv_min: vec2<f32>,
    @location(4) uv_size: vec2<f32>,
) -> VertexOutput {
    // Strip order: (0, 0), (1, 0), (0, 1), (1, 1).
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    var out: VertexOutput;
    out.position = vec4<f32>(origin + u * corner.x + v * corner.y, 0.0, 1.0);
    out.uv = uv_min + uv_size * corner;
    return out;
}

@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(atlas, atlas_sampler, in.uv);
}
