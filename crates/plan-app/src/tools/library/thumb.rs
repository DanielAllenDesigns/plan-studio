//! 3D thumbnails of library objects for the Library Browser's preview pane.
//!
//! A thumbnail is the object's 3D shape (its symbol mesh; an item with only a
//! plan symbol is shown as that outline extruded to its height) path-traced by
//! `plan-render` from a three-quarter view with few samples, then denoised.
//! The pictures are kept in `~/.plan-studio/thumbs` ([`cache`]) so each shape is
//! traced once; [`thumbnail`] returns the cached picture or renders and stores
//! it. The render is a pure function of the model, so the same shape always
//! gives the same bytes.

use plan_library::image::Rgba8Image;
use plan_library::thumbs::ThumbCache;
use plan_library::{CatalogItem, Model3d};
use plan_render::{Camera, Environment, RenderSettings, Renderer, Sun, Technique, ToneMap};
use std::path::PathBuf;

/// Side of a thumbnail, pixels.
pub const THUMB_PX: u32 = 160;
/// Path-tracing samples per pixel (low: the denoiser does the rest).
pub const THUMB_SAMPLES: u32 = 12;
/// Renderer name in the cache key; bump with any change to the look.
pub const RENDERER: &str = "plan-render-pt-1";
/// Default view of a thumbnail, degrees.
pub const YAW: f32 = 35.0;
pub const PITCH: f32 = 25.0;

/// The cache folder (`~/.plan-studio/thumbs`), `None` without a home.
pub fn cache_dir() -> Option<PathBuf> {
    crate::paths::user_file("thumbs")
}

/// The thumbnail cache of this user.
pub fn cache() -> Option<ThumbCache> {
    cache_dir().map(ThumbCache::new)
}

/// The model of `model` as render meshes (a mesh per part, with the part's
/// own color).
fn meshes(model: &Model3d) -> Vec<plan_3d::Mesh> {
    model
        .parts
        .iter()
        .map(|p| {
            let mut m = plan_3d::import::mesh_from_triangles(
                &p.positions,
                None,
                &p.indices,
                plan_calib::mesh3d::material_for(p.color),
                None,
            );
            m.color = p.color;
            m
        })
        .filter(|m| !m.indices.is_empty())
        .collect()
}

/// The camera that frames `model` from `yaw` / `pitch` degrees.
fn camera_for(lo: [f32; 3], hi: [f32; 3], yaw: f32, pitch: f32) -> Camera {
    let c = [
        (lo[0] + hi[0]) * 0.5,
        (lo[1] + hi[1]) * 0.5,
        (lo[2] + hi[2]) * 0.5,
    ];
    let r = (0..3)
        .map(|k| (hi[k] - lo[k]).powi(2))
        .sum::<f32>()
        .sqrt()
        .max(1.0)
        * 0.5;
    let fov = 30.0_f32;
    let dist = r / (fov * 0.5).to_radians().sin() * 1.05;
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    let eye = [
        c[0] + dist * sy * cp,
        c[1] + dist * sp,
        c[2] + dist * cy * cp,
    ];
    Camera {
        eye,
        target: c,
        up: [0.0, 1.0, 0.0],
        fov_deg: fov,
        aperture: 0.0,
        focus_dist: dist,
    }
}

/// Path-traces `model` at `px` x `px` pixels with `samples` samples per
/// pixel. `None` for a model without triangles.
pub fn render_model(model: &Model3d, px: u32, samples: u32) -> Option<Rgba8Image> {
    let meshes = meshes(model);
    if meshes.is_empty() {
        return None;
    }
    let scene = plan_3d::Scene { meshes };
    let (lo, hi) = scene.bounds()?;
    let cam = camera_for(lo, hi, YAW, PITCH);
    let env = Environment {
        sky_color_zenith: [0.92, 0.93, 0.95],
        sky_color_horizon: [0.88, 0.89, 0.91],
        ground_color: [0.55, 0.55, 0.55],
        sun: Some(Sun::from_azimuth_altitude(215.0, 48.0)),
        ..Environment::default()
    };
    let settings = RenderSettings {
        width: px,
        height: px,
        samples: samples.max(1),
        max_bounces: 3,
        threads: 2,
        tone_map: ToneMap::Aces,
        denoise: true,
        technique: Technique::PhysicallyBased,
        textures: false,
        ..RenderSettings::default()
    };
    let img = Renderer::new(&scene).render(&cam, &env, &[], &settings);
    Some(Rgba8Image {
        width: img.width,
        height: img.height,
        rgba: img.rgba,
    })
}

/// The cache key of `item` shown as `model`.
pub fn key(item: &CatalogItem, model: &Model3d) -> String {
    ThumbCache::key(item, Some(model), THUMB_PX, RENDERER)
}

/// The cached thumbnail of `item` / `model` in `cache`, else a fresh render
/// that is stored there. `None` when the model has nothing to draw. A failure
/// to write the cache does not lose the picture.
pub fn thumbnail(
    cache: Option<&ThumbCache>,
    item: &CatalogItem,
    model: &Model3d,
) -> Option<Rgba8Image> {
    let k = key(item, model);
    if let Some(img) = cache.and_then(|c| c.load(&k)) {
        return Some(img);
    }
    let img = render_model(model, THUMB_PX, THUMB_SAMPLES)?;
    if let Some(c) = cache {
        let _ = c.store(&k, &img);
    }
    Some(img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_library::{Placement, Symbol2d};

    fn item() -> CatalogItem {
        CatalogItem::new(
            "user.t.1",
            "Crate",
            Placement::FreeStanding,
            Symbol2d::default(),
        )
        .with_size(20.0, 20.0, 20.0)
    }

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ps-thumb-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_box_is_path_traced_into_a_picture_that_is_not_blank() {
        let model = Model3d::box_model(20.0, 20.0, 20.0, Some([200, 40, 40]));
        let img = render_model(&model, 48, 4).expect("renders");
        assert_eq!((img.width, img.height), (48, 48));
        assert_eq!(img.rgba.len(), 48 * 48 * 4);
        // The box differs from the backdrop: some pixels are clearly red.
        let red = img
            .rgba
            .chunks(4)
            .filter(|p| p[0] > p[1].saturating_add(30) && p[0] > p[2].saturating_add(30))
            .count();
        assert!(red > 100, "{red} red pixels");
        // The same shape always renders the same bytes.
        assert_eq!(render_model(&model, 48, 4).unwrap(), img);
        assert!(render_model(&Model3d::default(), 48, 4).is_none());
    }

    #[test]
    fn thumbnails_are_traced_once_and_then_read_from_the_cache() {
        let c = ThumbCache::new(dir("cache"));
        let model = Model3d::box_model(20.0, 30.0, 10.0, None);
        let it = item();
        assert!(c.is_empty());
        let a = thumbnail(Some(&c), &it, &model).unwrap();
        assert_eq!((a.width, a.height), (THUMB_PX, THUMB_PX));
        assert_eq!(c.len(), 1, "stored in the cache folder");
        assert!(c.contains(&key(&it, &model)));
        // A damaged-free second call reads the stored PNG: identical pixels.
        let b = thumbnail(Some(&c), &it, &model).unwrap();
        assert_eq!(a, b);
        assert_eq!(c.len(), 1);
        // Another shape is another file.
        let other = Model3d::box_model(20.0, 30.0, 40.0, None);
        thumbnail(Some(&c), &it, &other).unwrap();
        assert_eq!(c.len(), 2);
        // No cache at all still renders.
        assert!(thumbnail(None, &it, &model).is_some());
        let _ = std::fs::remove_dir_all(c.dir());
    }

    #[test]
    fn the_camera_looks_at_the_middle_from_the_chosen_side() {
        let cam = camera_for([0.0, 0.0, 0.0], [10.0, 20.0, 10.0], 0.0, 0.0);
        assert_eq!(cam.target, [5.0, 10.0, 5.0]);
        assert!(
            cam.eye[2] > 5.0 && (cam.eye[0] - 5.0).abs() < 1e-3,
            "in front, level"
        );
        let up = camera_for([0.0; 3], [10.0; 3], 0.0, 45.0);
        assert!(up.eye[1] > up.target[1]);
    }
}
