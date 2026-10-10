//! Library Object Specification (CB-57, CB-60) for a placed library symbol:
//! General (source catalog and object, size with Keep aspect, elevation,
//! position, angle, reflect), Options (library-specific choices: door style,
//! cabinet door, hardware), Materials, Label, Layer, Object Information and
//! Schedule. Sizes stretch the symbol's 3D mesh too. "Replace From Library"
//! swaps the symbol's catalog id for the active Library Browser item and
//! keeps its position and angle.
//!
//! The same dialog is the Library Browser's Open Object:
//! `SymbolDialog::for_library_item` edits a library object's own defaults
//! (size, elevation, layer, label, schedule, options) instead of a placed
//! copy; see `dialogs::library_object`.

// The shell opens this dialog; until it is wired the items are unused.
#![allow(dead_code)]

use super::images::{DistributionForm, ImageForm};
use super::{
    dis_combo, off, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_INK,
    PV_WALL,
};
use crate::editor::placed::{placed_symbol_strokes, stroke_polylines, symbol_placement};
use crate::tools::library::chief::{self, LICENSE_NOTE};
use crate::tools::library::{active_item, find_item};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use plan_core::geometry::Point;
use plan_core::images::DistKind;
use plan_core::PlacedSymbol;
use plan_library::{CatalogItem, LibType, ObjectDefaults, Placement};

mod import3d;
pub use import3d::{show_import, start_import};
// The scenario tests drive the dialog through these.
#[cfg(test)]
pub use import3d::{
    accept as accept_import, close as close_import, is_open as import_is_open, open_path,
    with_dialog as with_import, Side,
};

const TABS: &[Tab] = &[
    on("General"),
    on("Options"),
    off("3D"),
    on("Materials"),
    on("Layer"),
    on("Label"),
    off("Components"),
    on("Object Information"),
    on("Schedule"),
];

/// The names of the tabs, in order.
pub fn tab_names() -> Vec<&'static str> {
    TABS.iter().map(|t| t.name).collect()
}

pub struct SymbolDialog {
    frame: SpecDialog,
    form: SymbolForm,
    /// Pictures and distribution records have their own forms
    /// (`dialogs::images`); `form` is then an unused placeholder.
    special: Option<Special>,
}

enum Special {
    Image(ImageForm),
    Distribution(DistributionForm),
}

/// What the dialog edits when it is the Library Browser's Open Object.
struct LibraryMode {
    item_id: String,
    /// A User Catalog item: OK saves the changes into it. Built-in and Chief
    /// objects are read-only.
    editable: bool,
}

struct SymbolForm {
    draft: PlacedSymbol,
    name: String,
    category: String,
    placement: Placement,
    /// Where the item comes from: the Chief catalog's name, or the built-in
    /// library.
    source: String,
    /// The item is Chief Architect licensed content.
    chief: bool,
    /// Library size, for "Reset to Library Size".
    library_size: Option<(f64, f64, f64)>,
    layers: Vec<String>,
    fields: Fields,
    /// What the last "Replace From Library" did.
    note: String,
    /// Materials tab: the material the symbol was painted with (`None` =
    /// the symbol's own look) and whether the dialog changed it.
    material: Option<String>,
    material_changed: bool,
    material_search: String,
    /// General tab: resizing one dimension scales the other two with it.
    keep_aspect: bool,
    /// The browser type of the library item (decides the Options rows).
    lib_type: Option<LibType>,
    /// Object Information rows and the item's keywords.
    info: Vec<(&'static str, String)>,
    keywords: Vec<String>,
    /// Some when the dialog edits a library object, not a placed copy.
    library: Option<LibraryMode>,
    /// Open Object: how the item's 3D model is turned about the vertical
    /// axis, degrees (the General tab's Rotation row).
    model_rotation: f64,
}

impl SymbolForm {
    fn blank(draft: PlacedSymbol, layers: Vec<String>) -> SymbolForm {
        SymbolForm {
            name: String::new(),
            category: String::new(),
            placement: Placement::FreeStanding,
            source: String::new(),
            chief: false,
            library_size: None,
            layers,
            fields: Fields::default(),
            note: String::new(),
            material: None,
            material_changed: false,
            material_search: String::new(),
            keep_aspect: false,
            lib_type: None,
            info: Vec::new(),
            keywords: Vec::new(),
            library: None,
            model_rotation: 0.0,
            draft,
        }
    }
}

/// Schedules a placed object can be filed under by hand.
const SCHEDULE_CATEGORIES: &[&str] = &["Fixture", "Furniture", "Plant", "Appliance"];

/// Scales the other two sizes when one changed (Keep aspect). `old` is the
/// size before the edit, `changed` says which of width, depth, height was
/// typed into.
fn keep_aspect(d: &mut PlacedSymbol, old: (f64, f64, f64), changed: (bool, bool, bool)) {
    let ratio = match changed {
        (true, _, _) if old.0 > 0.0 => d.width / old.0,
        (_, true, _) if old.1 > 0.0 => d.depth / old.1,
        (_, _, true) if old.2 > 0.0 => d.height / old.2,
        _ => return,
    };
    if !ratio.is_finite() || ratio <= 0.0 {
        return;
    }
    if !changed.0 {
        d.width = old.0 * ratio;
    }
    if !changed.1 {
        d.depth = old.1 * ratio;
    }
    if !changed.2 {
        d.height = old.2 * ratio;
    }
}

/// The library-specific rows of the Options tab for a browser type: the
/// option key (stored in `PlacedSymbol::options`) and its label.
fn option_rows(t: Option<LibType>) -> &'static [(&'static str, &'static str)] {
    match t {
        Some(LibType::Doors) => &[("door_style", "Door style"), ("hardware", "Hardware")],
        Some(LibType::Cabinets) => &[("cabinet_door", "Cabinet door"), ("hardware", "Hardware")],
        Some(LibType::Windows) => &[("hardware", "Hardware")],
        _ => &[],
    }
}

/// Hardware finishes offered next to the library's hardware objects.
const FINISHES: &[&str] = &[
    "Brushed Nickel",
    "Polished Chrome",
    "Oil-Rubbed Bronze",
    "Matte Black",
    "Satin Brass",
];

/// The choices of an option key: door styles are the library's doors, cabinet
/// doors its Cabinet Doors styles, hardware its hardware objects and the
/// common finishes.
fn option_choices(key: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |n: String| {
        if !out.contains(&n) {
            out.push(n);
        }
    };
    match key {
        "door_style" => {
            for i in crate::tools::library::library_catalog().all_items() {
                if plan_library::types::classify(i) == Some(LibType::Doors) {
                    push(i.name.clone());
                }
            }
        }
        "cabinet_door" => {
            let styles = crate::tools::library::door_styles::available();
            for st in crate::tools::library::door_styles::doors(&styles) {
                push(st.name.clone());
            }
            // The three fronts every cabinet can be built with.
            for n in ["Slab", "Shaker", "Raised Panel"] {
                push(n.to_string());
            }
        }
        _ => {
            for i in crate::tools::library::library_catalog().all_items() {
                if plan_library::types::classify(i) == Some(LibType::Hardware) {
                    push(i.name.clone());
                }
            }
            for f in FINISHES {
                push((*f).to_string());
            }
        }
    }
    out
}

/// The Object Information rows of a library item.
pub fn info_rows(item: &plan_library::CatalogItem, source: &str) -> Vec<(&'static str, String)> {
    let trim = |v: f64| {
        if (v - v.round()).abs() < 0.05 {
            format!("{}", v.round() as i64)
        } else {
            format!("{v:.1}")
        }
    };
    let mut v = vec![
        ("Name", item.name.clone()),
        ("Type", item.kind.label().to_string()),
        (
            "Browser type",
            plan_library::types::classify(item)
                .map_or("-", LibType::label)
                .to_string(),
        ),
        ("Category", item.category_label()),
        ("Source", source.to_string()),
        (
            "Size",
            format!(
                "{} \u{00D7} {} \u{00D7} {} in",
                trim(item.width),
                trim(item.depth),
                trim(item.height)
            ),
        ),
        ("Placement", placement_name(item.placement).to_string()),
        ("Default layer", plan_library::rules::default_layer(item)),
    ];
    if item.elevation != 0.0 {
        v.push(("Elevation", format!("{} in", trim(item.elevation))));
    }
    if let Some(m) = &item.manufacturer {
        v.push(("Manufacturer", m.clone()));
    }
    if let Some(st) = &item.style {
        v.push(("Style", st.clone()));
    }
    v.push((
        "3D model",
        if item.model3d.is_some() || chief::is_chief_id(&item.id) {
            "Yes"
        } else {
            "Plan symbol only"
        }
        .to_string(),
    ));
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

impl SymbolDialog {
    /// A dialog for `symbol`; `layers` are the plan's layer names.
    pub fn new(symbol: PlacedSymbol, layers: Vec<String>) -> Self {
        if symbol.image.is_some() {
            let title = if symbol.image.as_ref().is_some_and(|i| i.billboard) {
                "Billboard Image Specification"
            } else {
                "Image Specification"
            };
            return Self::special(
                symbol.clone(),
                layers.clone(),
                Special::Image(ImageForm::new(symbol, layers)),
                title,
            );
        }
        if let Some(d) = &symbol.distribution {
            let title = match d.kind {
                DistKind::Path => "Distribution Path Specification",
                DistKind::Region => "Distribution Region Specification",
            };
            return Self::special(
                symbol.clone(),
                layers.clone(),
                Special::Distribution(DistributionForm::new(symbol, layers)),
                title,
            );
        }
        let mut form = SymbolForm::blank(symbol, layers);
        form.describe();
        Self {
            frame: SpecDialog::new("Library Object Specification", "symbol"),
            form,
            special: None,
        }
    }

    fn special(symbol: PlacedSymbol, layers: Vec<String>, special: Special, title: &str) -> Self {
        let form = SymbolForm::blank(symbol, layers);
        Self {
            frame: SpecDialog::new(title, "symbol_special"),
            form,
            special: Some(special),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        match &mut self.special {
            Some(Special::Image(f)) => self.frame.show(ctx, f),
            Some(Special::Distribution(f)) => self.frame.show(ctx, f),
            None => self.frame.show(ctx, &mut self.form),
        }
    }

    /// Open Object: the Library Object Specification of library item `id`.
    /// It edits the item's own defaults (size, elevation, layer, label,
    /// schedule, options and Reflect) on a stand-in placed copy. Only User
    /// Catalog items can be saved ([`library_update`](Self::library_update));
    /// built-in and Chief objects are read-only. `None` for an unknown id.
    pub fn for_library_item(id: &str, layers: Vec<String>) -> Option<Self> {
        let item = find_item(id)?;
        let mut proto = PlacedSymbol::new(
            item.id.clone(),
            Point::ZERO,
            item.width,
            item.depth,
            item.height,
        );
        proto.elevation = item.elevation;
        proto.layer = item
            .layer
            .clone()
            .unwrap_or_else(|| plan_library::rules::default_layer(&item));
        if let Some(d) = &item.defaults {
            proto.flip = d.flip;
            proto.label = d.label.clone();
            proto.schedule = d.schedule.clone();
            proto.options = d.options.clone();
        }
        let editable = id.starts_with("user.") && crate::tools::library::user::item(id).is_some();
        let mut form = SymbolForm::blank(proto, layers);
        form.model_rotation = item.model_rotation;
        form.library = Some(LibraryMode {
            item_id: id.to_string(),
            editable,
        });
        form.describe();
        Some(Self {
            frame: SpecDialog::new("Library Object Specification", "library_object"),
            form,
            special: None,
        })
    }

    /// Changes the draft the way typing into the fields would (tests).
    #[cfg(test)]
    pub fn edit_draft(&mut self, f: impl FnOnce(&mut PlacedSymbol)) {
        f(&mut self.form.draft);
    }

    /// Picks a material on the Materials tab the way the combo would (tests).
    #[cfg(test)]
    pub fn choose_material(&mut self, material: Option<String>) {
        self.form.material = material;
        self.form.material_changed = true;
    }

    /// The library item Open Object edits (`None` for a placed copy).
    pub fn library_item_id(&self) -> Option<&str> {
        self.form.library.as_ref().map(|l| l.item_id.as_str())
    }

    /// Open Object, after OK: the User Catalog item with the dialog's size,
    /// elevation, layer and defaults. The drawing is stretched to the new
    /// width and depth. `None` for a read-only item or a placed copy.
    pub fn library_update(&self) -> Option<CatalogItem> {
        let lib = self.form.library.as_ref().filter(|l| l.editable)?;
        let mut item = (*crate::tools::library::user::item(&lib.item_id)?).clone();
        let d = &self.form.draft;
        if item.width > 0.0 && item.depth > 0.0 && (d.width != item.width || d.depth != item.depth)
        {
            item.symbol = item
                .symbol
                .scaled_xy(d.width / item.width, d.depth / item.depth);
        }
        item.width = d.width;
        item.depth = d.depth;
        item.height = d.height;
        item.elevation = d.elevation;
        item.model_rotation = self.form.model_rotation;
        let mut probe = item.clone();
        probe.layer = None;
        item.layer =
            (d.layer != plan_library::rules::default_layer(&probe)).then(|| d.layer.clone());
        let defaults = ObjectDefaults {
            flip: d.flip,
            label: d.label.clone(),
            schedule: d.schedule.clone(),
            options: d.options.clone(),
        };
        item.defaults = (!defaults.is_default()).then_some(defaults);
        Some(item)
    }

    /// Starts the Materials tab on the material the symbol is painted with.
    pub fn with_material(mut self, current: Option<String>) -> Self {
        self.form.material = current;
        self
    }

    /// The Materials tab's choice when it was changed: `Some(None)` puts the
    /// symbol's own look back, `Some(Some(name))` paints it with a library
    /// material (apply it with `tools::materials::apply_symbol_material`).
    pub fn material_choice(&self) -> Option<Option<String>> {
        self.form
            .material_changed
            .then(|| self.form.material.clone())
    }

    pub fn draft(&self) -> &PlacedSymbol {
        match &self.special {
            Some(Special::Image(f)) => &f.draft,
            Some(Special::Distribution(f)) => &f.draft,
            None => &self.form.draft,
        }
    }
}

impl SymbolForm {
    /// Fills name, category, source, library size, type and Object
    /// Information from the draft's catalog id.
    fn describe(&mut self) {
        let id = self.draft.catalog_id.clone();
        let item = find_item(&id);
        self.name = item.as_ref().map_or_else(|| id.clone(), |i| i.name.clone());
        self.category = item
            .as_ref()
            .map_or_else(String::new, |i| i.category.join(" > "));
        self.placement = symbol_placement(&self.draft);
        self.library_size = item.as_ref().map(|i| (i.width, i.depth, i.height));
        self.chief = chief::is_chief_id(&id);
        self.source = match chief::installed(&id) {
            Some(c) => c.catalog_name,
            None if self.chief => "Chief Architect catalog (not loaded)".into(),
            None if id.starts_with("user.") => "User Catalog".into(),
            None => "Plan Studio library".into(),
        };
        self.lib_type = item.as_ref().and_then(|i| plan_library::types::classify(i));
        self.info = item
            .as_ref()
            .map_or_else(Vec::new, |i| info_rows(i, &self.source));
        self.keywords = item.as_ref().map_or_else(Vec::new, |i| {
            i.tags
                .iter()
                .filter(|t| {
                    !t.starts_with("image:") && !t.starts_with(plan_library::resolve::GUID_PREFIX)
                })
                .cloned()
                .collect()
        });
    }

    /// Replace From Library: the active Library Browser item takes over the
    /// symbol's catalog id; position, angle and size stay.
    fn replace_from_library(&mut self) {
        self.note = match active_item() {
            None => "Pick an item in the Library Browser first".into(),
            Some(id) if id == self.draft.catalog_id => "Already that library item".into(),
            Some(id) => {
                self.draft.catalog_id = id;
                self.describe();
                format!("Replaced with {}", self.name)
            }
        };
    }

    /// Add to Library: saves this symbol (as sized and flipped here) in the
    /// User Catalog. With `convert` the placed symbol also switches to the
    /// new user item, so it no longer depends on its source catalog.
    fn add_to_library(&mut self, convert: bool) {
        self.note = match crate::tools::library::user::add_symbol(&self.draft, None) {
            Ok(item) => {
                if convert {
                    self.draft.catalog_id = item.id.clone();
                    self.describe();
                    format!("Converted to the user symbol \"{}\"", item.name)
                } else {
                    format!("Added to the User Catalog as \"{}\"", item.name)
                }
            }
            Err(e) => e,
        };
    }

    fn general(&mut self, ui: &mut Ui) {
        let mut replace = false;
        let mut add_lib = false;
        let mut convert = false;
        let in_library = self.library.is_some();
        let f = &mut self.fields;
        let d = &mut self.draft;
        section(ui, "Symbol");
        row(ui, "Name", |ui| ui.label(&self.name));
        row(ui, "Source catalog", |ui| ui.label(&self.source));
        row(ui, "Category", |ui| ui.label(&self.category));
        row(ui, "Placement", |ui| {
            ui.label(placement_name(self.placement))
        });
        if self.chief {
            ui.weak(LICENSE_NOTE);
        }
        if in_library {
            if !self.library.as_ref().is_some_and(|l| l.editable) {
                row(ui, "User Catalog", |ui| {
                    if !self.chief && ui.button("Add to Library").clicked() {
                        add_lib = true;
                    }
                });
            }
        } else {
            row(ui, "Library", |ui| {
                if ui.button("Replace From Library").clicked() {
                    replace = true;
                }
            });
            row(ui, "User Catalog", |ui| {
                if ui.button("Add to Library").clicked() {
                    add_lib = true;
                }
                if ui
                    .button("Convert to Symbol")
                    .on_hover_text("Save as a user symbol and use that item")
                    .clicked()
                {
                    convert = true;
                }
            });
        }
        if !self.note.is_empty() {
            ui.weak(&self.note);
        }
        section(ui, "Size");
        let (w0, dp0, h0) = (d.width, d.depth, d.height);
        let cw = f.length_row(ui, "Width", "width", &mut d.width);
        let cd = f.length_row(ui, "Depth", "depth", &mut d.depth);
        let ch = f.length_row(ui, "Height", "height", &mut d.height);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.checkbox(&mut self.keep_aspect, "Keep aspect")
                .on_hover_text("Changing one size scales the other two with it");
        });
        if self.keep_aspect {
            keep_aspect(d, (w0, dp0, h0), (cw, cd, ch));
        }
        if let Some((w, dp, h)) = self.library_size {
            if ui.button("Reset to Library Size").clicked() {
                d.width = w;
                d.depth = dp;
                d.height = h;
            }
        }
        section(ui, if in_library { "Defaults" } else { "Position" });
        f.length_row(ui, "Elevation (from floor)", "elev", &mut d.elevation);
        super::elevation_ref::row(ui, "Elevation Reference");
        if in_library {
            f.degrees_row(
                ui,
                "Rotation (3D model)",
                "deg_model_rot",
                &mut self.model_rotation,
            );
        } else {
            f.length_row(ui, "Position X (back center)", "pos_x", &mut d.position.x);
            f.length_row(ui, "Position Y (back center)", "pos_y", &mut d.position.y);
            f.degrees_row(ui, "Angle", "deg_angle", &mut d.angle);
        }
        ui.checkbox(&mut d.flip, "Reflect (mirror left to right)");
        if replace {
            self.replace_from_library();
        }
        if add_lib || convert {
            self.add_to_library(convert);
        }
    }

    /// The Materials tab: the library material the symbol is painted with
    /// (the Material Painter's whole-object paint), with search.
    fn materials(&mut self, ui: &mut Ui) {
        let lib = crate::tools::materials::library();
        section(ui, "Materials");
        ui.weak("A symbol has no named parts: the material paints the whole object.");
        row(ui, "Material", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.material_search)
                    .hint_text("search")
                    .desired_width(140.0),
            );
        });
        let before = self.material.clone();
        row(ui, "Painted with", |ui| {
            egui::ComboBox::from_id_salt("sym_material")
                .selected_text(self.material.clone().unwrap_or_else(|| "Default".into()))
                .width(200.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(self.material.is_none(), "Default (the symbol's own)")
                        .clicked()
                    {
                        self.material = None;
                    }
                    for m in lib.search(&self.material_search) {
                        if ui
                            .selectable_label(self.material.as_deref() == Some(&m.name), &m.name)
                            .clicked()
                        {
                            self.material = Some(m.name.clone());
                        }
                    }
                });
        });
        if self.material != before {
            self.material_changed = true;
        }
        if let Some(d) = self.material.as_deref().and_then(|n| lib.resolve(n)) {
            row(ui, "Color", |ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(40.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    2.0,
                    egui::Color32::from_rgb(d.color[0], d.color[1], d.color[2]),
                );
            });
            ui.weak(format!("Class {}", d.class.name()));
        }
    }

    fn options(&mut self, ui: &mut Ui) {
        section(ui, "Options");
        ui.checkbox(&mut self.draft.flip, "Reflect (mirror left to right)");
        let rows = option_rows(self.lib_type);
        if rows.is_empty() {
            ui.weak("This library object has no other options.");
            return;
        }
        section(ui, "Library choices");
        for (key, label) in rows {
            let choices = option_choices(key);
            let current = self.draft.options.get(*key).cloned();
            let mut pick = current.clone();
            row(ui, label, |ui| {
                egui::ComboBox::from_id_salt(("sym_option", *key))
                    .selected_text(current.clone().unwrap_or_else(|| "Default".into()))
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(current.is_none(), "Default").clicked() {
                            pick = None;
                        }
                        for c in &choices {
                            if ui
                                .selectable_label(current.as_deref() == Some(c.as_str()), c)
                                .clicked()
                            {
                                pick = Some(c.clone());
                            }
                        }
                    });
            });
            if pick != current {
                match pick {
                    Some(v) => {
                        self.draft.options.insert((*key).to_string(), v);
                    }
                    None => {
                        self.draft.options.remove(*key);
                    }
                }
            }
        }
    }

    /// The Object Information tab: what the library item is.
    fn object_information(&mut self, ui: &mut Ui) {
        section(ui, "Object Information");
        if self.info.is_empty() {
            ui.weak("The library item is not available, so there is nothing to list.");
            return;
        }
        for (k, v) in &self.info {
            row(ui, k, |ui| ui.label(v));
        }
        if !self.keywords.is_empty() {
            row(ui, "Keywords", |ui| ui.label(self.keywords.join(", ")));
        }
        if self.chief {
            ui.weak(LICENSE_NOTE);
        }
    }

    /// The Schedule tab: whether and how the object appears in schedules.
    fn schedule(&mut self, ui: &mut Ui) {
        section(ui, "Schedule");
        let mut sch = self.draft.schedule.clone().unwrap_or_default();
        let before = sch.clone();
        let mut include = !sch.exclude;
        ui.checkbox(&mut include, "Include in schedules");
        sch.exclude = !include;
        ui.add_enabled_ui(include, |ui| {
            row(ui, "Schedule", |ui| {
                let shown = if sch.category.is_empty() {
                    "By library category".to_string()
                } else {
                    sch.category.clone()
                };
                egui::ComboBox::from_id_salt("sym_schedule_cat")
                    .selected_text(shown)
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(sch.category.is_empty(), "By library category")
                            .clicked()
                        {
                            sch.category.clear();
                        }
                        for c in SCHEDULE_CATEGORIES {
                            if ui.selectable_label(sch.category == *c, *c).clicked() {
                                sch.category = (*c).to_string();
                            }
                        }
                    });
            });
            row(ui, "Mark", |ui| ui.text_edit_singleline(&mut sch.mark));
            row(ui, "Manufacturer", |ui| {
                ui.text_edit_singleline(&mut sch.manufacturer)
            });
            row(ui, "Model", |ui| ui.text_edit_singleline(&mut sch.model));
            row(ui, "Note", |ui| ui.text_edit_singleline(&mut sch.note));
        });
        if sch != before {
            self.draft.schedule = (!sch.is_default()).then_some(sch);
        }
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        let d = &mut self.draft;
        row(ui, "Layer", |ui| {
            if self.layers.is_empty() {
                dis_combo(ui, "sym_layer", &d.layer);
                return;
            }
            super::select_layer::layer_field(
                ui,
                "sym_layer",
                &mut d.layer,
                self.layers.iter().map(String::as_str),
            );
        });
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Label");
        row(ui, "Label", |ui| {
            ui.text_edit_singleline(&mut self.draft.label);
        });
    }
}

impl SpecPages for SymbolForm {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter valid lengths".into());
        }
        if self.library.as_ref().is_some_and(|l| !l.editable) {
            return Some(
                "This library object is read-only: use Add to Library to edit a copy".into(),
            );
        }
        let d = &self.draft;
        (d.width < 1.0 || d.depth < 1.0 || d.height <= 0.0)
            .then(|| "Width, depth and height must be positive".to_string())
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TABS.get(tab).map(|t| t.name) {
            Some("General") => self.general(ui),
            Some("Options") => self.options(ui),
            Some("Materials") => self.materials(ui),
            Some("Layer") => self.layer(ui),
            Some("Label") => self.label(ui),
            Some("Object Information") => self.object_information(ui),
            Some("Schedule") => self.schedule(ui),
            _ => {}
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let mut s = self.draft.clone();
        s.position = Point::ZERO;
        s.angle = 0.0;
        let foot = s.footprint();
        let mut pts: Vec<Point> = foot.to_vec();
        let lines = placed_symbol_strokes(&s).map(|sym| stroke_polylines(&sym));
        if let Some(lines) = &lines {
            pts.extend(lines.iter().flat_map(|(v, _)| v.iter().copied()));
        }
        let (mut lo, mut hi) = (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        );
        for q in &pts {
            lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
            hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
        }
        let area = rect.shrink2(Vec2::new(4.0, 16.0));
        let (w, h) = ((hi.x - lo.x).max(1.0) as f32, (hi.y - lo.y).max(1.0) as f32);
        let scale = (area.width() / w).min(area.height() / h);
        let size = Vec2::new(w * scale, h * scale);
        let tl = area.center() - size * 0.5;
        let map = |q: Point| {
            Pos2::new(
                tl.x + (q.x - lo.x) as f32 * scale,
                tl.y + size.y - (q.y - lo.y) as f32 * scale,
            )
        };
        let ink = Stroke::new(1.0_f32, PV_INK);
        let outline: Vec<Pos2> = foot.iter().map(|q| map(*q)).collect();
        painter.add(egui::Shape::closed_line(
            outline,
            Stroke::new(0.8_f32, PV_WALL),
        ));
        painter.rect_stroke(
            Rect::from_min_size(tl, size),
            0.0,
            Stroke::new(0.5_f32, PV_WALL),
            StrokeKind::Outside,
        );
        match lines {
            Some(lines) => {
                for (v, closed) in lines {
                    let mut sp: Vec<Pos2> = v.iter().map(|q| map(*q)).collect();
                    if closed {
                        if let Some(first) = sp.first().copied() {
                            sp.push(first);
                        }
                    }
                    painter.add(egui::Shape::line(sp, ink));
                }
            }
            None => pv_text(
                painter,
                rect.center(),
                Align2::CENTER_CENTER,
                "No symbol drawing",
                10.0,
            ),
        }
        pv_text(
            painter,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            &self.name,
            10.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_edits_a_draft_and_validates() {
        let item = crate::tools::library::library_catalog()
            .all_items()
            .find(|i| i.placement == Placement::FreeStanding)
            .unwrap();
        let s = PlacedSymbol::new(
            item.id.clone(),
            Point::new(10.0, 20.0),
            item.width,
            item.depth,
            item.height,
        );
        let mut dlg = SymbolDialog::new(s, vec!["CAD, Default".into(), "Electrical".into()]);
        assert_eq!(dlg.form.name, item.name);
        assert!(dlg.form.error().is_none());
        dlg.form.draft.flip = true;
        dlg.form.draft.width = 0.0;
        assert!(dlg.form.error().is_some());
        dlg.form.draft.width = 30.0;
        assert!(dlg.draft().flip);

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = dlg.show(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                for tab in 0..TABS.len() {
                    dlg.form.page(ui, tab);
                }
                let (_, painter) =
                    ui.allocate_painter(Vec2::new(220.0, 300.0), egui::Sense::hover());
                dlg.form.preview(&painter, painter.clip_rect());
            });
        });
        // An unknown catalog id still opens.
        let unknown = SymbolDialog::new(
            PlacedSymbol::new("nope", Point::ZERO, 10.0, 10.0, 10.0),
            Vec::new(),
        );
        assert_eq!(unknown.form.name, "nope");
    }

    #[test]
    fn the_materials_tab_remembers_the_choice() {
        let s = PlacedSymbol::new("nope", Point::ZERO, 10.0, 10.0, 10.0);
        let mut dlg = SymbolDialog::new(s, Vec::new()).with_material(Some("Drywall".into()));
        assert_eq!(dlg.form.material.as_deref(), Some("Drywall"));
        assert_eq!(dlg.material_choice(), None, "not changed yet");
        let ctx = egui::Context::default();
        let tab = TABS.iter().position(|t| t.name == "Materials").unwrap();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| dlg.form.page(ui, tab));
        });
        assert_eq!(
            dlg.material_choice(),
            None,
            "drawing the tab changes nothing"
        );
        dlg.form.material = Some("Quartz – White".into());
        dlg.form.material_changed = true;
        assert_eq!(dlg.material_choice(), Some(Some("Quartz – White".into())));
        dlg.form.material = None;
        assert_eq!(dlg.material_choice(), Some(None));
    }

    #[test]
    fn chief_symbols_show_their_source_and_replace_from_the_library() {
        use crate::editor::EditorContext;
        use crate::plan_defaults;
        use crate::tools::library::{library_catalog, set_active_item};
        use plan_library::{CatalogItem, Symbol2d};

        let item = CatalogItem::new(
            "chief.cafe-0001.12",
            "Round Tank Toilet",
            Placement::FreeStanding,
            Symbol2d::default(),
        )
        .with_category(&["Interiors", "Bath"])
        .with_size(15.5, 28.0, 30.0);
        chief::install_item(item, "Core Interiors");
        let mut s = PlacedSymbol::new(
            "chief.cafe-0001.12",
            Point::new(10.0, 20.0),
            20.0,
            30.0,
            32.0,
        );
        s.angle = 90.0;
        let mut dlg = SymbolDialog::new(s, Vec::new());
        assert_eq!(dlg.form.name, "Round Tank Toilet");
        assert_eq!(dlg.form.source, "Core Interiors");
        assert!(dlg.form.chief);
        assert_eq!(dlg.form.library_size, Some((15.5, 28.0, 30.0)));

        // Nothing active yet on this thread's tool state: a hint, no change.
        crate::tools::library::clear_active_item();
        dlg.form.replace_from_library();
        assert_eq!(dlg.draft().catalog_id, "chief.cafe-0001.12");
        // Replace with a built-in item: id changes, position, angle and size stay.
        let other = library_catalog().all_items().next().unwrap().id.clone();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        assert!(set_active_item(&mut cx, &other));
        dlg.form.replace_from_library();
        let d = dlg.draft();
        assert_eq!(d.catalog_id, other);
        assert_eq!(
            (d.position, d.angle, d.width),
            (Point::new(10.0, 20.0), 90.0, 20.0)
        );
        assert_eq!(dlg.form.source, "Plan Studio library");
        assert!(!dlg.form.chief);
    }

    #[test]
    fn keep_aspect_scales_the_other_two_sizes() {
        let mut d = PlacedSymbol::new("x", Point::ZERO, 20.0, 10.0, 40.0);
        d.width = 40.0;
        keep_aspect(&mut d, (20.0, 10.0, 40.0), (true, false, false));
        assert_eq!((d.width, d.depth, d.height), (40.0, 20.0, 80.0));
        let mut d = PlacedSymbol::new("x", Point::ZERO, 20.0, 10.0, 40.0);
        d.height = 20.0;
        keep_aspect(&mut d, (20.0, 10.0, 40.0), (false, false, true));
        assert_eq!((d.width, d.depth, d.height), (10.0, 5.0, 20.0));
        // Nothing changed, or a zero old size: nothing happens.
        let mut d = PlacedSymbol::new("x", Point::ZERO, 20.0, 10.0, 40.0);
        keep_aspect(&mut d, (20.0, 10.0, 40.0), (false, false, false));
        assert_eq!((d.width, d.depth, d.height), (20.0, 10.0, 40.0));
        keep_aspect(&mut d, (0.0, 10.0, 40.0), (true, false, false));
        assert_eq!((d.width, d.depth, d.height), (20.0, 10.0, 40.0));
    }

    #[test]
    fn options_follow_the_object_type_and_are_stored_on_the_symbol() {
        assert!(option_rows(Some(LibType::Doors))
            .iter()
            .any(|r| r.0 == "door_style"));
        assert!(option_rows(Some(LibType::Cabinets))
            .iter()
            .any(|r| r.0 == "cabinet_door"));
        assert!(option_rows(Some(LibType::Furniture)).is_empty());
        assert!(option_rows(None).is_empty());
        assert!(option_choices("hardware").contains(&"Matte Black".to_string()));
        assert!(!option_choices("door_style").is_empty());
        assert!(!option_choices("cabinet_door").is_empty());

        // A built-in door opens with the door rows; the tab draws them.
        let door = crate::tools::library::library_catalog()
            .all_items()
            .find(|i| plan_library::types::classify(i) == Some(LibType::Doors));
        if let Some(door) = door {
            let s = PlacedSymbol::new(
                door.id.clone(),
                Point::ZERO,
                door.width,
                door.depth,
                door.height,
            );
            let mut dlg = SymbolDialog::new(s, Vec::new());
            assert_eq!(dlg.form.lib_type, Some(LibType::Doors));
            dlg.form
                .draft
                .options
                .insert("door_style".into(), "Shaker".into());
            let ctx = egui::Context::default();
            let tab = TABS.iter().position(|t| t.name == "Options").unwrap();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| dlg.form.page(ui, tab));
            });
            assert_eq!(
                dlg.draft().options.get("door_style").map(String::as_str),
                Some("Shaker"),
                "drawing the tab keeps the choice"
            );
        }
    }

    #[test]
    fn the_schedule_and_object_information_tabs_draw_and_edit_the_record() {
        let item = crate::tools::library::library_catalog()
            .all_items()
            .next()
            .unwrap();
        let s = PlacedSymbol::new(
            item.id.clone(),
            Point::ZERO,
            item.width,
            item.depth,
            item.height,
        );
        let mut dlg = SymbolDialog::new(s, Vec::new());
        assert!(dlg.form.info.iter().any(|(k, _)| *k == "Size"));
        assert!(dlg
            .form
            .info
            .iter()
            .any(|(k, v)| *k == "Name" && *v == item.name));
        assert!(dlg.draft().schedule.is_none());
        let ctx = egui::Context::default();
        for name in ["Object Information", "Schedule"] {
            let tab = TABS.iter().position(|t| t.name == name).unwrap();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| dlg.form.page(ui, tab));
            });
        }
        assert!(dlg.draft().schedule.is_none(), "drawing changes nothing");
        // A changed record is kept; the default record is dropped again.
        dlg.form.draft.schedule = Some(plan_core::SymbolSchedule {
            exclude: true,
            ..Default::default()
        });
        let tab = TABS.iter().position(|t| t.name == "Schedule").unwrap();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| dlg.form.page(ui, tab));
        });
        assert!(dlg.draft().schedule.as_ref().is_some_and(|s| s.exclude));
        // Open Object on a user item is editable, on a built-in one it is not.
        assert!(SymbolDialog::for_library_item(&item.id, Vec::new())
            .unwrap()
            .form
            .error()
            .is_some_and(|e| e.contains("read-only")));
    }

    #[test]
    fn open_object_saves_the_models_rotation_with_the_item() {
        use crate::tools::library::user::{self as store, tests_support};
        use plan_library::{CatalogItem, Symbol2d};
        tests_support::fresh(false);
        let item = CatalogItem::new(
            store::new_id(plan_library::ItemKind::Symbol),
            "Bench",
            Placement::FreeStanding,
            Symbol2d::default(),
        )
        .with_category(&["User", "Symbols"])
        .with_size(48.0, 16.0, 18.0);
        let item = store::add(item, None).unwrap();
        let mut dlg = SymbolDialog::for_library_item(&item.id, Vec::new()).unwrap();
        assert!(dlg.library_update().is_some(), "a user item is editable");
        dlg.form.model_rotation = 90.0;
        dlg.edit_draft(|d| d.elevation = 3.0);
        let up = dlg.library_update().unwrap();
        assert_eq!((up.model_rotation, up.elevation), (90.0, 3.0));
        // The Defaults section draws, with the rotation row.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| dlg.form.page(ui, 0));
        });
        assert_eq!(dlg.library_update().unwrap().model_rotation, 90.0);
    }
}
