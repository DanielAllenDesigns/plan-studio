//! Drawing Groups (LAY-36): Default Settings > Drawing Groups, where every
//! kind of object gets its group number (the plan draws the lowest group first,
//! so a higher group lies on top), and Edit > Drawing Group > Set Drawing
//! Group, which gives the selected objects a group of their own. See
//! `plan_core::drawing_group`. Each OK is one undo step.

use crate::editor::EditorContext;
use crate::tools::cad_ops;
use eframe::egui;
use plan_core::drawing_group::{DrawingGroupTable, GROUP_MAX, GROUP_MIN, KINDS};
use std::cell::RefCell;

/// The Drawing Groups table being edited.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultsDialog {
    pub table: DrawingGroupTable,
}

impl DefaultsDialog {
    pub fn new(cx: &EditorContext) -> Self {
        Self {
            table: cx.project.drawing_group_defaults.clone(),
        }
    }

    /// OK: the table goes into the plan (one undo step; nothing when it did
    /// not change). Returns whether it changed.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        if cx.project.drawing_group_defaults == self.table {
            return false;
        }
        cx.begin_change("Drawing Group Defaults");
        cx.project.drawing_group_defaults = self.table.clone();
        cx.mark_dirty();
        true
    }

    fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut open = true;
        let mut out = Outcome::Stay;
        egui::Window::new("Drawing Groups")
            .id(egui::Id::new("drawing_groups_defaults"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Objects draw from the lowest group number to the highest.");
                egui::Grid::new("drawing_groups_grid")
                    .num_columns(2)
                    .spacing([16.0, 4.0])
                    .show(ui, |ui| {
                        for (kind, _) in KINDS {
                            ui.label(kind);
                            let mut g = self.table.group_of(kind);
                            if ui
                                .add(egui::DragValue::new(&mut g).range(GROUP_MIN..=GROUP_MAX))
                                .changed()
                            {
                                self.table.set(kind, g);
                            }
                            ui.end_row();
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        out = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        out = Outcome::Cancel;
                    }
                    if ui.button("Reset").clicked() {
                        self.table.reset();
                    }
                });
            });
        if !open {
            out = Outcome::Cancel;
        }
        out
    }
}

/// The Set Drawing Group window.
#[derive(Clone, Debug, PartialEq)]
pub struct SetDialog {
    pub group: i32,
    /// Put the objects back in the group of their kind.
    pub kind_default: bool,
}

impl SetDialog {
    pub fn new(cx: &EditorContext) -> Self {
        let table = &cx.project.drawing_group_defaults;
        let group = cx
            .selection
            .items
            .iter()
            .filter_map(|o| o.to_group_ref())
            .next()
            .map_or(GROUP_MAX / 2, |r| cx.floor().drawing_group(table, r));
        Self {
            group,
            kind_default: false,
        }
    }

    /// OK on the selection of `cx`.
    pub fn apply(&self, cx: &mut EditorContext) -> Result<usize, String> {
        cad_ops::set_drawing_group(cx, (!self.kind_default).then_some(self.group))
    }

    fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut open = true;
        let mut out = Outcome::Stay;
        egui::Window::new("Set Drawing Group")
            .id(egui::Id::new("drawing_groups_set"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.checkbox(&mut self.kind_default, "Use the group of its kind");
                ui.add_enabled_ui(!self.kind_default, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Drawing group");
                        ui.add(egui::DragValue::new(&mut self.group).range(GROUP_MIN..=GROUP_MAX));
                    });
                });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        out = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        out = Outcome::Cancel;
                    }
                });
            });
        if !open {
            out = Outcome::Cancel;
        }
        out
    }
}

enum Outcome {
    Stay,
    Ok,
    Cancel,
}

thread_local! {
    static DEFAULTS: RefCell<Option<DefaultsDialog>> = const { RefCell::new(None) };
    static SET: RefCell<Option<SetDialog>> = const { RefCell::new(None) };
    /// Default Settings > Drawing Groups was asked for; the window opens on
    /// the next frame from the plan's table.
    static WANT_DEFAULTS: RefCell<bool> = const { RefCell::new(false) };
}

/// Asks for Default Settings > Drawing Groups.
pub fn open_defaults() {
    WANT_DEFAULTS.with(|w| *w.borrow_mut() = true);
}

/// Opens Set Drawing Group for the selection.
pub fn open_set(cx: &EditorContext) {
    SET.with(|s| {
        let mut s = s.borrow_mut();
        if s.is_none() {
            *s = Some(SetDialog::new(cx));
        }
    });
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn defaults_open() -> bool {
    DEFAULTS.with(|d| d.borrow().is_some()) || WANT_DEFAULTS.with(|w| *w.borrow())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn set_open() -> bool {
    SET.with(|s| s.borrow().is_some())
}

/// Closes both windows without applying them.
#[cfg_attr(not(test), allow(dead_code))]
pub fn close_all() {
    DEFAULTS.with(|d| *d.borrow_mut() = None);
    SET.with(|s| *s.borrow_mut() = None);
    WANT_DEFAULTS.with(|w| *w.borrow_mut() = false);
}

/// Draws the open windows; the shell calls it once a frame.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if WANT_DEFAULTS.with(|w| std::mem::take(&mut *w.borrow_mut())) {
        DEFAULTS.with(|d| {
            let mut d = d.borrow_mut();
            if d.is_none() {
                *d = Some(DefaultsDialog::new(cx));
            }
        });
    }
    if let Some(mut d) = DEFAULTS.with(|d| d.borrow_mut().take()) {
        match d.show(ctx) {
            Outcome::Stay => DEFAULTS.with(|slot| *slot.borrow_mut() = Some(d)),
            Outcome::Ok => {
                d.apply(cx);
            }
            Outcome::Cancel => {}
        }
    }
    if let Some(mut d) = SET.with(|s| s.borrow_mut().take()) {
        match d.show(ctx) {
            Outcome::Stay => SET.with(|slot| *slot.borrow_mut() = Some(d)),
            Outcome::Ok => {
                if let Err(e) = d.apply(cx) {
                    cx.status = e;
                }
            }
            Outcome::Cancel => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::ObjectRef;
    use crate::plan_defaults;
    use plan_core::cad::CadItem;
    use plan_core::Point;

    fn cx_with_circle() -> (EditorContext, plan_core::Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Circle {
                center: Point::ZERO,
                radius: 10.0,
            },
        );
        cx.selection.set(ObjectRef::Cad(id));
        (cx, id)
    }

    #[test]
    fn ok_on_the_defaults_is_one_undo_step_and_unchanged_is_none() {
        let (mut cx, _) = cx_with_circle();
        let mut d = DefaultsDialog::new(&cx);
        assert!(!d.apply(&mut cx), "nothing changed, no undo step");
        assert!(cx.undo_label().is_none());
        d.table.set("CAD", 60);
        assert!(d.apply(&mut cx));
        assert_eq!(cx.undo_label(), Some("Drawing Group Defaults"));
        assert_eq!(cx.project.drawing_group_defaults.group_of("CAD"), 60);
        cx.undo();
        assert_eq!(cx.project.drawing_group_defaults.group_of("CAD"), 21);
    }

    #[test]
    fn set_drawing_group_starts_at_the_group_of_the_selection() {
        let (mut cx, id) = cx_with_circle();
        let mut d = SetDialog::new(&cx);
        assert_eq!(d.group, 21);
        d.group = 12;
        assert_eq!(d.apply(&mut cx).unwrap(), 1);
        let t = cx.project.drawing_group_defaults.clone();
        let r = plan_core::ObjectRef::Cad(id);
        assert_eq!(cx.floor().drawing_group(&t, r), 12);
        d.kind_default = true;
        assert_eq!(d.apply(&mut cx).unwrap(), 1);
        assert_eq!(cx.floor().drawing_group_override(r), None);
    }

    #[test]
    fn the_windows_draw_headless() {
        let (mut cx, _) = cx_with_circle();
        close_all();
        open_defaults();
        open_set(&cx);
        assert!(defaults_open() && set_open());
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show(ctx, &mut cx));
        }
        assert!(defaults_open() && set_open());
        close_all();
        assert!(!defaults_open() && !set_open());
    }
}
