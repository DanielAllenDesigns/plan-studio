//! Callout Specification (TXT-49..51; manual pp. 552 to 554). Panels:
//! Callout, Attributes, Line Style, Section Arrow, Main Text Style and Link.
//! The same dialog is the Callout Defaults dialog (title "Callout Defaults -
//! <Saved Default>") and the one the Callout tool opens when clicked.

use super::annot::{
    color_row, draw_preview, insert_menu, line_look_rows, line_style_page, link_lines, lock_check,
    text_style_page, transparency_row, AnnotDialog, Env, Macros, Mode,
};
use crate::dialogs::{on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::EditorContext;
use eframe::egui::{self, Painter, Rect, Ui};
use plan_core::cad::ArrowStyle;
use plan_core::callout::{callout_with_caution, ArrowSize, Callout, CalloutShape, LineAlign, Vars};
use plan_core::Id;

pub const CALLOUT_TABS: &[Tab] = &[
    on("Callout"),
    on("Attributes"),
    on("Line Style"),
    on("Section Arrow"),
    on("Main Text Style"),
    on("Link"),
];

pub struct CalloutDialog {
    frame: SpecDialog,
    form: CalloutForm,
}

struct CalloutForm {
    spec: Callout,
    orig: Callout,
    mode: Mode,
    env: Env,
    fields: Fields,
}

impl CalloutDialog {
    pub fn new(spec: Callout, mode: Mode, env: Env) -> Self {
        let title = super::annot::title(super::annot::AnnotKind::Callout, mode, &env.saved_name);
        Self {
            frame: SpecDialog::new(title, "callout"),
            form: CalloutForm {
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
            super::annot::AnnotKind::Callout,
            self.form.mode,
            &self.form.env.saved_name,
        )
    }

    pub fn tab_names(&self) -> Vec<&'static str> {
        CALLOUT_TABS.iter().map(|t| t.name).collect()
    }

    /// Test access: the draft the form edits.
    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Callout {
        &mut self.form.spec
    }

    pub fn draft(&self) -> &Callout {
        &self.form.spec
    }

    /// Places, changes or stores the defaults (one undo step).
    pub fn apply(&self, cx: &mut EditorContext) -> Option<Id> {
        let mut spec = self.form.spec.clone();
        let at = spec.center;
        let expand = |cx: &EditorContext, s: &str| crate::tools::text::TextTool::expand(cx, s, at);
        spec.label = expand(cx, &spec.label);
        spec.text_below = expand(cx, &spec.text_below);
        spec.section.above.text = expand(cx, &spec.section.above.text);
        spec.section.below.text = expand(cx, &spec.section.below.text);
        let fl = cx.floor;
        match self.form.mode {
            Mode::New => {
                if cx.layers().is_locked(&spec.layer) {
                    cx.status = format!("The layer \"{}\" is locked", spec.layer);
                    return None;
                }
                cx.begin_change("Place Callout");
                let id = cx.project.add_callout(fl, spec);
                AnnotDialog::select(cx, id);
                cx.mark_dirty();
                Some(id)
            }
            Mode::Edit(id) => {
                if self.form.spec == self.form.orig || !lock_check(cx, id) {
                    return None;
                }
                let idx = match cx.floor().annot_of(id) {
                    Some(plan_core::callout::AnnotRef::Callout(i)) => i,
                    _ => return None,
                };
                cx.begin_change("Change Callout");
                let cur = cx.project.floors[fl].annots.callouts[idx].clone();
                cx.project.floors[fl].annots.callouts[idx] = Callout {
                    items: cur.items,
                    pose_idx: cur.pose_idx,
                    pose: cur.pose,
                    leaders: cur.leaders,
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
                cx.begin_change("Change Callout Defaults");
                cx.project.annot_defaults.callout = Callout {
                    items: Vec::new(),
                    pose: None,
                    leaders: Vec::new(),
                    center: plan_core::geometry::Point::ZERO,
                    link: None,
                    ..spec
                };
                cx.mark_dirty();
                Some(0)
            }
        }
    }
}

impl CalloutForm {
    fn vars(&self) -> Vars {
        Vars {
            link: self
                .spec
                .link
                .as_ref()
                .and_then(|l| self.env.links.iter().find(|c| c.link == *l))
                .map(|c| c.info.clone()),
            ..Vars::default()
        }
    }

    fn callout_page(&mut self, ui: &mut Ui) {
        let c = &mut self.spec;
        section(ui, "Label");
        row(ui, "Callout Label", |ui| {
            ui.add(egui::TextEdit::singleline(&mut c.label).desired_width(150.0));
            insert_menu(ui, "callout_label", &mut c.label, Macros::Callout);
        });
        row(ui, "Text Below Line", |ui| {
            ui.add(egui::TextEdit::singleline(&mut c.text_below).desired_width(150.0));
            insert_menu(ui, "callout_below", &mut c.text_below, Macros::Callout);
        });
        row(ui, "", |ui| {
            ui.checkbox(&mut c.auto_below, "Automatic")
                .on_hover_text("Fill with the layout page label of the linked view");
        });
        section(ui, "Shape");
        ui.horizontal_wrapped(|ui| {
            for s in CalloutShape::ALL {
                ui.radio_value(&mut c.shape, s, s.label());
            }
        });
        section(ui, "Fill Color");
        ui.checkbox(&mut c.filled, "Filled");
        if c.filled {
            color_row(ui, "Color", &mut c.fill_color);
            transparency_row(ui, &mut c.transparency);
        }
        section(ui, "Size/Orientation");
        ui.checkbox(&mut c.auto_size, "Automatic");
        if !c.auto_size {
            self.fields
                .length_row(ui, "Size", "callout_size", &mut c.size);
        }
        self.fields
            .degrees_row(ui, "Shape Angle", "deg_callout_shape", &mut c.shape_angle);
        row(ui, "Text Angle", |ui| {
            let mut auto = c.text_angle.is_none();
            if ui.checkbox(&mut auto, "Automatic").changed() {
                c.text_angle = if auto { None } else { Some(c.shape_angle) };
            }
            if let Some(a) = &mut c.text_angle {
                self.fields.degrees(ui, "deg_callout_text", a);
            }
        });
        ui.checkbox(&mut c.auto_adjust_text, "Auto Adjust Text Direction");
    }

    fn line_text(
        ui: &mut Ui,
        id: &'static str,
        label: &str,
        t: &mut plan_core::callout::LineText,
        styles: &[String],
        below: bool,
    ) {
        row(ui, label, |ui| {
            ui.add(egui::TextEdit::singleline(&mut t.text).desired_width(150.0));
            insert_menu(ui, id, &mut t.text, Macros::Callout);
        });
        row(ui, "Text Style", |ui| {
            let mut m = t.style.is_none();
            let matches = if below {
                "Match Text Above"
            } else {
                "Match Callout"
            };
            if ui.checkbox(&mut m, matches).changed() {
                t.style = if m { None } else { styles.first().cloned() };
            }
            if let Some(s) = &mut t.style {
                egui::ComboBox::from_id_salt((id, "style"))
                    .selected_text(s.clone())
                    .show_ui(ui, |ui| {
                        for n in styles {
                            ui.selectable_value(s, n.clone(), n.as_str());
                        }
                    });
            }
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
                .selected_text(t.align.label("Callout"))
                .show_ui(ui, |ui| {
                    for a in opts {
                        ui.selectable_value(&mut t.align, *a, a.label("Callout"));
                    }
                });
        });
    }

    fn attributes_page(&mut self, ui: &mut Ui) {
        let styles = self.env.styles.clone();
        let c = &mut self.spec;
        section(ui, "Cross Section Line");
        ui.checkbox(&mut c.section.on, "Cross Section Line");
        ui.add_enabled_ui(c.section.on, |ui| {
            ui.checkbox(&mut c.section.double, "Double Callout");
            line_look_rows(ui, &mut c.section.look);
            self.fields.length_row(
                ui,
                "Minimum Length",
                "callout_sec_min",
                &mut c.section.min_length,
            );
            self.fields.degrees_row(
                ui,
                "Relative Angle",
                "deg_callout_sec_rel",
                &mut c.section.rel_angle,
            );
            ui.checkbox(
                &mut c.section.auto_adjust_text,
                "Auto Adjust Text Direction",
            );
            Self::line_text(
                ui,
                "callout_above",
                "Text Above Line",
                &mut c.section.above,
                &styles,
                false,
            );
            Self::line_text(
                ui,
                "callout_belowline",
                "Text Below Line",
                &mut c.section.below,
                &styles,
                true,
            );
        });
        section(ui, "Callout Arrows");
        row(ui, "Size", |ui| {
            ui.radio_value(&mut c.arrows.size, ArrowSize::Small, "Small");
            ui.radio_value(&mut c.arrows.size, ArrowSize::Large, "Large");
        });
        ui.checkbox(&mut c.arrows.filled, "Filled");
        color_row(ui, "Color", &mut c.arrows.color);
        transparency_row(ui, &mut c.arrows.transparency);
        row(ui, "Number of Arrows", |ui| {
            let mut n = c.arrows.angles.len();
            if ui.add(egui::DragValue::new(&mut n).range(0..=8)).changed() {
                c.arrows.set_count(n);
            }
        });
        ui.weak("The angle of each arrow is set with the callout's edit handles.");
    }

    fn section_arrow_page(&mut self, ui: &mut Ui) {
        let c = &mut self.spec;
        section(ui, "Section Arrow");
        ui.add_enabled_ui(!c.section.double, |ui| {
            let mut on = c.section.arrow;
            if ui.checkbox(&mut on, "Include Arrow").changed() {
                c.section.arrow = on;
                if on {
                    c.section.on = true;
                }
            }
            row(ui, "Arrow Style", |ui| {
                egui::ComboBox::from_id_salt("callout_arrow_style")
                    .selected_text(c.section.arrow_style.name())
                    .show_ui(ui, |ui| {
                        for s in ArrowStyle::ALL {
                            ui.selectable_value(&mut c.section.arrow_style, s, s.name());
                        }
                    });
            });
            self.fields.length_row(
                ui,
                "Arrow Length",
                "callout_arrow_len",
                &mut c.section.arrow_length,
            );
        });
        if c.section.double {
            ui.weak("Not available for a Double Callout.");
        }
    }

    fn link_page(&mut self, ui: &mut Ui) {
        section(ui, "Link Information");
        let info = self
            .spec
            .link
            .as_ref()
            .and_then(|l| self.env.links.iter().find(|c| c.link == *l))
            .map(|c| c.info.clone());
        for (k, v) in link_lines(info.as_ref()) {
            row(ui, k, |ui| ui.label(v));
        }
        if self.spec.link.is_some() && info.as_ref().is_none_or(|i| !i.valid) {
            ui.colored_label(
                egui::Color32::from_rgb(200, 140, 0),
                "The link is broken: the view or page is gone.",
            );
        }
        ui.horizontal(|ui| {
            let relink = self.spec.link.is_some();
            ui.menu_button(if relink { "Relink" } else { "Link" }, |ui| {
                if self.env.links.is_empty() {
                    ui.weak("No saved views or layout pages");
                }
                for c in self.env.links.clone() {
                    if ui.button(&c.label).clicked() {
                        self.spec.link = Some(c.link);
                        self.spec.ignore_link = false;
                        ui.close_menu();
                    }
                }
            });
            if ui
                .add_enabled(self.spec.link.is_some(), egui::Button::new("Unlink"))
                .clicked()
            {
                self.spec.link = None;
            }
        });
        ui.weak(
            "Use %linked_view_name% in the label and %linked_view_layout_page_label% in the Text Below Line.",
        );
    }
}

impl SpecPages for CalloutForm {
    fn tabs(&self) -> &'static [Tab] {
        CALLOUT_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.spec.height <= 0.0 {
            return Some("The text height must be greater than zero".into());
        }
        if !self.spec.auto_size && self.spec.size <= 0.0 {
            return Some("The size must be greater than zero".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match CALLOUT_TABS[tab].name {
            "Callout" => self.callout_page(ui),
            "Attributes" => self.attributes_page(ui),
            "Line Style" => {
                let layers = self.env.layers.clone();
                line_style_page(ui, &mut self.spec.line, &mut self.spec.layer, &layers);
            }
            "Section Arrow" => self.section_arrow_page(ui),
            "Main Text Style" => {
                let styles = self.env.styles.clone();
                text_style_page(
                    ui,
                    &mut self.spec.text_style,
                    &mut self.spec.height,
                    &styles,
                    &mut self.fields,
                );
            }
            "Link" => self.link_page(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let g = callout_with_caution(&self.spec, &self.vars(), false);
        draw_preview(p, rect, &g);
    }
}
