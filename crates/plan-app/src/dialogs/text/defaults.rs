//! Default Settings > Text, Callouts and Markers (TXT-22): the defaults
//! dialogs of Text, Rich Text, Callouts, Markers and Notes. Callouts, Markers
//! and Notes are their specification dialogs (`annot::defaults_dialog`); Text
//! and Rich Text have the Text Specification's panels that a default can set
//! (Text Style and Appearance). Each is titled with the Saved Default's name
//! and stores into `Project::annot_defaults`, which the tools read.

use super::annot::{self, AnnotDialog, AnnotKind};
use crate::dialogs::{on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::EditorContext;
use eframe::egui::{self, Painter, Rect, Ui};
use plan_core::callout::TextSpec;
use plan_core::text_box::{HAlign, VAlign};

/// Which defaults dialog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DefaultsKind {
    Text,
    RichText,
    Callout,
    Marker,
    Note,
}

impl DefaultsKind {
    pub const ALL: [DefaultsKind; 5] = [
        DefaultsKind::Text,
        DefaultsKind::RichText,
        DefaultsKind::Callout,
        DefaultsKind::Marker,
        DefaultsKind::Note,
    ];

    pub fn name(self) -> &'static str {
        match self {
            DefaultsKind::Text => "Text",
            DefaultsKind::RichText => "Rich Text",
            DefaultsKind::Callout => "Callout",
            DefaultsKind::Marker => "Marker",
            DefaultsKind::Note => "Note",
        }
    }
}

const TEXT_TABS: &[Tab] = &[on("Text Style"), on("Appearance")];

pub struct TextDefaultsDialog {
    frame: SpecDialog,
    form: TextDefaultsForm,
    rich: bool,
}

struct TextDefaultsForm {
    spec: TextSpec,
    orig: TextSpec,
    styles: Vec<String>,
    fields: Fields,
}

/// A defaults dialog, whichever kind.
pub enum DefaultsDialog {
    Annot(Box<AnnotDialog>),
    Text(Box<TextDefaultsDialog>),
}

/// The defaults dialog of `kind` for the plan in `cx`.
pub fn open(cx: &EditorContext, kind: DefaultsKind) -> DefaultsDialog {
    let d = &cx.project.annot_defaults;
    match kind {
        DefaultsKind::Callout => {
            DefaultsDialog::Annot(Box::new(annot::defaults_dialog(cx, AnnotKind::Callout)))
        }
        DefaultsKind::Marker => {
            DefaultsDialog::Annot(Box::new(annot::defaults_dialog(cx, AnnotKind::Marker)))
        }
        DefaultsKind::Note => {
            DefaultsDialog::Annot(Box::new(annot::defaults_dialog(cx, AnnotKind::Note)))
        }
        DefaultsKind::Text | DefaultsKind::RichText => {
            let rich = kind == DefaultsKind::RichText;
            let spec = if rich { d.rich.clone() } else { d.text.clone() };
            let title = format!("{} Defaults - {}", kind.name(), d.saved_name);
            DefaultsDialog::Text(Box::new(TextDefaultsDialog {
                frame: SpecDialog::new(
                    title,
                    if rich {
                        "rich_defaults"
                    } else {
                        "text_defaults"
                    },
                ),
                form: TextDefaultsForm {
                    orig: spec.clone(),
                    spec,
                    styles: cx
                        .project
                        .text_styles
                        .names()
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                    fields: Fields::default(),
                },
                rich,
            }))
        }
    }
}

impl DefaultsDialog {
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        match self {
            DefaultsDialog::Annot(d) => d.show(ctx),
            DefaultsDialog::Text(d) => d.frame.show(ctx, &mut d.form),
        }
    }

    pub fn title(&self) -> String {
        match self {
            DefaultsDialog::Annot(d) => d.title(),
            DefaultsDialog::Text(d) => d.frame.geometry_key().to_string(),
        }
    }

    pub fn tab_names(&self) -> Vec<&'static str> {
        match self {
            DefaultsDialog::Annot(d) => d.tab_names(),
            DefaultsDialog::Text(_) => TEXT_TABS.iter().map(|t| t.name).collect(),
        }
    }

    /// Stores the defaults (one undo step). False when nothing changed.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        match self {
            DefaultsDialog::Annot(d) => d.apply(cx).is_some(),
            DefaultsDialog::Text(d) => {
                if d.form.spec == d.form.orig {
                    return false;
                }
                cx.begin_change(if d.rich {
                    "Change Rich Text Defaults"
                } else {
                    "Change Text Defaults"
                });
                let dd = &mut cx.project.annot_defaults;
                if d.rich {
                    dd.rich = d.form.spec.clone();
                } else {
                    dd.text = d.form.spec.clone();
                }
                cx.mark_dirty();
                true
            }
        }
    }
}

impl SpecPages for TextDefaultsForm {
    fn tabs(&self) -> &'static [Tab] {
        TEXT_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.spec.height < 0.0 {
            return Some("The text height cannot be negative".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TEXT_TABS[tab].name {
            "Text Style" => {
                section(ui, "Text Style");
                row(ui, "Text Style", |ui| {
                    let shown = self
                        .spec
                        .style
                        .clone()
                        .unwrap_or_else(|| "Use Layer Text Style".to_string());
                    egui::ComboBox::from_id_salt("text_defaults_style")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.spec.style, None, "Use Layer Text Style");
                            for n in &self.styles {
                                ui.selectable_value(
                                    &mut self.spec.style,
                                    Some(n.clone()),
                                    n.as_str(),
                                );
                            }
                        });
                });
                self.fields.length_row(
                    ui,
                    "Character Height",
                    "text_def_height",
                    &mut self.spec.height,
                );
                ui.weak("0 follows the plan's text height.");
            }
            _ => {
                let tb = &mut self.spec.text_box;
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
                section(ui, "Text Box");
                self.fields
                    .length_row(ui, "Wrap Width", "text_def_width", &mut tb.width);
                ui.checkbox(&mut tb.border, "Border");
                if tb.border {
                    self.fields
                        .length_row(ui, "Margin", "text_def_margin", &mut tb.margin);
                }
                let mut fill = tb.background.is_some();
                if ui.checkbox(&mut fill, "Background Fill").changed() {
                    tb.background = fill.then_some([255, 255, 255]);
                }
                if let Some(k) = &mut tb.background {
                    row(ui, "Fill Color", |ui| ui.color_edit_button_srgb(k));
                }
            }
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        p.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Text",
            egui::FontId::proportional(16.0),
            crate::dialogs::PV_INK,
        );
    }
}
