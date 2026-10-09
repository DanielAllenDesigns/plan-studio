//! Soffit Specification (reference manual "Soffits", pp. 1082 to 1084;
//! parity rows CB-459..CB-461): a rectangular or polygon soffit, which
//! the program stores as a cabinet of kind Soffit. Tabs: General (size,
//! thickness, elevation, the hang-from-ceiling shortcut), Materials, Label
//! and Layer. A soffit works as a crown, a box beam or a drop by its size
//! and elevation; the Materials List takes its surface as
//! `plan_core::material_region::soffit_surface` says.
//!
//! The window edits a copy of the cabinet; OK is one undo step.

use super::{row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::{placed, selection, EditorContext, ObjectRef};
use eframe::egui::{self, Painter, Rect, Ui};
use plan_cabinets::{Cabinet, CabinetKind, MaterialChoice};
use plan_core::units::fmt_ft_in;
use plan_core::Id;
use std::cell::RefCell;

const TABS: &[Tab] = &[
    Tab {
        name: "General",
        enabled: true,
    },
    Tab {
        name: "Materials",
        enabled: true,
    },
    Tab {
        name: "Label",
        enabled: true,
    },
    Tab {
        name: "Layer",
        enabled: true,
    },
];

pub const SOFFIT_SPEC: &str = "soffit.spec";

pub struct SoffitDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: Cabinet,
    original: Cabinet,
    layer: String,
    ceiling: f64,
    fields: Fields,
}

impl SoffitDialog {
    /// The window for soffit `id`, or `None` when it is gone or not a soffit.
    pub fn new(cx: &EditorContext, id: Id) -> Option<Self> {
        let c = placed::cabinet_by_id(cx.floor(), id).filter(|c| c.kind == CabinetKind::Soffit)?;
        let layer = selection::layer_of(cx.floor(), ObjectRef::Cabinet(id)).unwrap_or_default();
        Some(Self {
            frame: SpecDialog::new("Soffit Specification", "soffit_spec"),
            form: Form {
                original: c.clone(),
                draft: c,
                layer,
                ceiling: cx.floor().ceiling_height,
                fields: Fields::default(),
            },
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft_mut(&mut self) -> &mut Cabinet {
        &mut self.form.draft
    }

    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        apply(cx, &self.form.draft)
    }
}

/// Stores the soffit: one undo step, nothing when it did not change.
pub fn apply(cx: &mut EditorContext, draft: &Cabinet) -> bool {
    let Some(old) = placed::cabinet_by_id(cx.floor(), draft.id) else {
        return false;
    };
    let mut new = draft.clone();
    // A polygon soffit is as thick as it is high.
    if let Some(custom) = new.custom.as_mut() {
        custom.thickness = new.height;
    }
    if new == old {
        return false;
    }
    cx.begin_change("Soffit Specification");
    let fl = cx.floor;
    placed::replace_cabinet(&mut cx.project, fl, &new);
    cx.mark_dirty();
    true
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        let d = &self.draft;
        if d.width <= 0.0 || d.depth <= 0.0 || d.height <= 0.0 {
            return Some("The soffit needs a width, a depth and a thickness".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.general(ui),
            1 => {
                section(ui, "Materials");
                row(ui, "Soffit", |ui| {
                    egui::ComboBox::from_id_salt("soffit_material")
                        .selected_text(self.draft.materials.carcass.name())
                        .show_ui(ui, |ui| {
                            for m in MaterialChoice::ALL {
                                ui.selectable_value(&mut self.draft.materials.carcass, m, m.name());
                            }
                        });
                });
            }
            2 => {
                section(ui, "Label");
                row(ui, "Label text", |ui| {
                    ui.text_edit_singleline(&mut self.draft.label)
                });
                ui.weak("Blank is the automatic label.");
            }
            _ => {
                section(ui, "Layer");
                row(ui, "Layer", |ui| ui.label(self.layer.clone()));
            }
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let ink = egui::Color32::from_rgb(0x2B, 0x2B, 0x2B);
        let d = &self.draft;
        // Elevation sketch: the floor line, the ceiling line and the box.
        let h = rect.height() - 20.0;
        let k = (h as f64 / self.ceiling.max(d.elevation + d.height).max(1.0)) as f32;
        let floor_y = rect.bottom() - 10.0;
        painter.hline(rect.x_range(), floor_y, egui::Stroke::new(1.0_f32, ink));
        let ceil_y = floor_y - self.ceiling as f32 * k;
        painter.hline(rect.x_range(), ceil_y, egui::Stroke::new(1.0_f32, ink));
        let top = floor_y - (d.elevation + d.height) as f32 * k;
        let bottom = floor_y - d.elevation as f32 * k;
        let w = (rect.width() * 0.5).max(10.0);
        let r = Rect::from_min_max(
            egui::pos2(rect.center().x - w * 0.5, top),
            egui::pos2(rect.center().x + w * 0.5, bottom),
        );
        painter.rect_filled(r, 0.0, egui::Color32::from_rgb(0xC9, 0xC6, 0xBC));
        painter.rect_stroke(
            r,
            0.0,
            egui::Stroke::new(1.0_f32, ink),
            egui::StrokeKind::Inside,
        );
    }
}

impl Form {
    fn general(&mut self, ui: &mut Ui) {
        let polygon = self.draft.custom.is_some();
        let d = &mut self.draft;
        section(ui, "Size");
        ui.add_enabled_ui(!polygon, |ui| {
            self.fields.length_row(ui, "Width", "width", &mut d.width);
            self.fields.length_row(ui, "Depth", "depth", &mut d.depth);
        });
        if polygon {
            ui.weak("A polygon soffit takes its width and depth from its outline.");
        }
        self.fields
            .length_row(ui, "Thickness (height)", "height", &mut d.height);
        ui.add_space(4.0);
        section(ui, "Elevation");
        self.fields
            .length_row(ui, "Bottom elevation", "elevation", &mut d.elevation);
        row(ui, "Top elevation", |ui| {
            ui.label(fmt_ft_in(d.elevation + d.height))
        });
        ui.horizontal(|ui| {
            if ui
                .button("Hang from ceiling")
                .on_hover_text("Put the top of the soffit at the ceiling height")
                .clicked()
            {
                d.elevation = (self.ceiling - d.height).max(0.0);
                // The field shows its formatted value again.
                self.fields = Fields::default();
            }
            if ui.button("Reset").clicked() {
                *d = self.original.clone();
                self.fields = Fields::default();
            }
        });
    }
}

thread_local! {
    static DIALOG: RefCell<Option<SoffitDialog>> = const { RefCell::new(None) };
}

/// Opens the window for soffit `id`.
pub fn open(cx: &EditorContext, id: Id) {
    if let Some(d) = SoffitDialog::new(cx, id) {
        DIALOG.with(|slot| *slot.borrow_mut() = Some(d));
    }
}

pub fn is_open() -> bool {
    DIALOG.with(|d| d.borrow().is_some())
}

pub fn close() {
    DIALOG.with(|d| *d.borrow_mut() = None);
}

/// The one selected soffit, if the selection is exactly that.
pub fn selected_soffit(cx: &EditorContext) -> Option<Id> {
    let [ObjectRef::Cabinet(id)] = cx.selection.items.as_slice() else {
        return None;
    };
    placed::cabinet_by_id(cx.floor(), *id)
        .filter(|c| c.kind == CabinetKind::Soffit)
        .map(|_| *id)
}

/// The Edit toolbar button for a selected soffit.
pub fn edit_actions(cx: &EditorContext) -> Vec<crate::editor::actions::EditAction> {
    use crate::editor::actions::{EditAction, EditActionKind};
    if selected_soffit(cx).is_none() {
        return Vec::new();
    }
    vec![EditAction {
        kind: EditActionKind::Custom {
            id: SOFFIT_SPEC,
            label: "Soffit Specification",
            icon: "",
        },
        label: "Soffit Specification",
        icon: None,
        enabled: true,
    }]
}

/// Runs the soffit command.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    if id != SOFFIT_SPEC {
        return false;
    }
    match selected_soffit(cx) {
        Some(s) => open(cx, s),
        None => cx.status = "Select one soffit first".into(),
    }
    true
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

    fn cx_with_soffit() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = placed::add_cabinet(&mut cx.project, 0, Cabinet::new(CabinetKind::Soffit, 48.0))
            .unwrap();
        (cx, id)
    }

    #[test]
    fn only_soffits_open_and_an_unchanged_one_stores_nothing() {
        let (mut cx, id) = cx_with_soffit();
        let d = SoffitDialog::new(&cx, id).unwrap();
        assert!(!d.apply(&mut cx));
        let base =
            placed::add_cabinet(&mut cx.project, 0, Cabinet::new(CabinetKind::Base, 24.0)).unwrap();
        assert!(SoffitDialog::new(&cx, base).is_none());
    }

    #[test]
    fn ok_changes_size_elevation_material_and_label_in_one_step() {
        let (mut cx, id) = cx_with_soffit();
        let mut d = SoffitDialog::new(&cx, id).unwrap();
        {
            let c = d.draft_mut();
            c.height = 8.0;
            c.elevation = 90.0;
            c.width = 96.0;
            c.materials.carcass = MaterialChoice::Painted;
            c.label = "S1".into();
        }
        assert!(d.apply(&mut cx));
        assert_eq!(cx.undo_label(), Some("Soffit Specification"));
        let c = placed::cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!((c.height, c.elevation, c.width), (8.0, 90.0, 96.0));
        assert_eq!(c.label, "S1");
        cx.undo();
        assert_eq!(placed::cabinet_by_id(cx.floor(), id).unwrap().width, 48.0);
    }

    #[test]
    fn a_polygon_soffit_keeps_its_thickness_in_step_with_its_height() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let ring = [
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(60.0, 30.0),
            Point::new(30.0, 50.0),
            Point::new(0.0, 30.0),
        ];
        let c = Cabinet::soffit_polygon(&ring, 12.0, 84.0).unwrap();
        let id = placed::add_cabinet(&mut cx.project, 0, c).unwrap();
        let mut d = SoffitDialog::new(&cx, id).unwrap();
        d.draft_mut().height = 6.0;
        assert!(d.apply(&mut cx));
        let c = placed::cabinet_by_id(cx.floor(), id).unwrap();
        assert_eq!(c.custom.as_ref().unwrap().thickness, 6.0);
    }

    #[test]
    fn a_selected_soffit_gets_the_edit_button() {
        let (mut cx, id) = cx_with_soffit();
        cx.selection.items = vec![ObjectRef::Cabinet(id)];
        assert_eq!(selected_soffit(&cx), Some(id));
        assert_eq!(edit_actions(&cx).len(), 1);
        close();
        assert!(run_command(&mut cx, SOFFIT_SPEC));
        assert!(is_open());
        close();
    }
}
