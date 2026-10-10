//! Wall Edit toolbar commands (W-23, W-43, W-44, W-66..W-68, S-41, S-42,
//! S-55): Break Wall, Remove Break, Reverse Layers, Change Line/Arc and
//! Make Arc Tangent. The buttons come from `EditorContext::extra_edit_actions`
//! and run through `run_custom`; Break Wall waits for a click on the wall.

use super::selection::{hit_test_cx, ObjectRef};
use super::EditorContext;
use plan_core::geometry::Point;
use plan_core::{Id, WallEnd};
use std::cell::Cell;

pub mod icons;
pub use icons::{draw as draw_icons, draw_layer_handles, select_for_icon};

/// Custom command ids (the `id` of `EditActionKind::Custom`).
pub const BREAK_WALL: &str = "wall.break";
pub const REMOVE_BREAK: &str = "wall.remove_break";
pub const REVERSE_LAYERS: &str = "wall.reverse_layers";
pub const CHANGE_LINE_ARC: &str = "wall.line_arc";
pub const ARC_TANGENT: &str = "wall.arc_tangent";
pub const CONVERT_POLYLINE: &str = "wall.to_polyline";
pub const MAKE_INVISIBLE: &str = "wall.make_invisible";
pub const MAKE_VISIBLE: &str = "wall.make_visible";
pub const FIX_OFF_ANGLE: &str = "wall.fix_off_angle";
pub const IGNORE_ICON: &str = "wall.ignore_icon";
pub const IGNORE_ALL: &str = "wall.ignore_all_icons";
pub const RESET_ICONS: &str = "wall.reset_icons";
pub const CONNECT_WALLS: &str = "wall.connect_walls";
pub const LOCK_START: &str = "wall.lock_start";
pub const LOCK_END: &str = "wall.lock_end";
pub const ALIGN_ABOVE: &str = "wall.align_above";
pub const ALIGN_BELOW: &str = "wall.align_below";
pub const RESET_LAYER_JOINS: &str = "wall.reset_layer_joins";
pub const RESET_VALUES: &str = "wall.reset_values";

thread_local! {
    /// Break Wall was picked and waits for the click that sets the point.
    static BREAK_PENDING: Cell<bool> = const { Cell::new(false) };
    /// Connect Walls has its first wall and waits for the click on the second
    /// (W-136). It shares the click and Esc path of Break Wall.
    static CONNECT_PENDING: Cell<Option<Id>> = const { Cell::new(None) };
}

/// Is Break Wall waiting for its click?
pub fn break_pending() -> bool {
    BREAK_PENDING.with(Cell::get) || CONNECT_PENDING.with(Cell::get).is_some()
}

/// Drops a pending Break Wall; true when there was one (Esc).
pub fn cancel_break() -> bool {
    let connect = CONNECT_PENDING.with(|c| c.replace(None)).is_some();
    BREAK_PENDING.with(|b| b.replace(false)) || connect
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
    let all_hidden = walls
        .iter()
        .all(|i| cx.floor().wall(*i).is_some_and(|w| w.flags.invisible));
    if all_hidden {
        v.push(button(MAKE_VISIBLE, "Make Wall(s) Visible", "", true));
    } else {
        v.push(button(MAKE_INVISIBLE, "Make Wall(s) Invisible", "", true));
    }
    v.push(button(ALIGN_ABOVE, "Align With Wall Above", "", true));
    v.push(button(ALIGN_BELOW, "Align With Wall Below", "", true));
    v.push(button(RESET_ICONS, "Reset Notification Icons", "", true));
    v.push(button(RESET_VALUES, "Reset Walls to Defaults", "", true));
    v.push(button(
        RESET_LAYER_JOINS,
        "Reset Wall Layer Intersections",
        "",
        true,
    ));
    if walls.len() == 2 {
        v.push(button(CONNECT_WALLS, "Connect Walls", "", true));
    }
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
        let (allowed, inc) = angle_rules(cx);
        let icons = plan_core::wall_repair::off_angle(w, &allowed, inc).is_some()
            || !cx.project.unconnected_ends(cx.floor, w.id).is_empty();
        v.push(button(CONNECT_WALLS, "Connect Walls", "", single));
        v.push(button(
            FIX_OFF_ANGLE,
            "Fix Off Angle Wall",
            "",
            single && plan_core::wall_repair::off_angle(w, &allowed, inc).is_some(),
        ));
        let only_loose = plan_core::wall_repair::off_angle(w, &allowed, inc).is_none();
        v.push(button(
            IGNORE_ICON,
            if only_loose {
                "Ignore Unconnected Wall"
            } else {
                "Ignore"
            },
            "",
            single && icons,
        ));
        v.push(button(IGNORE_ALL, "Ignore All", "", icons));
        let (ls, le) = (w.flags.lock_start, w.flags.lock_end);
        v.push(button(
            LOCK_START,
            if ls {
                "Enable Auto Connect (Start)"
            } else {
                "Lock Auto Connect (Start)"
            },
            "",
            single,
        ));
        v.push(button(
            LOCK_END,
            if le {
                "Enable Auto Connect (End)"
            } else {
                "Lock Auto Connect (End)"
            },
            "",
            single,
        ));
    }
    v
}

/// Where a dragged wall end lands (W-15..W-18): the snapped point, held to
/// 90 or 45 degrees by Shift, or replaced by the typed length and angle.
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
        let set = super::snap::restrictive_angles(&cx.defaults.editing);
        let held = super::snap::angle_snap_list(fixed, raw, cx.defaults.grid.snap, &set);
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
        MAKE_INVISIBLE => {
            set_invisible(cx, true);
        }
        MAKE_VISIBLE => {
            set_invisible(cx, false);
        }
        FIX_OFF_ANGLE => {
            if let Some(ObjectRef::Wall(id)) = cx.selection.single() {
                crate::dialogs::fix_connections::open(cx, id);
            }
        }
        IGNORE_ICON => {
            ignore_icons(cx, false);
        }
        IGNORE_ALL => {
            ignore_icons(cx, true);
        }
        RESET_ICONS => {
            reset_icons(cx);
        }
        RESET_LAYER_JOINS => {
            reset_layer_joins(cx);
        }
        RESET_VALUES => {
            reset_wall_values(cx);
        }
        CONNECT_WALLS => {
            connect_selected(cx);
        }
        LOCK_START => {
            toggle_auto_connect_lock(cx, WallEnd::Start);
        }
        LOCK_END => {
            toggle_auto_connect_lock(cx, WallEnd::End);
        }
        ALIGN_ABOVE => {
            align_with(cx, 1);
        }
        ALIGN_BELOW => {
            align_with(cx, -1);
        }
        _ => return false,
    }
    true
}

/// The allowed angles and the increment of the plan defaults.
fn angle_rules(cx: &EditorContext) -> (Vec<f64>, f64) {
    (
        cx.defaults.editing.snap_angles.clone(),
        cx.defaults.editing.angle_snap_deg,
    )
}

/// The angle wall `id` should have when it carries the off-angle icon.
pub fn off_angle_target(cx: &EditorContext, id: Id) -> Option<f64> {
    let (allowed, inc) = angle_rules(cx);
    let w = cx.floor().wall(id)?;
    if w.flags.ignore_off_angle {
        return None;
    }
    plan_core::wall_repair::off_angle(w, &allowed, inc)
}

/// Make Wall(s) Invisible / Visible (W-130): sets the Invisible flag of every
/// selected wall. One undo step; returns how many walls changed.
pub fn set_invisible(cx: &mut EditorContext, invisible: bool) -> usize {
    let ids = selected_walls(cx);
    if ids.is_empty() || ids.iter().any(|i| !cx.check_unlocked(ObjectRef::Wall(*i))) {
        return 0;
    }
    cx.begin_change(if invisible {
        "Make Walls Invisible"
    } else {
        "Make Walls Visible"
    });
    let fl = cx.floor;
    let mut n = 0;
    for id in ids {
        if let Some(w) = cx.project.floors[fl].wall_mut(id) {
            if w.flags.invisible != invisible {
                w.flags.invisible = invisible;
                n += 1;
            }
        }
    }
    if n == 0 {
        cx.cancel_change();
        return 0;
    }
    cx.project.sync_platform_walls();
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!(
        "{n} wall{} made {}",
        if n == 1 { "" } else { "s" },
        if invisible { "invisible" } else { "visible" }
    );
    n
}

/// Fix Off Angle Wall (W-133): turns wall `id` to `angle` degrees around the
/// start, center or end; walls joined to the moved ends follow. One undo step.
pub fn fix_off_angle(
    cx: &mut EditorContext,
    id: Id,
    angle: f64,
    lock: plan_core::wall_repair::FixLock,
) -> bool {
    if !cx.check_unlocked(ObjectRef::Wall(id)) {
        return false;
    }
    let fl = cx.floor;
    let Some(before) = cx.project.floors[fl].wall(id).cloned() else {
        return false;
    };
    cx.begin_change("Fix Off Angle Wall");
    if cx.project.fix_off_angle(fl, id, angle, lock).is_none() {
        cx.cancel_change();
        return false;
    }
    follow_moved_ends(cx, id, &before);
    cx.project.sync_platform_walls();
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!("Wall turned to {angle:.2} degrees");
    true
}

/// Ignore (the selected walls) or Ignore All (every wall of the floor): the
/// off-angle and unconnected icons go away until Reset Notification Icons.
pub fn ignore_icons(cx: &mut EditorContext, all: bool) -> usize {
    let ids = selected_walls(cx);
    let fl = cx.floor;
    cx.begin_change(if all { "Ignore All" } else { "Ignore" });
    let n = cx
        .project
        .ignore_wall_icons(fl, (!all).then_some(&ids[..]), true, true);
    if n == 0 {
        cx.cancel_change();
        return 0;
    }
    cx.mark_dirty();
    cx.status = format!(
        "Ignoring the icons of {n} wall{}",
        if n == 1 { "" } else { "s" }
    );
    n
}

/// Reset Notification Icons (W-132): every ignored icon of the floor returns.
pub fn reset_icons(cx: &mut EditorContext) -> usize {
    let fl = cx.floor;
    cx.begin_change("Reset Notification Icons");
    let n = cx.project.reset_notification_icons(fl);
    if n == 0 {
        cx.cancel_change();
        cx.status = "No notification icon was ignored".into();
        return 0;
    }
    cx.mark_dirty();
    cx.status = format!(
        "Notification icons of {n} wall{} are back",
        if n == 1 { "" } else { "s" }
    );
    n
}

/// The wall types in force (the plan's own, else the defaults').
fn wall_types(cx: &EditorContext) -> Vec<plan_core::defaults::WallTypeDef> {
    if cx.project.wall_types.is_empty() {
        cx.defaults.wall_types.clone()
    } else {
        cx.project.wall_types.clone()
    }
}

/// The slide a layer handle dragged to `want` inches settles at: it snaps to
/// the nearest layer line of the wall it meets (W-144, DECISIONS WR5).
pub fn snap_layer_slide(cx: &EditorContext, id: Id, end: WallEnd, layer: usize, want: f64) -> f64 {
    let types = wall_types(cx);
    let cands = plan_core::walls::intersect::layer_snap_candidates(
        &cx.floor().walls,
        &types,
        id,
        end,
        layer,
        0.5,
    );
    plan_core::walls::intersect::snap_slide(&cands, want, 3.0)
}

/// The unit direction pointing out of the wall at `end`.
fn outward(w: &plan_core::Wall, end: WallEnd) -> Point {
    match end {
        WallEnd::Start => -w.direction(),
        WallEnd::End => w.direction(),
    }
}

/// How far past the wall end a pointer at `at` is, along the wall: the
/// `want` of [`slide_layer`] for a handle dragged there.
pub fn layer_drag_amount(cx: &EditorContext, id: Id, end: WallEnd, at: Point) -> Option<f64> {
    let w = cx.floor().wall(id)?;
    let base = if end == WallEnd::Start {
        w.start
    } else {
        w.end
    };
    Some((at - base).dot(outward(w, end)))
}

/// The Edit Wall Intersections handle under `at` on a selected straight
/// multi-layer wall (W-144): the wall, its end and the layer. A press nearer
/// to the wall's own end handle is left to the stretch drag.
pub fn layer_handle_at(cx: &EditorContext, at: Point, tol: f64) -> Option<(Id, WallEnd, usize)> {
    let types = wall_types(cx);
    for id in selected_walls(cx) {
        let Some(w) = cx.floor().wall(id) else {
            continue;
        };
        if w.is_curved() {
            continue;
        }
        let ty = w
            .wall_type
            .as_deref()
            .and_then(|n| types.iter().find(|t| t.name == n));
        let layers = plan_core::joins::wall_layer_bands(w, ty).len();
        if layers < 2 {
            continue;
        }
        for end in [WallEnd::Start, WallEnd::End] {
            let tip = if end == WallEnd::Start {
                w.start
            } else {
                w.end
            };
            for k in 0..layers {
                let Some(h) = plan_core::walls::intersect::layer_handle(w, &types, end, k) else {
                    continue;
                };
                if h.dist(at) <= tol && h.dist(at) < tip.dist(at) {
                    return Some((id, end, k));
                }
            }
        }
    }
    None
}

/// Edit Wall Intersections (W-144): drags the handle of structural layer
/// `layer` at `end` of wall `id` to `want` inches past its joined position;
/// the slide snaps to the layer lines of the wall it meets (the magnet
/// positions). One undo step. Returns the slide that was stored.
pub fn slide_layer(
    cx: &mut EditorContext,
    id: Id,
    end: WallEnd,
    layer: usize,
    want: f64,
) -> Option<f64> {
    let fl = cx.floor;
    let shift = snap_layer_slide(cx, id, end, layer, want);
    cx.begin_change("Edit Wall Intersections");
    if !cx.project.set_layer_join(fl, id, end, layer, shift) {
        cx.cancel_change();
        return None;
    }
    cx.mark_dirty();
    cx.status = format!("Layer slid {shift:.2} in");
    Some(shift)
}

/// Reset Wall Layer Intersections (W-144): the selected walls (every wall of
/// the floor when none is selected) get their layers back where the join
/// rules put them. One undo step; nothing slid, no step.
pub fn reset_layer_joins(cx: &mut EditorContext) -> usize {
    let ids = selected_walls(cx);
    let fl = cx.floor;
    cx.begin_change("Reset Wall Layer Intersections");
    let n = cx
        .project
        .reset_layer_joins(fl, (!ids.is_empty()).then_some(&ids[..]), false);
    if n == 0 {
        cx.cancel_change();
        cx.status = "No wall layer was slid".into();
        return 0;
    }
    cx.mark_dirty();
    cx.status = format!(
        "Layer intersections of {n} wall{} reset",
        if n == 1 { "" } else { "s" }
    );
    n
}

/// Auto Connect lock on one end of the selected wall (W-135): a locked end
/// never snaps to other walls and carries no unconnected symbol.
pub fn toggle_auto_connect_lock(cx: &mut EditorContext, end: WallEnd) -> Option<bool> {
    let Some(ObjectRef::Wall(id)) = cx.selection.single() else {
        cx.status = "Select one wall to lock or unlock Auto Connect".into();
        return None;
    };
    if !cx.check_unlocked(ObjectRef::Wall(id)) {
        return None;
    }
    cx.begin_change("Auto Connect Lock");
    let fl = cx.floor;
    let w = cx.project.floors[fl].wall_mut(id)?;
    let flag = match end {
        WallEnd::Start => &mut w.flags.lock_start,
        WallEnd::End => &mut w.flags.lock_end,
    };
    *flag = !*flag;
    let now = *flag;
    cx.mark_dirty();
    cx.status = if now {
        "Auto Connect is locked on that end".into()
    } else {
        "Auto Connect is on for that end".into()
    };
    Some(now)
}

/// Connect Walls (W-136) with two walls selected connects them now; with one
/// selected it waits for the click on the second wall.
pub fn connect_selected(cx: &mut EditorContext) {
    match selected_walls(cx)[..] {
        [a, b] => {
            connect_walls(cx, a, b);
        }
        [a] => {
            CONNECT_PENDING.with(|c| c.set(Some(a)));
            cx.status = "Connect Walls: click the wall to join (Esc cancels)".into();
        }
        _ => cx.status = "Select a wall (or two) to connect".into(),
    }
}

/// Joins the close ends of walls `a` and `b`. One undo step; returns the
/// number of edits (0 when they are too far apart or an end is locked).
pub fn connect_walls(cx: &mut EditorContext, a: Id, b: Id) -> usize {
    if a == b || !cx.check_unlocked(ObjectRef::Wall(a)) || !cx.check_unlocked(ObjectRef::Wall(b)) {
        return 0;
    }
    cx.begin_change("Connect Walls");
    let opts = super::connect::ConnectOptions::from_defaults(&cx.defaults);
    let rooms_before = room_count(cx);
    let n = super::connect::connect_walls_project(&mut cx.project, cx.floor, a, b, &opts);
    if n == 0 {
        cx.cancel_change();
        cx.status =
            "Connect Walls: those walls are already connected, too far apart or locked".into();
        return 0;
    }
    merge_collinear_at(cx, a);
    merge_collinear_at(cx, b);
    auto_reverse_if_closed(cx, a, rooms_before);
    auto_reverse_if_closed(cx, b, rooms_before);
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!("Connect Walls: {n} edit{}", if n == 1 { "" } else { "s" });
    n
}

/// How many rooms the walls of the active floor enclose right now.
pub fn room_count(cx: &EditorContext) -> usize {
    plan_core::rooms::detect_rooms(&cx.floor().walls, 0.5).len()
}

/// Auto Merge Collinear Walls (W-120): when the switch is on, wall `id` is
/// merged with the one wall that meets each of its ends in a straight line
/// and has the same specification (`Project::join_collinear_walls`). Runs
/// inside the caller's undo step; returns how many merges were made.
pub fn merge_collinear_at(cx: &mut EditorContext, id: Id) -> usize {
    if !cx.defaults.walls_connect.auto_merge_collinear {
        return 0;
    }
    let fl = cx.floor;
    let mut n = 0;
    for end in [WallEnd::Start, WallEnd::End] {
        let Some(w) = cx.project.floors[fl].wall(id) else {
            break;
        };
        let p = if end == WallEnd::Start {
            w.start
        } else {
            w.end
        };
        let others = super::ops::walls_at(&cx.project, fl, p, 0.5, Some(id));
        if let [(other, _)] = others[..] {
            if cx.project.join_collinear_walls(fl, id, other).is_some() {
                n += 1;
            }
        }
    }
    n
}

/// Auto Reverse Wall Layers (W-131): when the edit just closed a room
/// (`rooms_before` was the count before it) and the switch is on, the
/// exterior walls of the new room turn their exterior layers outward. Runs
/// inside the caller's undo step; returns how many walls turned.
pub fn auto_reverse_if_closed(cx: &mut EditorContext, id: Id, rooms_before: usize) -> usize {
    if !cx.defaults.walls_connect.auto_reverse_layers || room_count(cx) <= rooms_before {
        return 0;
    }
    let fl = cx.floor;
    cx.project.auto_reverse_enclosed(fl, id)
}

/// Reset to Defaults for the selected walls, or every wall of the floor when
/// none is selected (W-121): wall type, thickness and height follow the
/// defaults again. One undo step; returns how many walls changed.
pub fn reset_wall_values(cx: &mut EditorContext) -> usize {
    let ids = selected_walls(cx);
    if ids.iter().any(|i| !cx.check_unlocked(ObjectRef::Wall(*i))) {
        return 0;
    }
    cx.begin_change("Reset Walls to Defaults");
    let (d, fl) = (cx.defaults.clone(), cx.floor);
    let n = cx.project.reset_walls_to_defaults(&d, fl, &ids);
    if n == 0 {
        cx.cancel_change();
        cx.status = "Reset to Defaults: the walls already match the defaults".into();
        return 0;
    }
    cx.project.sync_platform_walls();
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!(
        "Reset {n} wall{} to the defaults",
        if n == 1 { "" } else { "s" }
    );
    n
}

/// The click that names the second wall of Connect Walls. Stays pending when
/// the click misses every wall; ends the mode otherwise.
fn connect_click(cx: &mut EditorContext, first: Id, world: Point) -> bool {
    let tol = cx.pick_tol();
    let hit = hit_test_cx(cx, world, tol)
        .into_iter()
        .find_map(|h| match h {
            ObjectRef::Wall(id) if id != first => Some(id),
            _ => None,
        });
    let Some(second) = hit else {
        cx.status = "Connect Walls: click another wall (Esc cancels)".into();
        return false;
    };
    CONNECT_PENDING.with(|c| c.set(None));
    connect_walls(cx, first, second) > 0
}

/// Align With Wall Above (`dir` 1) / Below (-1) (W-145): each selected
/// straight wall slides sideways until its main-layer outer edge lines up
/// with that of the overlapping wall one floor over; joined walls follow.
/// One undo step; returns how many walls moved.
pub fn align_with(cx: &mut EditorContext, dir: isize) -> usize {
    let ids = selected_walls(cx);
    if ids.is_empty() || ids.iter().any(|i| !cx.check_unlocked(ObjectRef::Wall(*i))) {
        return 0;
    }
    let fl = cx.floor;
    let label = if dir > 0 {
        "Align With Wall Above"
    } else {
        "Align With Wall Below"
    };
    let moves: Vec<(Id, Point)> = ids
        .iter()
        .filter_map(|id| {
            cx.project
                .align_candidate(fl, *id, dir, cx.wall_types())
                .map(|(_, shift)| (*id, shift))
        })
        .collect();
    if moves.is_empty() {
        cx.status = format!("{label}: no overlapping wall to line up with");
        return 0;
    }
    cx.begin_change(label);
    for (id, shift) in &moves {
        super::ops::translate_walls_with_followers(&mut cx.project, fl, &[*id], *shift);
    }
    cx.project.sync_platform_walls();
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!(
        "{label}: {} wall{} moved",
        moves.len(),
        if moves.len() == 1 { "" } else { "s" }
    );
    moves.len()
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
    if let Some(first) = CONNECT_PENDING.with(Cell::get) {
        return connect_click(cx, first, world);
    }
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
        fn shift_holds_the_angle_and_ctrl_frees_the_stretched_end() {
            let (mut cx, id) = cx_with_wall();
            cx.defaults.editing.angle_snaps = false;
            cx.defaults.editing.restrictive_angle_deg = 45.0;
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
            // Ctrl: no snap at all, so the end sits exactly at the pointer.
            let alt = Modifiers {
                ctrl: true,
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
