//! Moving objects in the 3D view (C-43): with the Select tool, pressing on a
//! selected cabinet, symbol, device, detail or stair and dragging slides it
//! along the floor plane under the pointer.
//!
//! * The pointer ray meets the horizontal plane at the object's base height
//!   ([`floor_point_at`]); the move is the difference between that point and
//!   the one under the press ([`floor_move`], a pure function of the camera
//!   and the two pixels).
//! * The result goes through the same snapping the Select tool uses in the
//!   plan: cabinets settle against walls and neighbours, symbols and stairs
//!   snap to the grid, everything else moves by whole snap units; Alt
//!   suspends the snap. Shift holds the move to the object's own axis (or
//!   the one across it).
//! * The whole drag is one undo step, "Move Objects"; a drag that ends where
//!   it began leaves none. Escape puts everything back.
//! * Arrow keys nudge the selection by one snap unit (ten with Shift) along
//!   the plan axis nearest the screen direction ([`nudge_dir`]).
//!
//! Walls, openings and the other kinds the plan moves with their own rules
//! are not dragged here ([`movable`]): a press on them orbits as before.

use super::pick::pick_ray_at;
use crate::editor::handles::HandleKind;
use crate::editor::stairs_view::{self, StairHandleKind};
use crate::editor::{placed, transform, EditorContext, ObjectRef};
use crate::tools::PointerEvent;
use eframe::egui;
use plan_core::geometry::Point;
use plan_core::{Id, Project};
use plan_view3d::math::Vec3;
use plan_view3d::Camera;

/// Pixels the pointer must travel before a press becomes a drag.
pub const DRAG_START_PX: f32 = 4.0;
/// Undo label of a drag or a nudge of objects in the 3D view.
pub const LABEL: &str = "Move Objects";

/// Can a drag in the 3D view move this kind of object?
pub fn movable(obj: ObjectRef) -> bool {
    matches!(
        obj,
        ObjectRef::Cabinet(_)
            | ObjectRef::Symbol(_)
            | ObjectRef::Device(_)
            | ObjectRef::Detail(_)
            | ObjectRef::Stair(_)
    )
}

/// The plan point (x, y) where the ray through `pos` meets the horizontal
/// plane at scene height `plane_y` (inches); `None` when the ray runs level
/// or the plane is behind the eye. Scene z is the negated plan y.
pub fn floor_point_at(
    cam: &Camera,
    rect: egui::Rect,
    pos: egui::Pos2,
    plane_y: f32,
) -> Option<Point> {
    let (origin, dir) = pick_ray_at(cam, rect, pos);
    plane_hit(origin, dir, plane_y)
}

fn plane_hit(origin: Vec3, dir: Vec3, plane_y: f32) -> Option<Point> {
    if dir[1].abs() < 1e-4 {
        return None;
    }
    let t = (plane_y - origin[1]) / dir[1];
    if !t.is_finite() || t < 0.0 {
        return None;
    }
    let x = origin[0] + dir[0] * t;
    let z = origin[2] + dir[2] * t;
    Some(Point::new(f64::from(x), -f64::from(z)))
}

/// How far (plan inches) an object on the plane at `plane_y` moves when the
/// pointer goes from pixel `from` to pixel `to` of the viewport `rect`.
pub fn floor_move(
    cam: &Camera,
    rect: egui::Rect,
    from: egui::Pos2,
    to: egui::Pos2,
    plane_y: f32,
) -> Option<Point> {
    let a = floor_point_at(cam, rect, from, plane_y)?;
    let b = floor_point_at(cam, rect, to, plane_y)?;
    Some(b - a)
}

/// `d` held to the axis `axis` (unit) or the one across it, whichever it
/// follows more.
pub fn constrain_to_axis(d: Point, axis: Point) -> Point {
    let across = Point::new(-axis.y, axis.x);
    let (along, over) = (d.dot(axis), d.dot(across));
    if along.abs() >= over.abs() {
        axis * along
    } else {
        across * over
    }
}

/// The plan axis (a unit vector) a key press nudges along: the one nearest
/// the screen direction of the arrow, seen from `cam`.
pub fn nudge_dir(cam: &Camera, key: egui::Key) -> Option<Point> {
    // The view direction on the floor in plan axes; looking (nearly) straight
    // down, the screen's up direction stands in for it.
    let flat = |v: Vec3| Point::new(f64::from(v[0]), -f64::from(v[2]));
    let mut ahead = flat(cam.forward());
    if ahead.length() < 0.25 {
        let v = cam.view_matrix();
        ahead = flat([v[1], v[5], v[9]]);
    }
    if ahead.length() < 1e-6 {
        return None;
    }
    let ahead = nearest_axis(ahead);
    let right = Point::new(ahead.y, -ahead.x);
    match key {
        egui::Key::ArrowUp => Some(ahead),
        egui::Key::ArrowDown => Some(ahead * -1.0),
        egui::Key::ArrowRight => Some(right),
        egui::Key::ArrowLeft => Some(right * -1.0),
        _ => None,
    }
}

fn nearest_axis(v: Point) -> Point {
    if v.x.abs() >= v.y.abs() {
        Point::new(v.x.signum(), 0.0)
    } else {
        Point::new(0.0, v.y.signum())
    }
}

/// The object's own axis in the plan, for Shift.
fn axis_of(project: &Project, floor: usize, obj: ObjectRef) -> Point {
    let f = &project.floors[floor];
    match obj {
        ObjectRef::Cabinet(id) => placed::cabinet_by_id(f, id).map_or(Point::new(1.0, 0.0), |c| {
            Point::new(c.angle.cos(), c.angle.sin())
        }),
        ObjectRef::Symbol(id) => f
            .symbol(id)
            .map_or(Point::new(1.0, 0.0), |s| placed::symbol_axes(s).0),
        ObjectRef::Stair(id) => {
            stairs_view::find(f, id).map_or(Point::new(1.0, 0.0), |s| s.along())
        }
        _ => Point::new(1.0, 0.0),
    }
}

/// The height of the lowest point of the meshes of `ids` in `scene`: the
/// plane the drag slides on. `None` when none of them is drawn.
pub fn base_height(scene: &plan_3d::Scene, ids: &[Id]) -> Option<f32> {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id.is_some_and(|i| ids.contains(&i)))
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .reduce(f32::min)
}

/// A press that may become a drag.
pub struct ObjDrag {
    /// The project as it was at the press.
    original: Project,
    /// The selected objects the drag moves.
    items: Vec<ObjectRef>,
    /// Plan point under the press, and the plane it was found on.
    start: Point,
    plane_y: f32,
    start_px: egui::Pos2,
    axis: Point,
    /// The pointer has gone far enough: the undo step is open.
    pub active: bool,
}

impl ObjDrag {
    /// A drag armed by a press at `start_px` on `pressed`, moving the movable
    /// objects of the selection; `None` when `pressed` is not among them or
    /// the press does not land on the plane.
    pub fn arm(
        cx: &EditorContext,
        pressed: ObjectRef,
        cam: &Camera,
        rect: egui::Rect,
        start_px: egui::Pos2,
        scene: &plan_3d::Scene,
    ) -> Option<ObjDrag> {
        if !movable(pressed) || !cx.selection.items.contains(&pressed) {
            return None;
        }
        let items: Vec<ObjectRef> = cx
            .selection
            .items
            .iter()
            .copied()
            .filter(|o| movable(*o))
            .collect();
        let ids: Vec<Id> = items.iter().map(|o| o.id()).collect();
        let plane_y = base_height(scene, &ids)?;
        let start = floor_point_at(cam, rect, start_px, plane_y)?;
        Some(ObjDrag {
            original: cx.project.clone(),
            axis: axis_of(&cx.project, cx.floor, pressed),
            items,
            start,
            plane_y,
            start_px,
            active: false,
        })
    }

    /// The pointer is at `px`: moves the objects (opening the undo step the
    /// first time the pointer has gone far enough). Returns whether the
    /// drag is under way.
    pub fn update(
        &mut self,
        cx: &mut EditorContext,
        cam: &Camera,
        rect: egui::Rect,
        px: egui::Pos2,
        (shift, alt): (bool, bool),
    ) -> bool {
        if !self.active {
            if (px - self.start_px).length() < DRAG_START_PX {
                return false;
            }
            self.active = true;
            cx.begin_change(LABEL);
        }
        let Some(mut total) = floor_move(cam, rect, self.start_px, px, self.plane_y) else {
            return true;
        };
        if shift {
            total = constrain_to_axis(total, self.axis);
        }
        cx.project = self.original.clone();
        move_items(cx, &self.items, self.start, total, alt);
        true
    }

    /// The button went up. Keeps the result as one undo step, or drops the
    /// step when nothing moved. Returns whether anything moved.
    pub fn finish(self, cx: &mut EditorContext) -> bool {
        if !self.active {
            return false;
        }
        if self.original.to_json().ok() == cx.project.to_json().ok() {
            cx.cancel_change();
            return false;
        }
        cx.status = format!("Moved {}", describe(&self.items));
        true
    }

    /// Escape: everything goes back where it was.
    pub fn cancel(self, cx: &mut EditorContext) {
        if self.active {
            cx.project = self.original;
            cx.cancel_change();
            cx.mark_dirty();
        }
    }
}

fn describe(items: &[ObjectRef]) -> String {
    match items {
        [one] => match one {
            ObjectRef::Cabinet(_) => "the cabinet".into(),
            ObjectRef::Symbol(_) => "the symbol".into(),
            ObjectRef::Stair(_) => "the stairs".into(),
            ObjectRef::Device(_) => "the device".into(),
            _ => "the object".into(),
        },
        many => format!("{} objects", many.len()),
    }
}

/// Moves `items` as if the pointer had gone `total` from `start` in the
/// plan, from the project as it stands (the caller restored it). One cabinet,
/// symbol or stair settles the way the Select tool settles it; anything else
/// moves by whole snap units.
fn move_items(cx: &mut EditorContext, items: &[ObjectRef], start: Point, total: Point, alt: bool) {
    let fl = cx.floor;
    let unit = cx.snap_unit();
    let mut ev = PointerEvent::at(cx, start + total);
    ev.modifiers.alt = alt;
    match items {
        [ObjectRef::Cabinet(id)] => {
            if let Some(orig) = placed::cabinet_by_id(&cx.project.floors[fl], *id) {
                let c = crate::tools::cabinet::apply_edit(cx, HandleKind::Move, &orig, start, &ev);
                placed::replace_cabinet(&mut cx.project, fl, &c);
                placed::rejoin_if_enabled(cx);
            }
        }
        [ObjectRef::Symbol(id)] => {
            if let Some(orig) = cx.project.floors[fl].symbol(*id).cloned() {
                let s = crate::tools::library::apply_drag(cx, HandleKind::Move, &orig, start, &ev);
                if let Some(slot) = cx.project.floors[fl]
                    .symbols
                    .iter_mut()
                    .find(|x| x.id == *id)
                {
                    *slot = s;
                }
                placed::sync_distributions(cx);
            }
        }
        [ObjectRef::Stair(id)] => {
            if let Some(orig) = stairs_view::find(&cx.project.floors[fl], *id) {
                let mut n =
                    stairs_view::drag_handle(&orig, StairHandleKind::Move, start, start + total);
                if !alt {
                    n.stair.origin = crate::editor::snap::snap_to_grid(n.stair.origin, unit);
                }
                stairs_view::update(&mut cx.project, fl, *id, |o| *o = n);
            }
        }
        _ => {
            let d = if alt {
                total
            } else {
                Point::new(round_to(total.x, unit), round_to(total.y, unit))
            };
            translate(cx, items, d);
        }
    }
    cx.mark_dirty();
}

fn round_to(v: f64, unit: f64) -> f64 {
    if unit > 0.0 {
        (v / unit).round() * unit
    } else {
        v
    }
}

/// Moves `items` by `d` the way a group drag in the plan does.
fn translate(cx: &mut EditorContext, items: &[ObjectRef], d: Point) {
    transform::translate_objects(cx, items, d);
    placed::sync_distributions(cx);
    placed::rejoin_if_enabled(cx);
}

/// Nudges the selection by `d` plan inches as one undo step (arrow keys in
/// the 3D view). Returns whether anything was nudged.
pub fn nudge(cx: &mut EditorContext, d: Point) -> bool {
    let items = cx.selection.items.clone();
    if items.is_empty() || items.iter().any(|o| !cx.check_unlocked(*o)) {
        return false;
    }
    let before = cx.project.clone();
    cx.begin_change(LABEL);
    translate(cx, &items, d);
    cx.mark_dirty();
    if before.to_json().ok() == cx.project.to_json().ok() {
        cx.cancel_change();
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_view3d::CameraMode;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(40.0, 30.0), egui::vec2(800.0, 600.0))
    }

    fn camera() -> Camera {
        let mut cam = Camera::default();
        cam.fit_to_bounds([0.0, 0.0, -300.0], [400.0, 100.0, 0.0]);
        cam
    }

    /// The pixel a scene point lands on in `rect()`.
    fn pixel_of(cam: &Camera, p: Vec3) -> egui::Pos2 {
        let ndc = plan_view3d::math::transform_point(&cam.view_projection(800.0 / 600.0), p);
        egui::pos2(
            rect().min.x + (ndc[0] + 1.0) * 0.5 * 800.0,
            rect().min.y + (1.0 - ndc[1]) * 0.5 * 600.0,
        )
    }

    #[test]
    fn a_screen_delta_maps_to_a_floor_move() {
        let cam = camera();
        // Two points on the floor (scene y = 0): plan (100, 50) and (160, 90).
        let a = pixel_of(&cam, [100.0, 0.0, -50.0]);
        let b = pixel_of(&cam, [160.0, 0.0, -90.0]);
        let d = floor_move(&cam, rect(), a, b, 0.0).unwrap();
        assert!(
            (d.x - 60.0).abs() < 0.05 && (d.y - 40.0).abs() < 0.05,
            "{d:?}"
        );
        // The same pixels on a plane 30" up give a different move: the ray
        // meets that plane nearer the eye.
        let up = floor_move(&cam, rect(), a, b, 30.0).unwrap();
        assert!((up.x - d.x).abs() > 1.0 || (up.y - d.y).abs() > 1.0);
        // No move, no delta.
        let none = floor_move(&cam, rect(), a, a, 0.0).unwrap();
        assert!(none.length() < 1e-6);
        // A level ray (an elevation) never meets the floor.
        let mut front = Camera::default();
        front.fit_to_bounds([0.0, 0.0, -300.0], [400.0, 100.0, 0.0]);
        front.set_mode(CameraMode::ElevationFront);
        assert!(floor_point_at(&front, rect(), rect().center(), 0.0).is_none());
        // A plane above the eye is behind the ray.
        assert!(floor_point_at(&cam, rect(), rect().center(), 1.0e6).is_none());
    }

    #[test]
    fn shift_holds_a_move_to_the_object_axis_or_across_it() {
        let axis = Point::new(1.0, 0.0);
        let d = constrain_to_axis(Point::new(30.0, 8.0), axis);
        assert_eq!((d.x, d.y), (30.0, 0.0));
        let d = constrain_to_axis(Point::new(5.0, -20.0), axis);
        assert_eq!((d.x, d.y), (0.0, -20.0));
        // A turned object: its own axis is 45 degrees.
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let d = constrain_to_axis(Point::new(10.0, 12.0), Point::new(s, s));
        assert!((d.x - d.y).abs() < 1e-9, "{d:?}");
    }

    #[test]
    fn arrows_nudge_along_the_axis_nearest_the_screen_direction() {
        // The default overview looks from +Z toward -Z (plan +y is away).
        let mut cam = Camera {
            yaw: 0.0,
            ..camera()
        };
        let up = nudge_dir(&cam, egui::Key::ArrowUp).unwrap();
        assert_eq!((up.x, up.y), (0.0, 1.0));
        let right = nudge_dir(&cam, egui::Key::ArrowRight).unwrap();
        assert_eq!((right.x, right.y), (1.0, 0.0));
        let left = nudge_dir(&cam, egui::Key::ArrowLeft).unwrap();
        assert_eq!((left.x, left.y), (-1.0, 0.0));
        // Looking from the right-hand side (+X toward -X), up is plan -x.
        cam.yaw = std::f32::consts::FRAC_PI_2;
        let up = nudge_dir(&cam, egui::Key::ArrowUp).unwrap();
        assert_eq!((up.x, up.y), (-1.0, 0.0));
        let right = nudge_dir(&cam, egui::Key::ArrowRight).unwrap();
        assert_eq!((right.x, right.y), (0.0, 1.0));
        // Straight down (Plan Overhead) uses the screen's up.
        let mut top = camera();
        top.set_mode(CameraMode::PlanOverhead);
        let up = nudge_dir(&top, egui::Key::ArrowUp).unwrap();
        assert_eq!((up.x, up.y), (0.0, 1.0));
        assert!(nudge_dir(&cam, egui::Key::Enter).is_none());
    }

    #[test]
    fn only_the_objects_the_plan_slides_freely_are_dragged() {
        assert!(movable(ObjectRef::Cabinet(1)));
        assert!(movable(ObjectRef::Symbol(1)));
        assert!(movable(ObjectRef::Stair(1)));
        assert!(movable(ObjectRef::Device(1)));
        assert!(movable(ObjectRef::Detail(1)));
        assert!(!movable(ObjectRef::Wall(1)));
        assert!(!movable(ObjectRef::Opening(1)));
        assert!(!movable(ObjectRef::Room(0)));
    }

    #[test]
    fn the_drag_plane_is_the_lowest_point_of_the_objects() {
        use plan_3d::{Material, Mesh, Scene, Vertex};
        let v = |y: f32| Vertex {
            position: [0.0, y, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        };
        let mesh = |id: Id, ys: [f32; 3]| Mesh {
            vertices: ys.iter().map(|y| v(*y)).collect(),
            indices: vec![0, 1, 2],
            material: Material::Trim,
            object_id: Some(id),
            color: None,
        };
        let scene = Scene {
            meshes: vec![mesh(1, [4.0, 30.0, 12.0]), mesh(2, [-2.0, 9.0, 9.0])],
        };
        assert_eq!(base_height(&scene, &[1]), Some(4.0));
        assert_eq!(base_height(&scene, &[1, 2]), Some(-2.0));
        assert_eq!(base_height(&scene, &[3]), None);
    }
}
