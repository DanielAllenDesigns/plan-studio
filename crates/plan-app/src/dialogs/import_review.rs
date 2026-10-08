//! Import Property Data: the review of an imported workbook or CSV before
//! anything changes.
//!
//! The dialog lists every change the import found (object, field, current
//! value, value from Excel) with a checkbox each, flags the ones the field's
//! type refuses (never applied) and the ones whose value also changed in the
//! plan since the export (Excel's value would overwrite a newer one), and
//! lists the notes: objects deleted since the export, computed columns that
//! were edited (ignored), rows with no id. OK applies the checked changes as
//! one undo step.

use super::{Outcome, ERROR_RED};
use crate::editor::EditorContext;
use eframe::egui::{self, Color32, RichText};
use plan_docs::props_exchange::{apply_changes, ApplyReport, Change, ImportPlan, NoteKind};

/// Orange for a conflict: 6:1 on the dark panel.
const CONFLICT: Color32 = Color32::from_rgb(0xFF, 0xB3, 0x47);

/// The review window and its checkboxes.
pub struct ImportReview {
    plan: ImportPlan,
    accept: Vec<bool>,
    /// The file name shown in the title line.
    source: String,
}

impl ImportReview {
    /// Changes that can be applied start checked; refused ones cannot be.
    pub fn new(plan: ImportPlan, source: impl Into<String>) -> Self {
        let accept = plan.changes.iter().map(|c| c.problem.is_none()).collect();
        Self {
            plan,
            accept,
            source: source.into(),
        }
    }

    pub fn plan(&self) -> &ImportPlan {
        &self.plan
    }

    /// How many changes are checked.
    pub fn accepted(&self) -> usize {
        self.accept.iter().filter(|a| **a).count()
    }

    /// Checks or unchecks change `i` (a refused change stays unchecked).
    pub fn set_accept(&mut self, i: usize, on: bool) {
        if let (Some(a), Some(c)) = (self.accept.get_mut(i), self.plan.changes.get(i)) {
            *a = on && c.problem.is_none();
        }
    }

    pub fn select_all(&mut self, on: bool) {
        for i in 0..self.accept.len() {
            self.set_accept(i, on);
        }
    }

    /// Applies the checked changes as one undo step "Import Property Data"
    /// and reports the count in the status bar. No undo step is left when
    /// nothing was applied.
    pub fn apply(&self, cx: &mut EditorContext) -> ApplyReport {
        let chosen: Vec<&Change> = self
            .plan
            .changes
            .iter()
            .zip(&self.accept)
            .filter(|(_, a)| **a)
            .map(|(c, _)| c)
            .collect();
        cx.refresh();
        let rooms = cx.rooms.clone();
        let floor = cx.floor;
        cx.begin_change("Import Property Data");
        let report = apply_changes(&mut cx.project, &chosen, Some((floor, &rooms)));
        if report.applied == 0 {
            cx.cancel_change();
        } else {
            cx.mark_dirty();
        }
        cx.status = status_line(&report, &self.source);
        report
    }

    /// Draws the window. `Ok` means apply, `Cancel` means close.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        egui::Window::new("Import Property Data")
            .id(egui::Id::new("import_property_data"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([760.0, 480.0])
            .show(ctx, |ui| {
                self.body(ui, &mut outcome);
            });
        if !open {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn body(&mut self, ui: &mut egui::Ui, outcome: &mut Outcome) {
        let p = &self.plan;
        ui.label(RichText::new(&self.source).strong());
        let mut meta = format!(
            "{} sheet{}, {} row{} read",
            p.sheets_read,
            if p.sheets_read == 1 { "" } else { "s" },
            p.rows_read,
            if p.rows_read == 1 { "" } else { "s" }
        );
        if !p.exported_at.is_empty() {
            meta.push_str(&format!("; exported {}", p.exported_at));
        }
        if !p.plan_path.is_empty() {
            meta.push_str(&format!(" from {}", p.plan_path));
        }
        ui.weak(meta);
        ui.add_space(4.0);
        if self.plan.changes.is_empty() {
            ui.label("No changes: every cell matches the plan.");
        }
        let mut toggles: Vec<(usize, bool)> = Vec::new();
        egui::ScrollArea::vertical()
            .max_height((ui.available_height() - 90.0).max(120.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if !self.plan.changes.is_empty() {
                    egui::Grid::new("import_changes")
                        .num_columns(5)
                        .striped(true)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            ui.strong("");
                            ui.strong("Object");
                            ui.strong("Field");
                            ui.strong("In the plan");
                            ui.strong("From the file");
                            ui.end_row();
                            for (i, c) in self.plan.changes.iter().enumerate() {
                                let mut on = self.accept[i];
                                let r = ui.add_enabled(
                                    c.problem.is_none(),
                                    egui::Checkbox::without_text(&mut on),
                                );
                                if r.changed() {
                                    toggles.push((i, on));
                                }
                                ui.label(&c.object);
                                ui.label(&c.field_title);
                                ui.label(blank_as_dash(&c.old));
                                ui.vertical(|ui| {
                                    ui.label(blank_as_dash(&c.new));
                                    if let Some(p) = &c.problem {
                                        ui.colored_label(ERROR_RED, format!("Not applied: {p}"));
                                    } else if let Some(w) = &c.conflict {
                                        ui.colored_label(CONFLICT, format!("Conflict: {w}"));
                                    }
                                });
                                ui.end_row();
                            }
                        });
                }
                if !self.plan.notes.is_empty() {
                    ui.add_space(8.0);
                    ui.strong(format!("Left out ({})", self.plan.notes.len()));
                    for n in &self.plan.notes {
                        let (tag, color) = match n.kind {
                            NoteKind::ObjectDeleted => ("Deleted", ERROR_RED),
                            NoteKind::Computed => ("Computed", CONFLICT),
                            NoteKind::NoId => ("No id", CONFLICT),
                            NoteKind::Unknown => ("Unknown", CONFLICT),
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(color, tag);
                            ui.label(format!("{}: {}", n.sheet, n.text));
                        });
                    }
                }
            });
        for (i, on) in toggles {
            self.set_accept(i, on);
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Select All").clicked() {
                self.select_all(true);
            }
            if ui.button("Select None").clicked() {
                self.select_all(false);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let n = self.accepted();
                let label = format!(
                    "Import {n} Change{}",
                    if n == 1 { "" } else { "s" }
                );
                if ui
                    .add_enabled(n > 0, egui::Button::new(RichText::new(label).strong()))
                    .clicked()
                {
                    *outcome = Outcome::Ok;
                }
                if ui.button("Cancel").clicked() {
                    *outcome = Outcome::Cancel;
                }
            });
        });
    }
}

fn blank_as_dash(s: &str) -> String {
    if s.trim().is_empty() {
        "\u{2014}".to_string()
    } else {
        s.to_string()
    }
}

/// The status bar line after an import.
pub fn status_line(report: &ApplyReport, source: &str) -> String {
    let n = report.applied;
    let mut s = format!(
        "Imported {n} change{} from {source}",
        if n == 1 { "" } else { "s" }
    );
    if !report.failed.is_empty() {
        s.push_str(&format!("; {} could not be applied", report.failed.len()));
    }
    s
}
