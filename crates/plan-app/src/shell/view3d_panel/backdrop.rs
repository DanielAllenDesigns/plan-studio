//! 3D backdrop pictures (`docs/parity/3d-views-cameras.md`, C-70): the
//! Backdrop tab of a camera names a picture in Chief's Backdrops folder, which
//! is read from the install when a view needs it. Nothing is bundled or
//! copied into the plan: the camera keeps only the file name.

use plan_view3d::BackdropImage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

/// Picture formats the decoder reads.
const EXTENSIONS: [&str; 3] = ["jpg", "jpeg", "png"];

/// The folders searched for backdrop pictures, most specific first:
/// `PLAN_STUDIO_BACKDROPS`, then Chief's Backdrops folder in the user's data.
pub fn backdrop_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(extra) = std::env::var_os("PLAN_STUDIO_BACKDROPS") {
        dirs.push(PathBuf::from(extra));
    }
    if let Some(home) = crate::paths::home_dir() {
        dirs.push(home.join("Documents/Chief Architect Premier X18 Data/Backdrops"));
    }
    dirs.retain(|d| d.is_dir());
    dirs
}

fn is_picture(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// The pictures in `dir` and one folder level below it, as names relative to
/// `dir` (`Rolling Hills.jpg`, `-My Backdrops/Lake.png`), sorted.
pub fn list_in(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if let Ok(inner) = std::fs::read_dir(&path) {
                for f in inner.flatten() {
                    if is_picture(&f.path()) {
                        out.push(format!("{name}/{}", f.file_name().to_string_lossy()));
                    }
                }
            }
        } else if is_picture(&path) {
            out.push(name);
        }
    }
    out.sort_by_key(|n| n.to_lowercase());
    out
}

/// Every backdrop picture of the search folders.
pub fn list_all() -> Vec<String> {
    let mut all: Vec<String> = backdrop_dirs().iter().flat_map(|d| list_in(d)).collect();
    all.dedup();
    all
}

/// The file a camera's backdrop name stands for: a full path as it is, else
/// the first search folder that has it.
pub fn resolve(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let direct = Path::new(name);
    if direct.is_absolute() {
        return direct.is_file().then(|| direct.to_path_buf());
    }
    dirs.iter().map(|d| d.join(name)).find(|p| p.is_file())
}

type Cache = HashMap<PathBuf, (Option<SystemTime>, Option<Arc<BackdropImage>>)>;

fn cache() -> &'static Mutex<Cache> {
    static CACHE: std::sync::OnceLock<Mutex<Cache>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Decodes the picture at `path` (cached until the file changes).
pub fn load_file(path: &Path) -> Option<Arc<BackdropImage>> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let mut cache = cache().lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((when, image)) = cache.get(path) {
        if *when == modified {
            return image.clone();
        }
    }
    let image = plan_library::image::decode_file(path)
        .ok()
        .and_then(|img| BackdropImage::new(img.width, img.height, img.rgba))
        .map(Arc::new);
    cache.insert(path.to_path_buf(), (modified, image.clone()));
    image
}

/// The backdrop picture named `name`, read from the search folders.
pub fn load(name: &str) -> Option<Arc<BackdropImage>> {
    load_file(&resolve(name, &backdrop_dirs())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("plan-backdrops-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_folder_listing_finds_pictures_one_level_down() {
        let d = temp_dir("list");
        std::fs::create_dir_all(d.join("-My Backdrops")).unwrap();
        for f in ["b.JPG", "a.png", "notes.txt", "-My Backdrops/Lake.jpeg"] {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        assert_eq!(list_in(&d), ["-My Backdrops/Lake.jpeg", "a.png", "b.JPG"]);
        assert!(list_in(&d.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_name_resolves_through_the_folders_or_as_a_full_path() {
        let d = temp_dir("resolve");
        std::fs::write(d.join("sky.png"), b"x").unwrap();
        let dirs = [d.clone()];
        assert_eq!(resolve("sky.png", &dirs), Some(d.join("sky.png")));
        assert_eq!(resolve("  ", &dirs), None);
        assert_eq!(resolve("nope.png", &dirs), None);
        let abs = d.join("sky.png");
        assert_eq!(resolve(abs.to_str().unwrap(), &[]), Some(abs));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_picture_is_decoded_once_and_again_when_it_changes() {
        let d = temp_dir("load");
        let img = plan_library::image::Rgba8Image::filled(4, 2, [10, 200, 30, 255]);
        let png = plan_render::encode_png(&plan_render::Image {
            width: 4,
            height: 2,
            rgba: img.rgba.clone(),
            hdr: Vec::new(),
        });
        let path = d.join("g.png");
        std::fs::write(&path, png).unwrap();
        let a = load_file(&path).expect("decodes");
        assert_eq!((a.width, a.height), (4, 2));
        let b = load_file(&path).expect("cached");
        assert!(Arc::ptr_eq(&a, &b));
        assert!(load_file(&d.join("missing.png")).is_none());
        std::fs::write(d.join("bad.png"), b"not a picture").unwrap();
        assert!(load_file(&d.join("bad.png")).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
