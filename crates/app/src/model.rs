//! The 3D microwave (ADR-0026): the face images on the SVG sheets of
//! `assets/sprites/`, the hand-written meshes (the body box with an opening
//! in the front and a lit cavity behind it, the door, the turntable), the
//! model matrices of one flyer and the light.
//!
//! Everything here has no platform dependency and is tested on every host.
//! Model units are body heights, as in `core::flight`: the body is
//! [`BODY_SIZE`] at scale 1.0 and centered on the origin, its front faces +z.
//! Every quad is wound counterclockwise seen from the side its normal
//! points to, so back faces can be culled.

use std::f32::consts::TAU;
use std::ops::Range;

use flying_microwave_core::clock;
use flying_microwave_core::flight::FlyerInstance;
use flying_microwave_core::sim::BODY_COLORS;
use flying_microwave_core::space::{Mat4, Quat, Vec3};

/// Background color (8-bit RGB). Dark, as guideline G-5 asks.
pub const BACKGROUND_RGB: [u8; 3] = [6, 8, 18];

/// The body sheets, in the order of [`FlyerInstance::color`].
pub const BODY_SVGS: [&str; BODY_COLORS as usize] = [
    include_str!("../../../assets/sprites/microwave-body-silver.svg"),
    include_str!("../../../assets/sprites/microwave-body-graphite.svg"),
    include_str!("../../../assets/sprites/microwave-body-white.svg"),
    include_str!("../../../assets/sprites/microwave-body-mint.svg"),
];
/// The door sheet: outside, inside and edges.
pub const DOOR_SVG: &str = include_str!("../../../assets/sprites/microwave-door.svg");
/// The interior sheet: cavity walls and turntable.
pub const INTERIOR_SVG: &str = include_str!("../../../assets/sprites/microwave-interior.svg");
/// The LED window.
pub const LED_SVG: &str = include_str!("../../../assets/sprites/microwave-led.svg");

/// SVG units of the body sheets per model unit (the front is 240 x 160).
const SVG_PER_UNIT: f32 = 160.0;
/// Body size in SVG units: width, height and depth.
const BODY_SVG: [f32; 3] = [240.0, 160.0, 210.0];
/// The door on the front image: left, top, width, height (SVG units).
const DOOR_RECT: [f32; 4] = [10.0, 10.0, 156.0, 140.0];
/// The opening of the cavity behind the door, on the front image.
const OPENING_RECT: [f32; 4] = [24.0, 24.0, 128.0, 112.0];
/// The LED window on the front image.
const LED_RECT: [f32; 4] = [178.0, 18.0, 48.0, 22.0];
/// Texels per SVG unit of the body and door faces (512 per model unit).
const TEXELS_PER_SVG: f32 = 3.2;
/// Texels per SVG unit of the LED window (its segments are thin).
const LED_TEXELS_PER_SVG: f32 = 8.0;
/// Door thickness in model units.
const DOOR_THICKNESS: f32 = 0.045;
/// How far the LED quad sits in front of the front face (against z-fighting).
const LED_OFFSET: f32 = 0.003;
/// How far the lit LED segments sit in front of the LED quad.
const LED_SEGMENT_OFFSET: f32 = 0.002;

/// One LED segment: left, top, width and height on the LED image (SVG
/// units of `microwave-led.svg`), and its bit in the mask of `core::clock`.
type Segment = ([f32; 4], u32);

/// Rectangles of the segments a to g of a digit whose left edge is `x0`
/// (the same as the unlit segments of `microwave-led.svg`).
const fn digit_rects(x0: f32) -> [[f32; 4]; 7] {
    const Y0: f32 = 4.0;
    [
        [x0 + 1.0, Y0, 6.0, 2.0],
        [x0 + 6.0, Y0 + 2.0, 2.0, 5.0],
        [x0 + 6.0, Y0 + 7.0, 2.0, 5.0],
        [x0 + 1.0, Y0 + 12.0, 6.0, 2.0],
        [x0, Y0 + 7.0, 2.0, 5.0],
        [x0, Y0 + 2.0, 2.0, 5.0],
        [x0 + 1.0, Y0 + 6.0, 6.0, 2.0],
    ]
}

/// Left edges of the four digits on the LED image.
const DIGIT_X: [f32; clock::DIGITS as usize] = [3.0, 13.0, 27.0, 37.0];

/// Every LED segment that can be lit: the digits, the two dots of the
/// colon and the PM dot.
fn led_segments() -> Vec<Segment> {
    let mut out = Vec::new();
    for (digit, x0) in (0..).zip(DIGIT_X) {
        for (segment, rect) in (0..).zip(digit_rects(x0)) {
            out.push((rect, clock::segment_bit(digit, segment)));
        }
    }
    out.push(([23.0, 7.5, 2.0, 2.0], clock::COLON_BIT));
    out.push(([23.0, 12.5, 2.0, 2.0], clock::COLON_BIT));
    out.push(([0.5, 16.0, 2.0, 2.0], clock::PM_BIT));
    out
}
/// Wall thickness behind the cavity.
const CAVITY_BACK_WALL: f32 = 0.1;
/// Segments of the turntable disc.
const TURNTABLE_SEGMENTS: u32 = 48;

/// Brightness of the unlit parts: they ignore the scene light. 0 means
/// "lit by the scene light".
const EMIT_CEILING: f32 = 1.0;
const EMIT_FLOOR: f32 = 0.9;
const EMIT_BACK: f32 = 0.8;
const EMIT_SIDES: f32 = 0.7;
const EMIT_TURNTABLE: f32 = 0.95;
const EMIT_LED: f32 = 1.0;

/// Brightness of a lit face: `AMBIENT + DIFFUSE * (0.5 + 0.5 * cos)` of the
/// angle to the light, so it stays within 0.55..1.00 whatever the
/// orientation (guideline G-3; ADR-0026).
pub const AMBIENT: f32 = 0.55;
pub const DIFFUSE: f32 = 0.45;

/// Unit vector toward the light in view space: from the upper left, in front.
pub fn light_dir() -> Vec3 {
    Vec3::new(-0.35, 0.8, 0.5).normalized()
}

/// An SVG document that holds face images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sheet {
    /// The body of a color, below [`BODY_COLORS`].
    Body(u8),
    Door,
    Interior,
    Led,
}

impl Sheet {
    pub fn svg(self) -> &'static str {
        match self {
            Sheet::Body(color) => BODY_SVGS[usize::from(color) % BODY_SVGS.len()],
            Sheet::Door => DOOR_SVG,
            Sheet::Interior => INTERIOR_SVG,
            Sheet::Led => LED_SVG,
        }
    }
}

/// Every sheet, each once.
pub fn sheets() -> Vec<Sheet> {
    (0..BODY_COLORS)
        .map(Sheet::Body)
        .chain([Sheet::Door, Sheet::Interior, Sheet::Led])
        .collect()
}

/// One face image: where it is on its sheet and its size in the atlas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceImage {
    pub sheet: Sheet,
    /// Left, top, width and height on the sheet, in SVG units.
    pub rect: [f32; 4],
    /// Size in pixels.
    pub width: u32,
    pub height: u32,
}

/// The face images of the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Front(u8),
    Back(u8),
    /// The right side.
    Side(u8),
    /// The left side, the top and the bottom.
    Plain(u8),
    DoorOuter,
    DoorInner,
    DoorEdge,
    Cavity,
    Turntable,
    Led,
    /// The color of a lit LED segment.
    LedSegment,
}

/// Number of face images.
#[cfg(test)]
pub const FACE_COUNT: usize = 4 * BODY_COLORS as usize + 7;

impl Face {
    /// Every face, in the order of [`Face::index`].
    pub fn all() -> Vec<Face> {
        let colors = 0..BODY_COLORS;
        colors
            .clone()
            .map(Face::Front)
            .chain(colors.clone().map(Face::Back))
            .chain(colors.clone().map(Face::Side))
            .chain(colors.map(Face::Plain))
            .chain([
                Face::DoorOuter,
                Face::DoorInner,
                Face::DoorEdge,
                Face::Cavity,
                Face::Turntable,
                Face::Led,
                Face::LedSegment,
            ])
            .collect()
    }

    /// Position in [`Face::all`] and in the atlas cells.
    pub fn index(self) -> usize {
        let colors = usize::from(BODY_COLORS);
        let color = |c: u8| usize::from(c) % colors;
        match self {
            Face::Front(c) => color(c),
            Face::Back(c) => colors + color(c),
            Face::Side(c) => 2 * colors + color(c),
            Face::Plain(c) => 3 * colors + color(c),
            Face::DoorOuter => 4 * colors,
            Face::DoorInner => 4 * colors + 1,
            Face::DoorEdge => 4 * colors + 2,
            Face::Cavity => 4 * colors + 3,
            Face::Turntable => 4 * colors + 4,
            Face::Led => 4 * colors + 5,
            Face::LedSegment => 4 * colors + 6,
        }
    }

    /// Where the image is on its sheet and how large it is rasterized.
    /// The positions are those of the comments in the SVG files.
    pub fn image(self) -> FaceImage {
        let body = |color: u8, rect: [f32; 4], width: u32, height: u32| FaceImage {
            sheet: Sheet::Body(color),
            rect,
            width,
            height,
        };
        let px = |units: f32| (units * TEXELS_PER_SVG).round() as u32;
        let [bw, bh, bd] = BODY_SVG;
        let [_, _, dw, dh] = DOOR_RECT;
        let [_, _, lw, lh] = LED_RECT;
        let other = |sheet: Sheet, rect: [f32; 4], width: u32, height: u32| FaceImage {
            sheet,
            rect,
            width,
            height,
        };
        match self {
            Face::Front(c) => body(c, [0.0, 0.0, bw, bh], px(bw), px(bh)),
            Face::Back(c) => body(c, [250.0, 0.0, bw, bh], px(bw), px(bh)),
            Face::Side(c) => body(c, [500.0, 0.0, bd, bh], px(bd), px(bh)),
            Face::Plain(c) => body(c, [0.0, 170.0, 40.0, 40.0], 64, 64),
            Face::DoorOuter => other(Sheet::Door, [0.0, 0.0, dw, dh], px(dw), px(dh)),
            Face::DoorInner => other(Sheet::Door, [160.0, 0.0, dw, dh], px(dw), px(dh)),
            Face::DoorEdge => other(Sheet::Door, [320.0, 0.0, 10.0, 10.0], 16, 16),
            Face::Cavity => other(Sheet::Interior, [0.0, 0.0, 128.0, 128.0], 128, 128),
            Face::Turntable => other(Sheet::Interior, [140.0, 0.0, 256.0, 256.0], 256, 256),
            Face::LedSegment => other(Sheet::Interior, [410.0, 0.0, 10.0, 10.0], 16, 16),
            Face::Led => other(
                Sheet::Led,
                [0.0, 0.0, lw, lh],
                (lw * LED_TEXELS_PER_SVG) as u32,
                (lh * LED_TEXELS_PER_SVG) as u32,
            ),
        }
    }
}

/// A rectangle in texture coordinates: u0, v0 (top left), u1, v1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvRect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

impl UvRect {
    /// The part of this rectangle between the fractions `fx0..fx1` across
    /// and `fy0..fy1` down.
    fn sub(&self, fx0: f32, fy0: f32, fx1: f32, fy1: f32) -> UvRect {
        let du = self.u1 - self.u0;
        let dv = self.v1 - self.v0;
        UvRect {
            u0: self.u0 + du * fx0,
            v0: self.v0 + dv * fy0,
            u1: self.u0 + du * fx1,
            v1: self.v0 + dv * fy1,
        }
    }
}

/// One mesh vertex as the vertex shader takes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Brightness of an unlit part, or 0 for a part lit by the scene light.
    pub emissive: f32,
    /// 0, or for a segment of the LED clock, its bit in the mask of
    /// `core::clock` plus 1: drawn only while that bit is set.
    pub segment: u32,
}

/// Bytes of one [`Vertex`] in the vertex buffer (nine `f32` and a `u32`).
pub const VERTEX_BYTES: u64 = 40;

impl Vertex {
    /// Appends the vertex, in the order of the shader's locations.
    pub fn write_to(&self, out: &mut Vec<u8>) {
        for v in self
            .pos
            .iter()
            .chain(&self.normal)
            .chain(&self.uv)
            .chain([&self.emissive])
        {
            out.extend_from_slice(&v.to_ne_bytes());
        }
        out.extend_from_slice(&self.segment.to_ne_bytes());
    }
}

/// Every mesh, in one vertex and one index list.
#[derive(Debug, Clone, PartialEq)]
pub struct Meshes {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// The body of each color, in the order of [`BODY_SVGS`].
    pub body: [Range<u32>; BODY_COLORS as usize],
    /// The door in hinge coordinates (see [`matrices`]).
    pub door: Range<u32>,
    /// The turntable: a disc of radius 1 facing +y.
    pub turntable: Range<u32>,
}

/// Half extents of the body in model units.
fn half_extents() -> Vec3 {
    let [w, h, d] = BODY_SVG;
    Vec3::new(w, h, d).scale(0.5 / SVG_PER_UNIT)
}

/// A point of the front image (SVG units, y down) on the front plane.
fn front_point(sx: f32, sy: f32) -> (f32, f32) {
    let e = half_extents();
    (-e.x + sx / SVG_PER_UNIT, e.y - sy / SVG_PER_UNIT)
}

/// The hinge axis of the door: x and z of the vertical line.
fn hinge() -> Vec3 {
    let (x, _) = front_point(DOOR_RECT[0], 0.0);
    let (_, y) = front_point(0.0, DOOR_RECT[1] + DOOR_RECT[3] / 2.0);
    Vec3::new(x, y, half_extents().z)
}

/// The cavity: x0, y0 (bottom), x1, y1 (top), front z, back z.
fn cavity_box() -> (f32, f32, f32, f32, f32, f32) {
    let [ox, oy, ow, oh] = OPENING_RECT;
    let (x0, y1) = front_point(ox, oy);
    let (x1, y0) = front_point(ox + ow, oy + oh);
    let e = half_extents();
    (x0, y0, x1, y1, e.z, -e.z + CAVITY_BACK_WALL)
}

/// Center of the turntable in body coordinates, and its radius.
fn turntable_place() -> (Vec3, f32) {
    let (x0, y0, x1, _, zf, zb) = cavity_box();
    let center = Vec3::new((x0 + x1) / 2.0, y0 + 0.03, (zf + zb) / 2.0);
    let radius = (x1 - x0).min(zf - zb) / 2.0 * 0.85;
    (center, radius)
}

struct Builder {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Builder {
    /// A quad from its bottom-left corner and its right and up edges, with
    /// the image `uv` upright on it. The normal is `right x up`.
    fn quad(&mut self, bl: Vec3, right: Vec3, up: Vec3, uv: UvRect, emissive: f32) {
        self.corners([bl, bl + right, bl + right + up, bl + up], uv, emissive);
    }

    /// A quad from its corners: bottom left, bottom right, top right and top
    /// left, with the image `uv` upright on it. Neighboring quads pass the
    /// same corner values, so that their shared edges match exactly.
    fn corners(&mut self, [bl, br, tr, tl]: [Vec3; 4], uv: UvRect, emissive: f32) {
        let normal = (br - bl).cross(tl - bl).normalized();
        let base = self.vertices.len() as u32;
        let corners = [
            (bl, [uv.u0, uv.v1]),
            (br, [uv.u1, uv.v1]),
            (tr, [uv.u1, uv.v0]),
            (tl, [uv.u0, uv.v0]),
        ];
        for (p, uv) in corners {
            self.vertices.push(Vertex {
                pos: [p.x, p.y, p.z],
                normal: [normal.x, normal.y, normal.z],
                uv,
                emissive,
                segment: 0,
            });
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A face split into a grid of quads: `us` are the cuts from the left to
    /// the right of its image, `vs` from the bottom to the top, and `at(u, v)`
    /// is the point. The image `uv` spans the whole face. The quad at `skip`
    /// is left out (the opening in the front).
    fn grid(
        &mut self,
        us: &[f32],
        vs: &[f32],
        at: impl Fn(f32, f32) -> Vec3,
        uv: UvRect,
        skip: Option<(usize, usize)>,
    ) {
        let (u_first, u_last) = (us[0], us[us.len() - 1]);
        let (v_first, v_last) = (vs[0], vs[vs.len() - 1]);
        let fx = |u: f32| (u - u_first) / (u_last - u_first);
        // Image fractions grow downward, the cuts upward.
        let fy = |v: f32| (v_last - v) / (v_last - v_first);
        for i in 0..us.len() - 1 {
            for j in 0..vs.len() - 1 {
                if skip == Some((i, j)) {
                    continue;
                }
                let (u0, u1, v0, v1) = (us[i], us[i + 1], vs[j], vs[j + 1]);
                let part = uv.sub(fx(u0), fy(v1), fx(u1), fy(v0));
                self.corners([at(u0, v0), at(u1, v0), at(u1, v1), at(u0, v1)], part, 0.0);
            }
        }
    }

    fn mark(&self) -> u32 {
        self.indices.len() as u32
    }
}

/// Builds every mesh. `uvs[face.index()]` is the atlas cell of each face.
pub fn build(uvs: &[UvRect]) -> Meshes {
    let mut b = Builder {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    let body = std::array::from_fn(|color| {
        let start = b.mark();
        body(&mut b, uvs, color as u8);
        start..b.mark()
    });
    let start = b.mark();
    door(&mut b, uvs);
    let door = start..b.mark();
    let start = b.mark();
    turntable(&mut b, uvs[Face::Turntable.index()]);
    let turntable = start..b.mark();
    Meshes {
        vertices: b.vertices,
        indices: b.indices,
        body,
        door,
        turntable,
    }
}

const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);

fn body(b: &mut Builder, uvs: &[UvRect], color: u8) {
    let uv = |face: Face| uvs[face.index()];
    let e = half_extents();
    let plain = uv(Face::Plain(color));
    // Every face of the box is cut where the opening of the front is, so
    // that neighboring faces share all their edge vertices (no T-junctions,
    // which can leave one-pixel cracks).
    let (x0, y0, x1, y1, zf, zb) = cavity_box();
    let xs = [-e.x, x0, x1, e.x];
    let ys = [-e.y, y0, y1, e.y];
    let back_xs = [e.x, x1, x0, -e.x];
    let (front_zs, back_zs) = ([e.z, -e.z], [-e.z, e.z]);
    // The front, without its middle quad (the opening).
    let front = uv(Face::Front(color));
    b.grid(&xs, &ys, |x, y| Vec3::new(x, y, e.z), front, Some((1, 1)));
    // The back, seen from behind; the right side, its left edge at the front.
    let back = uv(Face::Back(color));
    b.grid(&back_xs, &ys, |x, y| Vec3::new(x, y, -e.z), back, None);
    let side = uv(Face::Side(color));
    b.grid(&front_zs, &ys, |z, y| Vec3::new(e.x, y, z), side, None);
    // Left, top and bottom.
    b.grid(&back_zs, &ys, |z, y| Vec3::new(-e.x, y, z), plain, None);
    b.grid(&xs, &front_zs, |x, z| Vec3::new(x, e.y, z), plain, None);
    b.grid(&xs, &back_zs, |x, z| Vec3::new(x, -e.y, z), plain, None);

    // The LED window, unlit, just in front of the panel.
    let [lx, ly, lw, lh] = LED_RECT;
    let (lx0, ly1) = front_point(lx, ly);
    let (lx1, ly0) = front_point(lx + lw, ly + lh);
    b.quad(
        Vec3::new(lx0, ly0, e.z + LED_OFFSET),
        X.scale(lx1 - lx0),
        Y.scale(ly1 - ly0),
        uv(Face::Led),
        EMIT_LED,
    );
    // The lit segments, just in front of it: the shader draws each one only
    // while its bit of the clock mask is set.
    let [_, _, lw_img, lh_img] = LED_RECT;
    let to_front = |x: f32, y: f32| {
        // LED image units to the front plane (the image fills the window).
        (
            lx0 + (lx1 - lx0) * x / lw_img,
            ly1 - (ly1 - ly0) * y / lh_img,
        )
    };
    let z = e.z + LED_OFFSET + LED_SEGMENT_OFFSET;
    for ([x, y, w, h], bit) in led_segments() {
        let (x0, y1) = to_front(x, y);
        let (x1, y0) = to_front(x + w, y + h);
        let corners = [
            Vec3::new(x0, y0, z),
            Vec3::new(x1, y0, z),
            Vec3::new(x1, y1, z),
            Vec3::new(x0, y1, z),
        ];
        let first = b.vertices.len();
        b.corners(corners, uv(Face::LedSegment), EMIT_LED);
        for v in &mut b.vertices[first..] {
            v.segment = bit + 1;
        }
    }

    // The cavity, with its faces turned inward. Its front edges are the
    // edges of the opening.
    let glow = uv(Face::Cavity);
    let p = Vec3::new;
    let walls = [
        // Back, floor, ceiling, left and right.
        (
            [p(x0, y0, zb), p(x1, y0, zb), p(x1, y1, zb), p(x0, y1, zb)],
            EMIT_BACK,
        ),
        (
            [p(x0, y0, zf), p(x1, y0, zf), p(x1, y0, zb), p(x0, y0, zb)],
            EMIT_FLOOR,
        ),
        (
            [p(x0, y1, zb), p(x1, y1, zb), p(x1, y1, zf), p(x0, y1, zf)],
            EMIT_CEILING,
        ),
        (
            [p(x0, y0, zf), p(x0, y0, zb), p(x0, y1, zb), p(x0, y1, zf)],
            EMIT_SIDES,
        ),
        (
            [p(x1, y0, zb), p(x1, y0, zf), p(x1, y1, zf), p(x1, y1, zb)],
            EMIT_SIDES,
        ),
    ];
    for (corners, emissive) in walls {
        b.corners(corners, glow, emissive);
    }
}

/// The door in hinge coordinates: the hinge axis is the y axis, the closed
/// door lies in x 0..width, z 0..thickness, centered on y = 0.
fn door(b: &mut Builder, uvs: &[UvRect]) {
    let uv = |face: Face| uvs[face.index()];
    let [_, _, dw, dh] = DOOR_RECT;
    let (w, h, th) = (dw / SVG_PER_UNIT, dh / SVG_PER_UNIT, DOOR_THICKNESS);
    let hh = h / 2.0;
    let edge = uv(Face::DoorEdge);
    b.quad(
        Vec3::new(0.0, -hh, th),
        X.scale(w),
        Y.scale(h),
        uv(Face::DoorOuter),
        0.0,
    );
    b.quad(
        Vec3::new(w, -hh, 0.0),
        X.scale(-w),
        Y.scale(h),
        uv(Face::DoorInner),
        0.0,
    );
    b.quad(Vec3::new(0.0, -hh, 0.0), Z.scale(th), Y.scale(h), edge, 0.0);
    b.quad(Vec3::new(w, -hh, th), Z.scale(-th), Y.scale(h), edge, 0.0);
    b.quad(Vec3::new(0.0, hh, th), X.scale(w), Z.scale(-th), edge, 0.0);
    b.quad(Vec3::new(0.0, -hh, 0.0), X.scale(w), Z.scale(th), edge, 0.0);
}

/// The turntable: a disc of radius 1 around the origin, facing +y.
fn turntable(b: &mut Builder, uv: UvRect) {
    let at = |fu: f32, fv: f32| [uv.u0 + (uv.u1 - uv.u0) * fu, uv.v0 + (uv.v1 - uv.v0) * fv];
    let base = b.vertices.len() as u32;
    let vertex = |pos: [f32; 3], uv: [f32; 2]| Vertex {
        pos,
        normal: [0.0, 1.0, 0.0],
        uv,
        emissive: EMIT_TURNTABLE,
        segment: 0,
    };
    b.vertices.push(vertex([0.0, 0.0, 0.0], at(0.5, 0.5)));
    for i in 0..TURNTABLE_SEGMENTS {
        let (s, c) = (TAU * i as f32 / TURNTABLE_SEGMENTS as f32).sin_cos();
        // Seen from above, image up is -z.
        b.vertices
            .push(vertex([c, 0.0, -s], at(0.5 + 0.5 * c, 0.5 - 0.5 * s)));
    }
    for i in 0..TURNTABLE_SEGMENTS {
        let next = (i + 1) % TURNTABLE_SEGMENTS;
        b.indices.extend([base, base + 1 + i, base + 1 + next]);
    }
}

/// The model matrices of one flyer: the body, the door and the turntable.
pub fn matrices(flyer: &FlyerInstance) -> [Mat4; 3] {
    let body = Mat4::translation(flyer.position)
        * flyer.orientation.normalized().to_mat4()
        * Mat4::uniform_scale(flyer.scale);
    // Negative about y swings the free edge toward +z (out of the front).
    let door = body * Mat4::translation(hinge()) * Quat::rotation_y(-flyer.door_angle).to_mat4();
    let (center, radius) = turntable_place();
    let turntable = body
        * Mat4::translation(center)
        * Quat::rotation_y(TAU * flyer.turntable_phase).to_mat4()
        * Mat4::uniform_scale(radius);
    [body, door, turntable]
}

/// Bytes of the model matrices of one flyer in the instance buffer.
pub const INSTANCE_BYTES: u64 = 3 * 64;

/// Appends the model matrices of `flyer`, column by column.
pub fn write_instance(flyer: &FlyerInstance, out: &mut Vec<u8>) {
    for m in matrices(flyer) {
        for column in m.0 {
            for v in column {
                out.extend_from_slice(&v.to_ne_bytes());
            }
        }
    }
}

/// The order in which to draw the door glass: back to front (the farthest
/// door first), so that the translucent windows blend correctly.
pub fn glass_order(flyers: &[FlyerInstance]) -> Vec<usize> {
    let depth = |f: &FlyerInstance| matrices(f)[1].transform(Vec3::new(0.0, 0.0, 0.0))[2];
    let mut order: Vec<usize> = (0..flyers.len()).collect();
    order.sort_by(|&a, &b| depth(&flyers[a]).total_cmp(&depth(&flyers[b])));
    order
}

#[cfg(test)]
mod tests {
    use flying_microwave_core::flight::{BODY_SIZE, BOUNDING_RADIUS, door_angle};

    use super::*;

    fn unit_uvs() -> Vec<UvRect> {
        vec![
            UvRect {
                u0: 0.0,
                v0: 0.0,
                u1: 1.0,
                v1: 1.0,
            };
            FACE_COUNT
        ]
    }

    fn flyer(door_angle: f32) -> FlyerInstance {
        FlyerInstance {
            position: Vec3::new(0.0, 0.0, 0.0),
            orientation: Quat::rotation_y(0.0),
            scale: 1.0,
            door_angle,
            turntable_phase: 0.0,
            color: 0,
        }
    }

    fn point(m: &Mat4, v: &Vertex) -> Vec3 {
        let [x, y, z, _] = m.transform(Vec3::new(v.pos[0], v.pos[1], v.pos[2]));
        Vec3::new(x, y, z)
    }

    fn vertices_of<'a>(m: &'a Meshes, range: &Range<u32>) -> impl Iterator<Item = &'a Vertex> {
        m.indices[range.start as usize..range.end as usize]
            .iter()
            .map(|&i| &m.vertices[i as usize])
    }

    #[test]
    fn faces_are_listed_once_in_index_order() {
        let all = Face::all();
        assert_eq!(all.len(), FACE_COUNT);
        for (i, face) in all.iter().enumerate() {
            assert_eq!(face.index(), i, "{face:?}");
        }
    }

    #[test]
    fn body_matches_the_flight_body_size() {
        let e = half_extents().scale(2.0);
        assert!((e - BODY_SIZE).length() < 1e-6, "{e:?} != {BODY_SIZE:?}");
    }

    /// The width and height of the viewBox of an SVG document.
    fn view_box(svg: &str) -> (f32, f32) {
        let start = svg.find("viewBox=\"").expect("a viewBox") + 9;
        let end = start + svg[start..].find('"').expect("closing quote");
        let numbers: Vec<f32> = svg[start..end]
            .split(' ')
            .map(|n| n.parse().expect("a number"))
            .collect();
        assert_eq!(numbers[..2], [0.0, 0.0]);
        (numbers[2], numbers[3])
    }

    #[test]
    fn faces_lie_on_their_sheets_without_touching() {
        let all = Face::all();
        for (i, a) in all.iter().enumerate() {
            let img = a.image();
            let [x, y, w, h] = img.rect;
            let (sw, sh) = view_box(img.sheet.svg());
            assert!(x >= 0.0 && y >= 0.0 && x + w <= sw && y + h <= sh, "{a:?}");
            assert!(img.width > 0 && img.height > 0, "{a:?}");
            // Each image keeps the proportions of its rectangle.
            let ratio = (img.width as f32 / img.height as f32) / (w / h);
            assert!((ratio - 1.0).abs() < 0.01, "{a:?}: {ratio}");
            for b in &all[i + 1..] {
                let other = b.image();
                if other.sheet != img.sheet {
                    continue;
                }
                let [ox, oy, ow, oh] = other.rect;
                let apart = x + w < ox || ox + ow < x || y + h < oy || oy + oh < y;
                assert!(apart, "{a:?} and {b:?} touch");
            }
        }
    }

    #[test]
    fn sheets_need_no_fonts_and_load_nothing() {
        // resvg is built without text and image support (crates/app/Cargo.toml).
        for sheet in sheets() {
            let svg = sheet.svg();
            assert!(svg.contains("<svg"), "{sheet:?} is not an SVG");
            for banned in ["<text", "<image", "href", "<script", "<foreignObject"] {
                assert!(!svg.contains(banned), "{banned} in {sheet:?}");
            }
        }
    }

    #[test]
    fn body_sheets_differ_only_in_color() {
        let strip = |svg: &str| {
            svg.lines()
                .filter(|l| !l.contains("stop-color") && !l.contains("<!--"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        for svg in &BODY_SVGS[1..] {
            assert_eq!(strip(svg), strip(BODY_SVGS[0]));
            assert_ne!(*svg, BODY_SVGS[0]);
        }
    }

    #[test]
    fn every_triangle_faces_its_normal() {
        let m = build(&unit_uvs());
        for tri in m.indices.chunks(3) {
            let p = |i: u32| {
                let v = m.vertices[i as usize].pos;
                Vec3::new(v[0], v[1], v[2])
            };
            let n = m.vertices[tri[0] as usize].normal;
            let face = (p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0]));
            assert!(
                face.dot(Vec3::new(n[0], n[1], n[2])) > 0.0,
                "triangle {tri:?}"
            );
        }
    }

    #[test]
    fn body_mesh_has_no_t_junctions() {
        // Review of plan 0005, task 6: a vertex in the middle of another
        // triangle's edge can leave one-pixel cracks when rasterized, which
        // show the background through the body without multisampling.
        let m = build(&unit_uvs());
        let range = &m.body[0];
        // The lit LED segments are decals over the LED quad, not part of the
        // closed body, so they are left out.
        let tris: Vec<[Vec3; 3]> = m.indices[range.start as usize..range.end as usize]
            .chunks(3)
            .filter(|t| m.vertices[t[0] as usize].segment == 0)
            .map(|t| {
                t.iter()
                    .map(|&i| {
                        let p = m.vertices[i as usize].pos;
                        Vec3::new(p[0], p[1], p[2])
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("three corners")
            })
            .collect();
        let points: Vec<Vec3> = tris.iter().flatten().copied().collect();
        // Corners that meet are the same values, bit for bit.
        for (i, &a) in points.iter().enumerate() {
            for &c in &points[i + 1..] {
                if (a - c).length() < 1e-5 {
                    assert_eq!(a, c, "corners that nearly meet");
                }
            }
        }
        for t in &tris {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let ab = b - a;
                for &p in &points {
                    let s = (p - a).dot(ab) / ab.dot(ab);
                    let off = (a + ab.scale(s) - p).length();
                    let inside = s > 1e-4 && s < 1.0 - 1e-4;
                    assert!(!(inside && off < 1e-5), "{p:?} on the edge {a:?}-{b:?}");
                }
            }
        }
    }

    #[test]
    fn led_segments_light_every_bit_and_match_the_unlit_ones() {
        let segments = led_segments();
        for bit in 0..=clock::PM_BIT {
            assert!(segments.iter().any(|&(_, b)| b == bit), "bit {bit}");
        }
        // The unlit segments of the image are the same rectangles.
        let unlit: Vec<[f32; 4]> = LED_SVG
            .lines()
            .filter(|l| l.contains("opacity=\"0.08\""))
            .map(|l| {
                let attr = |name: &str| {
                    let start = l.find(&format!(" {name}=\"")).expect(name) + name.len() + 3;
                    let end = start + l[start..].find('"').expect("quote");
                    l[start..end].parse::<f32>().expect("a number")
                };
                [attr("x"), attr("y"), attr("width"), attr("height")]
            })
            .collect();
        assert_eq!(unlit.len(), segments.len());
        for (rect, bit) in &segments {
            assert!(
                unlit.contains(rect),
                "bit {bit}: {rect:?} is not in the image"
            );
        }
    }

    #[test]
    fn lit_segments_lie_on_the_led_window() {
        let m = build(&unit_uvs());
        let e = half_extents();
        let [lx, ly, lw, lh] = LED_RECT;
        let (x0, y1) = front_point(lx, ly);
        let (x1, y0) = front_point(lx + lw, ly + lh);
        let mut seen = 0;
        for v in vertices_of(&m, &m.body[0]).filter(|v| v.segment != 0) {
            let [x, y, z] = v.pos;
            assert!((x0..=x1).contains(&x) && (y0..=y1).contains(&y), "{v:?}");
            assert!(z > e.z + LED_OFFSET, "in front of the LED quad: {v:?}");
            assert_eq!(v.emissive, EMIT_LED);
            assert!(v.segment <= clock::PM_BIT + 1);
            seen += 1;
        }
        assert_eq!(seen, 6 * led_segments().len());
    }

    #[test]
    fn meshes_cover_every_index_once() {
        let m = build(&unit_uvs());
        let mut ranges: Vec<Range<u32>> = m.body.to_vec();
        ranges.push(m.door.clone());
        ranges.push(m.turntable.clone());
        ranges.sort_by_key(|r| r.start);
        let mut next = 0;
        for r in ranges {
            assert_eq!(r.start, next);
            assert!(r.end > r.start);
            next = r.end;
        }
        assert_eq!(next as usize, m.indices.len());
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
    }

    #[test]
    fn body_meshes_use_the_faces_of_their_color() {
        let uvs: Vec<UvRect> = (0..FACE_COUNT)
            .map(|i| UvRect {
                u0: i as f32,
                v0: 0.0,
                u1: i as f32 + 1.0,
                v1: 1.0,
            })
            .collect();
        let m = build(&uvs);
        for color in 0..BODY_COLORS {
            let own = [
                Face::Front(color),
                Face::Back(color),
                Face::Side(color),
                Face::Plain(color),
                Face::Led,
                Face::LedSegment,
                Face::Cavity,
            ]
            .map(|f| f.index() as f32);
            for v in vertices_of(&m, &m.body[usize::from(color)]) {
                // Cell i spans u from i to i + 1.
                assert!(
                    own.iter().any(|&c| (c..=c + 1.0).contains(&v.uv[0])),
                    "color {color}: uv {:?}",
                    v.uv
                );
            }
        }
    }

    #[test]
    fn whole_microwave_stays_within_the_bounding_radius() {
        // core::flight treats a flyer as gone once a sphere of this radius
        // is outside the view, so no part may reach beyond it (plan 0005,
        // D-12): the door at every angle up to the widest, its thickness
        // and the LED quad included.
        let m = build(&unit_uvs());
        let mut farthest = 0.0f32;
        // The widest opening is where core::flight puts it (flap phase 0.5),
        // so the test follows a change of that angle.
        let widest = door_angle(0.5);
        assert!(widest >= door_angle(0.25) && widest > 1.0, "{widest}");
        for step in 0..=70 {
            let f = flyer(widest * step as f32 / 70.0);
            let [body, door, turntable] = matrices(&f);
            let parts = m
                .body
                .iter()
                .map(|r| (body, r))
                .chain([(door, &m.door), (turntable, &m.turntable)]);
            for (matrix, range) in parts {
                for v in vertices_of(&m, range) {
                    farthest = farthest.max(point(&matrix, v).length());
                }
            }
        }
        assert!(farthest <= BOUNDING_RADIUS, "{farthest}");
        // The open door reaches near the radius, so the radius is not loose.
        assert!(farthest > 1.6, "{farthest}");
    }

    #[test]
    fn bounding_radius_scales_with_the_flyer() {
        let m = build(&unit_uvs());
        let mut f = flyer(door_angle(0.5));
        f.scale = 2.5;
        f.orientation = Quat::from_axis_angle(Vec3::new(1.0, 2.0, 3.0).normalized(), 1.0);
        f.position = Vec3::new(3.0, -1.0, -15.0);
        let [_, door, _] = matrices(&f);
        for v in vertices_of(&m, &m.door) {
            let d = (point(&door, v) - f.position).length();
            assert!(d <= BOUNDING_RADIUS * f.scale, "{d}");
        }
    }

    #[test]
    fn closed_door_covers_the_front_and_hinge_stays_put() {
        let m = build(&unit_uvs());
        let e = half_extents();
        let closed = matrices(&flyer(0.0))[1];
        for v in vertices_of(&m, &m.door) {
            let p = point(&closed, v);
            // In front of the front face, within its outline.
            assert!(p.z >= e.z - 1e-6 && p.z <= e.z + DOOR_THICKNESS + 1e-6);
            assert!(p.x.abs() <= e.x && p.y.abs() <= e.y, "{p:?}");
        }
        let open = matrices(&flyer(1.0))[1];
        let on_hinge = Vec3::new(0.0, 0.3, 0.0);
        let [a, b] = [closed, open].map(|m| {
            let [x, y, z, _] = m.transform(on_hinge);
            Vec3::new(x, y, z)
        });
        assert!((a - b).length() < 1e-6);
        // Opening swings the free edge out of the front (+z).
        let free = Vec3::new(DOOR_RECT[2] / SVG_PER_UNIT, 0.0, 0.0);
        let [x, _, z, _] = open.transform(free);
        assert!(z > e.z + 0.5 && x < e.x, "{x}, {z}");
    }

    #[test]
    fn turntable_turns_inside_the_cavity() {
        let m = build(&unit_uvs());
        let (x0, y0, x1, y1, zf, zb) = cavity_box();
        for phase in [0.0, 0.3, 0.77] {
            let mut f = flyer(0.0);
            f.turntable_phase = phase;
            let tt = matrices(&f)[2];
            for v in vertices_of(&m, &m.turntable) {
                let p = point(&tt, v);
                assert!((x0..=x1).contains(&p.x), "{p:?}");
                assert!((y0..=y1).contains(&p.y), "{p:?}");
                assert!((zb..=zf).contains(&p.z), "{p:?}");
            }
        }
    }

    #[test]
    fn lit_brightness_stays_within_the_g3_range() {
        for i in 0..=100 {
            let cos = -1.0 + 2.0 * i as f32 / 100.0;
            let k = AMBIENT + DIFFUSE * (0.5 + 0.5 * cos);
            assert!((0.55 - 1e-6..=1.0 + 1e-6).contains(&k), "{k}");
        }
        assert!((light_dir().length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn instance_is_three_matrices_column_by_column() {
        let mut f = flyer(0.5);
        f.position = Vec3::new(1.0, 2.0, -12.0);
        let mut bytes = Vec::new();
        write_instance(&f, &mut bytes);
        assert_eq!(bytes.len() as u64, INSTANCE_BYTES);
        let (chunks, rest) = bytes.as_chunks::<4>();
        assert!(rest.is_empty());
        let floats: Vec<f32> = chunks.iter().map(|&b| f32::from_ne_bytes(b)).collect();
        // The translation of the body matrix is its fourth column.
        assert_eq!(floats[12..16], [1.0, 2.0, -12.0, 1.0]);
        let [_, door, _] = matrices(&f);
        assert_eq!(floats[16..20], door.0[0]);
    }

    #[test]
    fn glass_is_drawn_from_the_farthest_door() {
        let at = |z: f32| {
            let mut f = flyer(0.0);
            f.position = Vec3::new(0.0, 0.0, z);
            f
        };
        let flyers = [at(-12.0), at(-25.0), at(-10.0), at(-18.0)];
        assert_eq!(glass_order(&flyers), vec![1, 3, 0, 2]);
        assert!(glass_order(&[]).is_empty());
    }
}
