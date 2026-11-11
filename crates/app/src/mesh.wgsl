// Draws the microwave meshes (crates/app/src/model.rs; ADR-0026) with a
// depth buffer, the perspective camera and simple lighting: one directional
// light plus ambient, in half-Lambert form, so the brightness changes
// smoothly and stays within 0.55..1.00 as a body turns (guideline G-3).
// Emissive vertices (the lit cavity, the turntable, the LED) ignore it.
//
// The atlas holds premultiplied alpha. The opaque pass keeps only fully
// opaque texels and writes depth; the glass pass keeps only translucent
// texels (the door window), blends them and does not write depth.

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz: unit vector toward the light, in view space.
    light_dir: vec4<f32>,
    // x: ambient, y: diffuse.
    params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) emissive: f32,
    @location(4) model0: vec4<f32>,
    @location(5) model1: vec4<f32>,
    @location(6) model2: vec4<f32>,
    @location(7) model3: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) emissive: f32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(in.model0, in.model1, in.model2, in.model3);
    var out: VertexOutput;
    // The flyers are placed in view space, so the camera is the projection.
    out.clip = globals.view_proj * model * vec4<f32>(in.position, 1.0);
    // Model matrices are rotations, translations and uniform scales, so the
    // normal transforms like a direction.
    out.normal = (model * vec4<f32>(in.normal, 0.0)).xyz;
    out.uv = in.uv;
    out.emissive = in.emissive;
    return out;
}

fn shade(in: VertexOutput) -> vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, in.uv);
    var k = in.emissive;
    if (k <= 0.0) {
        let facing = dot(normalize(in.normal), globals.light_dir.xyz);
        k = globals.params.x + globals.params.y * (0.5 + 0.5 * facing);
    }
    // Premultiplied: scaling the color keeps it premultiplied.
    return vec4<f32>(texel.rgb * k, texel.a);
}

@fragment
fn fs_opaque(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = shade(in);
    if (color.a < 0.999) {
        discard;
    }
    return color;
}

@fragment
fn fs_glass(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = shade(in);
    if (color.a >= 0.999 || color.a < 0.004) {
        discard;
    }
    return color;
}
