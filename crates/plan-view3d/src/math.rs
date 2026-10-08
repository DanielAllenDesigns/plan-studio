//! Minimal 3D math: column-major 4x4 matrices and `[f32; 3]` vector helpers.
//!
//! Matrices are stored the way OpenGL expects them: `m[column * 4 + row]`.
//! Clip space follows OpenGL conventions (right-handed view space looking down
//! -Z, NDC depth in `[-1, 1]`).

/// Column-major 4x4 matrix.
pub type Mat4 = [f32; 16];
/// 3-component vector.
pub type Vec3 = [f32; 3];

/// The identity matrix.
pub const IDENTITY: Mat4 = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// Matrix product `a * b` (apply `b` first, then `a`).
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [0.0; 16];
    for col in 0..4 {
        for row in 0..4 {
            out[col * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[col * 4 + k]).sum();
        }
    }
    out
}

/// Perspective projection. `fov_y` is the vertical field of view in radians.
pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fov_y * 0.5).tan();
    let mut m = [0.0; 16];
    m[0] = f / aspect;
    m[5] = f;
    m[10] = (far + near) / (near - far);
    m[11] = -1.0;
    m[14] = 2.0 * far * near / (near - far);
    m
}

/// Orthographic (parallel) projection. `near`/`far` are distances along the
/// view direction and may be negative to include geometry behind the eye.
pub fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    let mut m = IDENTITY;
    m[0] = 2.0 / (right - left);
    m[5] = 2.0 / (top - bottom);
    m[10] = -2.0 / (far - near);
    m[12] = -(right + left) / (right - left);
    m[13] = -(top + bottom) / (top - bottom);
    m[14] = -(far + near) / (far - near);
    m
}

/// View matrix looking from `eye` toward `target` with the given `up` hint.
pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    let f = normalize(sub(target, eye));
    let s = normalize(cross(f, up));
    let u = cross(s, f);
    [
        s[0],
        u[0],
        -f[0],
        0.0,
        s[1],
        u[1],
        -f[1],
        0.0,
        s[2],
        u[2],
        -f[2],
        0.0,
        -dot(s, eye),
        -dot(u, eye),
        dot(f, eye),
        1.0,
    ]
}

/// Multiply a homogeneous `[x, y, z, w]` vector by `m`.
pub fn transform_vec4(m: &Mat4, v: [f32; 4]) -> [f32; 4] {
    let mut out = [0.0; 4];
    for (row, o) in out.iter_mut().enumerate() {
        *o = (0..4).map(|k| m[k * 4 + row] * v[k]).sum();
    }
    out
}

/// Transform a point (w = 1) and perform the perspective divide.
pub fn transform_point(m: &Mat4, p: Vec3) -> Vec3 {
    let r = transform_vec4(m, [p[0], p[1], p[2], 1.0]);
    let inv_w = if r[3].abs() > f32::EPSILON {
        1.0 / r[3]
    } else {
        1.0
    };
    [r[0] * inv_w, r[1] * inv_w, r[2] * inv_w]
}

/// `a + b`
pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `a - b`
pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a * k`
pub fn scale(a: Vec3, k: f32) -> Vec3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// Dot product.
pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Cross product.
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Euclidean length.
pub fn length(a: Vec3) -> f32 {
    dot(a, a).sqrt()
}

/// Unit vector in the direction of `a`, or `a` unchanged when it is ~zero.
pub fn normalize(a: Vec3) -> Vec3 {
    let l = length(a);
    if l > 1e-12 {
        scale(a, 1.0 / l)
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn identity_is_neutral() {
        let m = perspective(1.0, 1.5, 0.5, 100.0);
        assert_eq!(mul(&IDENTITY, &m), m);
        assert_eq!(mul(&m, &IDENTITY), m);
        assert_eq!(transform_point(&IDENTITY, [1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn perspective_maps_near_and_far_to_ndc_bounds() {
        let (near, far) = (2.0, 500.0);
        let p = perspective(60f32.to_radians(), 1.0, near, far);
        assert!(approx(transform_point(&p, [0.0, 0.0, -near])[2], -1.0));
        assert!(approx(transform_point(&p, [0.0, 0.0, -far])[2], 1.0));
        // Nearer than the near plane lies outside the clip volume.
        assert!(transform_point(&p, [0.0, 0.0, -1.0])[2] < -1.0);
    }

    #[test]
    fn look_at_puts_target_in_front_of_the_eye() {
        let v = look_at([0.0, 0.0, 10.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let t = transform_point(&v, [0.0, 0.0, 0.0]);
        assert!(approx(t[0], 0.0) && approx(t[1], 0.0));
        assert!(
            approx(t[2], -10.0),
            "target should sit at -distance on view Z"
        );
        // The eye itself maps to the origin.
        let e = transform_point(&v, [0.0, 0.0, 10.0]);
        assert!(e.iter().all(|c| approx(*c, 0.0)));
    }

    #[test]
    fn ortho_unit_box_maps_to_ndc() {
        let o = ortho(-2.0, 2.0, -1.0, 1.0, 0.0, 10.0);
        let p = transform_point(&o, [2.0, 1.0, -10.0]);
        assert!(approx(p[0], 1.0) && approx(p[1], 1.0) && approx(p[2], 1.0));
    }
}
