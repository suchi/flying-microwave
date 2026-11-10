//! The scenes of the spike and the look values to judge (plan 0005, E-1):
//! camera, light, rotation speeds, door flaps and flight.

use std::f32::consts::{PI, TAU};

use crate::gpu::{Flyer, Frame};
use crate::math::{Mat4, Quat, Vec3, rot_x, rot_y, rot_z, v3};
use crate::mesh;

/// Vertical field of view in degrees.
pub const FOV_Y_DEG: f32 = 40.0;
/// Near and far clip planes in model units (the body is 1.0 high).
pub const NEAR: f32 = 1.0;
pub const FAR: f32 = 100.0;
/// Distance range of the flyers from the camera. A body is 2.06 / distance
/// screen heights wide: 0.21 at 10, 0.11 at 18, 0.08 at 26. The v0.2.0 body
/// at scale 1.0 is 0.09 wide, the same as at distance 23.
pub const DEPTH_RANGE: (f32, f32) = (10.0, 26.0);
/// Tumbling speed range in radians per second (about 20 to 52 degrees).
pub const SPIN_RANGE: (f32, f32) = (0.35, 0.9);
/// Door flaps per second; the same range as v0.2.0 (core::sim).
pub const FLAP_RATE_RANGE: (f32, f32) = (1.2, 1.8);
/// Widest door opening in degrees (ADR-0023).
pub const MAX_DOOR_DEG: f32 = 70.0;
/// Turntable speed in radians per second (0.1 turns a second).
pub const TURNTABLE_SPEED: f32 = TAU * 0.1;
/// Flight speed in model units per second, the same for every depth, so
/// farther flyers look slower (F-08). 2.4 is 3.3 / distance screen heights
/// a second: 0.15 (v0.2.0 at speed 1.0) at distance 22.
pub const FLIGHT_SPEED: f32 = 2.4;
/// Toward the light (view space): from the upper left, in front.
pub fn light_dir() -> Vec3 {
    v3(-0.35, 0.8, 0.5).normalized()
}
/// Brightness of a lit face: AMBIENT + DIFFUSE * (0.5 + 0.5 * cos), so it
/// stays within 0.55..1.00 whatever the orientation (guideline G-3).
pub const AMBIENT: f32 = 0.55;
pub const DIFFUSE: f32 = 0.45;
/// The Level style: yaw, pitch and roll in degrees. Seen from the upper
/// right like the v0.2.0 sprite, tilted 8 degrees nose (left) down.
pub const LEVEL_YAW_DEG: f32 = -30.0;
pub const LEVEL_PITCH_DEG: f32 = 20.0;
pub const LEVEL_ROLL_DEG: f32 = 8.0;

pub fn frame(aspect: f32) -> Frame {
    Frame {
        view_proj: Mat4::perspective(FOV_Y_DEG.to_radians(), aspect, NEAR, FAR),
        light: light_dir(),
        ambient: AMBIENT,
        diffuse: DIFFUSE,
    }
}

/// Door angle in radians for a flap phase: closed at 0, widest at 0.5.
pub fn door_angle(phase: f32) -> f32 {
    MAX_DOOR_DEG.to_radians() * (1.0 - (TAU * phase).cos()) / 2.0
}

/// The matrices of one microwave.
pub fn place(color: usize, position: Vec3, orientation: Quat, door: f32, turntable: f32) -> Flyer {
    let body = Mat4::translation(position).mul(&orientation.normalized().to_mat4());
    let (hx, hz) = mesh::hinge();
    // Negative about y swings the free edge toward +z (out of the front).
    let door = body
        .mul(&Mat4::translation(v3(hx, 0.0, hz)))
        .mul(&rot_y(-door).to_mat4());
    let (center, radius) = mesh::turntable_place();
    let turntable = body
        .mul(&Mat4::translation(center))
        .mul(&rot_y(turntable).to_mat4())
        .mul(&Mat4::uniform_scale(radius));
    Flyer {
        color,
        body,
        door,
        turntable,
    }
}

/// A small deterministic random generator (SplitMix64).
struct Rng(u64);

impl Rng {
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, (lo, hi): (f32, f32)) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }
    /// A uniformly distributed unit vector.
    fn unit(&mut self) -> Vec3 {
        let z = self.range((-1.0, 1.0));
        let a = self.range((0.0, TAU));
        let r = (1.0 - z * z).sqrt();
        v3(r * a.cos(), r * a.sin(), z)
    }
}

/// One flyer of a flight scene.
pub struct Flight {
    color: usize,
    /// Position at t = 0, in view space.
    start: Vec3,
    axis: Vec3,
    spin: f32,
    initial: Quat,
    flap_phase: f32,
    flap_rate: f32,
    turntable_phase: f32,
}

/// Screen positions (NDC x, NDC y) and distances of six flyers: spread over
/// the screen and over the whole depth range.
const LAYOUT: [(f32, f32, f32); 6] = [
    (-0.55, 0.45, 10.0),
    (0.40, 0.55, 18.0),
    (0.70, -0.20, 13.0),
    (-0.10, -0.05, 23.0),
    (-0.70, -0.50, 16.0),
    (0.25, -0.62, 26.0),
];

pub fn flights(aspect: f32) -> Vec<Flight> {
    let mut rng = Rng(0x0000_53D5_F1E5);
    let half_h = (FOV_Y_DEG.to_radians() / 2.0).tan();
    LAYOUT
        .iter()
        .enumerate()
        .map(|(i, &(nx, ny, d))| {
            debug_assert!((DEPTH_RANGE.0..=DEPTH_RANGE.1).contains(&d));
            Flight {
                color: i % 4,
                start: v3(nx * d * half_h * aspect, ny * d * half_h, -d),
                axis: rng.unit(),
                spin: rng.range(SPIN_RANGE),
                initial: Quat::from_axis_angle(rng.unit(), rng.range((0.0, TAU))),
                flap_phase: rng.next_f32(),
                flap_rate: rng.range(FLAP_RATE_RANGE),
                turntable_phase: rng.range((0.0, TAU)),
            }
        })
        .collect()
}

/// Flight direction: left 2, down 1, as in v0.2.0.
fn velocity() -> Vec3 {
    v3(-2.0, -1.0, 0.0).normalized().scale(FLIGHT_SPEED)
}

pub fn level_orientation() -> Quat {
    rot_z(LEVEL_ROLL_DEG.to_radians())
        .mul(rot_x(LEVEL_PITCH_DEG.to_radians()))
        .mul(rot_y(LEVEL_YAW_DEG.to_radians()))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Style {
    Tumbling,
    /// Every flyer has the same orientation in view space. Off the screen
    /// center the perspective shows each one from a different side.
    Level,
    /// Level, turned toward the line of sight of each flyer, so every flyer
    /// shows the same sides wherever it is on the screen (as in v0.2.0).
    LevelFacing,
}

/// The rotation that takes the view direction (-z) to the direction from
/// the camera to `position`.
fn toward(position: Vec3) -> Quat {
    let forward = v3(0.0, 0.0, -1.0);
    let dir = position.normalized();
    let axis = forward.cross(dir);
    let sin = axis.length();
    if sin < 1e-6 {
        return rot_y(0.0);
    }
    let angle = sin.atan2(forward.dot(dir));
    Quat::from_axis_angle(axis.scale(1.0 / sin), angle)
}

/// The flyers of a flight scene at time `t` seconds.
pub fn flyers_at(flights: &[Flight], style: Style, t: f32) -> Vec<Flyer> {
    flights
        .iter()
        .map(|f| {
            let position = f.start.add(velocity().scale(t));
            let orientation = match style {
                Style::Tumbling => Quat::from_axis_angle(f.axis, f.spin * t).mul(f.initial),
                Style::Level => level_orientation(),
                Style::LevelFacing => toward(position).mul(level_orientation()),
            };
            place(
                f.color,
                position,
                orientation,
                door_angle(f.flap_phase + f.flap_rate * t),
                f.turntable_phase + TURNTABLE_SPEED * t,
            )
        })
        .collect()
}

/// Close-ups of one silver microwave: name, yaw, pitch (degrees), door
/// angle (degrees), distance.
pub const CLOSEUPS: [(&str, f32, f32, f32, f32); 4] = [
    ("front", 0.0, 0.0, 0.0, 3.4),
    ("angled", -30.0, 20.0, 20.0, 3.4),
    ("open", -30.0, 15.0, 70.0, 3.4),
    ("hinge", 55.0, 12.0, 70.0, 3.4),
];

pub fn closeup(yaw: f32, pitch: f32, door_deg: f32, distance: f32) -> Vec<Flyer> {
    let q = rot_x(pitch.to_radians()).mul(rot_y(yaw.to_radians()));
    vec![place(
        0,
        v3(0.0, 0.0, -distance),
        q,
        door_deg.to_radians(),
        PI / 5.0,
    )]
}
