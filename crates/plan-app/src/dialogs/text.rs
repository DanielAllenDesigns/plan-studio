//! Text Specification (TXT-3, `docs/parity/dimensions-text-cad.md`). Tabs:
//! Text, Appearance and Layer.
//!
//! A text object is a `CadItem::Text` (position, text, height, angle) on a
//! layer. Those are editable here. Font, bold/italic/underline, alignment,
//! border and fill have no model fields yet and are shown disabled, as in the
//! other specification dialogs.
//!
//! [`open_for`] builds the dialog for an `ObjectRef::Cad`/`Text` that holds a
//! text item; call [`TextDialog::show`] each frame and
//! [`TextDialog::apply`] on `Outcome::Ok`.

#![allow(dead_code)]

use super::{
    dis_check, dis_combo, dis_radio, fmt_short, on, pv_text, row, section, Fields, Outcome,
    SpecDialog, SpecPages, Tab, PV_FAINT, PV_INK,
};
use crate::editor::selection::cad_by_id;
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::cad::CadItem;
use plan_core::{CadObject, Id};

const TEXT_TABS: &[Tab] = &[on("Text"), on("Appearance"), on("Layer")];

pub struct TextDialog {
    frame: SpecDialog,
    form: TextForm,
}

struct TextForm {
    orig: CadObject,
    draft: CadObject,
    layers: Vec<String>,
    font: String,
    fields: Fields,
}

impl TextDialog {
    pub fn new(obj: CadObject, layers: Vec<String>, font: String) -> Self {
        Self {
            frame: SpecDialog::new("Text Specification", "text"),
            form: TextForm {
                orig: obj.clone(),
                draft: obj,
                layers,
                font,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn id(&self) -> Id {
        self.form.orig.id
    }

    pub fn draft(&self) -> &CadObject {
        &self.form.draft
    }

    /// Stores the edited text (one undo step). Returns false when nothing
    /// changed, the object is gone or its layer is locked.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        let new = &self.form.draft;
        if *new == self.form.orig {
            return false;
        }
        if !cx.check_unlocked(ObjectRef::Cad(new.id)) {
            return false;
        }
        let fl = cx.floor;
        if cad_by_id(cx.floor(), new.id).is_none() {
            return false;
        }
        cx.begin_change("Change Text");
        if let Some(slot) = cx.project.floors[fl]
            .cad
            .iter_mut()
            .find(|c| c.id == new.id)
        {
            *slot = new.clone();
        }
        cx.mark_dirty();
        true
    }
}

/// The Text Specification for `o`, if it is a text object of the current
/// floor.
pub fn open_for(cx: &EditorContext, o: ObjectRef) -> Option<TextDialog> {
    let (ObjectRef::Cad(id) | ObjectRef::Text(id)) = o else {
        return None;
    };
    let obj = cad_by_id(cx.floor(), id)?;
    if !matches!(obj.item, CadItem::Text { .. }) {
        return None;
    }
    let layers = cx.layers().layers.iter().map(|l| l.name.clone()).collect();
    Some(TextDialog::new(
        obj.clone(),
        layers,
        cx.defaults.text.font.clone(),
    ))
}

impl TextForm {
    fn text_page(&mut self, ui: &mut Ui) {
        let CadItem::Text {
            pos, text, angle, ..
        } = &mut self.draft.item
        else {
            return;
        };
        section(ui, "Text");
        ui.add(
            egui::TextEdit::multiline(text)
                .desired_rows(5)
                .desired_width(f32::INFINITY),
        );
        let mut deg = angle.to_degrees();
        if self.fields.degrees_row(ui, "Angle", "deg_angle", &mut deg) {
            *angle = deg.to_radians();
        }
        section(ui, "Position (Lower Left)");
        self.fields.length_row(ui, "X", "pos_x", &mut pos.x);
        self.fields.length_row(ui, "Y", "pos_y", &mut pos.y);
    }

    fn appearance(&mut self, ui: &mut Ui) {
        let CadItem::Text { height, .. } = &mut self.draft.item else {
            return;
        };
        section(ui, "Size");
        self.fields.length_row(ui, "Text Height", "height", height);
        ui.weak("Plan inches; the printed size follows the plan scale.");
        section(ui, "Font");
        row(ui, "Family", |ui| dis_combo(ui, "text_font", &self.font));
        dis_check(ui, "Bold", false);
        dis_check(ui, "Italic", false);
        dis_check(ui, "Underline", false);
        section(ui, "Alignment");
        ui.horizontal(|ui| {
            dis_radio(ui, "Left", true);
            dis_radio(ui, "Center", false);
            dis_radio(ui, "Right", false);
        });
        section(ui, "Box");
        dis_check(ui, "Border", false);
        dis_check(ui, "Background Fill", false);
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("text_layer")
                .selected_text(self.draft.layer.clone())
                .show_ui(ui, |ui| {
                    for name in &self.layers {
                        ui.selectable_value(&mut self.draft.layer, name.clone(), name.as_str());
                    }
                });
        });
    }
}

impl SpecPages for TextForm {
    fn tabs(&self) -> &'static [Tab] {
        TEXT_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        match &self.draft.item {
            CadItem::Text { text, .. } if text.trim().is_empty() => Some("Enter the text".into()),
            CadItem::Text { height, .. } if *height <= 0.0 => {
                Some("The text height must be greater than zero".into())
            }
            _ => None,
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TEXT_TABS[tab].name {
            "Text" => self.text_page(ui),
            "Appearance" => self.appearance(ui),
            "Layer" => self.layer(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let CadItem::Text { text, height, .. } = &self.draft.item else {
            return;
        };
        p.rect_stroke(
            rect.shrink(2.0),
            2.0,
            Stroke::new(0.8_f32, PV_FAINT),
            egui::StrokeKind::Inside,
        );
        let size = (*height as f32 * 2.5).clamp(10.0, 28.0);
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(size),
            PV_INK,
        );
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.max.y - 10.0),
            Align2::CENTER_CENTER,
            format!("Height {}", fmt_short(*height)),
            11.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;

    fn cx_with_text() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_cad(
            0,
            "Text",
            CadItem::Text {
                pos: Point::new(10.0, 10.0),
                text: "Hello".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        (cx, id)
    }

    #[test]
    fn opens_for_text_objects_only() {
        let (mut cx, id) = cx_with_text();
        assert!(open_for(&cx, ObjectRef::Cad(id)).is_some());
        assert!(open_for(&cx, ObjectRef::Text(id)).is_some());
        let line = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(1.0, 0.0),
            },
        );
        assert!(open_for(&cx, ObjectRef::Cad(line)).is_none());
        assert!(open_for(&cx, ObjectRef::Wall(id)).is_none());
    }

    #[test]
    fn edits_text_height_angle_and_layer() {
        let (mut cx, id) = cx_with_text();
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert!(!d.apply(&mut cx));
        if let CadItem::Text {
            text,
            height,
            angle,
            ..
        } = &mut d.form.draft.item
        {
            *text = "Kitchen".into();
            *height = 9.0;
            *angle = 90f64.to_radians();
        }
        d.form.draft.layer = "CAD, Default".into();
        assert!(d.apply(&mut cx));
        let c = cad_by_id(cx.floor(), id).unwrap();
        assert_eq!(c.layer, "CAD, Default");
        assert!(
            matches!(&c.item, CadItem::Text { text, height, .. } if text == "Kitchen" && *height == 9.0)
        );
        assert_eq!(cx.undo().as_deref(), Some("Change Text"));
        assert!(matches!(&cad_by_id(cx.floor(), id).unwrap().item,
            CadItem::Text { text, .. } if text == "Hello"));
    }

    #[test]
    fn empty_text_is_an_error_and_locked_layers_refuse() {
        let (mut cx, id) = cx_with_text();
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert!(d.form.error().is_none());
        if let CadItem::Text { text, .. } = &mut d.form.draft.item {
            text.clear();
        }
        assert!(d.form.error().is_some());
        if let CadItem::Text { text, .. } = &mut d.form.draft.item {
            *text = "x".into();
        }
        cx.project.layers.set_locked("Text", true);
        assert!(!d.apply(&mut cx));
    }

    #[test]
    fn dialog_draws_every_tab_without_panicking() {
        let (cx, id) = cx_with_text();
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        let ctx = egui::Context::default();
        for tab in 0..TEXT_TABS.len() {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
                d.show(ctx);
            });
        }
    }
}
