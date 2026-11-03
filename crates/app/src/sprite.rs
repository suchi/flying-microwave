//! The microwave sprite (ADR-0008): where the body and the flapping door are
//! drawn, how their textures are packed into one atlas and, on Windows,
//! rasterizing the SVG assets with resvg.
//!
//! Everything but the rasterization has no platform dependency and is tested
//! on every host. The assets and their coordinates are described in
//! `assets/sprites/README.md`.

use flying_microwave_core::sim::{BODY_COLORS, SpriteInstance};

/// Background color (8-bit RGB). Dark, as guideline G-5 asks.
pub const BACKGROUND_RGB: [u8; 3] = [6, 8, 18];

/// The body sprites, in the order of [`SpriteInstance::color`].
pub const BODY_SVGS: [&str; BODY_COLORS as usize] = [
    include_str!("../../../assets/sprites/microwave-body-silver.svg"),
    include_str!("../../../assets/sprites/microwave-body-graphite.svg"),
    include_str!("../../../assets/sprites/microwave-body-white.svg"),
    include_str!("../../../assets/sprites/microwave-body-mint.svg"),
];
/// The door sprite.
pub const DOOR_SVG: &str = include_str!("../../../assets/sprites/microwave-door.svg");

/// Side of the square body sprite in SVG units.
const BODY_SIDE: f32 = 320.0;
/// The door in body units: left, top, width and height.
const DOOR: [f32; 4] = [24.0, 100.0, 156.0, 140.0];
/// Where a point moves on the screen per unit it comes toward the viewer.
///
/// The body is drawn in an oblique projection: its back, 210 units deep, is
/// drawn (52, -36) units from the front, so the depth toward the viewer goes
/// to the lower left.
const TOWARD: [f32; 2] = [-52.0 / 210.0, 36.0 / 210.0];
/// Widest door opening in degrees.
///
/// Why not more: past about 76 degrees the outside of the door turns away
/// from the viewer in this projection, and the inside is not drawn.
pub const MAX_DOOR_ANGLE_DEG: f32 = 70.0;
/// Tilt of every flyer in degrees. Negative turns counterclockwise on the
/// screen, so the nose (left) points down along the flight.
const TILT_DEG: f32 = -8.0;
/// Largest body texture side in pixels, to bound the atlas memory.
const MAX_BODY_PX: u32 = 1536;
/// Smallest body texture side in pixels.
const MIN_BODY_PX: u32 = 8;
/// Transparent pixels between atlas cells, and around the door inside its cell.
pub const PAD: u32 = 2;

/// A parallelogram: `origin + s * u + t * v` for `s` and `t` in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub origin: [f32; 2],
    pub u: [f32; 2],
    pub v: [f32; 2],
}

/// Door opening angle in radians for a flap phase in `[0, 1)`.
///
/// The door is closed at phase 0, opens smoothly to [`MAX_DOOR_ANGLE_DEG`]
/// at phase 0.5 and closes again.
pub fn door_angle(flap: f32) -> f32 {
    let open = (1.0 - (std::f32::consts::TAU * flap).cos()) / 2.0;
    MAX_DOOR_ANGLE_DEG.to_radians() * open
}

/// The square of the body sprite, in body units.
pub fn body_quad() -> Quad {
    Quad {
        origin: [0.0, 0.0],
        u: [BODY_SIDE, 0.0],
        v: [0.0, BODY_SIDE],
    }
}

/// The door opened by `angle` radians about its left (hinge) edge, in body units.
///
/// The hinge edge stays in place and the free edge swings toward the viewer,
/// so the door is drawn as a parallelogram.
pub fn door_quad(angle: f32) -> Quad {
    let [left, top, width, height] = DOOR;
    let (sin, cos) = angle.sin_cos();
    Quad {
        origin: [left, top],
        // The free edge goes `width * cos` to the right and `width * sin`
        // toward the viewer.
        u: [width * (cos + sin * TOWARD[0]), width * sin * TOWARD[1]],
        v: [0.0, height],
    }
}

impl Quad {
    /// Grows the quad by `pad_u` and `pad_v` of its size on each side.
    fn grown(self, pad_u: f32, pad_v: f32) -> Self {
        Self {
            origin: [
                self.origin[0] - self.u[0] * pad_u - self.v[0] * pad_v,
                self.origin[1] - self.u[1] * pad_u - self.v[1] * pad_v,
            ],
            u: self.u.map(|c| c * (1.0 + 2.0 * pad_u)),
            v: self.v.map(|c| c * (1.0 + 2.0 * pad_v)),
        }
    }
}

/// Where a sprite lies on a surface of `height` pixels: its center, its
/// scale (pixels per body unit) and its tilt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    center: [f32; 2],
    scale: f32,
    cos: f32,
    sin: f32,
}

impl Placement {
    /// Places `sprite` on a surface `height` pixels high.
    pub fn new(sprite: &SpriteInstance, height: u32) -> Self {
        let height = height as f32;
        let (sin, cos) = TILT_DEG.to_radians().sin_cos();
        Self {
            center: [sprite.x * height, sprite.y * height],
            scale: sprite.size * height / BODY_SIDE,
            cos,
            sin,
        }
    }

    /// Scales and tilts a vector in body units into pixels.
    fn vector(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        let (x, y) = (x * self.scale, y * self.scale);
        // Same rotation as SVG's rotate(): y grows downward.
        [x * self.cos - y * self.sin, x * self.sin + y * self.cos]
    }

    /// Maps a quad in body units to surface pixels.
    pub fn on_surface(&self, quad: Quad) -> Quad {
        let half = BODY_SIDE / 2.0;
        let [dx, dy] = self.vector([quad.origin[0] - half, quad.origin[1] - half]);
        Quad {
            origin: [self.center[0] + dx, self.center[1] + dy],
            u: self.vector(quad.u),
            v: self.vector(quad.v),
        }
    }
}

/// Maps a quad in surface pixels to clip space (x right, y up, both -1..1).
pub fn to_clip(quad: Quad, width: u32, height: u32) -> Quad {
    let (sx, sy) = (2.0 / width.max(1) as f32, -2.0 / height.max(1) as f32);
    let vector = |[x, y]: [f32; 2]| [x * sx, y * sy];
    let [x, y] = vector(quad.origin);
    Quad {
        origin: [x - 1.0, y + 1.0],
        u: vector(quad.u),
        v: vector(quad.v),
    }
}

/// A rectangle of the atlas in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Where each texture lies in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasLayout {
    pub width: u32,
    pub height: u32,
    /// One square cell per body color.
    pub bodies: [Cell; BODY_COLORS as usize],
    /// The door, with [`PAD`] transparent pixels on every side inside the cell.
    pub door: Cell,
}

/// Lays out the atlas for a sprite of `side_px` pixels, within `max_dim`
/// pixels (the largest texture the device takes) on each side.
pub fn atlas_layout(side_px: f32, max_dim: u32) -> AtlasLayout {
    // One row: the bodies, then the door (156/320 of a body wide) and the gaps.
    // `as` saturates, so NaN and negative sizes become 0 and then the minimum.
    let fits = (max_dim.saturating_sub(8 * PAD) as f32 / 4.5) as u32;
    let body = (side_px.round() as u32)
        .min(MAX_BODY_PX)
        .min(fits)
        .max(MIN_BODY_PX);
    let door_size = |units: f32| ((units / BODY_SIDE * body as f32).round() as u32).max(1);
    let bodies = std::array::from_fn(|i| Cell {
        x: PAD + i as u32 * (body + PAD),
        y: PAD,
        w: body,
        h: body,
    });
    let door = Cell {
        x: PAD + u32::from(BODY_COLORS) * (body + PAD),
        y: PAD,
        w: door_size(DOOR[2]) + 2 * PAD,
        h: door_size(DOOR[3]) + 2 * PAD,
    };
    AtlasLayout {
        width: door.x + door.w + PAD,
        height: body.max(door.h) + 2 * PAD,
        bodies,
        door,
    }
}

impl AtlasLayout {
    /// Texture coordinates of `cell`: its top-left corner and its size.
    fn uv(&self, cell: Cell) -> ([f32; 2], [f32; 2]) {
        let (w, h) = (self.width as f32, self.height as f32);
        (
            [cell.x as f32 / w, cell.y as f32 / h],
            [cell.w as f32 / w, cell.h as f32 / h],
        )
    }
}

/// One textured quad as the vertex shader takes it, in clip space and
/// texture coordinates (0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuQuad {
    pub quad: Quad,
    pub uv_min: [f32; 2],
    pub uv_size: [f32; 2],
}

/// Bytes of one [`GpuQuad`] in the instance buffer (ten `f32`).
pub const GPU_QUAD_BYTES: u64 = 40;

impl GpuQuad {
    /// Appends the quad to an instance buffer, in the order of the shader's locations.
    pub fn write_to(&self, out: &mut Vec<u8>) {
        let q = &self.quad;
        for pair in [q.origin, q.u, q.v, self.uv_min, self.uv_size] {
            for c in pair {
                out.extend_from_slice(&c.to_ne_bytes());
            }
        }
    }
}

/// The quads that draw `sprite` on a `width` x `height` surface: the body,
/// then the door on top.
pub fn sprite_quads(
    sprite: &SpriteInstance,
    width: u32,
    height: u32,
    atlas: &AtlasLayout,
) -> [GpuQuad; 2] {
    let placement = Placement::new(sprite, height);
    let quad = |q: Quad, cell: Cell| {
        let (uv_min, uv_size) = atlas.uv(cell);
        GpuQuad {
            quad: to_clip(placement.on_surface(q), width, height),
            uv_min,
            uv_size,
        }
    };
    let body = atlas.bodies[usize::from(sprite.color) % atlas.bodies.len()];
    let door = atlas.door;
    // The door texture has PAD transparent pixels on each side; grow the quad
    // to match, so the door edges are smoothed by the texture filter.
    let pad = PAD as f32;
    let door_quad = door_quad(door_angle(sprite.flap)).grown(
        pad / (door.w as f32 - 2.0 * pad),
        pad / (door.h as f32 - 2.0 * pad),
    );
    [quad(body_quad(), body), quad(door_quad, door)]
}

/// Rasterizes every asset into an atlas of `layout`: premultiplied RGBA,
/// 8 bits per channel, rows of `layout.width` pixels, in the sRGB encoding.
#[cfg(windows)]
pub fn rasterize(layout: &AtlasLayout) -> Result<Vec<u8>, String> {
    use resvg::tiny_skia::{Pixmap, Transform};
    use resvg::usvg;

    let options = usvg::Options {
        // Why: the assets reference no images; refuse any, so that an asset
        // can never make the screensaver read a file.
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let mut atlas = Pixmap::new(layout.width, layout.height)
        .ok_or_else(|| format!("atlas of {}x{} pixels", layout.width, layout.height))?;
    let door_inner = Cell {
        x: layout.door.x + PAD,
        y: layout.door.y + PAD,
        w: layout.door.w - 2 * PAD,
        h: layout.door.h - 2 * PAD,
    };
    let jobs = BODY_SVGS
        .iter()
        .zip(layout.bodies)
        .chain([(&DOOR_SVG, door_inner)]);
    for (index, (svg, cell)) in jobs.enumerate() {
        let tree = usvg::Tree::from_str(svg, &options)
            .map_err(|e| format!("sprite {index} could not be parsed: {e}"))?;
        let size = tree.size();
        // Each asset fills its cell; nothing is drawn outside its own canvas.
        let transform = Transform::from_row(
            cell.w as f32 / size.width(),
            0.0,
            0.0,
            cell.h as f32 / size.height(),
            cell.x as f32,
            cell.y as f32,
        );
        resvg::render(&tree, transform, &mut atlas.as_mut());
    }
    Ok(atlas.take())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-4;

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < EPS && (a[1] - b[1]).abs() < EPS
    }

    /// The z component of u x v: positive when the quad keeps the closed door's orientation.
    fn cross(q: &Quad) -> f32 {
        q.u[0] * q.v[1] - q.u[1] * q.v[0]
    }

    fn sprite(x: f32, y: f32, flap: f32, color: u8) -> SpriteInstance {
        SpriteInstance {
            x,
            y,
            size: 0.12,
            flap,
            color,
        }
    }

    #[test]
    fn door_is_closed_at_phase_zero_and_widest_at_half() {
        assert_eq!(door_angle(0.0), 0.0);
        let max = MAX_DOOR_ANGLE_DEG.to_radians();
        assert!((door_angle(0.5) - max).abs() < EPS);
        assert!((door_angle(0.25) - max / 2.0).abs() < EPS);
        for i in 0..100 {
            let a = door_angle(i as f32 / 100.0);
            assert!((0.0..=max + EPS).contains(&a), "angle {a}");
        }
    }

    #[test]
    fn door_angle_is_continuous_across_the_wrap() {
        assert!(door_angle(0.999).abs() < 1e-3);
        assert!((door_angle(0.4) - door_angle(0.6)).abs() < EPS);
    }

    #[test]
    fn closed_door_is_the_door_rectangle() {
        let q = door_quad(0.0);
        assert!(close(q.origin, [24.0, 100.0]));
        assert!(close(q.u, [156.0, 0.0]));
        assert!(close(q.v, [0.0, 140.0]));
    }

    #[test]
    fn hinge_edge_stays_in_place() {
        for deg in [10.0_f32, 35.0, 55.0, MAX_DOOR_ANGLE_DEG] {
            let q = door_quad(deg.to_radians());
            assert!(close(q.origin, [24.0, 100.0]), "{deg}: {q:?}");
            assert!(close(q.v, [0.0, 140.0]), "{deg}: {q:?}");
        }
    }

    #[test]
    fn open_door_matches_the_approved_mockup() {
        // The mockup (microwave.py, door()) moves the free edge to
        // x = x0 + w cos t + w sin t * (-52 / 210) and y + w sin t * (36 / 210).
        let t = 55.0_f32.to_radians();
        let w = 156.0;
        let q = door_quad(t);
        let free_top = [q.origin[0] + q.u[0], q.origin[1] + q.u[1]];
        let want = [
            24.0 + w * t.cos() - w * t.sin() * 52.0 / 210.0,
            100.0 + w * t.sin() * 36.0 / 210.0,
        ];
        assert!(close(free_top, want), "{free_top:?} != {want:?}");
    }

    #[test]
    fn widest_door_still_shows_its_outside() {
        let q = door_quad(MAX_DOOR_ANGLE_DEG.to_radians());
        assert!(
            q.u[0] > 0.0,
            "the free edge stays right of the hinge: {q:?}"
        );
        assert!(cross(&q) > 0.0);
    }

    #[test]
    fn body_quad_is_the_sprite_square() {
        let q = body_quad();
        assert_eq!(q.origin, [0.0, 0.0]);
        assert_eq!(q.u, [320.0, 0.0]);
        assert_eq!(q.v, [0.0, 320.0]);
    }

    #[test]
    fn placement_centers_the_body_and_scales_it_to_the_sprite_size() {
        let p = Placement::new(&sprite(0.5, 0.25, 0.0, 0), 1000);
        let q = p.on_surface(body_quad());
        let center = [
            q.origin[0] + (q.u[0] + q.v[0]) / 2.0,
            q.origin[1] + (q.u[1] + q.v[1]) / 2.0,
        ];
        assert!(close(center, [500.0, 250.0]), "{center:?}");
        // 0.12 screen heights of 1000 pixels.
        assert!((q.u[0].hypot(q.u[1]) - 120.0).abs() < 1e-3);
        assert!((q.v[0].hypot(q.v[1]) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn placement_tilts_the_nose_down() {
        let p = Placement::new(&sprite(0.5, 0.5, 0.0, 0), 1000);
        let q = p.on_surface(body_quad());
        // The top edge rises to the right by 8 degrees (y grows downward).
        let deg = (-q.u[1]).atan2(q.u[0]).to_degrees();
        assert!((deg - 8.0).abs() < 1e-3, "tilt {deg}");
        assert!(cross(&q) > 0.0, "not mirrored");
    }

    #[test]
    fn tilted_sprite_stays_inside_its_square() {
        // The simulation treats a flyer as a square of side `size`: drawn
        // pixels must not leave it, or flyers would vanish while visible.
        // Corners of the drawing (body units): body, feet, widest door.
        let door = door_quad(MAX_DOOR_ANGLE_DEG.to_radians());
        let door_corners = [
            door.origin,
            [door.origin[0] + door.u[0], door.origin[1] + door.u[1]],
            [door.origin[0] + door.v[0], door.origin[1] + door.v[1]],
            [
                door.origin[0] + door.u[0] + door.v[0],
                door.origin[1] + door.u[1] + door.v[1],
            ],
        ];
        let body_corners = [[14.0, 54.0], [306.0, 54.0], [306.0, 256.0], [14.0, 256.0]];
        let p = Placement::new(&sprite(0.5, 0.5, 0.0, 0), 1000);
        let unit = Quad {
            origin: [0.0, 0.0],
            u: [1.0, 0.0],
            v: [0.0, 1.0],
        };
        for corner in body_corners.iter().chain(&door_corners) {
            let q = p.on_surface(Quad {
                origin: *corner,
                ..unit
            });
            for (c, center) in q.origin.iter().zip([500.0, 500.0]) {
                assert!(
                    (c - center).abs() <= 60.0,
                    "{corner:?} drawn at {:?}",
                    q.origin
                );
            }
        }
    }

    #[test]
    fn clip_space_spans_the_surface() {
        let q = Quad {
            origin: [0.0, 0.0],
            u: [800.0, 0.0],
            v: [0.0, 600.0],
        };
        let c = to_clip(q, 800, 600);
        assert!(close(c.origin, [-1.0, 1.0]));
        assert!(close(c.u, [2.0, 0.0]));
        assert!(close(c.v, [0.0, -2.0]));
    }

    fn overlaps(a: &Cell, b: &Cell) -> bool {
        a.x < b.x + b.w + PAD
            && b.x < a.x + a.w + PAD
            && a.y < b.y + b.h + PAD
            && b.y < a.y + a.h + PAD
    }

    fn assert_valid(layout: &AtlasLayout, max_dim: u32) {
        assert!(
            layout.width <= max_dim && layout.height <= max_dim,
            "{layout:?}"
        );
        let cells: Vec<&Cell> = layout.bodies.iter().chain([&layout.door]).collect();
        for (i, a) in cells.iter().enumerate() {
            assert!(a.w > 0 && a.h > 0, "{a:?}");
            assert!(a.x >= PAD && a.y >= PAD, "{a:?}");
            assert!(a.x + a.w + PAD <= layout.width, "{a:?} in {layout:?}");
            assert!(a.y + a.h + PAD <= layout.height, "{a:?} in {layout:?}");
            for b in &cells[i + 1..] {
                assert!(!overlaps(a, b), "{a:?} and {b:?} touch");
            }
        }
    }

    #[test]
    fn atlas_holds_the_sprite_at_its_drawn_size() {
        // 0.12 of a 4K screen height.
        let layout = atlas_layout(259.2, 16384);
        assert_valid(&layout, 16384);
        for cell in layout.bodies {
            assert_eq!((cell.w, cell.h), (259, 259));
        }
        // The door is 156 x 140 of the 320 units, plus the padding.
        assert_eq!(
            (layout.door.w, layout.door.h),
            (126 + 2 * PAD, 113 + 2 * PAD)
        );
    }

    #[test]
    fn atlas_fits_small_devices_and_extreme_sizes() {
        for (side, max_dim) in [(259.2, 512), (5000.0, 16384), (0.0, 2048), (f32::NAN, 2048)] {
            let layout = atlas_layout(side, max_dim);
            assert_valid(&layout, max_dim);
            let body = layout.bodies[0].w;
            assert!(
                (MIN_BODY_PX..=MAX_BODY_PX).contains(&body),
                "{side}: {body}"
            );
        }
    }

    #[test]
    fn sprite_quads_draw_the_body_color_then_the_door() {
        let layout = atlas_layout(120.0, 16384);
        let (w, h) = (layout.width as f32, layout.height as f32);
        for color in 0..BODY_COLORS {
            let [body, door] = sprite_quads(&sprite(0.5, 0.5, 0.0, color), 1000, 1000, &layout);
            let cell = layout.bodies[usize::from(color)];
            assert!(close(body.uv_min, [cell.x as f32 / w, cell.y as f32 / h]));
            assert!(close(body.uv_size, [cell.w as f32 / w, cell.h as f32 / h]));
            let d = layout.door;
            assert!(close(door.uv_min, [d.x as f32 / w, d.y as f32 / h]));
            assert!(close(door.uv_size, [d.w as f32 / w, d.h as f32 / h]));
            // The body of a sprite at the screen center is centered in clip space.
            let q = body.quad;
            let mid = [
                q.origin[0] + (q.u[0] + q.v[0]) / 2.0,
                q.origin[1] + (q.u[1] + q.v[1]) / 2.0,
            ];
            assert!(close(mid, [0.0, 0.0]), "{mid:?}");
        }
    }

    #[test]
    fn door_quad_covers_its_padding() {
        // The door texture has transparent pixels around it, so its quad is
        // slightly larger than the door and the edges are smoothed.
        let layout = atlas_layout(320.0, 16384);
        let [_, door] = sprite_quads(&sprite(0.5, 0.5, 0.0, 0), 1000, 1000, &layout);
        let p = Placement::new(&sprite(0.5, 0.5, 0.0, 0), 1000);
        let bare = to_clip(p.on_surface(door_quad(0.0)), 1000, 1000);
        let grow = |a: [f32; 2], b: [f32; 2]| a[0].hypot(a[1]) / b[0].hypot(b[1]);
        // A 320-pixel body gives a door texture of 156 x 140 pixels inside the padding.
        let want_u = (156.0 + 2.0 * PAD as f32) / 156.0;
        assert!((grow(door.quad.u, bare.u) - want_u).abs() < 1e-3);
        let want_v = (140.0 + 2.0 * PAD as f32) / 140.0;
        assert!((grow(door.quad.v, bare.v) - want_v).abs() < 1e-3);
    }

    #[test]
    fn unknown_color_falls_back_to_a_body() {
        let layout = atlas_layout(120.0, 16384);
        let [body, _] = sprite_quads(&sprite(0.5, 0.5, 0.0, 200), 1000, 1000, &layout);
        assert!(body.uv_min.iter().all(|c| c.is_finite() && *c < 1.0));
    }

    #[test]
    fn gpu_quad_is_ten_floats_in_shader_order() {
        let q = GpuQuad {
            quad: Quad {
                origin: [1.0, 2.0],
                u: [3.0, 4.0],
                v: [5.0, 6.0],
            },
            uv_min: [7.0, 8.0],
            uv_size: [9.0, 10.0],
        };
        let mut bytes = Vec::new();
        q.write_to(&mut bytes);
        assert_eq!(bytes.len() as u64, GPU_QUAD_BYTES);
        let (chunks, rest) = bytes.as_chunks::<4>();
        assert!(rest.is_empty());
        let floats: Vec<f32> = chunks.iter().map(|&b| f32::from_ne_bytes(b)).collect();
        assert_eq!(floats, (1..=10).map(|i| i as f32).collect::<Vec<_>>());
    }

    #[test]
    fn assets_need_no_fonts_and_load_nothing() {
        // resvg is built without text and image support (crates/app/Cargo.toml).
        for svg in BODY_SVGS.iter().chain([&DOOR_SVG]) {
            assert!(svg.contains("<svg"), "not an SVG");
            for banned in ["<text", "<image", "href", "<script", "<foreignObject"] {
                assert!(!svg.contains(banned), "{banned} in an asset");
            }
        }
    }

    #[test]
    fn body_assets_are_square_and_door_matches_its_rectangle() {
        for svg in BODY_SVGS {
            assert!(svg.contains(r#"viewBox="0 0 320 320""#));
        }
        assert!(DOOR_SVG.contains(r#"viewBox="0 0 156 140""#));
    }

    /// Alpha of the atlas pixel at (x, y).
    #[cfg(windows)]
    fn alpha(pixels: &[u8], layout: &AtlasLayout, x: u32, y: u32) -> u8 {
        pixels[((y * layout.width + x) * 4 + 3) as usize]
    }

    #[cfg(windows)]
    #[test]
    fn rasterized_atlas_has_every_sprite() {
        let layout = atlas_layout(320.0, 16384);
        let pixels = rasterize(&layout).expect("rasterize");
        assert_eq!(pixels.len(), (layout.width * layout.height * 4) as usize);
        for cell in layout.bodies {
            // The front of the body (body units 100, 245: below the door) is opaque.
            assert_eq!(alpha(&pixels, &layout, cell.x + 100, cell.y + 245), 255);
            // The corners of the square are empty, so that tilting needs no clipping.
            assert_eq!(alpha(&pixels, &layout, cell.x + 1, cell.y + 1), 0);
            assert_eq!(
                alpha(&pixels, &layout, cell.x + cell.w - 2, cell.y + cell.h - 2),
                0
            );
        }
        let d = layout.door;
        // Padding is transparent, the frame is opaque and the window lets light through.
        assert_eq!(alpha(&pixels, &layout, d.x, d.y + d.h / 2), 0);
        assert_eq!(alpha(&pixels, &layout, d.x + PAD + 6, d.y + d.h / 2), 255);
        let window = alpha(&pixels, &layout, d.x + d.w / 2, d.y + d.h * 3 / 4);
        assert!((60..230).contains(&window), "window alpha {window}");
        // The bodies differ in color.
        let rgb = |cell: Cell| {
            let i = (((cell.y + 245) * layout.width + cell.x + 100) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };
        let colors: Vec<[u8; 3]> = layout.bodies.iter().map(|&c| rgb(c)).collect();
        for (i, a) in colors.iter().enumerate() {
            for b in &colors[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn rasterizes_the_smallest_atlas() {
        let layout = atlas_layout(0.0, 64);
        let pixels = rasterize(&layout).expect("rasterize");
        assert!(pixels.iter().skip(3).step_by(4).any(|&a| a > 0));
    }
}
