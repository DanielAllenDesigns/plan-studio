//! Loads the path-traced 3D thumbnails of the Library Browser's preview pane
//! without freezing the window: a picture found in the thumbnail cache
//! (`~/.plan-studio/thumbs`) is ready at once; a missing one is rendered on a
//! background thread (`tools::library::thumb`), stored in the cache and
//! picked up on a later frame.

use crate::tools::library::thumb;
use eframe::egui::{self, TextureHandle};
use plan_library::image::Rgba8Image;
use plan_library::thumbs::ThumbCache;
use plan_library::{CatalogItem, Model3d};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// Renders running at once (each uses two threads of the path tracer).
const MAX_JOBS: usize = 2;
/// Textures kept; the oldest are dropped past this.
const MAX_TEXTURES: usize = 48;

/// What a thumbnail request found.
#[derive(Clone)]
pub enum Thumb {
    /// The picture, ready to paint.
    Ready(TextureHandle),
    /// Being rendered; ask again next frame.
    Rendering,
    /// Nothing to draw (a model without triangles, or the render failed).
    Unavailable,
}

/// The thumbnails of one browser.
pub struct ThumbService {
    cache: Option<ThumbCache>,
    jobs: HashMap<String, Receiver<Option<Rgba8Image>>>,
    textures: HashMap<String, Option<TextureHandle>>,
    /// Keys in the order their textures were made (oldest first).
    order: Vec<String>,
}

impl Default for ThumbService {
    /// The user's cache folder, trimmed to its newest
    /// [`plan_library::thumbs::DEFAULT_MAX_FILES`] pictures.
    fn default() -> Self {
        let cache = thumb::cache();
        if let Some(c) = &cache {
            c.prune(plan_library::thumbs::DEFAULT_MAX_FILES);
        }
        ThumbService::with_cache(cache)
    }
}

impl ThumbService {
    /// A service over `cache` (`None` renders without keeping pictures).
    pub fn with_cache(cache: Option<ThumbCache>) -> ThumbService {
        ThumbService {
            cache,
            jobs: HashMap::new(),
            textures: HashMap::new(),
            order: Vec::new(),
        }
    }

    #[cfg(test)]
    /// The cache folder, if any.
    pub fn cache(&self) -> Option<&ThumbCache> {
        self.cache.as_ref()
    }

    #[cfg(test)]
    /// Renders in progress.
    pub fn pending(&self) -> usize {
        self.jobs.len()
    }

    fn remember(&mut self, key: String, tex: Option<TextureHandle>) {
        self.textures.insert(key.clone(), tex);
        self.order.push(key);
        while self.order.len() > MAX_TEXTURES {
            let old = self.order.remove(0);
            self.textures.remove(&old);
        }
    }

    fn texture(ctx: &egui::Context, key: &str, img: &Rgba8Image) -> TextureHandle {
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [img.width as usize, img.height as usize],
            &img.rgba,
        );
        ctx.load_texture(
            format!("library_thumb_{key}"),
            color,
            egui::TextureOptions::LINEAR,
        )
    }

    /// Collects finished renders.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let keys: Vec<String> = self.jobs.keys().cloned().collect();
        for key in keys {
            let Some(rx) = self.jobs.get(&key) else {
                continue;
            };
            match rx.try_recv() {
                Ok(img) => {
                    self.jobs.remove(&key);
                    let tex = img.map(|i| Self::texture(ctx, &key, &i));
                    self.remember(key, tex);
                }
                Err(TryRecvError::Disconnected) => {
                    self.jobs.remove(&key);
                    self.remember(key, None);
                }
                Err(TryRecvError::Empty) => {}
            }
        }
    }

    /// The thumbnail of `item` shown as `model`: from memory, else the cache,
    /// else a background render.
    pub fn get(&mut self, ctx: &egui::Context, item: &CatalogItem, model: &Model3d) -> Thumb {
        self.poll(ctx);
        let key = thumb::key(item, model);
        if let Some(t) = self.textures.get(&key) {
            return match t {
                Some(t) => Thumb::Ready(t.clone()),
                None => Thumb::Unavailable,
            };
        }
        if self.jobs.contains_key(&key) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return Thumb::Rendering;
        }
        if let Some(img) = self.cache.as_ref().and_then(|c| c.load(&key)) {
            let tex = Self::texture(ctx, &key, &img);
            self.remember(key, Some(tex.clone()));
            return Thumb::Ready(tex);
        }
        if self.jobs.len() >= MAX_JOBS {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return Thumb::Rendering;
        }
        let (tx, rx) = channel();
        let (cache, item, model, repaint) =
            (self.cache.clone(), item.clone(), model.clone(), ctx.clone());
        std::thread::spawn(move || {
            let img = thumb::thumbnail(cache.as_ref(), &item, &model);
            let _ = tx.send(img);
            repaint.request_repaint();
        });
        self.jobs.insert(key, rx);
        Thumb::Rendering
    }

    /// Forgets the textures and deletes the cached files (Rebuild
    /// Thumbnails); returns the number of files deleted.
    pub fn rebuild(&mut self) -> usize {
        self.textures.clear();
        self.order.clear();
        self.cache.as_ref().map_or(0, ThumbCache::clear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_library::{Placement, Symbol2d};
    use std::time::{Duration, Instant};

    fn item() -> CatalogItem {
        CatalogItem::new(
            "user.th.1",
            "Block",
            Placement::FreeStanding,
            Symbol2d::default(),
        )
        .with_size(12.0, 12.0, 12.0)
    }

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ps-thumbsvc-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn wait_ready(
        svc: &mut ThumbService,
        ctx: &egui::Context,
        it: &CatalogItem,
        m: &Model3d,
    ) -> Thumb {
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            match svc.get(ctx, it, m) {
                Thumb::Rendering if Instant::now() < end => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                t => return t,
            }
        }
    }

    #[test]
    fn a_thumbnail_renders_in_the_background_then_comes_from_the_cache() {
        let ctx = egui::Context::default();
        let d = dir("bg");
        let mut svc = ThumbService::with_cache(Some(ThumbCache::new(&d)));
        let model = Model3d::box_model(12.0, 12.0, 12.0, Some([60, 120, 200]));
        let it = item();
        // The first request starts a render.
        assert!(matches!(svc.get(&ctx, &it, &model), Thumb::Rendering));
        assert_eq!(svc.pending(), 1);
        assert!(matches!(
            wait_ready(&mut svc, &ctx, &it, &model),
            Thumb::Ready(_)
        ));
        assert_eq!(svc.pending(), 0);
        assert_eq!(
            svc.cache().unwrap().len(),
            1,
            "stored under the thumbs folder"
        );

        // A new service (a new session) finds the picture without rendering.
        let mut again = ThumbService::with_cache(Some(ThumbCache::new(&d)));
        assert!(matches!(again.get(&ctx, &it, &model), Thumb::Ready(_)));
        assert_eq!(again.pending(), 0);

        // Rebuild deletes the files; the next request renders again.
        assert_eq!(again.rebuild(), 1);
        assert!(matches!(again.get(&ctx, &it, &model), Thumb::Rendering));
        assert!(matches!(
            wait_ready(&mut again, &ctx, &it, &model),
            Thumb::Ready(_)
        ));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_empty_model_has_no_thumbnail() {
        let ctx = egui::Context::default();
        let mut svc = ThumbService::with_cache(None);
        let it = item();
        let empty = Model3d::default();
        assert!(matches!(
            wait_ready(&mut svc, &ctx, &it, &empty),
            Thumb::Unavailable
        ));
    }
}
