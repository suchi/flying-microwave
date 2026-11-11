//! Minimal 3D math for the flight and the renderer: vectors, unit
//! quaternions for orientations, 4x4 matrices and the perspective
//! projection. Written by hand so the product needs no math crate
//! (ADR-0026; plan 0005, D-3 and E-5).
//!
//! Conventions: right-handed, y up, the camera looks along -z. Matrices are
//! column-major (`m[column][row]`), as WGSL expects, and act on column
//! vectors. Clip-space depth goes from 0 (near) to 1 (far), as in wgpu.

use std::ops::{Add, Mul, Sub};

/// A 3D vector or point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn scale(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// This vector scaled to length 1. The vector must not be zero.
    pub fn normalized(self) -> Vec3 {
        self.scale(1.0 / self.length())
    }
}

impl Add for Vec3 {
    type Output = Vec3;

    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;

    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

/// A unit quaternion for orientations: `w + xi + yj + zk`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Quat {
    /// No rotation.
    pub const IDENTITY: Quat = Quat {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Rotation by `angle` radians about the unit vector `axis`
    /// (counterclockwise when the axis points toward the viewer).
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Quat {
        let (s, c) = (angle / 2.0).sin_cos();
        Quat {
            w: c,
            x: axis.x * s,
            y: axis.y * s,
            z: axis.z * s,
        }
    }

    /// Rotation by `angle` radians about the x axis.
    pub fn rotation_x(angle: f32) -> Quat {
        Quat::from_axis_angle(Vec3::new(1.0, 0.0, 0.0), angle)
    }

    /// Rotation by `angle` radians about the y axis.
    pub fn rotation_y(angle: f32) -> Quat {
        Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), angle)
    }

    /// Rotation by `angle` radians about the z axis.
    pub fn rotation_z(angle: f32) -> Quat {
        Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), angle)
    }

    /// This quaternion scaled to length 1. It must not be zero.
    pub fn normalized(self) -> Quat {
        let n = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        Quat {
            w: self.w / n,
            x: self.x / n,
            y: self.y / n,
            z: self.z / n,
        }
    }

    /// Rotates the vector `v` by this (unit) quaternion.
    pub fn rotate(self, v: Vec3) -> Vec3 {
        // v + 2w (u x v) + 2 u x (u x v), with u the vector part.
        let u = Vec3::new(self.x, self.y, self.z);
        let t = u.cross(v).scale(2.0);
        v + t.scale(self.w) + u.cross(t)
    }

    /// The rotation matrix of this (unit) quaternion.
    pub fn to_mat4(self) -> Mat4 {
        let Quat { w, x, y, z } = self;
        let (xx, yy, zz) = (x * x, y * y, z * z);
        let (xy, xz, yz) = (x * y, x * z, y * z);
        let (wx, wy, wz) = (w * x, w * y, w * z);
        Mat4([
            [1.0 - 2.0 * (yy + zz), 2.0 * (xy + wz), 2.0 * (xz - wy), 0.0],
            [2.0 * (xy - wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz + wx), 0.0],
            [2.0 * (xz + wy), 2.0 * (yz - wx), 1.0 - 2.0 * (xx + yy), 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }
}

/// `self * o`: rotates by `o` first, then by `self`.
impl Mul for Quat {
    type Output = Quat;

    fn mul(self, o: Quat) -> Quat {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }
}

/// A 4x4 matrix, column-major: `self.0[column][row]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4(pub [[f32; 4]; 4]);

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    pub fn translation(t: Vec3) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.0[3] = [t.x, t.y, t.z, 1.0];
        m
    }

    pub fn uniform_scale(s: f32) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.0[0][0] = s;
        m.0[1][1] = s;
        m.0[2][2] = s;
        m
    }

    /// Right-handed perspective projection to wgpu clip space (depth 0..1).
    /// `fov_y` is the vertical field of view in radians.
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fov_y / 2.0).tan();
        let range = near - far;
        Mat4([
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, far / range, -1.0],
            [0.0, 0.0, near * far / range, 0.0],
        ])
    }

    /// Transforms the point `p` (w = 1) and returns x, y, z and w.
    pub fn transform(&self, p: Vec3) -> [f32; 4] {
        let m = &self.0;
        std::array::from_fn(|row| m[0][row] * p.x + m[1][row] * p.y + m[2][row] * p.z + m[3][row])
    }
}

/// `self * o`: applies `o` first, then `self`.
impl Mul for Mat4 {
    type Output = Mat4;

    fn mul(self, o: Mat4) -> Mat4 {
        let mut r = [[0.0f32; 4]; 4];
        for (c, column) in r.iter_mut().enumerate() {
            for (row, value) in column.iter_mut().enumerate() {
                *value = (0..4).map(|k| self.0[k][row] * o.0[c][k]).sum();
            }
        }
        Mat4(r)
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI};

    use super::*;

    const EPS: f32 = 1e-5;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < EPS
    }

    /// The point `p` transformed by `m`, without the perspective divide.
    fn point(m: &Mat4, p: Vec3) -> Vec3 {
        let [x, y, z, _] = m.transform(p);
        Vec3::new(x, y, z)
    }

    /// The point `p` projected by `m` to normalized device coordinates.
    fn ndc(m: &Mat4, p: Vec3) -> Vec3 {
        let [x, y, z, w] = m.transform(p);
        Vec3::new(x / w, y / w, z / w)
    }

    #[test]
    fn vector_arithmetic() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(-4.0, 0.5, 2.0);
        assert_eq!(a + b, Vec3::new(-3.0, 2.5, 5.0));
        assert_eq!(a - b, Vec3::new(5.0, 1.5, 1.0));
        assert_eq!(a.scale(2.0), Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(a.dot(b), -4.0 + 1.0 + 6.0);
        assert_eq!(Vec3::new(3.0, 4.0, 0.0).length(), 5.0);
        assert!(close(
            Vec3::new(0.0, 3.0, 4.0).normalized(),
            Vec3::new(0.0, 0.6, 0.8)
        ));
    }

    #[test]
    fn cross_product_is_right_handed() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        let z = Vec3::new(0.0, 0.0, 1.0);
        assert_eq!(x.cross(y), z);
        assert_eq!(y.cross(z), x);
        assert_eq!(z.cross(x), y);
    }

    #[test]
    fn identity_changes_nothing() {
        let p = Vec3::new(1.0, -2.0, 3.0);
        assert_eq!(Quat::IDENTITY.rotate(p), p);
        assert_eq!(Quat::IDENTITY.to_mat4(), Mat4::IDENTITY);
        assert_eq!(point(&Mat4::IDENTITY, p), p);
        let q = Quat::rotation_y(0.7);
        assert_eq!(q * Quat::IDENTITY, q);
        assert_eq!(Quat::IDENTITY * q, q);
    }

    #[test]
    fn quarter_turns_about_the_axes() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        let z = Vec3::new(0.0, 0.0, 1.0);
        // Counterclockwise when the axis points toward the viewer.
        assert!(close(Quat::rotation_x(FRAC_PI_2).rotate(y), z));
        assert!(close(Quat::rotation_y(FRAC_PI_2).rotate(z), x));
        assert!(close(Quat::rotation_y(FRAC_PI_2).rotate(x), z.scale(-1.0)));
        assert!(close(Quat::rotation_z(FRAC_PI_2).rotate(x), y));
    }

    #[test]
    fn axis_angle_matches_the_axis_rotations() {
        let axis = Vec3::new(0.0, 0.0, 1.0);
        let a = Quat::from_axis_angle(axis, 0.4);
        let b = Quat::rotation_z(0.4);
        let p = Vec3::new(0.3, -1.2, 2.0);
        assert!(close(a.rotate(p), b.rotate(p)));
    }

    #[test]
    fn product_applies_the_right_factor_first() {
        // Rotate about y first, then about z.
        let q = Quat::rotation_z(FRAC_PI_2) * Quat::rotation_y(FRAC_PI_2);
        assert!(close(
            q.rotate(Vec3::new(1.0, 0.0, 0.0)),
            Vec3::new(0.0, 0.0, -1.0)
        ));
        assert!(close(
            q.rotate(Vec3::new(0.0, 1.0, 0.0)),
            Vec3::new(-1.0, 0.0, 0.0)
        ));
    }

    #[test]
    fn rotation_keeps_lengths_and_matches_its_matrix() {
        let q = Quat::from_axis_angle(Vec3::new(1.0, 2.0, -2.0).normalized(), 2.3);
        let m = q.to_mat4();
        for p in [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.2, -3.0, 1.5),
            Vec3::new(-4.0, 1.0, 0.25),
        ] {
            let r = q.rotate(p);
            assert!((r.length() - p.length()).abs() < EPS);
            assert!(close(point(&m, p), r));
        }
    }

    #[test]
    fn a_full_turn_is_no_rotation() {
        let q = Quat::from_axis_angle(Vec3::new(0.0, 0.6, 0.8), 2.0 * PI);
        let p = Vec3::new(1.0, 2.0, 3.0);
        assert!(close(q.rotate(p), p));
    }

    #[test]
    fn normalized_quaternion_has_unit_length() {
        let q = Quat {
            w: 2.0,
            x: 0.0,
            y: -2.0,
            z: 1.0,
        }
        .normalized();
        let n = q.w * q.w + q.x * q.x + q.y * q.y + q.z * q.z;
        assert!((n - 1.0).abs() < EPS);
        assert!((q.w - 2.0 / 3.0).abs() < EPS);
    }

    #[test]
    fn matrix_product_order_and_translation() {
        let t = Mat4::translation(Vec3::new(1.0, 2.0, 3.0));
        let s = Mat4::uniform_scale(2.0);
        let p = Vec3::new(1.0, 1.0, 1.0);
        // Scale first, then translate.
        assert!(close(point(&(t * s), p), Vec3::new(3.0, 4.0, 5.0)));
        // Translate first, then scale.
        assert!(close(point(&(s * t), p), Vec3::new(4.0, 6.0, 8.0)));
        assert_eq!(t * Mat4::IDENTITY, t);
        assert_eq!(Mat4::IDENTITY * t, t);
        assert_eq!(t.transform(p)[3], 1.0);
    }

    #[test]
    fn perspective_maps_near_and_far_to_zero_and_one() {
        let p = Mat4::perspective(1.0, 1.5, 1.0, 100.0);
        assert!(ndc(&p, Vec3::new(0.0, 0.0, -1.0)).z.abs() < 1e-6);
        assert!((ndc(&p, Vec3::new(0.0, 0.0, -100.0)).z - 1.0).abs() < 1e-6);
    }

    #[test]
    fn perspective_maps_the_field_of_view_to_the_screen_edges() {
        let fov_y = 40.0_f32.to_radians();
        let aspect = 16.0 / 9.0;
        let p = Mat4::perspective(fov_y, aspect, 1.0, 100.0);
        let d = 20.0;
        let half_h = d * (fov_y / 2.0).tan();
        let top_right = ndc(&p, Vec3::new(half_h * aspect, half_h, -d));
        assert!((top_right.x - 1.0).abs() < EPS, "{top_right:?}");
        assert!((top_right.y - 1.0).abs() < EPS, "{top_right:?}");
        // The screen center stays at the center.
        let center = ndc(&p, Vec3::new(0.0, 0.0, -d));
        assert!(center.x.abs() < EPS && center.y.abs() < EPS);
    }

    #[test]
    fn perspective_draws_farther_things_smaller() {
        let p = Mat4::perspective(40.0_f32.to_radians(), 1.0, 1.0, 100.0);
        // The on-screen width of a segment one unit wide at depth d.
        let width =
            |d: f32| ndc(&p, Vec3::new(0.5, 0.0, -d)).x - ndc(&p, Vec3::new(-0.5, 0.0, -d)).x;
        assert!(width(10.0) > width(20.0));
        // Twice as far is half as wide.
        assert!((width(10.0) - 2.0 * width(20.0)).abs() < EPS);
    }
}
