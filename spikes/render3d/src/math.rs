//! Minimal vector, quaternion and matrix math, written by hand so the spike
//! needs no math crate (plan 0005, D-3 and E-5).
//!
//! Conventions: right-handed, y up, the camera looks along -z. Matrices are
//! column-major (`m[column][row]`), as WGSL expects, and act on column
//! vectors. Clip-space depth goes from 0 (near) to 1 (far), as in wgpu.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

impl Vec3 {
    pub fn add(self, o: Vec3) -> Vec3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    // Why: only the tests use it for now.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn sub(self, o: Vec3) -> Vec3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn scale(self, s: f32) -> Vec3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn normalized(self) -> Vec3 {
        self.scale(1.0 / self.length())
    }
    pub fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
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

    /// `self * o`: rotates by `o` first, then by `self`.
    pub fn mul(self, o: Quat) -> Quat {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }

    pub fn normalized(self) -> Quat {
        let n = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        Quat {
            w: self.w / n,
            x: self.x / n,
            y: self.y / n,
            z: self.z / n,
        }
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

/// A 4x4 matrix, column-major.
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

    /// `self * o`: applies `o` first, then `self`.
    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let mut r = [[0.0f32; 4]; 4];
        for (c, column) in r.iter_mut().enumerate() {
            for (row, value) in column.iter_mut().enumerate() {
                *value = (0..4).map(|k| self.0[k][row] * o.0[c][k]).sum();
            }
        }
        Mat4(r)
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let m = &self.0;
        v3(
            m[0][0] * p.x + m[1][0] * p.y + m[2][0] * p.z + m[3][0],
            m[0][1] * p.x + m[1][1] * p.y + m[2][1] * p.z + m[3][1],
            m[0][2] * p.x + m[1][2] * p.y + m[2][2] * p.z + m[3][2],
        )
    }

    pub fn write_to(&self, out: &mut Vec<u8>) {
        for column in &self.0 {
            for value in column {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
}

/// Rotation about one of the coordinate axes, in radians.
pub fn rot_x(a: f32) -> Quat {
    Quat::from_axis_angle(v3(1.0, 0.0, 0.0), a)
}
pub fn rot_y(a: f32) -> Quat {
    Quat::from_axis_angle(v3(0.0, 1.0, 0.0), a)
}
pub fn rot_z(a: f32) -> Quat {
    Quat::from_axis_angle(v3(0.0, 0.0, 1.0), a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.sub(b).length() < 1e-5
    }

    #[test]
    fn quaternion_matrix_matches_axis_rotation() {
        // A quarter turn about +y takes +x to -z.
        let m = rot_y(std::f32::consts::FRAC_PI_2).to_mat4();
        assert!(close(
            m.transform_point(v3(1.0, 0.0, 0.0)),
            v3(0.0, 0.0, -1.0)
        ));
        // A quarter turn about +z takes +x to +y.
        let m = rot_z(std::f32::consts::FRAC_PI_2).to_mat4();
        assert!(close(
            m.transform_point(v3(1.0, 0.0, 0.0)),
            v3(0.0, 1.0, 0.0)
        ));
    }

    #[test]
    fn quaternion_product_order() {
        // Rotate about y first, then about z.
        let q = rot_z(std::f32::consts::FRAC_PI_2).mul(rot_y(std::f32::consts::FRAC_PI_2));
        let p = q.to_mat4().transform_point(v3(1.0, 0.0, 0.0));
        assert!(close(p, v3(0.0, 0.0, -1.0)));
        let p = q.to_mat4().transform_point(v3(0.0, 1.0, 0.0));
        assert!(close(p, v3(-1.0, 0.0, 0.0)));
    }

    #[test]
    fn perspective_maps_near_and_far_to_zero_and_one() {
        let p = Mat4::perspective(1.0, 1.5, 1.0, 100.0);
        let depth = |z: f32| {
            let m = &p.0;
            let clip_z = m[2][2] * z + m[3][2];
            let clip_w = m[2][3] * z;
            clip_z / clip_w
        };
        assert!((depth(-1.0) - 0.0).abs() < 1e-6);
        assert!((depth(-100.0) - 1.0).abs() < 1e-6);
    }
}
