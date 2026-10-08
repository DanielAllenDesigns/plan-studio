//! The User Catalog: the user's own library items, their 3D models, folders,
//! favorites and recents, import and export, and the commands that save plan
//! objects into it.
//!
//! The items live in `~/.plan-studio/user-library.json` (the store of
//! [`crate::tools::images`], shared with Create Image Library). Next to it:
//!
//! * `user-library-meta.json`: folders (empty ones included), favorites and
//!   the recently-used list ([`plan_library::manage::UserMeta`]);
//! * `user-models/<id>.psm`: the 3D mesh of an item
//!   ([`plan_library::Model3d`]); the item's `model3d` names the file.
//!
//! Tests point the store at no file (everything stays in memory) or at a
//! temporary folder with [`crate::tools::images::set_user_library_path`].
//! Items, meta and models are cached per thread and reloaded when that path
//! changes.
//!
//! **Export Library** writes a zip with the extension `.calibz` holding the
//! catalog JSON and the `.psm` models ([`plan_library::archive`]). Chief
//! Architect cannot read it; Import Library reads only such Plan Studio
//! exports.

use super::make;
use crate::editor::placed::{self, symbol_center};
use crate::editor::{EditorContext, ObjectRef};
use crate::tools::images;
use plan_cabinets::Cabinet;
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::{Id, PlacedSymbol};
use plan_import::{ModelOptions, UpAxis};
use plan_library::manage::{self, UserMeta, USER_ROOT};
use plan_library::{archive, CatalogItem, ItemKind, Model3d, Placement};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// File name of the folders / favorites / recents file.
pub const META_FILE: &str = "user-library-meta.json";
/// Folder (next to the library file) of the model files.
pub const MODEL_DIR: &str = archive::MODEL_DIR;

/// Command ids (menu rows carry these; `run_command` runs them).
pub const ADD_SELECTION: &str = "library.add_selection";
pub const ADD_MATERIAL: &str = "library.add_material";
pub const IMPORT_MODEL: &str = "library.import_model";
pub const EXPORT_LIBRARY: &str = "library.export";
pub const IMPORT_LIBRARY: &str = "library.import";

// ----- state -----

#[derive(Default)]
struct State {
    /// The library path the data was loaded for.
    key: Option<Option<PathBuf>>,
    meta: UserMeta,
    models: HashMap<String, Arc<Model3d>>,
    /// Requests for the Library Browser (windows to open).
    requests: Vec<UiRequest>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Forgets the cached meta, models and requests, as after a restart (tests
/// use this to prove what reached the files).
#[cfg(test)]
pub(crate) fn forget_cache() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

/// Something a command asks the Library Browser panel to show.
#[derive(Debug, Clone, PartialEq)]
pub enum UiRequest {
    /// Open the Import 3D Model window for this file.
    ImportModel(PathBuf),
    /// Open Object Information for this item.
    ObjectInfo(String),
}

/// The folder the library file lives in.
pub fn library_dir() -> Option<PathBuf> {
    images::user_library_path().and_then(|p| p.parent().map(Path::to_path_buf))
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let key = images::user_library_path();
        if s.key.as_ref() != Some(&key) {
            s.models.clear();
            s.meta = key
                .as_ref()
                .and_then(|p| p.parent().map(|d| d.join(META_FILE)))
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|t| UserMeta::from_json(&t))
                .unwrap_or_default();
            s.key = Some(key);
        }
        f(&mut s)
    })
}

fn save_meta(meta: &UserMeta) {
    if let Some(dir) = library_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(META_FILE), meta.to_json());
    }
}

/// Edits the meta and saves it.
fn edit_meta<R>(f: impl FnOnce(&mut UserMeta) -> R) -> R {
    with_state(|s| {
        let r = f(&mut s.meta);
        save_meta(&s.meta);
        r
    })
}

/// Folders, favorites and recents.
pub fn meta() -> UserMeta {
    with_state(|s| s.meta.clone())
}

/// Records that `id` was used (placed): it leads the recent list.
pub fn touch_recent(id: &str) {
    edit_meta(|m| m.add_recent(id));
}

/// Stars or un-stars `id`; returns the new state.
pub fn toggle_favorite(id: &str) -> bool {
    edit_meta(|m| m.toggle_favorite(id))
}

/// Queues a request for the Library Browser.
pub fn push_request(r: UiRequest) {
    with_state(|s| s.requests.push(r));
}

/// Takes the oldest queued request.
pub fn take_request() -> Option<UiRequest> {
    with_state(|s| (!s.requests.is_empty()).then(|| s.requests.remove(0)))
}

// ----- models -----

fn model_file_name(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    format!("{MODEL_DIR}{safe}.psm")
}

/// The 3D model of `item`: from the cache, else read from its file.
pub fn model_of(item: &CatalogItem) -> Option<Arc<Model3d>> {
    let path = item.model3d.as_deref()?;
    if let Some(m) = with_state(|s| s.models.get(path).cloned()) {
        return Some(m);
    }
    let bytes = std::fs::read(library_dir()?.join(path)).ok()?;
    let model = Arc::new(Model3d::from_bytes(&bytes).ok()?);
    with_state(|s| s.models.insert(path.to_string(), model.clone()));
    Some(model)
}

/// Does `item` carry a 3D model?
pub fn has_model(item: &CatalogItem) -> bool {
    model_of(item).is_some()
}

fn store_model(path: &str, model: &Model3d) -> Result<(), String> {
    with_state(|s| s.models.insert(path.to_string(), Arc::new(model.clone())));
    if let Some(dir) = library_dir() {
        let full = dir.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&full, model.to_bytes())
            .map_err(|e| format!("Cannot save {}: {e}", full.display()))?;
    }
    Ok(())
}

/// Removes model files no item refers to any more.
fn prune_models(items: &[CatalogItem]) {
    let used: Vec<&str> = items.iter().filter_map(|i| i.model3d.as_deref()).collect();
    with_state(|s| s.models.retain(|p, _| used.contains(&p.as_str())));
    if let Some(dir) = library_dir() {
        if let Ok(rd) = std::fs::read_dir(dir.join(MODEL_DIR.trim_end_matches('/'))) {
            for e in rd.flatten() {
                let rel = format!("{MODEL_DIR}{}", e.file_name().to_string_lossy());
                if !used.contains(&rel.as_str()) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
}

// ----- items -----

/// The user library's items (a copy).
pub fn items() -> Vec<CatalogItem> {
    images::user_items().iter().map(|i| (**i).clone()).collect()
}

/// The user item with this id.
pub fn item(id: &str) -> Option<Arc<CatalogItem>> {
    images::user_item(id)
}

/// The folder new items go to by default, per kind.
pub fn default_folder(kind: ItemKind) -> Vec<String> {
    let leaf = match kind {
        ItemKind::Symbol | ItemKind::Fixture | ItemKind::Device => "Symbols",
        ItemKind::Cabinet => "Cabinets",
        ItemKind::CadBlock => "CAD Blocks",
        ItemKind::Text => "Text",
        ItemKind::Material => "Materials",
        ItemKind::Image => "Images",
        ItemKind::Model => "3D Models",
    };
    vec![USER_ROOT.to_string(), leaf.to_string()]
}

/// A fresh item id such as `user.model.4`.
pub fn new_id(kind: ItemKind) -> String {
    let prefix = format!("user.{}", kind.label().to_lowercase().replace(' ', ""));
    manage::unique_id(&items(), &prefix)
}

/// Saves `item` (replacing one with the same id) with its 3D `model`.
/// Gives the item the model file name when there is a model.
pub fn add(mut item: CatalogItem, model: Option<&Model3d>) -> Result<Arc<CatalogItem>, String> {
    if let Some(m) = model {
        let path = model_file_name(&item.id);
        store_model(&path, m)?;
        item.model3d = Some(path);
    }
    images::register_user_item(item)
}

/// Replaces an edited item (Object Information).
pub fn update(item: CatalogItem) -> Result<Arc<CatalogItem>, String> {
    images::register_user_item(item)
}

/// Deletes an item and its model file (unless a copy still uses it).
pub fn delete(id: &str) -> Result<bool, String> {
    let removed = images::remove_user_item(id)?;
    if removed {
        edit_meta(|m| m.forget(id));
        prune_models(&items());
    }
    Ok(removed)
}

/// Renames an item.
pub fn rename(id: &str, name: &str) -> Result<(), String> {
    let mut all = items();
    manage::rename_item(&mut all, id, name)?;
    images::set_user_items(all)
}

/// Duplicates an item next to the original; returns the copy's id. A copy
/// of a model item shares the model file.
pub fn duplicate(id: &str) -> Result<String, String> {
    let mut all = items();
    let new = manage::duplicate_item(&mut all, id).ok_or("No such item")?;
    images::set_user_items(all)?;
    Ok(new)
}

/// Moves an item to `folder`.
pub fn move_to(id: &str, folder: &[String]) -> Result<(), String> {
    let mut all = items();
    if !manage::move_item(&mut all, id, folder) {
        return Err("Cannot move that item there".into());
    }
    images::set_user_items(all)
}

// ----- folders -----

/// Every user folder path.
pub fn folders() -> Vec<Vec<String>> {
    meta().all_folders(&items())
}

/// Creates `User > ...` folder `path`.
pub fn create_folder(path: &[String]) -> Result<(), String> {
    edit_meta(|m| m.create_folder(path))
}

/// Renames the folder at `path`.
pub fn rename_folder(path: &[String], new_name: &str) -> Result<usize, String> {
    let mut all = items();
    let n = edit_meta(|m| m.rename_folder(&mut all, path, new_name))?;
    images::set_user_items(all)?;
    Ok(n)
}

/// Moves folder `path` into `new_parent`.
pub fn move_folder(path: &[String], new_parent: &[String]) -> Result<usize, String> {
    let mut all = items();
    let n = edit_meta(|m| m.move_folder(&mut all, path, new_parent))?;
    images::set_user_items(all)?;
    Ok(n)
}

/// Deletes the folder at `path` with everything inside; returns the number of
/// items removed.
pub fn delete_folder(path: &[String]) -> Result<usize, String> {
    let mut all = items();
    let gone = edit_meta(|m| m.delete_folder(&mut all, path));
    images::set_user_items(all.clone())?;
    prune_models(&all);
    Ok(gone.len())
}

// ----- Add to Library -----

fn folder_or_default(folder: Option<&[String]>, kind: ItemKind) -> Vec<String> {
    match folder {
        Some(f) if f.len() >= 2 && f[0] == USER_ROOT => f.to_vec(),
        _ => default_folder(kind),
    }
}

/// Saves a placed symbol (its drawing, size and 3D model) as a library item.
pub fn add_symbol(sym: &PlacedSymbol, folder: Option<&[String]>) -> Result<Arc<CatalogItem>, String> {
    let kind = crate::tools::library::find_item(&sym.catalog_id).map_or(ItemKind::Symbol, |i| i.kind);
    let id = new_id(kind);
    let (item, model) = make::item_from_symbol(sym, &id, &folder_or_default(folder, kind))?;
    add(item, model.as_ref())
}

/// Copies a library item (a built-in one, say) into the user catalog.
pub fn add_item_copy(src: &CatalogItem, folder: Option<&[String]>) -> Result<Arc<CatalogItem>, String> {
    let id = new_id(src.kind);
    let (item, model) = make::item_from_item(src, &id, &folder_or_default(folder, src.kind))?;
    add(item, model.as_ref())
}

/// Saves a cabinet.
pub fn add_cabinet(cab: &Cabinet, folder: Option<&[String]>) -> Result<Arc<CatalogItem>, String> {
    let id = new_id(ItemKind::Cabinet);
    let (item, model) = make::item_from_cabinet(cab, &id, &folder_or_default(folder, ItemKind::Cabinet))?;
    add(item, Some(&model))
}

/// Saves CAD items as a block (or a text item when all are text).
pub fn add_cad(items_: &[CadItem], name: &str, folder: Option<&[String]>) -> Result<Arc<CatalogItem>, String> {
    let all_text = items_.iter().all(|i| matches!(i, CadItem::Text { .. }));
    let kind = if all_text { ItemKind::Text } else { ItemKind::CadBlock };
    let id = new_id(kind);
    let item = make::item_from_cad(items_, &id, name, &folder_or_default(folder, kind))?;
    add(item, None)
}

/// Saves a material as a swatch.
pub fn add_material(name: &str, color: [u8; 3], folder: Option<&[String]>) -> Result<Arc<CatalogItem>, String> {
    let id = new_id(ItemKind::Material);
    let (item, model) =
        make::item_from_material(name, color, &id, &folder_or_default(folder, ItemKind::Material));
    add(item, Some(&model))
}

/// Saves the selected symbols, cabinets, CAD objects and text as library
/// items; returns their names. Fails when nothing savable is selected.
pub fn add_selection(cx: &EditorContext, folder: Option<&[String]>) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let floor = cx.floor();
    let mut cad_items: Vec<CadItem> = Vec::new();
    let mut last_err: Option<String> = None;
    for obj in &cx.selection.items {
        match *obj {
            ObjectRef::Symbol(id) => {
                let Some(sym) = floor.symbol(id) else { continue };
                match add_symbol(sym, folder) {
                    Ok(i) => names.push(i.name.clone()),
                    Err(e) => last_err = Some(e),
                }
            }
            ObjectRef::Cabinet(id) => {
                let Some(cab) = placed::cabinet_by_id(floor, id) else {
                    continue;
                };
                match add_cabinet(&cab, folder) {
                    Ok(i) => names.push(i.name.clone()),
                    Err(e) => last_err = Some(e),
                }
            }
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                if let Some(o) = floor.cad.iter().find(|c| c.id == id) {
                    cad_items.push(o.item.clone());
                }
            }
            _ => {}
        }
    }
    let (texts, shapes): (Vec<_>, Vec<_>) = cad_items
        .into_iter()
        .partition(|i| matches!(i, CadItem::Text { .. }));
    if !shapes.is_empty() {
        let n = new_id(ItemKind::CadBlock);
        let name = format!("CAD Block {}", n.rsplit('.').next().unwrap_or("1"));
        match add_cad(&shapes, &name, folder) {
            Ok(i) => names.push(i.name.clone()),
            Err(e) => last_err = Some(e),
        }
    }
    for t in texts {
        let name = match &t {
            CadItem::Text { text, .. } => text.chars().take(24).collect::<String>(),
            _ => String::new(),
        };
        let name = if name.trim().is_empty() { "Text".to_string() } else { name };
        match add_cad(&[t], &name, folder) {
            Ok(i) => names.push(i.name.clone()),
            Err(e) => last_err = Some(e),
        }
    }
    if names.is_empty() {
        return Err(last_err.unwrap_or_else(|| {
            "Select a symbol, cabinet, CAD object or text first".to_string()
        }));
    }
    Ok(names)
}

// ----- importing 3D models -----

/// What the Import 3D Model window collects.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelImport {
    /// Item name.
    pub name: String,
    /// Folder in the User catalog.
    pub folder: Vec<String>,
    /// Inches per source unit, and which axis is up.
    pub options: ModelOptions,
    /// Placement; `None` follows the folder and name.
    pub placement: Option<Placement>,
}

/// File-type defaults: glTF is meters, Y up; OBJ is taken as inches, Y up.
pub fn import_defaults(path: &Path) -> ModelImport {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let options = if ext == "gltf" || ext == "glb" {
        plan_import::gltf::default_options()
    } else {
        ModelOptions {
            unit_scale: 1.0,
            up_axis: UpAxis::Y,
        }
    };
    ModelImport {
        name: path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Model")
            .replace(['_', '-'], " "),
        folder: default_folder(ItemKind::Model),
        options,
        placement: None,
    }
}

/// Reads the model file at `path` (and its `.mtl` / `.bin` neighbors).
pub fn parse_model_file(path: &Path, opts: &ModelOptions) -> Result<plan_import::ImportedModel, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mtl = if ext.eq_ignore_ascii_case("obj") {
        plan_import::obj::mtl_libraries(&String::from_utf8_lossy(&bytes))
            .into_iter()
            .find_map(|n| std::fs::read_to_string(dir.join(n)).ok())
    } else {
        None
    };
    let resolver = |name: &str| std::fs::read(dir.join(name)).ok();
    make::parse_model(ext, &bytes, mtl.as_deref(), Some(&resolver), opts)
}

/// Makes the library item and model for an import without saving them.
pub fn build_model_item(
    imported: &plan_import::ImportedModel,
    settings: &ModelImport,
) -> Result<(CatalogItem, Model3d), String> {
    let model = make::model_from_import(imported);
    if model.is_empty() {
        return Err("The file has no triangles".into());
    }
    let id = new_id(ItemKind::Model);
    let name = manage::valid_name(&settings.name)?;
    let folder = folder_or_default(Some(&settings.folder), ItemKind::Model);
    let item = make::item_from_model(&model, &id, &name, &folder, settings.placement)?;
    Ok((item, model))
}

/// Imports the model file at `path` into the user catalog.
pub fn import_model_file(path: &Path, settings: &ModelImport) -> Result<Arc<CatalogItem>, String> {
    let imported = parse_model_file(path, &settings.options)?;
    let (item, model) = build_model_item(&imported, settings)?;
    add(item, Some(&model))
}

// ----- export and import of the whole library -----

/// The user library as an export archive (see [`plan_library::archive`]).
pub fn export_bytes() -> Result<Vec<u8>, String> {
    let all = items();
    let meta = meta();
    archive::export_library(&all, &meta, &|p| {
        if let Some(m) = with_state(|s| s.models.get(p).cloned()) {
            return Some(m.to_bytes());
        }
        std::fs::read(library_dir()?.join(p)).ok()
    })
    .map_err(|e| e.0)
}

/// Writes the export archive to `path` (a `.calibz` file).
pub fn export_library_to(path: &Path) -> Result<usize, String> {
    let bytes = export_bytes()?;
    std::fs::write(path, &bytes).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    Ok(items().len())
}

/// Merges an export archive into the library: items with the same id are
/// replaced, folders and favorites are added. Returns the number of items.
pub fn import_bytes(bytes: &[u8]) -> Result<usize, String> {
    let a = archive::import_library(bytes).map_err(|e| e.0)?;
    for (path, data) in &a.models {
        let m = Model3d::from_bytes(data).map_err(|e| e.0)?;
        store_model(path, &m)?;
    }
    let mut all = items();
    for it in &a.items {
        all.retain(|x| x.id != it.id);
        all.push(it.clone());
    }
    images::set_user_items(all)?;
    edit_meta(|m| {
        for f in &a.meta.folders {
            let _ = m.create_folder(f);
        }
        for id in &a.meta.favorites {
            if !m.is_favorite(id) {
                m.favorites.push(id.clone());
            }
        }
    });
    Ok(a.items.len())
}

/// Imports an export archive file.
pub fn import_library_from(path: &Path) -> Result<usize, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    import_bytes(&bytes)
}

// ----- placing -----

/// The default layer of placed copies of `item`.
pub fn layer_of(item: &CatalogItem) -> String {
    plan_library::rules::default_layer(item)
}

/// Makes sure the plan has a layer called `name`.
pub fn ensure_layer(cx: &mut EditorContext, name: &str) {
    if cx.project.layers.get(name).is_none() {
        cx.project
            .layers
            .add(plan_core::layers::Layer::new(name, [90, 90, 90], 18));
    }
}

/// Places the real object a payload item stands for (a cabinet, CAD items or
/// text) where the placement tool put `sym`. `None` for ordinary items.
pub fn place_payload(
    cx: &mut EditorContext,
    item: &CatalogItem,
    sym: &PlacedSymbol,
) -> Option<ObjectRef> {
    let payload = item.payload.as_ref()?;
    let fl = cx.floor;
    match item.kind {
        ItemKind::Cabinet => {
            let mut cab: Cabinet = serde_json::from_value(payload.clone()).ok()?;
            let u = Point::new(sym.angle.to_radians().cos(), sym.angle.to_radians().sin());
            cab.angle = sym.angle.to_radians();
            cab.position = sym.position - u * (cab.width * 0.5);
            cab.elevation = sym.elevation;
            let id = placed::add_cabinet(&mut cx.project, fl, cab)?;
            Some(ObjectRef::Cabinet(id))
        }
        ItemKind::CadBlock | ItemKind::Text => {
            let items: Vec<CadItem> = serde_json::from_value(payload.clone()).ok()?;
            let layer = layer_of(item);
            ensure_layer(cx, &layer);
            let center = symbol_center(sym);
            let angle = sym.angle.to_radians();
            let mut first: Option<Id> = None;
            for i in &items {
                let id = cx
                    .project
                    .add_cad(fl, layer.clone(), make::placed_cad(i, angle, center));
                first.get_or_insert(id);
            }
            let id = first?;
            Some(if item.kind == ItemKind::Text {
                ObjectRef::Text(id)
            } else {
                ObjectRef::Cad(id)
            })
        }
        _ => None,
    }
}

/// 3D meshes of a placed symbol whose library item has a user model, fitted
/// to the symbol's size and moved to its place (scene axes). `None` when the
/// item has no model. The item's `model_rotation` and `model_origin` apply.
pub fn placed_meshes(sym: &PlacedSymbol, floor_elevation: f64) -> Option<Vec<plan_3d::Mesh>> {
    let item = crate::tools::library::find_item(&sym.catalog_id)?;
    let model = model_of(&item)?;
    let model = if item.model_rotation != 0.0 {
        model.rotated_y(item.model_rotation)
    } else {
        (*model).clone()
    };
    let meshes: Vec<plan_3d::Mesh> = model
        .parts
        .iter()
        .map(|p| {
            plan_3d::import::mesh_from_triangles(
                &p.positions,
                None,
                &p.indices,
                plan_calib::mesh3d::material_for(p.color),
                None,
            )
        })
        .filter(|m| !m.indices.is_empty())
        .collect();
    if meshes.is_empty() {
        return None;
    }
    let fitted =
        plan_3d::import::fit_meshes_to_box(&meshes, sym.width as f32, sym.depth as f32, sym.height as f32);
    // The model origin is an offset in the symbol's own axes (x right, plan
    // y forward, up), turned with the symbol.
    let a = sym.angle.to_radians();
    let (ox, oy, oz) = (item.model_origin[0], item.model_origin[1], item.model_origin[2]);
    let dx = ox * a.cos() - oz * a.sin();
    let dy = ox * a.sin() + oz * a.cos();
    let origin = [
        (sym.position.x + dx) as f32,
        (floor_elevation + sym.elevation + oy) as f32,
        (-(sym.position.y + dy)) as f32,
    ];
    let yaw = (sym.angle + 180.0).to_radians() as f32;
    Some(
        fitted
            .iter()
            .map(|m| plan_3d::import::transform_mesh(m, origin, yaw, [1.0; 3], sym.flip))
            .collect(),
    )
}

// ----- Replace From Library for cabinets and devices -----

/// Replace From Library for a selected cabinet or electrical device: the
/// active library item takes its place.
///
/// * A cabinet is replaced by an active **cabinet item**: same position,
///   angle and elevation, the item's size and construction, and the old
///   cabinet's inserts (sink and cooktop cutouts, appliance, moldings and
///   label) are kept. Other items are refused.
/// * A device becomes the active library **symbol** at its position, facing
///   the way the device faced, on the device's height.
pub fn replace_other(cx: &mut EditorContext, sel: ObjectRef) -> bool {
    let Some(active) = crate::tools::library::active_item() else {
        cx.status = "Pick an item in the Library Browser, then choose Replace From Library".into();
        return false;
    };
    let Some(item) = crate::tools::library::find_item(&active) else {
        cx.status = format!("Unknown library item: {active}");
        return false;
    };
    match sel {
        ObjectRef::Cabinet(id) => replace_cabinet(cx, id, &item),
        ObjectRef::Device(id) => replace_device(cx, id, &item),
        _ => false,
    }
}

fn replace_cabinet(cx: &mut EditorContext, id: Id, item: &CatalogItem) -> bool {
    let (Some(payload), ItemKind::Cabinet) = (item.payload.as_ref(), item.kind) else {
        cx.status = "Pick a cabinet from the User Catalog (Add to Library saves one)".into();
        return false;
    };
    let Ok(mut new_cab) = serde_json::from_value::<Cabinet>(payload.clone()) else {
        cx.status = "That library cabinet is damaged".into();
        return false;
    };
    let Some(old) = placed::cabinet_by_id(cx.floor(), id) else {
        return false;
    };
    new_cab.id = old.id;
    new_cab.position = old.position;
    new_cab.angle = old.angle;
    new_cab.elevation = old.elevation;
    // Inserts stay with the cabinet; cutouts that no longer fit are dropped.
    new_cab.appliance = old.appliance.clone();
    new_cab.moldings = old.moldings.clone();
    new_cab.cutouts = old.cutouts.clone();
    if !old.label.trim().is_empty() {
        new_cab.label = old.label.clone();
    }
    let kept = new_cab.cutouts.len();
    cx.begin_change("Replace From Library");
    let fl = cx.floor;
    if placed::replace_cabinet(&mut cx.project, fl, &new_cab) {
        cx.mark_dirty();
        cx.status = format!("Replaced the cabinet with {} (kept {kept} insert(s))", item.name);
        true
    } else {
        cx.cancel_change();
        false
    }
}

fn replace_device(cx: &mut EditorContext, id: Id, item: &CatalogItem) -> bool {
    if item.payload.is_some() {
        cx.status = "Pick a plain library symbol to replace a device".into();
        return false;
    }
    let Some(dev) = crate::editor::site_view::load_electrical(cx.floor())
        .device(id)
        .cloned()
    else {
        return false;
    };
    let mut sym = PlacedSymbol::new(item.id.clone(), dev.position, item.width, item.depth, item.height);
    sym.angle = (dev.angle.to_degrees() - 90.0).rem_euclid(360.0);
    sym.elevation = dev.height;
    sym.layer = layer_of(item);
    if item.placement != Placement::WallMounted {
        // Free symbols hang their origin on the center.
        let v = Point::new(-sym.angle.to_radians().sin(), sym.angle.to_radians().cos());
        sym.position = dev.position - v * (item.depth * 0.5);
    }
    ensure_layer(cx, &sym.layer.clone());
    crate::editor::site_view::edit_electrical(cx, "Replace From Library", |layer, _| {
        layer.remove(id);
    });
    let fl = cx.floor;
    let new_id = cx.project.add_symbol(fl, sym);
    cx.selection.set(ObjectRef::Symbol(new_id));
    cx.mark_dirty();
    cx.status = format!("Replaced the device with {}", item.name);
    true
}

// ----- commands -----

/// Runs a Library menu command by id; false when the id is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        ADD_SELECTION => {
            cx.status = match add_selection(cx, None) {
                Ok(names) => format!("Added to the User Catalog: {}", names.join(", ")),
                Err(e) => e,
            };
        }
        ADD_MATERIAL => {
            cx.status = match crate::tools::materials::active_material() {
                None => "Pick a material in the Material Painter first".into(),
                Some(name) => {
                    let lib = crate::tools::materials::library();
                    match lib.find(&name) {
                        Some(def) => match add_material(&def.name, def.color, None) {
                            Ok(i) => format!("Added to the User Catalog: {}", i.name),
                            Err(e) => e,
                        },
                        None => format!("Unknown material: {name}"),
                    }
                }
            };
        }
        IMPORT_MODEL => match rfd::FileDialog::new()
            .set_title("Import 3D Model")
            .add_filter("3D models", &["obj", "gltf", "glb"])
            .pick_file()
        {
            Some(path) => {
                push_request(UiRequest::ImportModel(path));
                cx.status = "Choose the units and up axis, then Import".into();
            }
            None => cx.status = "Import cancelled".into(),
        },
        EXPORT_LIBRARY => {
            cx.status = match rfd::FileDialog::new()
                .set_title("Export Library")
                .set_file_name("plan-studio-library.calibz")
                .add_filter("Plan Studio library (Chief Architect cannot open it)", &["calibz"])
                .save_file()
            {
                Some(path) => match export_library_to(&path) {
                    Ok(n) => format!(
                        "Exported {n} item(s) to {} (Plan Studio only; Chief cannot read it)",
                        path.display()
                    ),
                    Err(e) => e,
                },
                None => "Export cancelled".into(),
            };
        }
        IMPORT_LIBRARY => {
            cx.status = match rfd::FileDialog::new()
                .set_title("Import Library")
                .add_filter("Plan Studio library", &["calibz", "calib", "zip"])
                .pick_file()
            {
                Some(path) => match import_library_from(&path) {
                    Ok(n) => format!("Imported {n} item(s) into the User Catalog"),
                    Err(e) => e,
                },
                None => "Import cancelled".into(),
            };
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_library::{Stroke, Symbol2d};

    use super::tests_support::fresh;

    fn folder(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| s.to_string()).collect()
    }

    fn box_item(name: &str, f: &[&str]) -> (CatalogItem, Model3d) {
        let m = Model3d::box_model(30.0, 20.0, 40.0, Some([200, 30, 30]));
        let id = new_id(ItemKind::Model);
        (make::item_from_model(&m, &id, name, &folder(f), None).unwrap(), m)
    }

    #[test]
    fn crud_round_trips_through_the_files() {
        let dir = fresh(true).unwrap();
        let (item, model) = box_item("Ottoman", &["User", "Furniture"]);
        let id = item.id.clone();
        let added = add(item, Some(&model)).unwrap();
        assert_eq!(added.model3d.as_deref(), Some(model_file_name(&id).as_str()));
        assert!(dir.join(added.model3d.as_ref().unwrap()).exists());

        // A fresh load (as after a restart) sees it, model included.
        images::set_user_library_path(Some(Some(dir.join("user-library.json"))));
        forget_cache();
        let back = self::item(&id).expect("persisted");
        assert_eq!((back.name.as_str(), back.kind), ("Ottoman", ItemKind::Model));
        assert_eq!(*model_of(&back).unwrap(), model);

        rename(&id, "Pouf").unwrap();
        assert_eq!(self::item(&id).unwrap().name, "Pouf");
        let copy = duplicate(&id).unwrap();
        assert_eq!(self::item(&copy).unwrap().name, "Pouf copy");
        move_to(&copy, &folder(&["User", "Other"])).unwrap();
        assert_eq!(self::item(&copy).unwrap().category, folder(&["User", "Other"]));
        // The copy shares the model file, so deleting the original keeps it.
        assert!(delete(&id).unwrap());
        assert!(self::item(&id).is_none());
        assert!(model_of(&self::item(&copy).unwrap()).is_some());
        assert!(dir.join(model_file_name(&id)).exists());
        assert!(delete(&copy).unwrap());
        assert!(!dir.join(model_file_name(&id)).exists(), "orphaned model removed");
        assert!(!delete(&copy).unwrap());
        images::set_user_library_path(None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn folders_favorites_and_recents_persist() {
        let dir = fresh(true).unwrap();
        create_folder(&folder(&["User", "Kitchen"])).unwrap();
        let (item, model) = box_item("Island", &["User", "Kitchen"]);
        let id = item.id.clone();
        add(item, Some(&model)).unwrap();
        assert!(toggle_favorite(&id));
        touch_recent(&id);
        touch_recent("core.something");
        // Restart.
        images::set_user_library_path(Some(Some(dir.join("user-library.json"))));
        forget_cache();
        let m = meta();
        assert!(m.is_favorite(&id));
        assert_eq!(m.recent, ["core.something".to_string(), id.clone()]);
        assert!(folders().contains(&folder(&["User", "Kitchen"])));
        assert_eq!(rename_folder(&folder(&["User", "Kitchen"]), "Cooking").unwrap(), 1);
        assert_eq!(self::item(&id).unwrap().category, folder(&["User", "Cooking"]));
        assert!(folders().contains(&folder(&["User", "Cooking"])));
        assert_eq!(delete_folder(&folder(&["User", "Cooking"])).unwrap(), 1);
        assert!(self::item(&id).is_none());
        assert!(!meta().is_favorite(&id));
        assert!(!dir.join(model_file_name(&id)).exists());
        images::set_user_library_path(None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_exported_library_reimports_identical_into_an_empty_one() {
        fresh(false);
        let (a, ma) = box_item("Chair", &["User", "Furniture"]);
        let aid = a.id.clone();
        add(a, Some(&ma)).unwrap();
        let (b, mb) = box_item("Table", &["User", "Furniture", "Dining"]);
        let bid = b.id.clone();
        add(b, Some(&mb)).unwrap();
        let (mat, mm) = make::item_from_material("Slate", [1, 2, 3], &new_id(ItemKind::Material), &folder(&["User", "Materials"]));
        let mid = mat.id.clone();
        add(mat, Some(&mm)).unwrap();
        create_folder(&folder(&["User", "Empty"])).unwrap();
        toggle_favorite(&aid);
        let before = items();
        let zip = export_bytes().unwrap();

        fresh(false);
        assert!(items().is_empty());
        assert_eq!(import_bytes(&zip).unwrap(), 3);
        let mut after = items();
        let mut want = before;
        after.sort_by(|x, y| x.id.cmp(&y.id));
        want.sort_by(|x, y| x.id.cmp(&y.id));
        assert_eq!(after, want, "items come back identical");
        for id in [&aid, &bid, &mid] {
            let it = self::item(id).unwrap();
            assert!(model_of(&it).is_some(), "{id} has its model");
        }
        assert_eq!(*model_of(&self::item(&aid).unwrap()).unwrap(), ma);
        assert!(meta().is_favorite(&aid));
        assert!(folders().contains(&folder(&["User", "Empty"])));
        // Importing twice replaces, not duplicates.
        assert_eq!(import_bytes(&zip).unwrap(), 3);
        assert_eq!(items().len(), 3);
        // Foreign files are refused with a reason.
        assert!(import_bytes(b"SQLite format 3\0.....................").unwrap_err().contains("Chief"));
        images::set_user_library_path(None);
    }

    #[test]
    fn importing_obj_and_gltf_files_makes_model_items_with_symbols() {
        let dir = fresh(true).unwrap();
        let obj = dir.join("crate.obj");
        std::fs::write(
            &obj,
            "o Crate\nv 0 0 0\nv 24 0 0\nv 24 0 18\nv 0 0 18\nv 0 12 0\nv 24 12 0\nv 24 12 18\nv 0 12 18\n\
             f 1 2 3 4\nf 5 8 7 6\nf 1 5 6 2\nf 2 6 7 3\nf 3 7 8 4\nf 4 8 5 1\n",
        )
        .unwrap();
        let mut settings = import_defaults(&obj);
        assert_eq!(settings.name, "crate");
        settings.options.unit_scale = 2.0;
        let item = import_model_file(&obj, &settings).unwrap();
        assert_eq!(item.kind, ItemKind::Model);
        // Y up, scaled x2: 48 wide, 36 deep, 24 high.
        assert_eq!((item.width, item.depth, item.height), (48.0, 36.0, 24.0));
        assert!(!item.symbol.is_empty());
        let model = model_of(&item).unwrap();
        assert_eq!(model.triangle_count(), 12);
        assert_eq!(item.category, default_folder(ItemKind::Model));

        // Z up: the 18-long z becomes the height, the 12-long y the depth.
        let mut z_up = settings.clone();
        z_up.options.up_axis = UpAxis::Z;
        z_up.options.unit_scale = 1.0;
        z_up.name = "Crate Z".into();
        let item2 = import_model_file(&obj, &z_up).unwrap();
        assert_eq!((item2.width, item2.depth, item2.height), (24.0, 12.0, 18.0));

        // glTF in meters: a 1 m cube is 39.37 inches.
        let gltf = dir.join("cube.gltf");
        let cube = crate::tools::library::user::tests_support::cube_gltf();
        std::fs::write(&gltf, cube).unwrap();
        let g = import_model_file(&gltf, &import_defaults(&gltf)).unwrap();
        assert!((g.width - 39.370_08).abs() < 0.01, "{}", g.width);
        assert!(!g.symbol.is_empty());

        assert!(import_model_file(&dir.join("missing.obj"), &settings).is_err());
        std::fs::write(dir.join("bad.obj"), "hello").unwrap();
        assert!(import_model_file(&dir.join("bad.obj"), &settings).is_err());
        images::set_user_library_path(None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn placed_meshes_follow_the_item_model_rotation_and_origin() {
        fresh(false);
        let (mut item, model) = box_item("Long", &["User", "X"]);
        item.width = 60.0;
        item.depth = 20.0;
        let m = Model3d::box_model(60.0, 20.0, 40.0, None);
        let id = item.id.clone();
        add(item, Some(&m)).unwrap();
        let _ = model;
        let sym = PlacedSymbol::new(id.clone(), Point::new(100.0, 200.0), 60.0, 20.0, 40.0);
        let meshes = placed_meshes(&sym, 0.0).unwrap();
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for me in &meshes {
            let (l, h) = me.bounds().unwrap();
            for k in 0..3 {
                lo[k] = lo[k].min(l[k]);
                hi[k] = hi[k].max(h[k]);
            }
        }
        // x spans 100 +- 30; scene z = -plan y, depth 20 in front (-z).
        assert!((lo[0] - 70.0).abs() < 1e-3 && (hi[0] - 130.0).abs() < 1e-3);
        assert!((hi[1] - 40.0).abs() < 1e-3);
        assert!((hi[2] + 200.0).abs() < 1e-3 && (lo[2] + 220.0).abs() < 1e-3, "{lo:?} {hi:?}");

        let mut item = self::item(&id).unwrap().as_ref().clone();
        item.model_origin = [10.0, 5.0, 0.0];
        update(item.clone()).unwrap();
        let moved = placed_meshes(&sym, 0.0).unwrap();
        let (l2, _) = moved[0].bounds().unwrap();
        assert!((l2[0] - 80.0).abs() < 1e-3, "origin offset in x: {l2:?}");
        item.model_rotation = 90.0;
        update(item).unwrap();
        assert!(placed_meshes(&sym, 0.0).is_some());
        // An item without a model has no meshes.
        let plain = PlacedSymbol::new("core.nothing", Point::ZERO, 1.0, 1.0, 1.0);
        assert!(placed_meshes(&plain, 0.0).is_none());
        images::set_user_library_path(None);
    }

    fn cx_with_wall() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            plan_core::WallKind::Interior,
        );
        cx
    }

    #[test]
    fn add_to_library_from_symbols_cabinets_cad_and_text() {
        fresh(false);
        let mut cx = cx_with_wall();
        // A placed built-in symbol (resized), a cabinet, two CAD shapes and a note.
        let src = crate::tools::library::library_catalog()
            .all_items()
            .find(|i| i.placement == Placement::FreeStanding && !i.symbol.is_empty())
            .unwrap();
        let mut sym = PlacedSymbol::new(src.id.clone(), Point::new(100.0, 100.0), src.width * 2.0, src.depth * 2.0, src.height);
        sym.label = "Big Thing".into();
        let sid = cx.project.add_symbol(0, sym);
        let mut cab = Cabinet::base(30.0);
        cab.position = Point::new(10.0, 10.0);
        let cid = placed::add_cabinet(&mut cx.project, 0, cab).unwrap();
        let l1 = cx.project.add_cad(0, "CAD, Default", CadItem::Line { a: Point::new(0.0, 0.0), b: Point::new(10.0, 0.0) });
        let l2 = cx.project.add_cad(0, "CAD, Default", CadItem::Circle { center: Point::new(5.0, 5.0), radius: 3.0 });
        let t = cx.project.add_cad(0, "Text", CadItem::Text { pos: Point::new(0.0, 0.0), text: "Hello there".into(), height: 3.0, angle: 0.0 });
        cx.selection.items = vec![
            ObjectRef::Symbol(sid),
            ObjectRef::Cabinet(cid),
            ObjectRef::Cad(l1),
            ObjectRef::Cad(l2),
            ObjectRef::Text(t),
        ];
        let names = add_selection(&cx, None).unwrap();
        assert_eq!(names.len(), 4, "{names:?}");
        assert!(names.contains(&"Big Thing".to_string()));
        let all = items();
        let sym_item = all.iter().find(|i| i.name == "Big Thing").unwrap();
        assert_eq!((sym_item.width, sym_item.depth), (src.width * 2.0, src.depth * 2.0));
        assert_eq!(sym_item.category, default_folder(ItemKind::Symbol));
        // The copy's drawing is scaled with the placed size.
        let want = src.symbol.bounds().unwrap().width() * 2.0;
        assert!((sym_item.symbol.bounds().unwrap().width() - want).abs() < 0.5);
        assert!(all.iter().any(|i| i.kind == ItemKind::Cabinet && i.model3d.is_some()));
        assert!(all.iter().any(|i| i.kind == ItemKind::CadBlock));
        assert!(all.iter().any(|i| i.kind == ItemKind::Text && i.name.starts_with("Hello")));

        // Nothing selected: a clear message.
        cx.selection.items.clear();
        assert!(add_selection(&cx, None).unwrap_err().contains("Select"));
        // A Chief object is refused.
        let chief = PlacedSymbol::new("chief.cafe-0001.5", Point::ZERO, 10.0, 10.0, 10.0);
        assert!(add_symbol(&chief, None).unwrap_err().contains("Chief"));
        images::set_user_library_path(None);
    }

    #[test]
    fn placing_payload_items_makes_real_cabinets_and_cad() {
        fresh(false);
        let mut cx = cx_with_wall();
        let mut cab = Cabinet::base(30.0);
        cab.cutouts = Vec::new();
        let item = add_cabinet(&cab, None).unwrap();
        let sym = {
            let mut s = PlacedSymbol::new(item.id.clone(), Point::new(100.0, 3.0), item.width, item.depth, item.height);
            s.angle = 0.0;
            s
        };
        let r = place_payload(&mut cx, &item, &sym);
        let Some(ObjectRef::Cabinet(id)) = r else {
            panic!("{r:?}")
        };
        let placed_cab = placed::cabinet_by_id(cx.floor(), id).unwrap();
        // Back-center at x = 100: the back-left corner is half a width left.
        assert!((placed_cab.position.x - 85.0).abs() < 1e-9 && (placed_cab.position.y - 3.0).abs() < 1e-9);

        let block = add_cad(
            &[CadItem::Line { a: Point::new(0.0, 0.0), b: Point::new(20.0, 0.0) }],
            "Bar",
            None,
        )
        .unwrap();
        let mut s2 = PlacedSymbol::new(block.id.clone(), Point::new(50.0, 40.0), block.width, block.depth, 0.0);
        s2.angle = 90.0;
        let r = place_payload(&mut cx, &block, &s2);
        assert!(matches!(r, Some(ObjectRef::Cad(_))));
        let line = cx.floor().cad.last().unwrap();
        match &line.item {
            CadItem::Line { a, b } => {
                // Turned a quarter turn: vertical, 20 long.
                assert!((a.x - b.x).abs() < 1e-9 && ((a.y - b.y).abs() - 20.0).abs() < 1e-9);
            }
            _ => unreachable!(),
        }
        assert_eq!(line.layer, "CAD, Default");
        // An ordinary item has no payload to place.
        let plain = box_item("P", &["User", "X"]).0;
        assert!(place_payload(&mut cx, &plain, &sym).is_none());
        images::set_user_library_path(None);
    }

    #[test]
    fn replace_from_library_swaps_cabinets_keeping_inserts_and_devices_become_symbols() {
        fresh(false);
        let mut cx = cx_with_wall();
        let mut tall = Cabinet::base(36.0);
        tall.height = 40.0;
        let item = add_cabinet(&tall, None).unwrap();
        let mut old = Cabinet::base(24.0);
        old.position = Point::new(30.0, 3.0);
        old.angle = 0.0;
        old.appliance = Some("Dishwasher".into());
        old.label = "DW".into();
        let id = placed::add_cabinet(&mut cx.project, 0, old).unwrap();
        cx.selection.set(ObjectRef::Cabinet(id));
        // Nothing active yet.
        crate::tools::library::clear_active_item();
        assert!(!replace_other(&mut cx, ObjectRef::Cabinet(id)));
        crate::tools::library::set_active_item(&mut cx, &item.id);
        assert!(replace_other(&mut cx, ObjectRef::Cabinet(id)));
        let got = placed::cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!((got.width, got.height), (36.0, 40.0), "the library cabinet's size");
        assert_eq!(got.position, Point::new(30.0, 3.0));
        assert_eq!(got.appliance.as_deref(), Some("Dishwasher"), "inserts stay");
        assert_eq!(got.label, "DW");
        assert!(cx.undo().is_some());
        assert_eq!(placed::cabinet_by_id(cx.floor(), id).unwrap().width, 24.0);

        // A plain symbol is refused for a cabinet.
        let plain = crate::tools::library::library_catalog().all_items().next().unwrap();
        crate::tools::library::set_active_item(&mut cx, &plain.id);
        assert!(!replace_other(&mut cx, ObjectRef::Cabinet(id)));
        assert!(cx.status.contains("cabinet"));

        // A device becomes the active symbol.
        let dev = plan_electrical::Device {
            id: 0,
            kind: plan_electrical::DeviceKind::Outlet110,
            position: Point::new(60.0, 3.0),
            angle: std::f64::consts::FRAC_PI_2,
            height: 16.0,
            wall_id: None,
            circuit: None,
            label: String::new(),
            switched_by: Vec::new(),
            finish: String::new(),
            hide_label: false,
        };
        let mut layer = crate::editor::site_view::load_electrical(cx.floor());
        let did = layer.add(dev);
        crate::editor::site_view::save_electrical(&mut cx.project, 0, &layer);
        crate::tools::library::set_active_item(&mut cx, &plain.id);
        assert!(replace_other(&mut cx, ObjectRef::Device(did)));
        assert!(crate::editor::site_view::load_electrical(cx.floor()).device(did).is_none());
        let s = cx.floor().symbols.last().unwrap();
        assert_eq!(s.catalog_id, plain.id);
        assert_eq!(s.elevation, 16.0);
        images::set_user_library_path(None);
    }

    #[test]
    fn symbol_stroke_helper_is_available_for_the_browser() {
        let _ = Symbol2d::new(vec![Stroke::Circle { center: Point::ZERO, radius: 1.0 }]);
    }
}

/// Fixtures shared with the Library Browser tests.
#[cfg(test)]
pub(crate) mod tests_support {
    use crate::tools::images;
    use std::path::PathBuf;

    /// Points the store at a fresh temporary folder (or at memory only) and
    /// forgets everything cached.
    pub(crate) fn fresh(on_disk: bool) -> Option<PathBuf> {
        let dir = on_disk.then(|| {
            let dir = std::env::temp_dir().join(format!(
                "plan-studio-user-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        });
        images::set_user_library_path(Some(dir.as_ref().map(|d| d.join("user-library.json"))));
        super::forget_cache();
        dir
    }

    /// A glTF text of a 1 m cube (12 triangles) with a data-URI buffer.
    pub(crate) fn cube_gltf() -> String {
        let v: [[f32; 3]; 8] = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let idx: [u16; 36] = [
            4, 5, 6, 4, 6, 7, 1, 0, 3, 1, 3, 2, 5, 1, 2, 5, 2, 6, 0, 4, 7, 0, 7, 3, 7, 6, 2, 7, 2,
            3, 0, 1, 5, 0, 5, 4,
        ];
        let mut bin = Vec::new();
        for p in v {
            for c in p {
                bin.extend_from_slice(&c.to_le_bytes());
            }
        }
        for i in idx {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        let b64 = {
            const T: &[u8; 64] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut s = String::new();
            for c in bin.chunks(3) {
                let n = (c[0] as u32) << 16
                    | (*c.get(1).unwrap_or(&0) as u32) << 8
                    | *c.get(2).unwrap_or(&0) as u32;
                s.push(T[(n >> 18) as usize & 63] as char);
                s.push(T[(n >> 12) as usize & 63] as char);
                s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
                s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
            }
            s
        };
        format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],
"nodes":[{{"mesh":0}}],"meshes":[{{"name":"cube","primitives":[{{"attributes":{{"POSITION":0}},"indices":1}}]}}],
"buffers":[{{"byteLength":{len},"uri":"data:application/octet-stream;base64,{b64}"}}],
"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":96}},{{"buffer":0,"byteOffset":96,"byteLength":72}}],
"accessors":[{{"bufferView":0,"componentType":5126,"count":8,"type":"VEC3"}},{{"bufferView":1,"componentType":5123,"count":36,"type":"SCALAR"}}]}}"#,
            len = bin.len()
        )
    }
}
