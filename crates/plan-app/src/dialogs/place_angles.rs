//! Place at Allowed Angles (S-179, manual p. 300). After an Edit Area move or
//! turn, when more than 1 percent of the straight walls inside it sit off the
//! allowed angles, this window offers three things: turn the largest group of
//! walls that share one angle onto the nearest allowed angle, add that angle
//! to the allowed list (Snap Settings, Additional Angles), or do nothing.
//! `build_tools::show_all` draws it every frame; `tools::select::area` opens
//! it through [`offer`].

use crate::editor::selection::ObjectRef;
use crate::editor::{snap, transform, EditorContext};
use eframe::egui;
use plan_core::transform::Xform;
use plan_core::wall_repair::{nearest_allowed, wall_angle_deg};
use plan_core::{Id, Point};
use std::cell::RefCell;

/// A wall this many degrees from the nearest allowed angle (or more) is off.
pub const OFF_TOLERANCE_DEG: f64 = 0.05;
/// The share of straight walls that must be off before the window appears.
pub const OFF_SHARE: f64 = 0.01;
/// Walls shorter than this (inches) are not straight enough to judge.
const MIN_LEN: f64 = 1.0;

/// Walls that share one angle and sit off the allowed ones.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// The walls' angle, degrees, 0 up to 180 (a wall and its opposite agree).
    pub angle: f64,
    /// The allowed angle they would turn to, degrees, 0 up to 180.
    pub target: f64,
    pub walls: Vec<Id>,
    /// Their total length, inches.
    pub length: f64,
}

/// What a set of walls looks like against the allowed angles.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Straight walls looked at.
    pub straight: usize,
    /// Of those, how many are off.
    pub off: usize,
    /// The off walls, grouped by shared angle, largest group first.
    pub groups: Vec<Group>,
}

impl Report {
    /// More than 1 percent of the straight walls are off an allowed angle.
    pub fn needs_window(&self) -> bool {
        self.off > 0 && self.off as f64 > OFF_SHARE * self.straight as f64
    }
}

fn allowed(cx: &EditorContext) -> Vec<f64> {
    let e = &cx.defaults.editing;
    snap::allowed_angle_set(e, e.angle_snap_deg, false)
}

/// Sorts `walls` (on the active floor) into straight and off-angle ones.
pub fn analyze(cx: &EditorContext, walls: &[Id]) -> Report {
    let set = allowed(cx);
    let inc = cx.defaults.editing.angle_snap_deg;
    let mut rep = Report::default();
    let mut groups: Vec<Group> = Vec::new();
    for id in walls {
        let Some(w) = cx.floor().wall(*id) else {
            continue;
        };
        if w.is_curved() || w.length() < MIN_LEN {
            continue;
        }
        rep.straight += 1;
        let a = wall_angle_deg(w);
        let t = nearest_allowed(a, &set, inc);
        let d = ((a - t + 180.0).rem_euclid(360.0) - 180.0).abs();
        if d < OFF_TOLERANCE_DEG {
            continue;
        }
        rep.off += 1;
        let a180 = a.rem_euclid(180.0);
        match groups.iter_mut().find(|g| (g.angle - a180).abs() < 0.1) {
            Some(g) => {
                g.walls.push(*id);
                g.length += w.length();
            }
            None => groups.push(Group {
                angle: a180,
                target: t.rem_euclid(180.0),
                walls: vec![*id],
                length: w.length(),
            }),
        }
    }
    groups.sort_by(|a, b| {
        b.walls
            .len()
            .cmp(&a.walls.len())
            .then(b.length.total_cmp(&a.length))
    });
    rep.groups = groups;
    rep
}

/// Turns the walls of `group` onto its target angle about their middle (one
/// undo step). Returns how many changed.
pub fn rotate_group(cx: &mut EditorContext, group: &Group) -> usize {
    let mut pts: Vec<Point> = Vec::new();
    for id in &group.walls {
        if let Some(w) = cx.floor().wall(*id) {
            pts.extend([w.start, w.end]);
        }
    }
    if pts.is_empty() {
        return 0;
    }
    let n = pts.len() as f64;
    let center = Point::new(
        pts.iter().map(|p| p.x).sum::<f64>() / n,
        pts.iter().map(|p| p.y).sum::<f64>() / n,
    );
    // The shortest turn: at most a quarter turn either way.
    let delta = (group.target - group.angle + 90.0).rem_euclid(180.0) - 90.0;
    let items: Vec<ObjectRef> = group.walls.iter().map(|i| ObjectRef::Wall(*i)).collect();
    cx.begin_change("Place at Allowed Angles");
    let rep = transform::apply_xform(cx, &items, &Xform::rotate(center, delta.to_radians()));
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!(
        "{} walls placed at {:.2} degrees",
        group.walls.len(),
        group.target
    );
    rep.changed
}

/// Adds the group's angle to the allowed list (Additional Angles). False when
/// it is there already.
pub fn allow_angle(cx: &mut EditorContext, group: &Group) -> bool {
    let a = (group.angle * 100.0).round() / 100.0;
    let have = allowed(cx).iter().any(|x| (x - a).abs() < 0.005);
    if have {
        return false;
    }
    cx.defaults.editing.additional_angles.push(a);
    cx.status = format!("{a:.2} degrees added to the allowed angles");
    true
}

/// What the person chose.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    /// Turn the largest group onto an allowed angle.
    Rotate,
    /// Add the group's angle to the allowed angles.
    Allow,
    /// Leave everything as it is.
    Nothing,
}

/// The window's draft.
pub struct PlaceAnglesDialog {
    pub report: Report,
    pub choice: Choice,
}

impl PlaceAnglesDialog {
    pub fn new(report: Report) -> Self {
        Self {
            report,
            choice: Choice::Rotate,
        }
    }

    /// OK: carries out the choice. Returns whether the plan changed.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        let Some(g) = self.report.groups.first() else {
            return false;
        };
        match self.choice {
            Choice::Rotate => rotate_group(cx, g) > 0,
            Choice::Allow => allow_angle(cx, g),
            Choice::Nothing => false,
        }
    }

    /// Draws the window; false once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let (mut ok, mut cancel) = (false, false);
        let g = self.report.groups.first().cloned();
        egui::Window::new("Place at Allowed Angles")
            .id(egui::Id::new("place_at_allowed_angles"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "{} of {} straight walls are not on an allowed angle.",
                    self.report.off, self.report.straight
                ));
                if let Some(g) = &g {
                    ui.label(format!(
                        "The largest group ({} walls) is at {:.2}\u{b0}.",
                        g.walls.len(),
                        g.angle
                    ));
                    ui.radio_value(
                        &mut self.choice,
                        Choice::Rotate,
                        format!("Rotate the group to {:.2}\u{b0}", g.target),
                    );
                    ui.radio_value(
                        &mut self.choice,
                        Choice::Allow,
                        format!("Add {:.2}\u{b0} to the allowed angles", g.angle),
                    );
                    ui.radio_value(&mut self.choice, Choice::Nothing, "Do nothing");
                }
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
    static WINDOW: RefCell<Option<PlaceAnglesDialog>> = const { RefCell::new(None) };
}

/// Looks at `walls` and opens the window when more than 1 percent of the
/// straight ones are off an allowed angle. True when it opened.
pub fn offer(cx: &EditorContext, walls: &[Id]) -> bool {
    let report = analyze(cx, walls);
    if !report.needs_window() {
        return false;
    }
    WINDOW.with(|w| *w.borrow_mut() = Some(PlaceAnglesDialog::new(report)));
    true
}

/// Is the window open?
pub fn is_open() -> bool {
    WINDOW.with(|w| w.borrow().is_some())
}

/// Closes the window without doing anything.
pub fn close() {
    WINDOW.with(|w| *w.borrow_mut() = None);
}

/// Draws the open window, if any (called every frame).
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
    use crate::plan_defaults;
    use plan_core::WallKind;

    fn wall(cx: &mut EditorContext, a: (f64, f64), b: (f64, f64)) -> Id {
        cx.project.add_wall(
            0,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            4.5,
            96.0,
            WallKind::Exterior,
        )
    }

    #[test]
    fn walls_on_allowed_angles_need_no_window() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let ids = [
            wall(&mut cx, (0.0, 0.0), (240.0, 0.0)),
            wall(&mut cx, (240.0, 0.0), (240.0, 120.0)),
        ];
        let rep = analyze(&cx, &ids);
        assert_eq!((rep.straight, rep.off), (2, 0));
        assert!(!rep.needs_window());
    }

    #[test]
    fn a_turned_group_is_found_and_rotated_to_the_nearest_allowed_angle() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        // Two walls at 20 degrees (parallel) and one square wall.
        let (c, s) = (20f64.to_radians().cos(), 20f64.to_radians().sin());
        let a = wall(&mut cx, (0.0, 0.0), (240.0 * c, 240.0 * s));
        let b = wall(&mut cx, (0.0, 100.0), (240.0 * c, 100.0 + 240.0 * s));
        let sq = wall(&mut cx, (500.0, 0.0), (740.0, 0.0));
        let rep = analyze(&cx, &[a, b, sq]);
        assert_eq!((rep.straight, rep.off), (3, 2));
        assert!(rep.needs_window());
        assert_eq!(rep.groups.len(), 1);
        let g = rep.groups[0].clone();
        assert!((g.angle - 20.0).abs() < 1e-6 && (g.target - 15.0).abs() < 1e-6);
        let undo_before = cx.action_history().0.len();
        assert_eq!(rotate_group(&mut cx, &g), 2);
        assert_eq!(cx.action_history().0.len(), undo_before + 1);
        let after = analyze(&cx, &[a, b, sq]);
        assert_eq!(after.off, 0);
    }

    #[test]
    fn adding_the_angle_makes_the_group_allowed() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let (c, s) = (20f64.to_radians().cos(), 20f64.to_radians().sin());
        let a = wall(&mut cx, (0.0, 0.0), (240.0 * c, 240.0 * s));
        let g = analyze(&cx, &[a]).groups[0].clone();
        assert!(allow_angle(&mut cx, &g));
        assert_eq!(analyze(&cx, &[a]).off, 0);
        assert!(!allow_angle(&mut cx, &g), "already allowed");
    }

    #[test]
    fn offer_opens_the_window_only_past_one_percent() {
        close();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let on = wall(&mut cx, (0.0, 0.0), (240.0, 0.0));
        assert!(!offer(&cx, &[on]) && !is_open());
        let off = wall(&mut cx, (0.0, 50.0), (240.0, 80.0));
        assert!(offer(&cx, &[on, off]) && is_open());
        close();
    }
}
