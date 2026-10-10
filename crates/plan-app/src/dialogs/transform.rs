//! Edit > Transform/Replicate Object (S-47, S-103, S-104): copies, move,
//! rotate, resize and reflect of the selection in one window. Apply is one
//! undo step. The Edit windows (this one, Delete Objects, Send to Layer and
//! the Action History) share [`show_edit_windows`], which the shell calls
//! once a frame.

use crate::editor::transform::{self as xf, ReflectAxis, TransformParams};
use crate::editor::EditorContext;
use eframe::egui;
use plan_core::geometry::Point;
use plan_core::units::parse_ft_in;
use std::cell::RefCell;

/// The dialog's fields, as text where the user types lengths.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformDialog {
    pub make_copies: bool,
    pub copies: u32,
    pub move_x: String,
    pub move_y: String,
    /// Move by a distance and an angle instead of X and Y.
    pub move_polar: bool,
    pub move_dist: String,
    /// Counter-clockwise degrees from the X axis.
    pub move_angle: f64,
    pub rotate_deg: f64,
    pub about_point: bool,
    pub about_x: String,
    pub about_y: String,
    pub resize_pct: f64,
    pub reflect: bool,
    /// `true`: mirror about a vertical line (x = value).
    pub reflect_vertical: bool,
    /// Put the mirror line through the center of the selection.
    pub reflect_center: bool,
    pub reflect_at: String,
    /// Mirror about the line through two points instead of an X or Y line.
    pub reflect_line: bool,
    pub line_ax: String,
    pub line_ay: String,
    pub line_bx: String,
    pub line_by: String,
    /// Move: X, Y is where the selection's center goes, not a distance.
    pub move_to: bool,
    /// Move: measure X, Y along the object's own direction, not the plan's.
    pub move_own_axes: bool,
    /// What the last Apply said.
    pub message: String,
}

impl Default for TransformDialog {
    fn default() -> Self {
        Self {
            make_copies: false,
            copies: 1,
            move_x: "0\"".into(),
            move_y: "0\"".into(),
            move_polar: false,
            move_dist: "0\"".into(),
            move_angle: 0.0,
            rotate_deg: 0.0,
            about_point: false,
            about_x: "0\"".into(),
            about_y: "0\"".into(),
            resize_pct: 100.0,
            reflect: false,
            reflect_vertical: true,
            reflect_center: true,
            reflect_at: "0\"".into(),
            reflect_line: false,
            line_ax: "0\"".into(),
            line_ay: "0\"".into(),
            line_bx: "0\"".into(),
            line_by: "10'".into(),
            move_to: false,
            move_own_axes: false,
            message: String::new(),
        }
    }
}

fn len(s: &str) -> f64 {
    parse_ft_in(s).unwrap_or(0.0)
}

impl TransformDialog {
    /// The parameters the fields stand for; `center` is the center of the
    /// selection (the default pivot and mirror line).
    pub fn params(&self, center: Point) -> TransformParams {
        let reflect = self.reflect.then(|| {
            let at = |c: f64| {
                if self.reflect_center {
                    c
                } else {
                    len(&self.reflect_at)
                }
            };
            if self.reflect_line {
                ReflectAxis::Line(
                    Point::new(len(&self.line_ax), len(&self.line_ay)),
                    Point::new(len(&self.line_bx), len(&self.line_by)),
                )
            } else if self.reflect_vertical {
                ReflectAxis::Vertical(at(center.x))
            } else {
                ReflectAxis::Horizontal(at(center.y))
            }
        });
        let (move_x, move_y) = if self.move_polar {
            let (d, a) = (len(&self.move_dist), self.move_angle.to_radians());
            (d * a.cos(), d * a.sin())
        } else {
            (len(&self.move_x), len(&self.move_y))
        };
        TransformParams {
            copies: if self.make_copies { self.copies } else { 0 },
            move_x,
            move_y,
            rotate_deg: self.rotate_deg,
            rotate_about: self
                .about_point
                .then(|| Point::new(len(&self.about_x), len(&self.about_y))),
            resize: self.resize_pct / 100.0,
            reflect,
            move_to: self.move_to && !self.move_polar,
            move_frame: 0.0,
        }
    }

    /// Apply: runs the transform on the selection of `cx`.
    pub fn apply(&mut self, cx: &mut EditorContext) -> bool {
        let Some(center) = xf::selection_center(cx) else {
            self.message = "Select objects to transform".into();
            return false;
        };
        let mut params = self.params(center);
        if self.move_own_axes && !params.move_to {
            params.move_frame = xf::selection_angle(cx);
        }
        match xf::transform_replicate(cx, &params) {
            Ok(msg) => {
                cx.status = msg.clone();
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
        let mut apply = false;
        let mut close = false;
        egui::Window::new("Transform/Replicate Object")
            .id(egui::Id::new("transform_replicate"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.make_copies, "Make copies");
                    ui.add_enabled(
                        self.make_copies,
                        egui::DragValue::new(&mut self.copies).range(1..=500),
                    );
                    ui.label("Number of copies");
                });
                ui.separator();
                egui::Grid::new("transform_grid")
                    .num_columns(3)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Move");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.move_polar, false, "X, Y");
                            ui.radio_value(&mut self.move_polar, true, "Distance, angle");
                        });
                        ui.end_row();
                        ui.label("");
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(!self.move_polar, |ui| {
                                ui.checkbox(&mut self.move_to, "Move center to");
                                ui.add_enabled(
                                    !self.move_to,
                                    egui::Checkbox::new(&mut self.move_own_axes, "Along itself"),
                                );
                            });
                        });
                        ui.end_row();
                        ui.label("");
                        ui.horizontal(|ui| {
                            if self.move_polar {
                                ui.label("Distance");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.move_dist)
                                        .desired_width(70.0),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut self.move_angle)
                                        .speed(0.5)
                                        .suffix(" deg"),
                                );
                            } else {
                                ui.label("X");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.move_x)
                                        .desired_width(70.0),
                                );
                                ui.label("Y");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.move_y)
                                        .desired_width(70.0),
                                );
                            }
                        });
                        ui.end_row();
                        ui.label("Rotate");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut self.rotate_deg)
                                    .speed(0.5)
                                    .suffix(" deg"),
                            );
                            ui.checkbox(&mut self.about_point, "About point");
                        });
                        ui.end_row();
                        ui.label("");
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(self.about_point, |ui| {
                                ui.label("X");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.about_x)
                                        .desired_width(70.0),
                                );
                                ui.label("Y");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.about_y)
                                        .desired_width(70.0),
                                );
                            });
                            if !self.about_point {
                                ui.weak("(center of the selection)");
                            }
                        });
                        ui.end_row();
                        ui.label("Resize");
                        ui.add(
                            egui::DragValue::new(&mut self.resize_pct)
                                .range(1.0..=1000.0)
                                .speed(0.5)
                                .suffix("%"),
                        );
                        ui.end_row();
                        ui.label("Reflect");
                        ui.checkbox(&mut self.reflect, "Mirror the objects");
                        ui.end_row();
                    });
                ui.add_enabled_ui(self.reflect, |ui| {
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.reflect_vertical, true, "About X line");
                        ui.radio_value(&mut self.reflect_vertical, false, "About Y line");
                        ui.checkbox(&mut self.reflect_line, "About a line");
                    });
                    if self.reflect_line {
                        ui.horizontal(|ui| {
                            for (l, f) in [
                                ("From X", &mut self.line_ax),
                                ("Y", &mut self.line_ay),
                                ("To X", &mut self.line_bx),
                                ("Y", &mut self.line_by),
                            ] {
                                ui.label(l);
                                ui.add(egui::TextEdit::singleline(f).desired_width(56.0));
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.reflect_center, "Through the center");
                        ui.add_enabled(
                            !self.reflect_center,
                            egui::TextEdit::singleline(&mut self.reflect_at).desired_width(80.0),
                        );
                    });
                });
                ui.separator();
                if !self.message.is_empty() {
                    ui.label(&self.message);
                }
                ui.horizontal(|ui| {
                    apply = ui.button("Apply").clicked();
                    close = ui.button("Close").clicked();
                });
            });
        if apply {
            self.apply(cx);
        }
        open && !close
    }
}

/// Edit > Align/Distribute (S-54): buttons that line up or space out the
/// selected objects, with an optional fixed gap for Distribute.
#[derive(Clone, Debug, Default)]
pub struct AlignDialog {
    pub use_gap: bool,
    pub gap: String,
    pub message: String,
}

impl AlignDialog {
    /// Runs Align `mode` on the selection of `cx`.
    pub fn align(&mut self, cx: &mut EditorContext, mode: plan_core::transform::AlignMode) {
        self.message = match xf::align_selection(cx, mode) {
            Ok(n) => format!("Aligned {n} object{}", if n == 1 { "" } else { "s" }),
            Err(e) => e,
        };
    }

    /// Runs Distribute along `axis`; with `use_gap` the spacing is the typed
    /// gap instead of an even one.
    pub fn distribute(&mut self, cx: &mut EditorContext, axis: plan_core::transform::Axis) {
        let gap = self.use_gap.then(|| len(&self.gap));
        self.message = match xf::distribute_selection(cx, axis, gap) {
            Ok(n) => format!("Moved {n} object{}", if n == 1 { "" } else { "s" }),
            Err(e) => e,
        };
    }

    /// Draws the window; false once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        use plan_core::transform::{AlignMode, Axis};
        let mut open = true;
        let mut clicked: Option<AlignMode> = None;
        let mut spread: Option<Axis> = None;
        egui::Window::new("Align/Distribute")
            .id(egui::Id::new("align_distribute"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Align the selected objects");
                ui.horizontal_wrapped(|ui| {
                    for m in AlignMode::ALL {
                        if ui.button(m.label().trim_start_matches("Align ")).clicked() {
                            clicked = Some(m);
                        }
                    }
                });
                ui.separator();
                ui.label("Distribute with equal gaps");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.use_gap, "Spacing");
                    ui.add_enabled(
                        self.use_gap,
                        egui::TextEdit::singleline(&mut self.gap).desired_width(70.0),
                    );
                });
                ui.horizontal(|ui| {
                    if ui.button("Horizontally").clicked() {
                        spread = Some(Axis::Horizontal);
                    }
                    if ui.button("Vertically").clicked() {
                        spread = Some(Axis::Vertical);
                    }
                });
                if !self.message.is_empty() {
                    ui.separator();
                    ui.label(&self.message);
                }
            });
        if let Some(m) = clicked {
            self.align(cx, m);
        }
        if let Some(a) = spread {
            self.distribute(cx, a);
        }
        open
    }
}

thread_local! {
    static DIALOG: RefCell<Option<TransformDialog>> = const { RefCell::new(None) };
    static ALIGN: RefCell<Option<AlignDialog>> = const { RefCell::new(None) };
}

/// Opens the Align/Distribute window.
pub fn open_align(cx: &mut EditorContext) {
    if cx.selection.len() < 2 {
        cx.status = "Select two or more objects to align".into();
        return;
    }
    ALIGN.with(|d| {
        let mut d = d.borrow_mut();
        if d.is_none() {
            *d = Some(AlignDialog::default());
        }
    });
}

/// Opens the dialog for the selection (nothing happens without one).
pub fn open(cx: &mut EditorContext) {
    if cx.selection.is_empty() {
        cx.status = "Select objects to transform".into();
        return;
    }
    DIALOG.with(|d| {
        let mut d = d.borrow_mut();
        if d.is_none() {
            *d = Some(TransformDialog::default());
        }
    });
}

/// Opens the dialog set up to place `copies` copies, each `delta` further
/// along than the last (the hand-off of the Replicate edit behavior).
pub fn open_replicate(cx: &mut EditorContext, delta: Point, copies: u32) {
    if cx.selection.is_empty() {
        cx.status = "Select objects to replicate".into();
        return;
    }
    let mut d = TransformDialog {
        make_copies: true,
        copies: copies.max(1),
        move_x: plan_core::units::fmt_ft_in(delta.x),
        move_y: plan_core::units::fmt_ft_in(delta.y),
        ..TransformDialog::default()
    };
    d.message = "Replicate: adjust the move and the copy count, then Apply".into();
    DIALOG.with(|slot| *slot.borrow_mut() = Some(d));
}

/// Opens the dialog set up to rotate by a quarter turn.
pub fn open_rotate(cx: &mut EditorContext) {
    open(cx);
    DIALOG.with(|d| {
        if let Some(d) = d.borrow_mut().as_mut() {
            if d.rotate_deg == 0.0 {
                d.rotate_deg = 90.0;
            }
        }
    });
}

#[cfg(test)]
pub fn is_open() -> bool {
    DIALOG.with(|d| d.borrow().is_some())
}

/// Closes the window (tests).
#[cfg(test)]
pub fn close_for_tests() {
    DIALOG.with(|d| *d.borrow_mut() = None);
}

/// The open Transform/Replicate window's fields (tests).
#[cfg(test)]
pub fn current() -> Option<TransformDialog> {
    DIALOG.with(|d| d.borrow().clone())
}

/// Draws every open Edit window; the shell calls it once a frame.
pub fn show_edit_windows(ctx: &egui::Context, cx: &mut EditorContext) {
    if let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) {
        if d.show(ctx, cx) {
            DIALOG.with(|slot| *slot.borrow_mut() = Some(d));
        }
    }
    if let Some(mut d) = ALIGN.with(|d| d.borrow_mut().take()) {
        if d.show(ctx, cx) {
            ALIGN.with(|slot| *slot.borrow_mut() = Some(d));
        }
    }
    super::delete_objects::show(ctx, cx);
    super::send_to_layer::show(ctx, cx);
    super::action_history::show(ctx, cx);
    super::multiple_copy::show(ctx, cx);
    super::drawing_groups::show(ctx, cx);
    super::arch_block::show_all(ctx, cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{EditorContext, ObjectRef};
    use crate::plan_defaults;
    use plan_core::{Point, WallKind};

    fn cx_with_wall() -> (EditorContext, plan_core::Id) {
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
        (cx, w)
    }

    #[test]
    fn the_fields_become_params_and_apply_is_one_undo_step() {
        let (mut cx, _) = cx_with_wall();
        let mut d = TransformDialog {
            make_copies: true,
            copies: 2,
            move_x: "2'".into(),
            move_y: "0".into(),
            ..TransformDialog::default()
        };
        let p = d.params(Point::ZERO);
        assert_eq!(p.copies, 2);
        assert!((p.move_x - 24.0).abs() < 1e-9 && p.move_y == 0.0);
        assert!(d.apply(&mut cx));
        assert_eq!(cx.floor().walls.len(), 3);
        assert_eq!(cx.undo_label(), Some("Transform/Replicate"));
        assert!(d.message.contains("2 copies"), "{}", d.message);
        cx.undo();
        assert_eq!(cx.floor().walls.len(), 1);
    }

    #[test]
    fn distance_and_angle_move_and_the_mirror_line_default_to_the_center() {
        let mut d = TransformDialog {
            move_polar: true,
            move_dist: "10'".into(),
            move_angle: 90.0,
            ..TransformDialog::default()
        };
        let p = d.params(Point::ZERO);
        assert!(p.move_x.abs() < 1e-9 && (p.move_y - 120.0).abs() < 1e-9);
        d.reflect = true;
        let p = d.params(Point::new(40.0, 25.0));
        assert_eq!(p.reflect, Some(ReflectAxis::Vertical(40.0)));
        d.reflect_vertical = false;
        d.reflect_center = false;
        d.reflect_at = "5'".into();
        assert_eq!(
            d.params(Point::ZERO).reflect,
            Some(ReflectAxis::Horizontal(60.0))
        );
        d.about_point = true;
        d.about_x = "1'".into();
        d.about_y = "2'".into();
        assert_eq!(
            d.params(Point::ZERO).rotate_about,
            Some(Point::new(12.0, 24.0))
        );
    }

    #[test]
    fn nothing_selected_means_no_dialog_and_an_empty_transform_is_refused() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        open(&mut cx);
        assert!(!is_open());
        assert_eq!(cx.status, "Select objects to transform");
        let (mut cx, _) = cx_with_wall();
        let mut d = TransformDialog::default();
        assert!(!d.apply(&mut cx), "all values leave the objects alone");
        assert!(!cx.can_undo());
    }

    #[test]
    fn the_align_dialog_aligns_and_distributes_with_a_gap() {
        use plan_core::cad::CadItem;
        use plan_core::transform::{AlignMode, Axis};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let ids: Vec<_> = [(0.0, 10.0), (30.0, 50.0), (90.0, 100.0)]
            .iter()
            .map(|(x0, x1)| {
                cx.project.add_cad(
                    0,
                    "CAD, Default",
                    CadItem::Line {
                        a: Point::new(*x0, 0.0),
                        b: Point::new(*x1, 10.0),
                    },
                )
            })
            .collect();
        cx.selection.items = ids.iter().map(|i| ObjectRef::Cad(*i)).collect();
        let mut d = AlignDialog::default();
        d.align(&mut cx, AlignMode::Left);
        assert!(d.message.starts_with("Aligned 2"), "{}", d.message);
        cx.undo();
        d.use_gap = true;
        d.gap = "5".into();
        d.distribute(&mut cx, Axis::Horizontal);
        assert!(d.message.starts_with("Moved"), "{}", d.message);
        let xs: Vec<f64> = cx
            .floor()
            .cad
            .iter()
            .map(|c| match c.item {
                CadItem::Line { a, .. } => a.x,
                _ => unreachable!(),
            })
            .collect();
        // First box stays at 0..10; the next starts 5" after it (15..35), and
        // the last 5" after that (40..50).
        assert_eq!(xs, vec![0.0, 15.0, 40.0]);
        // One object: a message and no step.
        cx.selection.items.truncate(1);
        open_align(&mut cx);
        assert!(cx.status.contains("two or more"));
    }

    #[test]
    fn the_edit_windows_draw_without_panicking() {
        let (mut cx, _) = cx_with_wall();
        open(&mut cx);
        super::super::delete_objects::open();
        super::super::send_to_layer::open(&mut cx);
        super::super::action_history::open();
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                show_edit_windows(ctx, &mut cx)
            });
        }
        assert!(is_open());
        DIALOG.with(|d| *d.borrow_mut() = None);
    }
}
