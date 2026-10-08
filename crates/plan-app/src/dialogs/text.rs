//! Text Specification (TXT-3, `docs/parity/dimensions-text-cad.md`). Tabs:
//! Text, Text Style, Appearance and Layer.
//!
//! A text object is a `CadItem::Text` (position, text, height, angle) on a
//! layer. Those are editable here, and so are the extras kept with the object
//! (`plan_core::cad::CadAttrs`): the named text style and rich text runs
//! (bold, italic, underline, size scale, color), typed as markup like
//! `<b>bold</b> <size=1.5>big</size>`. The Appearance tab edits the text box
//! (`plan_core::text_box::TextBox`, TXT-16): left/center/right and
//! top/middle/bottom alignment, wrap width, minimum height, border with its
//! margin and weight, and the background fill.
//!
//! [`open_for`] builds the dialog for an `ObjectRef::Cad`/`Text` that holds a
//! text item; call [`TextDialog::show`] each frame and
//! [`TextDialog::apply`] on `Outcome::Ok`.

#![allow(dead_code)]

pub mod manage;

use super::{
    fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_FAINT,
    PV_INK,
};
use crate::editor::selection::cad_by_id;
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::cad::{CadAttrs, CadItem};
use plan_core::text_box::{HAlign, VAlign};
use plan_core::text_styles::{runs_from_markup, runs_plain, runs_to_markup, RichRun};
use plan_core::{CadObject, Id};

/// The wrap width a text gets when Wrap Text is first ticked (plan inches).
const DEFAULT_BOX_WIDTH: f64 = 96.0;

const TEXT_TABS: &[Tab] = &[on("Text"), on("Text Style"), on("Appearance"), on("Layer")];

pub struct TextDialog {
    frame: SpecDialog,
    form: TextForm,
}

struct TextForm {
    orig: CadObject,
    draft: CadObject,
    orig_attrs: CadAttrs,
    attrs: CadAttrs,
    /// The text as typed: the words, or markup when `rich`.
    markup: String,
    rich: bool,
    /// Text style names to pick from.
    styles: Vec<String>,
    layers: Vec<String>,
    font: String,
    fields: Fields,
    /// The plan's text styles, the style of the layer the text is on, and
    /// the sheet's paper scale (inches per foot): the size on paper (TXT-2).
    style_defs: plan_core::TextStyles,
    layer_style: String,
    ipf: f64,
}

impl TextDialog {
    pub fn new(obj: CadObject, layers: Vec<String>, font: String) -> Self {
        Self {
            frame: SpecDialog::new("Text Specification", "text"),
            form: TextForm {
                orig_attrs: CadAttrs::new(obj.id),
                attrs: CadAttrs::new(obj.id),
                markup: match &obj.item {
                    CadItem::Text { text, .. } => text.clone(),
                    _ => String::new(),
                },
                rich: false,
                styles: Vec::new(),
                orig: obj.clone(),
                draft: obj,
                layers,
                font,
                fields: Fields::default(),
                style_defs: plan_core::TextStyles::default(),
                layer_style: String::new(),
                ipf: 0.25,
            },
        }
    }

    /// The extras stored with the text, and the text style names to pick from.
    pub fn with_attrs(mut self, attrs: CadAttrs, styles: Vec<String>) -> Self {
        self.form.rich = !attrs.runs.is_empty();
        if self.form.rich {
            self.form.markup = runs_to_markup(&attrs.runs);
        }
        self.form.orig_attrs = attrs.clone();
        self.form.attrs = attrs;
        self.form.styles = styles;
        self
    }

    /// The text styles, the text style of the object's layer and the sheet
    /// scale in inches per foot, so the dialog can say how big the text is
    /// on paper (a printed-size style holds its size at any scale).
    pub fn with_sizing(
        mut self,
        styles: &plan_core::TextStyles,
        layer_style: &str,
        ipf: f64,
    ) -> Self {
        self.form.style_defs = styles.clone();
        self.form.layer_style = layer_style.to_string();
        self.form.ipf = ipf;
        self
    }

    pub fn attrs(&self) -> &CadAttrs {
        &self.form.attrs
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

    /// Test access: the draft the form edits, so a scenario can change a
    /// value and press OK without typing into the egui widgets.
    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut CadObject {
        &mut self.form.draft
    }

    /// Stores the edited text and its extras (one undo step). Returns false
    /// when nothing changed, the object is gone or its layer is locked.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        let new = &self.form.draft;
        let attrs_changed = self.form.attrs != self.form.orig_attrs;
        if *new == self.form.orig && !attrs_changed {
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
        if attrs_changed {
            let mut a = self.form.attrs.clone();
            a.target = new.id;
            cx.project.set_cad_attrs(fl, a);
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
    let styles = cx
        .project
        .text_styles
        .names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let layer_style = cx
        .layers()
        .get(&obj.layer)
        .map(|l| l.text_style.clone())
        .unwrap_or_default();
    Some(
        TextDialog::new(obj.clone(), layers, cx.defaults.text.font.clone())
            .with_attrs(
                cx.floor()
                    .cad_attrs(id)
                    .unwrap_or_else(|| CadAttrs::new(id)),
                styles,
            )
            .with_sizing(
                &cx.project.text_styles,
                &layer_style,
                cx.sheet.scale.inches_per_foot(),
            ),
    )
}

/// The runs worth keeping: none when no run carries a format.
fn normalized(runs: Vec<RichRun>) -> Vec<RichRun> {
    let formatted = runs.iter().any(|r| {
        r.bold || r.italic || r.underline || (r.scale - 1.0).abs() > 1e-9 || r.color.is_some()
    });
    if formatted {
        runs
    } else {
        Vec::new()
    }
}

impl TextForm {
    /// Keeps the item's words and the runs in step with what was typed.
    fn sync_text(&mut self) {
        let CadItem::Text { text, .. } = &mut self.draft.item else {
            return;
        };
        if self.rich {
            let runs = runs_from_markup(&self.markup);
            *text = runs_plain(&runs);
            self.attrs.runs = normalized(runs);
        } else {
            *text = self.markup.clone();
            self.attrs.runs.clear();
        }
    }

    fn text_page(&mut self, ui: &mut Ui) {
        section(ui, "Text");
        let mut rich = self.rich;
        if ui
            .checkbox(&mut rich, "Rich text (markup: <b> <i> <u> <size=1.5>)")
            .changed()
        {
            if rich {
                self.markup = runs_to_markup(&[RichRun::plain(self.markup.clone())]);
            } else {
                self.markup = runs_plain(&runs_from_markup(&self.markup));
            }
            self.rich = rich;
            self.sync_text();
        }
        // Misspelled words are underlined in red (TXT-21).
        let mut underline = super::spell_check::layouter(self.rich);
        let changed = ui
            .add(
                egui::TextEdit::multiline(&mut self.markup)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .layouter(&mut underline),
            )
            .changed();
        if changed {
            self.sync_text();
        }
        if super::spell_check::local_controls(ui, &mut self.markup, self.rich) {
            self.sync_text();
        }
        let CadItem::Text { pos, angle, .. } = &mut self.draft.item else {
            return;
        };
        let mut deg = angle.to_degrees();
        if self.fields.degrees_row(ui, "Angle", "deg_angle", &mut deg) {
            *angle = deg.to_radians();
        }
        section(ui, "Position (Lower Left)");
        self.fields.length_row(ui, "X", "pos_x", &mut pos.x);
        self.fields.length_row(ui, "Y", "pos_y", &mut pos.y);
    }

    /// Is `get` set on every run?
    fn all_runs(&self, get: impl Fn(&RichRun) -> bool) -> bool {
        !self.attrs.runs.is_empty() && self.attrs.runs.iter().all(get)
    }

    /// Sets a format on the whole text, making it rich if needed.
    fn set_format(&mut self, set: impl Fn(&mut RichRun)) {
        let mut runs = if self.rich {
            runs_from_markup(&self.markup)
        } else {
            vec![RichRun::plain(self.markup.clone())]
        };
        for r in &mut runs {
            set(r);
        }
        let formatted = !normalized(runs.clone()).is_empty();
        self.rich = formatted;
        self.markup = if formatted {
            runs_to_markup(&runs)
        } else {
            runs_plain(&runs)
        };
        self.sync_text();
    }

    fn text_style(&mut self, ui: &mut Ui) {
        section(ui, "Text Style");
        row(ui, "Style", |ui| {
            let shown = self
                .attrs
                .text_style
                .clone()
                .unwrap_or_else(|| "(layer's style)".to_string());
            egui::ComboBox::from_id_salt("text_style_pick")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.attrs.text_style, None, "(layer's style)");
                    for n in &self.styles {
                        ui.selectable_value(
                            &mut self.attrs.text_style,
                            Some(n.clone()),
                            n.as_str(),
                        );
                    }
                });
        });
        row(ui, "Font", |ui| ui.label(self.font.clone()));
        section(ui, "Format");
        for (label, get, set) in [
            (
                "Bold",
                (|r: &RichRun| r.bold) as fn(&RichRun) -> bool,
                (|r: &mut RichRun, on: bool| r.bold = on) as fn(&mut RichRun, bool),
            ),
            ("Italic", |r| r.italic, |r, on| r.italic = on),
            ("Underline", |r| r.underline, |r, on| r.underline = on),
        ] {
            let mut on = self.all_runs(get);
            if ui.checkbox(&mut on, label).changed() {
                self.set_format(|r| set(r, on));
            }
        }
        // The installed face the style's font and these toggles map to.
        let (bold, italic) = (self.all_runs(|r| r.bold), self.all_runs(|r| r.italic));
        crate::fonts::face_preview(ui, &crate::fonts::spec_named(&self.font, bold, italic));
        ui.weak("Mixed formats inside one text are typed as markup on the Text tab.");
    }

    fn appearance(&mut self, ui: &mut Ui) {
        let CadItem::Text { height, .. } = &mut self.draft.item else {
            return;
        };
        section(ui, "Size");
        self.fields.length_row(ui, "Text Height", "height", height);
        // The style the text is set in decides whether that height is a
        // plan height or follows the paper size.
        let name = self
            .attrs
            .text_style
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| self.layer_style.clone());
        match self.style_defs.resolve(&name) {
            Some(st) if st.is_printed_size() => {
                let plan_h = st.text_height(*height, self.ipf);
                let on_paper = plan_h * self.ipf / 12.0;
                ui.weak(format!(
                    "Printed Size style \"{}\": {on_paper:.3}\" on paper at any scale, {} in the plan at this sheet scale.",
                    st.name,
                    fmt_short(plan_h)
                ));
            }
            Some(st) => {
                let on_paper = *height * self.ipf / 12.0;
                ui.weak(format!(
                    "Character Height style \"{}\": plan inches, {on_paper:.3}\" on paper at this sheet scale.",
                    st.name
                ));
            }
            None => {
                ui.weak("Plan inches; the printed size follows the plan scale.");
            }
        }
        self.box_sections(ui);
    }

    /// Alignment, the text box (wrap width, minimum height), border and
    /// background fill (TXT-16).
    fn box_sections(&mut self, ui: &mut Ui) {
        let tb = &mut self.attrs.text_box;
        section(ui, "Alignment");
        ui.horizontal(|ui| {
            for a in HAlign::ALL {
                ui.radio_value(&mut tb.halign, a, a.label());
            }
        });
        ui.horizontal(|ui| {
            for a in VAlign::ALL {
                ui.radio_value(&mut tb.valign, a, a.label());
            }
        });
        ui.weak("Left, center or right of the box; top, middle or bottom when the box is taller than the text.");
        section(ui, "Text Box");
        let mut wrap = tb.width > 0.0;
        if ui.checkbox(&mut wrap, "Wrap Text at Box Width").changed() {
            tb.width = if wrap { DEFAULT_BOX_WIDTH } else { 0.0 };
        }
        if tb.width > 0.0 {
            self.fields
                .length_row(ui, "Box Width", "box_width", &mut tb.width);
        }
        self.fields
            .length_row(ui, "Minimum Box Height", "box_height", &mut tb.height);
        ui.weak("The box grows taller to hold the text; 0 fits the text.");
        section(ui, "Border");
        ui.checkbox(&mut tb.border, "Border");
        if tb.border {
            self.fields
                .length_row(ui, "Margin", "box_margin", &mut tb.margin);
            row(ui, "Line Weight", |ui| {
                let mut w = tb.border_weight;
                if ui
                    .add(
                        egui::DragValue::new(&mut w)
                            .range(0..=200)
                            .suffix(" /100 mm"),
                    )
                    .changed()
                {
                    tb.border_weight = w;
                }
            });
            ui.weak("Weight 0 follows the layer.");
        }
        section(ui, "Background");
        let mut fill = tb.background.is_some();
        if ui.checkbox(&mut fill, "Background Fill").changed() {
            tb.background = fill.then_some(tb.background.unwrap_or([255, 255, 255]));
        }
        if let Some(k) = &mut tb.background {
            row(ui, "Fill Color", |ui| ui.color_edit_button_srgb(k));
        }
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
            "Text Style" => self.text_style(ui),
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

    #[test]
    fn text_style_and_rich_formats_are_stored_with_the_text() {
        let (mut cx, id) = cx_with_text();
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert!(d.form.styles.contains(&"Room Label Style".to_string()));
        d.form.attrs.text_style = Some("Room Label Style".into());
        d.form.set_format(|r| r.bold = true);
        assert!(d.form.rich, "a format makes the text rich");
        assert_eq!(d.form.markup, "<b>Hello</b>");
        d.form.set_format(|r| r.underline = true);
        assert!(d.apply(&mut cx));
        let a = cx.floor().cad_attrs(id).unwrap();
        assert_eq!(a.text_style.as_deref(), Some("Room Label Style"));
        assert_eq!(a.runs.len(), 1);
        assert!(a.runs[0].bold && a.runs[0].underline);
        // The item keeps the plain words.
        assert!(matches!(&cad_by_id(cx.floor(), id).unwrap().item,
            CadItem::Text { text, .. } if text == "Hello"));
        // Reopened it is rich again; typing markup updates the runs.
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert!(d.form.rich);
        d.form.markup = "<i>Hi</i> there".into();
        d.form.sync_text();
        assert!(matches!(&d.form.draft.item, CadItem::Text { text, .. } if text == "Hi there"));
        assert_eq!(
            d.form.attrs.runs,
            vec![RichRun::italic("Hi"), RichRun::plain(" there")]
        );
        assert!(d.apply(&mut cx));
        assert_eq!(cx.floor().cad_attrs(id).unwrap().runs.len(), 2);
        // Turning every format off makes it plain again.
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        d.form.set_format(|r| r.italic = false);
        assert!(!d.form.rich && d.form.attrs.runs.is_empty());
        assert!(d.apply(&mut cx));
        assert!(cx.floor().cad_attrs(id).unwrap().runs.is_empty());
        assert_eq!(cx.undo().as_deref(), Some("Change Text"));
    }

    #[test]
    fn the_text_style_page_draws() {
        let (cx, id) = cx_with_text();
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        let ctx = egui::Context::default();
        let tab = TEXT_TABS
            .iter()
            .position(|t| t.name == "Text Style")
            .unwrap();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
        });
    }
    #[test]
    fn the_appearance_tab_stores_alignment_box_border_and_fill() {
        use plan_core::text_box::TextBox;
        let (mut cx, id) = cx_with_text();
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert!(d.attrs().text_box.is_plain());
        d.form.attrs.text_box = TextBox {
            width: 80.0,
            height: 30.0,
            halign: HAlign::Center,
            valign: VAlign::Middle,
            border: true,
            margin: 2.0,
            border_weight: 35,
            background: Some([240, 230, 200]),
        };
        assert!(d.apply(&mut cx));
        let tb = cx.floor().cad_attrs(id).unwrap().text_box;
        assert_eq!((tb.halign, tb.valign), (HAlign::Center, VAlign::Middle));
        assert!(tb.border && tb.margin == 2.0 && tb.border_weight == 35);
        assert_eq!(tb.background, Some([240, 230, 200]));
        assert_eq!((tb.width, tb.height), (80.0, 30.0));
        // Reopened, the dialog shows what was stored.
        let again = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert_eq!(again.attrs().text_box, tb);
        // The tab draws with every part on, and with them off.
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        let tab = TEXT_TABS
            .iter()
            .position(|t| t.name == "Appearance")
            .unwrap();
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
            });
            d.form.attrs.text_box = TextBox::default();
        }
        // One undo step takes it all back.
        assert_eq!(cx.undo().as_deref(), Some("Change Text"));
        assert!(cx
            .floor()
            .cad_attrs(id)
            .is_none_or(|a| a.text_box.is_plain()));
    }
}
