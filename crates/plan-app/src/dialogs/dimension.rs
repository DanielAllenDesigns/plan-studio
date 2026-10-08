//! Dimension Specification (DIM-31, `docs/chief-x18-dialogs.md` "Dimension
//! Defaults"). Tabs follow Chief's dimension dialog: General, Primary Format,
//! Arrow, Text Style, Layer and Label.
//!
//! The model keeps a dimension as two measured points, a line offset, a kind
//! and an optional text override, so those are the editable controls: the
//! points and offset on General (with the objects each point is located on
//! and whether its extension line shows), the layer (Manual or Automatic) on
//! Layer, the value text on Label and the text style on Text Style (a
//! character-height or printed-size style, DIM-7). The format and arrow of a
//! dimension come from the Dimension Defaults in force and are shown disabled.
//!
//! Use [`open_for`] to build the dialog for an `ObjectRef::Dimension` (the
//! shell's `EditorRequest::OpenSpec` handler), call [`DimensionDialog::show`]
//! each frame and [`DimensionDialog::apply`] on `Outcome::Ok`.

#![allow(dead_code)]

use super::{
    dis_check, dis_combo, dis_radio, fmt_short, on, pv_text, row, section, Fields, Outcome,
    SpecDialog, SpecPages, Tab, PV_ACCENT, PV_FAINT, PV_INK,
};
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::{AutoGroup, DimFormat, Dimension, DimensionKind, Id, PlanDefaults};

const DIM_TABS: &[Tab] = &[
    on("General"),
    on("Primary Format"),
    on("Arrow"),
    on("Text Style"),
    on("Layer"),
    on("Label"),
];

const MANUAL_LAYER: &str = "Dimensions, Manual";
const AUTO_LAYER: &str = "Dimensions, Automatic";

pub struct DimensionDialog {
    frame: SpecDialog,
    form: DimForm,
}

struct DimForm {
    orig: Dimension,
    draft: Dimension,
    fmt: DimFormat,
    arrow_size: f64,
    ext_gap: f64,
    ext_past: f64,
    text_height: f64,
    text_above: bool,
    font: String,
    /// The dimension set's text style name, and every style that can be
    /// chosen for this dimension.
    set_style: String,
    style_names: Vec<String>,
    /// The sheet's paper scale (inches per foot) the sizes are shown at.
    ipf: f64,
    printed_set: bool,
    styles: plan_core::TextStyles,
    use_override: bool,
    override_text: String,
    fields: Fields,
}

impl DimensionDialog {
    pub fn new(dim: Dimension, defaults: &PlanDefaults) -> Self {
        let (use_override, override_text) = match &dim.text_override {
            Some(t) => (true, t.clone()),
            None => (false, String::new()),
        };
        Self {
            frame: SpecDialog::new("Dimension Specification", "dimension"),
            form: DimForm {
                orig: dim.clone(),
                draft: dim,
                fmt: defaults.dim_format(),
                arrow_size: defaults.dimensions.arrow_size,
                ext_gap: defaults.dimensions.extension_gap,
                ext_past: defaults.dimensions.extension_past,
                text_height: defaults.text.height,
                text_above: defaults.dimensions.text_above_line,
                font: defaults.text.font.clone(),
                set_style: defaults.dimensions.text_style.clone(),
                style_names: defaults
                    .text_styles
                    .names()
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                ipf: 0.25,
                printed_set: defaults.dimensions.printed_size,
                styles: defaults.text_styles.clone(),
                use_override,
                override_text,
                fields: Fields::default(),
            },
        }
    }

    /// The text styles of the plan and the sheet scale (inches per foot)
    /// the sizes are read at.
    pub fn with_styles(mut self, styles: &plan_core::TextStyles, ipf: f64) -> Self {
        self.form.style_names = styles.names().into_iter().map(str::to_string).collect();
        self.form.styles = styles.clone();
        self.form.ipf = ipf;
        self
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn id(&self) -> Id {
        self.form.orig.id
    }

    /// The dimension as edited, with the text override applied.
    pub fn draft(&self) -> Dimension {
        self.form.result()
    }

    /// Test access: the draft the form edits, so a scenario can change a
    /// value and press OK without typing into the egui widgets.
    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Dimension {
        &mut self.form.draft
    }

    /// Stores the edited dimension (one undo step). Editing the points or
    /// offset of an automatic dimension makes it manual (DIM-33). Returns
    /// false when nothing changed or the dimension is gone.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        let new = self.form.result();
        if new == self.form.orig {
            return false;
        }
        let fl = cx.floor;
        if !cx.project.floors[fl]
            .dimensions
            .iter()
            .any(|d| d.id == new.id)
        {
            return false;
        }
        cx.begin_change("Change Dimension");
        if let Some(slot) = cx.project.floors[fl]
            .dimensions
            .iter_mut()
            .find(|d| d.id == new.id)
        {
            *slot = new;
        }
        cx.mark_dirty();
        true
    }
}

/// The Dimension Specification for `o`, if it is a dimension of the current
/// floor.
pub fn open_for(cx: &EditorContext, o: ObjectRef) -> Option<DimensionDialog> {
    let ObjectRef::Dimension(id) = o else {
        return None;
    };
    let d = cx.floor().dimensions.iter().find(|d| d.id == id)?.clone();
    Some(
        DimensionDialog::new(d, &cx.defaults)
            .with_styles(&cx.project.text_styles, cx.sheet.scale.inches_per_foot()),
    )
}

impl DimForm {
    /// The draft with the override applied and, if the geometry changed on
    /// an automatic dimension, the kind switched to manual.
    fn result(&self) -> Dimension {
        let mut d = self.draft.clone();
        d.text_override = (self.use_override && !self.override_text.is_empty())
            .then(|| self.override_text.clone());
        let moved =
            d.start != self.orig.start || d.end != self.orig.end || d.offset != self.orig.offset;
        if moved && d.kind == DimensionKind::AutoExterior && self.orig.kind == d.kind {
            d.kind = DimensionKind::Manual;
        }
        // A manual dimension belongs to no automatic run.
        if d.kind == DimensionKind::Manual {
            d.auto_group = AutoGroup::None;
        }
        d
    }

    fn point_rows(&mut self, ui: &mut Ui) {
        let f = &mut self.fields;
        section(ui, "Measured Points");
        f.length_row(ui, "Start X", "start_x", &mut self.draft.start.x);
        f.length_row(ui, "Start Y", "start_y", &mut self.draft.start.y);
        f.length_row(ui, "End X", "end_x", &mut self.draft.end.x);
        f.length_row(ui, "End Y", "end_y", &mut self.draft.end.y);
    }

    /// The objects each measured point is tied to (DIM-3) and whether its
    /// extension line is drawn.
    fn located_rows(&mut self, ui: &mut Ui) {
        section(ui, "Located Objects");
        for (k, name) in ["Start", "End"].into_iter().enumerate() {
            let what = match self.draft.anchors[k] {
                Some(a) => a.describe(),
                None => "Free point".to_string(),
            };
            row(ui, name, |ui| {
                ui.label(what);
            });
            let mut show = !self.draft.hide_ext[k];
            if ui
                .checkbox(&mut show, format!("Show Extension Line at {name}"))
                .changed()
            {
                self.draft.hide_ext[k] = !show;
            }
        }
        match self.draft.auto_group {
            AutoGroup::Exterior => ui.weak("Automatic exterior dimension"),
            AutoGroup::Interior => ui.weak("Automatic interior dimension"),
            _ => ui.weak("Located points follow their objects when they move"),
        };
    }

    fn general(&mut self, ui: &mut Ui) {
        section(ui, "Dimension");
        row(ui, "Type", |ui| {
            ui.label(match self.draft.kind {
                DimensionKind::Manual => "Manual Dimension",
                DimensionKind::AutoExterior => "Automatic Dimension",
                DimensionKind::Temporary => "Temporary Dimension",
            });
        });
        row(ui, "Value", |ui| {
            ui.label(self.fmt.fmt_len(self.draft.length()));
        });
        self.fields.length_row(
            ui,
            "Offset From Measured Line",
            "offset",
            &mut self.draft.offset,
        );
        self.point_rows(ui);
        self.located_rows(ui);
        if self.draft.length() < 0.5 {
            ui.colored_label(super::ERROR_RED, "The measured points are too close");
        }
    }

    fn primary_format(&mut self, ui: &mut Ui) {
        section(ui, "Format (from the active Dimension Defaults)");
        row(ui, "Units", |ui| {
            dis_combo(ui, "dim_units", "' \u{2013} \"")
        });
        dis_check(ui, "Show Unit Indicators", self.fmt.unit_indicators);
        section(ui, "Accuracy");
        row(ui, "Smallest Fraction", |ui| {
            ui.label(format!("1/{}", self.fmt.smallest_fraction));
        });
        dis_check(ui, "Show Denominator", true);
        dis_check(ui, "Reduce Fractions", true);
    }

    fn arrow(&mut self, ui: &mut Ui) {
        section(ui, "Arrow");
        row(ui, "Style", |ui| dis_combo(ui, "dim_arrow", "Tick"));
        row(ui, "Size", |ui| {
            ui.label(fmt_short(self.arrow_size));
        });
        section(ui, "Extension Lines");
        row(ui, "Gap From Marked Object", |ui| {
            ui.label(fmt_short(self.ext_gap));
        });
        row(ui, "Length Past Dimension Line", |ui| {
            ui.label(fmt_short(self.ext_past));
        });
    }

    /// The style this dimension's number is set in: its own, else the
    /// dimension set's, else "Dimension Text Style".
    fn style_name(&self) -> String {
        self.draft
            .text_style
            .clone()
            .filter(|n| !n.is_empty())
            .or(Some(self.set_style.clone()).filter(|n| !n.is_empty()))
            .unwrap_or_else(|| "Dimension Text Style".to_string())
    }

    fn text_style(&mut self, ui: &mut Ui) {
        section(ui, "Text Style");
        let current = self
            .draft
            .text_style
            .clone()
            .unwrap_or_else(|| "From Dimension Defaults".to_string());
        row(ui, "Style", |ui| {
            egui::ComboBox::from_id_salt("dim_text_style")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.draft.text_style,
                        None,
                        "From Dimension Defaults",
                    );
                    for n in &self.style_names {
                        ui.selectable_value(&mut self.draft.text_style, Some(n.clone()), n);
                    }
                });
        });
        let name = self.style_name();
        if let Some(st) = self.styles.resolve(&name) {
            row(ui, "Font", |ui| dis_combo(ui, "dim_font", &st.font));
            let printed = st.is_printed_size() || self.printed_set;
            row(ui, "Size", |ui| {
                ui.label(if printed {
                    "Printed Size"
                } else {
                    "Character Height"
                });
            });
            let plan_h = st.plan_height_at(self.ipf, self.printed_set);
            row(ui, "Printed", |ui| {
                // Inches on paper at the sheet's scale.
                let on_paper = plan_h * self.ipf / 12.0;
                ui.label(format!("{on_paper:.3}\" ({:.1} pt)", on_paper * 72.0));
            });
            row(ui, "In the Plan", |ui| {
                ui.label(fmt_short(plan_h));
            });
        } else {
            row(ui, "Font", |ui| dis_combo(ui, "dim_font", &self.font));
            row(ui, "Height", |ui| {
                ui.label(fmt_short(self.text_height));
            });
        }
        section(ui, "Position");
        dis_radio(ui, "Centered On Dimension Line", !self.text_above);
        dis_radio(ui, "Above Dimension Line", self.text_above);
        dis_radio(ui, "Below Dimension Line", false);
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        let mut layer = match self.draft.kind {
            DimensionKind::AutoExterior => AUTO_LAYER,
            _ => MANUAL_LAYER,
        };
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("dim_layer")
                .selected_text(layer)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut layer, MANUAL_LAYER, MANUAL_LAYER);
                    ui.selectable_value(&mut layer, AUTO_LAYER, AUTO_LAYER);
                });
        });
        self.draft.kind = if layer == AUTO_LAYER {
            DimensionKind::AutoExterior
        } else {
            DimensionKind::Manual
        };
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Value Text");
        ui.checkbox(&mut self.use_override, "Specify the dimension text");
        ui.add_enabled(
            self.use_override,
            egui::TextEdit::singleline(&mut self.override_text).desired_width(260.0),
        );
        ui.weak(format!(
            "Measured value: {}",
            self.fmt.fmt_len(self.draft.length())
        ));
    }
}

impl SpecPages for DimForm {
    fn tabs(&self) -> &'static [Tab] {
        DIM_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.draft.length() < 0.5 {
            return Some("The measured points are too close".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match DIM_TABS[tab].name {
            "General" => self.general(ui),
            "Primary Format" => self.primary_format(ui),
            "Arrow" => self.arrow(ui),
            "Text Style" => self.text_style(ui),
            "Layer" => self.layer(ui),
            "Label" => self.label(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let y = rect.center().y;
        let (x0, x1) = (rect.min.x + 14.0, rect.max.x - 14.0);
        let ink = Stroke::new(1.2_f32, PV_INK);
        let faint = Stroke::new(0.8_f32, PV_FAINT);
        // The two measured objects, extension lines and the dimension line.
        for x in [x0, x1] {
            p.line_segment([Pos2::new(x, y + 38.0), Pos2::new(x, y - 22.0)], faint);
            p.line_segment([Pos2::new(x, y + 14.0), Pos2::new(x, y - 14.0)], ink);
        }
        p.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], ink);
        for x in [x0, x1] {
            p.line_segment(
                [Pos2::new(x - 4.0, y + 4.0), Pos2::new(x + 4.0, y - 4.0)],
                ink,
            );
        }
        let text = self
            .draft_text()
            .unwrap_or_else(|| self.fmt.fmt_len(self.draft.length()));
        pv_text(
            p,
            Pos2::new(rect.center().x, y - 10.0),
            Align2::CENTER_CENTER,
            text,
            12.0,
        );
        pv_text(
            p,
            Pos2::new(rect.center().x, y + 52.0),
            Align2::CENTER_CENTER,
            match self.draft.kind {
                DimensionKind::AutoExterior => "Automatic",
                _ => "Manual",
            },
            11.0,
        );
        p.circle_filled(Pos2::new(x0, y + 38.0), 2.5, PV_ACCENT);
        p.circle_filled(Pos2::new(x1, y + 38.0), 2.5, PV_ACCENT);
    }
}

impl DimForm {
    fn draft_text(&self) -> Option<String> {
        (self.use_override && !self.override_text.is_empty()).then(|| self.override_text.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;

    fn cx_with_dim(kind: DimensionKind) -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_dimension(
            0,
            Dimension::new(0, kind, Point::ZERO, Point::new(120.0, 0.0), 24.0),
        );
        (cx, id)
    }

    #[test]
    fn opens_only_for_dimensions() {
        let (cx, id) = cx_with_dim(DimensionKind::Manual);
        assert!(open_for(&cx, ObjectRef::Dimension(id)).is_some());
        assert!(open_for(&cx, ObjectRef::Dimension(id + 100)).is_none());
        assert!(open_for(&cx, ObjectRef::Wall(id)).is_none());
    }

    #[test]
    fn text_override_and_offset_are_applied_as_one_undo_step() {
        let (mut cx, id) = cx_with_dim(DimensionKind::Manual);
        let mut d = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
        assert!(!d.apply(&mut cx), "nothing changed");
        d.form.use_override = true;
        d.form.override_text = "EQ".into();
        d.form.draft.offset = 36.0;
        assert!(d.apply(&mut cx));
        let got = &cx.floor().dimensions[0];
        assert_eq!(got.text_override.as_deref(), Some("EQ"));
        assert_eq!(got.offset, 36.0);
        assert_eq!(cx.undo().as_deref(), Some("Change Dimension"));
        assert_eq!(cx.floor().dimensions[0].text_override, None);
        // Clearing the override.
        d.form.use_override = false;
        assert_eq!(d.draft().text_override, None);
    }

    #[test]
    fn editing_an_automatic_dimension_makes_it_manual() {
        let (mut cx, id) = cx_with_dim(DimensionKind::AutoExterior);
        let mut d = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
        d.form.draft.end = Point::new(150.0, 0.0);
        assert!(d.apply(&mut cx));
        assert_eq!(cx.floor().dimensions[0].kind, DimensionKind::Manual);
        // Only the text changed: it stays automatic.
        let (mut cx, id) = cx_with_dim(DimensionKind::AutoExterior);
        let mut d = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
        d.form.use_override = true;
        d.form.override_text = "X".into();
        d.apply(&mut cx);
        assert_eq!(cx.floor().dimensions[0].kind, DimensionKind::AutoExterior);
    }

    #[test]
    fn dialog_draws_every_tab_without_panicking() {
        let (cx, id) = cx_with_dim(DimensionKind::Manual);
        let mut d = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
        let ctx = egui::Context::default();
        for tab in 0..DIM_TABS.len() {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
                d.show(ctx);
            });
        }
        assert!(d.form.error().is_none());
        d.form.draft.end = d.form.draft.start;
        assert!(d.form.error().is_some());
    }

    #[test]
    fn shows_the_located_objects_and_toggles_extension_lines_per_point() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let south = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            plan_core::WallKind::Exterior,
        );
        let id = cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(0.0, -3.0),
                Point::new(500.0, -3.0),
                24.0,
            ),
        );
        assert_eq!(cx.project.floors[0].attach_dimension(id), 1);
        let mut d = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
        let a = d.draft().anchors;
        assert_eq!(a[0].map(|x| x.wall), Some(south));
        assert!(a[0].unwrap().describe().starts_with("Wall"));
        assert!(a[1].is_none());
        // The extension line at the start is switched off: a change that is
        // not a geometry edit, so the kind stays.
        d.form.draft.hide_ext = [true, false];
        assert!(d.apply(&mut cx));
        let got = &cx.floor().dimensions[0];
        assert_eq!(got.hide_ext, [true, false]);
        assert_eq!(got.visible_extension_lines().len(), 1);
        assert_eq!(got.kind, DimensionKind::Manual);
        assert_eq!(cx.undo().as_deref(), Some("Change Dimension"));
        assert_eq!(cx.floor().dimensions[0].hide_ext, [false, false]);
    }

    #[test]
    fn the_text_style_tab_names_the_size_on_paper() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::ZERO,
                Point::new(120.0, 0.0),
                24.0,
            ),
        );
        cx.sheet.scale = plan_docs::Scale::EighthInch;
        let mut d = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
        // Dimension Text Style is 4.5" tall in the plan: 3/64" on paper at 1/8".
        assert_eq!(d.form.style_name(), "Dimension Text Style");
        let st = d.form.styles.resolve("Dimension Text Style").unwrap();
        assert!(!st.is_printed_size());
        assert!((st.plan_height_at(d.form.ipf, false) * d.form.ipf / 12.0 - 0.046875).abs() < 1e-9);
        // Choosing another style is stored on the dimension.
        d.form.draft.text_style = Some("Schedule Style".into());
        assert_eq!(d.form.style_name(), "Schedule Style");
        assert!(d.apply(&mut cx));
        assert_eq!(
            cx.floor().dimensions[0].text_style.as_deref(),
            Some("Schedule Style")
        );
        // The tab draws for a printed-size style too.
        let i = cx
            .project
            .text_styles
            .styles
            .iter()
            .position(|s| s.name == "Schedule Style")
            .unwrap();
        cx.project.text_styles.styles[i].use_printed_size(true);
        let mut d = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, 3));
        });
    }
}
