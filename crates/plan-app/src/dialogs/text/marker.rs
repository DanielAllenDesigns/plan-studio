//! Marker Specification (TXT-52, TXT-53; manual pp. 556 to 558). Panels:
//! Marker, Line Style and Text Style. The Marker panel holds the type
//! (Level Line, Test Boring, Point, Elevation), the line (minimum length and
//! angle), the label above and below the line, and the size (radius and
//! height relative to 0).

use super::annot::{
    draw_preview, insert_menu, line_style_page, lock_check, text_style_page, AnnotDialog, Env,
    Macros, Mode,
};
use crate::dialogs::{on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::EditorContext;
use eframe::egui::{self, Painter, Rect, Ui};
use plan_core::callout::{marker_items, LineAlign, LineText, Marker, MarkerKind, Vars};
use plan_core::Id;

pub const MARKER_TABS: &[Tab] = &[on("Marker"), on("Line Style"), on("Text Style")];

pub struct MarkerDialog {
    frame: SpecDialog,
    form: MarkerForm,
}

struct MarkerForm {
    spec: Marker,
    orig: Marker,
    mode: Mode,
    env: Env,
    fields: Fields,
}

impl MarkerDialog {
    pub fn new(spec: Marker, mode: Mode, env: Env) -> Self {
        let title = super::annot::title(super::annot::AnnotKind::Marker, mode, &env.saved_name);
        Self {
            frame: SpecDialog::new(title, "marker"),
            form: MarkerForm {
                orig: spec.clone(),
                spec,
                mode,
                env,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn mode(&self) -> Mode {
        self.form.mode
    }

    pub fn title(&self) -> String {
        super::annot::title(
            super::annot::AnnotKind::Marker,
            self.form.mode,
            &self.form.env.saved_name,
        )
    }

    pub fn tab_names(&self) -> Vec<&'static str> {
        MARKER_TABS.iter().map(|t| t.name).collect()
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Marker {
        &mut self.form.spec
    }

    pub fn draft(&self) -> &Marker {
        &self.form.spec
    }

    pub fn apply(&self, cx: &mut EditorContext) -> Option<Id> {
        let mut spec = self.form.spec.clone();
        let at = spec.center;
        spec.label.text = crate::tools::text::TextTool::expand(cx, &spec.label.text, at);
        spec.below.text = crate::tools::text::TextTool::expand(cx, &spec.below.text, at);
        let fl = cx.floor;
        match self.form.mode {
            Mode::New => {
                if cx.layers().is_locked(&spec.layer) {
                    cx.status = format!("The layer \"{}\" is locked", spec.layer);
                    return None;
                }
                cx.begin_change("Place Marker");
                let id = cx.project.add_marker(fl, spec);
                AnnotDialog::select(cx, id);
                cx.mark_dirty();
                Some(id)
            }
            Mode::Edit(id) => {
                if self.form.spec == self.form.orig || !lock_check(cx, id) {
                    return None;
                }
                let idx = match cx.floor().annot_of(id) {
                    Some(plan_core::callout::AnnotRef::Marker(i)) => i,
                    _ => return None,
                };
                cx.begin_change("Change Marker");
                let cur = cx.project.floors[fl].annots.markers[idx].clone();
                cx.project.floors[fl].annots.markers[idx] = Marker {
                    items: cur.items,
                    pose_idx: cur.pose_idx,
                    pose: cur.pose,
                    center: cur.center,
                    ..spec
                };
                cx.project.sync_annotations();
                cx.mark_dirty();
                Some(id)
            }
            Mode::Defaults => {
                if self.form.spec == self.form.orig {
                    return None;
                }
                cx.begin_change("Change Marker Defaults");
                cx.project.annot_defaults.marker = Marker {
                    items: Vec::new(),
                    pose: None,
                    center: plan_core::geometry::Point::ZERO,
                    ..spec
                };
                cx.mark_dirty();
                Some(0)
            }
        }
    }
}

impl MarkerForm {
    fn text_block(
        ui: &mut Ui,
        id: &'static str,
        label: &str,
        t: &mut LineText,
        below: bool,
        noun: &str,
    ) {
        row(ui, label, |ui| {
            ui.add(egui::TextEdit::singleline(&mut t.text).desired_width(150.0));
            insert_menu(ui, id, &mut t.text, Macros::Marker);
        });
        row(ui, "Alignment", |ui| {
            let opts: &[LineAlign] = if below {
                &[
                    LineAlign::Match,
                    LineAlign::Toward,
                    LineAlign::Centered,
                    LineAlign::Away,
                ]
            } else {
                &[LineAlign::Toward, LineAlign::Centered, LineAlign::Away]
            };
            egui::ComboBox::from_id_salt((id, "align"))
                .selected_text(t.align.label(noun))
                .show_ui(ui, |ui| {
                    for a in opts {
                        ui.selectable_value(&mut t.align, *a, a.label(noun));
                    }
                });
        });
    }

    fn marker_page(&mut self, ui: &mut Ui) {
        let m = &mut self.spec;
        section(ui, "Type");
        ui.horizontal_wrapped(|ui| {
            for k in MarkerKind::ALL {
                ui.radio_value(&mut m.kind, k, k.label());
            }
        });
        ui.weak("Framing Reference Markers are placed from Build > Framing and affect the model.");
        section(ui, "Line");
        self.fields
            .length_row(ui, "Minimum Length", "marker_min", &mut m.min_length);
        self.fields
            .degrees_row(ui, "Angle", "deg_marker_angle", &mut m.angle);
        if !m.kind.has_line() {
            ui.weak("For this type the minimum length is the gap between the marker and its text.");
        }
        section(ui, "Label");
        ui.checkbox(&mut m.auto_adjust_text, "Auto Adjust Text Direction");
        let has_align = m.kind.has_line();
        ui.add_enabled_ui(true, |ui| {
            Self::text_block(ui, "marker_label", "Text", &mut m.label, false, "Marker");
        });
        if !has_align {
            ui.weak("Alignment applies to Level Line and Elevation markers.");
        }
        ui.add_enabled_ui(m.kind.has_line(), |ui| {
            Self::text_block(ui, "marker_below", "Text Below Line", &mut m.below, true, "Marker");
        });
        section(ui, "Size");
        self.fields
            .length_row(ui, "Marker Radius", "marker_radius", &mut m.radius);
        self.fields
            .length_row(ui, "Height", "marker_height", &mut m.height_z);
        ui.weak("Shown by the %height% macro in the label; in plan view it does not move the marker.");
    }
}

impl SpecPages for MarkerForm {
    fn tabs(&self) -> &'static [Tab] {
        MARKER_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.spec.radius <= 0.0 {
            return Some("The marker radius must be greater than zero".into());
        }
        if self.spec.text_height <= 0.0 {
            return Some("The text height must be greater than zero".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match MARKER_TABS[tab].name {
            "Marker" => self.marker_page(ui),
            "Line Style" => {
                let layers = self.env.layers.clone();
                line_style_page(ui, &mut self.spec.line, &mut self.spec.layer, &layers);
            }
            "Text Style" => {
                let styles = self.env.styles.clone();
                text_style_page(
                    ui,
                    &mut self.spec.text_style,
                    &mut self.spec.text_height,
                    &styles,
                    &mut self.fields,
                );
            }
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let g = marker_items(&self.spec, &Vars::default());
        draw_preview(p, rect, &g);
    }
}
