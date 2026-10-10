//! Wall Edit toolbar commands (W-23, W-43, W-44, W-66..W-68, S-41, S-42,
//! S-55): Break Wall, Remove Break, Reverse Layers, Change Line/Arc and
//! Make Arc Tangent. The buttons come from `EditorContext::extra_edit_actions`
//! and run through `run_custom`; Break Wall waits for a click on the wall.

use super::selection::{hit_test_cx, ObjectRef};
use super::EditorContext;
use plan_core::geometry::Point;
use plan_core::{Id, WallEnd};
use std::cell::Cell;

/// Custom command ids (the `id` of `EditActionKind::Custom`).
pub const BREAK_WALL: &str = "wall.break";
pub const REMOVE_BREAK: &str = "wall.remove_break";
pub const REVERSE_LAYERS: &str = "wall.reverse_layers";
pub const CHANGE_LINE_ARC: &str = "wall.line_arc";
pub const ARC_TANGENT: &str = "wall.arc_tangent";
pub const CONVERT_POLYLINE: &str = "wall.to_polyline";

thread_local! {
    /// Break Wall was picked and waits for the click that sets the point.
    static BREAK_PENDING: Cell<bool> = const { Cell::new(false) };
}

/// Is Break Wall waiting for its click?
pub fn break_pending() -> bool {
    BREAK_PENDING.with(Cell::get)
}

/// Drops a pending Break Wall; true when there was one (Esc).
pub fn cancel_break() -> bool {
    BREAK_PENDING.with(|b| b.replace(false))
}

/// Two straight walls that cross in the middle and were left whole (Walls
/// Connect with Split Walls at T-Intersections off) draw as one shape (W-36):
/// the lines of each inside the other are covered with the main fill and the
/// outline of the pair is drawn over. `joins::crossing_merges` finds them.
pub fn draw_crossing_merges(
    cx: &EditorContext,
    painter: &eframe::egui::Painter,
    cam: &super::Camera,
) {
    use eframe::egui::{Shape, Stroke};
    // Splitting is the default; only the plan that opted out has crossings.
    if cx.defaults.walls_connect.split_on_tee {
        return;
    }
    let walls = &cx.floor().walls;
    if walls.len() < 2 {
        return;
    }
    let pal = &cx.palette;
    for m in plan_core::joins::crossing_merges(walls, 0.5) {
        let (a, b) = (
            cx.floor().wall(m.wall_ids[0]),
            cx.floor().wall(m.wall_ids[1]),
        );
        let (Some(a), Some(b)) = (a, b) else { continue };
        if !cx.layers().is_visible(&a.layer) || !cx.layers().is_visible(&b.layer) {
            continue;
        }
        let thick = if a.thickness >= b.thickness { a } else { b };
        let fill = match thick.kind {
            plan_core::WallKind::Exterior => pal.wall_fill_exterior,
            plan_core::WallKind::Interior => pal.wall_fill_interior,
        };
        let fill = if cam.px_per_in >= 1.0 {
            crate::theme::scale(fill, 0.78)
        } else {
            fill
        };
        let pts = |poly: &[Point]| -> Vec<eframe::egui::Pos2> {
            poly.iter().map(|p| cam.world_to_screen(*p)).collect()
        };
        // The stroke in the fill color wipes the overlap's own edges.
        painter.add(Shape::convex_polygon(
            pts(&m.overlap),
            fill,
            Stroke::new(2.0_f32, fill),
        ));
        painter.add(Shape::closed_line(
            pts(&m.outline),
            Stroke::new(1.5_f32, pal.wall_stroke),
        ));
    }
}

/// The wall buttons of the Edit toolbar for the current selection: Reverse
/// Layers for any number of walls, the others for one wall (Make Arc Tangent
/// only for a curved one).
pub fn edit_actions(cx: &EditorContext) -> Vec<super::EditAction> {
    let walls = selected_walls(cx);
    if walls.is_empty() {
        return Vec::new();
    }
    let button = |id, label, icon: &'static str, enabled| super::EditAction {
        kind: super::EditActionKind::Custom { id, label, icon },
        label,
        icon: (!icon.is_empty()).then_some(icon),
        enabled,
    };
    let mut v = vec![button(REVERSE_LAYERS, "Reverse Layers", "", true)];
    v.push(button(CONVERT_POLYLINE, "Convert to Polyline", "", true));
    if let (1, Some(w)) = (walls.len(), cx.floor().wall(walls[0])) {
        let single = cx.selection.single().is_some();
        v.push(button(BREAK_WALL, "Break Wall", "wall_break", single));
        v.push(button(REMOVE_BREAK, "Remove Break", "", single));
        v.push(button(
            CHANGE_LINE_ARC,
            "Change Line/Arc",
            "wall_curved",
            single,
        ));
        v.push(button(
            ARC_TANGENT,
            "Make Arc Tangent",
            "",
            single && w.is_curved(),
        ));
    }
    v
}

/// Where a dragged wall end lands (W-15..W-18): the snapped point, held to
/// the angle increment by Shift, or replaced by the typed length and angle.
/// Also sets the status readout while typing.
pub fn drag_end(
    cx: &mut EditorContext,
    fixed: Point,
    raw: Point,
    snapped: Point,
    shift: bool,
    alt: bool,
) -> Point {
    let mut to = snapped;
    if shift && !alt {
        let e = &cx.defaults.editing;
        let inc = if e.angle_snap_deg >= 1.0 {
            e.angle_snap_deg
        } else {
            15.0
        };
        let held = if e.snap_angles.is_empty() {
            super::snap::angle_snap(fixed, raw, cx.defaults.grid.snap, inc)
        } else {
            super::snap::angle_snap_list(fixed, raw, cx.defaults.grid.snap, &e.snap_angles)
        };
        if let Some(p) = held {
            to = p;
        }
    }
    if cx.typed_input.is_armed() {
        if let Some(p) = super::tempdim::typed_point(cx, fixed, raw) {
            to = p;
        }
        let ti = &cx.typed_input;
        let field = |t: &str, live: String, on: bool| {
            if ti.has_text() && on {
                format!("{t}|")
            } else {
                live
            }
        };
        cx.readout = Some(format!(
            "Length: {}   Angle: {}",
            field(
                ti.length_text(),
                cx.fmt_dim(fixed.dist(to)),
                ti.field() == super::typed_input::TypedField::Length
            ),
            field(
                ti.angle_text(),
                format!("{:.1}\u{b0}", super::typed_input::angle_deg(fixed, to)),
                ti.field() == super::typed_input::TypedField::Angle
            ),
        ));
    }
    to
}

/// The wall ids in the selection.
pub fn selected_walls(cx: &EditorContext) -> Vec<Id> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Wall(id) => Some(*id),
            _ => None,
        })
        .collect()
}

/// Runs a wall command by id. False when `id` is not one of ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        BREAK_WALL => {
            BREAK_PENDING.with(|b| b.set(true));
            cx.status = "Break Wall: click the wall where it should break (Esc cancels)".into();
        }
        REMOVE_BREAK => {
            remove_break(cx);
        }
        REVERSE_LAYERS => {
            reverse_layers(cx);
        }
        CHANGE_LINE_ARC => {
            change_line_arc(cx);
        }
        ARC_TANGENT => {
            make_arc_tangent(cx);
        }
        CONVERT_POLYLINE => {
            convert_to_polyline(cx);
        }
        _ => return false,
    }
    true
}

/// Reverse Layers on every selected wall (W-23): one undo step.
pub fn reverse_layers(cx: &mut EditorContext) -> usize {
    let ids = selected_walls(cx);
    if ids.is_empty() || ids.iter().any(|i| !cx.check_unlocked(ObjectRef::Wall(*i))) {
        return 0;
    }
    cx.begin_change("Reverse Layers");
    let fl = cx.floor;
    let n = ids
        .iter()
        .filter(|id| cx.project.reverse_wall_layers(fl, **id))
        .count();
    cx.mark_dirty();
    cx.status = format!(
        "Reversed the layers of {n} wall{}",
        if n == 1 { "" } else { "s" }
    );
    n
}

/// Change Line/Arc on the selected wall (W-67): a straight wall becomes an
/// arc with a bulge handle, a curved one becomes straight. One undo step.
pub fn change_line_arc(cx: &mut EditorContext) -> Option<bool> {
    let Some(ObjectRef::Wall(id)) = cx.selection.single() else {
        cx.status = "Select one wall to change between line and arc".into();
        return None;
    };
    if !cx.check_unlocked(ObjectRef::Wall(id)) {
        return None;
    }
    cx.begin_change("Change Line/Arc");
    let fl = cx.floor;
    let now = cx.project.change_line_arc(fl, id, 0.0);
    if now.is_none() {
        cx.cancel_change();
        cx.status = "That wall is too short to curve".into();
        return None;
    }
    cx.mark_dirty();
    cx.status = if now == Some(true) {
        "Curved: drag the handle at the apex to set the bulge".into()
    } else {
        "Straightened the wall".into()
    };
    now
}

/// Make Arc Tangent on the selected curved wall (S-55, W-68): one undo step.
pub fn make_arc_tangent(cx: &mut EditorContext) -> Option<WallEnd> {
    let Some(ObjectRef::Wall(id)) = cx.selection.single() else {
        cx.status = "Select one curved wall to make tangent".into();
        return None;
    };
    if !cx.check_unlocked(ObjectRef::Wall(id)) {
        return None;
    }
    cx.begin_change("Make Arc Tangent");
    let fl = cx.floor;
    match cx.project.make_arc_tangent(fl, id) {
        Ok(end) => {
            cx.mark_dirty();
            cx.status = format!(
                "The arc is tangent to the wall at its {}",
                if end == WallEnd::Start {
                    "start"
                } else {
                    "end"
                }
            );
            Some(end)
        }
        Err(e) => {
            cx.cancel_change();
            cx.status = e;
            None
        }
    }
}

/// Convert to Polyline (W-90): the selected walls' centerlines become CAD
/// polylines (connected straight walls join into one chain; an arc becomes a
/// run of segments) and the walls, with their openings, are removed. One undo
/// step. Returns the new polyline ids.
pub fn convert_to_polyline(cx: &mut EditorContext) -> Vec<Id> {
    let walls = selected_walls(cx);
    if walls.is_empty() {
        cx.status = "Select the walls to convert".into();
        return Vec::new();
    }
    if walls
        .iter()
        .any(|i| !cx.check_unlocked(ObjectRef::Wall(*i)))
    {
        return Vec::new();
    }
    let fl = cx.floor;
    let mut straight = Vec::new();
    let mut lines: Vec<(Vec<Point>, bool)> = Vec::new();
    for id in &walls {
        let Some(w) = cx.floor().wall(*id) else {
            continue;
        };
        if w.is_curved() {
            lines.push((w.sample_points(24), false));
        } else {
            straight.push((w.start, w.end));
        }
    }
    lines.extend(plan_core::cad::lines_to_polylines(&straight, 0.5));
    cx.begin_change("Convert to Polyline");
    let mut made = Vec::new();
    for (points, closed) in lines {
        made.push(cx.project.add_cad(
            fl,
            plan_core::cad::DEFAULT_CAD_LAYER,
            plan_core::CadItem::Polyline { points, closed },
        ));
    }
    let doomed: Vec<ObjectRef> = walls.iter().map(|i| ObjectRef::Wall(*i)).collect();
    super::ops::delete_objects(&mut cx.project, fl, &doomed);
    super::details_view::drop_orphans(cx);
    cx.selection.items = made.iter().map(|i| ObjectRef::Cad(*i)).collect();
    cx.mark_dirty();
    cx.status = format!(
        "Converted {} wall{} to {} polyline{}",
        walls.len(),
        if walls.len() == 1 { "" } else { "s" },
        made.len(),
        if made.len() == 1 { "" } else { "s" }
    );
    made
}

/// Remove Break on the selected wall: merges it with the straight wall that
/// continues it. One undo step.
pub fn remove_break(cx: &mut EditorContext) -> Option<Id> {
    let Some(ObjectRef::Wall(id)) = cx.selection.single() else {
        cx.status = "Select one wall to remove a break from".into();
        return None;
    };
    if !cx.check_unlocked(ObjectRef::Wall(id)) {
        return None;
    }
    let fl = cx.floor;
    let next = cx
        .project
        .wall_connections(fl, id)
        .into_iter()
        .filter(|c| c.kind == plan_core::joins::ConnectionKind::Through)
        .map(|c| c.other)
        .next();
    let Some(other) = next else {
        cx.status = "No wall continues this one in a straight line".into();
        return None;
    };
    cx.begin_change("Remove Break");
    match cx.project.join_collinear_walls(fl, id, other) {
        Some(merged) => {
            cx.selection.set(ObjectRef::Wall(merged));
            cx.mark_dirty();
            cx.status = "Removed the break".into();
            Some(merged)
        }
        None => {
            cx.cancel_change();
            cx.status = "The walls differ in thickness, type or height and cannot be merged".into();
            None
        }
    }
}

/// Break Wall at `point` (W-43): the wall splits in two joined walls; the
/// openings go to the half that holds them. Returns the two ids.
pub fn break_wall_at(cx: &mut EditorContext, id: Id, point: Point) -> Option<(Id, Id)> {
    if !cx.check_unlocked(ObjectRef::Wall(id)) {
        return None;
    }
    cx.begin_change("Break Wall");
    let fl = cx.floor;
    match cx.project.break_wall(fl, id, point) {
        Some((a, b)) => {
            cx.selection.set(ObjectRef::Wall(a));
            cx.mark_dirty();
            cx.status = "Broke the wall in two".into();
            Some((a, b))
        }
        None => {
            cx.cancel_change();
            cx.status =
                "Cannot break the wall there: too close to an end or inside an opening".into();
            None
        }
    }
}

/// The click that sets the Break Wall point: the wall under `world` (the
/// selected wall wins). Stays pending when the click misses every wall or the
/// break is refused. Returns whether the wall was broken.
pub fn break_click(cx: &mut EditorContext, world: Point) -> bool {
    let tol = cx.pick_tol();
    let hits = hit_test_cx(cx, world, tol);
    let selected = cx.selection.single();
    let target = hits
        .iter()
        .copied()
        .find(|h| Some(*h) == selected && matches!(h, ObjectRef::Wall(_)))
        .or_else(|| hits.into_iter().find(|h| matches!(h, ObjectRef::Wall(_))));
    let Some(ObjectRef::Wall(id)) = target else {
        cx.status = "Break Wall: click on a wall (Esc cancels)".into();
        return false;
    };
    let done = break_wall_at(cx, id, world).is_some();
    if done {
        BREAK_PENDING.with(|b| b.set(false));
    }
    done
}

/// The shortest thickness `wall` may be given: the layers of its wall type
/// that are not the main layer plus a sliver of main layer (W-30).
pub fn min_thickness(cx: &EditorContext, wall: &plan_core::Wall) -> f64 {
    let ty = wall
        .wall_type
        .as_deref()
        .and_then(|n| cx.wall_types().iter().find(|t| t.name == n));
    plan_core::walls::min_thickness(ty)
}

/// A typed length for wall `id` (its start staying put), held at the shortest
/// that still hosts the wall's openings, with a warning in the status line
/// when it had to be raised (W-85). Other lengths come back unchanged.
pub fn clamp_length(cx: &mut EditorContext, id: Id, value: f64) -> f64 {
    let min = cx
        .project
        .min_wall_length(cx.floor, id, plan_core::walls::LengthLock::Start);
    if value + 1e-9 >= min {
        return value;
    }
    cx.status = format!(
        "That length is too short for the wall's openings; held at {}",
        cx.fmt_dim(min)
    );
    min
}

/// The end `to` of wall `id` (the `end` being moved, the other staying put)
/// brought back along the wall's line to the shortest length that still hosts
/// its openings, with a warning (W-85). A curved wall or a point that is long
/// enough comes back unchanged.
pub fn clamp_end_for_openings(cx: &mut EditorContext, id: Id, end: WallEnd, to: Point) -> Point {
    let Some(w) = cx.floor().wall(id) else {
        return to;
    };
    if w.is_curved() {
        return to;
    }
    let (fixed, lock) = match end {
        WallEnd::Start => (w.end, plan_core::walls::LengthLock::End),
        WallEnd::End => (w.start, plan_core::walls::LengthLock::Start),
    };
    let len = fixed.dist(to);
    let min = cx.project.min_wall_length(cx.floor, id, lock);
    if len < 1e-9 || len + 1e-9 >= min {
        return to;
    }
    cx.status = format!(
        "That length is too short for the wall's openings; held at {}",
        cx.fmt_dim(min)
    );
    fixed + (to - fixed).normalized() * min
}

/// After a wall's start or end was set by a dialog (the wall already holds
/// its new geometry): the walls joined to the old end points follow them and
/// walls resting on its side stay on its new line, with their far ends fixed
/// (W-76), exactly as when the end is dragged. `before` is the wall as it was.
pub fn follow_moved_ends(cx: &mut EditorContext, id: Id, before: &plan_core::Wall) {
    let fl = cx.floor;
    let Some(now) = cx.floor().wall(id).cloned() else {
        return;
    };
    // Put the old geometry back, then move each end the way a drag would.
    if let Some(w) = cx.project.floors[fl].wall_mut(id) {
        w.start = before.start;
        w.end = before.end;
    }
    for (end, to) in [(WallEnd::Start, now.start), (WallEnd::End, now.end)] {
        let from = match end {
            WallEnd::Start => before.start,
            WallEnd::End => before.end,
        };
        if from.dist(to) > 1e-9 {
            super::ops::move_wall_end_joined(&mut cx.project, fl, id, end, to);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{joins, OpeningKind, WallKind};

    fn cx_with_wall() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        cx.selection.set(ObjectRef::Wall(id));
        (cx, id)
    }

    #[test]
    fn break_wall_splits_and_both_halves_connect() {
        let (mut cx, id) = cx_with_wall();
        let win = cx
            .project
            .add_opening(0, id, 180.0, OpeningKind::Window)
            .unwrap();
        assert!(run_command(&mut cx, BREAK_WALL));
        assert!(break_pending());
        // A click off every wall keeps the mode.
        assert!(!break_click(&mut cx, Point::new(120.0, 80.0)));
        assert!(break_pending());
        assert!(break_click(&mut cx, Point::new(100.0, 1.5)));
        assert!(!break_pending());
        let walls = &cx.floor().walls;
        assert_eq!(walls.len(), 2);
        let (a, b) = (&walls[0], &walls[1]);
        assert_eq!(
            (a.start, a.end),
            (Point::new(0.0, 0.0), Point::new(100.0, 0.0))
        );
        assert_eq!(
            (b.start, b.end),
            (Point::new(100.0, 0.0), Point::new(240.0, 0.0))
        );
        assert_eq!((a.thickness, b.thickness), (6.0, 6.0));
        assert_eq!(a.kind, b.kind);
        // The window went with the half that holds it.
        let o = cx.floor().openings.iter().find(|o| o.id == win).unwrap();
        assert_eq!(o.wall_id, b.id);
        assert_eq!(o.center_offset, 80.0);
        // Both halves are joined end to end.
        let conns = cx.project.wall_connections(0, a.id);
        assert!(conns.iter().any(|c| c.other == b.id
            && c.at == WallEnd::End
            && c.kind == joins::ConnectionKind::Through));
        assert_eq!(cx.undo().as_deref(), Some("Break Wall"));
        assert_eq!(cx.floor().walls.len(), 1);
    }

    #[test]
    fn break_wall_refuses_an_opening_and_the_ends() {
        let (mut cx, id) = cx_with_wall();
        cx.project
            .add_opening(0, id, 120.0, OpeningKind::Window)
            .unwrap();
        assert!(break_wall_at(&mut cx, id, Point::new(120.0, 0.0)).is_none());
        assert!(cx.status.contains("opening"));
        assert!(break_wall_at(&mut cx, id, Point::new(0.0, 0.0)).is_none());
        assert_eq!(cx.floor().walls.len(), 1);
        // Nothing was recorded for the refusals.
        assert!(cx.undo().is_none());
    }

    #[test]
    fn remove_break_merges_the_halves_back() {
        let (mut cx, id) = cx_with_wall();
        let (a, _) = break_wall_at(&mut cx, id, Point::new(100.0, 0.0)).unwrap();
        cx.selection.set(ObjectRef::Wall(a));
        assert!(remove_break(&mut cx).is_some());
        let walls = &cx.floor().walls;
        assert_eq!(walls.len(), 1);
        assert_eq!(
            (walls[0].start, walls[0].end),
            (Point::new(0.0, 0.0), Point::new(240.0, 0.0))
        );
        // A lone wall has nothing to merge with.
        assert!(remove_break(&mut cx).is_none());
    }

    #[test]
    fn reverse_layers_runs_on_every_selected_wall() {
        let (mut cx, id) = cx_with_wall();
        let other = cx.project.add_wall(
            0,
            Point::new(0.0, 100.0),
            Point::new(240.0, 100.0),
            4.0,
            100.0,
            WallKind::Interior,
        );
        cx.selection.items = vec![ObjectRef::Wall(id), ObjectRef::Wall(other)];
        let before: Vec<_> = cx.floor().walls.iter().map(|w| w.exterior_side).collect();
        assert!(run_command(&mut cx, REVERSE_LAYERS));
        for (w, b) in cx.floor().walls.iter().zip(before.clone()) {
            assert_eq!(w.exterior_side, b.opposite());
        }
        assert_eq!(cx.undo().as_deref(), Some("Reverse Layers"));
        let after: Vec<_> = cx.floor().walls.iter().map(|w| w.exterior_side).collect();
        assert_eq!(after, before);
    }

    #[test]
    fn change_line_arc_and_arc_tangent_commands() {
        let (mut cx, id) = cx_with_wall();
        let next = cx.project.add_wall(
            0,
            Point::new(240.0, 0.0),
            Point::new(240.0, 120.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        // Tangent needs a curved wall.
        assert!(make_arc_tangent(&mut cx).is_none());
        assert!(cx.status.contains("curved"));
        assert_eq!(change_line_arc(&mut cx), Some(true));
        let w = cx.floor().wall(id).unwrap();
        assert_eq!(w.curve.unwrap().bulge, 60.0);
        // The first wall now meets the next one at its end; the arc is made
        // tangent to it there (the next wall runs north).
        assert_eq!(make_arc_tangent(&mut cx), Some(WallEnd::End));
        let w = cx.floor().wall(id).unwrap();
        let arrive = -w.end_tangent(WallEnd::End);
        assert!(
            arrive.x.abs() < 1e-9 && (arrive.y - 1.0).abs() < 1e-9,
            "{arrive:?}"
        );
        let _ = next;
        assert_eq!(cx.undo().as_deref(), Some("Make Arc Tangent"));
        assert_eq!(change_line_arc(&mut cx), Some(false));
        assert!(cx.floor().wall(id).unwrap().curve.is_none());
        // Several walls selected: nothing happens.
        cx.selection.items = vec![ObjectRef::Wall(id), ObjectRef::Wall(next)];
        assert_eq!(change_line_arc(&mut cx), None);
    }

    #[test]
    fn convert_to_polyline_chains_the_centerlines() {
        let (mut cx, a) = cx_with_wall();
        let b = cx.project.add_wall(
            0,
            Point::new(240.0, 0.0),
            Point::new(240.0, 120.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let curved = cx.project.add_wall(
            0,
            Point::new(500.0, 0.0),
            Point::new(600.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        cx.project
            .set_wall_curve(0, curved, Some(plan_core::WallCurve { bulge: 20.0 }));
        cx.project
            .add_opening(0, a, 60.0, OpeningKind::Door)
            .unwrap();
        cx.selection.items = vec![
            ObjectRef::Wall(a),
            ObjectRef::Wall(b),
            ObjectRef::Wall(curved),
        ];
        let made = convert_to_polyline(&mut cx);
        assert_eq!(made.len(), 2);
        assert!(cx.floor().walls.is_empty() && cx.floor().openings.is_empty());
        let polys: Vec<usize> = cx
            .floor()
            .cad
            .iter()
            .filter_map(|c| match &c.item {
                plan_core::CadItem::Polyline { points, closed } => {
                    assert!(!closed);
                    Some(points.len())
                }
                _ => None,
            })
            .collect();
        // The curved wall samples 25 points; the two straight walls chain.
        assert!(polys.contains(&25) && polys.contains(&3), "{polys:?}");
        assert_eq!(cx.selection.items.len(), 2);
        assert_eq!(cx.undo().as_deref(), Some("Convert to Polyline"));
        assert_eq!(cx.floor().walls.len(), 3);
        assert!(cx.floor().cad.is_empty());
    }

    mod select_drags {
        use super::*;
        use crate::tools::select::SelectTool;
        use crate::tools::{KeyEvent, PointerEvent, Tool};
        use eframe::egui::{Key, Modifiers};

        fn press(tool: &mut SelectTool, cx: &mut EditorContext, at: Point) {
            let p = PointerEvent::at(cx, at).with_down(true);
            tool.pointer_down(cx, p);
        }

        fn drag_to(tool: &mut SelectTool, cx: &mut EditorContext, to: Point, m: Modifiers) {
            cx.cursor_world = Some(to);
            let p = PointerEvent::at(cx, to).with_modifiers(m).with_down(true);
            tool.pointer_move(cx, p);
        }

        #[test]
        fn dragging_the_bulge_handle_curves_and_flattens_the_wall() {
            let (mut cx, id) = cx_with_wall();
            cx.project
                .set_wall_curve(0, id, Some(plan_core::WallCurve { bulge: 30.0 }));
            let mut tool = SelectTool::default();
            press(&mut tool, &mut cx, Point::new(120.0, 30.0));
            drag_to(&mut tool, &mut cx, Point::new(120.0, 50.0), Modifiers::NONE);
            let p = PointerEvent::at(&cx, Point::new(120.0, 50.0));
            tool.pointer_up(&mut cx, p);
            assert_eq!(cx.floor().wall(id).unwrap().curve.unwrap().bulge, 50.0);
            assert_eq!(cx.undo().as_deref(), Some("Curve Wall"));
            assert_eq!(cx.floor().wall(id).unwrap().curve.unwrap().bulge, 30.0);
            // Dragging across the chord bulges the other way; onto it, flat.
            press(&mut tool, &mut cx, Point::new(120.0, 30.0));
            drag_to(
                &mut tool,
                &mut cx,
                Point::new(120.0, -20.0),
                Modifiers::NONE,
            );
            assert_eq!(cx.floor().wall(id).unwrap().curve.unwrap().bulge, -20.0);
            drag_to(&mut tool, &mut cx, Point::new(120.0, 0.2), Modifiers::NONE);
            let p = PointerEvent::at(&cx, Point::new(120.0, 0.2));
            tool.pointer_up(&mut cx, p);
            assert!(cx.floor().wall(id).unwrap().curve.is_none());
            // The bulge never passes a semicircle.
            cx.project
                .set_wall_curve(0, id, Some(plan_core::WallCurve { bulge: 30.0 }));
            press(&mut tool, &mut cx, Point::new(120.0, 30.0));
            drag_to(
                &mut tool,
                &mut cx,
                Point::new(120.0, 900.0),
                Modifiers::NONE,
            );
            assert_eq!(cx.floor().wall(id).unwrap().curve.unwrap().bulge, 120.0);
        }

        #[test]
        fn typing_while_a_wall_end_drags_sets_its_length_and_angle() {
            let (mut cx, id) = cx_with_wall();
            let mut tool = SelectTool::default();
            press(&mut tool, &mut cx, Point::new(240.0, 0.0));
            drag_to(&mut tool, &mut cx, Point::new(300.0, 20.0), Modifiers::NONE);
            assert!(cx.typed_input.is_armed());
            assert!(tool.key(&mut cx, KeyEvent::text("10'")).consumed);
            // The wall follows the typed length along the pointer's direction.
            let w = cx.floor().wall(id).unwrap();
            assert!((w.length() - 120.0).abs() < 1e-6, "{}", w.length());
            assert!(cx.readout.as_deref().unwrap().contains("10'|"));
            assert!(tool.key(&mut cx, KeyEvent::key(Key::Tab)).consumed);
            assert!(tool.key(&mut cx, KeyEvent::text("90")).consumed);
            let r = tool.key(&mut cx, KeyEvent::key(Key::Enter));
            assert_eq!(r.commit.as_deref(), Some("Stretch Wall"));
            let w = cx.floor().wall(id).unwrap();
            assert_eq!((w.start, w.end), (Point::ZERO, Point::new(0.0, 120.0)));
            assert!(!cx.typed_input.is_armed() && cx.readout.is_none());
            assert_eq!(cx.undo().as_deref(), Some("Stretch Wall"));
            assert_eq!(cx.floor().wall(id).unwrap().end, Point::new(240.0, 0.0));
        }

        #[test]
        fn esc_drops_the_typed_text_then_the_drag() {
            let (mut cx, id) = cx_with_wall();
            let mut tool = SelectTool::default();
            press(&mut tool, &mut cx, Point::new(240.0, 0.0));
            drag_to(&mut tool, &mut cx, Point::new(300.0, 0.0), Modifiers::NONE);
            tool.key(&mut cx, KeyEvent::text("5"));
            assert!(cx.typed_input.has_text());
            tool.key(&mut cx, KeyEvent::escape());
            assert!(!cx.typed_input.has_text() && cx.typed_input.is_armed());
            // The wall is back at the pointer, not the typed length.
            assert_eq!(cx.floor().wall(id).unwrap().end, Point::new(300.0, 0.0));
            tool.key(&mut cx, KeyEvent::escape());
            assert_eq!(cx.floor().wall(id).unwrap().end, Point::new(240.0, 0.0));
            assert!(!cx.typed_input.is_armed());
        }

        #[test]
        fn shift_holds_the_angle_and_alt_frees_the_stretched_end() {
            let (mut cx, id) = cx_with_wall();
            cx.defaults.editing.angle_snaps = false;
            cx.defaults.editing.angle_snap_deg = 45.0;
            let mut tool = SelectTool::default();
            press(&mut tool, &mut cx, Point::new(240.0, 0.0));
            let shift = Modifiers {
                shift: true,
                ..Modifiers::NONE
            };
            drag_to(&mut tool, &mut cx, Point::new(200.0, 140.0), shift);
            let w = cx.floor().wall(id).unwrap();
            let v = w.end - w.start;
            assert!((v.x - v.y).abs() < 1e-9 && v.x > 100.0, "{v:?}");
            // Alt: no snap at all, so the end sits exactly at the pointer.
            let alt = Modifiers {
                alt: true,
                ..Modifiers::NONE
            };
            drag_to(&mut tool, &mut cx, Point::new(200.3, 140.7), alt);
            assert_eq!(cx.floor().wall(id).unwrap().end, Point::new(200.3, 140.7));
            let p = PointerEvent::at(&cx, Point::new(200.3, 140.7));
            tool.pointer_up(&mut cx, p);
        }

        #[test]
        fn break_wall_waits_for_a_click_in_the_select_tool() {
            let (mut cx, id) = cx_with_wall();
            assert!(run_command(&mut cx, BREAK_WALL));
            let mut tool = SelectTool::default();
            // Esc cancels the pending break.
            tool.key(&mut cx, KeyEvent::escape());
            assert!(!break_pending());
            run_command(&mut cx, BREAK_WALL);
            let p = PointerEvent::at(&cx, Point::new(90.0, 1.0)).with_down(true);
            let r = tool.pointer_down(&mut cx, p);
            assert_eq!(r.commit.as_deref(), Some("Break Wall"));
            assert_eq!(cx.floor().walls.len(), 2);
            assert_eq!(cx.floor().wall(id).unwrap().end, Point::new(90.0, 0.0));
        }
    }

    #[test]
    fn a_typed_length_is_held_at_what_the_openings_need() {
        let (mut cx, id) = cx_with_wall();
        let win = cx
            .project
            .add_opening(0, id, 180.0, OpeningKind::Window)
            .unwrap();
        let o = cx
            .floor()
            .openings
            .iter()
            .find(|o| o.id == win)
            .unwrap()
            .clone();
        let min = o.end_offset() + plan_core::walls::OPENING_JAMB_MARGIN;
        // Long enough: untouched, no warning.
        assert_eq!(clamp_length(&mut cx, id, 200.0), 200.0);
        assert!(cx.status.is_empty() || !cx.status.contains("too short"));
        // Too short: raised to what the window needs.
        let held = clamp_length(&mut cx, id, 100.0);
        assert!((held - min).abs() < 1e-9, "{held} vs {min}");
        assert!(cx.status.contains("too short"), "{}", cx.status);
        // The same for a dragged or typed end, along the wall's line.
        cx.status.clear();
        let to = clamp_end_for_openings(&mut cx, id, WallEnd::End, Point::new(100.0, 0.0));
        assert!((to.x - min).abs() < 1e-9 && to.y == 0.0, "{to:?}");
        assert!(cx.status.contains("too short"));
        // Moving the start keeps the openings' distance from the end.
        cx.status.clear();
        let to = clamp_end_for_openings(&mut cx, id, WallEnd::Start, Point::new(200.0, 0.0));
        assert!(to.x < 200.0, "{to:?}");
        let start_min = 240.0 - to.x;
        assert!(
            (start_min
                - cx.project
                    .min_wall_length(0, id, plan_core::walls::LengthLock::End))
            .abs()
                < 1e-9
        );
        // A wall with no openings is never held.
        let (mut cx2, id2) = cx_with_wall();
        assert_eq!(clamp_length(&mut cx2, id2, 2.0), 2.0);
        assert_eq!(
            clamp_end_for_openings(&mut cx2, id2, WallEnd::End, Point::new(5.0, 0.0)),
            Point::new(5.0, 0.0)
        );
    }

    #[test]
    fn follow_moved_ends_drags_the_joined_walls_with_a_dialog_edit() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let a = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let b = cx.project.add_wall(
            0,
            Point::new(240.0, 0.0),
            Point::new(240.0, 144.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let before = cx.floor().wall(a).unwrap().clone();
        // The dialog turned wall a about its start: its end is now at (0, 240).
        cx.project.floors[0].wall_mut(a).unwrap().end = Point::new(0.0, 240.0);
        follow_moved_ends(&mut cx, a, &before);
        assert_eq!(cx.floor().wall(a).unwrap().end, Point::new(0.0, 240.0));
        let wb = cx.floor().wall(b).unwrap();
        assert_eq!(wb.start, Point::new(0.0, 240.0), "the joined end follows");
        assert_eq!(wb.end, Point::new(240.0, 144.0), "the far end stays");
    }
}
