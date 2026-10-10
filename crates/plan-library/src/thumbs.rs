//! The thumbnail cache of the Library Browser's preview pane.
//!
//! A thumbnail is a small rendering of an item's 3D shape. Rendering it with
//! the path tracer takes a moment, so the app renders once and keeps the PNG
//! in a folder (`~/.plan-studio/thumbs`); this module owns that folder: the
//! cache [`key`] (a hash of everything that changes the picture), [`load`],
//! [`store`] and [`prune`].
//!
//! The key covers the item's id, size, plan symbol, model rotation and the
//! model's geometry and colors, the pixel size, the renderer's name and
//! [`RENDER_VERSION`]. Editing an item, or changing the renderer, makes a new
//! key, so a stale picture is never shown; old files age out in [`prune`].
//!
//! [`key`]: ThumbCache::key
//! [`load`]: ThumbCache::load
//! [`store`]: ThumbCache::store
//! [`prune`]: ThumbCache::prune

use crate::catalog::CatalogItem;
use crate::image::{self, Rgba8Image};
use crate::model::Model3d;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Bumped when the thumbnail renderer changes its output; part of every key.
pub const RENDER_VERSION: u32 = 1;

/// Most files [`ThumbCache::prune`] keeps by default.
pub const DEFAULT_MAX_FILES: usize = 2000;

/// FNV-1a, 64 bit: a stable hash (the std hasher is not stable across runs).
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }

    fn f64(&mut self, v: f64) {
        self.bytes(&v.to_bits().to_le_bytes());
    }

    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.bytes(s.as_bytes());
    }
}

/// A folder of cached thumbnails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbCache {
    dir: PathBuf,
}

impl ThumbCache {
    /// A cache in `dir` (made on the first [`store`](Self::store)).
    pub fn new(dir: impl Into<PathBuf>) -> ThumbCache {
        ThumbCache { dir: dir.into() }
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The cache key (16 hex digits) of `item` rendered at `px` pixels by
    /// `renderer` with the optional 3D `model`. The model, when the item has
    /// one, stands for its geometry; an item without one is keyed by its plan
    /// symbol and size, from which the preview solid is built.
    pub fn key(item: &CatalogItem, model: Option<&Model3d>, px: u32, renderer: &str) -> String {
        let mut h = Fnv::new();
        h.u32(RENDER_VERSION);
        h.u32(px);
        h.str(renderer);
        h.str(&item.id);
        h.f64(item.width);
        h.f64(item.depth);
        h.f64(item.height);
        h.f64(item.model_rotation);
        for c in item.model_origin {
            h.f64(c);
        }
        match model {
            Some(m) => {
                h.u32(1);
                for p in &m.parts {
                    h.u32(p.positions.len() as u32);
                    for v in &p.positions {
                        for c in v {
                            h.u32(c.to_bits());
                        }
                    }
                    h.u32(p.indices.len() as u32);
                    for &i in &p.indices {
                        h.u32(i);
                    }
                    match p.color {
                        Some(c) => h.bytes(&c),
                        None => h.bytes(&[0, 0, 0, 255]),
                    }
                }
            }
            None => {
                h.u32(0);
                h.str(&serde_json::to_string(&item.symbol).unwrap_or_default());
            }
        }
        format!("{:016x}", h.0)
    }

    /// The file a key lives in.
    pub fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.png"))
    }

    /// The cached picture for `key`, when there is one and it decodes.
    pub fn load(&self, key: &str) -> Option<Rgba8Image> {
        let bytes = fs::read(self.path(key)).ok()?;
        image::decode(&bytes).ok()
    }

    /// Whether `key` is cached.
    pub fn contains(&self, key: &str) -> bool {
        self.path(key).is_file()
    }

    /// Saves `img` under `key` (written beside it, then renamed, so a
    /// reader never sees half a file).
    pub fn store(&self, key: &str, img: &Rgba8Image) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let png = image::png::encode_rgba(img);
        let tmp = self.dir.join(format!("{key}.tmp"));
        fs::write(&tmp, png)?;
        fs::rename(&tmp, self.path(key))
    }

    /// The cached files, as (path, modified seconds), newest first.
    fn files(&self) -> Vec<(PathBuf, u64)> {
        let Ok(rd) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut v: Vec<(PathBuf, u64)> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "png"))
            .map(|p| {
                let t = fs::metadata(&p)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                (p, t)
            })
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        v
    }

    /// Number of cached pictures.
    pub fn len(&self) -> usize {
        self.files().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Deletes the oldest pictures until at most `max_files` remain; returns
    /// how many went.
    pub fn prune(&self, max_files: usize) -> usize {
        let files = self.files();
        let mut gone = 0;
        for (p, _) in files.into_iter().skip(max_files) {
            if fs::remove_file(p).is_ok() {
                gone += 1;
            }
        }
        gone
    }

    /// Deletes every cached picture (the Library Browser's "Rebuild
    /// Thumbnails"); returns how many went.
    pub fn clear(&self) -> usize {
        self.prune(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Placement;
    use crate::symbol::Symbol2d;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "plan-library-thumbs-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn item(id: &str) -> CatalogItem {
        CatalogItem::new(id, id, Placement::FreeStanding, Symbol2d::default())
            .with_size(10.0, 20.0, 30.0)
    }

    fn picture(v: u8) -> Rgba8Image {
        Rgba8Image::filled(4, 3, [v, 10, 20, 255])
    }

    #[test]
    fn a_stored_picture_loads_back_exactly() {
        let c = ThumbCache::new(dir("rt"));
        assert!(c.is_empty());
        let key = ThumbCache::key(&item("a"), None, 128, "pt");
        assert!(c.load(&key).is_none());
        c.store(&key, &picture(7)).unwrap();
        assert!(c.contains(&key));
        assert_eq!(c.load(&key).unwrap(), picture(7));
        assert_eq!(c.len(), 1);
        let _ = fs::remove_dir_all(c.dir());
    }

    #[test]
    fn the_key_changes_with_everything_that_changes_the_picture() {
        let a = item("a");
        let base = ThumbCache::key(&a, None, 128, "pt");
        assert_eq!(base, ThumbCache::key(&a, None, 128, "pt"));
        assert_eq!(base.len(), 16);
        assert_ne!(base, ThumbCache::key(&item("b"), None, 128, "pt"));
        assert_ne!(base, ThumbCache::key(&a, None, 96, "pt"));
        assert_ne!(base, ThumbCache::key(&a, None, 128, "raster"));
        let mut wider = a.clone();
        wider.width = 11.0;
        assert_ne!(base, ThumbCache::key(&wider, None, 128, "pt"));
        let mut turned = a.clone();
        turned.model_rotation = 90.0;
        assert_ne!(base, ThumbCache::key(&turned, None, 128, "pt"));
        let m1 = Model3d::box_model(10.0, 10.0, 10.0, Some([200, 0, 0]));
        let m2 = Model3d::box_model(10.0, 10.0, 11.0, Some([200, 0, 0]));
        let m3 = Model3d::box_model(10.0, 10.0, 10.0, Some([0, 200, 0]));
        let k1 = ThumbCache::key(&a, Some(&m1), 128, "pt");
        assert_ne!(k1, base);
        assert_ne!(k1, ThumbCache::key(&a, Some(&m2), 128, "pt"));
        assert_ne!(k1, ThumbCache::key(&a, Some(&m3), 128, "pt"));
        // A different symbol changes the key of an item without a model.
        let mut drawn = a.clone();
        drawn.symbol = Symbol2d::new(vec![crate::symbol::Stroke::Circle {
            center: plan_core::geometry::Point::new(0.0, 0.0),
            radius: 3.0,
        }]);
        assert_ne!(base, ThumbCache::key(&drawn, None, 128, "pt"));
    }

    #[test]
    fn prune_keeps_the_newest_and_clear_empties() {
        let c = ThumbCache::new(dir("prune"));
        for i in 0..5u8 {
            c.store(&format!("{i:016x}"), &picture(i)).unwrap();
        }
        assert_eq!(c.len(), 5);
        assert_eq!(c.prune(10), 0);
        assert_eq!(c.prune(2), 3);
        assert_eq!(c.len(), 2);
        assert_eq!(c.clear(), 2);
        assert!(c.is_empty());
        let _ = fs::remove_dir_all(c.dir());
    }

    #[test]
    fn a_damaged_file_is_a_miss() {
        let c = ThumbCache::new(dir("bad"));
        fs::create_dir_all(c.dir()).unwrap();
        fs::write(c.path("deadbeefdeadbeef"), b"not a png").unwrap();
        assert!(c.load("deadbeefdeadbeef").is_none());
        let _ = fs::remove_dir_all(c.dir());
    }
}
