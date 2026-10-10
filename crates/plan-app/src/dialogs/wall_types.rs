//! Wall Type Definitions (Build > Wall > Define Wall Types, and "Define..." in
//! a Wall Specification; manual pp. 414-421): the table of a wall type's
//! layers from the exterior face to the interior face in Exterior, Main and
//! Interior sections, with Name, Thickness, Extension, Role, Fill and
//! Material columns, a Main checkbox (several Main layers may sit together),
//! Insert/Delete/Move buttons, Edit Layer (the Wall Layer Specification), a
//! Wall Properties tab, Delete All Unused, Copy, Import, "Resize About" and a
//! preview that rotates or shows the plan view.
//!
//! [`LayerTable`] is the plain data model (and is unit tested); the dialog
//! edits a copy of the wall types and hands back the ones that changed.

use super::wall_layer::WallLayerDialog;
use super::{layer_stack, row, section, Outcome};
use crate::editor::EditorContext;
use eframe::egui::{self, Align2, Color32, Key, Modifiers, Pos2, RichText, Sense, Stroke, Vec2};
use plan_core::assemblies::LayerRole;
use plan_core::defaults::{WallLayer, WallTypeDef};
use plan_core::fill_styles::ColorSource;
use plan_core::wall_types::{self, ImportReport, LayerGroup, WallTypeProps, MIN_MAIN_LAYER};
use plan_core::{Project, ResizeAbout};
use std::collections::BTreeSet;

/// A row of the table is a layer of the type, whole, so what the table does
/// not show (the layer's Role options, its line and its framing) survives
/// an edit.
pub type LayerRow = WallLayer;

/// The layers of one wall type, exterior first, with the selected row and
/// the type's Wall Properties.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerTable {
    pub rows: Vec<LayerRow>,
    pub selected: usize,
    pub props: WallTypeProps,
}

impl LayerTable {
    pub fn from_def(def: &WallTypeDef) -> Self {
        let mut t = Self {
            rows: def.layers.clone(),
            selected: 0,
            props: def.props.clone(),
        };
        t.selected = t.main_index().unwrap_or(0);
        t
    }

    /// An empty table (no type).
    pub fn empty() -> Self {
        Self {
            rows: Vec::new(),
            selected: 0,
            props: WallTypeProps::default(),
        }
    }

    /// The definition the table stands for, `base` giving the name and kind.
    pub fn to_def(&self, base: &WallTypeDef) -> WallTypeDef {
        let mut def = base.clone();
        def.layers = self.rows.clone();
        def.props = self.props.clone();
        def.sync_derived();
        def
    }

    /// Writes the table back into `def` (name and kind are kept).
    pub fn apply_to(&self, def: &mut WallTypeDef) {
        *def = self.to_def(def);
    }

    /// Total thickness, inches.
    pub fn total(&self) -> f64 {
        self.rows.iter().map(|r| r.thickness).sum()
    }

    /// The first Main layer.
    pub fn main_index(&self) -> Option<usize> {
        self.rows.iter().position(|r| r.is_main)
    }

    pub fn main_count(&self) -> usize {
        self.rows.iter().filter(|r| r.is_main).count()
    }

    /// The section of the table row `i` is in.
    pub fn group_of(&self, i: usize) -> LayerGroup {
        self.to_def(&blank_def()).group_of(i)
    }

    fn new_row() -> LayerRow {
        WallLayer::new("New Layer", 0.5, false, "Generic")
    }

    /// Keeps the alignment layers (Build Platform, Dimension, Foundation)
    /// on the same layers when the list changes: `f` maps an old index to
    /// the new one, or `None` when the layer is gone.
    fn remap_align(&mut self, f: impl Fn(usize) -> Option<usize>) {
        self.props.align.each_index(|ix| {
            if let Some(i) = *ix {
                *ix = f(i);
            }
        });
    }

    /// Inserts a layer above (exterior side of) the selected one and selects it.
    pub fn insert_above(&mut self) {
        let at = self.selected.min(self.rows.len());
        self.rows.insert(at, Self::new_row());
        self.remap_align(|i| Some(if i >= at { i + 1 } else { i }));
        self.selected = at;
    }

    /// Inserts a layer below (interior side of) the selected one and selects it.
    pub fn insert_below(&mut self) {
        let at = if self.rows.is_empty() {
            0
        } else {
            self.selected.min(self.rows.len() - 1) + 1
        };
        self.rows.insert(at, Self::new_row());
        self.remap_align(|i| Some(if i >= at { i + 1 } else { i }));
        self.selected = at;
    }

    /// Deletes the selected layer; the last layer cannot be deleted. When the
    /// last main layer goes, the nearest remaining layer becomes the main layer.
    pub fn delete_selected(&mut self) -> bool {
        if self.rows.len() <= 1 || self.selected >= self.rows.len() {
            return false;
        }
        let at = self.selected;
        let removed = self.rows.remove(at);
        self.remap_align(|i| match i.cmp(&at) {
            std::cmp::Ordering::Less => Some(i),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(i - 1),
        });
        if removed.is_main && self.main_count() == 0 {
            let near = self.selected.min(self.rows.len() - 1);
            self.rows[near].is_main = true;
        }
        self.selected = self.selected.min(self.rows.len() - 1);
        true
    }

    fn swap_rows(&mut self, a: usize, b: usize) {
        self.rows.swap(a, b);
        self.remap_align(|i| {
            Some(if i == a {
                b
            } else if i == b {
                a
            } else {
                i
            })
        });
    }

    pub fn move_up(&mut self) -> bool {
        if self.selected == 0 || self.selected >= self.rows.len() {
            return false;
        }
        self.swap_rows(self.selected, self.selected - 1);
        self.selected -= 1;
        true
    }

    pub fn move_down(&mut self) -> bool {
        if self.selected + 1 >= self.rows.len() {
            return false;
        }
        self.swap_rows(self.selected, self.selected + 1);
        self.selected += 1;
        true
    }

    /// Makes row `i` the (only) main layer.
    pub fn set_main(&mut self, i: usize) {
        if i >= self.rows.len() {
            return;
        }
        for (k, r) in self.rows.iter_mut().enumerate() {
            r.is_main = k == i;
        }
    }

    /// The Main checkbox of row `i`: turns it on or off. A Main layer joins
    /// the Main section only next to another Main layer, and the last Main
    /// layer stays. Returns whether anything changed.
    pub fn toggle_main(&mut self, i: usize) -> bool {
        if i >= self.rows.len() {
            return false;
        }
        if self.rows[i].is_main {
            if self.main_count() <= 1 {
                return false;
            }
            // Only the edge layers of the section can leave it.
            let (first, last) = self.to_def(&blank_def()).main_span().unwrap_or((i, i));
            if i != first && i != last {
                return false;
            }
            self.rows[i].is_main = false;
            return true;
        }
        let next_to_main = (i > 0 && self.rows[i - 1].is_main)
            || (i + 1 < self.rows.len() && self.rows[i + 1].is_main);
        if self.main_count() > 0 && !next_to_main {
            return false;
        }
        self.rows[i].is_main = true;
        true
    }

    /// Picks the role of row `i`.
    pub fn set_role(&mut self, i: usize, role: LayerRole) {
        if let Some(r) = self.rows.get_mut(i) {
            r.set_role(role);
        }
    }

    /// Sets the total thickness: the outermost Main layer takes the change
    /// (DECISIONS 47: it may go down to 1/16 in, so the total cannot drop
    /// below the other layers plus 1/16 in). False when refused.
    pub fn set_total(&mut self, total: f64) -> bool {
        let mut def = self.to_def(&blank_def());
        if !def.set_total(total) {
            return false;
        }
        self.rows = def.layers;
        true
    }

    /// The least a total thickness can be: the layers other than the
    /// outermost Main layer, plus 1/16 in for it.
    pub fn min_total(&self) -> f64 {
        match self.main_index() {
            Some(m) => self.total() - self.rows[m].thickness + MIN_MAIN_LAYER,
            None => self.total(),
        }
    }

    /// Why the table cannot be accepted, if it cannot.
    pub fn error(&self) -> Option<String> {
        self.to_def(&blank_def())
            .problems()
            .first()
            .map(wall_types::TypeProblem::message)
    }
}

fn blank_def() -> WallTypeDef {
    WallTypeDef {
        name: String::new(),
        layers: Vec::new(),
        kind: plan_core::WallKind::Interior,
        props: WallTypeProps::default(),
    }
}

/// Chief's "Resize About" choices, in the order of its combo box.
pub const RESIZE_ABOUT: [(ResizeAbout, &str); 5] = [
    (ResizeAbout::MainLayerOutside, "Main Layer Outside"),
    (ResizeAbout::MainLayerInside, "Main Layer Inside"),
    (ResizeAbout::WallCenter, "Wall Center"),
    (ResizeAbout::OuterSurface, "Outer Surface"),
    (ResizeAbout::InnerSurface, "Inner Surface"),
];

pub fn resize_about_label(r: ResizeAbout) -> &'static str {
    RESIZE_ABOUT
        .iter()
        .find(|(v, _)| *v == r)
        .map_or("Wall Center", |(_, l)| l)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Layers,
    Properties,
}

/// The Wall Type Definitions dialog.
pub struct WallTypeDialog {
    id: egui::Id,
    original: Vec<WallTypeDef>,
    types: Vec<WallTypeDef>,
    current: usize,
    table: LayerTable,
    resize_about: ResizeAbout,
    tab: Tab,
    /// Names of the types in use; `None` when the host cannot tell (Delete All
    /// Unused is then off).
    used: Option<BTreeSet<String>>,
    removed: Vec<String>,
    layer_dialog: Option<(usize, WallLayerDialog)>,
    show_plan: bool,
    /// Preview rotation about the vertical axis, radians.
    rotation: f32,
    note: String,
}

impl WallTypeDialog {
    /// `current` names the type to start on (the wall's own); `about` is the
    /// wall's Resize About reference.
    pub fn new(types: Vec<WallTypeDef>, current: Option<&str>, about: ResizeAbout) -> Self {
        let idx = current
            .and_then(|n| types.iter().position(|t| t.name == n))
            .unwrap_or(0);
        let table = types
            .get(idx)
            .map(LayerTable::from_def)
            .unwrap_or_else(LayerTable::empty);
        Self {
            id: egui::Id::new("wall_type_definitions"),
            original: types.clone(),
            types,
            current: idx,
            table,
            resize_about: about,
            tab: Tab::Layers,
            used: None,
            removed: Vec::new(),
            layer_dialog: None,
            show_plan: false,
            rotation: 0.6,
            note: String::new(),
        }
    }

    /// The types the plan's walls and defaults use, which Delete All Unused
    /// keeps.
    pub fn with_used(mut self, used: BTreeSet<String>) -> Self {
        self.used = Some(used);
        self
    }

    /// The table edits are folded into `types` before it is read.
    fn commit_table(&mut self) {
        if let Some(t) = self.types.get_mut(self.current) {
            self.table.apply_to(t);
        }
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.types.get(self.current).map(|t| t.name.as_str())
    }

    pub fn resize_about(&self) -> ResizeAbout {
        self.resize_about
    }

    pub fn table(&self) -> &LayerTable {
        &self.table
    }

    pub fn table_mut(&mut self) -> &mut LayerTable {
        &mut self.table
    }

    /// Every type of the list as edited.
    pub fn types(&mut self) -> &[WallTypeDef] {
        self.commit_table();
        &self.types
    }

    /// The types that were edited or created.
    pub fn changed_types(&mut self) -> Vec<WallTypeDef> {
        self.commit_table();
        self.types
            .iter()
            .filter(|t| self.original.iter().find(|o| o.name == t.name) != Some(t))
            .cloned()
            .collect()
    }

    /// The names of the types Delete All Unused took out.
    pub fn removed_types(&self) -> &[String] {
        &self.removed
    }

    fn select_type(&mut self, i: usize) {
        if i == self.current || i >= self.types.len() {
            return;
        }
        self.commit_table();
        self.current = i;
        self.table = LayerTable::from_def(&self.types[i]);
        self.layer_dialog = None;
    }

    /// Copy: a new type starting from the current one, under a fresh name;
    /// becomes current.
    pub fn new_type(&mut self) {
        self.commit_table();
        let Some(base) = self.types.get(self.current).cloned() else {
            return;
        };
        let mut n = 1;
        let name = loop {
            let candidate = format!("{} copy {n}", base.name);
            if !self.types.iter().any(|t| t.name == candidate) {
                break candidate;
            }
            n += 1;
        };
        let mut t = base;
        t.name = name;
        self.types.push(t);
        self.go_to_last();
    }

    /// A Room Divider type: 0 in thick, a dashed pair of lines.
    pub fn new_room_divider(&mut self) {
        self.commit_table();
        let name =
            wall_types::unique_name("Room Divider", |n| self.types.iter().any(|t| t.name == n));
        self.types.push(WallTypeDef::room_divider_type(&name));
        self.go_to_last();
    }

    /// Gives the current type a new name (the Name field of the dialog); a
    /// name another type already has is refused. True when it changed.
    pub fn rename_current(&mut self, name: &str) -> bool {
        self.commit_table();
        let taken = self
            .types
            .iter()
            .enumerate()
            .any(|(i, t)| i != self.current && t.name == name);
        if name.trim().is_empty() || taken {
            return false;
        }
        match self.types.get_mut(self.current) {
            Some(t) => t.name = name.trim().to_string(),
            None => return false,
        }
        true
    }

    fn go_to_last(&mut self) {
        let last = self.types.len() - 1;
        self.current = last;
        self.table = LayerTable::from_def(&self.types[last]);
        self.layer_dialog = None;
    }

    /// Delete All Unused: takes out the types no wall or default uses.
    /// Returns how many went; 0 (and nothing changes) when the host gave no
    /// list of the types in use.
    pub fn delete_unused(&mut self) -> usize {
        let Some(used) = self.used.clone() else {
            return 0;
        };
        self.commit_table();
        let current_name = self.selected_name().map(str::to_string);
        // The type on screen is kept, the way Chief keeps what you are editing.
        let mut keep = used;
        if let Some(n) = &current_name {
            keep.insert(n.clone());
        }
        let gone = wall_types::unused_names(&self.types, &keep);
        let n = wall_types::delete_unused(&mut self.types, &keep);
        self.removed.extend(gone);
        self.current = current_name
            .and_then(|n| self.types.iter().position(|t| t.name == n))
            .unwrap_or(0);
        self.table = self
            .types
            .get(self.current)
            .map(LayerTable::from_def)
            .unwrap_or_else(LayerTable::empty);
        n
    }

    /// Import Wall Types from another plan: same-named types that differ
    /// arrive as `Name_2`.
    pub fn import_from(&mut self, src: &Project) -> ImportReport {
        self.commit_table();
        let report = wall_types::import_types(&mut self.types, &src.wall_types);
        self.removed
            .retain(|n| !self.types.iter().any(|t| &t.name == n));
        report
    }

    fn is_new(&self) -> bool {
        self.types
            .get(self.current)
            .is_some_and(|t| !self.original.iter().any(|o| o.name == t.name))
    }

    /// Opens the Wall Layer Specification on the selected row.
    pub fn edit_layer(&mut self, on_fill: bool) {
        let i = self.table.selected;
        let Some(layer) = self.table.rows.get(i).cloned() else {
            return;
        };
        let d = WallLayerDialog::new(layer, self.table.group_of(i), i);
        self.layer_dialog = Some((i, if on_fill { d.on_fill_page() } else { d }));
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = self.table.error();
        // The layer dialog sits on top: its keys are its own.
        let layer_open = self.layer_dialog.is_some();
        egui::Window::new("Wall Type Definitions")
            .id(self.id)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([760.0, 560.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                self.body(ui, error.as_deref(), &mut outcome);
            });
        if let Some((i, mut d)) = self.layer_dialog.take() {
            match d.show(ctx) {
                Outcome::Open => self.layer_dialog = Some((i, d)),
                Outcome::Ok => {
                    if let Some(r) = self.table.rows.get_mut(i) {
                        *r = d.layer().clone();
                    }
                }
                Outcome::Cancel => {}
            }
        }
        if !open {
            outcome = Outcome::Cancel;
        }
        if layer_open {
            return outcome;
        }
        // Consumed so the dialog underneath does not react to the same key.
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
            && error.is_none()
            && outcome == Outcome::Open
        {
            outcome = Outcome::Ok;
        }
        outcome
    }

    fn body(&mut self, ui: &mut egui::Ui, error: Option<&str>, outcome: &mut Outcome) {
        section(ui, "Wall Type");
        ui.horizontal(|ui| {
            let shown = self.selected_name().unwrap_or("-").to_string();
            let mut pick = None;
            egui::ComboBox::from_id_salt("wt_dialog_type")
                .selected_text(shown)
                .width(190.0)
                .show_ui(ui, |ui| {
                    for (i, t) in self.types.iter().enumerate() {
                        if ui.selectable_label(i == self.current, &t.name).clicked() {
                            pick = Some(i);
                        }
                    }
                });
            if let Some(i) = pick {
                self.select_type(i);
            }
            if ui
                .button("Copy")
                .on_hover_text("Start a new type from this one")
                .clicked()
            {
                self.new_type();
            }
            if ui.button("Room Divider").clicked() {
                self.new_room_divider();
            }
            let can = self.used.is_some();
            if ui
                .add_enabled(can, egui::Button::new("Delete All Unused"))
                .clicked()
            {
                let n = self.delete_unused();
                self.note = format!("{n} unused wall type(s) deleted");
            }
            if ui.button("Import\u{2026}").clicked() {
                self.import_dialog();
            }
        });
        if self.is_new() {
            let cur = self.current;
            row(ui, "Name", |ui| {
                let mut name = self.types[cur].name.clone();
                if ui.text_edit_singleline(&mut name).changed() && !name.trim().is_empty() {
                    self.types[cur].name = name;
                }
            });
        }
        if !self.note.is_empty() {
            ui.weak(&self.note);
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Layers, "Wall Layers");
            ui.selectable_value(&mut self.tab, Tab::Properties, "Wall Properties");
        });
        ui.separator();
        match self.tab {
            Tab::Layers => self.layers_tab(ui),
            Tab::Properties => self.properties_tab(ui),
        }

        section(ui, "Preview");
        self.preview(ui);

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if let Some(e) = error {
                ui.colored_label(super::ERROR_RED, e);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ok = egui::Button::new(RichText::new("   OK   ").strong());
                if ui.add_enabled(error.is_none(), ok).clicked() {
                    *outcome = Outcome::Ok;
                }
                if ui.button("Cancel").clicked() {
                    *outcome = Outcome::Cancel;
                }
            });
        });
    }

    /// Import: picks a Plan Studio file and brings its wall types in.
    fn import_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Plan Studio", &["plan"])
            .pick_file()
        else {
            return;
        };
        match plan_core::io::load_project(&path) {
            Ok(p) => {
                let r = self.import_from(&p);
                self.note = format!(
                    "Imported {} wall type(s), {} renamed, {} already here",
                    r.added.len(),
                    r.renamed.len(),
                    r.same.len()
                );
            }
            Err(e) => self.note = format!("Import failed: {e}"),
        }
    }

    fn layers_tab(&mut self, ui: &mut egui::Ui) {
        let groups: Vec<LayerGroup> = (0..self.table.rows.len())
            .map(|i| self.table.group_of(i))
            .collect();
        let mut main_toggle = None;
        let mut role_pick = None;
        let mut open_fill = None;
        egui::Grid::new("wt_layers")
            .num_columns(8)
            .spacing([8.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for h in [
                    "#",
                    "Main",
                    "Name",
                    "Thickness",
                    "Extension",
                    "Role",
                    "Fill",
                    "Material",
                ] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                let mut last_group = None;
                for (i, r) in self.table.rows.iter_mut().enumerate() {
                    if last_group != Some(groups[i]) {
                        last_group = Some(groups[i]);
                        ui.label(RichText::new(groups[i].label()).italics().weak());
                        for _ in 0..7 {
                            ui.label("");
                        }
                        ui.end_row();
                    }
                    if ui
                        .selectable_label(self.table.selected == i, format!("{}", i + 1))
                        .clicked()
                    {
                        self.table.selected = i;
                    }
                    let mut main = r.is_main;
                    if ui.checkbox(&mut main, "").clicked() {
                        main_toggle = Some(i);
                    }
                    ui.add(egui::TextEdit::singleline(&mut r.name).desired_width(110.0));
                    ui.add(
                        egui::DragValue::new(&mut r.thickness)
                            .speed(0.0625)
                            .range(0.0..=48.0)
                            .suffix("\""),
                    );
                    if groups[i] == LayerGroup::Exterior {
                        ui.add(
                            egui::DragValue::new(&mut r.spec.extension)
                                .speed(0.25)
                                .range(0.0..=240.0)
                                .suffix("\""),
                        );
                    } else {
                        ui.label("");
                    }
                    let mut role = r.resolved_role();
                    egui::ComboBox::from_id_salt(("wt_role", i))
                        .selected_text(role.label())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for x in super::wall_layer::ROLES {
                                ui.selectable_value(&mut role, x, x.label());
                            }
                        });
                    if role != r.resolved_role() {
                        role_pick = Some((i, role));
                    }
                    let (swatch, _) = ui.allocate_exact_size(Vec2::new(40.0, 16.0), Sense::click());
                    let resp = ui.interact(swatch, ui.id().with(("wt_fill", i)), Sense::click());
                    paint_swatch(ui, swatch, r);
                    if resp.clicked() {
                        open_fill = Some(i);
                    }
                    ui.add(egui::TextEdit::singleline(&mut r.material).desired_width(110.0));
                    ui.end_row();
                }
            });
        if let Some((i, role)) = role_pick {
            self.table.set_role(i, role);
        }
        if let Some(i) = main_toggle {
            self.table.toggle_main(i);
            self.table.selected = i;
        }
        if let Some(i) = open_fill {
            self.table.selected = i;
            self.edit_layer(true);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Insert Above").clicked() {
                self.table.insert_above();
            }
            if ui.button("Insert Below").clicked() {
                self.table.insert_below();
            }
            if ui.button("Delete").clicked() {
                self.table.delete_selected();
            }
            // With several Main layers the order inside the section is the
            // type's own business, so the arrows stay for a single one.
            if ui.button("Move Up").clicked() {
                self.table.move_up();
            }
            if ui.button("Move Down").clicked() {
                self.table.move_down();
            }
            if ui.button("Edit Layer\u{2026}").clicked() {
                self.edit_layer(false);
            }
        });
        let mut total = self.table.total();
        let min = self.table.min_total();
        ui.horizontal(|ui| {
            ui.label("Total Thickness");
            if ui
                .add(
                    egui::DragValue::new(&mut total)
                        .speed(0.0625)
                        .range(min..=480.0)
                        .suffix("\""),
                )
                .changed()
            {
                self.table.set_total(total);
            }
            ui.weak(format!("({})", super::fmt_short(self.table.total())));
        });
        row(ui, "Resize About", |ui| {
            egui::ComboBox::from_id_salt("wt_resize_about")
                .selected_text(resize_about_label(self.resize_about))
                .show_ui(ui, |ui| {
                    for (v, label) in RESIZE_ABOUT {
                        ui.selectable_value(&mut self.resize_about, v, label);
                    }
                });
        });
    }

    fn properties_tab(&mut self, ui: &mut egui::Ui) {
        let names: Vec<String> = self
            .table
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| format!("{}. {}", i + 1, r.name))
            .collect();
        section(ui, "Wall Properties");
        let p = &mut self.table.props;
        layer_pick(
            ui,
            "Dimension to Exterior of Layer",
            "wt_dim",
            &names,
            &mut p.align.dimension,
        );
        layer_pick(
            ui,
            "Foundation to Exterior of Layer",
            "wt_found",
            &names,
            &mut p.align.foundation,
        );
        row(ui, "Foundation Offset", |ui| {
            ui.add(
                egui::DragValue::new(&mut p.align.foundation_offset)
                    .speed(0.125)
                    .suffix("\""),
            )
        });
        layer_pick(
            ui,
            "Build Platform To This Line",
            "wt_platform",
            &names,
            &mut p.align.build_platform,
        );
        layer_pick(
            ui,
            "Roof Planes Build To",
            "wt_roof",
            &names,
            &mut p.align.roof,
        );
        row(ui, "Brick Ledge Depth", |ui| {
            ui.label(super::fmt_short(
                self.table.to_def(&blank_def()).max_extension(),
            ))
        });
        let p = &mut self.table.props;
        section(ui, "Kind of Wall");
        ui.checkbox(
            &mut p.partition,
            "Partition Wall (stops at the surfaces of floors, ceilings and walls)",
        );
        ui.checkbox(
            &mut p.room_divider,
            "Room Divider (0 in thick, dashed pair)",
        );
        section(ui, "Energy Values");
        ui.checkbox(&mut p.energy.framed, "Framed");
        row(ui, "Cavity R-Value", |ui| {
            ui.add(
                egui::DragValue::new(&mut p.energy.cavity_r)
                    .speed(0.5)
                    .range(0.0..=100.0),
            )
        });
        row(ui, "Continuous R-Value", |ui| {
            ui.add(
                egui::DragValue::new(&mut p.energy.continuous_r)
                    .speed(0.5)
                    .range(0.0..=100.0),
            )
        });
    }

    /// The preview: a cut through the wall you can turn by dragging, or the
    /// plan view with the layers' fills.
    fn preview(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_plan, "Show Plan View");
            if !self.show_plan {
                ui.weak("drag to rotate");
            }
        });
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(ui.available_width().min(560.0), 130.0),
            Sense::drag(),
        );
        if resp.dragged() && !self.show_plan {
            self.rotation = (self.rotation + resp.drag_delta().x * 0.02).clamp(-1.4, 1.4);
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 3.0, super::PV_BG);
        if self.show_plan {
            let layers: Vec<(&str, f64)> = self
                .table
                .rows
                .iter()
                .map(|r| (r.name.as_str(), r.thickness))
                .collect();
            layer_stack(&painter, rect.shrink(8.0), &layers);
            return;
        }
        // A block of the wall turned by `rotation`: every layer's front face
        // narrows with cos and its end shows with sin.
        let total = self.table.total().max(0.01) as f32;
        let area = rect.shrink(14.0);
        let k = self.rotation.cos().abs().max(0.12);
        let depth = self.rotation.sin() * 40.0;
        let w = (area.width() - 40.0) * k;
        let h = area.height() - 30.0;
        let origin = Pos2::new(area.min.x + 10.0, area.max.y);
        let mut x = origin.x;
        for r in &self.table.rows {
            let lw = r.thickness as f32 / total * w;
            let (c, _) = swatch_color(r);
            let front = [
                Pos2::new(x, origin.y),
                Pos2::new(x + lw, origin.y),
                Pos2::new(x + lw, origin.y - h),
                Pos2::new(x, origin.y - h),
            ];
            painter.add(egui::Shape::convex_polygon(
                front.to_vec(),
                c,
                Stroke::new(1.0_f32, super::PV_INK),
            ));
            let top = [
                Pos2::new(x, origin.y - h),
                Pos2::new(x + lw, origin.y - h),
                Pos2::new(x + lw + depth, origin.y - h - depth.abs() * 0.5),
                Pos2::new(x + depth, origin.y - h - depth.abs() * 0.5),
            ];
            painter.add(egui::Shape::convex_polygon(
                top.to_vec(),
                c.gamma_multiply(0.8),
                Stroke::new(1.0_f32, super::PV_INK),
            ));
            x += lw;
        }
    }
}

fn layer_pick(ui: &mut egui::Ui, label: &str, salt: &str, names: &[String], v: &mut Option<usize>) {
    row(ui, label, |ui| {
        let shown = match v {
            Some(i) => names.get(*i).cloned().unwrap_or_default(),
            None => "Main Layer (default)".into(),
        };
        egui::ComboBox::from_id_salt(salt)
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(v, None, "Main Layer (default)");
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(v, Some(i), n);
                }
            });
    });
}

/// The colour a layer shows in the preview and its Fill swatch: its fill's
/// single colour, else a neutral one by role. The flag says a fill is set.
fn swatch_color(l: &WallLayer) -> (Color32, bool) {
    match &l.spec.fill {
        Some(f) => {
            let rgb = match f.color {
                ColorSource::Single(c) => c,
                _ => [190, 190, 190],
            };
            (Color32::from_rgb(rgb[0], rgb[1], rgb[2]), true)
        }
        None if l.is_main => (Color32::from_rgb(0xE6, 0xD3, 0xA8), false),
        None => (Color32::from_rgb(0xF2, 0xF2, 0xF2), false),
    }
}

fn paint_swatch(ui: &egui::Ui, rect: egui::Rect, l: &WallLayer) {
    let (c, set) = swatch_color(l);
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, c);
    p.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0_f32, super::PV_INK),
        egui::StrokeKind::Inside,
    );
    if !set {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "none",
            egui::FontId::proportional(10.0),
            Color32::GRAY,
        );
    }
}

// ----- Build > Wall > Define Wall Types (the dialog on its own) -----

/// Opens the dialog on the plan's wall types (Build > Wall > Define Wall Types).
pub const OPEN: &str = "wall.types.define";
/// OK of the standalone dialog.
pub const APPLY: &str = "wall.types.apply";

thread_local! {
    static HOST: std::cell::RefCell<Option<WallTypeDialog>> =
        const { std::cell::RefCell::new(None) };
}

pub fn is_command(id: &str) -> bool {
    id == OPEN || id == APPLY
}

/// Whether the standalone dialog is open.
pub fn host_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

/// The plan's wall types as the dialog lists them: the plan's own over the
/// defaults' of the same name, then the plan's other ones.
pub fn plan_types(cx: &EditorContext) -> Vec<WallTypeDef> {
    let mut out = cx.defaults.wall_types.clone();
    for t in &cx.project.wall_types {
        match out.iter_mut().find(|o| o.name == t.name) {
            Some(slot) => *slot = t.clone(),
            None => out.push(t.clone()),
        }
    }
    out
}

pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        OPEN => {
            if !host_open() {
                let types = plan_types(cx);
                let used = wall_types::used_names(&cx.project, Some(&cx.defaults));
                let current = cx.defaults.exterior_wall.wall_type.clone();
                let d = WallTypeDialog::new(types, Some(&current), ResizeAbout::default())
                    .with_used(used);
                HOST.with(|h| *h.borrow_mut() = Some(d));
            }
            true
        }
        APPLY => {
            if let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) {
                apply(cx, &mut d);
            }
            true
        }
        _ => false,
    }
}

/// OK of the dialog: one undo step. The edited types go to the defaults and
/// the plan; walls of an edited type take its new total thickness; the types
/// Delete All Unused took go from both.
pub fn apply(cx: &mut EditorContext, d: &mut WallTypeDialog) {
    let changed = d.changed_types();
    let removed = d.removed_types().to_vec();
    if changed.is_empty() && removed.is_empty() {
        return;
    }
    cx.begin_change("Wall Type Definitions");
    for t in changed {
        match cx.defaults.wall_types.iter_mut().find(|x| x.name == t.name) {
            Some(slot) => *slot = t.clone(),
            None => cx.defaults.wall_types.push(t.clone()),
        }
        let thickness = t.thickness();
        for f in &mut cx.project.floors {
            for w in f
                .walls
                .iter_mut()
                .filter(|w| w.wall_type.as_deref() == Some(t.name.as_str()))
            {
                w.thickness = thickness;
            }
        }
        cx.project.register_wall_type(t);
    }
    cx.defaults
        .wall_types
        .retain(|t| !removed.contains(&t.name));
    cx.project.wall_types.retain(|t| !removed.contains(&t.name));
    cx.status = "Wall types updated".into();
}

/// Draws the standalone dialog; OK asks for [`APPLY`].
pub fn show_windows(ctx: &egui::Context, out: &mut Vec<crate::toolbar::Action>) {
    let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => HOST.with(|h| *h.borrow_mut() = Some(d)),
        Outcome::Ok => {
            HOST.with(|h| *h.borrow_mut() = Some(d));
            out.push(crate::toolbar::Action::Custom(APPLY));
        }
        Outcome::Cancel => {}
    }
}

/// Closes the standalone dialog without applying it.
pub fn close_host() {
    HOST.with(|h| *h.borrow_mut() = None);
}

/// Test hook: runs `f` on the standalone dialog.
pub fn with_host<R>(f: impl FnOnce(&mut WallTypeDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{PlanDefaults, WallKind};

    fn table() -> LayerTable {
        let d = PlanDefaults::chief_x18_daniel();
        LayerTable::from_def(d.wall_type("Interior-4").unwrap())
    }

    fn names(t: &LayerTable) -> Vec<&str> {
        t.rows.iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn total_matches_the_type_and_follows_edits() {
        let d = PlanDefaults::chief_x18_daniel();
        let def = d.wall_type("Stucco-6").unwrap();
        let mut t = LayerTable::from_def(def);
        assert!((t.total() - def.thickness()).abs() < 1e-9);
        t.rows[0].thickness += 0.25;
        assert!((t.total() - def.thickness() - 0.25).abs() < 1e-9);
        let mut copy = def.clone();
        t.apply_to(&mut copy);
        assert!((copy.thickness() - t.total()).abs() < 1e-9);
        assert_eq!(copy.name, def.name);
    }

    #[test]
    fn reorder_moves_the_selection_with_the_row() {
        let mut t = table();
        let first: Vec<String> = t.rows.iter().map(|r| r.name.clone()).collect();
        assert!(first.len() >= 3);
        t.selected = 0;
        assert!(!t.move_up());
        assert!(t.move_down());
        assert_eq!(t.selected, 1);
        assert_eq!(t.rows[1].name, first[0]);
        assert_eq!(t.rows[0].name, first[1]);
        assert!(t.move_up());
        assert_eq!(t.selected, 0);
        t.selected = t.rows.len() - 1;
        assert!(!t.move_down());
    }

    #[test]
    fn main_layer_is_a_radio() {
        let mut t = table();
        let old = t.main_index().unwrap();
        let other = if old == 0 { 1 } else { 0 };
        t.set_main(other);
        assert_eq!(t.main_index(), Some(other));
        assert_eq!(t.rows.iter().filter(|r| r.is_main).count(), 1);
        assert!(t.error().is_none());
        t.rows.iter_mut().for_each(|r| r.is_main = false);
        assert!(t.error().is_some());
    }

    #[test]
    fn insert_and_delete_keep_one_main_layer() {
        let mut t = table();
        let n = t.rows.len();
        t.selected = 1;
        t.insert_above();
        assert_eq!((t.rows.len(), t.selected), (n + 1, 1));
        t.insert_below();
        assert_eq!((t.rows.len(), t.selected), (n + 2, 2));
        assert_eq!(names(&t)[1], "New Layer");
        assert_eq!(names(&t)[2], "New Layer");
        // Deleting the main layer hands the role to a neighbour.
        t.selected = t.main_index().unwrap();
        assert!(t.delete_selected());
        assert_eq!(t.rows.iter().filter(|r| r.is_main).count(), 1);
        assert!(t.error().is_none());
        // The last layer stays.
        while t.rows.len() > 1 {
            t.selected = 0;
            assert!(t.delete_selected());
        }
        assert!(!t.delete_selected());
        assert!(t.rows[0].is_main);
    }

    #[test]
    fn dialog_reports_changed_and_new_types() {
        let d = PlanDefaults::chief_x18_daniel();
        let mut dlg = WallTypeDialog::new(
            d.wall_types.clone(),
            Some("Interior-4"),
            ResizeAbout::WallCenter,
        );
        assert!(dlg.changed_types().is_empty());
        dlg.table.rows[0].thickness += 0.125;
        let changed = dlg.changed_types();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].name, "Interior-4");
        assert_eq!(changed[0].kind, WallKind::Interior);
        dlg.new_type();
        assert!(dlg.is_new());
        assert_eq!(dlg.changed_types().len(), 2);
        assert_eq!(dlg.selected_name(), Some("Interior-4 copy 1"));
    }

    #[test]
    fn several_main_layers_sit_together_and_the_last_one_stays() {
        let mut t = LayerTable::from_def(
            PlanDefaults::chief_x18_daniel()
                .wall_type("Stucco-6")
                .unwrap(),
        );
        let m = t.main_index().unwrap();
        // A neighbour joins the Main section; one two rows away does not.
        assert!(t.toggle_main(m + 1));
        assert_eq!(t.main_count(), 2);
        assert!(t.error().is_none());
        assert_eq!(t.group_of(m), LayerGroup::Main);
        assert_eq!(t.group_of(0), LayerGroup::Exterior);
        assert!(!t.toggle_main(0), "not next to the Main section");
        // Leaving takes an edge layer; the last Main layer stays.
        assert!(t.toggle_main(m + 1));
        assert!(!t.toggle_main(m));
        assert_eq!(t.main_count(), 1);
    }

    #[test]
    fn the_total_is_taken_by_the_main_layer_down_to_one_sixteenth() {
        let mut t = LayerTable::from_def(
            PlanDefaults::chief_x18_daniel()
                .wall_type("Stucco-6")
                .unwrap(),
        );
        let m = t.main_index().unwrap();
        let before = t.total();
        let others = before - t.rows[m].thickness;
        assert!(t.set_total(before + 1.0));
        assert!((t.total() - before - 1.0).abs() < 1e-9);
        assert!((t.min_total() - (others + 0.0625)).abs() < 1e-9);
        assert!(!t.set_total(others), "the main layer cannot vanish");
        assert!(t.set_total(others + 0.0625));
        assert!((t.rows[m].thickness - 0.0625).abs() < 1e-9);
        assert!(t.error().is_none());
        t.rows[m].thickness = 0.03;
        assert!(t.error().is_some());
    }

    #[test]
    fn roles_and_extension_survive_the_table_and_alignments_follow_their_layer() {
        let d = PlanDefaults::chief_x18_daniel();
        let def = d.wall_type("Stucco-6").unwrap();
        let mut t = LayerTable::from_def(def);
        t.set_role(0, LayerRole::Cladding);
        t.rows[0].spec.extension = 3.0;
        t.props.align.dimension = Some(1);
        t.selected = 0;
        t.insert_above();
        assert_eq!(t.props.align.dimension, Some(2));
        t.selected = 0;
        assert!(t.delete_selected());
        assert_eq!(t.props.align.dimension, Some(1));
        let mut out = def.clone();
        t.apply_to(&mut out);
        assert_eq!(out.layers[0].resolved_role(), LayerRole::Cladding);
        assert_eq!(out.props.brick_ledge_depth, 3.0);
        assert_eq!(out.props.align.dimension, Some(1));
        // The two-field layers of an older type read back unchanged.
        let plain = LayerTable::from_def(def);
        let mut again = def.clone();
        plain.apply_to(&mut again);
        assert_eq!(&again, def);
    }

    #[test]
    fn delete_all_unused_keeps_the_type_on_screen_and_the_ones_in_use() {
        let d = PlanDefaults::chief_x18_daniel();
        let mut dlg = WallTypeDialog::new(
            d.wall_types.clone(),
            Some("Interior-4"),
            ResizeAbout::WallCenter,
        );
        assert_eq!(
            dlg.delete_unused(),
            0,
            "no list of the types in use: nothing goes"
        );
        let used: BTreeSet<String> = ["Stucco-6".to_string()].into();
        let n_before = d.wall_types.len();
        let mut dlg = dlg.with_used(used);
        let n = dlg.delete_unused();
        assert_eq!(n, n_before - 2);
        assert_eq!(dlg.removed_types().len(), n);
        assert!(dlg.types().iter().any(|t| t.name == "Stucco-6"));
        assert_eq!(dlg.selected_name(), Some("Interior-4"));
    }

    #[test]
    fn import_brings_new_types_and_renames_a_clash() {
        let d = PlanDefaults::chief_x18_daniel();
        let mut dlg = WallTypeDialog::new(
            d.wall_types.clone(),
            Some("Stucco-6"),
            ResizeAbout::WallCenter,
        );
        let mut src = Project::new("src");
        let mut clash = d.wall_type("Stucco-6").unwrap().clone();
        clash.layers[0].thickness += 0.25;
        let mut fresh = clash.clone();
        fresh.name = "Imported-7".into();
        src.wall_types = vec![clash, fresh];
        let r = dlg.import_from(&src);
        assert_eq!(r.added, vec!["Imported-7".to_string()]);
        assert_eq!(
            r.renamed,
            vec![("Stucco-6".to_string(), "Stucco-6_2".to_string())]
        );
        assert!(dlg.types().iter().any(|t| t.name == "Stucco-6_2"));
    }

    #[test]
    fn a_room_divider_is_a_zero_thick_type_that_validates() {
        let d = PlanDefaults::chief_x18_daniel();
        let mut dlg = WallTypeDialog::new(
            d.wall_types.clone(),
            Some("Interior-4"),
            ResizeAbout::WallCenter,
        );
        dlg.new_room_divider();
        assert_eq!(dlg.selected_name(), Some("Room Divider"));
        assert!(dlg.table().props.room_divider);
        assert_eq!(dlg.table().total(), 0.0);
        assert!(dlg.table().error().is_none());
        dlg.new_room_divider();
        assert_eq!(dlg.selected_name(), Some("Room Divider_2"));
    }
}
