//! Note Type Management, Text Macro Management and Text Style Management
//! (Text flyout).
//!
//! All are small list editors on a copy of the plan's settings; OK hands the
//! edited copy (for text styles, the list of renames and removals) back to
//! the Text tool, which stores it as one undo step.

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

/// One edit of the Text Style Management list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleOp {
    Rename { from: String, to: String },
    Remove(String),
}

/// Text Style Management: rename and remove the plan's text styles. A
/// rename repoints everything that used the style (layers, layer sets, plan
/// views, texts, dimensions, schedules); a removal sends them back to
/// inheriting (`Project::rename_text_style` / `remove_text_style`). The dialog
/// keeps the list of edits; OK applies them in order as one undo step
/// ([`StyleOp::apply_all`]).
pub struct TextStyleDialog {
    /// The style names as they stand after the edits so far.
    names: Vec<String>,
    ops: Vec<StyleOp>,
    selected: usize,
    rename_to: String,
    error: Option<String>,
}

impl TextStyleDialog {
    pub fn new(names: Vec<String>) -> Self {
        Self {
            names,
            ops: Vec::new(),
            selected: 0,
            rename_to: String::new(),
            error: None,
        }
    }

    /// The styles as they will be after OK.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The edits OK will apply.
    pub fn ops(&self) -> &[StyleOp] {
        &self.ops
    }

    fn is_default(name: &str) -> bool {
        name == plan_core::text_styles::DEFAULT_TEXT_STYLE_NAME
    }

    pub fn select(&mut self, name: &str) -> bool {
        match self.names.iter().position(|n| n == name) {
            Some(i) => {
                self.selected = i;
                self.rename_to = name.to_string();
                true
            }
            None => false,
        }
    }

    /// Renames style `from`; false (with the reason in [`Self::error`]) for
    /// the Default Text Style, an empty name or one already taken.
    pub fn rename(&mut self, from: &str, to: &str) -> bool {
        let to = to.trim();
        let err = if Self::is_default(from) {
            Some("The Default Text Style cannot be renamed")
        } else if to.is_empty() {
            Some("Give the style a name")
        } else if self.names.iter().any(|n| n == to) {
            Some("Another style already has that name")
        } else if !self.names.iter().any(|n| n == from) {
            Some("That style does not exist")
        } else {
            None
        };
        if let Some(e) = err {
            self.error = Some(e.into());
            return false;
        }
        if let Some(n) = self.names.iter_mut().find(|n| n.as_str() == from) {
            *n = to.to_string();
        }
        self.ops.push(StyleOp::Rename {
            from: from.to_string(),
            to: to.to_string(),
        });
        self.error = None;
        true
    }

    /// Removes style `name`; false for the Default Text Style.
    pub fn remove(&mut self, name: &str) -> bool {
        if Self::is_default(name) || !self.names.iter().any(|n| n == name) {
            self.error = Some("The Default Text Style cannot be removed".into());
            return false;
        }
        self.names.retain(|n| n != name);
        self.ops.push(StyleOp::Remove(name.to_string()));
        self.selected = self.selected.min(self.names.len().saturating_sub(1));
        self.error = None;
        true
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let (mut open, mut ok, mut cancel) = (true, false, false);
        self.selected = self.selected.min(self.names.len().saturating_sub(1));
        egui::Window::new("Text Style Management")
            .id(egui::Id::new("text_style_management"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([420.0, 360.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("text_style_list")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        let mut pick = None;
                        for (i, n) in self.names.iter().enumerate() {
                            if ui.selectable_label(self.selected == i, n).clicked() {
                                pick = Some(n.clone());
                            }
                        }
                        if let Some(n) = pick {
                            self.select(&n);
                        }
                    });
                ui.separator();
                let current = self.names.get(self.selected).cloned();
                if let Some(cur) = current {
                    let locked = Self::is_default(&cur);
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.add_enabled(
                            !locked,
                            egui::TextEdit::singleline(&mut self.rename_to).desired_width(200.0),
                        );
                        if ui
                            .add_enabled(!locked, egui::Button::new("Rename"))
                            .clicked()
                        {
                            let to = self.rename_to.clone();
                            self.rename(&cur, &to);
                        }
                        if ui
                            .add_enabled(!locked, egui::Button::new("Remove"))
                            .clicked()
                        {
                            self.remove(&cur);
                        }
                    });
                    ui.weak(if locked {
                        "The Default Text Style cannot be renamed or removed."
                    } else {
                        "Renaming follows the style everywhere it is used; removing sends its users back to the layer's style."
                    });
                }
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

impl StyleOp {
    /// Applies the edits to the plan in order; returns how many worked.
    pub fn apply_all(ops: &[StyleOp], project: &mut plan_core::Project) -> usize {
        ops.iter()
            .filter(|op| match op {
                StyleOp::Rename { from, to } => project.rename_text_style(from, to),
                StyleOp::Remove(name) => project.remove_text_style(name),
            })
            .count()
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
    #[test]
    fn text_style_dialog_renames_and_removes_but_never_the_default() {
        let mut d = TextStyleDialog::new(vec![
            "Default Text Style".into(),
            "Room Label Style".into(),
            "Schedule Style".into(),
        ]);
        assert!(!d.rename("Default Text Style", "X"));
        assert!(d.error().is_some());
        assert!(!d.rename("Room Label Style", ""));
        assert!(!d.rename("Room Label Style", "Schedule Style"));
        assert!(d.rename("Room Label Style", " Labels "));
        assert_eq!(d.names()[1], "Labels");
        assert!(!d.remove("Default Text Style"));
        assert!(d.remove("Schedule Style"));
        assert_eq!(d.names().len(), 2);
        assert_eq!(
            d.ops(),
            [
                StyleOp::Rename {
                    from: "Room Label Style".into(),
                    to: "Labels".into()
                },
                StyleOp::Remove("Schedule Style".into())
            ]
        );
        let ctx = egui::Context::default();
        let mut out = Outcome::Open;
        let _ = ctx.run(egui::RawInput::default(), |ctx| out = d.show(ctx));
        assert_eq!(out, Outcome::Open);
    }

    #[test]
    fn style_ops_follow_the_style_through_the_plan_in_order() {
        let mut p = plan_core::Project::new("t");
        let layer = p.layers.layers[0].name.clone();
        p.layers.layers[0].text_style = "Room Label Style".into();
        let ops = [
            StyleOp::Rename {
                from: "Room Label Style".into(),
                to: "Labels".into(),
            },
            StyleOp::Rename {
                from: "Labels".into(),
                to: "Tags".into(),
            },
        ];
        assert_eq!(StyleOp::apply_all(&ops, &mut p), 2);
        assert!(p.text_styles.get("Tags").is_some());
        assert!(p.text_styles.get("Room Label Style").is_none());
        assert_eq!(p.layers.get(&layer).unwrap().text_style, "Tags");
        assert_eq!(
            StyleOp::apply_all(&[StyleOp::Remove("Tags".into())], &mut p),
            1
        );
        assert_eq!(p.layers.get(&layer).unwrap().text_style, "");
        // Refusals are not counted.
        assert_eq!(
            StyleOp::apply_all(&[StyleOp::Remove("Default Text Style".into())], &mut p),
            0
        );
    }
}
