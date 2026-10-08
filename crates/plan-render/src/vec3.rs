//! Minimal 3-vector used throughout the renderer (`f32`, scene inches).

use std::ops::{Add, AddAssign, Div, Index, Mul, MulAssign, Neg, Sub};

/// A 3-component vector doubling as a linear RGB colour.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl V3 {
    pub const ZERO: V3 = V3::new(0.0, 0.0, 0.0);
    pub const ONE: V3 = V3::new(1.0, 1.0, 1.0);

    pub const fn new(x: f32, y: f32, z: f32) -> V3 {
        V3 { x, y, z }
    }

    pub const fn splat(v: f32) -> V3 {
        V3::new(v, v, v)
    }

    pub const fn from_array(a: [f32; 3]) -> V3 {
        V3::new(a[0], a[1], a[2])
    }

    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: V3) -> V3 {
        V3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length_sq(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }

    /// Unit-length copy; the zero vector is returned unchanged.
    pub fn normalized(self) -> V3 {
        let len = self.length();
        if len > 0.0 {
            self / len
        } else {
            self
        }
    }

    /// Largest component.
    pub fn max_comp(self) -> f32 {
        self.x.max(self.y).max(self.z)
    }

    /// Clamp every component to at least `m`.
    pub fn max_each(self, m: f32) -> V3 {
        V3::new(self.x.max(m), self.y.max(m), self.z.max(m))
    }

    /// Clamp every component to at most `m`.
    pub fn min_each(self, m: f32) -> V3 {
        V3::new(self.x.min(m), self.y.min(m), self.z.min(m))
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// Linear interpolation: `self` at `t = 0`, `o` at `t = 1`.
    pub fn lerp(self, o: V3, t: f32) -> V3 {
        self * (1.0 - t) + o * t
    }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl AddAssign for V3 {
    fn add_assign(&mut self, o: V3) {
        *self = *self + o;
    }
}

impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, k: f32) -> V3 {
        V3::new(self.x * k, self.y * k, self.z * k)
    }
}

/// Component-wise product (colour modulation).
impl Mul for V3 {
    type Output = V3;
    fn mul(self, o: V3) -> V3 {
        V3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

impl MulAssign for V3 {
    fn mul_assign(&mut self, o: V3) {
        *self = *self * o;
    }
}

impl Div<f32> for V3 {
    type Output = V3;
    fn div(self, k: f32) -> V3 {
        V3::new(self.x / k, self.y / k, self.z / k)
    }
}

impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        V3::new(-self.x, -self.y, -self.z)
    }
}

impl Index<usize> for V3 {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        match i {
            0 => &self.x,
            1 => &self.y,
            _ => &self.z,
        }
    }
}

/// Two unit vectors completing `n` (unit) to a right-handed orthonormal basis.
pub(crate) fn basis(n: V3) -> (V3, V3) {
    let sign = 1.0_f32.copysign(n.z);
    let a = -1.0 / (sign + n.z);
    let b = n.x * n.y * a;
    (
        V3::new(1.0 + sign * n.x * n.x * a, sign * b, -sign * n.x),
        V3::new(b, sign + n.y * n.y * a, -n.y),
    )
}

/// Rotate a vector given in the frame whose +Z is `n` into world space.
pub(crate) fn to_world(local: V3, n: V3) -> V3 {
    let (t, b) = basis(n);
    t * local.x + b * local.y + n * local.z
}
