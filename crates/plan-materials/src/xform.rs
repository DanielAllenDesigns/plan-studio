//! The Texture tab's offset, angle and blend colour, applied to a material's
//! bitmap, and the browsing of texture files (Chief's folders or the user's).

use std::path::{Path, PathBuf};

use crate::material::MaterialDef;

/// Does `def` move, turn or tint its texture?
pub fn has_texture_transform(def: &MaterialDef) -> bool {
    def.texture_offset_in != (0.0, 0.0)
        || def.texture_angle_deg.rem_euclid(360.0) > 1e-9
        || (def.blend_color.is_some() && def.blend_amount > 0.0)
}

fn texel(rgba: &[u8], w: usize, h: usize, x: i64, y: i64) -> [f32; 4] {
    let (x, y) = (
        x.rem_euclid(w as i64) as usize,
        y.rem_euclid(h as i64) as usize,
    );
    let o = (y * w + x) * 4;
    [
        f32::from(rgba[o]),
        f32::from(rgba[o + 1]),
        f32::from(rgba[o + 2]),
        f32::from(rgba[o + 3]),
    ]
}

/// Bilinear sample at pixel coordinates `(fx, fy)` (pixel centres at .5),
/// wrapping in both directions.
fn sample(rgba: &[u8], w: usize, h: usize, fx: f64, fy: f64) -> [f32; 4] {
    let (gx, gy) = (fx - 0.5, fy - 0.5);
    let (x0, y0) = (gx.floor(), gy.floor());
    let (tx, ty) = ((gx - x0) as f32, (gy - y0) as f32);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (a, b) = (texel(rgba, w, h, x0, y0), texel(rgba, w, h, x0 + 1, y0));
    let (c, d) = (
        texel(rgba, w, h, x0, y0 + 1),
        texel(rgba, w, h, x0 + 1, y0 + 1),
    );
    let mut out = [0.0; 4];
    for k in 0..4 {
        let top = a[k] + (b[k] - a[k]) * tx;
        let bot = c[k] + (d[k] - c[k]) * tx;
        out[k] = top + (bot - top) * ty;
    }
    out
}

/// `rgba` (`w` x `h`, straight sRGB RGBA8, one repeat `scale_in` inches
/// across) with `def`'s texture offset, angle and blend colour applied.
///
/// The offset slides the texture across the surface (the bitmap wraps, so it
/// stays tileable), the angle turns it about the centre of a repeat
/// (multiples of 90 degrees stay seamless; other angles resample with
/// wrap-around and may show a faint seam), and the blend colour is mixed in by
/// `blend_amount`. Alpha is kept. With nothing to apply the pixels come back
/// unchanged.
pub fn transform_rgba(
    rgba: &[u8],
    w: u32,
    h: u32,
    scale_in: [f32; 2],
    def: &MaterialDef,
) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 || rgba.len() != w * h * 4 || !has_texture_transform(def) {
        return rgba.to_vec();
    }
    let (sx, sy) = (
        f64::from(scale_in[0].max(1e-3)),
        f64::from(scale_in[1].max(1e-3)),
    );
    let (off_x, off_y) = def.texture_offset_in;
    let angle = def.texture_angle_deg.rem_euclid(360.0);
    let (sin, cos) = (-angle).to_radians().sin_cos();
    let tint = def
        .blend_color
        .filter(|_| def.blend_amount > 0.0)
        .map(|c| (c.map(f32::from), def.blend_amount.clamp(0.0, 1.0)));
    let mut out = vec![0u8; rgba.len()];
    for y in 0..h {
        for x in 0..w {
            // Destination pixel centre in inches from the repeat's centre.
            let (px, py) = (
                ((x as f64 + 0.5) / w as f64 - 0.5) * sx,
                ((y as f64 + 0.5) / h as f64 - 0.5) * sy,
            );
            // Undo the offset, then the turn.
            let (qx, qy) = (px - off_x, py - off_y);
            let (rx, ry) = (qx * cos - qy * sin, qx * sin + qy * cos);
            let (fx, fy) = ((rx / sx + 0.5) * w as f64, (ry / sy + 0.5) * h as f64);
            let mut p = sample(rgba, w, h, fx, fy);
            if let Some((c, amount)) = tint {
                for k in 0..3 {
                    p[k] += (c[k] - p[k]) * amount;
                }
            }
            let o = (y * w + x) * 4;
            for k in 0..4 {
                out[o + k] = p[k].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// One picture the Texture tab can pick.
#[derive(Debug, Clone, PartialEq)]
pub struct TextureFile {
    /// The file name without its extension, e.g. `Brick Red(36)`.
    pub name: String,
    pub path: PathBuf,
    /// The folder below the searched root, `/`-separated ("" at the root).
    pub folder: String,
    /// The tile size the name announces (`Brick(36).jpg` is 36), inches.
    pub tile_in: Option<f32>,
}

const PICTURE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];
const MAX_DEPTH: usize = 5;

/// The picture files in `dirs` (and their folders, five deep), sorted by
/// folder then name, at most `limit`. Folders that do not exist are skipped.
pub fn list_texture_files(dirs: &[PathBuf], limit: usize) -> Vec<TextureFile> {
    fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<TextureFile>, limit: usize) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            if out.len() >= limit {
                return;
            }
            let path = e.path();
            let file_name = e.file_name().to_string_lossy().into_owned();
            if file_name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                if depth < MAX_DEPTH {
                    walk(root, &path, depth + 1, out, limit);
                }
                continue;
            }
            let is_picture = path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| PICTURE_EXTENSIONS.contains(&x.to_ascii_lowercase().as_str()));
            if !is_picture {
                continue;
            }
            let folder = path
                .parent()
                .and_then(|p| p.strip_prefix(root).ok())
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            out.push(TextureFile {
                name: path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                tile_in: crate::textures::tile_inches_from_name(&file_name),
                path,
                folder,
            });
        }
    }
    let mut out = Vec::new();
    for d in dirs {
        walk(d, d, 0, &mut out, limit);
    }
    out.sort_by(|a, b| (&a.folder, &a.name).cmp(&(&b.folder, &b.name)));
    out.truncate(limit);
    out
}

/// Files of `files` whose name or folder contains `query` (case-insensitive).
pub fn filter_texture_files<'a>(files: &'a [TextureFile], query: &str) -> Vec<&'a TextureFile> {
    let q = query.trim().to_lowercase();
    files
        .iter()
        .filter(|f| {
            q.is_empty()
                || f.name.to_lowercase().contains(&q)
                || f.folder.to_lowercase().contains(&q)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad() -> Vec<u8> {
        // 2x2: red, green / blue, white.
        [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 255, 255],
        ]
        .concat()
    }

    fn def() -> MaterialDef {
        MaterialDef::new("T", &["Custom"], [10, 10, 10])
    }

    #[test]
    fn nothing_to_apply_returns_the_pixels() {
        let d = def();
        assert!(!has_texture_transform(&d));
        assert_eq!(transform_rgba(&quad(), 2, 2, [12.0, 12.0], &d), quad());
    }

    #[test]
    fn a_half_repeat_offset_rolls_the_bitmap() {
        let mut d = def();
        d.texture_offset_in = (6.0, 0.0);
        let out = transform_rgba(&quad(), 2, 2, [12.0, 12.0], &d);
        // One pixel to the right: the left column now holds the old right one.
        assert_eq!(out[..4], [0, 255, 0, 255]);
        assert_eq!(out[4..8], [255, 0, 0, 255]);
        assert_eq!(out[8..12], [255, 255, 255, 255]);
    }

    #[test]
    fn a_quarter_turn_is_exact_and_four_of_them_come_home() {
        let mut d = def();
        d.texture_angle_deg = 90.0;
        let turned = transform_rgba(&quad(), 2, 2, [12.0, 12.0], &d);
        assert_ne!(turned, quad());
        assert_eq!(turned.len(), quad().len());
        // Every pixel is one of the originals (no resampling blur).
        for p in turned.chunks(4) {
            assert!(quad().chunks(4).any(|q| q == p), "{p:?}");
        }
        d.texture_angle_deg = 360.0;
        assert_eq!(transform_rgba(&quad(), 2, 2, [12.0, 12.0], &d), quad());
    }

    #[test]
    fn the_blend_colour_tints_by_its_amount_and_keeps_alpha() {
        let mut d = def();
        d.blend_color = Some([0, 0, 0]);
        d.blend_amount = 0.5;
        let out = transform_rgba(&quad(), 2, 2, [12.0, 12.0], &d);
        assert_eq!(out[..4], [128, 0, 0, 255]);
        assert_eq!(out[12..16], [128, 128, 128, 255]);
        d.blend_amount = 1.0;
        let black = transform_rgba(&quad(), 2, 2, [12.0, 12.0], &d);
        assert!(black.chunks(4).all(|p| p[..3] == [0, 0, 0] && p[3] == 255));
    }

    #[test]
    fn texture_files_are_listed_sorted_filtered_and_capped() {
        let dir = std::env::temp_dir().join(format!("plan-xform-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Brick")).unwrap();
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        for f in [
            "Brick/Red(36).jpg",
            "Brick/Old.PNG",
            "Slate.png",
            "notes.txt",
            ".hidden/x.png",
        ] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let all = list_texture_files(std::slice::from_ref(&dir), 100);
        let names: Vec<_> = all.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            ["Slate", "Old", "Red(36)"],
            "root files first, no text, no dot folders"
        );
        assert_eq!(all[2].tile_in, Some(36.0));
        assert_eq!(all[2].folder, "Brick");
        assert_eq!(filter_texture_files(&all, "brick").len(), 2);
        assert_eq!(filter_texture_files(&all, "SLATE").len(), 1);
        assert_eq!(list_texture_files(std::slice::from_ref(&dir), 1).len(), 1);
        assert!(list_texture_files(&[dir.join("missing")], 10).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
