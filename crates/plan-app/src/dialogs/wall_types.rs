//! Wall Type Definitions (opened from the Wall Specification's Wall Types tab
//! with "Define..."): Chief's table of a wall type's layers from the exterior
//! face to the interior face, with Name, Thickness, Material and a radio
//! column for the Main Layer, Insert/Delete/Move buttons, a "Resize About"
//! reference and a live layer-stack preview.
//!
//! [`LayerTable`] is the plain data model (and is unit tested); the dialog
//! edits a copy of the wall types and hands back the ones that changed.

use super::{layer_stack, row, section, Outcome};
use eframe::egui::{self, Align2, Key, Modifiers, RichText, Sense, Vec2};
use plan_core::defaults::{WallLayer, WallTypeDef};
use plan_core::ResizeAbout;

const MIN_LAYER_THICKNESS: f64 = 0.0625;

#[derive(Clone, Debug, PartialEq)]
pub struct LayerRow {
    pub name: String,
    pub thickness: f64,
    pub material: String,
    pub is_main: bool,
}

/// The layers of one wall type, exterior first, with the selected row.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerTable {
    pub rows: Vec<LayerRow>,
    pub selected: usize,
}

impl LayerTable {
    pub fn from_def(def: &WallTypeDef) -> Self {
        let rows = def
            .layers
            .iter()
            .map(|l| LayerRow {
                name: l.name.clone(),
                thickness: l.thickness,
                material: l.material.clone(),
                is_main: l.is_main,
            })
            .collect();
        let mut t = Self { rows, selected: 0 };
        t.selected = t.main_index().unwrap_or(0);
        t
    }

    /// Writes the table back into `def` (name and kind are kept).
    pub fn apply_to(&self, def: &mut WallTypeDef) {
        def.layers = self
            .rows
            .iter()
            .map(|r| WallLayer {
                name: r.name.clone(),
                thickness: r.thickness,
                is_main: r.is_main,
                material: r.material.clone(),
            })
            .collect();
    }

    /// Total thickness, inches.
    pub fn total(&self) -> f64 {
        self.rows.iter().map(|r| r.thickness).sum()
    }

    pub fn main_index(&self) -> Option<usize> {
        self.rows.iter().position(|r| r.is_main)
    }

    fn new_row() -> LayerRow {
        LayerRow {
            name: "New Layer".into(),
            thickness: 0.5,
            material: "Generic".into(),
            is_main: false,
        }
    }

    /// Inserts a layer above (exterior side of) the selected one and selects it.
    pub fn insert_above(&mut self) {
        let at = self.selected.min(self.rows.len());
        self.rows.insert(at, Self::new_row());
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
        self.selected = at;
    }

    /// Deletes the selected layer; the last layer cannot be deleted. When the
    /// main layer goes, the nearest remaining layer becomes the main layer.
    pub fn delete_selected(&mut self) -> bool {
        if self.rows.len() <= 1 || self.selected >= self.rows.len() {
            return false;
        }
        let removed = self.rows.remove(self.selected);
        if removed.is_main {
            let near = self.selected.min(self.rows.len() - 1);
            self.rows[near].is_main = true;
        }
        self.selected = self.selected.min(self.rows.len() - 1);
        true
    }

    pub fn move_up(&mut self) -> bool {
        if self.selected == 0 || self.selected >= self.rows.len() {
            return false;
        }
        self.rows.swap(self.selected, self.selected - 1);
        self.selected -= 1;
        true
    }

    pub fn move_down(&mut self) -> bool {
        if self.selected + 1 >= self.rows.len() {
            return false;
        }
        self.rows.swap(self.selected, self.selected + 1);
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

    /// Why the table cannot be accepted, if it cannot.
    pub fn error(&self) -> Option<String> {
        if self.rows.is_empty() {
            return Some("A wall type needs at least one layer".into());
        }
        if self.rows.iter().filter(|r| r.is_main).count() != 1 {
            return Some("Exactly one layer must be the main layer".into());
        }
        if self.rows.iter().any(|r| r.thickness < MIN_LAYER_THICKNESS) {
            return Some("Every layer needs a thickness".into());
        }
        if self.rows.iter().any(|r| r.name.trim().is_empty()) {
            return Some("Every layer needs a name".into());
        }
        None
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

pub struct WallTypeDialog {
    id: egui::Id,
    original: Vec<WallTypeDef>,
    types: Vec<WallTypeDef>,
    current: usize,
    table: LayerTable,
    resize_about: ResizeAbout,
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
            .unwrap_or(LayerTable {
                rows: Vec::new(),
                selected: 0,
            });
        Self {
            id: egui::Id::new("wall_type_definitions"),
            original: types.clone(),
            types,
            current: idx,
            table,
            resize_about: about,
        }
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

    /// The types that were edited or created.
    pub fn changed_types(&mut self) -> Vec<WallTypeDef> {
        self.commit_table();
        self.types
            .iter()
            .filter(|t| self.original.iter().find(|o| o.name == t.name) != Some(t))
            .cloned()
            .collect()
    }

    fn select_type(&mut self, i: usize) {
        if i == self.current || i >= self.types.len() {
            return;
        }
        self.commit_table();
        self.current = i;
        self.table = LayerTable::from_def(&self.types[i]);
    }

    /// A copy of the current type under a fresh name; becomes current.
    fn new_type(&mut self) {
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
        let last = self.types.len() - 1;
        self.current = last;
        self.table = LayerTable::from_def(&self.types[last]);
    }

    fn is_new(&self) -> bool {
        self.types
            .get(self.current)
            .is_some_and(|t| !self.original.iter().any(|o| o.name == t.name))
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = self.table.error();
        egui::Window::new("Wall Type Definitions")
            .id(self.id)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([620.0, 480.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                self.body(ui, error.as_deref(), &mut outcome);
            });
        if !open {
            outcome = Outcome::Cancel;
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
        row(ui, "Type", |ui| {
            let shown = self.selected_name().unwrap_or("-").to_string();
            let mut pick = None;
            egui::ComboBox::from_id_salt("wt_dialog_type")
                .selected_text(shown)
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
            if ui.button("New Type").clicked() {
                self.new_type();
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

        section(ui, "Layers (exterior to interior)");
        egui::Grid::new("wt_layers")
            .num_columns(5)
            .spacing([8.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(RichText::new("#").strong());
                ui.label(RichText::new("Main Layer").strong());
                ui.label(RichText::new("Name").strong());
                ui.label(RichText::new("Thickness").strong());
                ui.label(RichText::new("Material").strong());
                ui.end_row();
                let mut main_pick = None;
                for (i, r) in self.table.rows.iter_mut().enumerate() {
                    if ui
                        .selectable_label(self.table.selected == i, format!("{}", i + 1))
                        .clicked()
                    {
                        self.table.selected = i;
                    }
                    if ui.radio(r.is_main, "").clicked() {
                        main_pick = Some(i);
                    }
                    ui.add(egui::TextEdit::singleline(&mut r.name).desired_width(120.0));
                    ui.add(
                        egui::DragValue::new(&mut r.thickness)
                            .speed(0.0625)
                            .range(0.0..=48.0)
                            .suffix("\""),
                    );
                    ui.add(egui::TextEdit::singleline(&mut r.material).desired_width(120.0));
                    ui.end_row();
                }
                if let Some(i) = main_pick {
                    self.table.set_main(i);
                    self.table.selected = i;
                }
            });
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
            if ui.button("Move Up").clicked() {
                self.table.move_up();
            }
            if ui.button("Move Down").clicked() {
                self.table.move_down();
            }
        });
        ui.label(format!(
            "Total thickness: {}",
            super::fmt_short(self.table.total())
        ));

        section(ui, "Resize About");
        row(ui, "Resize About", |ui| {
            egui::ComboBox::from_id_salt("wt_resize_about")
                .selected_text(resize_about_label(self.resize_about))
                .show_ui(ui, |ui| {
                    for (v, label) in RESIZE_ABOUT {
                        ui.selectable_value(&mut self.resize_about, v, label);
                    }
                });
        });

        section(ui, "Preview");
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width().min(520.0), 120.0),
            Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 3.0, super::PV_BG);
        let layers: Vec<(&str, f64)> = self
            .table
            .rows
            .iter()
            .map(|r| (r.name.as_str(), r.thickness))
            .collect();
        layer_stack(&painter, rect.shrink(8.0), &layers);

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
}
