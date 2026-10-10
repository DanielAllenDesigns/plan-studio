//! The Material Painter's click: what the 3D view's pick hook hit, which
//! surfaces the painter mode and scope reach from there, and the one undo
//! step that paints (or samples, or opens) them.
//!
//! * **Mode** (palette buttons): Component = the clicked surface of the
//!   clicked object; Object = all its surfaces; Room = everything that
//!   bounds or stands in the clicked room, plus the room's floor, ceiling and
//!   wall covering; Floor = everything on the clicked floor; Plan = every
//!   floor; Blend Colors = mixes the active material into what the clicked
//!   surface shows now.
//! * **Scope** (dropdown): which of those surfaces are painted: all, only
//!   those showing the clicked surface's material, or only objects of the
//!   clicked object's kind.
//! * **Use Default Material**: the click clears the paint instead (the
//!   surfaces go back to their class default or built-in material).
//!
//! The pick hook of the 3D view (`shell/view3d_panel/pick.rs`) calls
//! [`note_pick`] with the ray and [`paint_click`] with the object it found.

use super::{
    active_material, kind_of, library_with_blends, object_id_of, parts_of, parts_of_mesh, state,
    PainterMode,
};
use crate::editor::selection::ObjectRef;
use crate::editor::{placed, rooms_edit, EditorContext};
use plan_3d::{Material, Mesh};
use plan_core::object_materials::{PartMaterial, WHOLE_OBJECT};
use plan_core::{Id, Point, Project, Room, RoomName, WallKind};
use plan_materials::{blend_name, default_assignments_for, PaintMode, PaintScope};
use plan_view3d::math::{self, Vec3};
use std::cell::Cell;

/// What the last pick ray of the 3D view hit, in scene axes (X = plan x,
/// Y = up, Z = -plan y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickDetail {
    /// The scene material of the mesh that was hit.
    pub material: Material,
    /// The point on the surface.
    pub point: Vec3,
    /// The direction the ray travelled (eye to surface).
    pub dir: Vec3,
    /// The `object_id` of the mesh, if it has one.
    pub object: Option<Id>,
}

thread_local! {
    static PICK: Cell<Option<PickDetail>> = const { Cell::new(None) };
}

/// The recorded pick (tests set it with [`set_pick`]).
pub fn last_pick() -> Option<PickDetail> {
    PICK.with(Cell::get)
}

pub fn set_pick(detail: Option<PickDetail>) {
    PICK.with(|p| p.set(detail));
}

/// Moller-Trumbore, two-sided: distance along the ray to the triangle.
fn ray_triangle(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = math::sub(b, a);
    let e2 = math::sub(c, a);
    let p = math::cross(d, e2);
    let det = math::dot(e1, p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = math::sub(o, a);
    let u = math::dot(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = math::cross(s, e1);
    let v = math::dot(d, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = math::dot(e2, q) * inv;
    (t >= 0.0).then_some(t)
}

/// Records what the ray `origin` + t `dir` hits first among the `visible`
/// meshes (nothing when it hits none). The pick hook calls this while a
/// painter mode is on.
pub fn note_pick<'a>(
    meshes: impl IntoIterator<Item = &'a Mesh>,
    origin: Vec3,
    dir: Vec3,
    visible: impl Fn(&Mesh) -> bool,
) {
    let mut best: Option<(f32, Material, Option<Id>)> = None;
    for m in meshes {
        if !visible(m) {
            continue;
        }
        for tri in m.indices.as_chunks::<3>().0 {
            let at = |k: usize| m.vertices.get(tri[k] as usize).map(|v| v.position);
            let (Some(a), Some(b), Some(c)) = (at(0), at(1), at(2)) else {
                continue;
            };
            if let Some(t) = ray_triangle(origin, dir, a, b, c) {
                if best.is_none_or(|(bt, _, _)| t < bt) {
                    best = Some((t, m.material, m.object_id));
                }
            }
        }
    }
    set_pick(best.map(|(t, material, object)| PickDetail {
        material,
        point: math::add(origin, math::scale(dir, t)),
        dir,
        object,
    }));
}

// ----- surfaces -----

/// A surface of a room that has no 3D object of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomSurface {
    Floor,
    Ceiling,
    /// The wall covering of the room's interior faces.
    Walls,
}

impl RoomSurface {
    pub fn label(self) -> &'static str {
        match self {
            RoomSurface::Floor => "Floor",
            RoomSurface::Ceiling => "Ceiling",
            RoomSurface::Walls => "Wall covering",
        }
    }
}

/// One surface a click can paint.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A part of an object (`""` = the whole object) on floor `floor`.
    Part {
        floor: usize,
        obj: ObjectRef,
        part: String,
    },
    /// A surface of room `room` (index into the rooms of floor `floor`).
    Room {
        floor: usize,
        room: usize,
        surface: RoomSurface,
    },
}

/// What a click hit.
#[derive(Clone, Debug)]
pub struct Click {
    pub floor: usize,
    pub obj: Option<ObjectRef>,
    /// The clicked surface of `obj`, when the pick told which.
    pub part: Option<String>,
    /// The room under the click (floor, index).
    pub room: Option<(usize, usize)>,
    /// The click hit a room's floor or ceiling plate.
    pub plate: Option<RoomSurface>,
}

fn rooms_on(cx: &mut EditorContext, floor: usize) -> Vec<Room> {
    if floor == cx.floor {
        cx.rooms_now().to_vec()
    } else {
        cx.project
            .floors
            .get(floor)
            .map(|f| plan_core::detect_rooms(&f.walls, 0.5))
            .unwrap_or_default()
    }
}

fn room_at(rooms: &[Room], p: Point) -> Option<usize> {
    rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| r.contains(p))
        .min_by(|a, b| a.1.area_sq_in.total_cmp(&b.1.area_sq_in))
        .map(|(i, _)| i)
}

/// The floor whose storey contains height `y` (scene inches).
fn floor_at_height(project: &Project, y: f32) -> usize {
    let y = f64::from(y) + 1.0;
    project
        .floors
        .iter()
        .enumerate()
        .filter(|(_, f)| f.elevation <= y)
        .max_by(|a, b| a.1.elevation.total_cmp(&b.1.elevation))
        .map_or(0, |(i, _)| i)
}

fn plan_point(v: Vec3) -> Point {
    Point::new(f64::from(v[0]), -f64::from(v[2]))
}

/// Which face of a wall the ray hit: the interior face when the point a
/// little way back toward the viewer lies in a room.
fn wall_face(project: &Project, floor: usize, wall: Id, d: &PickDetail, rooms: &[Room]) -> String {
    const INTERIOR: &str = "Interior Wall Surface";
    const EXTERIOR: &str = "Exterior Wall Surface";
    let Some(w) = project.floors.get(floor).and_then(|f| f.wall(wall)) else {
        return INTERIOR.into();
    };
    if w.kind == WallKind::Interior {
        return INTERIOR.into();
    }
    let travel = Point::new(f64::from(d.dir[0]), -f64::from(d.dir[2]));
    let len = (travel.x * travel.x + travel.y * travel.y).sqrt();
    if len < 1e-6 {
        return EXTERIOR.into();
    }
    let back = Point::new(travel.x / len, travel.y / len);
    let reach = w.thickness / 2.0 + 6.0;
    let viewer = plan_point(d.point);
    let test = Point::new(viewer.x - back.x * reach, viewer.y - back.y * reach);
    if room_at(rooms, test).is_some() {
        INTERIOR.into()
    } else {
        EXTERIOR.into()
    }
}

/// Works out what a click on `hit` reached. The recorded pick is used only
/// when it is of the same object.
pub fn analyze(cx: &mut EditorContext, hit: Option<(usize, ObjectRef)>) -> Option<Click> {
    let hit_id = hit.and_then(|(_, o)| object_id_of(o));
    let detail = last_pick().filter(|d| d.object == hit_id);
    let floor = match (hit, detail) {
        (Some((f, _)), _) => f,
        (None, Some(d)) => floor_at_height(&cx.project, d.point[1]),
        (None, None) => return None,
    };
    let rooms = rooms_on(cx, floor);
    let at = detail.map(|d| plan_point(d.point));
    match hit {
        Some((_, obj)) => {
            // A room reference stands for the room itself.
            if let ObjectRef::Room(i) = obj {
                return Some(Click {
                    floor,
                    obj: None,
                    part: None,
                    room: (i < rooms.len()).then_some((floor, i)),
                    plate: None,
                });
            }
            object_id_of(obj)?;
            let part = detail.and_then(|d| match obj {
                ObjectRef::Wall(id) => Some(wall_face(&cx.project, floor, id, &d, &rooms)),
                _ => {
                    let parts = parts_of(&cx.project, floor, obj);
                    parts_of_mesh(d.material)
                        .iter()
                        .find(|p| parts.iter().any(|q| q == **p))
                        .map(|p| (*p).to_string())
                }
            });
            let room = detail.and_then(|d| {
                // For a wall, the room on the face that was hit.
                let p = match obj {
                    ObjectRef::Wall(_) => {
                        let t = Point::new(f64::from(d.dir[0]), -f64::from(d.dir[2]));
                        let l = (t.x * t.x + t.y * t.y).sqrt().max(1e-6);
                        let q = plan_point(d.point);
                        Point::new(q.x - t.x / l * 12.0, q.y - t.y / l * 12.0)
                    }
                    _ => plan_point(d.point),
                };
                room_at(&rooms, p).map(|i| (floor, i))
            });
            Some(Click {
                floor,
                obj: Some(obj),
                part,
                room,
                plate: None,
            })
        }
        None => {
            let d = detail?;
            let plate = match d.material {
                Material::Floor => RoomSurface::Floor,
                Material::Ceiling => RoomSurface::Ceiling,
                _ => return None,
            };
            let room = room_at(&rooms, at?)?;
            Some(Click {
                floor,
                obj: None,
                part: None,
                room: Some((floor, room)),
                plate: Some(plate),
            })
        }
    }
}

// ----- enumerating objects -----

fn cabinet_ids(project: &Project, floor: usize) -> Vec<Id> {
    project
        .floors
        .get(floor)
        .map(|f| placed::load_cabinets(f).into_iter().map(|c| c.id).collect())
        .unwrap_or_default()
}

fn roof_plane_ids(project: &Project, floor: usize) -> Vec<Id> {
    project
        .floors
        .get(floor)
        .map(|f| {
            f.roofs
                .iter()
                .filter(|v| v.get("kind").and_then(|k| k.as_str()) == Some("plane"))
                .filter_map(|v| v.get("id").and_then(|i| i.as_u64()))
                .collect()
        })
        .unwrap_or_default()
}

/// Every object of a floor that takes a material.
fn floor_objects(project: &Project, floor: usize) -> Vec<ObjectRef> {
    let Some(f) = project.floors.get(floor) else {
        return Vec::new();
    };
    let mut out: Vec<ObjectRef> = f.walls.iter().map(|w| ObjectRef::Wall(w.id)).collect();
    out.extend(f.openings.iter().map(|o| ObjectRef::Opening(o.id)));
    out.extend(
        cabinet_ids(project, floor)
            .into_iter()
            .map(ObjectRef::Cabinet),
    );
    out.extend(
        roof_plane_ids(project, floor)
            .into_iter()
            .map(ObjectRef::RoofPlane),
    );
    out
}

/// Distance from `p` to the closed polygon's outline.
fn boundary_distance(poly: &[Point], p: Point) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let (abx, aby) = (b.x - a.x, b.y - a.y);
            let l2 = abx * abx + aby * aby;
            let t = if l2 < 1e-12 {
                0.0
            } else {
                (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
            };
            let (cx, cy) = (a.x + abx * t, a.y + aby * t);
            ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

/// The walls that bound a room (centreline on its outline), the openings in
/// them and the cabinets standing in it.
pub fn room_objects(project: &Project, floor: usize, room: &Room) -> Vec<ObjectRef> {
    let Some(f) = project.floors.get(floor) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut wall_ids = Vec::new();
    for w in &f.walls {
        let tol = w.thickness.max(2.0);
        let mid = w.point_at(w.length() / 2.0);
        if boundary_distance(&room.polygon, w.start) <= tol
            && boundary_distance(&room.polygon, w.end) <= tol
            && boundary_distance(&room.polygon, mid) <= tol
        {
            out.push(ObjectRef::Wall(w.id));
            wall_ids.push(w.id);
        }
    }
    out.extend(
        f.openings
            .iter()
            .filter(|o| wall_ids.contains(&o.wall_id))
            .map(|o| ObjectRef::Opening(o.id)),
    );
    for c in placed::load_cabinets(f) {
        if room.contains(c.position) {
            out.push(ObjectRef::Cabinet(c.id));
        }
    }
    out
}

fn room_targets(floor: usize, room: usize) -> Vec<Target> {
    [RoomSurface::Floor, RoomSurface::Ceiling, RoomSurface::Walls]
        .into_iter()
        .map(|surface| Target::Room {
            floor,
            room,
            surface,
        })
        .collect()
}

fn whole(floor: usize, obj: ObjectRef) -> Target {
    Target::Part {
        floor,
        obj,
        part: WHOLE_OBJECT.to_string(),
    }
}

// ----- what a surface shows now -----

/// The kind of object, for the Same Object Type scope and the class defaults.
pub fn type_key(project: &Project, floor: usize, obj: ObjectRef) -> &'static str {
    kind_of(project, floor, obj).unwrap_or(match obj {
        ObjectRef::Stair(_) => "Stair",
        ObjectRef::Symbol(_) => "Symbol",
        ObjectRef::Foundation(_) => "Foundation",
        _ => "Other",
    })
}

fn built_in(class: &str, part: &str) -> Option<String> {
    default_assignments_for(class)
        .into_iter()
        .find(|a| a.component == part)
        .map(|a| a.material)
}

fn room_entry(project: &Project, floor: usize, room: &Room) -> Option<RoomName> {
    project
        .floors
        .get(floor)
        .and_then(|f| room.name_entry(&f.room_names).cloned())
}

/// The library material `target` shows now: its own paint, else its class
/// default, else the built-in material of its kind; `None` when none.
pub fn current_material(
    cx: &mut EditorContext,
    target: &Target,
    rooms_cache: &mut Vec<(usize, Vec<Room>)>,
) -> Option<String> {
    match target {
        Target::Part { floor, obj, part } => {
            let id = object_id_of(*obj)?;
            let class = kind_of(&cx.project, *floor, *obj);
            // The exact part, else the whole-object paint.
            if let Some(n) = cx.project.object_material(id, part) {
                return Some(n.to_string());
            }
            let class = class?;
            if let Some(n) = cx.project.class_material(class, part) {
                return Some(n.to_string());
            }
            built_in(class, part)
        }
        Target::Room {
            floor,
            room,
            surface,
        } => {
            // The room's own surface, else the floor's default surfaces
            // (Floor Defaults), else the plan's Room class default.
            let own = own_room_material(cx, *floor, *room, *surface, rooms_cache);
            let settings = &cx.project.floors.get(*floor)?.settings;
            let (floor_default, part) = match surface {
                RoomSurface::Floor => (&settings.floor_material, "Floor Finish"),
                RoomSurface::Ceiling => (&settings.ceiling_material, "Ceiling Finish"),
                RoomSurface::Walls => (&settings.wall_material, "Interior Wall Surface"),
            };
            own.or_else(|| named(floor_default))
                .or_else(|| cx.project.class_material("Room", part).map(str::to_string))
        }
    }
}

fn named(s: &str) -> Option<String> {
    (!s.trim().is_empty()).then(|| s.to_string())
}

/// The material the room itself was given for `surface` (its name entry),
/// without any default.
fn own_room_material(
    cx: &mut EditorContext,
    floor: usize,
    room: usize,
    surface: RoomSurface,
    rooms_cache: &mut Vec<(usize, Vec<Room>)>,
) -> Option<String> {
    if !rooms_cache.iter().any(|(f, _)| *f == floor) {
        let rooms = rooms_on(cx, floor);
        rooms_cache.push((floor, rooms));
    }
    let rooms = &rooms_cache.iter().find(|(f, _)| *f == floor)?.1;
    let r = rooms.get(room)?;
    let entry = room_entry(&cx.project, floor, r)?;
    match surface {
        RoomSurface::Floor => entry.floor_finish,
        RoomSurface::Ceiling => entry.ceiling_finish,
        RoomSurface::Walls => entry.misc.map(|m| m.wall_covering),
    }
    .as_deref()
    .and_then(named)
}

/// The kind a target belongs to (Same Object Type).
fn target_type(project: &Project, t: &Target) -> String {
    match t {
        Target::Part { floor, obj, .. } => type_key(project, *floor, *obj).to_string(),
        Target::Room { surface, .. } => format!("Room {}", surface.label()),
    }
}

// ----- choosing the surfaces -----

/// The surfaces `mode` reaches from `click`, narrowed by `scope`.
pub fn targets_for(
    cx: &mut EditorContext,
    click: &Click,
    mode: PaintMode,
    scope: PaintScope,
) -> Vec<Target> {
    // The surface itself.
    let primary: Option<Target> = match (&click.obj, &click.part, click.plate, click.room) {
        (Some(obj), Some(part), _, _)
            if matches!(mode, PaintMode::Component | PaintMode::BlendColors) =>
        {
            Some(Target::Part {
                floor: click.floor,
                obj: *obj,
                part: part.clone(),
            })
        }
        (Some(obj), _, _, _) => Some(whole(click.floor, *obj)),
        (None, _, Some(surface), Some((f, r))) => Some(Target::Room {
            floor: f,
            room: r,
            surface,
        }),
        _ => None,
    };
    let mut all: Vec<Target> = match mode {
        PaintMode::Component | PaintMode::Object | PaintMode::BlendColors => {
            primary.clone().into_iter().collect()
        }
        PaintMode::Room => match click.room {
            Some((f, r)) => {
                let rooms = rooms_on(cx, f);
                let mut v: Vec<Target> = rooms
                    .get(r)
                    .map(|room| {
                        room_objects(&cx.project, f, room)
                            .into_iter()
                            .map(|o| whole(f, o))
                            .collect()
                    })
                    .unwrap_or_default();
                v.extend(room_targets(f, r));
                v
            }
            None => Vec::new(),
        },
        PaintMode::Floor => floor_wide(cx, click.floor),
        PaintMode::Plan => (0..cx.project.floors.len())
            .flat_map(|f| floor_wide(cx, f))
            .collect(),
    };
    // The Scope dropdown narrows a wide extent.
    if mode.is_wide() {
        match scope {
            PaintScope::AllSurfaces => {}
            PaintScope::SameMaterial => {
                let mut cache = Vec::new();
                let want = primary
                    .as_ref()
                    .and_then(|t| current_material(cx, t, &mut cache));
                all.retain(|t| current_material(cx, t, &mut cache) == want);
            }
            PaintScope::SameType => {
                if let Some(p) = &primary {
                    let want = target_type(&cx.project, p);
                    all.retain(|t| target_type(&cx.project, t) == want);
                }
            }
        }
    }
    all
}

fn floor_wide(cx: &mut EditorContext, floor: usize) -> Vec<Target> {
    let mut v: Vec<Target> = floor_objects(&cx.project, floor)
        .into_iter()
        .map(|o| whole(floor, o))
        .collect();
    let n = rooms_on(cx, floor).len();
    for r in 0..n {
        v.extend(room_targets(floor, r));
    }
    v
}

// ----- applying -----

/// What a click does to each surface.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Set(String),
    Clear,
}

fn ensure_room_entry(cx: &mut EditorContext, floor: usize, room: &Room) -> Option<usize> {
    let f = cx.project.floors.get_mut(floor)?;
    if let Some(i) = f.room_names.iter().position(|n| room.contains(n.anchor)) {
        return Some(i);
    }
    let label = if room.label.is_empty() {
        "Room".to_string()
    } else {
        room.label.clone()
    };
    f.room_names.push(RoomName::new(
        rooms_edit::room_anchor(room),
        label,
        f.settings.default_room_type.clone(),
    ));
    Some(f.room_names.len() - 1)
}

/// Does `action` change `target`?
fn would_change(
    cx: &mut EditorContext,
    target: &Target,
    action: &Action,
    cache: &mut Vec<(usize, Vec<Room>)>,
) -> bool {
    match (target, action) {
        (Target::Part { obj, part, .. }, Action::Set(name)) => {
            let Some(id) = object_id_of(*obj) else {
                return false;
            };
            let parts = cx.project.object_materials_of(id);
            !parts.iter().any(|p| p.part == *part && p.material == *name)
        }
        (Target::Part { obj, part, .. }, Action::Clear) => {
            let Some(id) = object_id_of(*obj) else {
                return false;
            };
            cx.project
                .object_materials_of(id)
                .iter()
                .any(|p| part.is_empty() || p.part == *part)
        }
        (Target::Room { .. }, Action::Set(name)) => {
            current_material(cx, target, cache).as_deref() != Some(name.as_str())
        }
        (
            Target::Room {
                floor,
                room,
                surface,
            },
            Action::Clear,
        ) => own_room_material(cx, *floor, *room, *surface, cache).is_some(),
    }
}

/// Carries `action` out on `target` (no undo step is opened here).
fn apply_to(cx: &mut EditorContext, target: &Target, action: &Action) {
    match target {
        Target::Part { obj, part, .. } => {
            let Some(id) = object_id_of(*obj) else { return };
            match action {
                Action::Set(name) => {
                    // Painting the whole object replaces its part paint, so
                    // the whole-object material is what shows.
                    if part.is_empty() {
                        cx.project.clear_object_material(id, None);
                    }
                    cx.project.set_object_material(id, part, name);
                }
                Action::Clear => {
                    if part.is_empty() {
                        cx.project.clear_object_material(id, None);
                    } else {
                        cx.project.clear_object_material(id, Some(part));
                    }
                }
            }
        }
        Target::Room {
            floor,
            room,
            surface,
        } => {
            let rooms = rooms_on(cx, *floor);
            let Some(r) = rooms.get(*room) else { return };
            let value = match action {
                Action::Set(n) => Some(n.clone()),
                Action::Clear => None,
            };
            let Some(i) = ensure_room_entry(cx, *floor, r) else {
                return;
            };
            let entry = &mut cx.project.floors[*floor].room_names[i];
            match surface {
                RoomSurface::Floor => entry.floor_finish = value,
                RoomSurface::Ceiling => entry.ceiling_finish = value,
                RoomSurface::Walls => {
                    let misc = entry.misc.get_or_insert_with(Default::default);
                    misc.wall_covering = value.unwrap_or_default();
                }
            }
        }
    }
}

/// The paint of a whole object, remembered by the Object Eyedropper to paint
/// onto objects of the same kind.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectSet {
    pub kind: String,
    pub parts: Vec<PartMaterial>,
}

/// What a plan-wide result reports.
fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// A click of the painter. Returns whether the plan changed.
pub fn paint_click(cx: &mut EditorContext, hit: Option<(usize, ObjectRef)>) -> bool {
    let mode = super::painter_mode();
    if mode == PainterMode::Off {
        return false;
    }
    let Some(click) = analyze(cx, hit) else {
        cx.status = match hit {
            Some(_) => "That surface belongs to no object that can take a material".into(),
            None => "Click a surface of the model".into(),
        };
        return false;
    };
    match mode {
        PainterMode::Off => false,
        PainterMode::Paint => paint(cx, &click),
        PainterMode::Eyedropper | PainterMode::ObjectEyedropper | PainterMode::Adjust => {
            sample(cx, &click, mode);
            false
        }
        PainterMode::Erase => erase(cx, &click),
    }
}

/// The surface the click itself is on (Component when the pick told the
/// part, else the whole object or room surface).
fn clicked_target(click: &Click) -> Option<Target> {
    match (&click.obj, &click.part, click.plate, click.room) {
        (Some(obj), Some(part), _, _) => Some(Target::Part {
            floor: click.floor,
            obj: *obj,
            part: part.clone(),
        }),
        (Some(obj), _, _, _) => Some(whole(click.floor, *obj)),
        (None, _, Some(surface), Some((f, r))) => Some(Target::Room {
            floor: f,
            room: r,
            surface,
        }),
        _ => None,
    }
}

fn paint(cx: &mut EditorContext, click: &Click) -> bool {
    let (paint_mode, scope, use_default, blend_pct, set) = state(|s| {
        (
            s.paint_mode,
            s.scope,
            s.use_default,
            s.blend_pct,
            s.object_set.clone(),
        )
    });
    let lib = library_with_blends(&cx.project);
    let active = active_material();
    if !use_default {
        match active.as_deref() {
            None => {
                cx.status = "Pick a material in the Materials list first (3D > Materials)".into();
                return false;
            }
            Some(n) if lib.resolve(n).is_none() => {
                cx.status = format!("The material {n} is not in the library");
                return false;
            }
            Some(_) => {}
        }
    }
    let targets = targets_for(cx, click, paint_mode, scope);
    if targets.is_empty() {
        cx.status = match paint_mode {
            PaintMode::Room => "Click a surface inside a room to paint the room".into(),
            _ => "Nothing to paint there".into(),
        };
        return false;
    }
    let mut cache = Vec::new();
    // Work out each surface's action first so nothing is half painted.
    let mut plan: Vec<(Target, Action)> = Vec::new();
    let mut unblended = 0;
    for t in &targets {
        let action = if use_default {
            Action::Clear
        } else if paint_mode == PaintMode::BlendColors {
            let active = active.clone().unwrap_or_default();
            match current_material(cx, t, &mut cache) {
                Some(base) if base != active => Action::Set(blend_name(&base, &active, blend_pct)),
                _ => {
                    unblended += 1;
                    Action::Set(active)
                }
            }
        } else {
            Action::Set(active.clone().unwrap_or_default())
        };
        plan.push((t.clone(), action));
    }
    // An Object Eyedropper set paints its parts onto objects of its kind.
    if let (Some(set), false, false) = (&set, use_default, paint_mode == PaintMode::BlendColors) {
        let mut extra: Vec<(Target, Action)> = Vec::new();
        plan.retain(|(t, _)| match t {
            Target::Part { floor, obj, .. } if type_key(&cx.project, *floor, *obj) == set.kind => {
                for p in &set.parts {
                    extra.push((
                        Target::Part {
                            floor: *floor,
                            obj: *obj,
                            part: p.part.clone(),
                        },
                        Action::Set(p.material.clone()),
                    ));
                }
                false
            }
            _ => true,
        });
        plan.extend(extra);
    }
    let changing: Vec<&(Target, Action)> = plan
        .iter()
        .filter(|(t, a)| would_change(cx, t, a, &mut cache))
        .collect();
    if changing.is_empty() {
        return false;
    }
    let changing: Vec<(Target, Action)> = changing.into_iter().cloned().collect();
    cx.begin_change(if use_default {
        "Use Default Material"
    } else {
        "Paint Material"
    });
    for (t, a) in &changing {
        apply_to(cx, t, a);
    }
    cx.mark_dirty();
    let n = changing.len();
    cx.status = if use_default {
        format!("{} back to the default material", plural(n, "surface"))
    } else if let Some(set) = &set {
        format!("Painted the {} material set", set.kind)
    } else if paint_mode == PaintMode::BlendColors {
        let note = if unblended > 0 {
            format!(" ({unblended} had nothing to blend with)")
        } else {
            String::new()
        };
        format!(
            "Blended {}% of {} into {}{note}",
            blend_pct,
            active.unwrap_or_default(),
            plural(n, "surface")
        )
    } else if n == 1 {
        format!("Painted with {}", active.unwrap_or_default())
    } else {
        format!(
            "Painted {} with {}",
            plural(n, "surface"),
            active.unwrap_or_default()
        )
    };
    true
}

/// Delete Surface: the click clears the paint of the surfaces its mode and
/// scope reach.
fn erase(cx: &mut EditorContext, click: &Click) -> bool {
    let (paint_mode, scope) = state(|s| (s.paint_mode, s.scope));
    let targets = targets_for(cx, click, paint_mode, scope);
    let mut cache = Vec::new();
    let changing: Vec<Target> = targets
        .into_iter()
        .filter(|t| would_change(cx, t, &Action::Clear, &mut cache))
        .collect();
    if changing.is_empty() {
        return false;
    }
    cx.begin_change("Delete Surface");
    for t in &changing {
        apply_to(cx, t, &Action::Clear);
    }
    cx.mark_dirty();
    cx.status = "Removed the painted material".into();
    true
}

/// The material a click shows (what the eyedroppers pick).
pub fn material_of_click(cx: &mut EditorContext, click: &Click) -> Option<String> {
    let t = clicked_target(click)?;
    let mut cache = Vec::new();
    current_material(cx, &t, &mut cache)
}

/// Eyedropper (the clicked surface's material becomes the active one), Object
/// Eyedropper (the object's whole set of paint is remembered) and Adjust
/// Material Definition (opens the clicked material's specification).
fn sample(cx: &mut EditorContext, click: &Click, mode: PainterMode) {
    if mode == PainterMode::ObjectEyedropper {
        let Some(obj) = click.obj else {
            cx.status = "Click an object to copy its materials".into();
            return;
        };
        let Some(id) = object_id_of(obj) else { return };
        let parts = cx.project.object_materials_of(id).to_vec();
        if parts.is_empty() {
            cx.status = "That object has no painted materials to copy".into();
            return;
        }
        let kind = type_key(&cx.project, click.floor, obj).to_string();
        cx.status = format!(
            "Copied the {} material set ({}): paint it onto another {}",
            kind,
            plural(parts.len(), "part"),
            kind.to_lowercase()
        );
        state(|s| s.object_set = Some(ObjectSet { kind, parts }));
        return;
    }
    let Some(name) = material_of_click(cx, click) else {
        cx.status = "That surface has no material".into();
        return;
    };
    let lib = library_with_blends(&cx.project);
    if mode == PainterMode::Adjust {
        match lib.resolve(&name) {
            Some(def) => {
                super::open_spec_for(&def);
                cx.status = format!("Material definition of {name}");
            }
            None => cx.status = format!("The material {name} is not in the library"),
        }
        return;
    }
    cx.status = format!("Material {name} is active");
    state(|s| {
        s.active = Some(name);
        s.object_set = None;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{OpeningKind, WallKind};

    /// A 10 x 10 foot room of four exterior walls with a window and a door.
    fn house() -> (EditorContext, [Id; 4], Id, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            Point::new(0.0, 120.0),
        ];
        let mut walls = [0; 4];
        for i in 0..4 {
            walls[i] = cx
                .project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
        let win = cx
            .project
            .add_opening(0, walls[0], 60.0, OpeningKind::Window)
            .expect("window");
        let door = cx
            .project
            .add_opening(0, walls[1], 60.0, OpeningKind::Door)
            .expect("door");
        cx.refresh();
        (cx, walls, win, door)
    }

    fn use_mode(mode: PaintMode, scope: PaintScope) {
        super::super::set_painter_mode(PainterMode::Paint);
        state(|s| {
            s.paint_mode = mode;
            s.scope = scope;
            s.use_default = false;
            s.object_set = None;
        });
    }

    fn pick_on_wall(wall_at: Point, inward: Point) -> PickDetail {
        // A ray travelling from inside the room toward the wall point.
        let d = Point::new(wall_at.x - inward.x, wall_at.y - inward.y);
        let l = (d.x * d.x + d.y * d.y).sqrt();
        PickDetail {
            material: Material::WallInterior,
            point: [wall_at.x as f32, 48.0, -wall_at.y as f32],
            dir: [(d.x / l) as f32, 0.0, -(d.y / l) as f32],
            object: None,
        }
    }

    #[test]
    fn component_object_room_floor_and_plan_reach_more_and_more() {
        let (mut cx, walls, win, _door) = house();
        super::super::set_active(Some("Brick – Red".into()));
        let lib = super::super::library();
        let name = lib
            .materials
            .iter()
            .find(|m| m.category.first().map(String::as_str) == Some("Masonry"))
            .unwrap()
            .name
            .clone();
        super::super::set_active(Some(name.clone()));
        // Component: the exterior face only (a ray from outside the room).
        let d = {
            let mut d = pick_on_wall(Point::new(60.0, 0.0), Point::new(60.0, 60.0));
            // Travelling toward the room (+y plan): a viewer outside.
            d.dir = [0.0, 0.0, -1.0];
            d.object = Some(walls[0]);
            d
        };
        set_pick(Some(d));
        use_mode(PaintMode::Component, PaintScope::AllSurfaces);
        assert!(paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[0])))));
        assert_eq!(
            cx.project
                .object_material(walls[0], "Exterior Wall Surface"),
            Some(name.as_str())
        );
        assert_eq!(
            cx.project.object_materials_of(walls[0]).len(),
            1,
            "one part, not the whole wall"
        );
        assert_eq!(cx.undo_label(), Some("Paint Material"));
        // Object: the whole wall.
        set_pick(None);
        use_mode(PaintMode::Object, PaintScope::AllSurfaces);
        assert!(paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[1])))));
        assert_eq!(
            cx.project.object_material(walls[1], WHOLE_OBJECT),
            Some(name.as_str())
        );
        // Floor: every wall and opening (and the room surfaces).
        use_mode(PaintMode::Floor, PaintScope::AllSurfaces);
        assert!(paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[2])))));
        for w in walls {
            assert_eq!(
                cx.project.object_material(w, WHOLE_OBJECT),
                Some(name.as_str()),
                "wall {w}"
            );
        }
        assert_eq!(
            cx.project.object_material(win, WHOLE_OBJECT),
            Some(name.as_str())
        );
        let entry = &cx.project.floors[0].room_names;
        assert_eq!(entry.len(), 1, "the room got a name entry for its surfaces");
        assert_eq!(entry[0].floor_finish.as_deref(), Some(name.as_str()));
        assert_eq!(entry[0].ceiling_finish.as_deref(), Some(name.as_str()));
        assert_eq!(entry[0].misc.as_ref().unwrap().wall_covering, name);
        // One undo step restores all of it.
        cx.undo();
        assert_eq!(cx.project.object_material(walls[2], WHOLE_OBJECT), None);
        assert!(cx.project.floors[0].room_names.is_empty());
        super::super::set_painter_mode(PainterMode::Off);
    }

    #[test]
    fn plan_mode_reaches_every_floor() {
        let (mut cx, walls, _, _) = house();
        // A second floor with one wall.
        cx.project
            .floors
            .push(plan_core::Floor::new("2nd Floor", 108.0));
        let upper = cx.project.add_wall(
            1,
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        super::super::set_active(Some("Drywall".into()));
        set_pick(None);
        use_mode(PaintMode::Plan, PaintScope::AllSurfaces);
        assert!(paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[0])))));
        assert_eq!(
            cx.project.object_material(upper, WHOLE_OBJECT),
            Some("Drywall")
        );
        use_mode(PaintMode::Floor, PaintScope::AllSurfaces);
        super::super::set_painter_mode(PainterMode::Off);
    }

    #[test]
    fn room_mode_paints_the_room_and_the_scope_narrows_it() {
        let (mut cx, walls, win, door) = house();
        // Paint the window "Drywall" first so Same Material can find it.
        cx.project
            .set_object_material(win, WHOLE_OBJECT, "Clear Glass");
        super::super::set_active(Some("Quartz – White".into()));
        // Click the inside of wall 2 (a ray from the room's middle).
        let mut d = pick_on_wall(Point::new(60.0, 120.0), Point::new(60.0, 60.0));
        d.object = Some(walls[2]);
        set_pick(Some(d));
        use_mode(PaintMode::Room, PaintScope::SameType);
        assert!(paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[2])))));
        for w in walls {
            assert_eq!(
                cx.project.object_material(w, WHOLE_OBJECT),
                Some("Quartz – White")
            );
        }
        assert_eq!(
            cx.project.object_material(win, WHOLE_OBJECT),
            Some("Clear Glass"),
            "Same Object Type leaves the window alone"
        );
        assert_eq!(cx.project.object_material(door, WHOLE_OBJECT), None);
        assert!(
            cx.project.floors[0].room_names.is_empty(),
            "room surfaces are not walls"
        );
        // Same Material: paint the walls that show Quartz with Drywall.
        super::super::set_active(Some("Drywall".into()));
        use_mode(PaintMode::Room, PaintScope::SameMaterial);
        assert!(paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[2])))));
        for w in walls {
            assert_eq!(cx.project.object_material(w, WHOLE_OBJECT), Some("Drywall"));
        }
        assert_eq!(
            cx.project.object_material(win, WHOLE_OBJECT),
            Some("Clear Glass")
        );
        super::super::set_painter_mode(PainterMode::Off);
    }

    #[test]
    fn a_click_on_a_floor_plate_paints_that_rooms_floor() {
        let (mut cx, _, _, _) = house();
        super::super::set_active(Some("Oak Flooring".into()));
        set_pick(Some(PickDetail {
            material: Material::Floor,
            point: [60.0, 0.5, -60.0],
            dir: [0.0, -1.0, 0.0],
            object: None,
        }));
        use_mode(PaintMode::Object, PaintScope::AllSurfaces);
        assert!(paint_click(&mut cx, None));
        let e = &cx.project.floors[0].room_names[0];
        assert_eq!(e.floor_finish.as_deref(), Some("Oak Flooring"));
        assert_eq!(e.ceiling_finish, None, "only the floor");
        // Use Default Material puts it back.
        state(|s| s.use_default = true);
        assert!(paint_click(&mut cx, None));
        assert_eq!(cx.project.floors[0].room_names[0].floor_finish, None);
        // A miss in the sky does nothing.
        set_pick(None);
        assert!(!paint_click(&mut cx, None));
        assert!(cx.status.contains("Click a surface"));
        state(|s| s.use_default = false);
        super::super::set_painter_mode(PainterMode::Off);
    }

    #[test]
    fn use_default_material_clears_the_paint_in_the_extent() {
        let (mut cx, walls, win, _) = house();
        for w in walls {
            cx.project.set_object_material(w, WHOLE_OBJECT, "Drywall");
        }
        cx.project
            .set_object_material(win, WHOLE_OBJECT, "Clear Glass");
        set_pick(None);
        use_mode(PaintMode::Floor, PaintScope::AllSurfaces);
        state(|s| s.use_default = true);
        assert!(paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[0])))));
        assert!(cx.project.object_materials.is_empty());
        assert_eq!(cx.undo_label(), Some("Use Default Material"));
        // Nothing left to clear: no empty undo step.
        let before = cx.undo_label().map(str::to_string);
        assert!(!paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[0])))));
        assert_eq!(cx.undo_label().map(str::to_string), before);
        state(|s| s.use_default = false);
        super::super::set_painter_mode(PainterMode::Off);
    }

    #[test]
    fn blend_colors_mixes_the_active_material_into_the_current_one() {
        let (mut cx, walls, _, _) = house();
        cx.project
            .set_object_material(walls[0], WHOLE_OBJECT, "Drywall");
        super::super::set_active(Some("Color – Bone".into()));
        set_pick(None);
        use_mode(PaintMode::BlendColors, PaintScope::AllSurfaces);
        state(|s| s.blend_pct = 40);
        assert!(paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[0])))));
        let got = cx
            .project
            .object_material(walls[0], WHOLE_OBJECT)
            .unwrap()
            .to_string();
        assert_eq!(got, "Blend: Drywall | Color – Bone | 40%");
        // The name resolves to a material halfway between the two colours.
        let lib = library_with_blends(&cx.project);
        let m = lib.resolve(&got).unwrap();
        let a = lib.find("Drywall").unwrap().color;
        let b = lib.find("Color – Bone").unwrap().color;
        for k in 0..3 {
            let want = f32::from(a[k]) * 0.6 + f32::from(b[k]) * 0.4;
            assert!((f32::from(m.color[k]) - want).abs() <= 1.0);
        }
        // A wall with no paint blends with its built-in material.
        assert!(paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[1])))));
        assert_eq!(
            cx.project.object_material(walls[1], WHOLE_OBJECT),
            Some("Color – Bone"),
            "nothing under whole-object paint to blend with"
        );
        super::super::set_painter_mode(PainterMode::Off);
    }

    #[test]
    fn the_eyedroppers_and_adjust_pick_the_clicked_material() {
        let (mut cx, walls, win, _) = house();
        cx.project
            .set_object_material(walls[0], WHOLE_OBJECT, "Drywall");
        cx.project
            .set_object_material(walls[0], "Sill Plate", "Fir Framing");
        set_pick(None);
        super::super::set_active(None);
        super::super::set_painter_mode(PainterMode::Eyedropper);
        assert!(!paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[0])))));
        assert_eq!(active_material().as_deref(), Some("Drywall"));
        // An unpainted window shows its built-in glass material.
        assert!(!paint_click(&mut cx, Some((0, ObjectRef::Opening(win)))));
        // (Whole-object click: no part, so no built-in to pick.)
        assert_eq!(active_material().as_deref(), Some("Drywall"));
        // The Object Eyedropper keeps the whole set and paints it onto
        // another wall in one click.
        super::super::set_painter_mode(PainterMode::ObjectEyedropper);
        assert!(!paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[0])))));
        assert_eq!(state(|s| s.object_set.clone()).unwrap().parts.len(), 2);
        use_mode(PaintMode::Object, PaintScope::AllSurfaces);
        state(|s| {
            s.object_set = Some(ObjectSet {
                kind: "Wall".into(),
                parts: cx.project.object_materials_of(walls[0]).to_vec(),
            })
        });
        assert!(paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[3])))));
        assert_eq!(
            cx.project.object_materials_of(walls[3]),
            cx.project.object_materials_of(walls[0])
        );
        // Adjust Material Definition opens the specification.
        state(|s| s.object_set = None);
        super::super::set_painter_mode(PainterMode::Adjust);
        assert!(!paint_click(&mut cx, Some((0, ObjectRef::Wall(walls[0])))));
        assert!(super::super::spec_open());
        assert_eq!(super::super::spec_name().as_deref(), Some("Drywall"));
        super::super::close_spec();
        super::super::set_painter_mode(PainterMode::Off);
    }

    #[test]
    fn note_pick_finds_the_nearest_visible_mesh() {
        use plan_3d::Vertex;
        let tri = |z: f32, material, object| Mesh {
            vertices: [[0.0, 0.0], [10.0, 0.0], [0.0, 10.0]]
                .iter()
                .map(|p| Vertex {
                    position: [p[0], p[1], z],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                })
                .collect(),
            indices: vec![0, 1, 2],
            material,
            object_id: object,
            color: None,
        };
        let near = tri(5.0, Material::Floor, None);
        let far = tri(0.0, Material::WallInterior, Some(7));
        let meshes = [far, near];
        note_pick(&meshes, [2.0, 2.0, 20.0], [0.0, 0.0, -1.0], |_| true);
        let d = last_pick().unwrap();
        assert_eq!((d.material, d.object), (Material::Floor, None));
        assert!((d.point[2] - 5.0).abs() < 1e-4);
        // Hidden meshes are skipped; a miss records nothing.
        note_pick(&meshes, [2.0, 2.0, 20.0], [0.0, 0.0, -1.0], |m| {
            m.material != Material::Floor
        });
        assert_eq!(last_pick().unwrap().object, Some(7));
        note_pick(&meshes, [50.0, 50.0, 20.0], [0.0, 0.0, -1.0], |_| true);
        assert_eq!(last_pick(), None);
    }
}
