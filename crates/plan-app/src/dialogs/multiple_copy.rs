//! Edit > Multiple Copy: copies of the selection (walls, CAD, cabinets, every
//! kind the clipboard carries) at an even offset. The offset is typed as X and
//! Y or as a distance and an angle; it is either the step from one copy to the
//! next or the total to the last copy. Each copy can also turn a little more
//! than the one before. "Drag in Plan" does the same with two clicks and a
//! tick for every copy. Make Copies is one undo step.

use crate::editor::EditorContext;
use crate::tools::cad_ops::{self, MultipleCopy, MAX_COPIES};
use eframe::egui;
use plan_core::geometry::Point;
use plan_core::units::parse_ft_in;
use std::cell::RefCell;

/// The dialog's fields; lengths are text so feet and inches can be typed.
#[derive(Clone, Debug, PartialEq)]
pub struct MultipleCopyDialog {
    pub count: u32,
    /// Distance and angle instead of X and Y.
    pub polar: bool,
    pub x: String,
    pub y: String,
    pub dist: String,
    /// Counter-clockwise degrees from the X axis.
    pub angle: f64,
    /// The offset is the total to the last copy, not the step between copies.
    pub total: bool,
    /// Degrees each copy turns more than the one before.
    pub turn: f64,
    pub message: String,
}

impl Default for MultipleCopyDialog {
    fn default() -> Self {
        let last = cad_ops::copy_settings();
        Self {
            count: last.count,
            polar: false,
            x: format_len(last.step.x),
            y: format_len(last.step.y),
            dist: "4'".into(),
            angle: 0.0,
            total: false,
            turn: last.turn_deg,
            message: String::new(),
        }
    }
}

fn format_len(inches: f64) -> String {
    plan_core::units::fmt_ft_in(inches)
}

fn len(s: &str) -> f64 {
    parse_ft_in(s).unwrap_or(0.0)
}

impl MultipleCopyDialog {
    /// The values the fields stand for.
    pub fn values(&self) -> MultipleCopy {
        let offset = if self.polar {
            let a = self.angle.to_radians();
            let d = len(&self.dist);
            Point::new(d * a.cos(), d * a.sin())
        } else {
            Point::new(len(&self.x), len(&self.y))
        };
        let count = self.count.max(1);
        MultipleCopy {
            count,
            step: if self.total {
                offset.scale(1.0 / f64::from(count))
            } else {
                offset
            },
            turn_deg: self.turn,
        }
    }

    /// Make Copies on the selection of `cx`.
    pub fn apply(&mut self, cx: &mut EditorContext) -> bool {
        let v = self.values();
        cad_ops::set_copy_settings(v.clone());
        match cad_ops::multiple_copy(cx, &v) {
            Ok(msg) => {
                self.message = msg;
                true
            }
            Err(e) => {
                self.message = e;
                false
            }
        }
    }

    /// Draws the window; false once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let (mut copy, mut drag, mut close) = (false, false, false);
        egui::Window::new("Multiple Copy")
            .id(egui::Id::new("multiple_copy"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("multiple_copy_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Number of copies");
                        ui.add(egui::DragValue::new(&mut self.count).range(1..=MAX_COPIES));
                        ui.end_row();
                        ui.label("Offset");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.polar, false, "X, Y");
                            ui.radio_value(&mut self.polar, true, "Distance, angle");
                        });
                        ui.end_row();
                        ui.label("");
                        ui.horizontal(|ui| {
                            if self.polar {
                                ui.label("Distance");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.dist).desired_width(70.0),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut self.angle)
                                        .speed(0.5)
                                        .suffix(" deg"),
                                );
                            } else {
                                ui.label("X");
                                ui.add(egui::TextEdit::singleline(&mut self.x).desired_width(70.0));
                                ui.label("Y");
                                ui.add(egui::TextEdit::singleline(&mut self.y).desired_width(70.0));
                            }
                        });
                        ui.end_row();
                        ui.label("");
                        ui.checkbox(&mut self.total, "Offset to the last copy");
                        ui.end_row();
                        ui.label("Turn each copy");
                        ui.add(
                            egui::DragValue::new(&mut self.turn)
                                .speed(0.5)
                                .suffix(" deg"),
                        );
                        ui.end_row();
                    });
                ui.separator();
                if !self.message.is_empty() {
                    ui.label(&self.message);
                }
                ui.horizontal(|ui| {
                    copy = ui.button("Make Copies").clicked();
                    drag = ui.button("Drag in Plan").clicked();
                    close = ui.button("Close").clicked();
                });
            });
        if copy && self.apply(cx) {
            close = true;
        }
        if drag {
            let v = self.values();
            cad_ops::set_copy_settings(MultipleCopy {
                step: Point::new(0.0, 0.0),
                ..v
            });
            cx.run_custom(cad_ops::MULTIPLE_COPY_DRAG);
            close = true;
        }
        open && !close
    }
}

thread_local! {
    static DIALOG: RefCell<Option<MultipleCopyDialog>> = const { RefCell::new(None) };
}

/// Opens the window (if it is not open).
pub fn open() {
    DIALOG.with(|d| {
        let mut d = d.borrow_mut();
        if d.is_none() {
            *d = Some(MultipleCopyDialog::default());
        }
    });
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn is_open() -> bool {
    DIALOG.with(|d| d.borrow().is_some())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn close() {
    DIALOG.with(|d| *d.borrow_mut() = None);
}

/// Draws the window when it is open; the shell calls it once a frame.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) {
        if d.show(ctx, cx) {
            DIALOG.with(|slot| {
                if slot.borrow().is_none() {
                    *slot.borrow_mut() = Some(d);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::ObjectRef;
    use crate::plan_defaults;
    use plan_core::{Point, WallKind};

    fn cx_with_wall() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        cx.selection.set(ObjectRef::Wall(w));
        cx
    }

    #[test]
    fn the_fields_become_the_values_and_total_divides_by_the_count() {
        let mut d = MultipleCopyDialog {
            count: 4,
            x: "0\"".into(),
            y: "8'".into(),
            ..MultipleCopyDialog::default()
        };
        let v = d.values();
        assert_eq!((v.count, v.step), (4, Point::new(0.0, 96.0)));
        d.total = true;
        assert_eq!(d.values().step, Point::new(0.0, 24.0));
        d.polar = true;
        d.dist = "10'".into();
        d.angle = 90.0;
        d.total = false;
        let s = d.values().step;
        assert!(s.x.abs() < 1e-9 && (s.y - 120.0).abs() < 1e-9);
    }

    #[test]
    fn make_copies_is_one_undo_step_and_reports_refusals() {
        let mut cx = cx_with_wall();
        let mut d = MultipleCopyDialog {
            count: 2,
            x: "0\"".into(),
            y: "5'".into(),
            ..MultipleCopyDialog::default()
        };
        assert!(d.apply(&mut cx));
        assert_eq!(cx.floor().walls.len(), 3);
        assert_eq!(cx.undo_label(), Some("Multiple Copy"));
        assert!(d.message.contains("2 copies"), "{}", d.message);
        cx.undo();
        assert_eq!(cx.floor().walls.len(), 1);
        d.x = "0\"".into();
        d.y = "0\"".into();
        assert!(!d.apply(&mut cx));
        assert!(d.message.contains("land on the original"));
    }

    #[test]
    fn the_window_opens_draws_and_closes() {
        let mut cx = cx_with_wall();
        close();
        assert!(!is_open());
        open();
        assert!(is_open());
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show(ctx, &mut cx));
        }
        assert!(is_open());
        close();
    }
}
