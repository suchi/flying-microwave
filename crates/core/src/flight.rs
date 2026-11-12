//! 3D flight of the microwaves (plan 0005, F-02 to F-04 and F-08; ADR-0026).
//!
//! Coordinates: view space of [`crate::space`]. The camera is at the origin
//! and looks along -z, y is up. The unit is the body height (the body is
//! [`BODY_SIZE`] at `scale` 1.0). Each flyer keeps its distance from the
//! camera (its depth along -z) and flies from the upper right to the lower
//! left (left 2, down 1, as in v0.2.0), at the same speed whatever its depth,
//! so farther flyers look smaller and slower (F-08).
//!
//! The world advances in fixed steps of [`TIME_STEP`] (from [`crate::sim`]),
//! at most [`MAX_ELAPSED`] per call, and rendering interpolates between the
//! previous and the current step, so motion stays smooth at any display
//! refresh rate.
//!
//! The look values below are the starting values set with the spike
//! (ADR-0026, "見た目の値"). They are not decided yet: the maintainer judges
//! them in motion with `v0.3.0-alpha.2` (plan 0005, E-1 and task 8).

use std::f32::consts::TAU;
use std::time::Duration;

use crate::rng::Rng;
use crate::sim::{BODY_COLORS, MAX_ELAPSED, SimParams, TIME_STEP};
use crate::space::{Mat4, Quat, Vec3};

/// Size of the body box (width, height, depth) at `scale` 1.0, in body
/// heights: the proportions of the SVG art (ADR-0026).
pub const BODY_SIZE: Vec3 = Vec3::new(1.5, 1.0, 1.3125);

/// Radius of a sphere around the body center that holds the whole
/// microwave at `scale` 1.0, the open door included.
///
/// Why 1.75: the corners of the body box are 1.115 from the center, and the
/// free edge of the door opened 70 degrees reaches about 1.70 (its thickness
/// included) with the door of the renderer's mesh
/// (`crates/app/src/model.rs`). A flyer only leaves the frustum once this
/// sphere is fully outside, so the mesh must stay within it; the renderer
/// checks that in its tests
/// (`model::tests::whole_microwave_stays_within_the_bounding_radius`).
pub const BOUNDING_RADIUS: f32 = 1.75;

// Why: the values from here to `LEVEL_ROLL_DEG` are the starting values of
// the spike (plan 0005, E-1; ADR-0026), except the speed factor, which keeps
// the v0.2.0 range (plan 0005, task 5). The maintainer decides them in
// motion with `v0.3.0-alpha.2`.

/// Vertical field of view in degrees.
const FOV_Y_DEG: f32 = 40.0;
/// Near and far clip planes in body heights.
const NEAR: f32 = 1.0;
const FAR: f32 = 100.0;
/// Range of the flyer depths (distance from the camera along -z).
const DEPTH_RANGE: (f32, f32) = (10.0, 26.0);
/// Flight speed in body heights per second at `speed` 1.0, for every depth.
const FLIGHT_SPEED: f32 = 2.4;
/// Range of the random per-flyer speed factor (the same as v0.2.0).
const SPEED_FACTOR_RANGE: (f32, f32) = (0.8, 1.2);
/// Range of the random tumbling speed in radians per second. Like the door
/// flaps, the tumbling does not follow `flight.speed`, so how fast the
/// shading of the faces changes depends on this range alone (guideline G-3).
const SPIN_RANGE: (f32, f32) = (0.35, 0.9);
/// Range of the random per-flyer door flap rate (flaps per second); the
/// same as v0.2.0 and well below the three flashes a second of guideline G-3.
const FLAP_RATE_RANGE: (f32, f32) = (1.2, 1.8);
/// Widest door opening in degrees (the same as v0.2.0).
const MAX_DOOR_DEG: f32 = 70.0;
/// Turntable speed in turns per second.
const TURNTABLE_RATE: f32 = 0.1;
/// The orientation of the Level style, in degrees: seen from the upper right
/// like the v0.2.0 sprite, tilted 8 degrees nose (left) down.
const LEVEL_YAW_DEG: f32 = -30.0;
const LEVEL_PITCH_DEG: f32 = 20.0;
const LEVEL_ROLL_DEG: f32 = 8.0;

/// Flight direction (left 2, down 1) normalized: (-2, -1, 0) / sqrt(5).
const DIRECTION: Vec3 = Vec3::new(-0.894_427_2, -0.447_213_6, 0.0);

/// How the flyers are oriented (plan 0005, D-4 and D-10): the setting
/// `flight.style` (D-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
    /// Each flyer tumbles about its own random axis at its own speed (the
    /// default of the product, F-02).
    #[default]
    Tumbling,
    /// Every flyer shows the same sides, as in v0.2.0: one orientation for
    /// all, turned toward the line of sight of each flyer.
    Level,
}

impl Style {
    /// The value in the settings file.
    pub fn name(self) -> &'static str {
        match self {
            Style::Tumbling => "tumbling",
            Style::Level => "level",
        }
    }

    /// The style of a value in the settings file; values are exact.
    pub fn from_name(name: &str) -> Option<Self> {
        [Style::Tumbling, Style::Level]
            .into_iter()
            .find(|s| s.name() == name)
    }
}

/// Where and how to draw one flyer, interpolated for the current frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlyerInstance {
    /// Center of the body in view space.
    pub position: Vec3,
    /// Orientation of the body (a unit quaternion).
    pub orientation: Quat,
    /// Size multiplier of the body: the body is [`BODY_SIZE`] times this.
    pub scale: f32,
    /// Door opening angle in radians, from 0 (closed) to 70 degrees.
    pub door_angle: f32,
    /// Turntable phase in turns, in `[0, 1)`; the angle is `TAU` times this.
    pub turntable_phase: f32,
    /// Body color, below [`BODY_COLORS`].
    pub color: u8,
}

/// The projection matrix of the camera for a screen of the given `aspect`
/// (width / height). An aspect that is not finite or not positive is
/// replaced by 1.0, as in [`World::new`].
pub fn projection(aspect: f32) -> Mat4 {
    Mat4::perspective(FOV_Y_DEG.to_radians(), valid_aspect(aspect), NEAR, FAR)
}

/// Door opening angle in radians for a flap phase in `[0, 1)`.
///
/// The door is closed at phase 0, opens smoothly to 70 degrees at phase 0.5
/// and closes again: the same shape as the door of the v0.2.0 sprite
/// (ADR-0023).
pub fn door_angle(flap: f32) -> f32 {
    let open = (1.0 - (TAU * flap).cos()) / 2.0;
    MAX_DOOR_DEG.to_radians() * open
}

/// `aspect` if it is finite and positive, 1.0 otherwise (for example from a
/// minimized window with zero height).
fn valid_aspect(aspect: f32) -> f32 {
    if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    }
}

/// One flyer: the previous and the current step state, and what stays the
/// same until it respawns. Phases are in turns, in `[0, 1)`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Flyer {
    prev_position: Vec3,
    position: Vec3,
    velocity: Vec3,
    /// Tumbling axis (a unit vector).
    axis: Vec3,
    /// Tumbling speed in turns per second.
    spin_rate: f32,
    prev_spin: f32,
    spin: f32,
    /// Orientation at spin phase 0.
    initial: Quat,
    prev_flap: f32,
    flap: f32,
    /// Flaps per second.
    flap_rate: f32,
    prev_turntable: f32,
    turntable: f32,
    color: u8,
}

/// The simulated world: a set of flyers that cross the view.
#[derive(Debug, Clone)]
pub struct World {
    rng: Rng,
    style: Style,
    /// Half width and half height of the view at depth 1.
    tan_x: f32,
    tan_y: f32,
    /// How far past the left and the bottom edge of the view (along x and
    /// y) the center of a flyer can be while its bounding sphere still
    /// touches the view. See [`World::half_extents`].
    margin_x: f32,
    margin_y: f32,
    scale: f32,
    /// Flight speed before the per-flyer factor.
    speed: f32,
    flyers: Vec<Flyer>,
    accumulator: Duration,
}

impl World {
    /// Creates a world of `params.count` flyers spread over the view.
    ///
    /// `aspect` is the screen width divided by its height; a value that is
    /// not finite or not positive is replaced by 1.0. `params.speed` and
    /// `params.scale` multiply the flight speed and the size; callers pass
    /// finite, positive values (`core::config` validates them).
    pub fn new(seed: u64, aspect: f32, params: SimParams, style: Style) -> Self {
        let tan_y = (FOV_Y_DEG.to_radians() / 2.0).tan();
        let tan_x = tan_y * valid_aspect(aspect);
        // The bounding sphere is fully outside the left plane of the view
        // once its center is farther than the radius from that plane. At
        // depth d the plane is at x = -d * tan_x, and the radius measured
        // along x is radius / cos = radius * sqrt(1 + tan_x^2). The same
        // holds for the bottom plane with tan_y.
        let radius = BOUNDING_RADIUS * params.scale;
        let mut world = Self {
            rng: Rng::new(seed),
            style,
            tan_x,
            tan_y,
            margin_x: radius * (1.0 + tan_x * tan_x).sqrt(),
            margin_y: radius * (1.0 + tan_y * tan_y).sqrt(),
            scale: params.scale,
            speed: FLIGHT_SPEED * params.speed,
            flyers: Vec::new(),
            accumulator: Duration::ZERO,
        };
        // Start in the state the respawn keeps (see `respawn`): depths spread
        // evenly over the range, flyers spread evenly over the view.
        world.flyers = (0..params.count)
            .map(|_| {
                let depth = world.rng.range_f32(DEPTH_RANGE.0, DEPTH_RANGE.1);
                let (hx, hy) = world.half_extents(depth);
                let x = world.rng.range_f32(-hx, hx);
                let y = world.rng.range_f32(-hy, hy);
                world.spawn(Vec3::new(x, y, -depth))
            })
            .collect();
        world
    }

    /// Advances the simulation by `elapsed` wall-clock time.
    ///
    /// `elapsed` is clamped to [`MAX_ELAPSED`]. The simulation runs a whole
    /// number of [`TIME_STEP`]s; the remainder is kept for the next call.
    pub fn advance(&mut self, elapsed: Duration) {
        self.accumulator += elapsed.min(MAX_ELAPSED);
        while self.accumulator >= TIME_STEP {
            self.step();
            self.accumulator -= TIME_STEP;
        }
    }

    /// Returns the flyers to draw, interpolated between the last two steps.
    pub fn instances(&self) -> Vec<FlyerInstance> {
        let alpha = self.alpha();
        let level = level_orientation();
        self.flyers
            .iter()
            .map(|f| {
                let position = f.prev_position + (f.position - f.prev_position).scale(alpha);
                let orientation = match self.style {
                    Style::Tumbling => {
                        let spin = interpolate_phase(f.prev_spin, f.spin, alpha);
                        Quat::from_axis_angle(f.axis, TAU * spin) * f.initial
                    }
                    Style::Level => toward(position) * level,
                };
                FlyerInstance {
                    position,
                    orientation,
                    scale: self.scale,
                    door_angle: door_angle(interpolate_phase(f.prev_flap, f.flap, alpha)),
                    turntable_phase: interpolate_phase(f.prev_turntable, f.turntable, alpha),
                    color: f.color,
                }
            })
            .collect()
    }

    /// Interpolation factor in `[0, 1)`: how far the accumulator is into the next step.
    fn alpha(&self) -> f32 {
        (self.accumulator.as_secs_f64() / TIME_STEP.as_secs_f64()) as f32
    }

    /// Runs one fixed step: moves every flyer and respawns those that left
    /// the view.
    fn step(&mut self) {
        let dt = TIME_STEP.as_secs_f32();
        for i in 0..self.flyers.len() {
            let f = &mut self.flyers[i];
            f.prev_position = f.position;
            f.position = f.position + f.velocity.scale(dt);
            f.prev_spin = f.spin;
            f.spin = wrap_phase(f.spin + f.spin_rate * dt);
            f.prev_flap = f.flap;
            f.flap = wrap_phase(f.flap + f.flap_rate * dt);
            f.prev_turntable = f.turntable;
            f.turntable = wrap_phase(f.turntable + TURNTABLE_RATE * dt);
            // Flyers leave through the left and the bottom of the view.
            let p = f.position;
            let (hx, hy) = self.half_extents(-p.z);
            if p.x < -hx || p.y < -hy {
                self.flyers[i] = self.respawn();
            }
        }
    }

    /// Half width and half height of the area where a flyer at `depth` can
    /// touch the view: its center is within `[-x, x]` and `[-y, y]`.
    fn half_extents(&self, depth: f32) -> (f32, f32) {
        (
            depth * self.tan_x + self.margin_x,
            depth * self.tan_y + self.margin_y,
        )
    }

    /// A new flyer that enters the view at the top or the right.
    ///
    /// Within one depth this is the respawn of the 2D simulation of v0.2.0
    /// (ADR-0023): the entry line
    /// runs along the top of the area of [`World::half_extents`] from the
    /// top-left to the top-right corner, then down its right side. The
    /// flyer crosses the top at rate 1 (down) and the right side at rate 2
    /// (left), so each part is weighted by that rate. This keeps the flyers
    /// spread evenly over the view at that depth.
    ///
    /// The depth is drawn so that the depths of the flyers whose sphere can
    /// touch the view (the view at that depth plus the margin of the sphere)
    /// stay spread evenly over [`DEPTH_RANGE`], as they start in
    /// [`World::new`]. The margin is the same at every depth, so among the
    /// flyers whose center is in the view, far ones are more common (about
    /// 1 to 1.3 to 1.4 between the nearest and the farthest quarter at
    /// `scale` 1.0, depending on the aspect ratio).
    /// A flyer at depth d stays in the view for a time proportional to
    /// area / entry line = 4 hx hy / (2 hx + 4 hy), so the depth is drawn
    /// with a weight proportional to the inverse, 1 / hy + 2 / hx (by
    /// rejection). The weight falls as the depth grows, and hx and hy grow
    /// at most in proportion to the depth, so at least DEPTH_RANGE.0 /
    /// DEPTH_RANGE.1 (about 0.38) of the draws are accepted.
    fn respawn(&mut self) -> Flyer {
        let weight = |(hx, hy): (f32, f32)| 1.0 / hy + 2.0 / hx;
        let most = weight(self.half_extents(DEPTH_RANGE.0));
        let depth = loop {
            let depth = self.rng.range_f32(DEPTH_RANGE.0, DEPTH_RANGE.1);
            if self.rng.next_f32() * most < weight(self.half_extents(depth)) {
                break depth;
            }
        };
        let (hx, hy) = self.half_extents(depth);
        let top = 2.0 * hx;
        let right = 2.0 * hy;
        let u = self.rng.range_f32(0.0, top + 2.0 * right);
        let (x, y) = if u < top {
            (u - hx, hy)
        } else {
            (hx, hy - (u - top) / 2.0)
        };
        self.spawn(Vec3::new(x, y, -depth))
    }

    /// A new flyer at `position` with fresh random motion and looks.
    fn spawn(&mut self, position: Vec3) -> Flyer {
        let rng = &mut self.rng;
        let factor = rng.range_f32(SPEED_FACTOR_RANGE.0, SPEED_FACTOR_RANGE.1);
        let axis = random_unit_vector(rng);
        let spin_rate = rng.range_f32(SPIN_RANGE.0, SPIN_RANGE.1) / TAU;
        let initial = random_orientation(rng);
        let flap = rng.next_f32();
        let flap_rate = rng.range_f32(FLAP_RATE_RANGE.0, FLAP_RATE_RANGE.1);
        let turntable = rng.next_f32();
        // The top bits, as in `Rng::next_f32`; BODY_COLORS is far below 2^32.
        let color = ((rng.next_u64() >> 32) % u64::from(BODY_COLORS)) as u8;
        Flyer {
            // The previous state equals the new one so that interpolation
            // never draws a streak across the view or a jump of a phase.
            prev_position: position,
            position,
            velocity: DIRECTION.scale(self.speed * factor),
            axis,
            spin_rate,
            prev_spin: 0.0,
            spin: 0.0,
            initial,
            prev_flap: flap,
            flap,
            flap_rate,
            prev_turntable: turntable,
            turntable,
            color,
        }
    }
}

/// The orientation of the Level style before the correction toward the
/// line of sight: yaw first, then pitch, then roll.
fn level_orientation() -> Quat {
    Quat::rotation_z(LEVEL_ROLL_DEG.to_radians())
        * Quat::rotation_x(LEVEL_PITCH_DEG.to_radians())
        * Quat::rotation_y(LEVEL_YAW_DEG.to_radians())
}

/// The rotation that takes the view direction (-z) to the direction from
/// the camera to `position`, about the axis perpendicular to both.
///
/// Applied after the Level orientation, it shows the camera the same sides
/// of a flyer wherever it is on the screen (plan 0005, D-10).
fn toward(position: Vec3) -> Quat {
    let forward = Vec3::new(0.0, 0.0, -1.0);
    let dir = position.normalized();
    let axis = forward.cross(dir);
    let sin = axis.length();
    // Why: flyers are always in front of the camera (depth 10 or more), so
    // `dir` is never opposite to `forward`; only the screen center, where
    // no rotation is needed, has no axis.
    if sin < 1e-6 {
        return Quat::IDENTITY;
    }
    let angle = sin.atan2(forward.dot(dir));
    Quat::from_axis_angle(axis.scale(1.0 / sin), angle)
}

/// A uniformly distributed unit vector.
fn random_unit_vector(rng: &mut Rng) -> Vec3 {
    let z = rng.range_f32(-1.0, 1.0);
    let a = rng.range_f32(0.0, TAU);
    let r = (1.0 - z * z).max(0.0).sqrt();
    Vec3::new(r * a.cos(), r * a.sin(), z)
}

/// A uniformly distributed orientation (K. Shoemake, "Uniform random
/// rotations", Graphics Gems III).
fn random_orientation(rng: &mut Rng) -> Quat {
    let u = rng.next_f32();
    let a = TAU * rng.next_f32();
    let b = TAU * rng.next_f32();
    let (r1, r2) = ((1.0 - u).sqrt(), u.sqrt());
    Quat {
        w: r2 * b.cos(),
        x: r1 * a.sin(),
        y: r1 * a.cos(),
        z: r2 * b.sin(),
    }
}

/// Wraps a phase into `[0, 1)`.
fn wrap_phase(phase: f32) -> f32 {
    let wrapped = phase - phase.floor();
    // Why: a tiny negative phase gives 1.0 after rounding; keep it below 1.
    if wrapped >= 1.0 { 0.0 } else { wrapped }
}

/// The phase `alpha` of the way from `from` to `to`, moving forward across
/// the wrap. Phases only move forward, by less than one turn a step.
fn interpolate_phase(from: f32, to: f32, alpha: f32) -> f32 {
    let forward = if to >= from {
        to - from
    } else {
        to + 1.0 - from
    };
    wrap_phase(from + forward * alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASPECT: f32 = 16.0 / 9.0;
    const EPS: f32 = 1e-4;

    fn params(count: u32) -> SimParams {
        SimParams {
            count,
            food_count: 0,
            ..SimParams::default()
        }
    }

    fn world(seed: u64) -> World {
        World::new(seed, ASPECT, SimParams::default(), Style::Tumbling)
    }

    fn close(a: Vec3, b: Vec3, eps: f32) -> bool {
        (a - b).length() < eps
    }

    fn conjugate(q: Quat) -> Quat {
        Quat {
            w: q.w,
            x: -q.x,
            y: -q.y,
            z: -q.z,
        }
    }

    /// The angle in radians of the rotation from `a` to `b`.
    fn angle_between(a: Quat, b: Quat) -> f32 {
        let r = b * conjugate(a);
        // Why atan2: acos loses precision near 1 in f32.
        let sin = Vec3::new(r.x, r.y, r.z).length();
        2.0 * sin.atan2(r.w.abs())
    }

    fn ndc(m: &Mat4, p: Vec3) -> Vec3 {
        let [x, y, z, w] = m.transform(p);
        Vec3::new(x / w, y / w, z / w)
    }

    fn depth_of(p: Vec3) -> f32 {
        -p.z
    }

    /// A flyer at `position` with no streak, flying at the given speed factor.
    fn flyer_at(position: Vec3, speed_factor: f32) -> Flyer {
        Flyer {
            prev_position: position,
            position,
            velocity: DIRECTION.scale(FLIGHT_SPEED * speed_factor),
            axis: Vec3::new(0.0, 1.0, 0.0),
            spin_rate: 0.5 / TAU,
            prev_spin: 0.0,
            spin: 0.0,
            initial: Quat::IDENTITY,
            prev_flap: 0.0,
            flap: 0.0,
            flap_rate: 1.5,
            prev_turntable: 0.0,
            turntable: 0.0,
            color: 0,
        }
    }

    /// A one-flyer world whose only flyer is `flyer`.
    fn world_with(seed: u64, style: Style, flyer: Flyer) -> World {
        let mut w = World::new(seed, ASPECT, params(1), style);
        w.flyers[0] = flyer;
        w
    }

    // --- Camera -----------------------------------------------------------

    #[test]
    fn direction_is_normalized_left_two_down_one() {
        assert!((DIRECTION.length() - 1.0).abs() < 1e-6);
        assert!((DIRECTION.x / DIRECTION.y - 2.0).abs() < 1e-6);
        const { assert!(DIRECTION.x < 0.0 && DIRECTION.z == 0.0) };
    }

    #[test]
    fn projection_uses_the_field_of_view_and_the_clip_planes() {
        let p = projection(ASPECT);
        let d = 20.0;
        let half_h = d * (FOV_Y_DEG.to_radians() / 2.0).tan();
        let corner = ndc(&p, Vec3::new(half_h * ASPECT, half_h, -d));
        assert!((corner.x - 1.0).abs() < EPS, "{corner:?}");
        assert!((corner.y - 1.0).abs() < EPS, "{corner:?}");
        assert!(ndc(&p, Vec3::new(0.0, 0.0, -NEAR)).z.abs() < 1e-6);
        assert!((ndc(&p, Vec3::new(0.0, 0.0, -FAR)).z - 1.0).abs() < 1e-6);
    }

    #[test]
    fn projection_replaces_an_invalid_aspect_with_one() {
        for aspect in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(projection(aspect), projection(1.0), "aspect = {aspect}");
        }
    }

    #[test]
    fn farther_flyers_look_smaller_and_slower() {
        // F-08: the same size and speed in view space look smaller and
        // slower on the screen the farther they are.
        let p = projection(ASPECT);
        let near = DEPTH_RANGE.0;
        let far = DEPTH_RANGE.1;
        let on_screen = |d: f32, offset: Vec3| {
            let a = ndc(&p, Vec3::new(0.0, 0.0, -d));
            let b = ndc(&p, Vec3::new(0.0, 0.0, -d) + offset);
            (b - a).length()
        };
        let width = Vec3::new(BODY_SIZE.x, 0.0, 0.0);
        let one_second = DIRECTION.scale(FLIGHT_SPEED);
        assert!(on_screen(near, width) > on_screen(far, width));
        assert!(on_screen(near, one_second) > on_screen(far, one_second));
    }

    // --- Door and turntable -----------------------------------------------

    #[test]
    fn door_is_closed_at_zero_and_widest_at_half() {
        let max = MAX_DOOR_DEG.to_radians();
        assert_eq!(door_angle(0.0), 0.0);
        assert!((door_angle(0.5) - max).abs() < 1e-6);
        assert!((door_angle(0.25) - max / 2.0).abs() < 1e-6);
        assert!(door_angle(0.999).abs() < 1e-3, "continuous across the wrap");
    }

    #[test]
    fn door_angle_stays_between_zero_and_seventy_degrees() {
        for i in 0..1000 {
            let a = door_angle(i as f32 / 1000.0);
            assert!((0.0..=70.0_f32.to_radians() + 1e-6).contains(&a), "{a}");
        }
    }

    #[test]
    fn instances_have_a_door_angle_a_turntable_phase_and_a_body_color() {
        let mut w = World::new(29, ASPECT, params(200), Style::Tumbling);
        for _ in 0..50 {
            w.advance(Duration::from_millis(37));
            for i in w.instances() {
                assert!(
                    (0.0..=MAX_DOOR_DEG.to_radians() + 1e-6).contains(&i.door_angle),
                    "door = {}",
                    i.door_angle
                );
                assert!((0.0..1.0).contains(&i.turntable_phase));
                assert!(i.color < BODY_COLORS, "color = {}", i.color);
            }
        }
    }

    #[test]
    fn every_body_color_is_used() {
        let w = World::new(31, ASPECT, params(200), Style::Tumbling);
        let mut seen = [false; BODY_COLORS as usize];
        for i in w.instances() {
            seen[usize::from(i.color)] = true;
        }
        assert!(seen.iter().all(|&s| s), "colors seen: {seen:?}");
    }

    #[test]
    fn flap_rate_stays_well_below_three_per_second() {
        let w = World::new(37, ASPECT, params(500), Style::Tumbling);
        for f in &w.flyers {
            assert!(
                (FLAP_RATE_RANGE.0..FLAP_RATE_RANGE.1).contains(&f.flap_rate),
                "rate = {}",
                f.flap_rate
            );
        }
        assert!(FLAP_RATE_RANGE.0 > 0.0 && FLAP_RATE_RANGE.1 <= 2.0);
    }

    #[test]
    fn flap_and_turntable_advance_by_their_rates_and_wrap() {
        let mut f = flyer_at(Vec3::new(0.0, 0.0, -15.0), 1.0);
        f.flap_rate = 1.5;
        f.prev_flap = 0.995;
        f.flap = 0.995;
        f.prev_turntable = 0.9995;
        f.turntable = 0.9995;
        let mut w = world_with(1, Style::Tumbling, f);
        w.advance(TIME_STEP);
        let f = w.flyers[0];
        let dt = TIME_STEP.as_secs_f32();
        assert_eq!(f.prev_flap, 0.995);
        assert!(
            (f.flap - (0.995 + 1.5 * dt - 1.0)).abs() < 1e-6,
            "{}",
            f.flap
        );
        assert!(
            (f.turntable - (0.9995 + TURNTABLE_RATE * dt - 1.0)).abs() < 1e-6,
            "{}",
            f.turntable
        );
    }

    #[test]
    fn phases_interpolate_across_the_wrap() {
        let mut f = flyer_at(Vec3::new(0.0, 0.0, -15.0), 1.0);
        f.flap_rate = 1.5;
        f.prev_flap = 0.99;
        f.flap = 0.99;
        f.prev_turntable = 0.9999;
        f.turntable = 0.9999;
        let mut w = world_with(1, Style::Tumbling, f);
        // One step across the wrap, then half a step more.
        w.advance(TIME_STEP + TIME_STEP / 2);
        let f = w.flyers[0];
        let i = w.instances()[0];
        let halfway = |from: f32, to: f32| {
            let h = from + (to + 1.0 - from) / 2.0;
            h - h.floor()
        };
        let flap = halfway(f.prev_flap, f.flap);
        assert!((i.door_angle - door_angle(flap)).abs() < 1e-4);
        let turntable = halfway(f.prev_turntable, f.turntable);
        assert!(
            (i.turntable_phase - turntable).abs() < 1e-4,
            "{} vs {turntable}",
            i.turntable_phase
        );
    }

    // --- Counts, seeds and steps --------------------------------------------

    #[test]
    fn count_gives_that_many_instances() {
        for count in [0, 1, 5, 12, 100] {
            let mut w = World::new(1, ASPECT, params(count), Style::Tumbling);
            assert_eq!(w.instances().len(), count as usize);
            w.advance(Duration::from_secs(1));
            assert_eq!(w.instances().len(), count as usize);
        }
    }

    #[test]
    fn same_seed_and_same_advances_give_identical_instances() {
        for style in [Style::Tumbling, Style::Level] {
            let mut a = World::new(21, ASPECT, SimParams::default(), style);
            let mut b = World::new(21, ASPECT, SimParams::default(), style);
            for ms in [3, 16, 40, 7, 250, 1, 90] {
                a.advance(Duration::from_millis(ms));
                b.advance(Duration::from_millis(ms));
                assert_eq!(a.instances(), b.instances());
            }
        }
    }

    #[test]
    fn different_seeds_give_different_instances() {
        assert_ne!(world(1).instances(), world(2).instances());
    }

    #[test]
    fn style_changes_only_the_orientation() {
        let mut a = World::new(5, ASPECT, SimParams::default(), Style::Tumbling);
        let mut b = World::new(5, ASPECT, SimParams::default(), Style::Level);
        for _ in 0..100 {
            a.advance(Duration::from_millis(50));
            b.advance(Duration::from_millis(50));
            for (x, y) in a.instances().iter().zip(b.instances()) {
                assert_eq!(x.position, y.position);
                assert_eq!(x.door_angle, y.door_angle);
                assert_eq!(x.color, y.color);
            }
        }
    }

    #[test]
    fn elapsed_is_clamped_to_max_elapsed() {
        let mut long = world(9);
        let mut max = world(9);
        long.advance(Duration::from_secs(10));
        max.advance(MAX_ELAPSED);
        assert_eq!(long.flyers, max.flyers);
        assert_eq!(long.accumulator, max.accumulator);
        assert_eq!(long.rng.next_u64(), max.rng.next_u64());
        assert_eq!(long.instances(), max.instances());
    }

    #[test]
    fn advance_runs_exactly_three_steps_for_three_time_steps() {
        let mut f = flyer_at(Vec3::new(1.0, 2.0, -15.0), 1.0);
        f.spin_rate = 0.1;
        let mut w = world_with(17, Style::Tumbling, f);
        w.advance(TIME_STEP * 3);
        assert_eq!(w.accumulator, Duration::ZERO);
        let dt = TIME_STEP.as_secs_f32();
        let g = w.flyers[0];
        assert!(close(
            g.position,
            f.position + f.velocity.scale(3.0 * dt),
            1e-6
        ));
        assert!(close(
            g.prev_position,
            f.position + f.velocity.scale(2.0 * dt),
            1e-6
        ));
        assert!((g.spin - 3.0 * 0.1 * dt).abs() < 1e-6);
    }

    #[test]
    fn remainder_below_one_step_is_carried_over() {
        let mut w = world(17);
        let start = w.flyers.clone();
        w.advance(TIME_STEP - Duration::from_nanos(1));
        assert_eq!(w.flyers, start);
        assert_eq!(w.accumulator, TIME_STEP - Duration::from_nanos(1));
        w.advance(Duration::from_nanos(6));
        assert_eq!(w.accumulator, Duration::from_nanos(5));
        assert_ne!(w.flyers, start);
    }

    #[test]
    fn interpolation_factor_stays_below_one() {
        let mut w = world(13);
        w.advance(TIME_STEP - Duration::from_nanos(1));
        let alpha = w.alpha();
        assert!((0.0..1.0).contains(&alpha), "alpha = {alpha}");
        assert!(alpha > 0.999);
    }

    #[test]
    fn instances_interpolate_halfway_after_one_and_a_half_steps() {
        let f = flyer_at(Vec3::new(1.0, 2.0, -15.0), 1.0);
        let mut w = world_with(13, Style::Tumbling, f);
        w.advance(TIME_STEP + TIME_STEP / 2);
        let g = w.flyers[0];
        let i = w.instances()[0];
        assert_ne!(g.prev_position, g.position);
        let mid = (g.prev_position + g.position).scale(0.5);
        assert!(close(i.position, mid, 1e-6));
        let spin = (g.prev_spin + g.spin) / 2.0;
        let want = Quat::from_axis_angle(g.axis, TAU * spin) * g.initial;
        assert!(angle_between(i.orientation, want) < 1e-3);
    }

    // --- Flight -------------------------------------------------------------

    #[test]
    fn initial_flyers_start_inside_the_view_area_without_streak() {
        let w = World::new(3, ASPECT, params(500), Style::Tumbling);
        for f in &w.flyers {
            let d = depth_of(f.position);
            assert!((DEPTH_RANGE.0..=DEPTH_RANGE.1).contains(&d), "depth {d}");
            let (hx, hy) = w.half_extents(d);
            assert!(f.position.x.abs() <= hx && f.position.y.abs() <= hy);
            assert_eq!(f.prev_position, f.position);
        }
        for (f, i) in w.flyers.iter().zip(w.instances()) {
            assert_eq!(i.position, f.position);
        }
    }

    #[test]
    fn velocity_points_left_and_down_at_the_same_speed_for_every_depth() {
        for speed in [1.0, 0.25, 3.0] {
            let p = SimParams {
                count: 500,
                speed,
                scale: 1.0,
                food_count: 0,
            };
            let w = World::new(5, ASPECT, p, Style::Tumbling);
            for f in &w.flyers {
                let v = f.velocity;
                assert_eq!(v.z, 0.0, "the depth stays the same");
                assert!(v.x < 0.0 && v.y < 0.0);
                assert!((v.x / v.y - 2.0).abs() < 1e-4);
                // The speed does not depend on the depth (F-08).
                let s = v.length() / (FLIGHT_SPEED * speed);
                assert!(
                    (SPEED_FACTOR_RANGE.0 - 1e-5..=SPEED_FACTOR_RANGE.1 + 1e-5).contains(&s),
                    "factor = {s}"
                );
            }
        }
    }

    #[test]
    fn scale_multiplies_the_body_size() {
        for scale in [0.5, 1.0, 2.0] {
            let p = SimParams {
                count: 20,
                speed: 1.0,
                scale,
                food_count: 0,
            };
            let w = World::new(5, ASPECT, p, Style::Tumbling);
            for i in w.instances() {
                assert_eq!(i.scale, scale);
            }
        }
    }

    #[test]
    fn advance_moves_every_flyer_left_and_down_at_its_depth() {
        // A flyer at the right edge of the area cannot leave it within 0.1 s.
        let mut w = World::new(7, ASPECT, params(1), Style::Tumbling);
        let (hx, _) = w.half_extents(15.0);
        w.flyers[0] = flyer_at(Vec3::new(hx, 0.0, -15.0), 1.1);
        // Whole steps only, so both instances lag the state by one step.
        w.advance(TIME_STEP);
        let before = w.instances()[0];
        w.advance(TIME_STEP * 12);
        let after = w.instances()[0];
        let moved = after.position - before.position;
        assert!(
            moved.x < 0.0 && moved.y < 0.0 && moved.z == 0.0,
            "{moved:?}"
        );
        assert!((moved.x / moved.y - 2.0).abs() < 1e-3, "{moved:?}");
        let want = FLIGHT_SPEED * 1.1 * 12.0 * TIME_STEP.as_secs_f32();
        assert!((moved.length() - want).abs() < 1e-4 * want, "{moved:?}");
    }

    #[test]
    fn flyer_that_is_not_fully_outside_is_kept() {
        let mut w = World::new(1, ASPECT, params(1), Style::Tumbling);
        let (hx, hy) = w.half_extents(12.0);
        for start in [
            Vec3::new(-hx + 0.05, 0.0, -12.0),
            Vec3::new(0.0, -hy + 0.05, -12.0),
        ] {
            w.flyers[0] = flyer_at(start, 1.0);
            w.advance(TIME_STEP);
            let f = w.flyers[0];
            assert_eq!(f.prev_position, start);
            assert_eq!(f.velocity, flyer_at(start, 1.0).velocity);
        }
    }

    #[test]
    fn the_area_ends_where_the_bounding_sphere_leaves_the_view() {
        // At the edge of the area, the bounding sphere touches the left or
        // the bottom plane of the view from outside, for every size.
        let tan_y = (FOV_Y_DEG.to_radians() / 2.0).tan();
        let tan_x = tan_y * ASPECT;
        // Inward normals of the left and the bottom planes.
        let left = Vec3::new(1.0, 0.0, -tan_x).normalized();
        let bottom = Vec3::new(0.0, 1.0, -tan_y).normalized();
        for scale in [0.5, 1.0, 2.0] {
            let p = SimParams {
                count: 1,
                speed: 1.0,
                scale,
                food_count: 0,
            };
            let w = World::new(1, ASPECT, p, Style::Tumbling);
            let radius = BOUNDING_RADIUS * scale;
            for d in [DEPTH_RANGE.0, 17.0, DEPTH_RANGE.1] {
                let (hx, hy) = w.half_extents(d);
                let distance = left.dot(Vec3::new(-hx, 0.0, -d));
                assert!((distance + radius).abs() < 1e-4, "{distance} {radius}");
                let distance = bottom.dot(Vec3::new(0.0, -hy, -d));
                assert!((distance + radius).abs() < 1e-4, "{distance} {radius}");
            }
        }
    }

    #[test]
    fn flyer_whose_center_left_the_view_is_kept_while_its_body_is_seen() {
        let tan_y = (FOV_Y_DEG.to_radians() / 2.0).tan();
        let d = 12.0;
        let edge_x = -d * tan_y * ASPECT;
        let edge_y = -d * tan_y;
        let mut w = World::new(1, ASPECT, params(1), Style::Tumbling);
        // Half the bounding radius past the left or the bottom edge.
        for start in [
            Vec3::new(edge_x - BOUNDING_RADIUS / 2.0, 0.0, -d),
            Vec3::new(0.0, edge_y - BOUNDING_RADIUS / 2.0, -d),
        ] {
            w.flyers[0] = flyer_at(start, 1.0);
            w.advance(TIME_STEP);
            assert_eq!(w.flyers[0].prev_position, start, "kept");
        }
    }

    #[test]
    fn respawn_enters_at_the_top_or_the_right_without_streak() {
        let (mut tops, mut rights) = (0, 0);
        for seed in 0..64 {
            let mut w = World::new(seed, ASPECT, params(1), Style::Tumbling);
            let (hx, hy) = w.half_extents(18.0);
            // Just inside the left or the bottom edge; the first step takes
            // it fully outside. The factor 0.5 is below the respawn range, so
            // an in-range speed afterwards proves that a new one was drawn.
            for exiting in [
                Vec3::new(-hx + 1e-4, 0.0, -18.0),
                Vec3::new(0.0, -hy + 1e-4, -18.0),
            ] {
                for quarter in 0..4 {
                    w.flyers[0] = flyer_at(exiting, 0.5);
                    w.accumulator = Duration::ZERO;
                    w.advance(TIME_STEP + TIME_STEP * quarter / 4);
                    let f = w.flyers[0];
                    let d = depth_of(f.position);
                    assert!((DEPTH_RANGE.0..=DEPTH_RANGE.1).contains(&d), "depth {d}");
                    let (hx, hy) = w.half_extents(d);
                    let p = f.position;
                    if (p.y - hy).abs() < 1e-4 && p.x.abs() <= hx + 1e-4 {
                        tops += 1;
                    } else if (p.x - hx).abs() < 1e-4 && p.y.abs() <= hy + 1e-4 {
                        rights += 1;
                    } else {
                        panic!("not on the entry line: {p:?}, {hx}, {hy}");
                    }
                    assert_eq!(f.prev_position, f.position, "no streak");
                    let s = f.velocity.length() / FLIGHT_SPEED;
                    assert!((0.8 - 1e-5..=1.2 + 1e-5).contains(&s), "factor {s}");
                    // The drawn instance is the spawn point at every
                    // interpolation factor, and the phases do not jump.
                    let i = w.instances()[0];
                    assert_eq!(i.position, f.position);
                    assert_eq!((f.prev_spin, f.prev_flap), (f.spin, f.flap));
                    assert_eq!(f.prev_turntable, f.turntable);
                }
            }
        }
        assert!(tops > 0 && rights > 0, "tops = {tops}, rights = {rights}");
    }

    #[test]
    fn respawn_tracks_cover_the_view_evenly_at_every_depth() {
        // Along the flight direction (-2, -1) the value x - 2y stays the
        // same, so it names the track of a flyer. At depth d the tracks that
        // touch the view run from the top-left corner (-hx, hy) to the
        // bottom-right corner (hx, -hy) of the area. Normalized to [-1, 1],
        // they must be spread evenly, at near and far depths alike.
        let mut w = World::new(7, ASPECT, params(1), Style::Tumbling);
        let mut bins = [[0_u32; 8]; 2];
        for _ in 0..40_000 {
            w.flyers[0] = flyer_at(Vec3::new(-1000.0, 0.0, -15.0), 1.0);
            w.accumulator = Duration::ZERO;
            w.advance(TIME_STEP);
            let p = w.flyers[0].position;
            let d = depth_of(p);
            let (hx, hy) = w.half_extents(d);
            let track = (p.x - 2.0 * p.y) / (hx + 2.0 * hy);
            assert!((-1.0 - 1e-4..=1.0 + 1e-4).contains(&track), "{track}");
            let half = usize::from(d > (DEPTH_RANGE.0 + DEPTH_RANGE.1) / 2.0);
            let i = (((track + 1.0) / 2.0) * 8.0) as usize;
            bins[half][i.min(7)] += 1;
        }
        for half in bins {
            let total: u32 = half.iter().sum();
            for &n in &half {
                let share = n as f32 / total as f32;
                assert!((0.105..=0.145).contains(&share), "{half:?}");
            }
        }
    }

    #[test]
    fn flyers_stay_spread_evenly_over_the_view_and_the_depths() {
        // After every flyer has been replaced, the depths are still spread
        // evenly over the range and the flyers evenly over the view: the
        // respawn keeps the state the world started with.
        let p = SimParams {
            count: 2000,
            speed: 3.0,
            scale: 1.0,
            food_count: 0,
        };
        let mut w = World::new(41, ASPECT, p, Style::Tumbling);
        // The longest crossing (the far corner at depth 26, factor 0.8) is
        // about 50 units: 22 s at speed 1.0, 7.3 s at speed 3.0. Run 24 s.
        for _ in 0..240 {
            w.advance(Duration::from_millis(100));
        }
        let (lo, hi) = DEPTH_RANGE;
        let mut depth_bins = [0_u32; 4];
        let mut x_bins = [0_u32; 4];
        let mut y_bins = [0_u32; 4];
        let bin = |v: f32| ((v * 4.0) as usize).min(3);
        for f in &w.flyers {
            let d = depth_of(f.position);
            let (hx, hy) = w.half_extents(d);
            depth_bins[bin((d - lo) / (hi - lo))] += 1;
            x_bins[bin((f.position.x / hx + 1.0) / 2.0)] += 1;
            y_bins[bin((f.position.y / hy + 1.0) / 2.0)] += 1;
        }
        for bins in [depth_bins, x_bins, y_bins] {
            for &n in &bins {
                assert!(
                    (420..=580).contains(&n),
                    "{depth_bins:?} {x_bins:?} {y_bins:?}"
                );
            }
        }
    }

    #[test]
    fn invalid_aspect_gives_finite_instances() {
        for aspect in [0.0, -1.0, -0.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut w = World::new(23, aspect, SimParams::default(), Style::Level);
            let mut valid = World::new(23, 1.0, SimParams::default(), Style::Level);
            for _ in 0..200 {
                w.advance(Duration::from_millis(16));
                valid.advance(Duration::from_millis(16));
            }
            assert_eq!(w.instances(), valid.instances(), "aspect = {aspect}");
        }
    }

    // --- Orientation ----------------------------------------------------------

    #[test]
    fn tumbling_axes_are_unit_vectors_and_speeds_are_in_range() {
        let w = World::new(43, ASPECT, params(500), Style::Tumbling);
        let mut axes_differ = false;
        for f in &w.flyers {
            assert!((f.axis.length() - 1.0).abs() < 1e-4);
            let rad = f.spin_rate * TAU;
            assert!(
                (SPIN_RANGE.0 - 1e-5..=SPIN_RANGE.1 + 1e-5).contains(&rad),
                "spin = {rad}"
            );
            axes_differ |= !close(f.axis, w.flyers[0].axis, 0.1);
        }
        assert!(axes_differ);
    }

    #[test]
    fn tumbling_turns_at_the_flyer_spin_rate() {
        for seed in 0..16 {
            let mut w = World::new(seed, ASPECT, params(1), Style::Tumbling);
            // Keep the drawn axis, speed and start; fly from the middle so
            // the flyer cannot leave the view within the test.
            let drawn = w.flyers[0];
            let mut f = flyer_at(Vec3::new(0.0, 0.0, -15.0), 1.0);
            f.axis = drawn.axis;
            f.spin_rate = drawn.spin_rate;
            f.initial = drawn.initial;
            w.flyers[0] = f;
            // Whole steps only, so both instances lag the state by one step.
            w.advance(TIME_STEP);
            let before = w.instances()[0].orientation;
            w.advance(TIME_STEP * 24);
            let after = w.instances()[0].orientation;
            let rad_per_s = angle_between(before, after) / (24.0 * TIME_STEP.as_secs_f32());
            assert!(
                (rad_per_s - drawn.spin_rate * TAU).abs() < 1e-2,
                "{rad_per_s} vs {}",
                drawn.spin_rate * TAU
            );
            assert!((SPIN_RANGE.0 - 1e-2..=SPIN_RANGE.1 + 1e-2).contains(&rad_per_s));
        }
    }

    #[test]
    fn orientations_are_unit_quaternions() {
        for style in [Style::Tumbling, Style::Level] {
            let mut w = World::new(47, ASPECT, params(100), style);
            for _ in 0..30 {
                w.advance(Duration::from_millis(97));
                for i in w.instances() {
                    let q = i.orientation;
                    let n = q.w * q.w + q.x * q.x + q.y * q.y + q.z * q.z;
                    assert!((n - 1.0).abs() < 1e-4, "{q:?}");
                }
            }
        }
    }

    /// The orientation of the Level style before the correction.
    fn level() -> Quat {
        Quat::rotation_z(LEVEL_ROLL_DEG.to_radians())
            * Quat::rotation_x(LEVEL_PITCH_DEG.to_radians())
            * Quat::rotation_y(LEVEL_YAW_DEG.to_radians())
    }

    #[test]
    fn level_at_the_screen_center_is_the_level_orientation() {
        let f = flyer_at(Vec3::new(0.0, 0.0, -15.0), 1.0);
        let w = world_with(1, Style::Level, f);
        let q = w.instances()[0].orientation;
        assert!(angle_between(q, level()) < 1e-4, "{q:?}");
    }

    #[test]
    fn level_shows_the_same_sides_wherever_the_flyer_is() {
        // Seen from the flyer, the camera must be in the same direction of
        // its own (body) frame at every position on the screen: then the same
        // sides face the camera, as at the screen center.
        let want = conjugate(level()).rotate(Vec3::new(0.0, 0.0, 1.0));
        let mut w = World::new(53, ASPECT, params(300), Style::Level);
        let mut checked = 0;
        for _ in 0..20 {
            w.advance(Duration::from_millis(250));
            for i in w.instances() {
                let to_camera = i.position.scale(-1.0).normalized();
                let seen = conjugate(i.orientation).rotate(to_camera);
                assert!(close(seen, want, 1e-4), "{seen:?} vs {want:?}");
                checked += 1;
            }
        }
        assert_eq!(checked, 300 * 20);
        // Without the correction the sides would differ off the center.
        let corner = Vec3::new(10.0, 6.0, -15.0).scale(-1.0).normalized();
        assert!(!close(conjugate(level()).rotate(corner), want, 1e-2));
    }
}
