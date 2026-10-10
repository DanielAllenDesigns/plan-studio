//! Material packages in the app: File > Import > Material Package..., the
//! drop of a `.zip` on the window, the Lightbeans folder of the Library
//! Browser's Materials view and the Downloads watcher.
//!
//! Lightbeans (lightbeans.com/en/textures) offers seamless PBR texture sets as
//! one zip per material. Plan Studio never downloads them and never keeps them
//! in a plan or the repository: the user downloads, [`import_zip`] copies the
//! maps into `~/.plan-studio/textures/<package>/` (see
//! `plan_materials::package`) and saves a material for them in My Materials.
//! With *Watch Downloads folder* on (Preferences > Render), a zip with a
//! `productmetadata.txt` that appears in `~/Downloads` is offered in the
//! "New downloads" group with an Import button. The folder is polled every
//! three seconds on the UI thread; there are no threads.

use super::{save_to_user_library, set_active, swatch};
use crate::dialogs::preferences::pages;
use crate::editor::EditorContext;
use eframe::egui;
use plan_materials::package::{self, MaterialPackage, ReadOptions};
use plan_materials::MaterialDef;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Command id: File > Import > Material Package...
pub const IMPORT_PACKAGE: &str = "materials.import_package";
/// Command id: the "Browse Lightbeans textures..." row.
pub const BROWSE_LIGHTBEANS: &str = "materials.browse_lightbeans";
/// The Lightbeans texture catalogue.
pub const LIGHTBEANS_URL: &str = "https://lightbeans.com/en/textures";
/// How often the Downloads folder is looked at.
pub const POLL_EVERY: Duration = Duration::from_secs(3);
/// A zip older than this when watching starts is not "new".
const NEW_WINDOW: Duration = Duration::from_secs(3600);
/// Largest side of the thumbnails in the Lightbeans folder, pixels.
const THUMB_SIDE: u32 = 48;

#[derive(Default)]
struct Watch {
    /// Watching started (zips modified after `since` are new).
    since: Option<SystemTime>,
    last_poll: Option<Instant>,
    /// `(len, modified, is_package)` per zip already looked into.
    seen: HashMap<PathBuf, (u64, SystemTime, bool)>,
    /// Zips offered for import.
    new: Vec<PathBuf>,
    /// Zips imported or dismissed, not offered again.
    done: Vec<PathBuf>,
}

#[derive(Default)]
struct PkgState {
    watch: Watch,
    thumbs: HashMap<String, Option<egui::TextureHandle>>,
    #[cfg(test)]
    root: Option<PathBuf>,
    #[cfg(test)]
    downloads: Option<PathBuf>,
    #[cfg(test)]
    next_pick: Option<Vec<PathBuf>>,
    #[cfg(test)]
    opened: Vec<String>,
}

thread_local! {
    static PKG: RefCell<PkgState> = RefCell::new(PkgState::default());
}

fn pkg<R>(f: impl FnOnce(&mut PkgState) -> R) -> R {
    PKG.with(|s| f(&mut s.borrow_mut()))
}

// ----- places -----

/// `~/.plan-studio/textures`, where imported maps live.
pub fn textures_root() -> Option<PathBuf> {
    #[cfg(test)]
    {
        pkg(|s| s.root.clone())
    }
    #[cfg(not(test))]
    {
        crate::paths::user_file(package::TEXTURES_DIR)
    }
}

/// `~/Downloads`.
pub fn downloads_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        pkg(|s| s.downloads.clone())
    }
    #[cfg(not(test))]
    {
        crate::paths::home_dir().map(|h| h.join("Downloads"))
    }
}

/// Points the textures folder at `dir` for this thread (tests never touch the
/// real one).
#[cfg(test)]
pub fn set_textures_root_for_test(dir: Option<PathBuf>) {
    pkg(|s| s.root = dir);
}

/// Points the Downloads folder at `dir` for this thread (tests).
#[cfg(test)]
pub fn set_downloads_for_test(dir: Option<PathBuf>) {
    pkg(|s| s.downloads = dir);
}

/// The zips the next file box returns (tests).
#[cfg(test)]
pub fn set_next_pick_for_test(files: Option<Vec<PathBuf>>) {
    pkg(|s| s.next_pick = files);
}

/// URLs [`open_url`] was asked to open (tests).
#[cfg(test)]
pub fn opened_urls_for_test() -> Vec<String> {
    pkg(|s| s.opened.clone())
}

/// Opens `url` in the system browser (tests record it instead).
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(test)]
    {
        pkg(|s| s.opened.push(url.to_string()));
        Ok(())
    }
    #[cfg(not(test))]
    {
        crate::dialogs::app_info::open_external(url)
    }
}

// ----- import -----

/// The package zips the user picks (the native file box; tests preset them).
pub fn pick_zips() -> Vec<PathBuf> {
    #[cfg(test)]
    {
        pkg(|s| s.next_pick.take()).unwrap_or_default()
    }
    #[cfg(not(test))]
    {
        rfd::FileDialog::new()
            .set_title("Material Package")
            .add_filter("Material packages (zip)", &["zip"])
            .pick_files()
            .unwrap_or_default()
    }
}

/// Largest side kept decoded, from Preferences > Render.
fn max_side() -> u32 {
    pages::current().render.max_texture_side.clamp(256, 8192)
}

/// Imports the package zip at `path`: reads it, copies its maps into the
/// user's textures folder, and saves a material for it in My Materials.
/// Returns the material and any warnings.
pub fn import_zip(path: &Path) -> Result<(MaterialDef, Vec<String>), String> {
    let pkg = MaterialPackage::read_path_with(
        path,
        &ReadOptions {
            max_side: max_side(),
        },
    )?;
    let root = textures_root().ok_or_else(|| crate::paths::NO_HOME.to_string())?;
    let today = package::today();
    let imported = pkg.import_to(&root, &today)?;
    let def = MaterialDef::from_package(&pkg, &imported, &today);
    save_to_user_library(&def)?;
    // Not offered again.
    pkg_done(path);
    Ok((def, pkg.warnings))
}

fn pkg_done(path: &Path) {
    pkg(|s| {
        s.watch.new.retain(|p| p != path);
        if !s.watch.done.iter().any(|p| p == path) {
            s.watch.done.push(path.to_path_buf());
        }
    });
}

/// Imports every zip of `paths`; the last good material becomes the active
/// one. Sets the status line and returns the names imported.
pub fn import_zips(cx: &mut EditorContext, paths: &[PathBuf]) -> Vec<String> {
    let mut names = Vec::new();
    let mut problems = Vec::new();
    let mut notes = Vec::new();
    for p in paths {
        match import_zip(p) {
            Ok((def, warnings)) => {
                notes.extend(warnings);
                names.push(def.name.clone());
                set_active(Some(def.name));
            }
            Err(e) => problems.push(format!(
                "{}: {e}",
                p.file_name().map_or_else(
                    || p.display().to_string(),
                    |n| n.to_string_lossy().into_owned()
                )
            )),
        }
    }
    cx.status = match (names.len(), problems.first()) {
        (0, Some(e)) => format!("Could not import: {e}"),
        (0, None) => "No material package chosen".into(),
        (1, None) => format!("Imported {} to My Materials (Lightbeans)", names[0]),
        (n, None) => format!("Imported {n} material packages to My Materials"),
        (n, Some(e)) => format!("Imported {n} packages; could not import {e}"),
    };
    if let Some(w) = notes.first() {
        cx.status.push_str(&format!(" ({w})"));
    }
    names
}

/// File > Import > Material Package...: choose one or many zips.
pub fn import_command(cx: &mut EditorContext) -> Vec<String> {
    let files = pick_zips();
    if files.is_empty() {
        cx.status = "No material package chosen".into();
        return Vec::new();
    }
    import_zips(cx, &files)
}

/// `.zip` files dropped on the window are material packages: imports them
/// (a zip that is not one says so in the status line). Returns how many zips
/// were handled, 0 when the drop held none.
pub fn handle_dropped(cx: &mut EditorContext, dropped: &[PathBuf]) -> usize {
    let zips: Vec<PathBuf> = dropped
        .iter()
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
        })
        .cloned()
        .collect();
    if zips.is_empty() {
        return 0;
    }
    import_zips(cx, &zips);
    zips.len()
}

/// Runs a package command by id; false when the id is not one of ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        IMPORT_PACKAGE => {
            import_command(cx);
            true
        }
        BROWSE_LIGHTBEANS => {
            browse_lightbeans(cx);
            true
        }
        _ => false,
    }
}

/// Opens the Lightbeans texture catalogue in the system browser.
pub fn browse_lightbeans(cx: &mut EditorContext) {
    cx.status = match open_url(LIGHTBEANS_URL) {
        Ok(()) => format!("Opened {LIGHTBEANS_URL}: download a material, then drop the zip here"),
        Err(e) => e,
    };
}

/// The packages in My Materials, newest import first.
pub fn imported_packages() -> Vec<MaterialDef> {
    let mut v: Vec<MaterialDef> = super::user_library()
        .materials
        .into_iter()
        .filter(|m| m.package_imported.is_some())
        .collect();
    v.sort_by(|a, b| {
        b.package_imported
            .cmp(&a.package_imported)
            .then_with(|| a.name.cmp(&b.name))
    });
    v
}

// ----- Downloads watcher -----

/// The zips in `dir` (not looked into before, modified at or after `since`)
/// that hold a `productmetadata.txt`. `seen` remembers what was already
/// looked into by length and modification time, so each file is opened once
/// (a download still being written is looked at again as it grows).
pub fn scan_downloads(
    dir: &Path,
    since: SystemTime,
    seen: &mut HashMap<PathBuf, (u64, SystemTime, bool)>,
) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for e in rd.filter_map(Result::ok) {
        let path = e.path();
        let is_zip = path
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("zip"));
        if !is_zip {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        if modified < since {
            continue;
        }
        let stamp = (meta.len(), modified);
        let is_pkg = match seen.get(&path) {
            Some((l, m, ok)) if (*l, *m) == stamp => *ok,
            _ => {
                let ok = package::is_lightbeans_zip(&path);
                seen.insert(path.clone(), (stamp.0, stamp.1, ok));
                ok
            }
        };
        if is_pkg {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// Looks at the Downloads folder if it is time (every three seconds, only
/// with the preference on). `force` skips the timer. Returns true when the
/// list of new downloads changed.
pub fn poll_downloads(now: Instant, force: bool) -> bool {
    let on = pages::current().render.watch_downloads;
    let dir = downloads_dir();
    pkg(|s| {
        if !on {
            let had = !s.watch.new.is_empty();
            s.watch.new.clear();
            s.watch.since = None;
            s.watch.last_poll = None;
            return had;
        }
        if !force
            && s.watch
                .last_poll
                .is_some_and(|t| now.duration_since(t) < POLL_EVERY)
        {
            return false;
        }
        s.watch.last_poll = Some(now);
        let since = *s
            .watch
            .since
            .get_or_insert_with(|| SystemTime::now() - NEW_WINDOW);
        let Some(dir) = dir.clone() else {
            return false;
        };
        let mut found = scan_downloads(&dir, since, &mut s.watch.seen);
        found.retain(|p| !s.watch.done.contains(p));
        let changed = found != s.watch.new;
        s.watch.new = found;
        changed
    })
}

/// The package zips waiting in the "New downloads" group.
pub fn new_downloads() -> Vec<PathBuf> {
    pkg(|s| s.watch.new.clone())
}

/// Hides a zip from the "New downloads" group.
pub fn dismiss_download(path: &Path) {
    pkg_done(path);
}

/// Per-frame hook: polls the Downloads folder and schedules the next look.
pub fn tick(ctx: &egui::Context) {
    if pages::current().render.watch_downloads {
        poll_downloads(Instant::now(), false);
        ctx.request_repaint_after(POLL_EVERY);
    } else if !pkg(|s| s.watch.new.is_empty()) {
        poll_downloads(Instant::now(), true);
    }
}

// ----- the Lightbeans folder -----

fn thumb(ui: &egui::Ui, def: &MaterialDef) -> Option<egui::TextureHandle> {
    let path = def.texture_path.clone()?;
    if let Some(t) = pkg(|s| s.thumbs.get(&path).cloned()) {
        return t;
    }
    let handle = plan_library::image::decode_file(Path::new(&path))
        .ok()
        .map(|img| img.downscaled(THUMB_SIDE))
        .map(|img| {
            let ci = egui::ColorImage::from_rgba_unmultiplied(
                [img.width as usize, img.height as usize],
                &img.rgba,
            );
            ui.ctx().load_texture(
                format!("lightbeans_{path}"),
                ci,
                egui::TextureOptions::LINEAR,
            )
        });
    pkg(|s| s.thumbs.insert(path, handle.clone()));
    handle
}

/// The Lightbeans folder at the top of the Library Browser's Materials view:
/// the import and browse rows, the "New downloads" group and the imported
/// packages with thumbnails. Returns the material to edit when a package row
/// was double-clicked.
pub fn lightbeans_folder(ui: &mut egui::Ui, cx: &mut EditorContext) {
    let packages = imported_packages();
    let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
    egui::CollapsingHeader::new(format!("Lightbeans ({})", packages.len()))
        .id_salt("materials_lightbeans")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Import Material Package...").clicked() {
                    import_command(cx);
                }
            });
            if ui
                .link("Browse Lightbeans textures...")
                .on_hover_text(LIGHTBEANS_URL)
                .clicked()
            {
                browse_lightbeans(cx);
            }
            ui.weak(if hovering {
                "Drop the zip to import it"
            } else {
                "Drop a downloaded .zip on the window to import it"
            });
            let news = new_downloads();
            if !news.is_empty() {
                ui.separator();
                ui.strong("New downloads");
                for p in news {
                    ui.horizontal(|ui| {
                        ui.label(
                            p.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                        );
                        if ui.button("Import").clicked() {
                            import_zips(cx, std::slice::from_ref(&p));
                        }
                        if ui.small_button("Dismiss").clicked() {
                            dismiss_download(&p);
                        }
                    });
                }
                ui.separator();
            }
            if packages.is_empty() {
                ui.weak("No packages imported yet");
            }
            let active = super::active_material();
            for def in &packages {
                let tex = thumb(ui, def);
                let resp = ui
                    .horizontal(|ui| {
                        match &tex {
                            Some(t) => {
                                ui.image(egui::load::SizedTexture::new(
                                    t.id(),
                                    egui::vec2(THUMB_SIDE as f32 * 0.6, THUMB_SIDE as f32 * 0.6),
                                ));
                            }
                            None => swatch(ui, def.color),
                        }
                        let mut text = def.name.clone();
                        if !def.manufacturer.is_empty() {
                            text.push_str(&format!("  \u{b7}  {}", def.manufacturer));
                        }
                        ui.selectable_label(active.as_deref() == Some(def.name.as_str()), text)
                    })
                    .inner;
                if resp.clicked() {
                    set_active(Some(def.name.clone()));
                }
                if resp.double_clicked() {
                    super::open_spec_for(def);
                }
                resp.on_hover_text(format!(
                    "{} x {} in tile, imported {}",
                    def.texture_scale_in.0,
                    def.texture_scale_in.1,
                    def.package_imported.as_deref().unwrap_or("?")
                ));
            }
        });
}

#[cfg(test)]
pub(crate) mod tests;
