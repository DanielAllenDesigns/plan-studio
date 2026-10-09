//! The Materials List Specification dialog (manual pp. 1372-1376) with
//! Chief's panels (Categories, Columns, Options folded into General) and the
//! Report and Text Style tabs.

use super::State;
use crate::editor::EditorContext;
use eframe::egui::{self, Color32, RichText};
use plan_core::materials_data::{
    Appearance, FramingStyle, GroupBy, ListKind, ListScope, ListSpec, MlColumn, SortKey,
    SupplierFilter, CATEGORIES,
};
use plan_core::Id;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    General,
    Categories,
    Columns,
    Report,
    TextStyle,
    Layer,
}

impl Tab {
    const ALL: [Tab; 6] = [
        Tab::General,
        Tab::Categories,
        Tab::Columns,
        Tab::Report,
        Tab::TextStyle,
        Tab::Layer,
    ];

    fn title(self) -> &'static str {
        match self {
            Tab::General => "General",
            Tab::Categories => "Categories",
            Tab::Columns => "Columns",
            Tab::Report => "Report",
            Tab::TextStyle => "Text Style",
            Tab::Layer => "Layer",
        }
    }
}

/// Which kind of scope the General tab has selected.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScopeKind {
    All,
    Floor,
    Area,
    Room,
    Selection,
}

/// The dialog being edited.
pub struct SpecDialog {
    pub draft: ListSpec,
    pub tab: Tab,
    scope: ScopeKind,
    floor: usize,
    area: Option<Id>,
    room: usize,
    floors: Vec<String>,
    polylines: Vec<(Id, String)>,
    /// `(name, floor, x, y)` of the rooms of the active floor.
    rooms: Vec<(String, usize, f64, f64)>,
    selection: Vec<plan_core::materials_data::ObjectAddr>,
    suppliers: Vec<String>,
    layers: Vec<String>,
    selected_col: Option<MlColumn>,
}

impl SpecDialog {
    pub fn new(spec: &ListSpec, cx: &EditorContext) -> Self {
        let rooms: Vec<(String, usize, f64, f64)> = cx
            .rooms
            .iter()
            .map(|r| {
                (
                    plan_docs::room_name(cx.floor(), r),
                    cx.floor,
                    r.centroid.x,
                    r.centroid.y,
                )
            })
            .collect();
        let mut suppliers: Vec<String> = cx
            .project
            .materials
            .objects
            .values()
            .map(|o| o.supplier.clone())
            .filter(|s| !s.is_empty())
            .collect();
        suppliers.extend(
            crate::shell::layout_window::load_master_list()
                .items
                .iter()
                .map(|i| i.supplier.clone())
                .filter(|s| !s.is_empty()),
        );
        suppliers.sort();
        suppliers.dedup();
        let (scope, floor, area, room) = match &spec.scope {
            ListScope::AllFloors => (ScopeKind::All, cx.floor, None, 0),
            ListScope::Floor(f) => (ScopeKind::Floor, *f, None, 0),
            ListScope::Polyline(id) => (ScopeKind::Area, cx.floor, Some(*id), 0),
            ListScope::Room { floor, x, y } => {
                let at = rooms
                    .iter()
                    .position(|r| r.1 == *floor && (r.2 - x).abs() < 1.5 && (r.3 - y).abs() < 1.5)
                    .unwrap_or(0);
                (ScopeKind::Room, *floor, None, at)
            }
            ListScope::Selection(_) => (ScopeKind::Selection, cx.floor, None, 0),
        };
        let mut draft = spec.clone();
        draft.normalize_columns();
        Self {
            draft,
            tab: Tab::General,
            scope,
            floor,
            area,
            room,
            floors: cx.project.floors.iter().map(|f| f.name.clone()).collect(),
            polylines: cx
                .project
                .materials
                .polylines
                .iter()
                .map(|p| (p.cad_id, p.name.clone()))
                .collect(),
            rooms,
            selection: super::selection_addrs(cx),
            suppliers,
            layers: cx
                .project
                .layers
                .layers
                .iter()
                .map(|l| l.name.clone())
                .collect(),
            selected_col: None,
        }
    }

    /// The scope the General tab describes.
    fn scope_value(&self) -> ListScope {
        match self.scope {
            ScopeKind::All => ListScope::AllFloors,
            ScopeKind::Floor => ListScope::Floor(self.floor),
            ScopeKind::Area => match self.area {
                Some(id) => ListScope::Polyline(id),
                None => self.draft.scope.clone(),
            },
            ScopeKind::Room => match self.rooms.get(self.room) {
                Some((_, floor, x, y)) => ListScope::Room {
                    floor: *floor,
                    x: *x,
                    y: *y,
                },
                None => self.draft.scope.clone(),
            },
            ScopeKind::Selection => {
                if self.selection.is_empty() {
                    self.draft.scope.clone()
                } else {
                    ListScope::Selection(self.selection.clone())
                }
            }
        }
    }

    /// The spec with the General tab's scope applied.
    pub fn result(&self) -> ListSpec {
        let mut s = self.draft.clone();
        if s.kind == ListKind::Live {
            s.scope = self.scope_value();
        }
        s
    }
}

/// Draws the dialog; applies OK to the window's list.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext, st: &mut State) {
    let Some(mut d) = st.spec_dialog.take() else {
        return;
    };
    let mut ok = false;
    let mut cancel = false;
    let mut open = true;
    egui::Window::new("Materials List Specification")
        .id(egui::Id::new("materials_list_spec"))
        .open(&mut open)
        .collapsible(false)
        .default_size(egui::vec2(560.0, 440.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for t in Tab::ALL {
                    ui.selectable_value(&mut d.tab, t, t.title());
                }
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(330.0)
                .auto_shrink([false, false])
                .show(ui, |ui| match d.tab {
                    Tab::General => general(ui, &mut d),
                    Tab::Categories => categories(ui, &mut d.draft),
                    Tab::Columns => columns(ui, &mut d),
                    Tab::Report => report(ui, &mut d.draft),
                    Tab::TextStyle => text_style(ui, &mut d.draft.appearance),
                    Tab::Layer => layer(ui, &mut d),
                });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if ok {
        let new = d.result();
        if new != st.spec {
            if new.scope != st.spec.scope {
                st.selected.clear();
                st.expanded.clear();
            }
            st.spec = new;
            st.dirty = true;
        }
        let _ = cx;
    } else if !cancel && open {
        st.spec_dialog = Some(d);
    }
}

fn general(ui: &mut egui::Ui, d: &mut SpecDialog) {
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut d.draft.name);
    });
    ui.label(match d.draft.kind {
        ListKind::Live => "A Live List: it follows the plan.",
        ListKind::Report => "A Report: a static copy that does not follow the plan.",
    });
    ui.add_space(6.0);
    ui.add_enabled_ui(d.draft.kind == ListKind::Live, |ui| {
        ui.strong("Calculate");
        ui.radio_value(&mut d.scope, ScopeKind::All, "All floors");
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.scope, ScopeKind::Floor, "Restrict to floor");
            egui::ComboBox::from_id_salt("ml_spec_floor")
                .selected_text(d.floors.get(d.floor).cloned().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for (i, f) in d.floors.clone().iter().enumerate() {
                        ui.selectable_value(&mut d.floor, i, f);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.scope, ScopeKind::Area, "From area");
            let shown = d
                .polylines
                .iter()
                .find(|(id, _)| Some(*id) == d.area)
                .map_or("(none)".to_string(), |p| p.1.clone());
            egui::ComboBox::from_id_salt("ml_spec_area")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (id, name) in d.polylines.clone() {
                        ui.selectable_value(&mut d.area, Some(id), name);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.scope, ScopeKind::Room, "In room");
            let shown = d
                .rooms
                .get(d.room)
                .map_or("(none)".to_string(), |r| r.0.clone());
            egui::ComboBox::from_id_salt("ml_spec_room")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (i, r) in d.rooms.clone().iter().enumerate() {
                        ui.selectable_value(&mut d.room, i, &r.0);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.scope, ScopeKind::Selection, "From selection");
            ui.weak(format!(
                "{} object(s) selected in the plan",
                d.selection.len()
            ));
        });
    });
    ui.add_space(6.0);
    ui.strong("Options");
    ui.horizontal(|ui| {
        ui.label("Restrict to Supplier");
        let shown = match &d.draft.supplier {
            SupplierFilter::All => "Show All Suppliers".to_string(),
            SupplierFilter::NoSupplier => "Show Only No Supplier".to_string(),
            SupplierFilter::Only(s) => s.clone(),
        };
        egui::ComboBox::from_id_salt("ml_spec_supplier")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut d.draft.supplier,
                    SupplierFilter::All,
                    "Show All Suppliers",
                );
                ui.selectable_value(
                    &mut d.draft.supplier,
                    SupplierFilter::NoSupplier,
                    "Show Only No Supplier",
                );
                for s in d.suppliers.clone() {
                    ui.selectable_value(&mut d.draft.supplier, SupplierFilter::Only(s.clone()), s);
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Structural Member Reporting");
        egui::ComboBox::from_id_salt("ml_spec_framing")
            .selected_text(d.draft.framing.title())
            .show_ui(ui, |ui| {
                for f in FramingStyle::ALL {
                    ui.selectable_value(&mut d.draft.framing, f, f.title());
                }
            });
    });
}

fn categories(ui: &mut egui::Ui, spec: &mut ListSpec) {
    ui.horizontal(|ui| {
        if ui.button("Select All").clicked() {
            spec.show_all_categories(true);
        }
        if ui.button("Clear All").clicked() {
            spec.show_all_categories(false);
        }
    });
    ui.add_space(4.0);
    for c in CATEGORIES {
        let mut shown = spec.shows(c);
        if ui.checkbox(&mut shown, c).changed() {
            spec.set_category_shown(c, shown);
        }
    }
    ui.add_space(6.0);
    ui.weak("A category left unchecked is hidden in the window and the printout. Its lines stay in the list and in exported files.");
}

fn columns(ui: &mut egui::Ui, d: &mut SpecDialog) {
    ui.horizontal(|ui| {
        let can = d.selected_col.is_some();
        if ui.add_enabled(can, egui::Button::new("Move Up")).clicked() {
            if let Some(c) = d.selected_col {
                d.draft.move_column(c, -1);
            }
        }
        if ui
            .add_enabled(can, egui::Button::new("Move Down"))
            .clicked()
        {
            if let Some(c) = d.selected_col {
                d.draft.move_column(c, 1);
            }
        }
    });
    ui.add_space(4.0);
    let cols = d.draft.columns.clone();
    for c in cols {
        if !c.col.in_materials_list() {
            continue;
        }
        ui.horizontal(|ui| {
            let mut v = c.visible;
            if ui.checkbox(&mut v, "").changed() {
                d.draft.set_visible(c.col, v);
            }
            if ui
                .selectable_label(d.selected_col == Some(c.col), c.col.title())
                .clicked()
            {
                d.selected_col = Some(c.col);
            }
            ui.weak(if c.col.editable() { "" } else { "(calculated)" });
        });
    }
    ui.add_space(6.0);
    ui.weak("Columns show in the order listed. Use, Quantity and Default belong to the Master List. Drag a column's right edge in the list to change its width.");
}

fn report(ui: &mut egui::Ui, spec: &mut ListSpec) {
    let r = &mut spec.report;
    ui.horizontal(|ui| {
        ui.label("Group by");
        egui::ComboBox::from_id_salt("ml_rep_group")
            .selected_text(r.group_by.title())
            .show_ui(ui, |ui| {
                for g in GroupBy::ALL {
                    ui.selectable_value(&mut r.group_by, g, g.title());
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Sort by");
        egui::ComboBox::from_id_salt("ml_rep_sort")
            .selected_text(r.sort.title())
            .show_ui(ui, |ui| {
                for k in SortKey::ALL {
                    ui.selectable_value(&mut r.sort, k, k.title());
                }
            });
        ui.checkbox(&mut r.descending, "Descending");
    });
    ui.checkbox(&mut r.subtotals, "Subtotal under each group");
    ui.checkbox(&mut r.grand_total, "Total at the foot of the list");
}

fn text_style(ui: &mut egui::Ui, a: &mut Appearance) {
    ui.strong("Grid");
    ui.checkbox(&mut a.horizontal_lines, "Horizontal Lines");
    ui.checkbox(&mut a.vertical_lines, "Vertical Lines");
    ui.checkbox(&mut a.solid_lines, "Solid Lines");
    ui.add_space(4.0);
    ui.strong("Custom Colors");
    ui.checkbox(&mut a.custom_colors, "Custom Colors");
    ui.add_enabled_ui(a.custom_colors, |ui| {
        for (label, c) in [
            ("Background", &mut a.background),
            ("Text", &mut a.text),
            ("Grid", &mut a.grid),
        ] {
            ui.horizontal(|ui| {
                ui.color_edit_button_srgb(c);
                ui.label(label);
            });
        }
    });
    ui.add_space(4.0);
    ui.strong("Font");
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("ml_font")
            .selected_text(a.font.clone())
            .show_ui(ui, |ui| {
                for f in [
                    "Arial",
                    "Helvetica",
                    "Times New Roman",
                    "Georgia",
                    "Courier New",
                ] {
                    ui.selectable_value(&mut a.font, f.to_string(), f);
                }
            });
        ui.add(
            egui::DragValue::new(&mut a.font_size)
                .range(7.0..=24.0)
                .suffix(" pt"),
        );
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut a.bold, "Bold");
        ui.checkbox(&mut a.italic, "Italic");
        ui.checkbox(&mut a.underline, "Underline");
        ui.checkbox(&mut a.strikeout, "Strikeout");
    });
    if ui.button("Reset to Defaults").clicked() {
        *a = Appearance::default();
    }
    ui.add_space(4.0);
    let mut sample = RichText::new("Wall sheathing 7/16\" OSB 4x8 sheet")
        .size(a.font_size)
        .color(if a.custom_colors {
            Color32::from_rgb(a.text[0], a.text[1], a.text[2])
        } else {
            ui.visuals().text_color()
        });
    if a.bold {
        sample = sample.strong();
    }
    if a.italic {
        sample = sample.italics();
    }
    if a.underline {
        sample = sample.underline();
    }
    if a.strikeout {
        sample = sample.strikethrough();
    }
    ui.group(|ui| {
        ui.label(sample);
    });
}

fn layer(ui: &mut egui::Ui, d: &mut SpecDialog) {
    ui.label("The layer a list placed in the plan (a Materials List Polyline's label) draws on.");
    egui::ComboBox::from_id_salt("ml_layer")
        .selected_text(if d.draft.layer.is_empty() {
            "(default)".to_string()
        } else {
            d.draft.layer.clone()
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut d.draft.layer, String::new(), "(default)");
            for l in d.layers.clone() {
                ui.selectable_value(&mut d.draft.layer, l.clone(), l);
            }
        });
}
