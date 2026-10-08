//! Small edge-preserving filter for noisy low-sample renders.

const RADIUS: i32 = 2;
const SIGMA_SPATIAL: f32 = 1.5;
const SIGMA_RANGE: f32 = 0.15;

/// Bilateral filter; similarity is judged on `x / (1 + x)` so bright
/// sun patches do not dominate the range term.
pub(crate) fn bilateral(hdr: &[[f32; 3]], width: usize, height: usize) -> Vec<[f32; 3]> {
    let squash = |c: [f32; 3]| c.map(|v| v / (1.0 + v));
    let mut out = Vec::with_capacity(hdr.len());
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let centre = hdr[y as usize * width + x as usize];
            let centre_s = squash(centre);
            let mut sum = [0.0_f32; 3];
            let mut weight = 0.0_f32;
            for dy in -RADIUS..=RADIUS {
                for dx in -RADIUS..=RADIUS {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }
                    let px = hdr[ny as usize * width + nx as usize];
                    let ps = squash(px);
                    let range: f32 = (0..3).map(|k| (ps[k] - centre_s[k]).powi(2)).sum();
                    let space = (dx * dx + dy * dy) as f32;
                    let w = (-space / (2.0 * SIGMA_SPATIAL.powi(2))
                        - range / (2.0 * SIGMA_RANGE.powi(2)))
                    .exp();
                    for k in 0..3 {
                        sum[k] += px[k] * w;
                    }
                    weight += w;
                }
            }
            out.push(sum.map(|s| s / weight));
        }
    }
    out
}

/// Per-pixel first-hit guide buffers for [`guided`] (all `width * height` long).
pub(crate) struct Guides<'a> {
    pub albedo: &'a [[f32; 3]],
    pub normal: &'a [[f32; 3]],
    /// Distance to the first opaque hit; infinite for sky.
    pub depth: &'a [f32],
}

const G_RADIUS: i32 = 4;
const G_SIGMA_SPATIAL: f32 = 2.5;
/// Sharpness of the normal similarity `max(0, n . n')^k`.
const G_NORMAL_POWER: i32 = 24;
const G_SIGMA_ALBEDO: f32 = 0.08;
/// Range sigma on the squashed demodulated radiance (shadow edges).
const G_SIGMA_RANGE: f32 = 0.22;
/// Albedo floor when demodulating, so dark surfaces do not amplify noise.
const G_ALBEDO_FLOOR: f32 = 0.06;

/// Edge-aware denoiser guided by the first hit's albedo, normal and depth.
///
/// The radiance is divided by the albedo (so texture detail is not blurred),
/// smoothed with a joint bilateral filter that stops at changes of surface
/// (normal, depth, albedo) and at strong shadow edges, then multiplied by the
/// albedo again.
pub(crate) fn guided(
    hdr: &[[f32; 3]],
    g: &Guides<'_>,
    width: usize,
    height: usize,
) -> Vec<[f32; 3]> {
    let squash = |c: [f32; 3]| c.map(|v| v / (1.0 + v));
    let floor = |a: [f32; 3]| a.map(|v| v.max(G_ALBEDO_FLOOR));
    let sky = |i: usize| !g.depth[i].is_finite();
    let irr: Vec<[f32; 3]> = (0..hdr.len())
        .map(|i| {
            if sky(i) {
                hdr[i]
            } else {
                let a = floor(g.albedo[i]);
                [hdr[i][0] / a[0], hdr[i][1] / a[1], hdr[i][2] / a[2]]
            }
        })
        .collect();
    let mut out = Vec::with_capacity(hdr.len());
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let ci = y as usize * width + x as usize;
            let cs = squash(irr[ci]);
            let (cn, cd, ca) = (g.normal[ci], g.depth[ci], g.albedo[ci]);
            let mut sum = [0.0_f32; 3];
            let mut weight = 0.0_f32;
            for dy in -G_RADIUS..=G_RADIUS {
                for dx in -G_RADIUS..=G_RADIUS {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }
                    let ni = ny as usize * width + nx as usize;
                    let mut w =
                        (-((dx * dx + dy * dy) as f32) / (2.0 * G_SIGMA_SPATIAL.powi(2))).exp();
                    if sky(ci) != sky(ni) {
                        continue;
                    }
                    if !sky(ci) {
                        let nn = g.normal[ni];
                        let cos = (cn[0] * nn[0] + cn[1] * nn[1] + cn[2] * nn[2]).max(0.0);
                        w *= cos.powi(G_NORMAL_POWER);
                        let dz = (g.depth[ni] - cd).abs();
                        let sz = 0.08 * cd + 2.0;
                        w *= (-(dz * dz) / (2.0 * sz * sz)).exp();
                        let da: f32 = (0..3).map(|k| (g.albedo[ni][k] - ca[k]).powi(2)).sum();
                        w *= (-da / (2.0 * G_SIGMA_ALBEDO.powi(2))).exp();
                    }
                    let ns = squash(irr[ni]);
                    let dr: f32 = (0..3).map(|k| (ns[k] - cs[k]).powi(2)).sum();
                    w *= (-dr / (2.0 * G_SIGMA_RANGE.powi(2))).exp();
                    for k in 0..3 {
                        sum[k] += irr[ni][k] * w;
                    }
                    weight += w;
                }
            }
            let filtered = if weight > 1e-9 {
                sum.map(|s| s / weight)
            } else {
                irr[ci]
            };
            out.push(if sky(ci) {
                filtered
            } else {
                let a = floor(g.albedo[ci]);
                [filtered[0] * a[0], filtered[1] * a[1], filtered[2] * a[2]]
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooths_noise_but_keeps_a_hard_edge() {
        let (w, h) = (16, 8);
        let mut img = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let base = if x < 8 { 0.1 } else { 2.0 };
                let noise = if (x + y) % 2 == 0 { 0.02 } else { -0.02 };
                img.push([base + noise; 3]);
            }
        }
        let out = bilateral(&img, w, h);
        let left = out[4 * w + 3][0];
        let right = out[4 * w + 12][0];
        assert!((left - 0.1).abs() < 0.01, "left {left}");
        assert!((right - 2.0).abs() < 0.05, "right {right}");
    }

    /// Deterministic multiplicative noise in `[1 - amp, 1 + amp]`.
    fn noise(i: usize, amp: f32) -> f32 {
        let mut x = (i as u32).wrapping_mul(2_654_435_761).wrapping_add(12_345);
        x ^= x >> 15;
        x = x.wrapping_mul(2_246_822_519);
        x ^= x >> 13;
        1.0 + amp * ((x & 0xFFFF) as f32 / 32_768.0 - 1.0)
    }

    fn rms(
        img: &[[f32; 3]],
        truth: &[[f32; 3]],
        cols: std::ops::Range<usize>,
        w: usize,
        h: usize,
    ) -> f32 {
        let mut s = 0.0;
        let mut n = 0.0;
        for y in 0..h {
            for x in cols.clone() {
                s += (img[y * w + x][0] - truth[y * w + x][0]).powi(2);
                n += 1.0;
            }
        }
        (s / n).sqrt()
    }

    /// A synthetic frame: the true values, the noisy ones and the guides.
    struct Frame {
        truth: Vec<[f32; 3]>,
        noisy: Vec<[f32; 3]>,
        albedo: Vec<[f32; 3]>,
        normal: Vec<[f32; 3]>,
        depth: Vec<f32>,
    }

    impl Frame {
        fn guides(&self) -> Guides<'_> {
            Guides {
                albedo: &self.albedo,
                normal: &self.normal,
                depth: &self.depth,
            }
        }
    }

    /// Two surfaces side by side: dark floor (left), bright wall (right).
    fn two_surfaces(irr_left: f32, irr_right: f32, w: usize, h: usize) -> Frame {
        let mut f = Frame {
            truth: vec![],
            noisy: vec![],
            albedo: vec![],
            normal: vec![],
            depth: vec![],
        };
        for y in 0..h {
            for x in 0..w {
                let left = x < w / 2;
                let (a, n, irr) = if left {
                    (0.2, [0.0, 1.0, 0.0], irr_left)
                } else {
                    (0.8, [1.0, 0.0, 0.0], irr_right)
                };
                let t = a * irr;
                f.truth.push([t; 3]);
                f.noisy.push([t * noise(y * w + x, 0.5); 3]);
                f.albedo.push([a; 3]);
                f.normal.push(n);
                f.depth.push(120.0);
            }
        }
        f
    }

    #[test]
    fn guided_denoiser_removes_noise_but_not_the_edge_between_surfaces() {
        let (w, h) = (32, 16);
        let f = two_surfaces(1.0, 2.0, w, h);
        let (truth, noisy) = (&f.truth, &f.noisy);
        let out = guided(noisy, &f.guides(), w, h);
        for cols in [2..w / 2 - 3, w / 2 + 3..w - 2] {
            let (before, after) = (
                rms(noisy, truth, cols.clone(), w, h),
                rms(&out, truth, cols, w, h),
            );
            assert!(after < before * 0.4, "noise {before} -> {after}");
        }
        // The pixels next to the edge keep their own surface's value.
        let mid = h / 2;
        let (l, r) = (out[mid * w + w / 2 - 1][0], out[mid * w + w / 2][0]);
        assert!((l - 0.2).abs() < 0.06, "left of the edge {l}");
        assert!((r - 1.6).abs() < 0.4, "right of the edge {r}");
        assert!(r > 4.0 * l, "edge survived: {l} vs {r}");
    }

    #[test]
    fn guided_denoiser_keeps_a_shadow_edge_on_one_surface() {
        // One surface throughout; a hard shadow halves the way across.
        let (w, h) = (32, 16);
        let (mut truth, mut noisy) = (vec![], vec![]);
        for y in 0..h {
            for x in 0..w {
                let t = if x < w / 2 { 0.15 } else { 1.5 };
                truth.push([t; 3]);
                noisy.push([t * noise(y * w + x + 99, 0.3); 3]);
            }
        }
        let alb = vec![[0.5; 3]; w * h];
        let nor = vec![[0.0, 1.0, 0.0]; w * h];
        let dep = vec![80.0; w * h];
        let out = guided(
            &noisy,
            &Guides {
                albedo: &alb,
                normal: &nor,
                depth: &dep,
            },
            w,
            h,
        );
        let mid = h / 2;
        let (l, r) = (out[mid * w + w / 2 - 1][0], out[mid * w + w / 2][0]);
        assert!(l < 0.4 && r > 1.1, "shadow edge {l} | {r}");
        assert!(rms(&out, &truth, 3..w / 2 - 3, w, h) < rms(&noisy, &truth, 3..w / 2 - 3, w, h));
    }

    #[test]
    fn sky_pixels_are_filtered_with_sky_only() {
        let (w, h) = (8, 4);
        let hdr = vec![[0.5; 3]; w * h];
        let alb = vec![[0.0; 3]; w * h];
        let nor = vec![[0.0; 3]; w * h];
        let dep = vec![f32::INFINITY; w * h];
        let out = guided(
            &hdr,
            &Guides {
                albedo: &alb,
                normal: &nor,
                depth: &dep,
            },
            w,
            h,
        );
        assert!(out.iter().all(|p| (p[0] - 0.5).abs() < 1e-4));
    }
}
