//! Build Framing for Selected Object(s) and for Parent Object(s), Retain
//! Framing, Framing Groups and the framing edit buttons (manual pp. 914 to
//! 927).
//!
//! A build for selected objects makes the same members a full build makes
//! (`frame_floor_all_with`), then replaces only the ones that belong to the
//! objects: a wall's members by wall, a room's or platform's joists by
//! position, a roof plane's framing by the plane's outline, a tray ceiling's
//! members by their tray tag, a truss by itself. The step is one undo step
//! and is refused for an object that retains its framing.

use super::{
    details, find, frame_floor_all_with, group_of, group_of_manual, load, load_records,
    member_center, platform_rooms, reference_marker, roof_of, select, selected, settings,
    store_records, EditorContext, FramingSettings, ObjectRef, Record,
};
use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::{Floor, Id, Project, Room, RoomName};
use plan_framing::{
    BearingMode, BuildOptions, FramingMember, Group, GroupFlags, ManualMemberKind, Member,
    MemberKind, TRAY_MEMBER_FLAG,
};
use serde_json::Value;

/// The objects a build for selected objects can rebuild.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Wall(Id),
    /// The room (or the platform it is in) by index among the floor's rooms.
    Room(usize),
    RoofPlane(Id),
    Tray(Id),
    /// A truss (a manual or laid-out member): its envelope and webbing are
    /// made again.
    Truss(Id),
}

/// What a build for selected objects did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Rebuilt; this many members were made.
    Built(usize),
    /// A room next to rooms of its own Framing Group: Chief asks whether to
    /// start a new group for it (Yes: `new_group` true).
    AskGroup { room: usize },
    /// Nothing to do, and why in words.
    Refused(String),
}

fn near_edge(poly: &[Point], p: Point, tol: f64) -> bool {
    let n = poly.len();
    (0..n).any(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]) <= tol)
}

fn within(poly: &[Point], p: Point) -> bool {
    point_in_polygon(p, poly) || near_edge(poly, p, 2.0)
}

/// Options that make only `groups`, with nothing retained: the build for the
/// selected objects asks for one kind of framing.
fn only(st: &FramingSettings, groups: &[Group]) -> BuildOptions {
    let mut o = st.build.clone();
    o.build = GroupFlags::NONE;
    for g in groups {
        o.build.set(*g, true);
    }
    o.retain = GroupFlags::NONE;
    o.retain_walls.clear();
    o.floor_pick = plan_framing::FloorPick::All;
    o.ceiling_pick = plan_framing::FloorPick::All;
    o
}

/// Replaces the automatic members and the built records of `fi` in one go:
/// the records that are not `Built` stay.
fn store_all(floor: &mut Floor, members: &[Member], built: Vec<FramingMember>) {
    let mut records: Vec<Record> = load_records(floor)
        .into_iter()
        .filter(|r| !matches!(r, Record::Built(_)))
        .collect();
    records.extend(built.into_iter().map(Record::Built));
    let mut values: Vec<Value> = members
        .iter()
        .filter_map(|m| serde_json::to_value(m).ok())
        .collect();
    values.extend(records.iter().filter_map(|r| serde_json::to_value(r).ok()));
    values.extend(
        floor
            .framing
            .iter()
            .filter(|v| super::is_settings(v))
            .cloned(),
    );
    values.extend(
        floor
            .framing
            .iter()
            .filter(|v| plan_framing::catalog::is_catalog(v))
            .cloned(),
    );
    // The detail maps of a detail floor are never here; nothing else is kept.
    floor.framing = values;
}

/// Rebuilds the members of floor `fi` that `replace_old` picks, from a full
/// build made with `opts`, keeping the new members and built records that
/// `keep_new` / `keep_built` pick. Returns how many new members were made.
#[allow(clippy::too_many_arguments)]
fn rebuild_region(
    project: &mut Project,
    fi: usize,
    st: &FramingSettings,
    opts: &BuildOptions,
    replace_old: &dyn Fn(&Member) -> bool,
    replace_built: &dyn Fn(&FramingMember) -> bool,
    keep_new: &dyn Fn(&Member) -> bool,
    keep_built: &dyn Fn(&FramingMember) -> bool,
) -> usize {
    let fb = frame_floor_all_with(project, fi, true, st, opts);
    let fresh: Vec<Member> = fb.members.into_iter().filter(|m| keep_new(m)).collect();
    let mut built: Vec<FramingMember> = fb.built.into_iter().filter(|m| keep_built(m)).collect();
    for m in &mut built {
        m.id = project.alloc_id();
        m.layer_name = super::layer_for(m.kind).to_string();
    }
    let n = fresh.len() + built.len();
    let floor = &project.floors[fi];
    let mut members: Vec<Member> = load(floor)
        .into_iter()
        .filter(|m| !replace_old(m))
        .collect();
    members.extend(fresh);
    let mut all_built: Vec<FramingMember> = load_records(floor)
        .into_iter()
        .filter_map(|r| match r {
            Record::Built(m) if !replace_built(&m) => Some(m),
            _ => None,
        })
        .collect();
    all_built.extend(built);
    store_all(&mut project.floors[fi], &members, all_built);
    super::restamp(project, fi);
    n
}

// ----- the targets -----

/// Whether the wall's framing is retained.
fn wall_is_retained(st: &FramingSettings, wall: Id) -> bool {
    st.build.retain_walls.contains(&wall)
}

fn build_wall(project: &mut Project, fi: usize, wall: Id, st: &FramingSettings) -> Outcome {
    if wall_is_retained(st, wall) {
        return Outcome::Refused(
            "The wall retains its framing: turn Retain Wall Framing off first".into(),
        );
    }
    let Some(w) = project.floors[fi]
        .walls
        .iter()
        .find(|w| w.id == wall)
        .cloned()
    else {
        return Outcome::Refused("That wall is gone".into());
    };
    if w.flags.invisible || w.flags.room_divider || w.flags.railing {
        return Outcome::Refused("That wall has no framing".into());
    }
    let opts = only(st, &[Group::Wall]);
    let near = |p: Point| dist_to_segment(p, w.start, w.end) <= w.thickness + 1.0;
    let n = rebuild_region(
        project,
        fi,
        st,
        &opts,
        &|m| m.wall_id == Some(wall),
        // The posts under a beam that cross this wall.
        &|m| m.kind == ManualMemberKind::Post && near(m.start),
        &|m| m.wall_id == Some(wall),
        &|m| m.kind == ManualMemberKind::Post && near(m.start),
    );
    Outcome::Built(n)
}

/// The platform of floor `fi` that holds room `room`.
fn platform_of(project: &Project, fi: usize, st: &FramingSettings, anchor: Point) -> Option<Room> {
    platform_rooms(&project.floors[fi], &st.walls)
        .into_iter()
        .find(|r| r.contains(anchor))
}

/// Whether Chief would ask about a new Framing Group for the room at
/// `anchor`: platforms are shared (the exterior-walls mode) and another room
/// of the same group stands beside it.
fn needs_group_question(project: &Project, fi: usize, st: &FramingSettings, anchor: Point) -> bool {
    if st.walls.bearing != BearingMode::ExteriorAndBearingLines {
        return false;
    }
    let floor = &project.floors[fi];
    let rooms = plan_core::detect_rooms(&floor.walls, 0.5);
    let group = |r: &Room| {
        r.name_entry(&floor.room_names)
            .map_or(0, |n| n.options.framing_group)
    };
    let Some(mine) = rooms.iter().find(|r| r.contains(anchor)) else {
        return false;
    };
    let g = group(mine);
    // Another room of the group inside the same platform.
    platform_of(project, fi, st, anchor).is_some_and(|p| {
        rooms
            .iter()
            .filter(|r| !std::ptr::eq(*r, mine))
            .any(|r| group(r) == g && p.contains(crate::editor::rooms_edit::room_anchor(r)))
    })
}

fn next_group(floor: &Floor) -> u32 {
    floor
        .room_names
        .iter()
        .map(|n| n.options.framing_group)
        .max()
        .unwrap_or(0)
        + 1
}

/// Gives the room at `anchor` a Framing Group of its own, making its name
/// entry when it has none. Returns the group.
fn assign_new_group(project: &mut Project, fi: usize, anchor: Point) -> u32 {
    let rooms = plan_core::detect_rooms(&project.floors[fi].walls, 0.5);
    let group = next_group(&project.floors[fi]);
    let floor = &mut project.floors[fi];
    let at = rooms
        .iter()
        .find(|r| r.contains(anchor))
        .and_then(|r| floor.room_names.iter().position(|n| r.contains(n.anchor)));
    match at {
        Some(i) => floor.room_names[i].options.framing_group = group,
        None => {
            let mut n = RoomName {
                anchor,
                ..RoomName::default()
            };
            n.options.framing_group = group;
            floor.room_names.push(n);
        }
    }
    group
}

fn build_room(
    project: &mut Project,
    fi: usize,
    anchor: Point,
    st: &FramingSettings,
    new_group: Option<bool>,
) -> Outcome {
    let floor = &project.floors[fi];
    let rooms = plan_core::detect_rooms(&floor.walls, 0.5);
    let Some(room) = rooms.iter().find(|r| r.contains(anchor)).cloned() else {
        return Outcome::Refused("That room is gone".into());
    };
    if room
        .name_entry(&floor.room_names)
        .is_some_and(|n| n.options.retain_framing)
    {
        return Outcome::Refused(
            "The room retains its framing: turn Retain Floor/Ceiling Framing off first".into(),
        );
    }
    if needs_group_question(project, fi, st, anchor) {
        match new_group {
            None => return Outcome::AskGroup { room: 0 },
            Some(true) => {
                assign_new_group(project, fi, anchor);
            }
            Some(false) => {}
        }
    }
    let Some(platform) = platform_of(project, fi, st, anchor) else {
        return Outcome::Refused("The room has no floor platform".into());
    };
    let poly = platform.polygon.clone();
    let opts = only(st, &[Group::Floor, Group::Ceiling]);
    let is_platform_kind = |k: MemberKind| {
        matches!(
            k,
            MemberKind::Joist
                | MemberKind::RimJoist
                | MemberKind::TrimmerJoist
                | MemberKind::HeaderJoist
                | MemberKind::CeilingJoist
        )
    };
    let in_old = |m: &Member| {
        let g = group_of(m);
        (g == Group::Floor || g == Group::Ceiling)
            && m.wall_id.is_none_or(|w| w & TRAY_MEMBER_FLAG != 0)
            && (is_platform_kind(m.kind) || m.kind == MemberKind::Blocking)
            && within(&poly, member_center(m))
    };
    let in_built = |m: &FramingMember| {
        matches!(group_of_manual(m.kind), Group::Floor | Group::Ceiling)
            && matches!(
                m.kind,
                ManualMemberKind::Joist | ManualMemberKind::FloorCeilingBeam
            )
            && within(&poly, Point::lerp(m.start, m.end, 0.5))
    };
    let n = rebuild_region(
        project, fi, st, &opts, &in_old, &in_built, &in_old, &in_built,
    );
    Outcome::Built(n)
}

fn build_plane(project: &mut Project, fi: usize, plane: Id, st: &FramingSettings) -> Outcome {
    if st.build.retain_planes.contains(&plane) {
        return Outcome::Refused("The roof plane retains its framing".into());
    }
    let set = crate::editor::roof_view::load(&project.floors[fi]);
    let Some(p) = set.planes.iter().find(|p| p.id == plane) else {
        return Outcome::Refused("That is not a roof plane".into());
    };
    let poly: Vec<Point> = p
        .polygon3d
        .iter()
        .map(|v| Point::new(v[0], -v[2]))
        .collect();
    let opts = only(st, &[Group::Roof]);
    let roof_old = |m: &Member| group_of(m) == Group::Roof && within(&poly, member_center(m));
    let roof_built = |m: &FramingMember| {
        group_of_manual(m.kind) == Group::Roof
            && m.kind == ManualMemberKind::RoofTruss
            && within(&poly, Point::lerp(m.start, m.end, 0.5))
    };
    if roof_of(&project.floors[fi]).is_none() {
        return Outcome::Refused("The floor has no roof".into());
    }
    let n = rebuild_region(
        project,
        fi,
        st,
        &opts,
        &roof_old,
        &roof_built,
        &roof_old,
        &roof_built,
    );
    Outcome::Built(n)
}

fn build_tray(project: &mut Project, fi: usize, tray: Id, st: &FramingSettings) -> Outcome {
    let floor = &project.floors[fi];
    let Some(rec) = floor.trays.get(tray) else {
        return Outcome::Refused("That is not a tray ceiling".into());
    };
    if rec.retain_framing {
        return Outcome::Refused("The tray ceiling retains its framing".into());
    }
    let geom = plan_3d::tray::floor_trays(floor)
        .into_iter()
        .find(|g| g.id == tray);
    let Some(g) = geom.filter(plan_core::tray::TrayGeom::ok) else {
        return Outcome::Refused(
            "The tray ceiling has a caution: its shape or position is not supported".into(),
        );
    };
    let opts = only(st, &[Group::Ceiling]);
    let tag = TRAY_MEMBER_FLAG | tray;
    let _ = g;
    let n = rebuild_region(
        project,
        fi,
        st,
        &opts,
        &|m| m.wall_id == Some(tag),
        &|_| false,
        &|m| m.wall_id == Some(tag),
        &|_| false,
    );
    Outcome::Built(n)
}

/// Force Truss Rebuild for the truss `id` of floor `fi`: its envelope and
/// webbing are made again for where it stands. A locked truss is left alone.
fn build_truss(project: &mut Project, fi: usize, id: Id) -> Outcome {
    let mut records = load_records(&project.floors[fi]);
    let roof = roof_of(&project.floors[fi]);
    let Some(r) = records.iter_mut().find(|r| r.id() == id) else {
        return Outcome::Refused("That is not a truss".into());
    };
    let (Record::Manual(m) | Record::Built(m)) = r else {
        return Outcome::Refused("That is not a truss".into());
    };
    if !m.kind.is_truss() {
        return Outcome::Refused("That is not a truss".into());
    }
    if m.truss.as_ref().is_some_and(|t| t.locked) {
        return Outcome::Refused("The truss envelope and webbing are locked".into());
    }
    super::trusses::conform(m, roof.as_ref());
    store_records(&mut project.floors[fi], &records);
    Outcome::Built(1)
}

// ----- the commands -----

/// What is selected that a build can rebuild, on the active floor.
pub fn selected_targets(cx: &EditorContext) -> Vec<Target> {
    let mut out: Vec<Target> = Vec::new();
    for o in &cx.selection.items {
        match o {
            ObjectRef::Wall(id) => out.push(Target::Wall(*id)),
            ObjectRef::Room(i) => out.push(Target::Room(*i)),
            ObjectRef::RoofPlane(id)
                if crate::editor::roof_view::load(cx.floor())
                    .planes
                    .iter()
                    .any(|p| p.id == *id) =>
            {
                out.push(Target::RoofPlane(*id))
            }
            ObjectRef::Framing(id)
                if find(cx.floor(), *id)
                    .and_then(|r| r.member().map(|m| m.kind.is_truss()))
                    .unwrap_or(false) =>
            {
                out.push(Target::Truss(*id));
            }
            _ => {}
        }
    }
    if let Some(r) = crate::editor::rooms_edit::selected_room(cx) {
        if !out.contains(&Target::Room(r)) {
            out.push(Target::Room(r));
        }
    }
    if let Some(t) = crate::tools::tray_ceiling::selected_tray(cx) {
        out.push(Target::Tray(t));
    }
    out
}

/// The Build Framing for Parent Object(s) targets of the selected framing: the
/// wall of a Wall Detail's member, the platform a joist belongs to, the roof
/// plane a rafter stands in, the tray ceiling, or the truss itself.
pub fn parent_targets(cx: &EditorContext) -> Vec<Target> {
    let mut out: Vec<Target> = Vec::new();
    // A Wall Detail: its wall, whichever member is selected.
    if let Some(wall) = details::detail_wall(cx.floor()) {
        if !details::selected_wall_members(cx).is_empty() {
            out.push(Target::Wall(wall));
        }
        return out;
    }
    let floor = cx.floor();
    for id in selected(cx) {
        let Some(r) = find(floor, id) else { continue };
        let Some(m) = r.member() else { continue };
        let mid = Point::lerp(m.start, m.end, 0.5);
        if m.kind.is_truss() {
            out.push(Target::Truss(id));
        } else if let Some(i) = cx.rooms.iter().position(|r| r.contains(mid)) {
            if group_of_manual(m.kind) != Group::Roof {
                out.push(Target::Room(i));
            }
        }
        if group_of_manual(m.kind) == Group::Roof && !m.kind.is_truss() {
            if let Some(p) = crate::editor::roof_view::load(floor)
                .planes
                .iter()
                .find(|p| {
                    let poly: Vec<Point> = p
                        .polygon3d
                        .iter()
                        .map(|v| Point::new(v[0], -v[2]))
                        .collect();
                    point_in_polygon(mid, &poly)
                })
            {
                out.push(Target::RoofPlane(p.id));
            }
        }
    }
    out.dedup();
    out
}

/// Build Framing for Selected Object(s) / Parent Object(s) as one undo step.
/// `new_group` answers the Framing Group question for a room (see
/// [`Outcome::AskGroup`]). The result is that of the first target that could
/// not be built, else the total.
pub fn build_targets(
    cx: &mut EditorContext,
    targets: &[Target],
    new_group: Option<bool>,
) -> Outcome {
    if targets.is_empty() {
        return Outcome::Refused("Select a wall, room, roof plane, tray ceiling or truss".into());
    }
    let st = settings(&cx.project);
    let fi = cx.floor;
    // Questions come before the step opens, so cancelling leaves nothing.
    for t in targets {
        if let Target::Room(i) = t {
            if let Some(room) = cx.rooms.get(*i) {
                let anchor = crate::editor::rooms_edit::room_anchor(room);
                let group_set = room
                    .name_entry(&cx.floor().room_names)
                    .is_some_and(|n| n.options.framing_group != 0);
                let _ = group_set;
                if new_group.is_none() && needs_group_question(&cx.project, fi, &st, anchor) {
                    return Outcome::AskGroup { room: *i };
                }
            }
        }
    }
    cx.begin_change("Build Framing for Selected Object(s)");
    let mut total = 0;
    let mut refused: Option<String> = None;
    for t in targets {
        let r = match t {
            Target::Wall(id) => build_wall(&mut cx.project, fi, *id, &st),
            Target::Room(i) => match cx.rooms.get(*i) {
                Some(room) => {
                    let anchor = crate::editor::rooms_edit::room_anchor(room);
                    build_room(&mut cx.project, fi, anchor, &st, new_group)
                }
                None => Outcome::Refused("That room is gone".into()),
            },
            Target::RoofPlane(id) => build_plane(&mut cx.project, fi, *id, &st),
            Target::Tray(id) => build_tray(&mut cx.project, fi, *id, &st),
            Target::Truss(id) => build_truss(&mut cx.project, fi, *id),
        };
        match r {
            Outcome::Built(n) => total += n,
            Outcome::Refused(why) => refused = refused.or(Some(why)),
            Outcome::AskGroup { room } => {
                cx.cancel_change();
                return Outcome::AskGroup { room };
            }
        }
    }
    if total == 0 {
        if let Some(why) = refused {
            cx.cancel_change();
            cx.status = why.clone();
            return Outcome::Refused(why);
        }
    }
    super::ensure_manual_layers(&mut cx.project);
    super::ensure_layer(&mut cx.project, st.build.show_layers);
    details::refresh_details(cx);
    cx.mark_dirty();
    cx.refresh();
    cx.status = match refused {
        Some(why) => format!("Rebuilt {total} framing members; {why}"),
        None => format!("Rebuilt {total} framing members"),
    };
    Outcome::Built(total)
}

/// Build Framing for Selected Object(s) with the active selection.
pub fn build_selected(cx: &mut EditorContext, new_group: Option<bool>) -> Outcome {
    let t = selected_targets(cx);
    build_targets(cx, &t, new_group)
}

/// Build Framing for Parent Object(s) with the active selection.
pub fn build_parents(cx: &mut EditorContext, new_group: Option<bool>) -> Outcome {
    let t = parent_targets(cx);
    build_targets(cx, &t, new_group)
}

/// Retain Framing on roof planes `ids` (or off), one undo step; returns how
/// many changed. The Roof Plane Specification calls the in-step version.
pub fn set_planes_retained(cx: &mut EditorContext, ids: &[Id], retained: bool) -> usize {
    let mut st = settings(&cx.project);
    let n = ids
        .iter()
        .filter(|id| st.build.retain_planes.contains(id) != retained)
        .count();
    if n == 0 {
        return 0;
    }
    for id in ids {
        st.build.set_plane_retained(*id, retained);
    }
    cx.begin_change("Retain Framing");
    super::store_settings(&mut cx.project, &st);
    cx.mark_dirty();
    n
}

/// Retain Floor/Ceiling Framing on the room at `anchor` (or off), no undo
/// step of its own (the Room Specification calls it inside its own step).
pub fn set_room_retained_in(
    project: &mut Project,
    fi: usize,
    anchor: Point,
    retained: bool,
) -> bool {
    let rooms = plan_core::detect_rooms(&project.floors[fi].walls, 0.5);
    let floor = &mut project.floors[fi];
    let Some(i) = rooms
        .iter()
        .find(|r| r.contains(anchor))
        .and_then(|r| floor.room_names.iter().position(|n| r.contains(n.anchor)))
    else {
        return false;
    };
    floor.room_names[i].options.retain_framing = retained;
    true
}

// ----- Framing Groups -----

/// The Framing Group of the room at `anchor`.
pub fn room_group(project: &Project, fi: usize, anchor: Point) -> u32 {
    let floor = &project.floors[fi];
    plan_core::detect_rooms(&floor.walls, 0.5)
        .iter()
        .find(|r| r.contains(anchor))
        .and_then(|r| r.name_entry(&floor.room_names))
        .map_or(0, |n| n.options.framing_group)
}

// ----- editing members -----

/// Add Break (manual p. 290) on the manual member `id` at `at`: the member is
/// cut in two, one undo step. Returns the id of the new second piece.
pub fn add_break(cx: &mut EditorContext, id: Id, at: Point) -> Option<Id> {
    let mut records = load_records(cx.floor());
    let i = records.iter().position(|r| r.id() == id)?;
    let (Record::Manual(m) | Record::Built(m)) = &records[i] else {
        return None;
    };
    let new_id = cx.project.alloc_id();
    let (a, b) = m.break_at(at, new_id)?;
    cx.begin_change("Add Break");
    records[i] = Record::Manual(a);
    records.insert(i + 1, Record::Manual(b));
    store_records(cx.floor_mut(), &records);
    select(cx, vec![id, new_id]);
    cx.mark_dirty();
    cx.refresh();
    Some(new_id)
}

/// Join and Lap Ends (or Mitre Ends with `mitre`): the first of two selected
/// manual members is joined to the second. One undo step.
pub fn join_ends(cx: &mut EditorContext, first: Id, second: Id, mitre: bool) -> bool {
    let mut records = load_records(cx.floor());
    let get = |records: &[Record], id: Id| {
        records
            .iter()
            .position(|r| r.id() == id && matches!(r, Record::Manual(_) | Record::Built(_)))
    };
    let (Some(i), Some(j)) = (get(&records, first), get(&records, second)) else {
        return false;
    };
    let (Some(a), Some(b)) = (records[i].member().cloned(), records[j].member().cloned()) else {
        return false;
    };
    let (mut a, mut b) = (a, b);
    let ok = if mitre {
        a.join_mitre(&mut b)
    } else {
        a.join_lap(&mut b)
    };
    if !ok {
        return false;
    }
    cx.begin_change(if mitre {
        "Join and Mitre Ends"
    } else {
        "Join and Lap Ends"
    });
    records[i] = Record::Manual(a);
    records[j] = Record::Manual(b);
    store_records(cx.floor_mut(), &records);
    cx.mark_dirty();
    cx.refresh();
    true
}

/// The spacing the Move to Framing Ref edit tool uses for `m`: the default
/// spacing of its kind in the framing defaults.
fn reference_spacing(st: &FramingSettings, kind: ManualMemberKind) -> f64 {
    use ManualMemberKind as K;
    match kind {
        K::Joist | K::JoistBlocking | K::FloorCeilingBeam => st.walls.joist_spacing,
        K::Rafter | K::RoofBlocking | K::RoofBeam | K::RoofPurlin => st.roof.spacing,
        K::RoofTruss | K::GirderTruss => st.roof.truss_spacing,
        K::FloorCeilingTruss => st.build.trusses.spacing,
        _ => st.walls.stud_spacing,
    }
}

/// Move to Framing Ref (manual p. 919): the selected manual members move, as
/// one group, so the first of them lies on the grid that starts at the
/// floor's Framing Reference Marker. One undo step. Returns how many moved.
pub fn move_to_reference(cx: &mut EditorContext, ids: &[Id]) -> usize {
    let Some(marker) = reference_marker(&cx.project, cx.floor) else {
        cx.status = "Place a Framing Reference Marker first".into();
        return 0;
    };
    let st = settings(&cx.project);
    let records = load_records(cx.floor());
    let members: Vec<&FramingMember> = ids
        .iter()
        .filter_map(|id| records.iter().find(|r| r.id() == *id))
        .filter_map(Record::member)
        .filter(|m| m.is_linear() || m.kind.is_truss())
        .collect();
    let Some(first) = members.first() else {
        return 0;
    };
    let dir = (first.end - first.start).normalized();
    let centre = Point::lerp(first.start, first.end, 0.5);
    let delta = plan_framing::reference_delta(
        marker.point,
        dir,
        centre,
        reference_spacing(&st, first.kind),
    );
    if delta.length() < 1e-9 {
        return 0;
    }
    let movable: Vec<Id> = members.iter().map(|m| m.id).collect();
    cx.begin_change("Move to Framing Ref");
    let n = super::translate_in(cx.floor_mut(), &movable, delta);
    cx.mark_dirty();
    cx.refresh();
    n
}

// ----- Wall Detail member edits -----

/// Edits the automatic members of the wall of the Wall Detail on screen that
/// `pick` chooses, one undo step, then redraws the detail. `edit` returns
/// whether it kept the member (false removes it).
pub fn edit_wall_members(
    cx: &mut EditorContext,
    label: &str,
    pick: &[usize],
    mut edit: impl FnMut(&mut Member) -> bool,
) -> usize {
    let Some(wall) = details::detail_wall(cx.floor()) else {
        return 0;
    };
    let Some(fi) = details::floor_of_wall(&cx.project, wall) else {
        return 0;
    };
    cx.begin_change(label);
    let mut members = load(&cx.project.floors[fi]);
    let mut n = 0;
    let mut keep: Vec<Member> = Vec::with_capacity(members.len());
    for (i, mut m) in members.drain(..).enumerate() {
        if pick.contains(&i) && m.wall_id == Some(wall) {
            n += 1;
            if !edit(&mut m) {
                continue;
            }
        }
        keep.push(m);
    }
    let built: Vec<FramingMember> = load_records(&cx.project.floors[fi])
        .into_iter()
        .filter_map(|r| match r {
            Record::Built(m) => Some(m),
            _ => None,
        })
        .collect();
    store_all(&mut cx.project.floors[fi], &keep, built);
    super::restamp(&mut cx.project, fi);
    details::refresh_details(cx);
    cx.mark_dirty();
    cx.refresh();
    n
}
