//! View > Action History (S-79): the undo steps by name, oldest first, with
//! the redo steps below a rule. Clicking a step goes to the plan as it was
//! after that step.

use crate::editor::EditorContext;
use eframe::egui;
use std::cell::Cell;

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
}

pub fn is_open() -> bool {
    OPEN.with(Cell::get)
}

pub fn toggle() {
    OPEN.with(|o| o.set(!o.get()));
}

#[cfg(test)]
pub fn open() {
    OPEN.with(|o| o.set(true));
}

/// What a click on a row asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Jump {
    /// Go back to the state after undo step `n` (0 = the oldest).
    Back(usize),
    /// Redo this many steps.
    Forward(usize),
}

/// Runs a click on a history row. Returns how many steps went.
pub fn jump(cx: &mut EditorContext, to: Jump) -> usize {
    match to {
        Jump::Back(i) => cx.jump_back_to(i),
        Jump::Forward(n) => cx.jump_forward(n),
    }
}

pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if !is_open() {
        return;
    }
    let mut open = true;
    let (past, future) = cx.action_history();
    let mut clicked: Option<Jump> = None;
    egui::Window::new("Action History")
        .id(egui::Id::new("action_history"))
        .open(&mut open)
        .default_width(240.0)
        .show(ctx, |ui| {
            if past.is_empty() && future.is_empty() {
                ui.weak("Nothing has been done yet.");
            }
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    let last = past.len().saturating_sub(1);
                    for (i, label) in past.iter().enumerate() {
                        let r = ui.selectable_label(i == last, format!("{}. {label}", i + 1));
                        if r.clicked() && i != last {
                            clicked = Some(Jump::Back(i));
                        }
                    }
                    if !future.is_empty() {
                        ui.separator();
                        ui.weak("Undone:");
                    }
                    for (j, label) in future.iter().enumerate() {
                        let r = ui.add(
                            egui::Label::new(egui::RichText::new(label).weak())
                                .sense(egui::Sense::click()),
                        );
                        if r.clicked() {
                            clicked = Some(Jump::Forward(j + 1));
                        }
                    }
                });
        });
    if let Some(j) = clicked {
        jump(cx, j);
    }
    if !open {
        OPEN.with(|o| o.set(false));
    }
}
