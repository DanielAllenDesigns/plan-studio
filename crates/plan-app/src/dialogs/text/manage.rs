//! Note Type Management and Text Macro Management (Text flyout).
//!
//! Both are small list editors on a copy of the plan's settings; OK hands the
//! edited copy back to the Text tool, which stores it as one undo step.

#![allow(dead_code)]

use crate::dialogs::Outcome;
use eframe::egui::{self, Align2, RichText};
use plan_core::text_styles::{NoteTypes, TextMacros, BUILT_IN_MACROS};

fn finish(ctx: &egui::Context, open: bool, ok: bool, cancel: bool) -> Outcome {
    let esc = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if ok {
        Outcome::Ok
    } else if cancel || !open || esc {
        Outcome::Cancel
    } else {
        Outcome::Open
    }
}

/// Note Type Management: the kinds of note, each with a label prefix
/// (`Note 3:`, `E 1:`) and a text style.
pub struct NoteTypeDialog {
    draft: NoteTypes,
    selected: usize,
    new_name: String,
    new_prefix: String,
    styles: Vec<String>,
    error: Option<String>,
}

impl NoteTypeDialog {
    pub fn new(types: NoteTypes, styles: Vec<String>) -> Self {
        Self {
            draft: types,
            selected: 0,
            new_name: String::new(),
            new_prefix: String::new(),
            styles,
            error: None,
        }
    }

    pub fn draft(&self) -> &NoteTypes {
        &self.draft
    }

    /// Adds a type with the given name and prefix (as typing into the "new"
    /// fields and pressing Add does).
    pub fn add_new_named(&mut self, name: &str, prefix: &str) -> bool {
        self.new_name = name.to_string();
        self.new_prefix = prefix.to_string();
        self.add_new()
    }

    /// Adds a type from the "new" fields; false when the name or prefix is
    /// not usable.
    pub fn add_new(&mut self) -> bool {
        let ok = self.draft.add(&self.new_name, self.new_prefix.trim());
        if ok {
            self.selected = self.draft.types.len() - 1;
            self.new_name.clear();
            self.new_prefix.clear();
            self.error = None;
        } else {
            self.error = Some("Give the type a new name and a prefix of letters and digits".into());
        }
        ok
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let (mut open, mut ok, mut cancel) = (true, false, false);
        self.selected = self.selected.min(self.draft.types.len().saturating_sub(1));
        egui::Window::new("Note Type Management")
            .id(egui::Id::new("note_type_management"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([460.0, 380.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("note_type_list")
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for (i, t) in self.draft.types.iter().enumerate() {
                            let label = format!("{}    ({} 1: ...)", t.name, t.prefix);
                            if ui.selectable_label(self.selected == i, label).clicked() {
                                self.selected = i;
                            }
                        }
                    });
                ui.separator();
                let first = self.selected == 0;
                let styles = self.styles.clone();
                if let Some(t) = self.draft.types.get_mut(self.selected) {
                    ui.horizontal(|ui| {
                        ui.label("Prefix");
                        let mut prefix = t.prefix.clone();
                        if ui
                            .add(egui::TextEdit::singleline(&mut prefix).desired_width(80.0))
                            .changed()
                            && prefix.chars().all(char::is_alphanumeric)
                            && !prefix.is_empty()
                        {
                            t.prefix = prefix;
                        }
                        ui.label("Text style");
                        egui::ComboBox::from_id_salt("note_type_style")
                            .selected_text(if t.style.is_empty() {
                                "Default".to_string()
                            } else {
                                t.style.clone()
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut t.style, String::new(), "Default");
                                for s in &styles {
                                    ui.selectable_value(&mut t.style, s.clone(), s.as_str());
                                }
                            });
                    });
                    if ui
                        .add_enabled(!first, egui::Button::new("Remove this type"))
                        .clicked()
                    {
                        let name = t.name.clone();
                        self.draft.remove(&name);
                    }
                }
                ui.separator();
                ui.label(RichText::new("New note type").strong());
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut self.new_name).desired_width(150.0));
                    ui.label("Prefix");
                    ui.add(egui::TextEdit::singleline(&mut self.new_prefix).desired_width(60.0));
                    if ui.button("Add").clicked() {
                        self.add_new();
                    }
                });
                if let Some(e) = &self.error {
                    ui.colored_label(egui::Color32::from_rgb(0xE0, 0x4B, 0x4B), e);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("   OK   ").strong()).clicked() {
                        ok = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        finish(ctx, open, ok, cancel)
    }
}

/// Text Macro Management: the built-in macros and the user's own.
pub struct MacroDialog {
    draft: TextMacros,
    new_name: String,
    new_text: String,
    error: Option<String>,
}

impl MacroDialog {
    pub fn new(macros: TextMacros) -> Self {
        Self {
            draft: macros,
            new_name: String::new(),
            new_text: String::new(),
            error: None,
        }
    }

    pub fn draft(&self) -> &TextMacros {
        &self.draft
    }

    /// Adds a macro from the "new" fields.
    pub fn add_new(&mut self) -> bool {
        let ok = self.draft.add(self.new_name.trim(), &self.new_text);
        if ok {
            self.new_name.clear();
            self.new_text.clear();
            self.error = None;
        } else {
            self.error = Some(
                "Use a new name of letters, digits, '.', '_' or '-' that is not built in".into(),
            );
        }
        ok
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let (mut open, mut ok, mut cancel) = (true, false, false);
        egui::Window::new("Text Macro Management")
            .id(egui::Id::new("text_macro_management"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([520.0, 460.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.label(RichText::new("Built-in macros").strong());
                egui::Grid::new("builtin_macros")
                    .striped(true)
                    .show(ui, |ui| {
                        for (name, what) in BUILT_IN_MACROS {
                            ui.monospace(format!("%{name}%"));
                            ui.label(*what);
                            ui.end_row();
                        }
                    });
                ui.separator();
                ui.label(RichText::new("Your macros").strong());
                let mut remove = None;
                egui::ScrollArea::vertical()
                    .id_salt("user_macros")
                    .max_height(120.0)
                    .show(ui, |ui| {
                        if self.draft.macros.is_empty() {
                            ui.weak("None yet.");
                        }
                        for m in &mut self.draft.macros {
                            ui.horizontal(|ui| {
                                ui.monospace(format!("%{}%", m.name));
                                ui.add(
                                    egui::TextEdit::singleline(&mut m.text).desired_width(260.0),
                                );
                                if ui.button("Remove").clicked() {
                                    remove = Some(m.name.clone());
                                }
                            });
                        }
                    });
                if let Some(n) = remove {
                    self.draft.remove(&n);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut self.new_name).desired_width(110.0));
                    ui.label("Text");
                    ui.add(egui::TextEdit::singleline(&mut self.new_text).desired_width(200.0));
                    if ui.button("Add").clicked() {
                        self.add_new();
                    }
                });
                if let Some(e) = &self.error {
                    ui.colored_label(egui::Color32::from_rgb(0xE0, 0x4B, 0x4B), e);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("   OK   ").strong()).clicked() {
                        ok = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        finish(ctx, open, ok, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_type_dialog_adds_and_rejects() {
        let mut d = NoteTypeDialog::new(NoteTypes::default(), vec!["Default Text Style".into()]);
        assert!(!d.add_new(), "empty fields are refused");
        d.new_name = "Plumbing Note".into();
        d.new_prefix = "P".into();
        assert!(d.add_new());
        assert!(d.draft().get("Plumbing Note").is_some());
        d.new_name = "Plumbing Note".into();
        d.new_prefix = "Q".into();
        assert!(!d.add_new(), "duplicate names are refused");
        let ctx = egui::Context::default();
        let mut out = Outcome::Open;
        let _ = ctx.run(egui::RawInput::default(), |ctx| out = d.show(ctx));
        assert_eq!(out, Outcome::Open);
    }

    #[test]
    fn macro_dialog_adds_user_macros_only() {
        let mut d = MacroDialog::new(TextMacros::default());
        d.new_name = "room.name".into();
        d.new_text = "x".into();
        assert!(!d.add_new(), "built-in names are refused");
        d.new_name = "firm".into();
        d.new_text = "Daniel Allen Designs".into();
        assert!(d.add_new());
        assert_eq!(d.draft().get("firm").unwrap().text, "Daniel Allen Designs");
        let ctx = egui::Context::default();
        let mut out = Outcome::Open;
        let _ = ctx.run(egui::RawInput::default(), |ctx| out = d.show(ctx));
        assert_eq!(out, Outcome::Open);
    }
}
