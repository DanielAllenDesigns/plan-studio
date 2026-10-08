//! Edit Area, Edit Area Visible and Stretch CAD (S-90, S-91).
//!
//! Edit > Edit Area asks for a rubber band. The rectangle it leaves is the
//! edit area: dragging inside it moves everything wholly inside (Ctrl or Cmd
//! held when the drag starts copies it instead), dragging its Rotate handle
//! turns it about its center, Delete removes the contents, Esc or a click
//! outside ends the mode. A wall that crosses the rectangle's edge does not
//! come along whole: the end inside moves (or turns) and the end outside
//! stays, so the wall stretches, with its doors and windows keeping their
//! place. "Edit Area Visible" takes only objects on displayed layers; "Edit
//! Area" also takes those on hidden (never locked) layers.
//!
//! Stretch CAD takes the same rubber band, then one drag: every CAD vertex
//! inside the band moves with the pointer, the vertices outside stay.
//!
//! Each drag is rebuilt from the plan as it was when the drag began, and is
//! one undo step. Typed digits give the distance (and angle with Tab) of a
//! move, or the degrees of a turn.

use super::*;
use crate::editor::typed_input::TypedKey;
use plan_core::transform::Xform;
use std::cell::RefCell;

/// Which of the two commands is under way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AreaKind {
    Edit { visible_only: bool },
    StretchCad,
}

enum Stage {
    /// Waiting for the rubber band (`start` set once the button is down).
    Band {
        start: Option<Point>,
        current: Point,
    },
    /// The region is placed.
    Placed { lo: Point, hi: Point },
    /// Dragging the contents.
    Moving {
        lo: Point,
        hi: Point,
        start: Point,
        original: Box<Project>,
        copy: bool,
        shift: Point,
    },
    /// Dragging the Rotate handle.
    Turning {
        lo: Point,
        hi: Point,
        start: Point,
        original: Box<Project>,
        angle: f64,
    },
}

struct State {
    kind: AreaKind,
    stage: Stage,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn take() -> Option<State> {
    STATE.with(|s| s.borrow_mut().take())
}

fn put(st: State) {
    STATE.with(|s| *s.borrow_mut() = Some(st));
}

/// Is Edit Area or Stretch CAD waiting for or holding a region?
pub fn active() -> bool {
    STATE.with(|s| s.borrow().is_some())
}

/// The placed region (corners `lo`, `hi`), once the rubber band is done.
pub fn region() -> Option<(Point, Point)> {
    STATE.with(|s| match s.borrow().as_ref().map(|s| &s.stage) {
        Some(Stage::Placed { lo, hi })
        | Some(Stage::Moving { lo, hi, .. })
        | Some(Stage::Turning { lo, hi, .. }) => Some((*lo, *hi)),
        _ => None,
    })
}

/// Starts the command: the next press-drag draws the rubber band.
pub fn begin(cx: &mut EditorContext, kind: AreaKind) {
    transform::cancel_mode(cx);
    cancel(cx);
    put(State {
        kind,
        stage: Stage::Band {
            start: None,
            current: Point::ZERO,
        },
    });
    cx.status = match kind {
        AreaKind::Edit { visible_only: true } => {
            "Edit Area Visible: drag a rectangle around what to edit".into()
        }
        AreaKind::Edit { .. } => "Edit Area: drag a rectangle around what to edit".into(),
        AreaKind::StretchCad => "Stretch CAD: drag a window around the vertices to stretch".into(),
    };
}

/// Ends the mode; a drag in progress is undone.
pub fn cancel(cx: &mut EditorContext) {
    if let Some(st) = take() {
        match st.stage {
            Stage::Moving { original, .. } | Stage::Turning { original, .. } => {
                cx.project = *original;
                cx.cancel_change();
                cx.typed_input.disarm();
                cx.readout = None;
                cx.mark_dirty();
            }
            _ => {}
        }
    }
}

const ROTATE_HANDLE_PX: f64 = 26.0;

/// The Rotate handle above the middle of the region's top edge.
pub fn rotate_handle(lo: Point, hi: Point, px_per_in: f64) -> Point {
    Point::new(
        (lo.x + hi.x) * 0.5,
        hi.y + ROTATE_HANDLE_PX / px_per_in.max(1e-6),
    )
}

fn center_of(lo: Point, hi: Point) -> Point {
    Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5)
}

// ----- what the region holds -----

/// The objects wholly inside the region, apart from walls and openings (those
/// are handled end by end).
fn region_items(cx: &EditorContext, lo: Point, hi: Point, visible_only: bool) -> Vec<ObjectRef> {
    objects_in_box(cx, lo, hi, false, !visible_only)
        .into_iter()
        .filter(|o| !matches!(o, ObjectRef::Wall(_) | ObjectRef::Opening(_)))
        .collect()
}

/// The walls whose whole outline lies inside the region.
fn walls_inside(cx: &EditorContext, lo: Point, hi: Point, visible_only: bool) -> Vec<Id> {
    objects_in_box(cx, lo, hi, false, !visible_only)
        .into_iter()
        .filter_map(|o| match o {
            ObjectRef::Wall(id) => Some(id),
            _ => None,
        })
        .collect()
}

/// Moves (or turns) the ends of walls that lie inside the region; a wall with
/// one end in and one out stretches, its openings staying where they were
/// when the start end is the one that moved. Returns how many walls changed.
fn move_wall_ends(project: &mut Project, fl: usize, lo: Point, hi: Point, xf: &Xform) -> usize {
    let ids: Vec<Id> = project.floors[fl].walls.iter().map(|w| w.id).collect();
    let mut n = 0;
    for id in ids {
        let Some(w) = project.floors[fl].wall(id).cloned() else {
            continue;
        };
        let (s_in, e_in) = (in_rect(w.start, lo, hi), in_rect(w.end, lo, hi));
        if !s_in && !e_in {
            continue;
        }
        let ns = if s_in { xf.apply(w.start) } else { w.start };
        let ne = if e_in { xf.apply(w.end) } else { w.end };
        if ns.dist(ne) < 1.0 {
            continue;
        }
        // Openings stay where they are in the plan when only the start moves.
        let mut offsets: Vec<(Id, f64)> = Vec::new();
        if s_in && !e_in && !w.is_curved() {
            let dir_old = (w.end - w.start).normalized();
            let dir_new = (ne - ns).normalized();
            for o in project.floors[fl].openings_on(id) {
                let at = w.start + dir_old * o.center_offset;
                offsets.push((o.id, (at - ns).dot(dir_new)));
            }
        }
        let len = ns.dist(ne);
        if let Some(wm) = project.floors[fl].wall_mut(id) {
            wm.start = ns;
            wm.end = ne;
        }
        for (oid, off) in offsets {
            if let Some(o) = project.floors[fl].openings.iter_mut().find(|o| o.id == oid) {
                let half = o.width * 0.5;
                o.center_offset = off.clamp(half, (len - half).max(half));
            }
        }
        n += 1;
    }
    n
}

/// Stretch CAD: every CAD vertex inside the window moves by `d`. Circles,
/// arcs and text move whole when their center or anchor is inside. Returns the
/// number of objects that changed.
fn stretch_cad(cx: &mut EditorContext, lo: Point, hi: Point, d: Point) -> usize {
    let fl = cx.floor;
    let allowed: Vec<Id> = cx
        .floor()
        .cad
        .iter()
        .filter(|c| cx.layers().is_visible(&c.layer) && !cx.layers().is_locked(&c.layer))
        .map(|c| c.id)
        .collect();
    let mut n = 0;
    for c in cx.project.floors[fl].cad.iter_mut() {
        if !allowed.contains(&c.id) {
            continue;
        }
        let mut hit = false;
        let mut shift = |p: &mut Point| {
            if in_rect(*p, lo, hi) {
                *p = *p + d;
                hit = true;
            }
        };
        match &mut c.item {
            CadItem::Line { a, b } => {
                shift(a);
                shift(b);
            }
            CadItem::Polyline { points, .. } => points.iter_mut().for_each(shift),
            CadItem::Circle { center, .. } | CadItem::Arc { center, .. } => shift(center),
            CadItem::Text { pos, .. } => shift(pos),
        }
        if hit {
            n += 1;
        }
    }
    n
}

/// Rebuilds the plan from `original` with the region's contents moved by `d`
/// (a copy of them placed `d` away with `copy`) or, for `turn`, turned.
/// Returns what changed, for the status line.
fn apply(
    cx: &mut EditorContext,
    original: &Project,
    kind: AreaKind,
    (lo, hi): (Point, Point),
    d: Point,
    turn: Option<f64>,
    copy: bool,
) -> usize {
    cx.project = original.clone();
    let fl = cx.floor;
    let walls_before = cx.floor().walls.clone();
    let n = match kind {
        AreaKind::StretchCad => stretch_cad(cx, lo, hi, d),
        AreaKind::Edit { visible_only } => {
            let items = region_items(cx, lo, hi, visible_only);
            if copy {
                // Copies of everything inside, placed `d` away.
                let mut all = items;
                all.extend(
                    walls_inside(cx, lo, hi, visible_only)
                        .into_iter()
                        .map(ObjectRef::Wall),
                );
                let saved = std::mem::replace(&mut cx.selection.items, all);
                let clip = Clipboard::capture(cx);
                cx.selection.items = saved;
                clip.paste(cx, d, false).len()
            } else {
                let xf = match turn {
                    Some(a) => Xform::rotate(center_of(lo, hi), a),
                    None => Xform::translate(d),
                };
                let walls = move_wall_ends(&mut cx.project, fl, lo, hi, &xf);
                walls + transform::apply_xform(cx, &items, &xf).changed
            }
        }
    };
    if !copy {
        details_view::follow_walls(&mut cx.project, fl, &walls_before);
    }
    crate::editor::placed::sync_distributions(cx);
    cx.mark_dirty();
    n
}

fn label(kind: AreaKind, turn: bool, copy: bool) -> &'static str {
    match (kind, turn, copy) {
        (AreaKind::StretchCad, _, _) => "Stretch CAD",
        (_, true, _) => "Rotate Edit Area",
        (_, _, true) => "Copy Edit Area",
        _ => "Move Edit Area",
    }
}

// ----- events -----

/// The press: starts the rubber band, a move or a turn, or ends the mode.
/// `None` while no area command is active.
pub fn pointer_down(cx: &mut EditorContext, p: &PointerEvent) -> Option<ToolResult> {
    let mut st = take()?;
    let kind = st.kind;
    let mut keep = true;
    match &mut st.stage {
        Stage::Band { start, current } => {
            *start = Some(p.world);
            *current = p.world;
        }
        Stage::Placed { lo, hi } => {
            let (lo, hi) = (*lo, *hi);
            let handle = rotate_handle(lo, hi, cx.px_per_in);
            let turn = matches!(kind, AreaKind::Edit { .. })
                && p.world.dist(handle) <= cx.pick_tol() * 1.5;
            if turn {
                cx.begin_change(label(kind, true, false));
                cx.typed_input.arm_angle();
                st.stage = Stage::Turning {
                    lo,
                    hi,
                    start: p.world,
                    original: Box::new(cx.project.clone()),
                    angle: 0.0,
                };
            } else if in_rect(p.world, lo, hi) {
                let copy = matches!(kind, AreaKind::Edit { .. })
                    && (p.modifiers.ctrl || p.modifiers.command);
                cx.begin_change(label(kind, false, copy));
                cx.typed_input.arm();
                st.stage = Stage::Moving {
                    lo,
                    hi,
                    start: p.world,
                    original: Box::new(cx.project.clone()),
                    copy,
                    shift: Point::ZERO,
                };
            } else {
                keep = false;
                cx.status = "Edit area ended".into();
            }
        }
        // A press while a drag is under way (the button came up unseen).
        _ => {}
    }
    if keep {
        put(st);
    }
    Some(ToolResult::consumed())
}

/// The distance a move drag has gone, snapped to the grid unless a number is
/// typed or Alt is held.
fn move_delta(cx: &mut EditorContext, start: Point, p: &PointerEvent) -> Point {
    let raw = p.world;
    let world = typed_move_target(cx, start, raw);
    let mut d = world.unwrap_or(raw) - start;
    if world.is_none() && !p.modifiers.alt {
        let u = cx.snap_unit();
        d = Point::new(snap_unit_round(d.x, u), snap_unit_round(d.y, u));
    }
    let ti = &cx.typed_input;
    cx.readout = Some(format!(
        "Distance: {}   Angle: {}",
        if ti.has_text() && ti.field() == crate::editor::typed_input::TypedField::Length {
            format!("{}|", ti.length_text())
        } else {
            cx.fmt_dim(d.length())
        },
        if ti.has_text() && ti.field() == crate::editor::typed_input::TypedField::Angle {
            format!("{}|", ti.angle_text())
        } else {
            format!(
                "{:.1}\u{b0}",
                crate::editor::typed_input::angle_deg(Point::ZERO, d)
            )
        },
    ));
    d
}

/// The angle a turn drag has gone, in whole increments unless a number is
/// typed or Alt is held.
fn turn_angle(cx: &mut EditorContext, center: Point, start: Point, p: &PointerEvent) -> f64 {
    let typed = typed_rotate_target(cx, center, start);
    let to = typed.unwrap_or(p.world);
    let mut angle = (to - center).angle() - (start - center).angle();
    if typed.is_none() && !p.modifiers.alt {
        let inc = cx.defaults.grid.angle_snap_deg.max(1.0).to_radians();
        angle = (angle / inc).round() * inc;
    }
    let ti = &cx.typed_input;
    cx.readout = Some(format!(
        "Rotate: {}",
        if ti.has_text() {
            format!("{}|", ti.angle_text())
        } else {
            format!("{:.1}\u{b0}", angle.to_degrees())
        }
    ));
    angle
}

/// Re-applies the drag in progress for the pointer `p`.
fn drag_to(cx: &mut EditorContext, st: &mut State, p: &PointerEvent) {
    let kind = st.kind;
    match &mut st.stage {
        Stage::Moving {
            lo,
            hi,
            start,
            original,
            copy,
            shift,
        } => {
            let d = move_delta(cx, *start, p);
            *shift = d;
            apply(cx, original, kind, (*lo, *hi), d, None, *copy);
        }
        Stage::Turning {
            lo,
            hi,
            start,
            original,
            angle,
        } => {
            let center = center_of(*lo, *hi);
            let a = turn_angle(cx, center, *start, p);
            *angle = a;
            apply(cx, original, kind, (*lo, *hi), Point::ZERO, Some(a), false);
        }
        _ => {}
    }
}

/// Moves the pointer: stretches the rubber band or carries the drag. True
/// when an area command took the event.
pub fn pointer_move(cx: &mut EditorContext, p: &PointerEvent) -> bool {
    let Some(mut st) = take() else {
        return false;
    };
    match &mut st.stage {
        Stage::Band {
            start: Some(_),
            current,
        } if p.down => *current = p.world,
        Stage::Moving { .. } | Stage::Turning { .. } if p.down => drag_to(cx, &mut st, p),
        _ => {}
    }
    put(st);
    true
}

/// The release: places the region, or ends the drag as one undo step.
pub fn pointer_up(cx: &mut EditorContext, p: &PointerEvent) -> Option<ToolResult> {
    let mut st = take()?;
    let kind = st.kind;
    let mut result = ToolResult::consumed();
    let mut keep = true;
    match std::mem::replace(
        &mut st.stage,
        Stage::Band {
            start: None,
            current: Point::ZERO,
        },
    ) {
        Stage::Band { start: Some(a), .. } => {
            let size = (p.world - a).length() * cx.px_per_in;
            if size >= 6.0 {
                let lo = Point::new(a.x.min(p.world.x), a.y.min(p.world.y));
                let hi = Point::new(a.x.max(p.world.x), a.y.max(p.world.y));
                st.stage = Stage::Placed { lo, hi };
                cx.status = match kind {
                    AreaKind::StretchCad => "Stretch CAD: drag inside the window".into(),
                    _ => "Edit area: drag to move, the handle to rotate, Delete removes, Esc ends"
                        .into(),
                };
            }
        }
        Stage::Moving {
            lo,
            hi,
            start,
            original,
            copy,
            shift,
        } => {
            // The last position of the pointer counts.
            let mut tmp = State {
                kind,
                stage: Stage::Moving {
                    lo,
                    hi,
                    start,
                    original: original.clone(),
                    copy,
                    shift,
                },
            };
            drag_to(cx, &mut tmp, p);
            let shift = match &tmp.stage {
                Stage::Moving { shift, .. } => *shift,
                _ => shift,
            };
            cx.typed_input.disarm();
            cx.readout = None;
            if original.to_json().ok() == cx.project.to_json().ok() {
                cx.cancel_change();
                st.stage = Stage::Placed { lo, hi };
            } else {
                result = ToolResult::committed(label(kind, false, copy));
                if copy || kind == AreaKind::StretchCad {
                    keep = false;
                } else {
                    st.stage = Stage::Placed {
                        lo: lo + shift,
                        hi: hi + shift,
                    };
                }
            }
        }
        Stage::Turning {
            lo,
            hi,
            start,
            original,
            angle,
        } => {
            let mut tmp = State {
                kind,
                stage: Stage::Turning {
                    lo,
                    hi,
                    start,
                    original: original.clone(),
                    angle,
                },
            };
            drag_to(cx, &mut tmp, p);
            cx.typed_input.disarm();
            cx.readout = None;
            if original.to_json().ok() == cx.project.to_json().ok() {
                cx.cancel_change();
                st.stage = Stage::Placed { lo, hi };
            } else {
                // The turned region is no longer a rectangle: the mode ends.
                result = ToolResult::committed(label(kind, true, false));
                keep = false;
            }
        }
        other => st.stage = other,
    }
    if keep {
        put(st);
    } else {
        cx.status.clear();
    }
    Some(result)
}

/// Keys: Esc, Delete and the typed number of a drag in progress.
pub fn key(cx: &mut EditorContext, k: &KeyEvent) -> Option<ToolResult> {
    let mut st = take()?;
    let kind = st.kind;
    let dragging = matches!(st.stage, Stage::Moving { .. } | Stage::Turning { .. });
    if dragging {
        let res = cx.typed_input.handle(k.key, k.text.as_deref());
        if res != TypedKey::Ignored {
            let at = cx.cursor_world.unwrap_or(Point::ZERO);
            let p = PointerEvent::at(cx, at).with_down(true);
            drag_to(cx, &mut st, &p);
            if res == TypedKey::Commit {
                put(st);
                let up = PointerEvent::at(cx, at);
                return pointer_up(cx, &up);
            }
            put(st);
            return Some(ToolResult::consumed());
        }
    }
    if k.is(Key::Escape) {
        put(st);
        cancel(cx);
        cx.status.clear();
        return Some(ToolResult::consumed());
    }
    if (k.is(Key::Delete) || k.is(Key::Backspace)) && !dragging {
        if let (Stage::Placed { lo, hi }, AreaKind::Edit { visible_only }) = (&st.stage, kind) {
            let (lo, hi) = (*lo, *hi);
            let mut all = region_items(cx, lo, hi, visible_only);
            all.extend(
                walls_inside(cx, lo, hi, visible_only)
                    .into_iter()
                    .map(ObjectRef::Wall),
            );
            if all.is_empty() {
                cx.status = "Nothing inside the edit area".into();
                put(st);
                return Some(ToolResult::consumed());
            }
            cx.selection.items = all;
            cx.delete_selection();
            put(st);
            return Some(ToolResult::committed("Delete Edit Area"));
        }
    }
    put(st);
    // Other keys are swallowed while a rubber band is wanted; once the region
    // is placed they behave as usual.
    None
}

/// The rubber band, the region with its Rotate handle and the live outline of
/// a move or turn.
pub fn draw_overlay(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let (kind, band, placed, shift, angle) = STATE.with(|s| {
        let b = s.borrow();
        let Some(st) = b.as_ref() else {
            return (None, None, None, Point::ZERO, 0.0);
        };
        let mut band = None;
        let mut placed = None;
        let mut shift = Point::ZERO;
        let mut angle = 0.0;
        match &st.stage {
            Stage::Band {
                start: Some(a),
                current,
            } => band = Some((*a, *current)),
            Stage::Placed { lo, hi } => placed = Some((*lo, *hi)),
            Stage::Moving {
                lo, hi, shift: d, ..
            } => {
                placed = Some((*lo, *hi));
                shift = *d;
            }
            Stage::Turning {
                lo, hi, angle: a, ..
            } => {
                placed = Some((*lo, *hi));
                angle = *a;
            }
            _ => {}
        }
        (Some(st.kind), band, placed, shift, angle)
    });
    let Some(kind) = kind else { return };
    let col = cx.palette.selection;
    if let Some((a, b)) = band {
        let r = Rect::from_two_pos(cam.world_to_screen(a), cam.world_to_screen(b));
        painter.add(Shape::rect_filled(r, 0.0, col.gamma_multiply(0.10)));
        painter.rect_stroke(r, 0.0, Stroke::new(1.5_f32, col), egui::StrokeKind::Inside);
    }
    if let Some((lo, hi)) = placed {
        let center = center_of(lo, hi);
        let xf = Xform::rotate(center, angle);
        let pts: Vec<Pos2> = rect_corners(lo, hi)
            .iter()
            .map(|c| cam.world_to_screen(xf.apply(*c) + shift))
            .collect();
        painter.add(Shape::convex_polygon(
            pts.clone(),
            col.gamma_multiply(0.07),
            Stroke::new(1.5_f32, col),
        ));
        if matches!(kind, AreaKind::Edit { .. }) {
            let h = xf.apply(rotate_handle(lo, hi, cx.px_per_in)) + shift;
            let top = xf.apply(Point::new(center.x, hi.y)) + shift;
            painter.line_segment(
                [cam.world_to_screen(top), cam.world_to_screen(h)],
                Stroke::new(1.0_f32, col),
            );
            painter.circle_filled(cam.world_to_screen(h), 5.0, col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use crate::tools::select::SelectTool;
    use plan_core::WallKind;

    fn cx() -> EditorContext {
        cancel_state();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.px_per_in = 2.0;
        cx
    }

    fn cancel_state() {
        take();
    }

    fn ev(cx: &EditorContext, x: f64, y: f64) -> PointerEvent {
        PointerEvent::at(cx, Point::new(x, y))
    }

    fn down(t: &mut SelectTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let e = ev(cx, x, y).with_down(true);
        t.pointer_down(cx, e)
    }

    fn mv(t: &mut SelectTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let e = ev(cx, x, y).with_down(true);
        t.pointer_move(cx, e)
    }

    fn up(t: &mut SelectTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let e = ev(cx, x, y);
        t.pointer_up(cx, e)
    }

    fn drag(t: &mut SelectTool, cx: &mut EditorContext, a: (f64, f64), b: (f64, f64)) {
        down(t, cx, a.0, a.1);
        mv(t, cx, b.0, b.1);
        up(t, cx, b.0, b.1);
    }

    /// A 20' x 10' box of walls and a free CAD line inside it.
    fn house(cx: &mut EditorContext) -> [Id; 4] {
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 120.0),
            Point::new(0.0, 120.0),
        ];
        let mut ids = [0; 4];
        for i in 0..4 {
            ids[i] = cx
                .project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
        ids
    }

    #[test]
    fn rubber_band_then_move_stretches_walls_that_cross_the_edge() {
        let mut cx = cx();
        let ids = house(&mut cx);
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        assert!(active());
        // The band covers the right half of the house plus a margin: x 100..300.
        drag(&mut t, &mut cx, (100.0, -40.0), (300.0, 160.0));
        let (lo, hi) = region().expect("the region is placed");
        assert_eq!((lo.x, lo.y, hi.x, hi.y), (100.0, -40.0, 300.0, 160.0));
        // Move the region 48" to the right.
        drag(&mut t, &mut cx, (200.0, 60.0), (248.0, 60.0));
        let f = cx.floor();
        let bottom = f.wall(ids[0]).unwrap();
        assert_eq!(bottom.start, Point::new(0.0, 0.0), "the end outside stays");
        assert_eq!(bottom.end, Point::new(288.0, 0.0), "the end inside moves");
        let right = f.wall(ids[1]).unwrap();
        assert_eq!(right.start, Point::new(288.0, 0.0));
        assert_eq!(right.end, Point::new(288.0, 120.0));
        assert_eq!(f.wall(ids[3]).unwrap().start, Point::new(0.0, 120.0));
        // One undo step undoes the whole move.
        cx.undo();
        assert_eq!(cx.floor().wall(ids[0]).unwrap().end, Point::new(240.0, 0.0));
        // The region moved with the contents.
        assert!(active());
    }

    #[test]
    fn a_wall_inside_the_edit_area_turns_with_it_and_openings_keep_their_place() {
        let mut cx = cx();
        let ids = house(&mut cx);
        let door = cx
            .project
            .add_opening(0, ids[0], 60.0, plan_core::OpeningKind::Door)
            .unwrap();
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        // Only the left end of the bottom wall and the left wall are inside.
        drag(&mut t, &mut cx, (-40.0, -40.0), (80.0, 160.0));
        // Drag the Rotate handle a quarter turn about the region's center.
        let (lo, hi) = region().unwrap();
        let h = rotate_handle(lo, hi, cx.px_per_in);
        let center = center_of(lo, hi);
        let to = center + (h - center).perp();
        drag(&mut t, &mut cx, (h.x, h.y), (to.x, to.y));
        let w = cx.floor().wall(ids[0]).unwrap();
        // Its start turned, its end stayed: the wall no longer lies along x.
        assert_eq!(w.end, Point::new(240.0, 0.0));
        assert!((w.start.x - 0.0).abs() > 1.0 || (w.start.y - 0.0).abs() > 1.0);
        assert!(cx.floor().openings.iter().any(|o| o.id == door));
        // Turning ends the mode.
        assert!(!active());
    }

    #[test]
    fn ctrl_drag_inside_the_edit_area_copies_everything_inside() {
        let mut cx = cx();
        let ids = house(&mut cx);
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        drag(&mut t, &mut cx, (-20.0, -20.0), (260.0, 140.0));
        let mut down = ev(&cx, 100.0, 60.0).with_down(true);
        down.modifiers.ctrl = true;
        let _ = t.pointer_down(&mut cx, down);
        mv(&mut t, &mut cx, 100.0, 260.0);
        let upr = up(&mut t, &mut cx, 100.0, 260.0);
        assert_eq!(upr.commit.as_deref(), Some("Copy Edit Area"));
        assert_eq!(cx.floor().walls.len(), 8);
        assert_eq!(cx.floor().wall(ids[0]).unwrap().start, Point::new(0.0, 0.0));
        cx.undo();
        assert_eq!(cx.floor().walls.len(), 4);
    }

    #[test]
    fn delete_removes_what_lies_wholly_inside() {
        let mut cx = cx();
        house(&mut cx);
        let line = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(300.0, 0.0),
                b: Point::new(340.0, 0.0),
            },
        );
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        drag(&mut t, &mut cx, (-20.0, -20.0), (260.0, 140.0));
        let res = t.key(&mut cx, KeyEvent::key(Key::Delete));
        assert_eq!(res.commit.as_deref(), Some("Delete Edit Area"));
        assert!(cx.floor().walls.is_empty());
        assert!(cx.floor().cad.iter().any(|c| c.id == line), "outside stays");
    }

    #[test]
    fn stretch_cad_moves_only_the_vertices_inside_the_window() {
        let mut cx = cx();
        let line = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(100.0, 0.0),
            },
        );
        let poly = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Polyline {
                points: vec![
                    Point::new(0.0, 50.0),
                    Point::new(100.0, 50.0),
                    Point::new(100.0, 90.0),
                ],
                closed: false,
            },
        );
        let mut t = SelectTool::default();
        begin(&mut cx, AreaKind::StretchCad);
        // A window around the right-hand ends.
        drag(&mut t, &mut cx, (70.0, -20.0), (130.0, 110.0));
        drag(&mut t, &mut cx, (100.0, 20.0), (130.0, 20.0));
        let f = cx.floor();
        let CadItem::Line { a, b } = &f.cad.iter().find(|c| c.id == line).unwrap().item else {
            panic!()
        };
        assert_eq!((*a, *b), (Point::new(0.0, 0.0), Point::new(130.0, 0.0)));
        let CadItem::Polyline { points, .. } = &f.cad.iter().find(|c| c.id == poly).unwrap().item
        else {
            panic!()
        };
        assert_eq!(points[0], Point::new(0.0, 50.0));
        assert_eq!(points[1], Point::new(130.0, 50.0));
        assert_eq!(points[2], Point::new(130.0, 90.0));
        // One-shot: the mode is over and one undo restores the lines.
        assert!(!active());
        cx.undo();
        let CadItem::Line { b, .. } = &cx.floor().cad[0].item else {
            panic!()
        };
        assert_eq!(*b, Point::new(100.0, 0.0));
    }

    #[test]
    fn typed_distance_moves_the_edit_area_exactly() {
        let mut cx = cx();
        let ids = house(&mut cx);
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        drag(&mut t, &mut cx, (100.0, -40.0), (300.0, 160.0));
        down(&mut t, &mut cx, 200.0, 60.0);
        mv(&mut t, &mut cx, 212.0, 60.0);
        cx.cursor_world = Some(Point::new(212.0, 60.0));
        for ch in ["1", "0", "'"] {
            let r = t.key(&mut cx, KeyEvent::text(ch));
            assert!(r.consumed, "{ch} goes to the number");
        }
        assert!(cx.readout.as_deref().unwrap().contains("10'"));
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(r.commit.is_some());
        assert_eq!(cx.floor().wall(ids[0]).unwrap().end, Point::new(360.0, 0.0));
    }

    #[test]
    fn escape_cancels_a_drag_then_ends_the_mode() {
        let mut cx = cx();
        let ids = house(&mut cx);
        let mut t = SelectTool::default();
        begin(&mut cx, AreaKind::Edit { visible_only: true });
        drag(&mut t, &mut cx, (100.0, -40.0), (300.0, 160.0));
        down(&mut t, &mut cx, 200.0, 60.0);
        mv(&mut t, &mut cx, 260.0, 60.0);
        assert_eq!(cx.floor().wall(ids[0]).unwrap().end, Point::new(300.0, 0.0));
        let _ = t.key(&mut cx, KeyEvent::escape());
        assert!(!active(), "Esc ends the mode and the drag");
        assert_eq!(cx.floor().wall(ids[0]).unwrap().end, Point::new(240.0, 0.0));
        assert!(!cx.can_undo() || cx.undo_label() != Some("Move Edit Area"));
    }

    #[test]
    fn a_press_outside_the_region_ends_the_mode() {
        let mut cx = cx();
        house(&mut cx);
        let mut t = SelectTool::default();
        begin(
            &mut cx,
            AreaKind::Edit {
                visible_only: false,
            },
        );
        drag(&mut t, &mut cx, (100.0, -40.0), (300.0, 160.0));
        assert!(active());
        down(&mut t, &mut cx, -500.0, -500.0);
        assert!(!active());
    }
}
