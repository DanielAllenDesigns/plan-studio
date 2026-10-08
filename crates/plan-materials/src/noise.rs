//! Deterministic, tileable noise primitives (no external crates).
//!
//! All lattice noise wraps modulo its period, so any texture built from
//! integer base frequencies tiles seamlessly.

/// SplitMix64 finaliser: a fast, well-mixed 64-bit hash.
pub(crate) fn hash64(x: u64) -> u64 {
    let mut x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Hash of an integer lattice cell plus a seed.
pub(crate) fn hash_cell(ix: i64, iy: i64, seed: u32) -> u64 {
    hash64(
        hash64(ix as u64 ^ (u64::from(seed) << 32))
            ^ (iy as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93),
    )
}

/// Maps the top bits of a hash to `[0, 1)`.
pub(crate) fn unit_f32(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Maps the top bits of a hash to `[0, 1)` in double precision.
pub(crate) fn unit_f64(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Periodic value noise in `[0, 1)`. `x`/`y` are lattice coordinates and the
/// lattice repeats every `px` / `py` cells.
pub(crate) fn value_noise(x: f32, y: f32, px: i64, py: i64, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (smooth(x - x0), smooth(y - y0));
    let (ix, iy) = (x0 as i64, y0 as i64);
    let at = |i: i64, j: i64| unit_f32(hash_cell(i.rem_euclid(px), j.rem_euclid(py), seed));
    let top = at(ix, iy) + (at(ix + 1, iy) - at(ix, iy)) * fx;
    let bot = at(ix, iy + 1) + (at(ix + 1, iy + 1) - at(ix, iy + 1)) * fx;
    top + (bot - top) * fy
}

/// Fractal value noise over a tile `(u, v)` in `[0, 1)²`, normalised to
/// roughly `[0, 1]`. `fx`/`fy` are the integer base frequencies of the first
/// octave; each further octave doubles them.
pub(crate) fn fbm(u: f32, v: f32, fx: i64, fy: i64, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
    for o in 0..octaves {
        let (px, py) = (fx << o, fy << o);
        sum += amp
            * value_noise(
                u * px as f32,
                v * py as f32,
                px,
                py,
                seed.wrapping_add(o * 101),
            );
        norm += amp;
        amp *= 0.5;
    }
    sum / norm
}

/// Nearest / second-nearest feature distances of a periodic Worley field.
pub(crate) struct Worley {
    pub f1: f32,
    pub f2: f32,
    /// Hash identifying the nearest feature cell.
    pub id: u64,
}

/// Periodic cellular noise with `nx` x `ny` feature points per tile.
pub(crate) fn worley(u: f32, v: f32, nx: i64, ny: i64, seed: u32) -> Worley {
    let (x, y) = (u * nx as f32, v * ny as f32);
    let (cx, cy) = (x.floor() as i64, y.floor() as i64);
    let mut w = Worley {
        f1: f32::MAX,
        f2: f32::MAX,
        id: 0,
    };
    for dj in -1..=1 {
        for di in -1..=1 {
            let (ci, cj) = (cx + di, cy + dj);
            let h = hash_cell(ci.rem_euclid(nx), cj.rem_euclid(ny), seed);
            let fx = ci as f32 + unit_f32(h);
            let fy = cj as f32 + unit_f32(h << 24);
            let d = ((x - fx).powi(2) + (y - fy).powi(2)).sqrt();
            if d < w.f1 {
                w.f2 = w.f1;
                w.f1 = d;
                w.id = h;
            } else if d < w.f2 {
                w.f2 = d;
            }
        }
    }
    w
}
