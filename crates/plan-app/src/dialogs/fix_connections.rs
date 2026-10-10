//! Fix Off Angle Wall (W-133, manual p. 386): shows the wall's angle, takes
//! the angle it should have and which part stays where it is (start, center
//! or end). The window is hosted here; `build_tools::show_all` draws it every
//! frame and `editor::wall_edit` (the Edit toolbar button, the Caution menu)
//! opens it through [`open`].

use crate::editor::{wall_edit, EditorContext};
use eframe::egui;
use plan_core::wall_repair::{wall_angle_deg, FixLock};
use plan_core::Id;
use std::cell::RefCell;

/// The dialog's draft values.
pub struct FixOffAngleDialog {
    pub wall: Id,
    /// The wall's angle now, degrees.
    pub old_angle: f64,
    /// The angle to give it, degrees.
    pub new_angle: f64,
    pub lock: FixLock,
}

impl FixOffAngleDialog {
    /// The dialog for `wall`: the new angle starts at the nearest allowed one.
    pub fn new(cx: &EditorContext, wall: Id) -> Option<Self> {
        let w = cx.floor().wall(wall)?;
        let old = wall_angle_deg(w);
        let new = wall_edit::off_angle_target(cx, wall).unwrap_or(old);
        Some(Self {
            wall,
            old_angle: old,
            new_angle: new,
            lock: FixLock::Center,
        })
    }

    /// OK: turns the wall (one undo step).
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        wall_edit::fix_off_angle(cx, self.wall, self.new_angle, self.lock)
    }

    /// Draws the window; false once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let (mut ok, mut cancel) = (false, false);
        egui::Window::new("Fix Off Angle Wall")
            .id(egui::Id::new("fix_off_angle"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("fix_off_angle_grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Old angle");
                        ui.label(format!("{:.2}\u{b0}", self.old_angle));
                        ui.end_row();
                        ui.label("New angle");
                        ui.add(
                            egui::DragValue::new(&mut self.new_angle)
                                .speed(0.1)
                                .suffix("\u{b0}"),
                        );
                        ui.end_row();
                    });
                ui.label("Keep in place");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.lock, FixLock::Start, "Start");
                    ui.radio_value(&mut self.lock, FixLock::Center, "Center");
                    ui.radio_value(&mut self.lock, FixLock::End, "End");
                });
                ui.horizontal(|ui| {
                    ok = ui.button("OK").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if ok {
            self.apply(cx);
        }
        open && !ok && !cancel
    }
}

thread_local! {
    static WINDOW: RefCell<Option<FixOffAngleDialog>> = const { RefCell::new(None) };
}

/// Opens the dialog for wall `id` (Fix Off Angle Wall).
pub fn open(cx: &mut EditorContext, id: Id) {
    let d = FixOffAngleDialog::new(cx, id);
    WINDOW.with(|w| *w.borrow_mut() = d);
}

/// Draws the open dialog, if any (called every frame).
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut d) = WINDOW.with(|w| w.borrow_mut().take()) else {
        return;
    };
    if d.show(ctx, cx) {
        WINDOW.with(|w| *w.borrow_mut() = Some(d));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::selection::ObjectRef;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::WallKind;

    #[test]
    fn the_dialog_proposes_the_allowed_angle_and_one_undo_step_fixes_it() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 8.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        cx.selection.set(ObjectRef::Wall(id));
        let mut d = FixOffAngleDialog::new(&cx, id).unwrap();
        assert!(d.old_angle > 1.0 && d.old_angle < 3.0);
        assert!(d.new_angle.abs() < 1e-9, "{}", d.new_angle);
        d.lock = FixLock::Start;
        assert!(d.apply(&mut cx));
        assert_eq!(cx.undo_label(), Some("Fix Off Angle Wall"));
        let w = cx.floor().wall(id).unwrap();
        assert_eq!(w.start, Point::new(0.0, 0.0));
        assert!(w.end.y.abs() < 1e-6);
        // One undo step brings the off-angle wall back.
        cx.undo();
        assert!((cx.floor().wall(id).unwrap().end.y - 8.0).abs() < 1e-9);
    }
}
