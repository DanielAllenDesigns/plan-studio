//! Rendering-technique post-process for finished pictures (Chief's Vector
//! View, Technical Illustration, Line Drawing and Watercolor).
//!
//! The interactive view draws these looks in its own shaders; the path tracer
//! has only Physically Based and Clay. [`stylize`] turns any rendered or
//! captured RGBA frame into the technique's look with an edge detector (a
//! Sobel filter on lightness) and flat shading tiers, so a final view, a
//! recorded walkthrough and a panorama can all honour the camera's technique.
//!
//! Everything is a pure function of the pixels: the same frame always gives
//! the same bytes.

/// A technique's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Hidden-line drawing: three flat grey tiers and black outlines.
    VectorView,
    /// Material colour in four flat tiers with bold dark outlines.
    TechnicalIllustration,
    /// Black lines on white, nothing else.
    LineDrawing,
    /// Pigment washes: soft colour, paper grain and brown bleeding edges.
    Watercolor,
}

impl Style {
    /// Every style.
    pub const ALL: [Style; 4] = [
        Style::VectorView,
        Style::TechnicalIllustration,
        Style::LineDrawing,
        Style::Watercolor,
    ];

    /// The style for a rendering technique's menu name; `None` for the
    /// techniques that are not post-processed.
    pub fn from_technique_name(name: &str) -> Option<Style> {
        match name {
            "Vector View" => Some(Style::VectorView),
            "Technical Illustration" => Some(Style::TechnicalIllustration),
            "Line Drawing" => Some(Style::LineDrawing),
            "Watercolor" => Some(Style::Watercolor),
            _ => None,
        }
    }
}

/// Lightness 0..1 of an sRGB pixel.
fn luma(p: &[u8]) -> f32 {
    (0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])) / 255.0
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Sobel gradient magnitude of the lightness of `rgba`, 0..~1.4.
pub fn edge_strength(rgba: &[u8], w: usize, h: usize) -> Vec<f32> {
    let l: Vec<f32> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .take(w * h)
        .map(|p| luma(p))
        .collect();
    let at = |x: isize, y: isize| -> f32 {
        let (x, y) = (x.clamp(0, w as isize - 1), y.clamp(0, h as isize - 1));
        l[y as usize * w + x as usize]
    };
    let mut out = vec![0.0; w * h];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let gx = -at(x - 1, y - 1) - 2.0 * at(x - 1, y) - at(x - 1, y + 1)
                + at(x + 1, y - 1)
                + 2.0 * at(x + 1, y)
                + at(x + 1, y + 1);
            let gy = -at(x - 1, y - 1) - 2.0 * at(x, y - 1) - at(x + 1, y - 1)
                + at(x - 1, y + 1)
                + 2.0 * at(x, y + 1)
                + at(x + 1, y + 1);
            out[y as usize * w + x as usize] = (gx * gx + gy * gy).sqrt() * 0.25;
        }
    }
    out
}

/// Thickens lines: each pixel takes the strongest value of its 3 x 3
/// neighbourhood (a cheap dilation).
fn dilate(v: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut out = v.to_vec();
    for y in 0..h {
        for x in 0..w {
            let mut m = 0.0_f32;
            for dy in -1_isize..=1 {
                for dx in -1_isize..=1 {
                    let (sx, sy) = (x as isize + dx, y as isize + dy);
                    if sx >= 0 && sy >= 0 && (sx as usize) < w && (sy as usize) < h {
                        m = m.max(v[sy as usize * w + sx as usize]);
                    }
                }
            }
            out[y * w + x] = m;
        }
    }
    out
}

/// Box blur of the colour channels with the given radius (alpha is kept).
fn box_blur(rgba: &[u8], w: usize, h: usize, radius: usize) -> Vec<u8> {
    let mut tmp = rgba.to_vec();
    let mut out = rgba.to_vec();
    let r = radius as isize;
    for pass in 0..2 {
        let (src, dst): (&[u8], &mut [u8]) = if pass == 0 {
            (rgba, &mut tmp)
        } else {
            (&tmp, &mut out)
        };
        for y in 0..h as isize {
            for x in 0..w as isize {
                let mut sum = [0_u32; 3];
                let mut n = 0_u32;
                for d in -r..=r {
                    let (sx, sy) = if pass == 0 { (x + d, y) } else { (x, y + d) };
                    if sx < 0 || sy < 0 || sx >= w as isize || sy >= h as isize {
                        continue;
                    }
                    let i = (sy as usize * w + sx as usize) * 4;
                    for c in 0..3 {
                        sum[c] += u32::from(src[i + c]);
                    }
                    n += 1;
                }
                let o = (y as usize * w + x as usize) * 4;
                for c in 0..3 {
                    dst[o + c] = (sum[c] / n.max(1)) as u8;
                }
                dst[o + 3] = src[o + 3];
            }
        }
    }
    out
}

/// Deterministic value noise 0..1 at pixel `(x, y)` for grain of `cell` pixels.
fn grain(x: usize, y: usize, cell: usize) -> f32 {
    fn hash(mut h: u32) -> f32 {
        h ^= h >> 16;
        h = h.wrapping_mul(0x7FEB_352D);
        h ^= h >> 15;
        h = h.wrapping_mul(0x846C_A68B);
        h ^= h >> 16;
        (h & 0xFFFF) as f32 / 65_535.0
    }
    let (cx, cy) = (x / cell, y / cell);
    let (fx, fy) = (
        (x % cell) as f32 / cell as f32,
        (y % cell) as f32 / cell as f32,
    );
    let v = |gx: usize, gy: usize| {
        hash((gx as u32).wrapping_mul(73_856_093) ^ (gy as u32).wrapping_mul(19_349_663))
    };
    let top = v(cx, cy) * (1.0 - fx) + v(cx + 1, cy) * fx;
    let bottom = v(cx, cy + 1) * (1.0 - fx) + v(cx + 1, cy + 1) * fx;
    top * (1.0 - fy) + bottom * fy
}

fn to_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// `rgba` (`w * h * 4` bytes, rows top to bottom) in the look of `style`.
/// Alpha is kept. A buffer of the wrong size is returned unchanged.
pub fn stylize(rgba: &[u8], w: u32, h: u32, style: Style) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return rgba.to_vec();
    }
    let edges = edge_strength(rgba, w, h);
    let mut out = rgba.to_vec();
    match style {
        Style::LineDrawing => {
            let line = dilate(
                &edges
                    .iter()
                    .map(|&g| smoothstep(0.07, 0.22, g))
                    .collect::<Vec<_>>(),
                w,
                h,
            );
            for (i, p) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let v = to_byte(1.0 - line[i]);
                p[..3].fill(v);
            }
        }
        Style::VectorView => {
            let line = dilate(
                &edges
                    .iter()
                    .map(|&g| smoothstep(0.08, 0.25, g))
                    .collect::<Vec<_>>(),
                w,
                h,
            );
            for (i, p) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let l = luma(&rgba[i * 4..i * 4 + 4]);
                // Three flat tiers: lit, half shade and shade.
                let tier = if l > 0.62 {
                    0.97
                } else if l > 0.34 {
                    0.82
                } else {
                    0.66
                };
                let v = to_byte(tier * (1.0 - line[i]));
                p[..3].fill(v);
            }
        }
        Style::TechnicalIllustration => {
            let line = dilate(
                &edges
                    .iter()
                    .map(|&g| smoothstep(0.06, 0.2, g))
                    .collect::<Vec<_>>(),
                w,
                h,
            );
            for (i, p) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let src = &rgba[i * 4..i * 4 + 4];
                let l = luma(src).max(1e-3);
                // Material colour at four flat lightness steps.
                let tier = ((l * 4.0).ceil() / 4.0).clamp(0.25, 1.0);
                let k = tier / l;
                for c in 0..3 {
                    let v = f32::from(src[c]) / 255.0 * k;
                    p[c] = to_byte(v * (1.0 - line[i]) + 0.04 * line[i]);
                }
            }
        }
        Style::Watercolor => {
            let blurred = box_blur(&box_blur(rgba, w, h, 2), w, h, 1);
            let paper = [0.97_f32, 0.95, 0.90];
            let ink = [0.38_f32, 0.30, 0.22];
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    let wash = 0.9 * grain(x, y, 6) + 0.1 * grain(x, y, 2);
                    let bleed = smoothstep(0.04, 0.2, edges[i]) * 0.75;
                    for c in 0..3 {
                        let colour = f32::from(blurred[i * 4 + c]) / 255.0;
                        // Pigment thins toward the paper, pooled by the grain.
                        let thin = 0.80 * colour + 0.20 * paper[c];
                        let v = thin * (0.94 + 0.10 * wash);
                        out[i * 4 + c] = to_byte(v * (1.0 - bleed) + ink[c] * bleed);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A red wall (left, dark) and a lit pale wall (right) with a vertical
    /// boundary at `w / 2`.
    fn frame(w: usize, h: usize) -> Vec<u8> {
        let mut v = Vec::new();
        for _y in 0..h {
            for x in 0..w {
                if x < w / 2 {
                    v.extend_from_slice(&[150, 40, 30, 255]);
                } else {
                    v.extend_from_slice(&[235, 225, 210, 255]);
                }
            }
        }
        v
    }

    fn px(img: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * w + x) * 4;
        [img[i], img[i + 1], img[i + 2], img[i + 3]]
    }

    #[test]
    fn the_edge_detector_finds_the_boundary_only() {
        let (w, h) = (32, 16);
        let e = edge_strength(&frame(w, h), w, h);
        assert!(e[8 * w + w / 2] > 0.1, "{}", e[8 * w + w / 2]);
        assert!(e[8 * w + 5] < 1e-6 && e[8 * w + 26] < 1e-6);
    }

    #[test]
    fn line_drawing_is_white_with_dark_lines_on_the_boundary() {
        let (w, h) = (32, 16);
        let out = stylize(&frame(w, h), w as u32, h as u32, Style::LineDrawing);
        assert_eq!(px(&out, w, 4, 8), [255, 255, 255, 255]);
        assert_eq!(px(&out, w, 28, 8), [255, 255, 255, 255]);
        assert!(px(&out, w, w / 2, 8)[0] < 40, "{:?}", px(&out, w, w / 2, 8));
    }

    #[test]
    fn vector_view_uses_flat_grey_tiers_and_black_lines() {
        let (w, h) = (32, 16);
        let out = stylize(&frame(w, h), w as u32, h as u32, Style::VectorView);
        let dark = px(&out, w, 4, 8);
        let light = px(&out, w, 28, 8);
        assert!(dark[0] == dark[1] && dark[1] == dark[2], "grey");
        assert!(light[0] > dark[0], "the lit wall is a lighter tier");
        // Flat: every pixel away from the line has one of three values.
        let mut tiers: Vec<u8> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, _)| x < w / 2 - 3 || x > w / 2 + 3)
            .map(|(x, y)| px(&out, w, x, y)[0])
            .collect();
        tiers.sort_unstable();
        tiers.dedup();
        assert!(tiers.len() <= 3, "{tiers:?}");
        assert!(px(&out, w, w / 2, 8)[0] < 60);
    }

    #[test]
    fn technical_illustration_keeps_the_material_hue_in_flat_tiers() {
        let (w, h) = (32, 16);
        let out = stylize(
            &frame(w, h),
            w as u32,
            h as u32,
            Style::TechnicalIllustration,
        );
        let red = px(&out, w, 4, 8);
        assert!(
            red[0] > red[1] + 30 && red[0] > red[2] + 30,
            "still red: {red:?}"
        );
        // Flat: the whole red wall is one value.
        assert_eq!(px(&out, w, 2, 1), px(&out, w, 10, 14));
        assert!(px(&out, w, w / 2, 8)[0] < 70);
    }

    #[test]
    fn watercolor_softens_darkens_the_edge_and_is_deterministic() {
        let (w, h) = (48, 24);
        let src = frame(w, h);
        let a = stylize(&src, w as u32, h as u32, Style::Watercolor);
        let b = stylize(&src, w as u32, h as u32, Style::Watercolor);
        assert_eq!(a, b);
        assert_eq!(a.len(), src.len());
        assert!(a.chunks(4).all(|p| p[3] == 255));
        // The boundary bleeds darker than either side of it.
        let edge = px(&a, w, w / 2, 12)[0];
        assert!(edge < px(&a, w, w / 2 + 12, 12)[0], "{edge}");
        // The pale wall is pulled toward the paper, not left at full strength.
        assert!(px(&a, w, 40, 12)[0] < 250);
        // The paper grain varies from pixel to pixel.
        assert_ne!(px(&a, w, 38, 5), px(&a, w, 44, 9));
    }

    #[test]
    fn a_wrong_sized_buffer_comes_back_unchanged() {
        for s in Style::ALL {
            assert_eq!(stylize(&[1, 2, 3], 4, 4, s), vec![1, 2, 3]);
            assert!(stylize(&[], 0, 0, s).is_empty());
        }
    }

    #[test]
    fn technique_names_map_to_styles() {
        assert_eq!(
            Style::from_technique_name("Watercolor"),
            Some(Style::Watercolor)
        );
        assert_eq!(
            Style::from_technique_name("Line Drawing"),
            Some(Style::LineDrawing)
        );
        assert_eq!(Style::from_technique_name("Standard"), None);
    }
}
