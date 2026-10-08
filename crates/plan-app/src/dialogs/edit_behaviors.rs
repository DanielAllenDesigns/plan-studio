//! Edit > Edit Behaviors (S-65): what dragging does in the Select tool.
//! Default moves; Resize scales a CAD selection from the opposite corner;
//! Concentric adds offset copies of a polyline, line or round; Fillet rounds
//! a polyline corner you drag; Alternate moves along one axis; Replicate
//! leaves the originals and places copies. The mode and its parameters live in
//! `PlanDefaults::editing.behavior` (see `editor::behaviors`).

use crate::editor::EditorContext;
use eframe::egui;
use plan_core::defaults::{EditBehavior, EditBehaviorSettings};

/// Copies one drag may leave, as in `editor::behaviors`.
const MAX_COPIES: u32 = 50;

pub struct EditBehaviorsDialog {
    pub draft: EditBehaviorSettings,
    fields: super::Fields,
}

/// One line on what a mode does.
pub fn describe(mode: EditBehavior) -> &'static str {
    match mode {
        EditBehavior::Default => "Dragging moves objects and reshapes them in place.",
        EditBehavior::Resize => {
            "Dragging a CAD selection scales it from the corner opposite the one you grab."
        }
        EditBehavior::Concentric => {
            "Dragging a polyline, line, circle or arc leaves it and adds offset copies."
        }
        EditBehavior::Fillet => "Dragging a polyline corner handle rounds that corner.",
        EditBehavior::Alternate => "Dragging moves along the dominant axis only.",
        EditBehavior::Replicate => {
            "Dragging leaves the originals and places copies, each one more drag along."
        }
    }
}

/// Stores the draft as the editing behavior. One place so tests and the
/// dialog agree.
pub fn apply(cx: &mut EditorContext, draft: &EditBehaviorSettings) {
    let mut b = draft.clone();
    b.concentric_copies = b.concentric_copies.clamp(1, MAX_COPIES);
    b.replicate_copies = b.replicate_copies.clamp(1, MAX_COPIES);
    b.concentric_distance = b.concentric_distance.max(0.0);
    b.fillet_radius = b.fillet_radius.max(0.0);
    cx.status = format!("Edit behavior: {}", b.mode.label());
    cx.defaults.editing.behavior = b;
}

impl EditBehaviorsDialog {
    pub fn new(cx: &EditorContext) -> Self {
        Self {
            draft: cx.defaults.editing.behavior.clone(),
            fields: super::Fields::default(),
        }
    }

    /// Draws the window; false once it is closed (OK or Cancel).
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let mut ok = false;
        let mut cancel = false;
        egui::Window::new("Edit Behaviors")
            .id(egui::Id::new("edit_behaviors"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                for mode in EditBehavior::ALL {
                    ui.radio_value(&mut self.draft.mode, mode, mode.label());
                }
                ui.separator();
                ui.weak(describe(self.draft.mode));
                let b = &mut self.draft;
                match b.mode {
                    EditBehavior::Default => {}
                    EditBehavior::Resize => {
                        ui.checkbox(
                            &mut b.resize_proportional,
                            "Keep proportions (or hold Shift)",
                        );
                    }
                    EditBehavior::Concentric => {
                        super::row(ui, "Offset Distance", |ui| {
                            self.fields.length(
                                ui,
                                "concentric_distance",
                                &mut b.concentric_distance,
                            )
                        });
                        ui.weak("0 follows the drag.");
                        super::row(ui, "Copies", |ui| {
                            ui.add(
                                egui::DragValue::new(&mut b.concentric_copies)
                                    .range(1..=MAX_COPIES),
                            );
                        });
                    }
                    EditBehavior::Fillet => {
                        super::row(ui, "Fillet Radius", |ui| {
                            self.fields
                                .length(ui, "fillet_radius", &mut b.fillet_radius)
                        });
                        ui.weak("0 follows the drag.");
                    }
                    EditBehavior::Alternate => {
                        ui.checkbox(&mut b.alternate_lock_axis, "Lock to the dominant axis");
                    }
                    EditBehavior::Replicate => {
                        super::row(ui, "Copies", |ui| {
                            ui.add(
                                egui::DragValue::new(&mut b.replicate_copies).range(1..=MAX_COPIES),
                            );
                        });
                    }
                }
                ui.horizontal(|ui| {
                    let valid = !self.fields.any_invalid();
                    ok = ui.add_enabled(valid, egui::Button::new("OK")).clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        // Enter is OK and Esc is Cancel, as in the other dialogs.
        let (enter, esc) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        let ok = ok || (enter && !self.fields.any_invalid());
        let cancel = cancel || esc;
        if ok {
            apply(cx, &self.draft);
        }
        open && !ok && !cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    #[test]
    fn apply_stores_the_mode_and_clamps_the_counts() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut d = EditBehaviorsDialog::new(&cx);
        assert_eq!(d.draft.mode, EditBehavior::Default);
        d.draft.mode = EditBehavior::Replicate;
        d.draft.replicate_copies = 500;
        d.draft.fillet_radius = -4.0;
        apply(&mut cx, &d.draft);
        let b = &cx.defaults.editing.behavior;
        assert_eq!(b.mode, EditBehavior::Replicate);
        assert_eq!((b.replicate_copies, b.fillet_radius), (50, 0.0));
        assert_eq!(cx.status, "Edit behavior: Replicate");
        // A new dialog starts from what is stored.
        assert_eq!(
            EditBehaviorsDialog::new(&cx).draft.mode,
            EditBehavior::Replicate
        );
    }

    #[test]
    fn every_mode_has_a_description() {
        for m in EditBehavior::ALL {
            assert!(describe(m).len() > 10, "{}", m.label());
        }
    }

    #[test]
    fn the_window_draws_for_every_mode() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        for m in EditBehavior::ALL {
            let mut d = EditBehaviorsDialog::new(&cx);
            d.draft.mode = m;
            let ctx = egui::Context::default();
            let mut open = false;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                open = d.show(ctx, &mut cx);
            });
            assert!(open);
        }
    }
}
