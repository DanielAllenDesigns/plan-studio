//! Material Layers Definition (reference manual "Material Layers Definition
//! Dialogs", pp. 1088 to 1091; parity rows CB-462..CB-467): the layer table
//! of a floor or wall Material Region. Each row has a material, a fill
//! style, a role (Standard, Framing, Air Gap, 3D Cladding) and a thickness;
//! the buttons Insert Above/Below, Move Up/Down and Delete edit the table.
//! The Options page holds Cut Finish Layers and Auto Detail as Insulation.
//!
//! The structure is `plan_core::material_region::RegionStructure`. OK is one
//! undo step; the region's own material and thickness follow the table.

use super::{row, section, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::{details_view, EditorContext};
use eframe::egui::{self, Painter, Rect, Ui};
use plan_core::details::{DetailRef, DetailsLayer};
use plan_core::material_region::{LayerRole, RegionStructure};
use plan_core::Id;
use std::cell::RefCell;

const TABS: &[Tab] = &[
    Tab {
        name: "Layers",
        enabled: true,
    },
    Tab {
        name: "Label",
        enabled: true,
    },
    Tab {
        name: "Options",
        enabled: true,
    },
];

pub struct MaterialLayersDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    region: Id,
    structure: RegionStructure,
    cut_finish_layers: bool,
    wall: bool,
    selected: usize,
    materials: Vec<String>,
}

impl MaterialLayersDialog {
    /// The window for region `id`, or `None` when it is gone.
    pub fn new(cx: &EditorContext, id: Id) -> Option<Self> {
        let f = cx.floor();
        let details = DetailsLayer::load(f);
        let DetailRef::Region(_) = details.find(id)? else {
            return None;
        };
        let r = details.regions.iter().find(|r| r.id == id)?;
        let structure = match f.region_structure(id) {
            Some(s) if !s.layers.is_empty() => s.clone(),
            _ => RegionStructure::new(id, f.region_layers_of(r)),
        };
        let mut materials = details_view::material_names();
        for l in &structure.layers {
            if !materials.contains(&l.material) {
                materials.push(l.material.clone());
            }
        }
        Some(Self {
            frame: SpecDialog::new("Material Layers Definition", "material_layers"),
            form: Form {
                region: id,
                structure,
                cut_finish_layers: r.cut_finish_layers,
                wall: !r.is_floor(),
                selected: 0,
                materials,
            },
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn structure(&self) -> &RegionStructure {
        &self.form.structure
    }

    pub fn structure_mut(&mut self) -> &mut RegionStructure {
        &mut self.form.structure
    }

    pub fn set_cut_finish_layers(&mut self, on: bool) {
        self.form.cut_finish_layers = on;
    }

    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        apply(cx, &self.form.structure, self.form.cut_finish_layers)
    }
}

/// Stores the layer table (and the cut switch) of its region: one undo
/// step, nothing when neither changed.
pub fn apply(cx: &mut EditorContext, s: &RegionStructure, cut: bool) -> bool {
    let fl = cx.floor;
    let details = DetailsLayer::load(cx.floor());
    let Some(region) = details.regions.iter().find(|r| r.id == s.region).cloned() else {
        return false;
    };
    let old = {
        let f = cx.floor();
        match f.region_structure(s.region) {
            Some(o) if !o.layers.is_empty() => o.clone(),
            _ => RegionStructure::new(s.region, f.region_layers_of(&region)),
        }
    };
    if old == *s && region.cut_finish_layers == cut {
        return false;
    }
    cx.begin_change("Material Layers Definition");
    if region.cut_finish_layers != cut {
        let mut layer = details_view::load(cx);
        if let Some(r) = layer.regions.iter_mut().find(|r| r.id == s.region) {
            r.cut_finish_layers = cut;
        }
        details_view::save(&mut cx.project, fl, &layer);
    }
    cx.project.floors[fl].set_region_structure(s.clone());
    cx.mark_dirty();
    true
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        self.structure.error()
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.layers(ui),
            1 => {
                section(ui, "Label");
                row(ui, "Label text", |ui| {
                    ui.text_edit_singleline(&mut self.structure.label)
                });
                ui.weak("Blank is the automatic label (none).");
            }
            _ => {
                section(ui, "Options");
                ui.add_enabled(
                    !self.wall,
                    egui::Checkbox::new(&mut self.cut_finish_layers, "Cut Finish Layers"),
                )
                .on_hover_text("The region replaces the finish layers it covers");
                ui.checkbox(
                    &mut self.structure.auto_detail_insulation,
                    "Auto Detail as Insulation",
                );
            }
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let layers: Vec<(&str, f64)> = self
            .structure
            .layers
            .iter()
            .map(|l| (l.material.as_str(), l.thickness))
            .collect();
        super::layer_stack(painter, rect, &layers);
    }
}

impl Form {
    fn layers(&mut self, ui: &mut Ui) {
        section(ui, "Material Layers (top to bottom)");
        let n = self.structure.layers.len();
        self.selected = self.selected.min(n.saturating_sub(1));
        egui::Grid::new("material_layers_grid")
            .num_columns(5)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("");
                ui.strong("Material");
                ui.strong("Fill");
                ui.strong("Role");
                ui.strong("Thickness");
                ui.end_row();
                for i in 0..n {
                    let layer = &mut self.structure.layers[i];
                    if ui
                        .selectable_label(self.selected == i, format!("{}", i + 1))
                        .clicked()
                    {
                        self.selected = i;
                    }
                    egui::ComboBox::from_id_salt(("layer_material", i))
                        .selected_text(layer.material.clone())
                        .show_ui(ui, |ui| {
                            for m in &self.materials {
                                ui.selectable_value(&mut layer.material, m.clone(), m);
                            }
                        });
                    ui.add(egui::TextEdit::singleline(&mut layer.fill).desired_width(90.0));
                    egui::ComboBox::from_id_salt(("layer_role", i))
                        .selected_text(layer.role.name())
                        .show_ui(ui, |ui| {
                            for r in LayerRole::ALL {
                                ui.selectable_value(&mut layer.role, r, r.name());
                            }
                        });
                    ui.add(
                        egui::DragValue::new(&mut layer.thickness)
                            .speed(0.0625)
                            .range(0.0..=240.0)
                            .suffix(" in"),
                    );
                    ui.end_row();
                }
            });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            let i = self.selected;
            if ui.button("Insert Above").clicked() {
                self.selected = self.structure.insert_above(i);
            }
            if ui.button("Insert Below").clicked() {
                self.selected = self.structure.insert_below(i);
            }
            if ui.button("Move Up").clicked() {
                self.selected = self.structure.move_up(i);
            }
            if ui.button("Move Down").clicked() {
                self.selected = self.structure.move_down(i);
            }
            if ui
                .add_enabled(n > 1, egui::Button::new("Delete"))
                .on_hover_text("A region keeps at least one layer")
                .clicked()
            {
                self.selected = self.structure.delete(i);
            }
        });
        ui.add_space(4.0);
        ui.label(format!(
            "Total thickness {}",
            plan_core::units::fmt_ft_in(self.structure.total_thickness())
        ));
    }
}

thread_local! {
    static DIALOG: RefCell<Option<MaterialLayersDialog>> = const { RefCell::new(None) };
}

/// Opens the window for region `id`.
pub fn open(cx: &EditorContext, id: Id) {
    if let Some(d) = MaterialLayersDialog::new(cx, id) {
        DIALOG.with(|slot| *slot.borrow_mut() = Some(d));
    }
}

pub fn is_open() -> bool {
    DIALOG.with(|d| d.borrow().is_some())
}

pub fn close() {
    DIALOG.with(|d| *d.borrow_mut() = None);
}

/// Draws the window when open and applies an OK.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => DIALOG.with(|slot| *slot.borrow_mut() = Some(d)),
        Outcome::Ok => {
            d.apply(cx);
        }
        Outcome::Cancel => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::material_region::MaterialLayer;

    fn cx_with_region() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = details_view::add_floor_region(
            &mut cx,
            vec![
                Point::new(0.0, 0.0),
                Point::new(96.0, 0.0),
                Point::new(96.0, 96.0),
                Point::new(0.0, 96.0),
            ],
        );
        (cx, id)
    }

    #[test]
    fn the_table_starts_from_the_region_and_edits_like_chiefs() {
        let (cx, id) = cx_with_region();
        let mut d = MaterialLayersDialog::new(&cx, id).unwrap();
        assert_eq!(d.structure().layers.len(), 1);
        let s = d.structure_mut();
        let i = s.insert_below(0);
        assert_eq!(i, 1);
        s.layers[1] = MaterialLayer::new("Concrete", 2.0);
        let i = s.insert_below(1);
        s.layers[i].role = LayerRole::AirGap;
        s.layers[i].thickness = 1.0;
        assert_eq!(s.move_up(2), 1);
        assert_eq!(s.layers[1].role, LayerRole::AirGap);
        assert_eq!(s.delete(1), 1);
        assert_eq!(s.layers.len(), 2);
    }

    #[test]
    fn ok_stores_the_structure_and_the_region_follows_it() {
        let (mut cx, id) = cx_with_region();
        let mut d = MaterialLayersDialog::new(&cx, id).unwrap();
        assert!(!d.apply(&mut cx), "unchanged: no undo step");
        let s = d.structure_mut();
        s.layers[0] = MaterialLayer::new("Oak Flooring", 0.75);
        s.layers.push(MaterialLayer::new("Plywood", 0.5));
        s.label = "Entry".into();
        d.set_cut_finish_layers(true);
        assert!(d.apply(&mut cx));
        assert_eq!(cx.undo_label(), Some("Material Layers Definition"));
        let f = cx.floor();
        assert_eq!(f.region_structure(id).unwrap().layers.len(), 2);
        let r = DetailsLayer::load(f).regions[0].clone();
        assert_eq!(r.material, "Oak Flooring");
        assert!((r.thickness - 1.25).abs() < 1e-9);
        assert!(r.cut_finish_layers);
        cx.undo();
        assert!(cx.floor().region_structure(id).is_none());
        assert!(!DetailsLayer::load(cx.floor()).regions[0].cut_finish_layers);
    }

    #[test]
    fn an_empty_or_zero_thickness_table_blocks_ok() {
        let (cx, id) = cx_with_region();
        let mut d = MaterialLayersDialog::new(&cx, id).unwrap();
        d.structure_mut().layers[0].thickness = 0.0;
        assert!(d.form.error().is_some());
        d.structure_mut().layers.clear();
        assert!(d.form.error().is_some());
    }

    #[test]
    fn deleting_the_region_forgets_its_layers() {
        let (mut cx, id) = cx_with_region();
        let mut d = MaterialLayersDialog::new(&cx, id).unwrap();
        d.structure_mut()
            .layers
            .push(MaterialLayer::new("Plywood", 0.5));
        d.apply(&mut cx);
        cx.selection.items = vec![crate::editor::ObjectRef::Detail(id)];
        cx.delete_selection();
        assert!(cx.floor().region_structure(id).is_none());
    }
}
