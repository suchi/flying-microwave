// Draws the microwave meshes (crates/app/src/model.rs; ADR-0026) with a
// depth buffer, the perspective camera and simple lighting: one directional
// light plus ambient, in half-Lambert form, so the brightness changes
// smoothly and stays within 0.55..1.00 as a body turns (guideline G-3).
// Emissive vertices (the lit cavity, the turntable, the LED) ignore it.
//
// The atlas holds premultiplied alpha. Three passes, all on the same depth
// buffer:
// - solid (body, cavity, turntable, LED): their images are opaque to the
//   edges, so the texel alpha is not tested; nothing is discarded, which
//   keeps edges whole and lets the early depth test work. On the LED window
//   the shader lights the segments of the clock mask.
// - cutout (door): keeps only fully opaque texels and writes depth.
// - glass (door): keeps only translucent texels (the window), blends them
//   and does not write depth.

// Number of LED segments (crates/app/src/model.rs, LED_SEGMENT_COUNT) and
// of the vectors that hold their bits, four to a vector.
const LED_SEGMENTS: u32 = 31u;
const LED_BIT_VECTORS: u32 = 8u;

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz: unit vector toward the light, in view space.
    light_dir: vec4<f32>,
    // x: ambient, y: diffuse.
    params: vec4<f32>,
    // x: the lit segments of the LED clock (core::clock::led_mask).
    led: vec4<u32>,
    // The cell of the LED image in the atlas: u0, v0, u1, v1.
    led_uv: vec4<f32>,
    // xy: size of the LED image in its own units.
    led_size: vec4<f32>,
    // rgb: color of a lit segment.
    led_color: vec4<f32>,
    // Each LED segment: left, top, width and height on the LED image
    // (crates/app/src/model.rs, led_segments).
    led_rects: array<vec4<f32>, LED_SEGMENTS>,
    // The bit of each segment in `led.x`, four to a vector.
    led_bits: array<vec4<u32>, LED_BIT_VECTORS>,
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
    // 1 on the LED window, 0 elsewhere.
    @location(8) led: u32,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) emissive: f32,
    @location(3) @interpolate(flat) led: u32,
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
    out.led = in.led;
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

// How much of the pixel at `p` (LED image units, `px` units per pixel)
// the rectangle `r` (left, top, width, height) covers: the overlap of a
// one-pixel box around `p` with the rectangle, so that the edges are smooth
// and thin segments of a far LED fade instead of breaking up (plan 0005,
// D-16).
fn coverage(p: vec2<f32>, px: vec2<f32>, r: vec4<f32>) -> f32 {
    let overlap = min(p + 0.5 * px, r.xy + r.zw) - max(p - 0.5 * px, r.xy);
    let a = clamp(overlap / px, vec2<f32>(0.0), vec2<f32>(1.0));
    return a.x * a.y;
}

@fragment
fn fs_solid(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = shade(in);
    // The position on the LED image and its size per pixel. Computed for
    // every fragment, so the derivatives stay in uniform control flow.
    let p = (in.uv - globals.led_uv.xy) / (globals.led_uv.zw - globals.led_uv.xy)
        * globals.led_size.xy;
    let px = max(fwidth(p), vec2<f32>(1e-4));
    if (in.led == 0u) {
        return vec4<f32>(color.rgb, 1.0);
    }
    // The LED window: light the segments whose bit is set over the image,
    // which shows every segment unlit.
    var lit = 0.0;
    for (var i = 0u; i < LED_SEGMENTS; i++) {
        let bit = globals.led_bits[i / 4u][i % 4u];
        if (((globals.led.x >> bit) & 1u) == 1u) {
            lit = max(lit, coverage(p, px, globals.led_rects[i]));
        }
    }
    return vec4<f32>(mix(color.rgb, globals.led_color.rgb, lit), 1.0);
}

@fragment
fn fs_cutout(in: VertexOutput) -> @location(0) vec4<f32> {
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
