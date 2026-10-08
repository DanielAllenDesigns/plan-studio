//! Edit > Find/Replace Text: a small window that finds a string in the plan's
//! text objects (notes, callouts, labels) and replaces it, on the active
//! floor or on every floor. Replace All is one undo step.

use crate::editor::EditorContext;
use eframe::egui;
use plan_core::find_text::{TextMatch, TextSearch};

/// Matches listed in the window at most.
const LIST_CAP: usize = 12;

#[derive(Default)]
pub struct FindReplaceDialog {
    pub search: TextSearch,
    pub all_floors: bool,
    /// What the last Find or Replace All reported.
    pub result: String,
    pub matches: Vec<TextMatch>,
}

/// Replace All: swaps the text in every matching text object in one undo
/// step. Returns `(objects changed, occurrences replaced)`; a search that
/// matches nothing records no step.
pub fn replace_all(
    cx: &mut EditorContext,
    search: &TextSearch,
    all_floors: bool,
) -> (usize, usize) {
    if search.find.is_empty()
        || cx
            .project
            .find_text(search, all_floors, cx.floor)
            .is_empty()
    {
        return (0, 0);
    }
    cx.begin_change("Replace Text");
    let (objects, occurrences) = cx.project.replace_text(search, all_floors, cx.floor);
    cx.mark_dirty();
    cx.status = format!(
        "Replaced {occurrences} occurrence{} in {objects} text object{}",
        if occurrences == 1 { "" } else { "s" },
        if objects == 1 { "" } else { "s" },
    );
    (objects, occurrences)
}

impl FindReplaceDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Find: lists what matches and says how many.
    pub fn find(&mut self, cx: &EditorContext) {
        if self.search.find.is_empty() {
            self.matches.clear();
            self.result = "Enter the text to find".into();
            return;
        }
        self.matches = cx
            .project
            .find_text(&self.search, self.all_floors, cx.floor);
        let n: usize = self.matches.iter().map(|m| m.count).sum();
        self.result = if n == 0 {
            "No matches".into()
        } else {
            format!(
                "{n} match{} in {} text object{}",
                if n == 1 { "" } else { "es" },
                self.matches.len(),
                if self.matches.len() == 1 { "" } else { "s" }
            )
        };
    }

    /// Replace All with the dialog's settings.
    pub fn replace_all(&mut self, cx: &mut EditorContext) {
        if self.search.find.is_empty() {
            self.result = "Enter the text to find".into();
            return;
        }
        let (objects, occurrences) = replace_all(cx, &self.search, self.all_floors);
        self.matches.clear();
        self.result = if occurrences == 0 {
            "No matches".into()
        } else {
            format!("Replaced {occurrences} in {objects} text object(s)")
        };
    }

    /// Draws the window; false once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let mut find = false;
        let mut replace = false;
        egui::Window::new("Find/Replace Text")
            .id(egui::Id::new("find_replace_text"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("find_replace_grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Find");
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut self.search.find).desired_width(220.0),
                        );
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            find = true;
                        }
                        ui.end_row();
                        ui.label("Replace With");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search.replace)
                                .desired_width(220.0),
                        );
                        ui.end_row();
                    });
                ui.checkbox(&mut self.search.match_case, "Match Case");
                ui.checkbox(&mut self.search.whole_word, "Whole Words Only");
                ui.checkbox(&mut self.all_floors, "All Floors");
                ui.horizontal(|ui| {
                    find |= ui.button("Find").clicked();
                    replace |= ui.button("Replace All").clicked();
                });
                if !self.result.is_empty() {
                    ui.label(&self.result);
                }
                for m in self.matches.iter().take(LIST_CAP) {
                    ui.weak(format!("{}: {}", cx.project.floors[m.floor].name, m.text));
                }
                if self.matches.len() > LIST_CAP {
                    ui.weak(format!("{} more", self.matches.len() - LIST_CAP));
                }
            });
        if find {
            self.find(cx);
        }
        if replace {
            self.replace_all(cx);
        }
        open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::cad::CadItem;
    use plan_core::geometry::Point;

    fn cx_with(texts: &[&str]) -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        for t in texts {
            cx.project.add_cad(
                0,
                "CAD, Default",
                CadItem::Text {
                    pos: Point::ZERO,
                    text: (*t).into(),
                    height: 3.0,
                    angle: 0.0,
                },
            );
        }
        cx
    }

    fn texts(cx: &EditorContext) -> Vec<String> {
        cx.floor()
            .cad
            .iter()
            .filter_map(|c| match &c.item {
                CadItem::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn find_counts_and_replace_all_is_one_undo_step() {
        let mut cx = cx_with(&["Bath 1", "Hall", "Bath 2 and Bath 3"]);
        let mut d = FindReplaceDialog::new();
        d.search.find = "bath".into();
        d.search.replace = "Bathroom".into();
        d.find(&cx);
        assert_eq!(d.result, "3 matches in 2 text objects");
        assert_eq!(d.matches.len(), 2);
        d.replace_all(&mut cx);
        assert_eq!(
            texts(&cx),
            vec!["Bathroom 1", "Hall", "Bathroom 2 and Bathroom 3"]
        );
        assert_eq!(cx.undo_label(), Some("Replace Text"));
        cx.undo();
        assert_eq!(texts(&cx), vec!["Bath 1", "Hall", "Bath 2 and Bath 3"]);
    }

    #[test]
    fn nothing_found_records_no_step() {
        let mut cx = cx_with(&["Hall"]);
        let mut d = FindReplaceDialog::new();
        d.search.find = "Porch".into();
        d.replace_all(&mut cx);
        assert_eq!(d.result, "No matches");
        assert!(!cx.can_undo());
        d.search.find.clear();
        d.find(&cx);
        assert_eq!(d.result, "Enter the text to find");
    }

    #[test]
    fn the_window_draws() {
        let mut cx = cx_with(&["Hall"]);
        let mut d = FindReplaceDialog::new();
        d.search.find = "Hall".into();
        d.find(&cx);
        let ctx = egui::Context::default();
        let mut open = true;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            open = d.show(ctx, &mut cx);
        });
        assert!(open);
    }
}
