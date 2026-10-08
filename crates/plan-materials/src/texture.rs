//! Procedural, tileable texture bitmaps for 3D materials.

use std::f32::consts::TAU;

use crate::material::{MaterialDef, ProceduralKind, Texture};
use crate::noise::{fbm, hash64, unit_f32, value_noise, worley};
use crate::pattern::Pattern;

/// Default texture edge length in pixels.
pub const DEFAULT_TEXTURE_SIZE: u32 = 256;

/// Largest size [`render_texture`] will produce.
pub const MAX_TEXTURE_SIZE: u32 = 4096;

type Rgb = [f32; 3];

/// Per-material generation parameters shared by every pixel.
struct Ctx {
    base: Rgb,
    seed: u32,
    /// Real-world size of one tile in inches.
    scale: (f64, f64),
    pattern: Pattern,
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn mul(a: Rgb, k: f32) -> Rgb {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn rgb(c: [u8; 3]) -> Rgb {
    [
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
    ]
}

/// Renders `def` into a `size_px` x `size_px` RGBA8 bitmap (row-major,
/// top row first). Solid materials give a flat fill; procedural kinds tile
/// seamlessly in both directions. Alpha is `1 - transparency`. The result is
/// deterministic for a given material name and parameters. `size_px` is
/// clamped to [`MAX_TEXTURE_SIZE`]; `0` yields an empty vector.
pub fn render_texture(def: &MaterialDef, size_px: u32) -> Vec<u8> {
    let n = size_px.min(MAX_TEXTURE_SIZE) as usize;
    let alpha = ((1.0 - def.transparency).clamp(0.0, 1.0) * 255.0).round() as u8;
    let ctx = Ctx {
        base: rgb(def.color),
        seed: (hash64(def.name.bytes().fold(0xCBF2_9CE4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01B3)
        })) >> 32) as u32,
        scale: def.texture_scale_in,
        pattern: def.pattern.clone(),
    };
    let mut out = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        let v = (y as f32 + 0.5) / n as f32;
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32;
            let c = match &def.texture {
                Texture::Solid => ctx.base,
                Texture::Procedural(kind) => sample(kind, &ctx, u, v),
            };
            out.extend(c.iter().map(|&ch| (ch.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
            out.push(alpha);
        }
    }
    out
}

fn sample(kind: &ProceduralKind, c: &Ctx, u: f32, v: f32) -> Rgb {
    match kind {
        ProceduralKind::Wood {
            grain_color,
            ring_spacing,
        } => wood(c, u, v, rgb(*grain_color), *ring_spacing),
        ProceduralKind::Brick { mortar_color } => brick(c, u, v, rgb(*mortar_color)),
        ProceduralKind::Stucco { grain } => stucco(c, u, v, *grain),
        ProceduralKind::Concrete => concrete(c, u, v),
        ProceduralKind::Shingles => shingles(c, u, v),
        ProceduralKind::LapSiding => lap_siding(c, u, v),
        ProceduralKind::Tile { grout } => tile(c, u, v, rgb(*grout)),
        ProceduralKind::Carpet => carpet(c, u, v),
        ProceduralKind::Grass => grass(c, u, v),
        ProceduralKind::Stone => stone(c, u, v),
        ProceduralKind::Glass => glass(c, u, v),
        ProceduralKind::Metal => metal(c, u, v),
    }
}

/// Number of whole pieces of `piece_in` that fit in `tile_in`, at least `min`.
fn count(tile_in: f64, piece_in: f64, min: i64) -> i64 {
    if piece_in > 0.0 && tile_in.is_finite() {
        ((tile_in / piece_in).round() as i64).clamp(min, 64)
    } else {
        min
    }
}

fn wood(c: &Ctx, u: f32, v: f32, grain: Rgb, ring_spacing: f64) -> Rgb {
    let stripes = count(c.scale.0, ring_spacing, 2).min(48);
    let warp = fbm(u, v, 2, 3, c.seed, 3) - 0.5;
    let ring = (0.5 + 0.5 * (TAU * (u * stripes as f32 + warp * 1.4)).sin()).powf(1.6);
    let fibre = fbm(u, v, 24, 2, c.seed ^ 0x77, 2) - 0.5;
    let t = 0.6 * ring + 0.5 * fibre;
    mul(
        mix(c.base, grain, t),
        0.94 + 0.12 * fbm(u, v, 3, 3, c.seed ^ 5, 2),
    )
}

fn brick(c: &Ctx, u: f32, v: f32, mortar: Rgb) -> Rgb {
    let (len, h) = match c.pattern {
        Pattern::Brick { length, height } => (length, height),
        _ => (8.0, 2.25),
    };
    let nx = count(c.scale.0, len, 1);
    // Even course count keeps the half-brick stagger seamless vertically.
    let ny = (count(c.scale.1, h, 2) + 1) / 2 * 2;
    let by = v * ny as f32 + 0.25;
    let row = by.floor() as i64;
    let odd = row.rem_euclid(ny) % 2 == 1;
    let bx = u * nx as f32 + 0.25 + if odd { 0.5 } else { 0.0 };
    let col = bx.floor() as i64;
    let (fx, fy) = (bx.fract(), by.fract());
    let grain = fbm(u, v, 32, 32, c.seed, 3);
    if fx < 0.05 || fy < 0.16 {
        return mul(mortar, 0.9 + 0.2 * grain);
    }
    let id =
        hash64(((col.rem_euclid(nx) as u64) << 16) ^ row.rem_euclid(ny) as u64 ^ u64::from(c.seed));
    mul(c.base, 0.82 + 0.3 * unit_f32(id) + 0.18 * (grain - 0.5))
}

fn stucco(c: &Ctx, u: f32, v: f32, grain: f32) -> Rgb {
    let n = fbm(u, v, 48, 48, c.seed, 3) - 0.5;
    let speck = value_noise(u * 128.0, v * 128.0, 128, 128, c.seed ^ 9) - 0.5;
    mul(
        c.base,
        1.0 + (n * 0.5 + speck * 0.6) * grain.clamp(0.0, 1.0),
    )
}

fn concrete(c: &Ctx, u: f32, v: f32) -> Rgb {
    let cloud = fbm(u, v, 4, 4, c.seed, 4) - 0.5;
    let fine = fbm(u, v, 64, 64, c.seed ^ 3, 2) - 0.5;
    let pit = if value_noise(u * 40.0, v * 40.0, 40, 40, c.seed ^ 11) > 0.93 {
        0.78
    } else {
        1.0
    };
    mul(c.base, (1.0 + cloud * 0.28 + fine * 0.12) * pit)
}

fn shingles(c: &Ctx, u: f32, v: f32) -> Rgb {
    let (exposure, width) = match c.pattern {
        Pattern::Shingle { exposure, width } => (exposure, width),
        _ => (5.0, 12.0),
    };
    let nx = count(c.scale.0, width, 1);
    let ny = count(c.scale.1, exposure, 2);
    let by = v * ny as f32 + 0.3;
    let row = by.floor() as i64;
    let rid = hash64(row.rem_euclid(ny) as u64 ^ u64::from(c.seed));
    let bx = u * nx as f32 + 0.15 + 0.7 * unit_f32(rid);
    let col = bx.floor() as i64;
    let (fx, fy) = (bx.fract(), by.fract());
    let tid = hash64(rid ^ (col.rem_euclid(nx) as u64).wrapping_mul(0x9E37_79B9));
    let mut col_rgb = mul(c.base, 0.85 + 0.3 * unit_f32(tid));
    // Shadow under the course above, darkest at the bottom edge.
    col_rgb = mul(col_rgb, 0.55 + 0.45 * (fy / 0.2).min(1.0));
    if fx < 0.04 {
        col_rgb = mul(col_rgb, 0.6);
    }
    mul(col_rgb, 0.92 + 0.16 * fbm(u, v, 24, 24, c.seed, 2))
}

fn lap_siding(c: &Ctx, u: f32, v: f32) -> Rgb {
    let exposure = match c.pattern {
        Pattern::LapSiding { exposure } => exposure,
        _ => 6.0,
    };
    let ny = count(c.scale.1, exposure, 2);
    let by = v * ny as f32 + 0.3;
    let fy = by.fract();
    let grain = fbm(u, v, 3, ny * 8, c.seed, 2) - 0.5;
    let shade = if fy < 0.1 {
        0.62 + 3.8 * fy
    } else {
        1.02 - 0.12 * fy
    };
    mul(c.base, shade * (1.0 + 0.1 * grain))
}

fn tile(c: &Ctx, u: f32, v: f32, grout: Rgb) -> Rgb {
    let (tw, th) = match c.pattern {
        Pattern::Tile { w, h } => (w, h),
        _ => (c.scale.0 / 2.0, c.scale.1 / 2.0),
    };
    let (nx, ny) = (count(c.scale.0, tw, 1), count(c.scale.1, th, 1));
    let (bx, by) = (u * nx as f32 + 0.25, v * ny as f32 + 0.25);
    let (fx, fy) = (bx.fract(), by.fract());
    let noise = fbm(u, v, 16, 16, c.seed, 3) - 0.5;
    if fx < 0.03 / nx as f32 * 2.0 + 0.012 || fy < 0.03 / ny as f32 * 2.0 + 0.012 {
        return mul(grout, 0.95 + 0.1 * noise);
    }
    let id = hash64(
        ((bx.floor() as i64).rem_euclid(nx) as u64) << 8
            ^ (by.floor() as i64).rem_euclid(ny) as u64
            ^ u64::from(c.seed),
    );
    mul(c.base, 0.96 + 0.08 * unit_f32(id) + 0.1 * noise)
}

fn carpet(c: &Ctx, u: f32, v: f32) -> Rgb {
    let coarse = fbm(u, v, 48, 48, c.seed, 2);
    let fibre = value_noise(u * 160.0, v * 160.0, 160, 160, c.seed ^ 1);
    mul(c.base, 0.78 + 0.3 * coarse + 0.2 * fibre)
}

fn grass(c: &Ctx, u: f32, v: f32) -> Rgb {
    let blades = fbm(u, v, 96, 6, c.seed, 2);
    let patch = fbm(u, v, 3, 3, c.seed ^ 2, 3);
    let dark = mul(c.base, 0.55);
    let light = mix(c.base, [0.55, 0.75, 0.25], 0.35);
    mul(
        mix(dark, light, blades * 0.8 + patch * 0.4 - 0.1),
        0.95 + 0.1 * patch,
    )
}

fn stone(c: &Ctx, u: f32, v: f32) -> Rgb {
    let w = worley(u, v, 5, 5, c.seed);
    let rough = fbm(u, v, 24, 24, c.seed ^ 4, 3) - 0.5;
    if w.f2 - w.f1 < 0.09 {
        return mul(mix(c.base, [0.25, 0.24, 0.22], 0.75), 0.9 + 0.2 * rough);
    }
    // Slight dome shading toward each stone's edge.
    let dome = 1.05 - 0.25 * (w.f2 - w.f1).min(0.5);
    mul(
        c.base,
        (0.72 + 0.5 * unit_f32(w.id >> 7)) * dome * (1.0 + 0.25 * rough),
    )
}

fn glass(c: &Ctx, u: f32, v: f32) -> Rgb {
    let sheen = 0.5 + 0.5 * (TAU * (u + v)).sin();
    let streak = (-((TAU * (u + v)).sin() * 4.0).powi(2)).exp();
    mix(
        mul(c.base, 0.94 + 0.08 * sheen),
        [1.0, 1.0, 1.0],
        streak * 0.18,
    )
}

fn metal(c: &Ctx, u: f32, v: f32) -> Rgb {
    let brush = fbm(u, v, 2, 128, c.seed, 2);
    let band = 0.5 + 0.5 * (TAU * u).sin();
    mul(c.base, 0.84 + 0.22 * brush + 0.1 * band)
}
