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
}
