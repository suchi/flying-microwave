//! Hand-written meshes of the microwave: the body box (with an opening in
//! the front and a lit cavity behind it), the door (a thin box) and the
//! turntable (a disc). One vertex and one index buffer hold them all.
//!
//! Every quad is wound counterclockwise seen from the side its normal points
//! to, so back faces can be culled.

use std::ops::Range;

use crate::art::{self, BODY_SVG, DOOR_RECT, LED_RECT, OPENING_RECT, PALETTES, SVG_PER_UNIT};
use crate::atlas::{Job, UvRect};
use crate::math::{Vec3, v3};

/// Texels per model unit for the body faces (the front is 768 x 512).
pub const TEXELS_PER_UNIT: f32 = 512.0;
/// Texels per SVG unit for the LED window (its digits are thin).
const LED_TEXELS_PER_SVG: f32 = 8.0;
/// Door thickness in model units.
pub const DOOR_THICKNESS: f32 = 0.045;
/// How far the LED quad sits in front of the front face (against z-fighting).
const LED_OFFSET: f32 = 0.003;
/// Wall thickness behind the cavity.
const CAVITY_BACK_WALL: f32 = 0.1;

/// Emissive factors of the unlit parts: they ignore the directional light.
/// 0 means "lit by the scene light".
const EMIT_CEILING: f32 = 1.0;
const EMIT_FLOOR: f32 = 0.9;
const EMIT_BACK: f32 = 0.8;
const EMIT_SIDES: f32 = 0.7;
const EMIT_TURNTABLE: f32 = 0.95;
const EMIT_LED: f32 = 1.0;

/// Half extents of the body in model units.
pub fn half_extents() -> Vec3 {
    let [w, h, d] = BODY_SVG;
    v3(w, h, d).scale(0.5 / SVG_PER_UNIT)
}

/// A point of the front image (SVG units, y down) on the front plane.
fn front_point(sx: f32, sy: f32) -> (f32, f32) {
    let e = half_extents();
    (-e.x + sx / SVG_PER_UNIT, e.y - sy / SVG_PER_UNIT)
}

/// Where the hinge axis of the door is: x and z of the vertical line.
pub fn hinge() -> (f32, f32) {
    let (x, _) = front_point(DOOR_RECT[0], 0.0);
    (x, half_extents().z)
}

/// Center of the turntable in body coordinates, and its radius.
pub fn turntable_place() -> (Vec3, f32) {
    let (x0, y0, x1, _, zf, zb) = cavity_box();
    let center = v3((x0 + x1) / 2.0, y0 + 0.03, (zf + zb) / 2.0);
    let radius = (x1 - x0).min(zf - zb) / 2.0 * 0.85;
    (center, radius)
}

/// The cavity: x0, y0 (bottom), x1, y1 (top), front z, back z.
fn cavity_box() -> (f32, f32, f32, f32, f32, f32) {
    let [ox, oy, ow, oh] = OPENING_RECT;
    let (x0, y1) = front_point(ox, oy);
    let (x1, y0) = front_point(ox + ow, oy + oh);
    let e = half_extents();
    (x0, y0, x1, y1, e.z, -e.z + CAVITY_BACK_WALL)
}

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub emissive: f32,
}

pub const VERTEX_BYTES: u64 = 36;

impl Vertex {
    pub fn write_to(&self, out: &mut Vec<u8>) {
        for v in self
            .pos
            .iter()
            .chain(&self.normal)
            .chain(&self.uv)
            .chain([&self.emissive])
        {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}

/// Indices of the face images in the atlas jobs.
pub struct Textures {
    pub front: [usize; 4],
    pub side: [usize; 4],
    pub plain: [usize; 4],
    pub back: [usize; 4],
    pub door_outer: usize,
    pub door_inner: usize,
    pub door_edge: usize,
    pub cavity: usize,
    pub turntable: usize,
    pub led: usize,
}

fn px(units: f32) -> u32 {
    (units * TEXELS_PER_UNIT / SVG_PER_UNIT).round() as u32
}

/// The atlas jobs of every face image, and where each one is.
pub fn texture_jobs() -> (Vec<Job>, Textures) {
    let mut jobs = Vec::new();
    let mut add = |svg: String, w: u32, h: u32| {
        jobs.push(Job { svg, w, h });
        jobs.len() - 1
    };
    let [bw, bh, bd] = BODY_SVG;
    let front = PALETTES.map(|p| add(art::front(&p), px(bw), px(bh)));
    let side = PALETTES.map(|p| add(art::side_right(&p), px(bd), px(bh)));
    let plain = PALETTES.map(|p| add(art::plain(&p), 64, 64));
    let back = PALETTES.map(|p| add(art::back(&p), px(bw), px(bh)));
    let [_, _, dw, dh] = DOOR_RECT;
    let door_outer = add(art::door_outer(), px(dw), px(dh));
    let door_inner = add(art::door_inner(), px(dw), px(dh));
    let door_edge = add(art::door_edge(), 16, 16);
    let cavity = add(art::cavity(), 128, 128);
    let turntable = add(art::turntable(), 256, 256);
    let [_, _, lw, lh] = LED_RECT;
    let led = add(
        art::led("12:34"),
        (lw * LED_TEXELS_PER_SVG) as u32,
        (lh * LED_TEXELS_PER_SVG) as u32,
    );
    let textures = Textures {
        front,
        side,
        plain,
        back,
        door_outer,
        door_inner,
        door_edge,
        cavity,
        turntable,
        led,
    };
    (jobs, textures)
}

/// Every mesh, in one vertex and one index list.
pub struct Meshes {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// The body of each color, in the order of [`PALETTES`].
    pub body: [Range<u32>; 4],
    pub door: Range<u32>,
    pub turntable: Range<u32>,
}

struct Builder {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Builder {
    /// A quad from its bottom-left corner and its right and up edges, with
    /// the image `uv` upright on it. The normal is `right x up`.
    fn quad(&mut self, bl: Vec3, right: Vec3, up: Vec3, uv: UvRect, emissive: f32) {
        let normal = right.cross(up).normalized().to_array();
        let base = self.vertices.len() as u32;
        let corners = [
            (bl, [uv.u0, uv.v1]),
            (bl.add(right), [uv.u1, uv.v1]),
            (bl.add(right).add(up), [uv.u1, uv.v0]),
            (bl.add(up), [uv.u0, uv.v0]),
        ];
        for (p, uv) in corners {
            self.vertices.push(Vertex {
                pos: p.to_array(),
                normal,
                uv,
                emissive,
            });
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn mark(&self) -> u32 {
        self.indices.len() as u32
    }
}

/// Builds every mesh. `uvs` are the atlas cells of [`texture_jobs`].
pub fn build(uvs: &[UvRect], t: &Textures) -> Meshes {
    let mut b = Builder {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    let body = std::array::from_fn(|color| {
        let start = b.mark();
        body(&mut b, uvs, t, color);
        start..b.mark()
    });
    let start = b.mark();
    door(&mut b, uvs, t);
    let door = start..b.mark();
    let start = b.mark();
    turntable(&mut b, uvs[t.turntable]);
    let turntable = start..b.mark();
    Meshes {
        vertices: b.vertices,
        indices: b.indices,
        body,
        door,
        turntable,
    }
}

fn body(b: &mut Builder, uvs: &[UvRect], t: &Textures, color: usize) {
    let e = half_extents();
    let (w, h, d) = (2.0 * e.x, 2.0 * e.y, 2.0 * e.z);
    let x = v3(1.0, 0.0, 0.0);
    let y = v3(0.0, 1.0, 0.0);
    let z = v3(0.0, 0.0, 1.0);
    let plain = uvs[t.plain[color]];
    // Back, right, left, top, bottom.
    b.quad(
        v3(e.x, -e.y, -e.z),
        x.scale(-w),
        y.scale(h),
        uvs[t.back[color]],
        0.0,
    );
    b.quad(
        v3(e.x, -e.y, e.z),
        z.scale(-d),
        y.scale(h),
        uvs[t.side[color]],
        0.0,
    );
    b.quad(v3(-e.x, -e.y, -e.z), z.scale(d), y.scale(h), plain, 0.0);
    b.quad(v3(-e.x, e.y, e.z), x.scale(w), z.scale(-d), plain, 0.0);
    b.quad(v3(-e.x, -e.y, -e.z), x.scale(w), z.scale(d), plain, 0.0);

    // The front, as four strips around the opening of the cavity.
    let front = uvs[t.front[color]];
    let [fw, fh, _] = BODY_SVG;
    let [ox, oy, ow, oh] = OPENING_RECT;
    let strips = [
        (0.0, 0.0, fw, oy),
        (0.0, oy + oh, fw, fh),
        (0.0, oy, ox, oy + oh),
        (ox + ow, oy, fw, oy + oh),
    ];
    for (sx0, sy0, sx1, sy1) in strips {
        let (x0, y1) = front_point(sx0, sy0);
        let (x1, y0) = front_point(sx1, sy1);
        let uv = front.sub(sx0 / fw, sy0 / fh, sx1 / fw, sy1 / fh);
        b.quad(v3(x0, y0, e.z), x.scale(x1 - x0), y.scale(y1 - y0), uv, 0.0);
    }

    // The LED window, unlit, just in front of the panel.
    let [lx, ly, lw, lh] = LED_RECT;
    let (x0, y1) = front_point(lx, ly);
    let (x1, y0) = front_point(lx + lw, ly + lh);
    b.quad(
        v3(x0, y0, e.z + LED_OFFSET),
        x.scale(x1 - x0),
        y.scale(y1 - y0),
        uvs[t.led],
        EMIT_LED,
    );

    // The cavity, with its faces turned inward.
    let (x0, y0, x1, y1, zf, zb) = cavity_box();
    let (cw, ch, cd) = (x1 - x0, y1 - y0, zf - zb);
    let glow = uvs[t.cavity];
    b.quad(v3(x0, y0, zb), x.scale(cw), y.scale(ch), glow, EMIT_BACK);
    b.quad(v3(x0, y0, zf), x.scale(cw), z.scale(-cd), glow, EMIT_FLOOR);
    b.quad(v3(x0, y1, zb), x.scale(cw), z.scale(cd), glow, EMIT_CEILING);
    b.quad(v3(x0, y0, zf), z.scale(-cd), y.scale(ch), glow, EMIT_SIDES);
    b.quad(v3(x1, y0, zb), z.scale(cd), y.scale(ch), glow, EMIT_SIDES);
}

/// The door in hinge coordinates: the hinge axis is the y axis, the closed
/// door lies in x 0..width, z 0..thickness, centered on y = 0.
fn door(b: &mut Builder, uvs: &[UvRect], t: &Textures) {
    let [_, _, dw, dh] = DOOR_RECT;
    let (w, h, th) = (dw / SVG_PER_UNIT, dh / SVG_PER_UNIT, DOOR_THICKNESS);
    let hh = h / 2.0;
    let x = v3(1.0, 0.0, 0.0);
    let y = v3(0.0, 1.0, 0.0);
    let z = v3(0.0, 0.0, 1.0);
    let edge = uvs[t.door_edge];
    b.quad(
        v3(0.0, -hh, th),
        x.scale(w),
        y.scale(h),
        uvs[t.door_outer],
        0.0,
    );
    b.quad(
        v3(w, -hh, 0.0),
        x.scale(-w),
        y.scale(h),
        uvs[t.door_inner],
        0.0,
    );
    b.quad(v3(0.0, -hh, 0.0), z.scale(th), y.scale(h), edge, 0.0);
    b.quad(v3(w, -hh, th), z.scale(-th), y.scale(h), edge, 0.0);
    b.quad(v3(0.0, hh, th), x.scale(w), z.scale(-th), edge, 0.0);
    b.quad(v3(0.0, -hh, 0.0), x.scale(w), z.scale(th), edge, 0.0);
}

/// The turntable: a disc of radius 1 around the origin, facing +y.
fn turntable(b: &mut Builder, uv: UvRect) {
    const SEGMENTS: u32 = 48;
    let at = |fu: f32, fv: f32| [uv.u0 + (uv.u1 - uv.u0) * fu, uv.v0 + (uv.v1 - uv.v0) * fv];
    let base = b.vertices.len() as u32;
    b.vertices.push(Vertex {
        pos: [0.0, 0.0, 0.0],
        normal: [0.0, 1.0, 0.0],
        uv: at(0.5, 0.5),
        emissive: EMIT_TURNTABLE,
    });
    for i in 0..SEGMENTS {
        let (s, c) = (std::f32::consts::TAU * i as f32 / SEGMENTS as f32).sin_cos();
        // Seen from above, image up is -z.
        b.vertices.push(Vertex {
            pos: [c, 0.0, -s],
            normal: [0.0, 1.0, 0.0],
            uv: at(0.5 + 0.5 * c, 0.5 - 0.5 * s),
            emissive: EMIT_TURNTABLE,
        });
    }
    for i in 0..SEGMENTS {
        let next = (i + 1) % SEGMENTS;
        b.indices.extend([base, base + 1 + i, base + 1 + next]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_matches_the_sprite_proportions() {
        let e = half_extents();
        assert!((e.x / e.y - 1.5).abs() < 1e-6);
        assert!((e.z / e.y - 210.0 / 160.0).abs() < 1e-6);
    }

    #[test]
    fn every_triangle_faces_its_normal() {
        let (jobs, t) = texture_jobs();
        let uvs: Vec<UvRect> = jobs
            .iter()
            .map(|_| UvRect {
                u0: 0.0,
                v0: 0.0,
                u1: 1.0,
                v1: 1.0,
            })
            .collect();
        let m = build(&uvs, &t);
        for tri in m.indices.chunks(3) {
            let p = |i: u32| {
                let v = m.vertices[i as usize].pos;
                v3(v[0], v[1], v[2])
            };
            let n = m.vertices[tri[0] as usize].normal;
            let face = p(tri[1]).sub(p(tri[0])).cross(p(tri[2]).sub(p(tri[0])));
            assert!(face.dot(v3(n[0], n[1], n[2])) > 0.0, "triangle {tri:?}");
        }
    }
}
