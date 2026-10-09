//! Construction Line Order Management (CAD-65; manual pp. 84-85): the rule
//! sets that number construction lines automatically.
//!
//! The list on the left holds the plan's rule sets, highest priority first
//! (a check box makes a set usable); Add, Copy, Delete, Increase and
//! Decrease Priority manage it. The Rules below name the selected set, the
//! view type and line angle it applies to, the count format and whether it
//! numbers in the reverse direction. System rule sets cannot be deleted and
//! keep their name, view type and angle. OK is one undo step.

use super::{row, section, Fields, Outcome};
use crate::editor::EditorContext;
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, RichText};
use plan_core::construction::{ConstructionSettings, CountFormat, RuleSet, ViewType};
use std::cell::RefCell;

thread_local! {
    static HOST: RefCell<Option<OrderDialog>> = const { RefCell::new(None) };
}

/// The Order Management dialog on a draft of the plan's rule sets.
pub struct OrderDialog {
    settings: ConstructionSettings,
    selected: usize,
    fields: Fields,
}

// The accessors are the dialog's model API (the tests drive it); the UI
// edits the same fields directly.
#[allow(dead_code)]
impl OrderDialog {
    pub fn new(settings: ConstructionSettings) -> Self {
        Self {
            settings,
            selected: 0,
            fields: Fields::default(),
        }
    }

    pub fn rule_sets(&self) -> &[RuleSet] {
        &self.settings.rule_sets
    }

    pub fn rule_sets_mut(&mut self) -> &mut Vec<RuleSet> {
        &mut self.settings.rule_sets
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select(&mut self, i: usize) {
        self.selected = i.min(self.settings.rule_sets.len().saturating_sub(1));
    }

    pub fn add(&mut self) {
        self.selected = self.settings.add_rule_set();
    }

    pub fn copy(&mut self) {
        if let Some(i) = self.settings.copy_rule_set(self.selected) {
            self.selected = i;
        }
    }

    pub fn can_delete(&self) -> bool {
        self.settings
            .rule_sets
            .get(self.selected)
            .is_some_and(|r| !r.system)
    }

    pub fn delete(&mut self) {
        if self.settings.delete_rule_set(self.selected) {
            self.select(self.selected);
        }
    }

    pub fn increase_priority(&mut self) {
        if let Some(i) = self.settings.raise_priority(self.selected) {
            self.selected = i;
        }
    }

    pub fn decrease_priority(&mut self) {
        if let Some(i) = self.settings.lower_priority(self.selected) {
            self.selected = i;
        }
    }

    /// A reason OK is not allowed.
    pub fn error(&self) -> Option<String> {
        (0..self.settings.rule_sets.len())
            .find_map(|i| self.settings.name_error(i))
            .or_else(|| {
                self.fields
                    .any_invalid()
                    .then(|| "A number is not valid".to_string())
            })
    }

    /// Draws the dialog; Enter is OK and Esc is Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = self.error();
        egui::Window::new("Construction Line Order Management")
            .id(egui::Id::new("construction_order_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(520.0);
                section(ui, "Rule Sets");
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(260.0);
                        egui::ScrollArea::vertical()
                            .id_salt("order_sets")
                            .max_height(150.0)
                            .show(ui, |ui| {
                                for i in 0..self.settings.rule_sets.len() {
                                    ui.horizontal(|ui| {
                                        let r = &mut self.settings.rule_sets[i];
                                        ui.checkbox(&mut r.enabled, "");
                                        let name = r.name.clone();
                                        if ui.selectable_label(self.selected == i, name).clicked() {
                                            self.selected = i;
                                        }
                                    });
                                }
                            });
                    });
                    ui.vertical(|ui| {
                        if ui.button("Add").clicked() {
                            self.add();
                        }
                        if ui.button("Copy").clicked() {
                            self.copy();
                        }
                        if ui
                            .add_enabled(self.can_delete(), egui::Button::new("Delete"))
                            .clicked()
                        {
                            self.delete();
                        }
                        if ui
                            .add_enabled(self.selected > 0, egui::Button::new("Increase Priority"))
                            .clicked()
                        {
                            self.increase_priority();
                        }
                        if ui
                            .add_enabled(
                                self.selected + 1 < self.settings.rule_sets.len(),
                                egui::Button::new("Decrease Priority"),
                            )
                            .clicked()
                        {
                            self.decrease_priority();
                        }
                    });
                });
                section(ui, "Rules");
                self.rules_ui(ui);
                ui.weak("Lines at the same position (collinear) share an order number.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if let Some(e) = &error {
                        ui.colored_label(egui::Color32::from_rgb(0xFF, 0x7B, 0x7B), e);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(error.is_none(), egui::Button::new(RichText::new("   OK   ").strong()))
                            .clicked()
                        {
                            outcome = Outcome::Ok;
                        }
                        if ui.button("Cancel").clicked() {
                            outcome = Outcome::Cancel;
                        }
                    });
                });
            });
        if !open {
            outcome = Outcome::Cancel;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        } else if error.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            outcome = Outcome::Ok;
        }
        outcome
    }

    fn rules_ui(&mut self, ui: &mut egui::Ui) {
        let Some(r) = self.settings.rule_sets.get_mut(self.selected) else {
            return;
        };
        let locked = r.system;
        row(ui, "Rule Set Name", |ui| {
            ui.add_enabled(
                !locked,
                egui::TextEdit::singleline(&mut r.name).desired_width(200.0),
            );
        });
        row(ui, "View Type", |ui| {
            ui.add_enabled_ui(!locked, |ui| {
                egui::ComboBox::from_id_salt("order_view")
                    .selected_text(r.view.label())
                    .show_ui(ui, |ui| {
                        for v in [ViewType::Plan, ViewType::Elevation] {
                            ui.selectable_value(&mut r.view, v, v.label());
                        }
                    });
            });
        });
        ui.add_enabled_ui(!locked, |ui| {
            self.fields
                .degrees_row(ui, "Line Angle", "deg_order_angle", &mut r.angle_deg);
        });
        row(ui, "Count Format", |ui| {
            egui::ComboBox::from_id_salt("order_format")
                .selected_text(r.format.label())
                .show_ui(ui, |ui| {
                    for f in CountFormat::ALL {
                        ui.selectable_value(&mut r.format, f, f.label());
                    }
                });
        });
        ui.checkbox(&mut r.reverse, "Reverse Order Direction")
            .on_hover_text("Number from right to left and from down to up");
    }
}

/// Applies an accepted draft as one undo step.
pub fn apply(cx: &mut EditorContext, d: &OrderDialog) {
    cx.begin_change("Construction Line Order Management");
    cx.project.construction.rule_sets = d.settings.rule_sets.clone();
    cx.mark_dirty();
    cx.status = "Updated the construction line rule sets".into();
}

/// Opens the dialog on the plan's rule sets.
pub fn open(cx: &EditorContext) {
    let d = OrderDialog::new(cx.project.construction.clone());
    HOST.with(|h| *h.borrow_mut() = Some(d));
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn dialog_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

/// Test access to the open dialog.
#[cfg(test)]
pub fn with_dialog<R>(f: impl FnOnce(&mut OrderDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

/// Closes the open dialog and applies it as OK would.
#[cfg(test)]
pub fn accept_dialog(cx: &mut EditorContext) -> bool {
    let Some(d) = HOST.with(|h| h.borrow_mut().take()) else {
        return false;
    };
    if d.error().is_some() {
        return false;
    }
    apply(cx, &d);
    true
}

/// Shows the open dialog once a frame and applies its OK.
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => HOST.with(|h| *h.borrow_mut() = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => apply(cx, &d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::construction::order_labels;
    use plan_core::geometry::Point;

    #[test]
    fn the_dialog_manages_rule_sets_like_the_manual_says() {
        let mut d = OrderDialog::new(ConstructionSettings::default());
        assert_eq!(d.rule_sets().len(), 4);
        assert!(!d.can_delete(), "a system rule set");
        d.add();
        assert_eq!(d.selected(), 4);
        assert!(d.can_delete());
        d.copy();
        assert_eq!(d.rule_sets()[5].name, "New Rule Set Copy");
        d.increase_priority();
        assert_eq!(d.selected(), 4);
        assert_eq!(d.rule_sets()[4].name, "New Rule Set Copy");
        d.decrease_priority();
        d.delete();
        assert_eq!(d.rule_sets().len(), 5);
        // A duplicate name blocks OK.
        d.rule_sets_mut()[4].name = "Plan Vertical".into();
        assert!(d.error().is_some());
        d.rule_sets_mut()[4].name = "Mine".into();
        assert!(d.error().is_none());
    }

    #[test]
    fn ok_changes_the_plans_numbering_in_one_undo_step() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let a = cx
            .project
            .add_construction_line(0, Point::new(100.0, 0.0), Point::new(100.0, 50.0), None)
            .unwrap();
        let b = cx
            .project
            .add_construction_line(0, Point::new(300.0, 0.0), Point::new(300.0, 50.0), None)
            .unwrap();
        let before = cx.project.construction_order(0, ViewType::Plan);
        assert_eq!((before[&a].as_str(), before[&b].as_str()), ("1", "2"));
        open(&cx);
        assert!(dialog_open());
        with_dialog(|d| {
            d.rule_sets_mut()[0].reverse = true;
            d.rule_sets_mut()[0].format = CountFormat::UpperRoman;
        })
        .unwrap();
        assert!(accept_dialog(&mut cx));
        let after = cx.project.construction_order(0, ViewType::Plan);
        assert_eq!((after[&a].as_str(), after[&b].as_str()), ("II", "I"));
        assert_eq!(
            cx.undo_label(),
            Some("Construction Line Order Management")
        );
        cx.undo();
        assert_eq!(cx.project.construction_order(0, ViewType::Plan)[&a], "1");
        // The labels the plan draws are the ones the rule sets give.
        let lines = [plan_core::construction::OrderLine {
            id: a,
            a: Point::new(100.0, 0.0),
            b: Point::new(100.0, 50.0),
        }];
        assert_eq!(
            order_labels(&lines, ViewType::Plan, &cx.project.construction.rule_sets)[&a],
            "1"
        );
    }
}
