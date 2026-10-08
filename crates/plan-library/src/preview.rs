//! A small software rasterizer that shades a [`Model3d`] into an RGBA
//! thumbnail for the Library Browser preview pane: orthographic view, z-buffer,
//! one headlight plus a fixed key light (Lambert), 2x2 supersampling.

use crate::model::Model3d;

/// A rendered preview: straight RGBA8, row-major, top row first. Pixels no
/// triangle covers are fully transparent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl Preview {
    /// Number of pixels a triangle covers.
    pub fn covered_pixels(&self) -> usize {
        self.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count()
    }
}

/// Color of parts that have none: a neutral blue-gray.
pub const DEFAULT_COLOR: [u8; 3] = [150, 160, 175];

const SS: usize = 2;
const MARGIN: f32 = 0.08;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(a: [f32; 3]) -> Option<[f32; 3]> {
    let l = dot(a, a).sqrt();
    (l.is_finite() && l > 1e-12).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

/// Renders `model` into a `size` x `size` thumbnail seen from `yaw_deg`
/// (turn about the vertical axis; 0 looks at the front, +Z) and `pitch_deg`
/// above the horizon. An empty model gives a fully transparent image.
pub fn render_preview(model: &Model3d, size: usize, yaw_deg: f32, pitch_deg: f32) -> Preview {
    let size = size.clamp(8, 1024);
    let mut out = Preview {
        width: size,
        height: size,
        rgba: vec![0; size * size * 4],
    };
    let Some((lo, hi)) = model.bounds() else {
        return out;
    };
    let center = [
        (lo[0] + hi[0]) * 0.5,
        (lo[1] + hi[1]) * 0.5,
        (lo[2] + hi[2]) * 0.5,
    ];
    // Camera basis: the eye sits at (sin yaw cos pitch, sin pitch, cos yaw
    // cos pitch) looking at the center; "right" is perpendicular, "up" the
    // rest.
    let (sy, cy) = (yaw_deg.to_radians().sin(), yaw_deg.to_radians().cos());
    let (sp, cp) = (pitch_deg.to_radians().sin(), pitch_deg.to_radians().cos());
    let eye = [sy * cp, sp, cy * cp];
    let right = [cy, 0.0, -sy];
    let up = cross(eye, right);

    let project = |p: [f32; 3]| -> [f32; 3] {
        let d = sub(p, center);
        [dot(d, right), dot(d, up), dot(d, eye)]
    };
    // Fit: the projected extent of the bounds corners.
    let mut min = [f32::MAX; 2];
    let mut max = [f32::MIN; 2];
    for k in 0..8 {
        let c = [
            if k & 1 == 0 { lo[0] } else { hi[0] },
            if k & 2 == 0 { lo[1] } else { hi[1] },
            if k & 4 == 0 { lo[2] } else { hi[2] },
        ];
        let q = project(c);
        for a in 0..2 {
            min[a] = min[a].min(q[a]);
            max[a] = max[a].max(q[a]);
        }
    }
    let span = (max[0] - min[0]).max(max[1] - min[1]).max(1e-6);
    let hi_res = size * SS;
    let scale = hi_res as f32 * (1.0 - 2.0 * MARGIN) / span;
    let mid = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
    let to_screen = |q: [f32; 3]| -> [f32; 3] {
        [
            hi_res as f32 * 0.5 + (q[0] - mid[0]) * scale,
            hi_res as f32 * 0.5 - (q[1] - mid[1]) * scale,
            q[2],
        ]
    };

    let key = normalize([0.4, 0.8, 0.5]).unwrap_or([0.0, 1.0, 0.0]);
    let mut color = vec![[0u8; 3]; hi_res * hi_res];
    let mut depth = vec![f32::MIN; hi_res * hi_res];
    let mut covered = vec![false; hi_res * hi_res];

    for part in &model.parts {
        let base = part.color.unwrap_or(DEFAULT_COLOR);
        for t in part.indices.as_chunks::<3>().0 {
            let get = |i: u32| part.positions.get(i as usize).copied();
            let (Some(a), Some(b), Some(c)) = (get(t[0]), get(t[1]), get(t[2])) else {
                continue;
            };
            let Some(n) = normalize(cross(sub(b, a), sub(c, a))) else {
                continue;
            };
            // Both sides shade (imported meshes often have flipped faces):
            // face the normal towards the eye.
            let facing = dot(n, eye);
            let n = if facing < 0.0 {
                [-n[0], -n[1], -n[2]]
            } else {
                n
            };
            let lambert = 0.35 + 0.35 * dot(n, eye).max(0.0) + 0.45 * dot(n, key).max(0.0);
            let shade = lambert.clamp(0.0, 1.0);
            let rgb = [
                (base[0] as f32 * shade) as u8,
                (base[1] as f32 * shade) as u8,
                (base[2] as f32 * shade) as u8,
            ];
            let (sa, sb, sc) = (
                to_screen(project(a)),
                to_screen(project(b)),
                to_screen(project(c)),
            );
            fill(
                &sa,
                &sb,
                &sc,
                hi_res,
                rgb,
                &mut color,
                &mut depth,
                &mut covered,
            );
        }
    }

    // Box-filter down; alpha is the covered fraction.
    for y in 0..size {
        for x in 0..size {
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let i = (y * SS + sy) * hi_res + x * SS + sx;
                    if covered[i] {
                        r += color[i][0] as u32;
                        g += color[i][1] as u32;
                        b += color[i][2] as u32;
                        n += 1;
                    }
                }
            }
            if let Some(n) = std::num::NonZeroU32::new(n) {
                let n = n.get();
                let o = (y * size + x) * 4;
                out.rgba[o] = (r / n) as u8;
                out.rgba[o + 1] = (g / n) as u8;
                out.rgba[o + 2] = (b / n) as u8;
                out.rgba[o + 3] = (255 * n / (SS * SS) as u32) as u8;
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn fill(
    a: &[f32; 3],
    b: &[f32; 3],
    c: &[f32; 3],
    n: usize,
    rgb: [u8; 3],
    color: &mut [[u8; 3]],
    depth: &mut [f32],
    covered: &mut [bool],
) {
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area.abs() < 1e-9 || !area.is_finite() {
        return;
    }
    let x0 = a[0].min(b[0]).min(c[0]).floor().max(0.0) as usize;
    let x1 = (a[0].max(b[0]).max(c[0]).ceil().max(0.0) as usize).min(n - 1);
    let y0 = a[1].min(b[1]).min(c[1]).floor().max(0.0) as usize;
    let y1 = (a[1].max(b[1]).max(c[1]).ceil().max(0.0) as usize).min(n - 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
            let w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
            let w2 = 1.0 - w0 - w1;
            let eps = -1e-4;
            if w0 < eps || w1 < eps || w2 < eps {
                continue;
            }
            let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
            let i = y * n + x;
            if z > depth[i] {
                depth[i] = z;
                color[i] = rgb;
                covered[i] = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_covers_pixels_and_an_empty_model_none() {
        let m = Model3d::box_model(36.0, 24.0, 30.0, Some([200, 40, 40]));
        let p = render_preview(&m, 64, 30.0, 25.0);
        assert_eq!((p.width, p.height, p.rgba.len()), (64, 64, 64 * 64 * 4));
        let n = p.covered_pixels();
        assert!(n > 600 && n < 64 * 64, "{n}");
        let empty = render_preview(&Model3d::default(), 32, 0.0, 0.0);
        assert_eq!(empty.covered_pixels(), 0);
    }

    #[test]
    fn rotating_the_view_changes_the_picture() {
        let m = Model3d::box_model(60.0, 10.0, 30.0, None);
        let a = render_preview(&m, 48, 0.0, 10.0);
        let b = render_preview(&m, 48, 90.0, 10.0);
        assert_ne!(a.rgba, b.rgba);
        // A wide thin box seen from the front fills more columns than from
        // the side.
        let cols = |p: &Preview| {
            (0..p.width)
                .filter(|&x| (0..p.height).any(|y| p.rgba[(y * p.width + x) * 4 + 3] > 0))
                .count()
        };
        assert!(cols(&a) >= cols(&b));
    }

    #[test]
    fn shading_depends_on_the_face_direction() {
        let m = Model3d::box_model(20.0, 20.0, 20.0, Some([200, 200, 200]));
        let p = render_preview(&m, 64, 35.0, 30.0);
        let mut seen = std::collections::BTreeSet::new();
        for px in p.rgba.as_chunks::<4>().0.iter().filter(|q| q[3] == 255) {
            seen.insert(px[0]);
        }
        assert!(seen.len() >= 3, "{seen:?}");
    }
}
