//! Dimension Specification (DIM-31, `docs/chief-x18-dialogs.md` "Dimension
//! Defaults"). Tabs follow Chief's dimension dialog: General, Primary Format,
//! Arrow, Text Style, Layer and Label.
//!
//! The model keeps a dimension as two measured points, a line offset, a kind
//! and an optional text override, so those are the editable controls: the
//! points and offset on General (with the objects each point is located on
//! and whether its extension line shows), the layer (Manual or Automatic) on
//! Layer, the value text on Label and the text style on Text Style (a
//! character-height or printed-size style, DIM-7). The Primary Format and
//! Arrow tabs follow the Dimension Defaults in force until their "use the
//! defaults" box is unticked; then the dimension keeps its own units,
//! fractions, decimal places, unit indicators and zero-feet suppression, its
//! own arrow style, size and fill, and its own extension line gap, overshoot
//! and fixed length (`plan_core::dimension::DimOverrides`, DIM-31, DIM-39).
//!
//! Use [`open_for`] to build the dialog for an `ObjectRef::Dimension` (the
//! shell's `EditorRequest::OpenSpec` handler), call [`DimensionDialog::show`]
//! each frame and [`DimensionDialog::apply`] on `Outcome::Ok`.

#![allow(dead_code)]

use super::{
    dis_combo, dis_radio, fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog,
    SpecPages, Tab, PV_ACCENT, PV_FAINT, PV_INK,
};
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::dimension::DimArrow;
use plan_core::units::LengthUnit;
use plan_core::{AutoGroup, DimFormat, Dimension, DimensionKind, Id, PlanDefaults};

const DIM_TABS: &[Tab] = &[
    on("General"),
    on("Primary Format"),
    on("Arrow"),
    on("Text Style"),
    on("Layer"),
    on("Label"),
];

/// Units a dimension can be written in.
const UNITS: [LengthUnit; 6] = [
    LengthUnit::FeetInches,
    LengthUnit::Inches,
    LengthUnit::DecimalFeet,
    LengthUnit::Millimeters,
    LengthUnit::Centimeters,
    LengthUnit::Meters,
];
/// Smallest fractions on offer.
const FRACTIONS: [u32; 6] = [2, 4, 8, 16, 32, 64];

fn unit_label(u: LengthUnit) -> &'static str {
    match u {
        LengthUnit::FeetInches => "Feet and Inches",
        LengthUnit::Inches => "Inches",
        LengthUnit::DecimalFeet => "Decimal Feet",
        LengthUnit::Millimeters => "Millimeters",
        LengthUnit::Centimeters => "Centimeters",
        LengthUnit::Meters => "Meters",
    }
}

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
    /// The mark the Dimension Defaults draw.
    arrow_default: DimArrow,
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
                arrow_default: DimArrow::from_name(&defaults.dimensions.arrow_style),
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
        let mut follow = !self.draft.look.has_format();
        if ui
            .checkbox(&mut follow, "Use the Dimension Defaults' format")
            .changed()
        {
            if follow {
                let l = &mut self.draft.look;
                l.units = None;
                l.fraction = None;
                l.decimals = None;
                l.unit_indicators = None;
                l.trailing_zeroes = None;
                l.suppress_zero_feet = None;
            } else {
                // Start from what the defaults show now.
                let base = self.fmt.length.unwrap_or_default();
                let l = &mut self.draft.look;
                l.units = Some(base.unit);
                l.fraction = Some(self.fmt.smallest_fraction.max(1));
                l.decimals = Some(base.decimals);
                l.unit_indicators = Some(self.fmt.unit_indicators);
                l.trailing_zeroes = Some(base.trailing_zeroes);
                l.suppress_zero_feet = Some(false);
            }
        }
        if follow {
            section(ui, "Format (from the active Dimension Defaults)");
            row(ui, "Units", |ui| {
                ui.label(
                    unit_label(self.fmt.length.map_or(LengthUnit::FeetInches, |l| l.unit))
                        .to_string(),
                )
            });
            row(ui, "Smallest Fraction", |ui| {
                ui.label(format!("1/{}", self.fmt.smallest_fraction));
            });
            ui.weak("Untick the box above to set this dimension's own format.");
            return;
        }
        section(ui, "Units");
        let l = &mut self.draft.look;
        let mut unit = l.units.unwrap_or_default();
        row(ui, "Units", |ui| {
            egui::ComboBox::from_id_salt("dim_units")
                .selected_text(unit_label(unit))
                .show_ui(ui, |ui| {
                    for u in UNITS {
                        ui.selectable_value(&mut unit, u, unit_label(u));
                    }
                });
        });
        l.units = Some(unit);
        let imperial = matches!(unit, LengthUnit::FeetInches | LengthUnit::Inches);
        section(ui, "Accuracy");
        if imperial {
            let mut frac = l.fraction.unwrap_or(16);
            row(ui, "Smallest Fraction", |ui| {
                egui::ComboBox::from_id_salt("dim_fraction")
                    .selected_text(format!("1/{frac}"))
                    .show_ui(ui, |ui| {
                        for d in FRACTIONS {
                            ui.selectable_value(&mut frac, d, format!("1/{d}"));
                        }
                    });
            });
            l.fraction = Some(frac);
        } else {
            let mut places = l.decimals.unwrap_or(2);
            row(ui, "Decimal Places", |ui| {
                ui.add(egui::DragValue::new(&mut places).range(0..=6))
            });
            l.decimals = Some(places);
            let mut zeroes = l.trailing_zeroes.unwrap_or(false);
            ui.checkbox(&mut zeroes, "Show Trailing Zeroes");
            l.trailing_zeroes = Some(zeroes);
        }
        section(ui, "Display");
        let mut marks = l.unit_indicators.unwrap_or(true);
        ui.checkbox(&mut marks, "Show Unit Indicators");
        l.unit_indicators = Some(marks);
        if unit == LengthUnit::FeetInches {
            let mut suppress = l.suppress_zero_feet.unwrap_or(false);
            ui.checkbox(&mut suppress, "Suppress Zero Feet (6\" rather than 0'-6\")");
            l.suppress_zero_feet = Some(suppress);
        }
        ui.weak(format!(
            "Reads: {}",
            self.draft.look.format_len(&self.fmt, self.draft.length())
        ));
    }

    fn arrow(&mut self, ui: &mut Ui) {
        let mut follow = self.draft.look.arrow.is_none()
            && self.draft.look.arrow_size.is_none()
            && self.draft.look.arrow_filled.is_none();
        if ui
            .checkbox(&mut follow, "Use the Dimension Defaults' arrows")
            .changed()
        {
            let l = &mut self.draft.look;
            if follow {
                l.arrow = None;
                l.arrow_size = None;
                l.arrow_filled = None;
            } else {
                l.arrow = Some(self.arrow_default);
                l.arrow_size = Some(self.arrow_size);
                l.arrow_filled = Some(true);
            }
        }
        if follow {
            section(ui, "Arrow (from the active Dimension Defaults)");
            row(ui, "Style", |ui| ui.label(self.arrow_default.label()));
            row(ui, "Size", |ui| {
                ui.label(fmt_short(self.arrow_size));
            });
        } else {
            section(ui, "Arrow");
            let l = &mut self.draft.look;
            let mut mark = l.arrow.unwrap_or(self.arrow_default);
            row(ui, "Style", |ui| {
                egui::ComboBox::from_id_salt("dim_arrow")
                    .selected_text(mark.label())
                    .show_ui(ui, |ui| {
                        for a in DimArrow::ALL {
                            ui.selectable_value(&mut mark, a, a.label());
                        }
                    });
            });
            l.arrow = Some(mark);
            let mut size = l.arrow_size.unwrap_or(self.arrow_size);
            if self.fields.length_row(ui, "Size", "arrow_size", &mut size) {
                l.arrow_size = Some(size.max(0.0));
            }
            let mut filled = l.arrow_filled.unwrap_or(true);
            ui.add_enabled(
                matches!(mark, DimArrow::Arrow | DimArrow::Dot),
                egui::Checkbox::new(&mut filled, "Filled"),
            );
            l.arrow_filled = Some(filled);
        }
        self.extension_lines(ui);
    }

    /// Gap, overshoot and fixed length of the extension lines.
    fn extension_lines(&mut self, ui: &mut Ui) {
        section(ui, "Extension Lines");
        let l = &mut self.draft.look;
        let mut follow = l.ext_gap.is_none() && l.ext_past.is_none() && l.ext_length.is_none();
        if ui
            .checkbox(&mut follow, "Use the Dimension Defaults' extension lines")
            .changed()
        {
            if follow {
                l.ext_gap = None;
                l.ext_past = None;
                l.ext_length = None;
            } else {
                l.ext_gap = Some(self.ext_gap);
                l.ext_past = Some(self.ext_past);
            }
        }
        if follow {
            row(ui, "Gap From Marked Object", |ui| {
                ui.label(fmt_short(self.ext_gap));
            });
            row(ui, "Length Past Dimension Line", |ui| {
                ui.label(fmt_short(self.ext_past));
            });
            return;
        }
        let mut gap = l.ext_gap.unwrap_or(self.ext_gap);
        if self
            .fields
            .length_row(ui, "Gap From Marked Object", "ext_gap", &mut gap)
        {
            l.ext_gap = Some(gap.max(0.0));
        }
        let mut past = l.ext_past.unwrap_or(self.ext_past);
        if self
            .fields
            .length_row(ui, "Length Past Dimension Line", "ext_past", &mut past)
        {
            l.ext_past = Some(past.max(0.0));
        }
        let mut fixed = l.ext_length.is_some();
        if ui
            .checkbox(&mut fixed, "Fixed Extension Line Length")
            .changed()
        {
            l.ext_length = fixed.then_some(self.draft.offset.abs().max(12.0));
        }
        if let Some(len) = &mut l.ext_length {
            self.fields
                .length_row(ui, "Length From Dimension Line", "ext_length", len);
            *len = len.max(0.0);
        }
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
    #[test]
    fn the_format_and_arrow_tabs_store_the_dimensions_own_look() {
        use plan_core::units::LengthUnit;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::ZERO,
                Point::new(78.25, 0.0),
                24.0,
            ),
        );
        let fmt = cx.dim_format();
        assert_eq!(cx.floor().dimensions[0].label(&fmt), "6'-6 1/4\"");
        let mut d = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
        assert!(d.form.draft.look.is_default());
        // Format: inches to the eighth, no zero feet.
        d.form.draft.look.units = Some(LengthUnit::FeetInches);
        d.form.draft.look.fraction = Some(2);
        d.form.draft.look.suppress_zero_feet = Some(true);
        // Arrow: a hollow arrow, 5" long; extension lines of its own.
        d.form.draft.look.arrow = Some(DimArrow::Arrow);
        d.form.draft.look.arrow_size = Some(5.0);
        d.form.draft.look.arrow_filled = Some(false);
        d.form.draft.look.ext_gap = Some(1.5);
        d.form.draft.look.ext_past = Some(2.5);
        d.form.draft.look.ext_length = Some(9.0);
        assert!(d.apply(&mut cx));
        let got = &cx.floor().dimensions[0];
        assert_eq!(got.label(&fmt), "6'-6 1/2\"");
        assert_eq!(got.look.arrow, Some(DimArrow::Arrow));
        assert_eq!(got.look.ext_length, Some(9.0));
        // The plan draws with that look.
        let look = crate::editor::render::DimLook::of(&cx, got);
        assert_eq!((look.arrow, look.gap, look.past), (5.0, 1.5, 2.5));
        assert_eq!(look.ext_length, Some(9.0));
        assert!(!look.filled);
        // The tabs draw both ways: following the defaults, and with a look.
        let ctx = egui::Context::default();
        for tab in [1, 2] {
            let mut open = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
            for follow in [false, true] {
                if follow {
                    open.form.draft.look = Default::default();
                }
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| open.form.page(ui, tab));
                });
            }
        }
        // Reopened, the dialog shows what the dimension keeps; one undo step.
        let again = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
        assert_eq!(again.form.draft.look.fraction, Some(2));
        assert_eq!(cx.undo().as_deref(), Some("Change Dimension"));
        assert!(cx.floor().dimensions[0].look.is_default());
    }

    #[test]
    fn decimal_units_show_decimal_places_and_metric_drops_the_feet() {
        use plan_core::units::LengthUnit;
        let (cx, id) = cx_with_dim(DimensionKind::Manual);
        let mut d = open_for(&cx, ObjectRef::Dimension(id)).unwrap();
        d.form.draft.look.units = Some(LengthUnit::Millimeters);
        d.form.draft.look.decimals = Some(0);
        d.form.draft.look.unit_indicators = Some(true);
        let fmt = cx.dim_format();
        assert_eq!(d.draft().label(&fmt), "3048 mm");
        d.form.draft.look.units = Some(LengthUnit::DecimalFeet);
        d.form.draft.look.decimals = Some(1);
        assert_eq!(d.draft().label(&fmt), "10'");
        d.form.draft.look.trailing_zeroes = Some(true);
        assert_eq!(d.draft().label(&fmt), "10.0'");
    }
}
