//! Chief Architect catalogs inside the app (backend, no UI).
//!
//! Daniel's installed Chief catalogs are read **in place** through
//! `plan-calib`; nothing is copied or saved into the plan or the repo. This
//! module keeps, per UI thread:
//!
//! * the preference ([`ChiefSettings`]: on/off and an install-folder override,
//!   stored under `"chief_catalogs"` in `~/.plan-studio/settings.json`);
//! * the discovered [`ChiefLibrary`] and the catalogs opened so far (they stay
//!   open: a handful of file handles);
//! * a transient in-memory catalog of bridged objects. Clicking a Chief object
//!   in the Library Browser bridges it (`plan_calib::bridge`) into a
//!   [`CatalogItem`] with id `chief.<catalog-uuid>.<object id>` so placement,
//!   wall auto-rotate, handles and the Symbol Specification work like for a
//!   built-in item;
//! * the decoded-mesh cache used by the 3D view.
//!
//! A plan only stores the id. After a reload the item is bridged again on
//! first use ([`resolve_item`]).

use plan_3d::Mesh;
use plan_calib::bridge::{bridge_object, BridgeStats};
use plan_calib::mesh3d::{parts_extent, place_triangles, MeshCache};
use plan_calib::{
    CatalogKind, ChiefCatalog, ChiefLibrary, ChiefRegistry, ObjectSummary, RegistryEntry,
};
use plan_core::PlacedSymbol;
use plan_library::CatalogItem;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Prefix of every Chief catalog id.
pub const ID_PREFIX: &str = "chief.";
/// A mesh whose height is under this share of the object's size is partial.
pub const PARTIAL_HEIGHT_RATIO: f32 = 0.6;
/// The key in `settings.json`.
const SETTINGS_KEY: &str = "chief_catalogs";
/// Licensing note shown wherever Chief content is described.
pub const LICENSE_NOTE: &str =
    "Chief Architect licensed content, read in place from your installation. It is never copied \
     into your plans or this program.";

/// `chief.<catalog-uuid>.<object id>` split into its parts.
pub fn parse_id(id: &str) -> Option<(&str, i64)> {
    let rest = id.strip_prefix(ID_PREFIX)?;
    let (uuid, obj) = rest.rsplit_once('.')?;
    (!uuid.is_empty()).then_some(())?;
    Some((uuid, obj.parse().ok()?))
}

/// Whether `id` names a Chief object.
pub fn is_chief_id(id: &str) -> bool {
    id.starts_with(ID_PREFIX)
}

// ----- settings -----

/// The user's choice about Chief catalogs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChiefSettings {
    /// "Use Chief Architect catalogs".
    pub enabled: bool,
    /// "Catalog folders...": an install folder (holding `Core Libraries`,
    /// `Bonus Libraries`, `Manufacturer Libraries`) replacing the default.
    pub folder: Option<PathBuf>,
}

impl ChiefSettings {
    /// Off, no override (what tests and headless runs use).
    pub fn off() -> Self {
        Self {
            enabled: false,
            folder: None,
        }
    }

    /// Reads the `"chief_catalogs"` object of a settings document. Without
    /// an explicit choice the catalogs are on when an install folder exists.
    pub fn from_json(v: &Value) -> Self {
        let o = v.get(SETTINGS_KEY);
        let folder = o
            .and_then(|o| o.get("folder"))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from);
        let enabled = o
            .and_then(|o| o.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or_else(|| install_folder_exists(folder.as_deref()));
        Self { enabled, folder }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "enabled": self.enabled,
            "folder": self.folder.as_ref().map(|p| p.to_string_lossy().into_owned()),
        })
    }

    /// Loads `~/.plan-studio/settings.json` (defaults on any problem).
    pub fn load() -> Self {
        let doc = crate::paths::user_file("settings.json")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .unwrap_or(Value::Null);
        Self::from_json(&doc)
    }

    /// Writes the choice into the settings file, keeping its other keys.
    pub fn save(&self) -> Result<(), String> {
        let path = crate::paths::user_file("settings.json").ok_or(crate::paths::NO_HOME)?;
        self.save_to(&path)
    }

    /// [`save`](Self::save) to an explicit file.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let mut doc = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        doc[SETTINGS_KEY] = self.to_json();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

/// Whether Chief's install folder (the override, or the default X18/X17
/// folder under `/Library/Application Support`) exists.
pub fn install_folder_exists(folder: Option<&Path>) -> bool {
    match folder {
        Some(f) => f.is_dir(),
        None => ["X18", "X17"].iter().any(|v| {
            Path::new("/Library/Application Support")
                .join(format!("Chief Architect Premier {v}"))
                .is_dir()
        }),
    }
}

// ----- imported catalogs (Library > Import Library, .calib / .calibz) -----

/// Settings key of the catalogs the user added with Import Library.
const IMPORTS_KEY: &str = "library_imports";

thread_local! {
    /// Tests point the imports at a temporary file.
    static IMPORTS_PATH: RefCell<Option<Option<PathBuf>>> = const { RefCell::new(None) };
}

/// Makes the imported-catalog list live in `path` (or nowhere, for `None`)
/// instead of `~/.plan-studio/settings.json`. For tests.
#[cfg(test)]
pub(crate) fn set_imports_path(path: Option<Option<PathBuf>>) {
    IMPORTS_PATH.with(|p| *p.borrow_mut() = path);
}

fn imports_path() -> Option<PathBuf> {
    match IMPORTS_PATH.with(|p| p.borrow().clone()) {
        Some(over) => over,
        None => crate::paths::user_file("settings.json"),
    }
}

fn read_imports(doc: &Value) -> Vec<PathBuf> {
    doc.get(IMPORTS_KEY)
        .and_then(|o| o.get("catalogs"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

fn load_doc(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// The catalog files the user imported (read in place, never copied).
pub fn imported_catalogs() -> Vec<PathBuf> {
    imports_path()
        .map(|p| read_imports(&load_doc(&p)))
        .unwrap_or_default()
}

fn save_imports(list: &[PathBuf]) -> Result<(), String> {
    let path = imports_path().ok_or(crate::paths::NO_HOME)?;
    let mut doc = load_doc(&path);
    doc[IMPORTS_KEY] = json!({
        "catalogs": list.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
    });
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

/// Registry entries for the imported catalogs that still exist (kind User:
/// they list under the User Catalog node).
fn imported_entries() -> Vec<RegistryEntry> {
    imported_catalogs()
        .into_iter()
        .filter(|p| p.is_file())
        .map(|p| RegistryEntry {
            uuid: ChiefCatalog::peek_id(&p)
                .ok()
                .flatten()
                .unwrap_or_else(|| p.to_string_lossy().into_owned()),
            name: p
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned()),
            kind: CatalogKind::User,
            path: Some(p),
        })
        .collect()
}

/// Adds a Chief `.calib` / `.calibz` file to the Library Browser, read in
/// place and read-only (nothing is copied). Returns the catalog's name. The
/// file must open as a Chief catalog.
pub fn register_catalog(path: &Path) -> Result<String, String> {
    let cat = ChiefCatalog::open(path).map_err(|e| {
        format!(
            "{} is not a readable Chief Architect catalog: {e}",
            path.display()
        )
    })?;
    let name = path.file_stem().map_or_else(
        || cat.name().to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    let mut list = imported_catalogs();
    if !list.iter().any(|p| p == path) {
        list.push(path.to_path_buf());
        save_imports(&list)?;
    }
    Ok(name)
}

/// Removes an imported catalog from the Library Browser (the file stays).
pub fn unregister_catalog(path: &Path) -> Result<bool, String> {
    let mut list = imported_catalogs();
    let n = list.len();
    list.retain(|p| p != path);
    if list.len() == n {
        return Ok(false);
    }
    save_imports(&list)?;
    Ok(true)
}

// ----- discovery -----

/// Finds the catalogs: the standard registry and install folders, or, with an
/// override, `folder` as the install root (registry file from the standard
/// place when present, else a plain scan of the folders).
pub fn discover(folder: Option<&Path>) -> ChiefLibrary {
    let base = discover_install(folder);
    let extra = imported_entries();
    let custom_user = user_library_override();
    if extra.is_empty() && custom_user.is_none() {
        return base;
    }
    let mut entries = base.catalogs().to_vec();
    if let Some(user) = custom_user {
        // Preferences > Folders > User Library: that folder's catalog
        // stands in for the one in Chief's data folder.
        entries.retain(|e| e.kind != CatalogKind::User || e.name != "User Library");
        entries.push(user);
    }
    let fresh: Vec<RegistryEntry> = extra
        .into_iter()
        .filter(|e| !entries.iter().any(|x| x.path == e.path))
        .collect();
    entries.extend(fresh);
    ChiefLibrary::from_entries(entries)
}

/// The `User_Library.calib` of the folder chosen in Preferences > Folders >
/// User Library, when one is chosen and holds that file.
fn user_library_override() -> Option<RegistryEntry> {
    use crate::dialogs::preferences::pages::{self, FolderKind};
    let dir = pages::current()
        .folders
        .get(FolderKind::UserLibrary)?
        .to_string();
    let user = Path::new(&dir).join("User_Library.calib");
    user.is_file().then(|| RegistryEntry {
        uuid: ChiefCatalog::peek_id(&user)
            .ok()
            .flatten()
            .unwrap_or_default(),
        name: "User Library".into(),
        kind: CatalogKind::User,
        path: Some(user),
    })
}

/// The install's own catalogs (no imported ones).
fn discover_install(folder: Option<&Path>) -> ChiefLibrary {
    let Some(root) = folder else {
        return ChiefLibrary::discover();
    };
    let home = crate::paths::home_dir();
    let registry = home.as_ref().and_then(|h| {
        ["X18", "X17"].iter().find_map(|v| {
            let json = h
                .join("Library/Application Support")
                .join(format!("Chief Architect Premier {v}"))
                .join("Chief Library.json");
            json.is_file().then_some(json)
        })
    });
    let mut entries = registry
        .and_then(|json| {
            let cache = home
                .as_ref()
                .map(|h| h.join(".plan-studio/chief-catalog-index.json"));
            ChiefRegistry::load(&json, root, cache.as_deref()).ok()
        })
        .map(|r| r.entries)
        .unwrap_or_else(|| scan_folder(root));
    if entries.iter().all(|e| e.path.is_none()) {
        entries = scan_folder(root);
    }
    if let Some(h) = &home {
        let user = h.join(
            "Documents/Chief Architect Premier X18 Data/Database Libraries/User_Library.calib",
        );
        if user.is_file() {
            entries.push(RegistryEntry {
                uuid: ChiefCatalog::peek_id(&user)
                    .ok()
                    .flatten()
                    .unwrap_or_default(),
                name: "User Library".into(),
                kind: CatalogKind::User,
                path: Some(user),
            });
        }
    }
    ChiefLibrary::from_entries(entries)
}

/// Every `.calib`/`.calibz` under the three install sub-folders of `root`
/// (kind by folder) and directly in `root` (user catalogs).
pub fn scan_folder(root: &Path) -> Vec<RegistryEntry> {
    let mut out = Vec::new();
    let spots = [
        ("Core Libraries", CatalogKind::Core),
        ("Bonus Libraries", CatalogKind::Bonus),
        ("Manufacturer Libraries", CatalogKind::Manufacturer),
        ("", CatalogKind::User),
    ];
    for (sub, kind) in spots {
        let dir = if sub.is_empty() {
            root.to_path_buf()
        } else {
            root.join(sub)
        };
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        for path in files {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase());
            let is_calib = ext.as_deref() == Some("calib");
            if !is_calib && ext.as_deref() != Some("calibz") {
                continue;
            }
            let name = path
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            let uuid = is_calib
                .then(|| ChiefCatalog::peek_id(&path).ok().flatten())
                .flatten()
                .unwrap_or_else(|| name.clone());
            out.push(RegistryEntry {
                uuid,
                name,
                kind,
                path: Some(path),
            });
        }
    }
    out
}

// ----- the per-thread state -----

/// A bridged object plus where it came from.
#[derive(Clone, Debug)]
pub struct ChiefItem {
    pub item: Arc<CatalogItem>,
    /// Display name of the source catalog.
    pub catalog_name: String,
}

struct Global {
    settings: ChiefSettings,
    library: Option<Arc<ChiefLibrary>>,
    catalogs: HashMap<String, Arc<ChiefCatalog>>,
    items: HashMap<String, ChiefItem>,
    /// Ids that could not be bridged (not retried every frame).
    failed: HashSet<String>,
    meshes: MeshCache,
    /// Ids whose 3D geometry is partial or missing (drawn as boxes).
    partial: HashSet<String>,
    notice: bool,
}

impl Global {
    fn new() -> Self {
        Self {
            settings: ChiefSettings::off(),
            library: None,
            catalogs: HashMap::new(),
            items: HashMap::new(),
            failed: HashSet::new(),
            meshes: MeshCache::default(),
            partial: HashSet::new(),
            notice: false,
        }
    }

    fn library(&mut self) -> Option<Arc<ChiefLibrary>> {
        if !self.settings.enabled {
            return None;
        }
        if self.library.is_none() {
            // Normally the browser's background scan has filled this in; this
            // is the blocking path for a plan that already has Chief symbols.
            self.library = Some(Arc::new(discover(self.settings.folder.as_deref())));
        }
        self.library.clone()
    }

    fn catalog(&mut self, uuid: &str) -> Option<Arc<ChiefCatalog>> {
        if let Some(c) = self.catalogs.get(uuid) {
            return Some(c.clone());
        }
        let lib = self.library()?;
        let idx = lib
            .catalogs()
            .iter()
            .position(|e| e.uuid == uuid && e.path.is_some())?;
        let cat = Arc::new(lib.open(idx).ok()?);
        self.catalogs.insert(uuid.to_owned(), cat.clone());
        Some(cat)
    }

    fn rehydrate(&mut self, id: &str) -> Option<ChiefItem> {
        let (uuid, oid) = parse_id(id)?;
        if self.failed.contains(id) {
            return None;
        }
        let found = (|| {
            let cat = self.catalog(uuid)?;
            let obj = cat.objects().ok()?.find(|o| o.library_object_id == oid)?;
            bridged(&cat, &obj).ok()
        })();
        match found {
            Some(ci) => {
                self.items.insert(id.to_owned(), ci.clone());
                Some(ci)
            }
            None => {
                self.failed.insert(id.to_owned());
                None
            }
        }
    }
}

thread_local! {
    static STATE: RefCell<Global> = RefCell::new(Global::new());
}

fn with<R>(f: impl FnOnce(&mut Global) -> R) -> R {
    STATE.with(|g| f(&mut g.borrow_mut()))
}

/// Applies the preference. Changing the folder forgets the discovered
/// library, the open catalogs and the mesh cache (bridged items stay: a
/// placed symbol must keep its drawing).
pub fn configure(settings: &ChiefSettings) {
    with(|g| {
        if g.settings.folder != settings.folder {
            g.library = None;
            g.catalogs.clear();
            g.meshes.clear();
            g.failed.clear();
            // The door styles scanned from the old install are out of date.
            super::door_styles::forget_chief();
        }
        if settings.enabled && !g.settings.enabled {
            g.failed.clear();
        }
        g.settings = settings.clone();
    });
}

/// Whether Chief catalogs are switched on.
pub fn enabled() -> bool {
    with(|g| g.settings.enabled)
}

/// Hands over the library a background scan found.
pub fn set_library(lib: Arc<ChiefLibrary>) {
    with(|g| {
        g.library = Some(lib);
        g.catalogs.clear();
    });
    super::door_styles::forget_chief();
}

/// The discovered library, scanning now when no background scan ran yet and
/// the catalogs are enabled.
pub fn library() -> Option<Arc<ChiefLibrary>> {
    with(Global::library)
}

/// The open catalog with this UUID (opened on first use, then kept).
pub fn catalog(uuid: &str) -> Option<Arc<ChiefCatalog>> {
    with(|g| g.catalog(uuid))
}

/// Bridges one object into a [`ChiefItem`] (reads its blobs).
pub fn bridged(cat: &ChiefCatalog, obj: &ObjectSummary) -> plan_calib::Result<ChiefItem> {
    let mut stats = BridgeStats::default();
    let (item, _thumb) = bridge_object(cat, obj, &mut stats)?;
    Ok(ChiefItem {
        item: Arc::new(item),
        catalog_name: cat.name().to_owned(),
    })
}

/// Bridges `obj` (cached by id) and keeps it in the transient catalog.
pub fn install_object(cat: &ChiefCatalog, obj: &ObjectSummary) -> plan_calib::Result<ChiefItem> {
    let id = format!("{ID_PREFIX}{}.{}", cat.id(), obj.library_object_id);
    if let Some(hit) = with(|g| g.items.get(&id).cloned()) {
        return Ok(hit);
    }
    let ci = bridged(cat, obj)?;
    with(|g| g.items.insert(ci.item.id.clone(), ci.clone()));
    Ok(ci)
}

/// Puts an already built item into the transient catalog.
pub fn install_item(item: CatalogItem, catalog_name: &str) -> Arc<CatalogItem> {
    let ci = ChiefItem {
        item: Arc::new(item),
        catalog_name: catalog_name.to_owned(),
    };
    with(|g| g.items.insert(ci.item.id.clone(), ci.clone()));
    ci.item
}

/// The transient item for `id`, without trying to bridge it.
pub fn installed(id: &str) -> Option<ChiefItem> {
    with(|g| g.items.get(id).cloned())
}

/// The item for a Chief id: from the transient catalog, else bridged from the
/// installed catalog now (once; failures are remembered).
pub fn resolve_item(id: &str) -> Option<ChiefItem> {
    if !is_chief_id(id) {
        return None;
    }
    with(|g| match g.items.get(id) {
        Some(ci) => Some(ci.clone()),
        None => g.rehydrate(id),
    })
}

// ----- 3D -----

/// What the 3D view draws for a placed Chief symbol.
#[derive(Debug)]
pub enum Chief3d {
    /// The decoded geometry fitted to the symbol.
    Meshes(Vec<Mesh>),
    /// No geometry or only part of the object: draw the box instead.
    Box,
    /// The catalog is not available: draw nothing.
    Missing,
}

/// Meshes for a placed Chief symbol (see [`Chief3d`]). Objects whose
/// decoded height is under [`PARTIAL_HEIGHT_RATIO`] of their size are
/// reported as [`Chief3d::Box`] and counted for [`take_partial_notice`].
pub fn placed_meshes(placed: &PlacedSymbol, floor_elevation: f64) -> Chief3d {
    let Some((uuid, oid)) = parse_id(&placed.catalog_id) else {
        return Chief3d::Missing;
    };
    with(|g| {
        let Some(cat) = g.catalog(uuid) else {
            return Chief3d::Missing;
        };
        let Ok(parts) = g.meshes.get_or_decode(&cat, oid) else {
            return Chief3d::Missing;
        };
        let reference = g
            .items
            .get(&placed.catalog_id)
            .map_or(placed.height, |c| c.item.height) as f32;
        let height = parts_extent(&parts)[2];
        if parts.is_empty() || height < reference * PARTIAL_HEIGHT_RATIO {
            if g.partial.insert(placed.catalog_id.clone()) {
                g.notice = true;
            }
            return Chief3d::Box;
        }
        Chief3d::Meshes(place_triangles(&parts, placed, floor_elevation))
    })
}

/// A status-bar sentence, once, after objects were found to have partial
/// 3D geometry.
pub fn take_partial_notice() -> Option<String> {
    with(|g| {
        if !std::mem::take(&mut g.notice) {
            return None;
        }
        let n = g.partial.len();
        Some(format!(
            "3D: {n} Chief object{} ha{} only partial geometry and show{} as a box",
            if n == 1 { "" } else { "s" },
            if n == 1 { "s" } else { "ve" },
            if n == 1 { "s" } else { "" },
        ))
    })
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    with(|g| *g = Global::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_library_folder_of_the_preferences_names_the_user_catalog() {
        use crate::dialogs::preferences::pages::{self, FolderKind};
        let dir = std::env::temp_dir().join(format!("plan-studio-userlib-{}", std::process::id()));
        let root = dir.join("root");
        std::fs::create_dir_all(&root).unwrap();
        let file = dir.join("User_Library.calib");
        std::fs::write(&file, b"not a real catalog").unwrap();
        pages::update(|p| {
            p.folders
                .set(FolderKind::UserLibrary, &dir.display().to_string())
        });
        let lib = discover(Some(&root));
        let user: Vec<_> = lib
            .catalogs()
            .iter()
            .filter(|e| e.kind == CatalogKind::User && e.name == "User Library")
            .collect();
        assert_eq!(user.len(), 1);
        assert_eq!(user[0].path.as_deref(), Some(file.as_path()));
        pages::set(pages::PagePrefs::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ids_round_trip() {
        assert_eq!(
            parse_id("chief.3f2a-11.42"),
            Some(("3f2a-11", 42)),
            "uuid and object id"
        );
        assert_eq!(parse_id("core.plumbing.toilet"), None);
        assert_eq!(parse_id("chief..5"), None);
        assert_eq!(parse_id("chief.abc.x"), None);
        assert!(is_chief_id("chief.a.1"));
        assert!(!is_chief_id("core.a.1"));
    }

    #[test]
    fn settings_default_follow_the_install_and_round_trip() {
        let none = ChiefSettings::from_json(&Value::Null);
        assert_eq!(none.folder, None);
        assert_eq!(none.enabled, install_folder_exists(None));
        let s = ChiefSettings {
            enabled: false,
            folder: Some(PathBuf::from("/some/where")),
        };
        assert_eq!(
            ChiefSettings::from_json(&json!({ "chief_catalogs": s.to_json() })),
            s
        );
        // An explicit "off" beats the install-folder default.
        let off = ChiefSettings::from_json(&json!({"chief_catalogs": {"enabled": false}}));
        assert!(!off.enabled);
    }

    #[test]
    fn saving_keeps_the_other_settings() {
        let dir = std::env::temp_dir().join(format!("plan-chief-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        std::fs::write(&file, r#"{"theme":"dark","brightness":0.8}"#).unwrap();
        let s = ChiefSettings {
            enabled: true,
            folder: Some(PathBuf::from("/x")),
        };
        s.save_to(&file).unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(doc["theme"], "dark");
        assert_eq!(ChiefSettings::from_json(&doc), s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_folder_lists_catalog_files_by_kind() {
        let dir = std::env::temp_dir().join(format!("plan-chief-scan-{}", std::process::id()));
        let core = dir.join("Core Libraries");
        std::fs::create_dir_all(&core).unwrap();
        std::fs::write(core.join("Alpha.calibz"), b"x").unwrap();
        std::fs::write(core.join("notes.txt"), b"x").unwrap();
        std::fs::write(dir.join("Mine.calibz"), b"x").unwrap();
        let found = scan_folder(&dir);
        let kinds: Vec<_> = found.iter().map(|e| (e.name.as_str(), e.kind)).collect();
        assert_eq!(
            kinds,
            [("Alpha", CatalogKind::Core), ("Mine", CatalogKind::User)]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn disabled_catalogs_resolve_nothing_and_installed_items_are_found() {
        reset_for_tests();
        configure(&ChiefSettings::off());
        assert!(library().is_none());
        assert!(resolve_item("chief.nope.1").is_none());
        let item = CatalogItem::new(
            "chief.abc.7",
            "Thing",
            plan_library::Placement::FreeStanding,
            plan_library::Symbol2d::default(),
        );
        install_item(item, "Cat");
        let ci = resolve_item("chief.abc.7").unwrap();
        assert_eq!(
            (ci.item.name.as_str(), ci.catalog_name.as_str()),
            ("Thing", "Cat")
        );
        // Without a catalog the 3D view draws nothing for it.
        let s = PlacedSymbol::new(
            "chief.abc.7",
            plan_core::geometry::Point::ZERO,
            1.0,
            1.0,
            1.0,
        );
        assert!(matches!(placed_meshes(&s, 0.0), Chief3d::Missing));
    }

    fn summary(id: i64, name: &str) -> ObjectSummary {
        ObjectSummary {
            library_object_id: id,
            unique_id: format!("u-{id}"),
            type_code: 0,
            name: name.into(),
            category_path: vec!["Interiors".into(), "Bath".into()],
            keywords: vec!["wc".into()],
            has_thumbnail: false,
            metric: false,
        }
    }

    #[test]
    fn bridging_a_synthetic_object_gives_the_chief_id() {
        use plan_calib::bridge::{item_from, SIZE_UNKNOWN_TAG};
        use plan_calib::decode::{decode_object, ObjectBlobs};
        reset_for_tests();
        let obj = summary(5, "Round Tank Toilet");
        let decoded = decode_object(&ObjectBlobs::default());
        let mut stats = BridgeStats::default();
        let item = item_from(
            "cafe-uuid",
            "Core Interiors",
            &obj,
            None,
            &decoded,
            &mut stats,
        );
        assert_eq!(item.id, "chief.cafe-uuid.5");
        assert_eq!(parse_id(&item.id), Some(("cafe-uuid", 5)));
        assert_eq!(item.name, "Round Tank Toilet");
        assert_eq!(item.category, ["Interiors", "Bath"]);
        assert!(item
            .tags
            .iter()
            .any(|t| t == "chief" || t == SIZE_UNKNOWN_TAG));
        assert!(!item.symbol.is_empty(), "a placeholder symbol is drawn");
        assert_eq!(stats.total, 1);

        // Installed, it is found by the placement code like a built-in item.
        install_item(item, "Core Interiors");
        let found = crate::tools::library::find_item("chief.cafe-uuid.5").unwrap();
        assert_eq!(found.name, "Round Tank Toilet");
        assert_eq!(
            crate::editor::placed::symbol_placement(&PlacedSymbol::new(
                "chief.cafe-uuid.5",
                plan_core::geometry::Point::ZERO,
                1.0,
                1.0,
                1.0
            )),
            plan_library::Placement::FreeStanding
        );
        let strokes = crate::editor::placed::placed_symbol_strokes(&PlacedSymbol::new(
            "chief.cafe-uuid.5",
            plan_core::geometry::Point::ZERO,
            found.width,
            found.depth,
            found.height,
        ));
        assert!(
            strokes.is_some_and(|s| !s.is_empty()),
            "2D uses the bridged symbol"
        );
    }

    #[test]
    fn imported_catalogs_are_listed_saved_and_forgotten() {
        let dir = std::env::temp_dir().join(format!(
            "ps-imports-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("settings.json");
        // The settings file keeps its other keys.
        std::fs::write(&settings, r#"{"theme":"dark"}"#).unwrap();
        set_imports_path(Some(Some(settings.clone())));
        assert!(imported_catalogs().is_empty());
        // Something that is no Chief catalog is refused and not remembered.
        let junk = dir.join("junk.calib");
        std::fs::write(&junk, "not a database").unwrap();
        assert!(register_catalog(&junk).is_err());
        assert!(imported_catalogs().is_empty());
        // The saved list round trips, ignoring files that vanished.
        save_imports(&[junk.clone(), dir.join("gone.calib")]).unwrap();
        assert_eq!(
            imported_catalogs(),
            vec![junk.clone(), dir.join("gone.calib")]
        );
        assert!(imported_entries().iter().all(|e| e.path.is_some()));
        let doc: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(doc["theme"], "dark");
        assert!(unregister_catalog(&junk).unwrap());
        assert!(!unregister_catalog(&junk).unwrap());
        assert_eq!(imported_catalogs(), vec![dir.join("gone.calib")]);
        set_imports_path(None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Library > Import Library on a real `.calib` that Chief's registry does
    /// not list (its Trash): added read-only, in place; run with `--ignored`.
    #[test]
    #[ignore = "reads Daniel's Chief data folder"]
    fn real_install_import_library_adds_a_calib_in_place() {
        let home = crate::paths::home_dir().expect("a home");
        let trash =
            home.join("Documents/Chief Architect Premier X18 Data/Database Libraries/Trash.calib");
        if !trash.is_file() {
            eprintln!("no Chief Trash.calib: skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("ps-imports-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        set_imports_path(Some(Some(dir.join("settings.json"))));
        let before = std::fs::metadata(&trash).unwrap().modified().unwrap();
        assert_eq!(register_catalog(&trash).unwrap(), "Trash");
        let lib = discover(None);
        let e = lib
            .catalogs()
            .iter()
            .position(|e| e.path.as_deref() == Some(trash.as_path()))
            .expect("the imported catalog is listed");
        assert_eq!(lib.catalogs()[e].kind, CatalogKind::User);
        lib.open(e).expect("it opens read-only");
        // Read in place: the file is untouched and nothing was copied.
        assert_eq!(
            std::fs::metadata(&trash).unwrap().modified().unwrap(),
            before
        );
        assert!(unregister_catalog(&trash).unwrap());
        set_imports_path(None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Needs Daniel's installed Chief catalogs; run with `--ignored`.
    #[test]
    #[ignore = "reads the real Chief X18 install"]
    fn real_install_round_tank_toilet() {
        use crate::shell::library_browser::png;
        reset_for_tests();
        let lib = discover(None);
        let st = lib.stats();
        eprintln!("catalogs: {st:?}");
        assert!(st.total >= 400 && st.installed >= 390, "{st:?}");
        let idx = lib
            .catalogs()
            .iter()
            .position(|e| e.name == "CoreArchitectural" && e.path.is_some())
            .expect("CoreArchitectural is installed");
        let cat = lib.open(idx).unwrap();
        let obj = cat
            .objects()
            .unwrap()
            .find(|o| o.name == "Round Tank Toilet")
            .expect("Round Tank Toilet in CoreArchitectural");
        let mut stats = BridgeStats::default();
        let (item, thumb) = bridge_object(&cat, &obj, &mut stats).unwrap();
        eprintln!(
            "{} {}x{}x{} symbol strokes {} stats {stats:?}",
            item.id,
            item.width,
            item.depth,
            item.height,
            item.symbol.strokes.len()
        );
        assert_eq!(stats.decoded_symbols, 1, "decoded plan symbol");
        assert_eq!(stats.decoded_sizes, 1, "decoded size");
        assert!(
            item.id.starts_with("chief.")
                && item.id.ends_with(&format!(".{}", obj.library_object_id))
        );

        let png_bytes = thumb.expect("a thumbnail");
        let img = png::decode(&png_bytes).expect("thumbnail decodes");
        eprintln!("thumbnail {}x{}", img.width, img.height);
        assert!(img.width > 16 && img.pixels.iter().any(|&b| b != 0));

        configure(&ChiefSettings {
            enabled: true,
            folder: None,
        });
        set_library(Arc::new(lib));
        let ci = install_object(&cat, &obj).unwrap();
        let placed = PlacedSymbol::new(
            ci.item.id.clone(),
            plan_core::geometry::Point::new(10.0, 10.0),
            ci.item.width,
            ci.item.depth,
            ci.item.height,
        );
        match placed_meshes(&placed, 0.0) {
            Chief3d::Meshes(m) => {
                let tris: usize = m.iter().map(Mesh::triangle_count).sum();
                eprintln!("meshes {} parts, {tris} triangles", m.len());
                assert!(!m.is_empty() && tris > 0);
            }
            other => panic!("expected meshes, got {other:?}"),
        }
        // A project reload only has the id: it bridges again on demand.
        reset_for_tests();
        configure(&ChiefSettings {
            enabled: true,
            folder: None,
        });
        let again = resolve_item(&placed.catalog_id).expect("rehydrates from the install");
        assert_eq!(again.item.name, "Round Tank Toilet");
        assert_eq!(again.catalog_name, "CoreArchitectural");
    }
}
