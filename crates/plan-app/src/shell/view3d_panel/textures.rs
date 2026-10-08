//! Bitmaps for the 3D view: the decoded pictures (PNG or JPEG) of placed
//! images and billboards, handed to the viewport as [`ImageTexture`]s.
//!
//! Decoding goes through `plan_library::image` (the same decoder as the
//! material textures), results are cached by file, modification time and
//! transparency key, and a file that cannot be read or decoded is remembered
//! as failed (its picture stays the flat-coloured quad).

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use plan_core::images::ImageSpec;
use plan_core::{Id, Project};
use plan_library::image;
use plan_view3d::{ImageTexture, MAX_PICTURE_SIDE};

/// Pictures kept decoded at once (each is at most 2048 x 2048 x 4 bytes).
const CACHE_PICTURES: usize = 24;

struct Decoded {
    width: u32,
    height: u32,
    rgba: Arc<Vec<u8>>,
}

type Cache = Mutex<(HashMap<u64, Option<Arc<Decoded>>>, Vec<u64>)>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new((HashMap::new(), Vec::new())))
}

/// What identifies one decoded picture: file, modified time, transparency key.
fn picture_key(spec: &ImageSpec) -> u64 {
    let mut h = DefaultHasher::new();
    spec.path.hash(&mut h);
    spec.transparency.hash(&mut h);
    spec.tolerance.hash(&mut h);
    if let Ok(meta) = std::fs::metadata(&spec.path) {
        meta.len().hash(&mut h);
        if let Ok(t) = meta.modified() {
            t.hash(&mut h);
        }
    }
    h.finish()
}

/// Reads `spec`'s file, keys out its transparency colour and shrinks it to
/// the GPU limit. `None` when the file is missing or not a PNG/JPEG we decode.
fn decode(spec: &ImageSpec) -> Option<Decoded> {
    let mut img = image::decode_file(std::path::Path::new(&spec.path)).ok()?;
    if img.width.max(img.height) > MAX_PICTURE_SIDE {
        img = img.downscaled(MAX_PICTURE_SIDE);
    }
    if let Some(key) = spec.transparency {
        let tol = i32::from(spec.tolerance);
        for px in img.rgba.as_chunks_mut::<4>().0 {
            if (0..3).all(|i| (i32::from(px[i]) - i32::from(key[i])).abs() <= tol) {
                px[3] = 0;
            }
        }
    }
    Some(Decoded {
        width: img.width,
        height: img.height,
        rgba: Arc::new(img.rgba),
    })
}

fn decoded(spec: &ImageSpec) -> Option<(u64, Arc<Decoded>)> {
    let key = picture_key(spec);
    {
        let g = cache().lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(hit) = g.0.get(&key) {
            return hit.clone().map(|d| (key, d));
        }
    }
    let d = decode(spec).map(Arc::new);
    let mut g = cache().lock().unwrap_or_else(PoisonError::into_inner);
    g.0.insert(key, d.clone());
    g.1.push(key);
    while g.1.len() > CACHE_PICTURES {
        let old = g.1.remove(0);
        g.0.remove(&old);
    }
    d.map(|d| (key, d))
}

/// The bitmaps of the pictures among `ids` (mesh object ids), over the first
/// `floors` floors. Pictures whose file cannot be decoded are left out.
pub fn picture_textures(project: &Project, floors: usize, ids: &HashSet<Id>) -> Vec<ImageTexture> {
    let mut out = Vec::new();
    for f in project.floors.iter().take(floors) {
        for s in &f.symbols {
            let Some(spec) = &s.image else { continue };
            if !ids.contains(&s.id) {
                continue;
            }
            let Some((key, d)) = decoded(spec) else {
                continue;
            };
            out.push(ImageTexture {
                object_id: s.id,
                key,
                width: d.width,
                height: d.height,
                rgba: Arc::clone(&d.rgba),
                flip: s.flip,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;
    use plan_core::PlacedSymbol;

    fn temp_png(name: &str, img: &image::Rgba8Image) -> String {
        let dir = std::env::temp_dir().join(format!("plan_v3d_pics_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, image::png::encode_rgba(img)).unwrap();
        p.to_string_lossy().into_owned()
    }

    fn project_with(spec: ImageSpec) -> (Project, Id) {
        let mut p = Project::new("pictures");
        let mut s = PlacedSymbol::picture(spec, Point::new(100.0, 50.0), 40.0, 20.0);
        s.id = 77;
        p.floors[0].symbols.push(s);
        (p, 77)
    }

    #[test]
    fn a_picture_gets_its_decoded_bitmap_with_the_key_colour_cut_out() {
        let mut img = image::Rgba8Image::filled(4, 2, [10, 200, 30, 255]);
        img.rgba[0..4].copy_from_slice(&[255, 0, 255, 255]); // magenta key pixel
        let path = temp_png("keyed.png", &img);
        let mut spec = ImageSpec::new(path, 4, 2);
        spec.transparency = Some([255, 0, 255]);
        let (p, id) = project_with(spec);
        let ids: HashSet<Id> = [id].into();
        let t = picture_textures(&p, 1, &ids);
        assert_eq!(t.len(), 1);
        assert_eq!((t[0].width, t[0].height, t[0].object_id), (4, 2, 77));
        assert_eq!(t[0].rgba[3], 0, "the key colour is transparent");
        assert_eq!(t[0].rgba[7], 255);
        assert!(t[0].has_alpha() && t[0].is_valid());
        // The second call hits the cache and gives the same bitmap and key.
        let again = picture_textures(&p, 1, &ids);
        assert_eq!(again[0].key, t[0].key);
        assert!(Arc::ptr_eq(&again[0].rgba, &t[0].rgba));
        // Pictures not asked for, and missing files, are left out.
        assert!(picture_textures(&p, 1, &HashSet::new()).is_empty());
        let (p2, id2) = project_with(ImageSpec::new("/nonexistent/none.png", 1, 1));
        assert!(picture_textures(&p2, 1, &[id2].into()).is_empty());
    }
}
