//! The User Catalog inside the Library Browser: folder menus, favorites and
//! recents, the filter bar, the preview pane (2D symbol or a software-shaded
//! 3D view with rotate buttons), Object Information, and the Import 3D Model
//! window.
//!
//! All state changes go through [`crate::tools::library::user`]; this module
//! only draws and turns clicks into [`UserAction`]s that [`UserUi::perform`]
//! carries out (so tests can drive them without a frame).

use super::{preview_shapes, trim_num, LibraryEvent};
use crate::tools::library::user::{self as store, ModelImport, UiRequest};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use plan_import::{ImportedModel, UpAxis};
use plan_library::browse::{Filter, SortKey};
use plan_library::manage::{UserMeta, USER_ROOT};
use plan_library::preview::render_preview;
use plan_library::{CatalogItem, ItemKind, Model3d, Placement};
use std::path::PathBuf;

/// Edge of the big preview, points.
const PANE_PX: f32 = 132.0;
/// Pixels rendered for the 3D preview (drawn at [`PANE_PX`]).
const RENDER_PX: usize = 192;
/// Default view of the 3D preview.
const DEFAULT_YAW: f32 = 30.0;
const DEFAULT_PITCH: f32 = 25.0;

/// What the tree and the list show besides a plain category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum View {
    /// The selected category (or search results).
    #[default]
    Category,
    /// The starred items.
    Favorites,
    /// The recently used items.
    Recent,
}

/// 2D symbol or 3D model in the preview pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PreviewMode {
    #[default]
    Plan,
    Model,
}

/// Something a menu, a drop or a button asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum UserAction {
    Select(String),
    ToggleFavorite(String),
    Duplicate(String),
    Delete(String),
    Rename(String),
    MoveTo(String, Vec<String>),
    ObjectInfo(String),
    /// Copy a (built-in) item into the User Catalog.
    AddCopy(String),
    NewFolder(Vec<String>),
    RenameFolder(Vec<String>),
    DeleteFolder(Vec<String>),
    MoveFolder(Vec<String>, Vec<String>),
}

/// What is dragged from a row onto a folder.
#[derive(Clone, Debug)]
pub struct DragItem(pub String);

/// A small dialog asking for a name or a yes.
#[derive(Clone, Debug, PartialEq)]
enum Prompt {
    NewFolder { parent: Vec<String>, name: String },
    RenameFolder { path: Vec<String>, name: String },
    RenameItem { id: String, name: String },
    DeleteItem { id: String, name: String },
    DeleteFolder { path: Vec<String>, count: usize },
}

/// The Object Information editor.
#[derive(Clone, Debug, PartialEq)]
struct InfoForm {
    original: CatalogItem,
    name: String,
    keywords: String,
    style: String,
    manufacturer: String,
    kind: ItemKind,
    folder: String,
    width: f64,
    depth: f64,
    height: f64,
    elevation: f64,
    placement: Placement,
    /// 0 = follow the placement, 1 = turn to walls, 2 = never.
    auto_rotate: u8,
    layer: String,
    symbol_choice: SymbolChoice,
    model_rotation: f64,
    origin: [f64; 3],
    error: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SymbolChoice {
    Keep,
    FromModel,
    Rectangle,
}

/// The Import 3D Model window.
struct ImportForm {
    path: PathBuf,
    settings: ModelImport,
    unit_index: usize,
    parsed: Result<ImportedModel, String>,
    /// Folder text (`User > 3D Models`).
    folder: String,
    placement_choice: Option<Placement>,
    error: String,
}

type TexKey = (String, i32, i32, usize, usize);

/// Everything the User Catalog part of the panel remembers.
pub struct UserUi {
    pub view: View,
    /// Type, catalog, style, size, favorites-only and sort. The query and the
    /// category come from the panel.
    pub filter: Filter,
    pub show_filters: bool,
    /// Thumbnail grid instead of the list.
    pub grid: bool,
    /// The item shown in the preview pane.
    pub selected: Option<String>,
    pub mode: PreviewMode,
    pub yaw: f32,
    pub pitch: f32,
    min_on: bool,
    max_on: bool,
    min_size: f64,
    max_size: f64,
    tex: Option<(TexKey, TextureHandle)>,
    import_tex: Option<(usize, TextureHandle)>,
    info: Option<InfoForm>,
    import: Option<ImportForm>,
    prompt: Option<Prompt>,
    /// Folders, favorites and recents as of this frame.
    pub meta: UserMeta,
    /// User folders as of this frame.
    pub folders: Vec<Vec<String>>,
}

impl Default for UserUi {
    fn default() -> Self {
        UserUi {
            view: View::Category,
            filter: Filter::default(),
            show_filters: false,
            grid: false,
            selected: None,
            mode: PreviewMode::Plan,
            yaw: DEFAULT_YAW,
            pitch: DEFAULT_PITCH,
            min_on: false,
            max_on: false,
            min_size: 12.0,
            max_size: 120.0,
            tex: None,
            import_tex: None,
            info: None,
            import: None,
            prompt: None,
            meta: UserMeta::default(),
            folders: Vec::new(),
        }
    }
}

// ----- object information -----

/// The Object Information lines of any library item.
pub fn info_lines(item: &CatalogItem) -> Vec<(&'static str, String)> {
    let mut v = vec![
        ("Type", item.kind.label().to_string()),
        ("Category", item.category_label()),
        (
            "Size",
            format!(
                "{} \u{00D7} {} \u{00D7} {} in",
                trim_num(item.width),
                trim_num(item.depth),
                trim_num(item.height)
            ),
        ),
        ("Placement", placement_name(item.placement).to_string()),
        ("Layer", plan_library::rules::default_layer(item)),
    ];
    if item.elevation != 0.0 {
        v.push(("Elevation", format!("{} in", trim_num(item.elevation))));
    }
    if let Some(s) = &item.style {
        v.push(("Style", s.clone()));
    }
    if let Some(m) = &item.manufacturer {
        v.push(("Manufacturer", m.clone()));
    }
    let tags: Vec<&str> = item
        .tags
        .iter()
        .filter(|t| !t.contains(':'))
        .map(String::as_str)
        .collect();
    if !tags.is_empty() {
        v.push(("Keywords", tags.join(", ")));
    }
    match store::model_of(item) {
        Some(m) => v.push((
            "3D model",
            format!("{} triangles, {} parts", m.triangle_count(), m.parts.len()),
        )),
        None => v.push(("3D model", "box from the size".to_string())),
    }
    v
}

fn placement_name(p: Placement) -> &'static str {
    match p {
        Placement::WallMounted => "Wall mounted",
        Placement::FreeStanding => "Free standing",
        Placement::Ceiling => "Ceiling",
        Placement::Countertop => "Countertop",
    }
}

fn folder_text(path: &[String]) -> String {
    path.join(" > ")
}

fn parse_folder(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .split('>')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if out.first().map(String::as_str) != Some(USER_ROOT) {
        out.insert(0, USER_ROOT.to_string());
    }
    out
}

impl InfoForm {
    fn new(item: &CatalogItem) -> Self {
        InfoForm {
            original: item.clone(),
            name: item.name.clone(),
            keywords: item
                .tags
                .iter()
                .filter(|t| !t.contains(':'))
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            style: item.style.clone().unwrap_or_default(),
            manufacturer: item.manufacturer.clone().unwrap_or_default(),
            kind: item.kind,
            folder: folder_text(&item.category),
            width: item.width,
            depth: item.depth,
            height: item.height,
            elevation: item.elevation,
            placement: item.placement,
            auto_rotate: match item.auto_rotate {
                None => 0,
                Some(true) => 1,
                Some(false) => 2,
            },
            layer: item.layer.clone().unwrap_or_default(),
            symbol_choice: SymbolChoice::Keep,
            model_rotation: item.model_rotation,
            origin: item.model_origin,
            error: String::new(),
        }
    }

    /// The edited item, or why it cannot be saved.
    fn build(&self) -> Result<(CatalogItem, Option<Model3d>), String> {
        let name = plan_library::manage::valid_name(&self.name)?;
        let folder = parse_folder(&self.folder);
        if folder.len() < 2 {
            return Err("Name a folder under User, such as User > Furniture".into());
        }
        for f in &folder[1..] {
            plan_library::manage::valid_name(f)?;
        }
        let mut it = self.original.clone();
        it.name = name;
        // Keywords replace the plain tags; tags with a colon (image:, material:)
        // are the item's own data and stay.
        let keep: Vec<String> = it.tags.iter().filter(|t| t.contains(':')).cloned().collect();
        it.tags = self
            .keywords
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        it.tags.extend(keep);
        it.style = (!self.style.trim().is_empty()).then(|| self.style.trim().to_string());
        it.manufacturer =
            (!self.manufacturer.trim().is_empty()).then(|| self.manufacturer.trim().to_string());
        it.kind = self.kind;
        it.category = folder;
        it.width = self.width.max(0.1);
        it.depth = self.depth.max(0.1);
        it.height = self.height.max(0.0);
        it.elevation = self.elevation.max(0.0);
        it.placement = self.placement;
        it.auto_rotate = match self.auto_rotate {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        };
        it.layer = (!self.layer.trim().is_empty()).then(|| self.layer.trim().to_string());
        it.model_rotation = self.model_rotation;
        it.model_origin = self.origin;
        let model = store::model_of(&self.original).map(|m| (*m).clone());
        match self.symbol_choice {
            SymbolChoice::Keep => {
                // A changed footprint rescales the drawing.
                let (ow, od) = (self.original.width, self.original.depth);
                if (ow - it.width).abs() > 1e-9 || (od - it.depth).abs() > 1e-9 {
                    let sx = if ow > 1e-9 { it.width / ow } else { 1.0 };
                    let sy = if od > 1e-9 { it.depth / od } else { 1.0 };
                    it.symbol = scale_symbol(&it.symbol, sx, sy);
                }
            }
            SymbolChoice::FromModel => {
                let m = model
                    .as_ref()
                    .ok_or("This item has no 3D model to draw the symbol from")?;
                let turned = if it.model_rotation != 0.0 {
                    m.rotated_y(it.model_rotation).normalized()
                } else {
                    m.clone()
                };
                it.symbol = crate::tools::library::make::silhouette_symbol(&turned, it.placement);
                if it.symbol.is_empty() {
                    return Err("The model has no plan outline".into());
                }
            }
            SymbolChoice::Rectangle => {
                it.symbol = rectangle_symbol(it.width, it.depth, it.placement);
            }
        }
        Ok((it, model))
    }
}

fn scale_symbol(s: &plan_library::Symbol2d, sx: f64, sy: f64) -> plan_library::Symbol2d {
    use plan_core::geometry::Point;
    use plan_library::Stroke as S;
    let p = |q: &Point| Point::new(q.x * sx, q.y * sy);
    let k = (sx * sy).abs().sqrt();
    plan_library::Symbol2d::new(
        s.strokes
            .iter()
            .map(|st| match st {
                S::Polyline { points, closed } => S::Polyline {
                    points: points.iter().map(p).collect(),
                    closed: *closed,
                },
                S::Circle { center, radius } => S::Circle {
                    center: p(center),
                    radius: radius * k,
                },
                S::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                } => S::Arc {
                    center: p(center),
                    radius: radius * k,
                    start_deg: *start_deg,
                    end_deg: *end_deg,
                },
            })
            .collect(),
    )
}

fn rectangle_symbol(w: f64, d: f64, placement: Placement) -> plan_library::Symbol2d {
    use plan_core::geometry::Point;
    let y0 = if placement == Placement::WallMounted { 0.0 } else { -d * 0.5 };
    plan_library::Symbol2d::new(vec![plan_library::Stroke::Polyline {
        points: vec![
            Point::new(-w * 0.5, y0),
            Point::new(w * 0.5, y0),
            Point::new(w * 0.5, y0 + d),
            Point::new(-w * 0.5, y0 + d),
        ],
        closed: true,
    }])
}

// ----- the import window's model -----

impl ImportForm {
    fn new(path: PathBuf) -> Self {
        let settings = store::import_defaults(&path);
        let unit_index = plan_import::model::UNITS
            .iter()
            .position(|(_, k)| (*k - settings.options.unit_scale).abs() < 1e-6)
            .unwrap_or(0);
        let mut f = ImportForm {
            folder: folder_text(&settings.folder),
            path,
            settings,
            unit_index,
            parsed: Err(String::new()),
            placement_choice: None,
            error: String::new(),
        };
        f.reparse();
        f
    }

    fn reparse(&mut self) {
        self.settings.options.unit_scale = plan_import::model::UNITS[self.unit_index].1;
        self.parsed = store::parse_model_file(&self.path, &self.settings.options);
    }

    fn do_import(&mut self) -> Result<String, String> {
        self.settings.folder = parse_folder(&self.folder);
        self.settings.placement = self.placement_choice;
        let item = store::import_model_file(&self.path, &self.settings)?;
        Ok(item.name.clone())
    }
}

// ----- the panel pieces -----

impl UserUi {
    /// Refreshes the per-frame copies of the meta and folders.
    pub fn refresh(&mut self) {
        self.meta = store::meta();
        self.folders = self.meta.all_folders(&store::items());
    }

    /// The filter with the panel's query and category filled in.
    pub fn full_filter(&self, query: &str, category: &[String]) -> Filter {
        let mut f = self.filter.clone();
        f.query = query.to_string();
        f.category = category.to_vec();
        f.favorites_only = f.favorites_only || self.view == View::Favorites;
        if self.view == View::Recent {
            f.sort = SortKey::Recent;
        }
        f.min_size = self.min_on.then_some(self.min_size);
        f.max_size = self.max_on.then_some(self.max_size);
        f
    }

    /// True when a view, type, catalog, style, size or favorites filter is
    /// on (so the list shows without a query or category).
    pub fn narrows(&self) -> bool {
        self.view != View::Category
            || !self.filter.kinds.is_empty()
            || self.filter.catalog.is_some()
            || !self.filter.style.trim().is_empty()
            || self.filter.favorites_only
            || self.min_on
            || self.max_on
    }

    fn filter_count(&self) -> usize {
        usize::from(!self.filter.kinds.is_empty())
            + usize::from(self.filter.catalog.is_some())
            + usize::from(!self.filter.style.trim().is_empty())
            + usize::from(self.min_on || self.max_on)
            + usize::from(self.filter.favorites_only)
    }

    /// The Filters toggle, and the filter widgets when open.
    pub fn filter_bar(&mut self, ui: &mut egui::Ui, catalogs: &[String]) {
        let n = self.filter_count();
        let label = if n == 0 {
            "Filters".to_string()
        } else {
            format!("Filters ({n})")
        };
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.show_filters, label)
                .on_hover_text("Type, catalog, style and size filters")
                .clicked()
            {
                self.show_filters = !self.show_filters;
            }
            egui::ComboBox::from_id_salt("library_sort")
                .selected_text(format!("Sort: {}", self.filter.sort.label()))
                .show_ui(ui, |ui| {
                    for k in SortKey::ALL {
                        ui.selectable_value(&mut self.filter.sort, k, k.label());
                    }
                });
            ui.selectable_value(&mut self.grid, false, "List");
            ui.selectable_value(&mut self.grid, true, "Grid");
            if n > 0 && ui.small_button("Clear").clicked() {
                self.filter = Filter {
                    sort: self.filter.sort,
                    ..Filter::default()
                };
                self.min_on = false;
                self.max_on = false;
            }
        });
        if !self.show_filters {
            return;
        }
        ui.indent("library_filters", |ui| {
            ui.horizontal(|ui| {
                ui.label("Type");
                let text = match self.filter.kinds.as_slice() {
                    [] => "Any".to_string(),
                    [k] => k.label().to_string(),
                    ks => format!("{} types", ks.len()),
                };
                egui::ComboBox::from_id_salt("library_type")
                    .selected_text(text)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(self.filter.kinds.is_empty(), "Any").clicked() {
                            self.filter.kinds.clear();
                        }
                        for k in ItemKind::ALL {
                            let mut on = self.filter.kinds.contains(&k);
                            if ui.checkbox(&mut on, k.label()).changed() {
                                if on {
                                    self.filter.kinds.push(k);
                                } else {
                                    self.filter.kinds.retain(|x| *x != k);
                                }
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Catalog");
                egui::ComboBox::from_id_salt("library_catalog")
                    .selected_text(self.filter.catalog.clone().unwrap_or_else(|| "Any".into()))
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(self.filter.catalog.is_none(), "Any").clicked() {
                            self.filter.catalog = None;
                        }
                        for c in catalogs {
                            let on = self.filter.catalog.as_deref() == Some(c.as_str());
                            if ui.selectable_label(on, c).clicked() {
                                self.filter.catalog = Some(c.clone());
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Style");
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter.style)
                        .hint_text("modern, traditional, ...")
                        .desired_width(120.0),
                );
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.min_on, "Min");
                ui.add_enabled(
                    self.min_on,
                    egui::DragValue::new(&mut self.min_size).speed(1.0).range(0.0..=1000.0).suffix(" in"),
                );
                ui.checkbox(&mut self.max_on, "Max");
                ui.add_enabled(
                    self.max_on,
                    egui::DragValue::new(&mut self.max_size).speed(1.0).range(0.0..=1000.0).suffix(" in"),
                );
            });
            ui.checkbox(&mut self.filter.favorites_only, "Favorites only");
        });
    }

    /// The Favorites / Recently Used entries above the tree. Returns true
    /// when the view changed.
    pub fn quick_lists(&mut self, ui: &mut egui::Ui) -> bool {
        let before = self.view;
        let favs = self.meta.favorites.len();
        let recent = self.meta.recent.len();
        if ui
            .selectable_label(self.view == View::Favorites, format!("\u{2605} Favorites ({favs})"))
            .clicked()
        {
            self.view = if self.view == View::Favorites { View::Category } else { View::Favorites };
        }
        if ui
            .selectable_label(self.view == View::Recent, format!("Recently Used ({recent})"))
            .clicked()
        {
            self.view = if self.view == View::Recent { View::Category } else { View::Recent };
        }
        self.view != before
    }

    /// Menu and drop target of a User folder's label. Returns an action to
    /// carry out. Does nothing for paths outside the User catalog.
    pub fn folder_hooks(&self, resp: &egui::Response, path: &[String]) -> Option<UserAction> {
        if path.first().map(String::as_str) != Some(USER_ROOT) {
            return None;
        }
        let mut out = None;
        // A dragged item dropped on the folder moves there.
        if path.len() >= 2 {
            if resp.dnd_hover_payload::<DragItem>().is_some() {
                ui_highlight(resp);
            }
            if let Some(d) = resp.dnd_release_payload::<DragItem>() {
                out = Some(UserAction::MoveTo(d.0.clone(), path.to_vec()));
            }
        }
        resp.context_menu(|ui| {
            if ui.button("New Folder\u{2026}").clicked() {
                out = Some(UserAction::NewFolder(path.to_vec()));
                ui.close_menu();
            }
            if path.len() >= 2 {
                if ui.button("Rename Folder\u{2026}").clicked() {
                    out = Some(UserAction::RenameFolder(path.to_vec()));
                    ui.close_menu();
                }
                ui.menu_button("Move Folder To", |ui| {
                    let mut targets: Vec<Vec<String>> = vec![vec![USER_ROOT.to_string()]];
                    targets.extend(self.folders.iter().cloned());
                    for t in targets {
                        let inside = t.len() >= path.len() && t[..path.len()] == *path;
                        let same_parent = t[..] == path[..path.len() - 1];
                        if inside || same_parent {
                            continue;
                        }
                        if ui.button(folder_text(&t)).clicked() {
                            out = Some(UserAction::MoveFolder(path.to_vec(), t.clone()));
                            ui.close_menu();
                        }
                    }
                });
                if ui.button("Delete Folder\u{2026}").clicked() {
                    out = Some(UserAction::DeleteFolder(path.to_vec()));
                    ui.close_menu();
                }
            }
        });
        out
    }

    /// The context menu entries every result row has (user items get the
    /// management ones). Returns an action to carry out.
    pub fn row_menu(&self, ui: &mut egui::Ui, item: &CatalogItem) -> Option<UserAction> {
        let mut out = None;
        let mine = item.id.starts_with("user.");
        let fav = self.meta.is_favorite(&item.id);
        if ui
            .button(if fav { "Remove from Favorites" } else { "Add to Favorites" })
            .clicked()
        {
            out = Some(UserAction::ToggleFavorite(item.id.clone()));
            ui.close_menu();
        }
        if ui.button("Open Object").clicked() {
            out = Some(if mine {
                UserAction::ObjectInfo(item.id.clone())
            } else {
                UserAction::Select(item.id.clone())
            });
            ui.close_menu();
        }
        if mine {
            if ui.button("Rename\u{2026}").clicked() {
                out = Some(UserAction::Rename(item.id.clone()));
                ui.close_menu();
            }
            if ui.button("Duplicate").clicked() {
                out = Some(UserAction::Duplicate(item.id.clone()));
                ui.close_menu();
            }
            ui.menu_button("Move To", |ui| {
                for f in &self.folders {
                    if *f == item.category {
                        continue;
                    }
                    if ui.button(folder_text(f)).clicked() {
                        out = Some(UserAction::MoveTo(item.id.clone(), f.clone()));
                        ui.close_menu();
                    }
                }
            });
            if ui.button("Delete\u{2026}").clicked() {
                out = Some(UserAction::Delete(item.id.clone()));
                ui.close_menu();
            }
        } else if ui.button("Add to User Library").clicked() {
            out = Some(UserAction::AddCopy(item.id.clone()));
            ui.close_menu();
        }
        out
    }

    // ----- actions -----

    /// Carries out `action`; returns a status-bar message.
    pub fn perform(&mut self, action: UserAction) -> Option<String> {
        let msg = |r: Result<String, String>| Some(r.unwrap_or_else(|e| e));
        match action {
            UserAction::Select(id) => {
                self.selected = Some(id);
                None
            }
            UserAction::ToggleFavorite(id) => {
                let on = store::toggle_favorite(&id);
                self.refresh();
                Some(if on { "Added to Favorites" } else { "Removed from Favorites" }.to_string())
            }
            UserAction::Duplicate(id) => {
                let r = store::duplicate(&id).map(|new| {
                    self.selected = Some(new);
                    "Duplicated".to_string()
                });
                self.refresh();
                msg(r)
            }
            UserAction::Delete(id) => {
                let name = store::item(&id).map(|i| i.name.clone()).unwrap_or_default();
                self.prompt = Some(Prompt::DeleteItem { id, name });
                None
            }
            UserAction::Rename(id) => {
                let name = store::item(&id).map(|i| i.name.clone()).unwrap_or_default();
                self.prompt = Some(Prompt::RenameItem { id, name });
                None
            }
            UserAction::MoveTo(id, folder) => {
                let r = store::move_to(&id, &folder).map(|()| format!("Moved to {}", folder_text(&folder)));
                self.refresh();
                msg(r)
            }
            UserAction::ObjectInfo(id) => {
                match store::item(&id) {
                    Some(it) => {
                        self.selected = Some(id);
                        self.info = Some(InfoForm::new(&it));
                    }
                    None => return Some("That item is no longer in the User Catalog".into()),
                }
                None
            }
            UserAction::AddCopy(id) => {
                let Some(src) = crate::tools::library::find_item(&id) else {
                    return Some("Unknown library item".into());
                };
                let r = store::add_item_copy(&src, None).map(|i| {
                    self.selected = Some(i.id.clone());
                    format!("Added \"{}\" to the User Catalog", i.name)
                });
                self.refresh();
                msg(r)
            }
            UserAction::NewFolder(parent) => {
                self.prompt = Some(Prompt::NewFolder { parent, name: String::new() });
                None
            }
            UserAction::RenameFolder(path) => {
                let name = path.last().cloned().unwrap_or_default();
                self.prompt = Some(Prompt::RenameFolder { path, name });
                None
            }
            UserAction::DeleteFolder(path) => {
                let count = store::items()
                    .iter()
                    .filter(|i| i.category.len() >= path.len() && i.category[..path.len()] == path[..])
                    .count();
                self.prompt = Some(Prompt::DeleteFolder { path, count });
                None
            }
            UserAction::MoveFolder(path, to) => {
                let r = store::move_folder(&path, &to).map(|n| format!("Moved the folder ({n} item(s))"));
                self.refresh();
                msg(r)
            }
        }
    }

    /// Opens the Import 3D Model window for `path`.
    pub fn open_import(&mut self, path: PathBuf) {
        self.import = Some(ImportForm::new(path));
    }

    /// Opens Object Information for a user item.
    pub fn open_info(&mut self, id: &str) -> bool {
        match store::item(id) {
            Some(it) => {
                self.info = Some(InfoForm::new(&it));
                true
            }
            None => false,
        }
    }

    // ----- preview pane -----

    fn texture(&mut self, ctx: &egui::Context, item: &CatalogItem) -> Option<TextureHandle> {
        let (model, _real) = preview_model(item);
        let model = if item.model_rotation != 0.0 {
            model.rotated_y(item.model_rotation)
        } else {
            model
        };
        let key: TexKey = (
            item.id.clone(),
            self.yaw.round() as i32 + (item.model_rotation as i32) * 1000,
            self.pitch.round() as i32,
            model.triangle_count(),
            (item.width + item.depth + item.height) as usize,
        );
        if let Some((k, t)) = &self.tex {
            if *k == key {
                return Some(t.clone());
            }
        }
        let img = render_preview(&model, RENDER_PX, self.yaw, self.pitch);
        let color = egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.rgba);
        let tex = ctx.load_texture("library_preview", color, egui::TextureOptions::LINEAR);
        self.tex = Some((key, tex.clone()));
        Some(tex)
    }

    /// The preview pane of `item`: 2D symbol or 3D view with rotate buttons,
    /// then Object Information. Returns an action (edit, add a copy).
    pub fn preview_pane(&mut self, ui: &mut egui::Ui, item: &CatalogItem) -> Option<UserAction> {
        let mut out = None;
        ui.horizontal(|ui| {
            ui.strong(&item.name);
            if item.id.starts_with("user.") && ui.small_button("Edit\u{2026}").clicked() {
                out = Some(UserAction::ObjectInfo(item.id.clone()));
            } else if !item.id.starts_with("user.")
                && !crate::tools::library::chief::is_chief_id(&item.id)
                && ui.small_button("Add to User Library").clicked()
            {
                out = Some(UserAction::AddCopy(item.id.clone()));
            }
            let fav = self.meta.is_favorite(&item.id);
            if ui
                .small_button(if fav { "\u{2605}" } else { "\u{2606}" })
                .on_hover_text("Favorite")
                .clicked()
            {
                out = Some(UserAction::ToggleFavorite(item.id.clone()));
            }
        });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.mode, PreviewMode::Plan, "2D");
            ui.selectable_value(&mut self.mode, PreviewMode::Model, "3D");
            if self.mode == PreviewMode::Model {
                if ui.small_button("\u{25C0}").on_hover_text("Rotate left").clicked() {
                    self.yaw -= 30.0;
                }
                if ui.small_button("\u{25B6}").on_hover_text("Rotate right").clicked() {
                    self.yaw += 30.0;
                }
                if ui.small_button("\u{25B2}").on_hover_text("Tilt up").clicked() {
                    self.pitch = (self.pitch + 15.0).min(80.0);
                }
                if ui.small_button("\u{25BC}").on_hover_text("Tilt down").clicked() {
                    self.pitch = (self.pitch - 15.0).max(-10.0);
                }
                if ui.small_button("Reset").clicked() {
                    self.yaw = DEFAULT_YAW;
                    self.pitch = DEFAULT_PITCH;
                }
            }
        });
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(PANE_PX), Sense::drag());
        ui.painter().rect_filled(rect, 3.0, Color32::from_gray(0xEC));
        match self.mode {
            PreviewMode::Plan => {
                let ink = Stroke::new(1.25_f32, Color32::from_gray(0x2B));
                for shape in preview_shapes(&item.symbol, rect, ink) {
                    ui.painter().add(shape);
                }
            }
            PreviewMode::Model => {
                if resp.dragged() {
                    let d = resp.drag_delta();
                    self.yaw += d.x * 0.6;
                    self.pitch = (self.pitch + d.y * 0.4).clamp(-10.0, 80.0);
                }
                if let Some(tex) = self.texture(ui.ctx(), item) {
                    ui.painter().image(
                        tex.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                resp.on_hover_text("Drag to rotate");
            }
        }
        for (k, v) in info_lines(item) {
            ui.horizontal_wrapped(|ui| {
                ui.weak(format!("{k}:"));
                ui.label(v);
            });
        }
        out
    }

    // ----- windows -----

    /// Draws the open dialogs; returns a status message when something was
    /// done. Also picks up requests queued by the Library menu.
    pub fn windows(&mut self, ctx: &egui::Context) -> Option<LibraryEvent> {
        while let Some(req) = store::take_request() {
            match req {
                UiRequest::ImportModel(p) => self.open_import(p),
                UiRequest::ObjectInfo(id) => {
                    self.open_info(&id);
                }
            }
        }
        let mut message = None;
        self.prompt_window(ctx, &mut message);
        self.info_window(ctx, &mut message);
        self.import_window(ctx, &mut message);
        message.map(LibraryEvent::Message)
    }

    fn prompt_window(&mut self, ctx: &egui::Context, message: &mut Option<String>) {
        let Some(mut prompt) = self.prompt.clone() else {
            return;
        };
        let mut close = false;
        let mut run = false;
        let title = match &prompt {
            Prompt::NewFolder { .. } => "New Folder",
            Prompt::RenameFolder { .. } => "Rename Folder",
            Prompt::RenameItem { .. } => "Rename Library Item",
            Prompt::DeleteItem { .. } => "Delete Library Item",
            Prompt::DeleteFolder { .. } => "Delete Folder",
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                match &mut prompt {
                    Prompt::NewFolder { parent, name } => {
                        ui.label(format!("Inside {}", folder_text(parent)));
                        let r = ui.text_edit_singleline(name);
                        r.request_focus();
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            run = true;
                        }
                    }
                    Prompt::RenameFolder { name, .. } | Prompt::RenameItem { name, .. } => {
                        let r = ui.text_edit_singleline(name);
                        r.request_focus();
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            run = true;
                        }
                    }
                    Prompt::DeleteItem { name, .. } => {
                        ui.label(format!("Delete \"{name}\" from the User Catalog?"));
                    }
                    Prompt::DeleteFolder { path, count } => {
                        ui.label(format!(
                            "Delete {} and the {count} item(s) inside it?",
                            folder_text(path)
                        ));
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        run = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        if run {
            let r: Result<String, String> = match prompt.clone() {
                Prompt::NewFolder { mut parent, name } => plan_library::manage::valid_name(&name)
                    .and_then(|n| {
                        parent.push(n);
                        store::create_folder(&parent).map(|()| format!("Made folder {}", folder_text(&parent)))
                    }),
                Prompt::RenameFolder { path, name } => {
                    store::rename_folder(&path, &name).map(|n| format!("Renamed the folder ({n} item(s))"))
                }
                Prompt::RenameItem { id, name } => store::rename(&id, &name).map(|()| "Renamed".to_string()),
                Prompt::DeleteItem { id, .. } => {
                    if self.selected.as_deref() == Some(id.as_str()) {
                        self.selected = None;
                    }
                    store::delete(&id).map(|_| "Deleted".to_string())
                }
                Prompt::DeleteFolder { path, .. } => {
                    store::delete_folder(&path).map(|n| format!("Deleted the folder and {n} item(s)"))
                }
            };
            match r {
                Ok(m) => {
                    *message = Some(m);
                    self.prompt = None;
                }
                Err(e) => {
                    *message = Some(e);
                    self.prompt = None;
                }
            }
            self.refresh();
        } else if close {
            self.prompt = None;
        } else {
            self.prompt = Some(prompt);
        }
    }

    fn info_window(&mut self, ctx: &egui::Context, message: &mut Option<String>) {
        let Some(mut form) = self.info.take() else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        let folders = self.folders.clone();
        let has_model = store::has_model(&form.original);
        egui::Window::new("Object Information")
            .open(&mut open)
            .collapsible(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                egui::Grid::new("object_info_grid")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut form.name);
                        ui.end_row();
                        ui.label("Keywords");
                        ui.text_edit_singleline(&mut form.keywords)
                            .on_hover_text("Separated by commas; the browser search finds them");
                        ui.end_row();
                        ui.label("Category");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut form.folder);
                            ui.menu_button("\u{25BE}", |ui| {
                                for f in &folders {
                                    if ui.button(folder_text(f)).clicked() {
                                        form.folder = folder_text(f);
                                        ui.close_menu();
                                    }
                                }
                            });
                        });
                        ui.end_row();
                        ui.label("Type");
                        egui::ComboBox::from_id_salt("info_kind")
                            .selected_text(form.kind.label())
                            .show_ui(ui, |ui| {
                                for k in ItemKind::ALL {
                                    ui.selectable_value(&mut form.kind, k, k.label());
                                }
                            });
                        ui.end_row();
                        ui.label("Style");
                        ui.text_edit_singleline(&mut form.style);
                        ui.end_row();
                        ui.label("Manufacturer");
                        ui.text_edit_singleline(&mut form.manufacturer);
                        ui.end_row();
                        for (label, value, max) in [
                            ("Width", &mut form.width, 1000.0),
                            ("Depth", &mut form.depth, 1000.0),
                            ("Height", &mut form.height, 1000.0),
                            ("Elevation", &mut form.elevation, 400.0),
                        ] {
                            ui.label(label);
                            ui.add(egui::DragValue::new(value).speed(0.5).range(0.0..=max).suffix(" in"));
                            ui.end_row();
                        }
                        ui.label("Placement");
                        egui::ComboBox::from_id_salt("info_placement")
                            .selected_text(placement_name(form.placement))
                            .show_ui(ui, |ui| {
                                for p in [
                                    Placement::WallMounted,
                                    Placement::FreeStanding,
                                    Placement::Ceiling,
                                    Placement::Countertop,
                                ] {
                                    ui.selectable_value(&mut form.placement, p, placement_name(p));
                                }
                            });
                        ui.end_row();
                        ui.label("Turn to wall");
                        egui::ComboBox::from_id_salt("info_autorotate")
                            .selected_text(["Follow placement", "Always", "Never"][form.auto_rotate as usize])
                            .show_ui(ui, |ui| {
                                for (i, t) in ["Follow placement", "Always", "Never"].iter().enumerate() {
                                    ui.selectable_value(&mut form.auto_rotate, i as u8, *t);
                                }
                            });
                        ui.end_row();
                        ui.label("Default layer");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.layer)
                                .hint_text(plan_library::rules::default_layer(&form.original)),
                        );
                        ui.end_row();
                        ui.label("2D symbol");
                        egui::ComboBox::from_id_salt("info_symbol")
                            .selected_text(match form.symbol_choice {
                                SymbolChoice::Keep => "Keep the current drawing",
                                SymbolChoice::FromModel => "Draw from the 3D model",
                                SymbolChoice::Rectangle => "Plain rectangle",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut form.symbol_choice, SymbolChoice::Keep, "Keep the current drawing");
                                ui.add_enabled_ui(has_model, |ui| {
                                    ui.selectable_value(&mut form.symbol_choice, SymbolChoice::FromModel, "Draw from the 3D model");
                                });
                                ui.selectable_value(&mut form.symbol_choice, SymbolChoice::Rectangle, "Plain rectangle");
                            });
                        ui.end_row();
                        if has_model {
                            ui.label("3D rotation");
                            ui.add(egui::DragValue::new(&mut form.model_rotation).speed(1.0).suffix("\u{00B0}"));
                            ui.end_row();
                            ui.label("3D origin x / up / y");
                            ui.horizontal(|ui| {
                                for v in &mut form.origin {
                                    ui.add(egui::DragValue::new(v).speed(0.5));
                                }
                            });
                            ui.end_row();
                        }
                    });
                if !form.error.is_empty() {
                    ui.colored_label(Color32::from_rgb(0xB0, 0x30, 0x30), &form.error);
                }
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if apply {
            match form.build().and_then(|(item, _)| store::update(item).map(|i| i.name.clone())) {
                Ok(name) => {
                    *message = Some(format!("Saved \"{name}\""));
                    self.tex = None;
                    self.refresh();
                    return;
                }
                Err(e) => form.error = e,
            }
        }
        if open && !cancel {
            self.info = Some(form);
        }
    }

    fn import_window(&mut self, ctx: &egui::Context, message: &mut Option<String>) {
        let Some(mut form) = self.import.take() else {
            return;
        };
        let mut open = true;
        let mut go = false;
        let mut cancel = false;
        let mut changed = false;
        let folders = self.folders.clone();
        // The 3D preview of what was parsed.
        let preview = match &form.parsed {
            Ok(m) => {
                let model = crate::tools::library::make::model_from_import(m);
                let key = model.triangle_count() + (form.unit_index + 1) * 1_000_003;
                if self.import_tex.as_ref().map(|(k, _)| *k) != Some(key) {
                    let img = render_preview(&model, 160, DEFAULT_YAW, DEFAULT_PITCH);
                    let color = egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.rgba);
                    let tex = ctx.load_texture("library_import_preview", color, egui::TextureOptions::LINEAR);
                    self.import_tex = Some((key, tex));
                }
                self.import_tex.as_ref().map(|(_, t)| t.clone())
            }
            Err(_) => None,
        };
        egui::Window::new("Import 3D Model")
            .open(&mut open)
            .collapsible(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.weak(form.path.display().to_string());
                match &form.parsed {
                    Ok(m) => {
                        let e = m.extent().unwrap_or([0.0; 3]);
                        ui.label(format!(
                            "{} triangles in {} part(s); {} \u{00D7} {} \u{00D7} {} in (width \u{00D7} depth \u{00D7} height)",
                            m.triangle_count(),
                            m.parts.len(),
                            trim_num(e[0] as f64),
                            trim_num(e[2] as f64),
                            trim_num(e[1] as f64)
                        ));
                        if let Some(t) = &preview {
                            let (rect, _) = ui.allocate_exact_size(Vec2::splat(120.0), Sense::hover());
                            ui.painter().rect_filled(rect, 3.0, Color32::from_gray(0xEC));
                            ui.painter().image(
                                t.id(),
                                rect,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                    }
                    Err(e) => {
                        ui.colored_label(Color32::from_rgb(0xB0, 0x30, 0x30), e);
                    }
                }
                egui::Grid::new("import_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut form.settings.name);
                    ui.end_row();
                    ui.label("File units");
                    egui::ComboBox::from_id_salt("import_units")
                        .selected_text(plan_import::model::UNITS[form.unit_index].0)
                        .show_ui(ui, |ui| {
                            for (i, (n, _)) in plan_import::model::UNITS.iter().enumerate() {
                                if ui.selectable_value(&mut form.unit_index, i, *n).changed() {
                                    changed = true;
                                }
                            }
                        });
                    ui.end_row();
                    ui.label("Up axis");
                    egui::ComboBox::from_id_salt("import_up")
                        .selected_text(match form.settings.options.up_axis {
                            UpAxis::Y => "Y up",
                            UpAxis::Z => "Z up",
                        })
                        .show_ui(ui, |ui| {
                            for (a, t) in [(UpAxis::Y, "Y up"), (UpAxis::Z, "Z up")] {
                                if ui.selectable_value(&mut form.settings.options.up_axis, a, t).changed() {
                                    changed = true;
                                }
                            }
                        });
                    ui.end_row();
                    ui.label("Folder");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut form.folder);
                        ui.menu_button("\u{25BE}", |ui| {
                            for f in &folders {
                                if ui.button(folder_text(f)).clicked() {
                                    form.folder = folder_text(f);
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                    ui.end_row();
                    ui.label("Placement");
                    egui::ComboBox::from_id_salt("import_placement")
                        .selected_text(form.placement_choice.map_or("Automatic", placement_name))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.placement_choice, None, "Automatic");
                            for p in [
                                Placement::WallMounted,
                                Placement::FreeStanding,
                                Placement::Ceiling,
                                Placement::Countertop,
                            ] {
                                ui.selectable_value(&mut form.placement_choice, Some(p), placement_name(p));
                            }
                        });
                    ui.end_row();
                });
                ui.weak("The plan symbol is drawn from the model seen from above.");
                if !form.error.is_empty() {
                    ui.colored_label(Color32::from_rgb(0xB0, 0x30, 0x30), &form.error);
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(form.parsed.is_ok(), egui::Button::new("Import")).clicked() {
                        go = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if changed {
            form.reparse();
        }
        if go {
            match form.do_import() {
                Ok(name) => {
                    *message = Some(format!("Imported \"{name}\" into the User Catalog"));
                    self.refresh();
                    return;
                }
                Err(e) => form.error = e,
            }
        }
        if open && !cancel {
            self.import = Some(form);
        }
    }

    /// Test hook: the import form's parsed triangle count.
    #[cfg(test)]
    pub(crate) fn import_triangles(&self) -> Option<usize> {
        self.import.as_ref().and_then(|f| f.parsed.as_ref().ok()).map(ImportedModel::triangle_count)
    }
}

fn ui_highlight(resp: &egui::Response) {
    let stroke = Stroke::new(2.0_f32, resp.ctx.style().visuals.selection.stroke.color);
    resp.ctx
        .layer_painter(resp.layer_id)
        .rect_stroke(resp.rect, 2.0, stroke, egui::StrokeKind::Outside);
}

/// The model the 3D preview draws, and whether it is the item's own (not a
/// box standing in for it).
pub fn preview_model(item: &CatalogItem) -> (Model3d, bool) {
    match store::model_of(item) {
        Some(m) => ((*m).clone(), true),
        None => (
            Model3d::box_model(
                item.width.max(1.0) as f32,
                item.depth.max(1.0) as f32,
                item.height.max(1.0) as f32,
                None,
            ),
            false,
        ),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::library::user::tests_support::{cube_gltf, fresh};
    use plan_library::Model3d;

    fn folder(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| s.to_string()).collect()
    }

    fn add_box(name: &str, f: &[&str]) -> CatalogItem {
        let m = Model3d::box_model(30.0, 20.0, 40.0, Some([200, 30, 30]));
        let id = store::new_id(ItemKind::Model);
        let item = crate::tools::library::make::item_from_model(&m, &id, name, &folder(f), None).unwrap();
        (*store::add(item, Some(&m)).unwrap()).clone()
    }

    #[test]
    fn object_information_lists_the_model_or_the_box() {
        fresh(false);
        let it = add_box("Ottoman", &["User", "Furniture"]);
        let lines = info_lines(&it);
        let get = |k: &str| lines.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone());
        assert_eq!(get("Type").as_deref(), Some("3D Model"));
        assert_eq!(get("Category").as_deref(), Some("User > Furniture"));
        assert_eq!(get("Size").as_deref(), Some("30 \u{00D7} 20 \u{00D7} 40 in"));
        assert!(get("3D model").unwrap().starts_with("12 triangles"));
        let builtin = crate::tools::library::library_catalog().all_items().next().unwrap();
        assert!(info_lines(builtin)
            .iter()
            .any(|(k, v)| *k == "3D model" && v.contains("box")));
    }

    #[test]
    fn actions_manage_favorites_duplicates_moves_and_deletes() {
        fresh(false);
        let it = add_box("Ottoman", &["User", "Furniture"]);
        let mut ui = UserUi::default();
        ui.refresh();
        assert_eq!(ui.perform(UserAction::ToggleFavorite(it.id.clone())).as_deref(), Some("Added to Favorites"));
        assert!(ui.meta.is_favorite(&it.id));
        assert_eq!(ui.perform(UserAction::Duplicate(it.id.clone())).as_deref(), Some("Duplicated"));
        let copy = ui.selected.clone().unwrap();
        assert_ne!(copy, it.id);
        let msg = ui
            .perform(UserAction::MoveTo(copy.clone(), folder(&["User", "Elsewhere"])))
            .unwrap();
        assert!(msg.contains("User > Elsewhere"), "{msg}");
        assert!(ui.folders.contains(&folder(&["User", "Elsewhere"])));
        // Delete asks first, then the prompt runs.
        assert!(ui.perform(UserAction::Delete(copy.clone())).is_none());
        assert!(matches!(ui.prompt, Some(Prompt::DeleteItem { .. })));
        let ctx = egui::Context::default();
        let mut message = None;
        // The prompt window's OK is a click; run it directly.
        ui.prompt = Some(Prompt::DeleteItem { id: copy.clone(), name: "x".into() });
        let _ = ctx.run(egui::RawInput::default(), |ctx| ui.prompt_window(ctx, &mut message));
        // (No click happened, so the prompt is still up.)
        assert!(ui.prompt.is_some());
        assert!(store::item(&copy).is_some());
        // Same result through the store, as the OK button calls it.
        store::delete(&copy).unwrap();
        assert!(store::item(&copy).is_none());
        // Folder prompts.
        assert!(ui.perform(UserAction::NewFolder(folder(&["User"]))).is_none());
        assert!(matches!(ui.prompt, Some(Prompt::NewFolder { .. })));
        assert!(ui.perform(UserAction::RenameFolder(folder(&["User", "Furniture"]))).is_none());
        assert!(matches!(&ui.prompt, Some(Prompt::RenameFolder { name, .. }) if name == "Furniture"));
        ui.perform(UserAction::DeleteFolder(folder(&["User", "Furniture"])));
        assert!(matches!(ui.prompt, Some(Prompt::DeleteFolder { count: 1, .. })));
        // Adding a copy of a built-in item.
        let builtin = crate::tools::library::library_catalog().all_items().next().unwrap();
        let msg = ui.perform(UserAction::AddCopy(builtin.id.clone())).unwrap();
        assert!(msg.contains("User Catalog"), "{msg}");
        // Chief objects cannot be copied.
        crate::tools::library::chief::install_item(
            CatalogItem::new("chief.cafe-0009.9", "Licensed", Placement::FreeStanding, Default::default())
                .with_size(10.0, 10.0, 10.0),
            "Core",
        );
        let msg = ui.perform(UserAction::AddCopy("chief.cafe-0009.9".into())).unwrap();
        assert!(msg.contains("Chief"), "{msg}");
    }

    #[test]
    fn the_filter_collects_type_size_and_favorites_and_flags_narrowing() {
        let mut ui = UserUi::default();
        assert!(!ui.narrows());
        assert!(ui.full_filter("", &[]).is_empty());
        ui.filter.kinds = vec![ItemKind::Cabinet];
        assert!(ui.narrows());
        ui.filter.kinds.clear();
        ui.min_on = true;
        ui.min_size = 24.0;
        let f = ui.full_filter("sink", &folder(&["User"]));
        assert_eq!((f.min_size, f.max_size, f.query.as_str()), (Some(24.0), None, "sink"));
        ui.min_on = false;
        ui.view = View::Favorites;
        assert!(ui.full_filter("", &[]).favorites_only);
        ui.view = View::Recent;
        assert_eq!(ui.full_filter("", &[]).sort, SortKey::Recent);
    }

    #[test]
    fn object_information_edits_become_the_saved_item() {
        fresh(false);
        let it = add_box("Ottoman", &["User", "Furniture"]);
        let mut form = InfoForm::new(&it);
        form.name = "  Pouf ".into();
        form.keywords = "round, soft ,".into();
        form.style = "Modern".into();
        form.folder = "Seating > Poufs".into(); // "User" is added in front
        form.width = 60.0;
        form.depth = 40.0;
        form.auto_rotate = 1;
        form.layer = "Furniture".into();
        form.model_rotation = 90.0;
        form.origin = [1.0, 2.0, 3.0];
        form.symbol_choice = SymbolChoice::FromModel;
        let (built, model) = form.build().unwrap();
        assert_eq!(built.name, "Pouf");
        assert_eq!(built.category, folder(&["User", "Seating", "Poufs"]));
        assert_eq!(built.tags, vec!["round".to_string(), "soft".to_string()]);
        assert_eq!(built.style.as_deref(), Some("Modern"));
        assert_eq!((built.width, built.depth), (60.0, 40.0));
        assert_eq!(built.auto_rotate, Some(true));
        assert_eq!(built.layer.as_deref(), Some("Furniture"));
        assert_eq!((built.model_rotation, built.model_origin), (90.0, [1.0, 2.0, 3.0]));
        assert!(!built.symbol.is_empty());
        assert!(model.is_some());
        store::update(built.clone()).unwrap();
        assert_eq!(store::item(&it.id).unwrap().name, "Pouf");

        // Keeping the drawing rescales it with the footprint.
        let mut keep = InfoForm::new(&store::item(&it.id).unwrap());
        let w0 = keep.original.symbol.bounds().unwrap().width();
        keep.width *= 2.0;
        let (scaled, _) = keep.build().unwrap();
        let w1 = scaled.symbol.bounds().unwrap().width();
        assert!((w1 - 2.0 * w0).abs() < 1e-6, "{w0} -> {w1}");

        // A plain rectangle on request; bad input is reported.
        let mut rect = InfoForm::new(&it);
        rect.symbol_choice = SymbolChoice::Rectangle;
        let (r, _) = rect.build().unwrap();
        assert_eq!(r.symbol.strokes.len(), 1);
        let mut bad = InfoForm::new(&it);
        bad.name = "  ".into();
        assert!(bad.build().is_err());
        bad.name = "ok".into();
        bad.folder = "a/b".into();
        assert!(bad.build().is_err());
        // A plain symbol cannot be drawn from a model it lacks.
        let builtin = crate::tools::library::library_catalog().all_items().next().unwrap();
        let mut no_model = InfoForm::new(builtin);
        no_model.symbol_choice = SymbolChoice::FromModel;
        assert!(no_model.build().is_err());
    }

    #[test]
    fn the_import_window_parses_reparses_and_imports() {
        let dir = fresh(true).unwrap();
        let path = dir.join("cube.gltf");
        std::fs::write(&path, cube_gltf()).unwrap();
        let mut ui = UserUi::default();
        ui.refresh();
        store::push_request(UiRequest::ImportModel(path.clone()));
        let ctx = egui::Context::default();
        let mut message = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            message = ui.windows(ctx);
        });
        assert_eq!(ui.import_triangles(), Some(12));
        // glTF defaults to meters.
        let f = ui.import.as_mut().unwrap();
        assert_eq!(plan_import::model::UNITS[f.unit_index].0, "Meters");
        let e = f.parsed.as_ref().unwrap().extent().unwrap();
        assert!((e[0] - 39.370_08).abs() < 0.01);
        // Changing the unit re-reads the file.
        f.unit_index = 0;
        f.reparse();
        assert!((f.parsed.as_ref().unwrap().extent().unwrap()[0] - 1.0).abs() < 1e-4);
        f.settings.name = "Cube Stool".into();
        f.folder = "User > Stools".into();
        let done = f.do_import().unwrap();
        assert_eq!(done, "Cube Stool");
        let item = store::items().into_iter().find(|i| i.name == "Cube Stool").unwrap();
        assert_eq!(item.category, folder(&["User", "Stools"]));
        assert!((item.width - 1.0).abs() < 1e-3);
        assert!(message.is_none());
        let _ = std::fs::remove_dir_all(dir);
        crate::tools::images::set_user_library_path(None);
    }

    #[test]
    fn the_preview_pane_and_filter_bar_draw_in_every_mode() {
        fresh(false);
        let it = add_box("Ottoman", &["User", "Furniture"]);
        let builtin = crate::tools::library::library_catalog().all_items().next().unwrap().clone();
        let ctx = egui::Context::default();
        let mut ui_state = UserUi::default();
        ui_state.refresh();
        ui_state.show_filters = true;
        let mut got = Vec::new();
        for (item, mode) in [
            (&it, PreviewMode::Plan),
            (&it, PreviewMode::Model),
            (&builtin, PreviewMode::Model),
            (&builtin, PreviewMode::Plan),
        ] {
            ui_state.mode = mode;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui_state.filter_bar(ui, &["Core".to_string()]);
                    if let Some(a) = ui_state.preview_pane(ui, item) {
                        got.push(a);
                    }
                    assert!(!ui_state.quick_lists(ui) || ui_state.view != View::Category);
                });
            });
        }
        assert!(got.is_empty(), "no click, no action: {got:?}");
        // The 3D picture is cached per view and rebuilt when the view turns.
        ui_state.mode = PreviewMode::Model;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui_state.preview_pane(ui, &it);
            });
        });
        let before = ui_state.tex.as_ref().map(|(k, _)| k.clone()).unwrap();
        ui_state.yaw += 30.0;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui_state.preview_pane(ui, &it);
            });
        });
        let after = ui_state.tex.as_ref().map(|(k, _)| k.clone()).unwrap();
        assert_ne!(before, after);
        assert_eq!(before.0, after.0);
    }
}
