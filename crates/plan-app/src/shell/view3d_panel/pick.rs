//! Picking and the live overlay of the 3D view (C-43).
//!
//! * **Picking**: a click ray-casts the cached scene (and the pictures) and
//!   selects the object whose mesh the nearest triangle belongs to
//!   ([`nearest_hit`], [`pick_ray`], `selection::pick_for_mesh_id`). The
//!   selection is the editor's own `cx.selection`, so the plan shows it when
//!   the user goes back.
//! * **Overlay**: what must follow the camera, the selection or the pointer
//!   without rebuilding the cached model: pictures (billboards turn to face
//!   the eye, `placed::image_meshes_facing` semantics), a translucent tint
//!   over the selected objects and a lighter one over the object under the
//!   pointer ([`overlay_meshes`]). The panel hands them to
//!   `Viewport3d::set_overlay`, which draws them after the cached scene and
//!   leaves that scene (and its shadow map) alone, whenever [`overlay_key`]
//!   changes (selection, hover, picture geometry at half-inch resolution),
//!   not on every frame.
//! * **Hover**: the object under the pointer is re-picked only when the
//!   pointer has moved and the last pick is old enough ([`hover_due`]).

use super::{apply_fill, clip_scene, ViewScope};
use crate::editor::selection::{pick_for_mesh_id, ObjectRef, Selection};
use crate::editor::{rooms_edit, EditorContext, EditorRequest};
use eframe::egui;
use plan_3d::images::image_mesh;
use plan_3d::{Material, Mesh, Scene};
use plan_core::{Id, Project};
use plan_view3d::math::{self, Vec3};
use plan_view3d::Camera;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// How far the selection tint stands off the surface it covers, inches.
const TINT_OFFSET: f32 = 0.4;
/// The tint over the object under the pointer: a light blue (the selection
/// tint is the orange of its material).
pub const HOVER_RGB: [u8; 3] = [150, 200, 255];
/// The pointer must move this far (pixels) before the hover is picked again...
pub const HOVER_MIN_MOVE_PX: f32 = 3.0;
/// ...and this long (seconds) after the last pick.
pub const HOVER_MIN_INTERVAL_S: f64 = 0.05;
/// Triangles flatter than this to the ray are skipped (determinant).
const PARALLEL_EPS: f32 = 1e-9;

// ----- rays -----

/// The ray through the screen point `(nx, ny)` (each -1..1, +y up) of a
/// viewport of `aspect` (width/height): its origin and unit direction in scene
/// axes. Perspective rays start at the eye; orthographic ones start on the
/// view plane pushed back by the clip distance (the viewport's near plane is
/// negative, so geometry behind the eye plane is visible and pickable).
pub fn pick_ray(cam: &Camera, aspect: f32, nx: f32, ny: f32) -> (Vec3, Vec3) {
    let v = cam.view_matrix();
    // The rotation part of the (column-major) view matrix: its rows are the
    // camera's right, up and backward axes in world space.
    let right = [v[0], v[4], v[8]];
    let up = [v[1], v[5], v[9]];
    let fwd = cam.forward();
    let eye = cam.eye();
    let aspect = if aspect.is_finite() && aspect > 1e-3 {
        aspect
    } else {
        1.0
    };
    if cam.mode.is_orthographic() {
        let h = cam.ortho_half_height.max(1e-3);
        let on_plane = math::add(
            eye,
            math::add(math::scale(right, nx * h * aspect), math::scale(up, ny * h)),
        );
        (math::sub(on_plane, math::scale(fwd, cam.distance)), fwd)
    } else {
        let half_h = (cam.fov_deg.to_radians() * 0.5).tan();
        let dir = math::add(
            fwd,
            math::add(
                math::scale(right, nx * aspect * half_h),
                math::scale(up, ny * half_h),
            ),
        );
        (eye, math::normalize(dir))
    }
}

/// The ray of the pixel `pos` inside the viewport `rect`.
pub fn pick_ray_at(cam: &Camera, rect: egui::Rect, pos: egui::Pos2) -> (Vec3, Vec3) {
    let (w, h) = (rect.width().max(1.0), rect.height().max(1.0));
    let nx = (pos.x - rect.min.x) / w * 2.0 - 1.0;
    let ny = 1.0 - (pos.y - rect.min.y) / h * 2.0;
    pick_ray(cam, w / h, nx, ny)
}

/// Distance along the ray to the triangle (Moller-Trumbore, two-sided).
fn ray_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = math::sub(b, a);
    let e2 = math::sub(c, a);
    let p = math::cross(dir, e2);
    let det = math::dot(e1, p);
    if det.abs() < PARALLEL_EPS {
        return None;
    }
    let inv = 1.0 / det;
    let s = math::sub(origin, a);
    let u = math::dot(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = math::cross(s, e1);
    let v = math::dot(dir, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = math::dot(e2, q) * inv;
    (t >= 0.0).then_some(t)
}

/// The nearest triangle hit by the ray among `meshes` that `visible` lets
/// through: its distance and its mesh's `object_id`.
pub fn nearest_hit<'a>(
    meshes: impl IntoIterator<Item = &'a Mesh>,
    origin: Vec3,
    dir: Vec3,
    visible: impl Fn(&Mesh) -> bool,
) -> Option<(f32, Option<Id>)> {
    let mut best: Option<(f32, Option<Id>)> = None;
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
                if best.is_none_or(|(bt, _)| t < bt) {
                    best = Some((t, m.object_id));
                }
            }
        }
    }
    best
}

/// Does the viewport draw this mesh? Ceilings and roofs are hidden in the
/// views that look into the model.
fn is_drawn(m: &Mesh, hide_ceiling_roof: bool) -> bool {
    !(hide_ceiling_roof && matches!(m.material, Material::Ceiling | Material::Roof))
}

/// The scene point on the nearest visible surface under `pos` (any surface,
/// object or not): where Alt-click puts the orbit centre.
pub fn surface_point(
    cam: &Camera,
    rect: egui::Rect,
    pos: egui::Pos2,
    base: &Scene,
    pictures: &[Mesh],
) -> Option<Vec3> {
    let (origin, dir) = pick_ray_at(cam, rect, pos);
    let hide = cam.mode.hides_ceiling_and_roof();
    let (t, _) = nearest_hit(base.meshes.iter().chain(pictures), origin, dir, |m| {
        is_drawn(m, hide)
    })?;
    Some(math::add(origin, math::scale(dir, t)))
}

/// Is it time to pick the object under a hovering pointer again? `last` is
/// where and when (seconds) the previous pick ran. A new pick needs the
/// pointer to have moved [`HOVER_MIN_MOVE_PX`] and [`HOVER_MIN_INTERVAL_S`]
/// to have passed, so a still pointer costs nothing and a fast one a few
/// picks a second.
pub fn hover_due(last: Option<(egui::Pos2, f64)>, pos: egui::Pos2, now: f64) -> bool {
    match last {
        None => true,
        Some((p, t)) => (pos - p).length() >= HOVER_MIN_MOVE_PX && now - t >= HOVER_MIN_INTERVAL_S,
    }
}

/// The object id a hover tints for a pick result: objects that have meshes.
pub fn hover_id(hit: Option<(usize, ObjectRef)>) -> Option<Id> {
    let (_, obj) = hit?;
    (!matches!(obj, ObjectRef::Room(_) | ObjectRef::Terrain) && obj.id() != 0).then(|| obj.id())
}

/// What a click at `pos` selects: the object of the nearest visible triangle
/// of `base` and the pictures, with the floor it is on. `None` for the sky
/// and for surfaces that are no object (slabs, the ground).
pub fn object_under(
    project: &Project,
    floor: usize,
    cam: &Camera,
    rect: egui::Rect,
    pos: egui::Pos2,
    base: &Scene,
    pictures: &[Mesh],
) -> Option<(usize, ObjectRef)> {
    let (origin, dir) = pick_ray_at(cam, rect, pos);
    let hide = cam.mode.hides_ceiling_and_roof();
    let (_, id) = nearest_hit(base.meshes.iter().chain(pictures), origin, dir, |m| {
        is_drawn(m, hide)
    })?;
    pick_for_mesh_id(project, floor, id?)
}

/// Applies a pick to the editor: the object becomes the selection (Shift
/// toggles it in the selection), on its floor; a miss clears the selection
/// unless Shift is down.
pub fn apply_pick(cx: &mut EditorContext, hit: Option<(usize, ObjectRef)>, add: bool) {
    // With the Material Painter on a click paints (or samples, or erases) the
    // object under it instead of selecting it.
    if crate::tools::materials::painter_active() {
        if let Some((floor, obj)) = hit {
            crate::tools::materials::paint_object(cx, floor, obj);
        }
        return;
    }
    match hit {
        Some((floor, obj)) => {
            if floor != cx.floor && floor < cx.project.floors.len() {
                cx.floor = floor;
                cx.mark_dirty();
            }
            rooms_edit::clear_room_selection();
            if add {
                cx.selection.toggle(obj);
            } else {
                cx.selection.set(obj);
            }
        }
        None if !add => {
            cx.selection.clear();
            rooms_edit::clear_room_selection();
        }
        None => {}
    }
}

/// Double-click: selects like a click, then asks for the specification (the
/// same `OpenSpec` request the Select tool makes).
pub fn apply_open(cx: &mut EditorContext, hit: Option<(usize, ObjectRef)>) {
    apply_pick(cx, hit, false);
    if let Some((_, obj)) = hit {
        cx.requests.push(EditorRequest::OpenSpec(obj));
    }
}

// ----- the overlay -----

/// Every picture of the floors a view shows, billboards turned to face `eye`
/// (scene axes), with the view's section cut and technique override applied
/// the way they are to the cached scene.
pub fn picture_meshes(project: &Project, scope: &ViewScope, eye: Vec3) -> Vec<Mesh> {
    let floors = scope
        .floor
        .map_or(project.floors.len(), |f| (f + 1).min(project.floors.len()));
    let mut meshes = Vec::new();
    for f in project.floors.iter().take(floors) {
        for s in &f.symbols {
            meshes.extend(image_mesh(s, f.elevation, Some(eye)));
        }
    }
    let mut scene = Scene { meshes };
    if let Some(cut) = &scope.section {
        scene = clip_scene(&scene, cut);
    }
    if let Some(m) = scope.fill {
        apply_fill(&mut scene, m);
    }
    scene.meshes
}

/// The mesh ids a selection tints.
fn selected_ids(selection: &Selection) -> Vec<Id> {
    let mut ids: Vec<Id> = selection
        .items
        .iter()
        .filter(|o| !matches!(o, ObjectRef::Room(_) | ObjectRef::Terrain))
        .map(|o| o.id())
        .filter(|id| *id != 0)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// A copy of `m` in the selection tint (or `color` over it), standing
/// `TINT_OFFSET` off its surface.
fn tint(m: &Mesh, color: Option<[u8; 3]>) -> Mesh {
    let mut t = m.clone();
    t.material = Material::Selection;
    t.color = color;
    for v in &mut t.vertices {
        for k in 0..3 {
            v.position[k] += v.normal[k] * TINT_OFFSET;
        }
    }
    t
}

/// The overlay: `pictures` followed by the selection tint over the selected
/// objects' meshes in `base` and `pictures`.
#[cfg(test)]
pub fn overlay_meshes(
    base: &Scene,
    pictures: Vec<Mesh>,
    selection: &Selection,
    hide_ceiling_roof: bool,
) -> Vec<Mesh> {
    overlay_with_hover(base, pictures, selection, None, hide_ceiling_roof)
}

/// [`overlay_meshes`] with the light hover tint over the object `hover` as
/// well (nothing extra when it is selected: the selection tint is there).
pub fn overlay_with_hover(
    base: &Scene,
    pictures: Vec<Mesh>,
    selection: &Selection,
    hover: Option<Id>,
    hide_ceiling_roof: bool,
) -> Vec<Mesh> {
    let ids = selected_ids(selection);
    let hover = hover.filter(|h| !ids.contains(h));
    let tints: Vec<Mesh> = if ids.is_empty() && hover.is_none() {
        Vec::new()
    } else {
        base.meshes
            .iter()
            .chain(&pictures)
            .filter(|m| is_drawn(m, hide_ceiling_roof))
            .filter_map(|m| {
                let id = m.object_id?;
                if ids.contains(&id) {
                    Some(tint(m, None))
                } else if hover == Some(id) {
                    Some(tint(m, Some(HOVER_RGB)))
                } else {
                    None
                }
            })
            .collect()
    };
    let mut out = pictures;
    out.extend(tints);
    out
}

/// Identifies an overlay by what it is made of: the selection and the
/// `pictures`' geometry (to half an inch). The panel re-uploads only when it
/// changes.
pub fn overlay_key(selection: &Selection, pictures: &[Mesh], hide_ceiling_roof: bool) -> u64 {
    overlay_key_with_hover(selection, None, pictures, hide_ceiling_roof)
}

/// [`overlay_key`] for an overlay that also tints the hovered object.
pub fn overlay_key_with_hover(
    selection: &Selection,
    hover: Option<Id>,
    pictures: &[Mesh],
    hide_ceiling_roof: bool,
) -> u64 {
    let mut h = DefaultHasher::new();
    selected_ids(selection).hash(&mut h);
    hover.hash(&mut h);
    hide_ceiling_roof.hash(&mut h);
    pictures.len().hash(&mut h);
    for m in pictures {
        m.object_id.hash(&mut h);
        (m.material as u8).hash(&mut h);
        for v in &m.vertices {
            for c in v.position {
                ((c * 2.0).round() as i32).hash(&mut h);
            }
        }
    }
    h.finish()
}

/// The key of "no overlay at all", what the cached scene alone corresponds to.
pub fn empty_overlay_key() -> u64 {
    overlay_key(&Selection::default(), &[], false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{EditorContext, EditorRequest};
    use crate::plan_defaults;
    use plan_3d::Vertex;
    use plan_core::geometry::Point;
    use plan_core::{ImageSpec, OpeningKind, PlacedSymbol, WallKind};
    use plan_view3d::{CameraMode, Viewport3d};

    const RECT_W: f32 = 800.0;
    const RECT_H: f32 = 600.0;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(40.0, 30.0), egui::vec2(RECT_W, RECT_H))
    }

    fn one_wall() -> (Project, Id) {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        (p, w)
    }

    fn scene_of(p: &Project) -> Scene {
        super::super::build_view_scene(
            p,
            &ViewScope {
                no_images: true,
                ..ViewScope::default()
            },
        )
    }

    /// The pixel a scene point lands on.
    fn pixel_of(cam: &Camera, p: Vec3) -> egui::Pos2 {
        let ndc = math::transform_point(&cam.view_projection(RECT_W / RECT_H), p);
        let r = rect();
        egui::pos2(
            r.min.x + (ndc[0] + 1.0) * 0.5 * RECT_W,
            r.min.y + (1.0 - ndc[1]) * 0.5 * RECT_H,
        )
    }

    fn camera_for(mode: CameraMode, scene: &Scene) -> Camera {
        let mut vp = Viewport3d::new();
        vp.queue_scene(scene);
        vp.set_mode(mode);
        vp.camera.clone()
    }

    #[test]
    fn a_ray_through_a_wall_mesh_selects_that_wall_in_every_view() {
        let (p, wall) = one_wall();
        let scene = scene_of(&p);
        // A point on the south face of the wall (scene z = -plan y).
        let on_wall: Vec3 = [120.0, 48.0, 3.0];
        for mode in [
            CameraMode::Orbit,
            CameraMode::DollHouse,
            CameraMode::ElevationFront,
        ] {
            let cam = camera_for(mode, &scene);
            let at = pixel_of(&cam, on_wall);
            let hit = object_under(&p, 0, &cam, rect(), at, &scene, &[]);
            assert_eq!(hit, Some((0, ObjectRef::Wall(wall))), "{mode:?}");
        }
        // Inside, looking at the wall.
        let mut cam = camera_for(CameraMode::FullCamera, &scene);
        cam.position = [120.0, 60.0, 100.0];
        cam.yaw = 0.0;
        cam.pitch = 0.0;
        let centre = egui::pos2(rect().center().x, rect().center().y);
        assert_eq!(
            object_under(&p, 0, &cam, rect(), centre, &scene, &[]),
            Some((0, ObjectRef::Wall(wall)))
        );
        // The ray itself starts at the eye and runs through the middle.
        let (origin, dir) = pick_ray_at(&cam, rect(), centre);
        assert_eq!(origin, [120.0, 60.0, 100.0]);
        assert!(dir[2] < -0.999 && dir[0].abs() < 1e-4, "{dir:?}");
    }

    #[test]
    fn the_nearest_object_wins_and_the_sky_selects_nothing() {
        let mut p = Project::new("t");
        let near = p.add_wall(
            0,
            Point::new(0.0, -100.0),
            Point::new(240.0, -100.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let _far = p.add_wall(
            0,
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let scene = scene_of(&p);
        let mut cam = camera_for(CameraMode::FullCamera, &scene);
        cam.position = [120.0, 48.0, 400.0];
        cam.yaw = 0.0;
        cam.pitch = 0.0;
        // Straight ahead the wall at plan y = -100 hides the one behind it.
        assert_eq!(
            object_under(&p, 0, &cam, rect(), rect().center(), &scene, &[]),
            Some((0, ObjectRef::Wall(near)))
        );
        // The top-left corner of the screen looks over both walls.
        assert_eq!(
            object_under(
                &p,
                0,
                &cam,
                rect(),
                rect().min + egui::vec2(2.0, 2.0),
                &scene,
                &[]
            ),
            None
        );
    }

    #[test]
    fn surfaces_without_an_object_block_the_view_but_select_nothing() {
        let (p, wall) = one_wall();
        let mut scene = Scene::default();
        scene.meshes.push(quad(Material::Floor, None, 10.0, 0.0));
        scene
            .meshes
            .push(quad(Material::Trim, Some(wall), 10.0, -50.0));
        let mut cam = Camera {
            mode: CameraMode::FullCamera,
            position: [0.0, 100.0, 0.0],
            yaw: 0.0,
            pitch: -std::f32::consts::FRAC_PI_2 + 0.01,
            ..Camera::default()
        };
        // Looking down at a floor slab that lies over the wall's mesh: the
        // slab is what is seen, and it is no object.
        let hit = object_under(&p, 0, &cam, rect(), rect().center(), &scene, &[]);
        assert_eq!(hit, None);
        scene.meshes.remove(0);
        cam.pitch = -std::f32::consts::FRAC_PI_2 + 0.01;
        let hit = object_under(&p, 0, &cam, rect(), rect().center(), &scene, &[]);
        assert_eq!(hit, Some((0, ObjectRef::Wall(wall))));
    }

    /// A 2x2 horizontal quad (two triangles) at height `y`.
    fn quad(material: Material, id: Option<Id>, half: f32, y: f32) -> Mesh {
        let v = |x: f32, z: f32| Vertex {
            position: [x, y, z],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        };
        Mesh {
            vertices: vec![
                v(-half, -half),
                v(half, -half),
                v(half, half),
                v(-half, half),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            material,
            object_id: id,
            color: None,
        }
    }

    #[test]
    fn ceilings_and_roofs_hidden_by_the_view_are_not_picked() {
        let (_, a) = one_wall();
        let b = a; // ids only need to resolve to an object
        let mut scene = Scene::default();
        scene
            .meshes
            .push(quad(Material::Ceiling, Some(a), 50.0, 100.0));
        scene.meshes.push(quad(Material::Trim, Some(b), 50.0, 0.0));
        let cam = Camera {
            mode: CameraMode::FullCamera,
            position: [0.0, 300.0, 0.0],
            pitch: -std::f32::consts::FRAC_PI_2 + 0.01,
            ..Camera::default()
        };
        // The ceiling shows (and is hit) from a Full Camera.
        let (origin, dir) = pick_ray_at(&cam, rect(), rect().center());
        let any = nearest_hit(&scene.meshes, origin, dir, |m| is_drawn(m, false)).unwrap();
        assert!(
            (any.0 - 200.0).abs() < 1.0,
            "the ceiling at 100\" is hit first: {any:?}"
        );
        // The Doll House hides it: the ray goes through to the floor.
        let hidden = nearest_hit(&scene.meshes, origin, dir, |m| is_drawn(m, true)).unwrap();
        assert!((hidden.0 - 300.0).abs() < 1.0, "{hidden:?}");
    }

    #[test]
    fn clicks_select_and_shift_clicks_add_and_a_miss_clears() {
        let (p, wall) = one_wall();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        cx.project
            .add_opening(0, wall, 120.0, OpeningKind::Door)
            .unwrap();
        let door = cx.project.floors[0].openings[0].id;
        apply_pick(&mut cx, Some((0, ObjectRef::Wall(wall))), false);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(wall)));
        apply_pick(&mut cx, Some((0, ObjectRef::Opening(door))), true);
        assert_eq!(cx.selection.len(), 2, "shift adds");
        apply_pick(&mut cx, Some((0, ObjectRef::Opening(door))), true);
        assert_eq!(
            cx.selection.single(),
            Some(ObjectRef::Wall(wall)),
            "shift toggles"
        );
        apply_pick(&mut cx, None, true);
        assert_eq!(cx.selection.len(), 1, "a shifted miss keeps the selection");
        apply_pick(&mut cx, None, false);
        assert!(cx.selection.is_empty());
    }

    #[test]
    fn a_click_on_another_floor_switches_floors_and_a_double_click_asks_for_the_spec() {
        let (p, w0) = one_wall();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        cx.project.build_new_floor(false);
        let w1 = cx.project.add_wall(
            1,
            Point::ZERO,
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        apply_pick(&mut cx, Some((1, ObjectRef::Wall(w1))), false);
        assert_eq!(cx.floor, 1);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(w1)));
        cx.requests.clear();
        apply_open(&mut cx, Some((0, ObjectRef::Wall(w0))));
        assert_eq!(cx.floor, 0);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(w0)));
        assert!(cx
            .requests
            .iter()
            .any(|r| matches!(r, EditorRequest::OpenSpec(ObjectRef::Wall(i)) if *i == w0)));
        // Nothing under a double-click asks for nothing.
        cx.requests.clear();
        apply_open(&mut cx, None);
        assert!(cx.requests.is_empty());
    }

    #[test]
    fn the_selection_is_tinted_in_the_view() {
        let (p, wall) = one_wall();
        let scene = scene_of(&p);
        let mut sel = Selection::default();
        assert!(overlay_meshes(&scene, Vec::new(), &sel, false).is_empty());
        assert_eq!(
            overlay_key(&sel, &[], false),
            empty_overlay_key(),
            "no selection, no pictures: the bare model"
        );
        sel.set(ObjectRef::Wall(wall));
        let overlay = overlay_meshes(&scene, Vec::new(), &sel, false);
        let walls: Vec<&Mesh> = scene
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(wall))
            .collect();
        assert!(!overlay.is_empty());
        assert_eq!(overlay.len(), walls.len());
        assert!(overlay.iter().all(|m| m.material == Material::Selection));
        assert!(overlay.iter().all(|m| m.object_id == Some(wall)));
        // Each tinted vertex stands off its wall along the normal.
        let (src, tinted) = (&walls[0].vertices[0], &overlay[0].vertices[0]);
        let moved = math::length(math::sub(tinted.position, src.position));
        assert!((moved - TINT_OFFSET).abs() < 1e-4, "{moved}");
        // The key follows the selection, not the frame.
        let k1 = overlay_key(&sel, &[], false);
        assert_ne!(k1, empty_overlay_key());
        assert_eq!(k1, overlay_key(&sel, &[], false));
        assert_ne!(
            k1,
            overlay_key(&sel, &[], true),
            "hidden ceilings change it"
        );
        sel.clear();
        assert_eq!(overlay_key(&sel, &[], false), empty_overlay_key());
        // Rooms and the terrain as a whole have no mesh to tint.
        sel.set(ObjectRef::Room(0));
        assert!(overlay_meshes(&scene, Vec::new(), &sel, false).is_empty());
        sel.set(ObjectRef::Terrain);
        assert!(overlay_meshes(&scene, Vec::new(), &sel, false).is_empty());
    }

    fn billboard_project() -> (Project, Id) {
        let mut p = Project::new("t");
        let mut spec = ImageSpec::new("tree.png", 10, 10);
        spec.color = [100, 120, 90];
        let id = p.add_symbol(
            0,
            PlacedSymbol::billboard(spec, Point::new(100.0, 100.0), 40.0, 72.0),
        );
        (p, id)
    }

    #[test]
    fn a_billboard_mesh_faces_the_eye_and_turns_when_the_eye_moves() {
        let (p, id) = billboard_project();
        let scope = ViewScope::default();
        let normal_to = |eye: Vec3| -> Vec3 {
            let m = picture_meshes(&p, &scope, eye);
            assert_eq!(m.len(), 1);
            assert_eq!(m[0].object_id, Some(id));
            m[0].vertices[0].normal
        };
        // The billboard stands on the middle of its footprint, near plan (100, 100).
        let facing = |eye: Vec3, n: Vec3| {
            let to = math::normalize([eye[0] - 100.0, 0.0, eye[2] + 100.0]);
            math::dot(n, to)
        };
        for eye in [
            [500.0, 60.0, -500.0],
            [-300.0, 60.0, -100.0],
            [100.0, 60.0, 600.0],
        ] {
            let n = normal_to(eye);
            assert_eq!(n[1], 0.0, "upright");
            assert!(facing(eye, n) > 0.999, "{eye:?} {n:?}");
        }
        // Two eyes on different sides give different overlays; a hair of
        // movement does not.
        let key = |eye: Vec3| {
            let o = picture_meshes(&p, &scope, eye);
            overlay_key(&Selection::default(), &o, false)
        };
        assert_ne!(key([500.0, 60.0, -500.0]), key([-300.0, 60.0, -100.0]));
        assert_eq!(key([500.0, 60.0, -500.0]), key([500.0, 60.0, -500.0]));
        assert_eq!(key([500.0, 60.0, -500.0]), key([500.001, 60.0, -500.0]));
    }

    #[test]
    fn pictures_follow_the_views_floors_and_technique() {
        let (mut p, _) = billboard_project();
        p.build_new_floor(false);
        let spec = ImageSpec::new("rug.png", 10, 10);
        p.add_symbol(
            1,
            PlacedSymbol::picture(spec, Point::new(0.0, 0.0), 40.0, 40.0),
        );
        let eye = [0.0, 60.0, 0.0];
        assert_eq!(picture_meshes(&p, &ViewScope::default(), eye).len(), 2);
        let first_floor = ViewScope {
            floor: Some(0),
            ..ViewScope::default()
        };
        assert_eq!(picture_meshes(&p, &first_floor, eye).len(), 1);
        let clay = ViewScope {
            fill: Some(Material::Ceiling),
            ..ViewScope::default()
        };
        assert!(picture_meshes(&p, &clay, eye)
            .iter()
            .all(|m| m.material == Material::Ceiling));
    }

    #[test]
    fn a_picture_can_be_clicked_and_is_tinted() {
        let (p, id) = billboard_project();
        let scope = ViewScope::default();
        let base = Scene::default();
        let cam = Camera {
            mode: CameraMode::FullCamera,
            position: [100.0, 36.0, 400.0],
            yaw: 0.0,
            pitch: 0.0,
            ..Camera::default()
        };
        let pictures = picture_meshes(&p, &scope, cam.eye());
        let at = pixel_of(&cam, [100.0, 36.0, -100.0]);
        assert_eq!(
            object_under(&p, 0, &cam, rect(), at, &base, &pictures),
            Some((0, ObjectRef::Symbol(id)))
        );
        let mut sel = Selection::default();
        sel.set(ObjectRef::Symbol(id));
        let overlay = overlay_meshes(&base, pictures, &sel, false);
        assert_eq!(overlay.len(), 2, "the picture and its tint");
        assert_eq!(overlay[1].material, Material::Selection);
    }

    #[test]
    fn the_material_painter_paints_the_object_under_the_click() {
        use crate::tools::materials::{self, PainterMode};
        use plan_core::object_materials::WHOLE_OBJECT;
        let (p, wall) = one_wall();
        let scene = scene_of(&p);
        let cam = camera_for(CameraMode::Orbit, &scene);
        let at = pixel_of(&cam, [120.0, 48.0, 3.0]);
        let hit = object_under(&p, 0, &cam, rect(), at, &scene, &[]);
        assert_eq!(hit, Some((0, ObjectRef::Wall(wall))));
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        // The material the Materials list would have made active.
        let name = materials::library()
            .materials
            .iter()
            .find(|m| m.category.first().map(String::as_str) == Some("Masonry"))
            .unwrap()
            .name
            .clone();
        materials::set_active(Some(name.clone()));
        materials::set_painter_mode(PainterMode::Paint);
        apply_pick(&mut cx, hit, false);
        materials::set_painter_mode(PainterMode::Off);
        assert_eq!(
            cx.project.object_material(wall, WHOLE_OBJECT),
            Some(name.as_str())
        );
        assert!(
            cx.selection.items.is_empty(),
            "a painter click never selects"
        );
        // The 3D scene of the painted plan shows it, and the view rebuilds.
        let painted = scene_of(&cx.project);
        let before: Vec<Material> = scene.meshes.iter().map(|m| m.material).collect();
        let after: Vec<Material> = painted.meshes.iter().map(|m| m.material).collect();
        assert_ne!(before, after);
        assert_ne!(
            super::super::project_hash(&cx.project),
            super::super::project_hash(&Project::new("t"))
        );
        // With the painter off a pick selects again.
        apply_pick(&mut cx, hit, false);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Wall(wall)));
    }

    #[test]
    fn hover_picks_wait_for_a_real_move_and_the_interval() {
        let p = egui::pos2(100.0, 100.0);
        assert!(hover_due(None, p, 0.0), "the first pointer position picks");
        let last = Some((p, 1.0));
        // Still, a wobble, a move that comes too soon: nothing.
        assert!(!hover_due(last, p, 2.0));
        assert!(!hover_due(last, p + egui::vec2(1.0, 1.0), 2.0));
        assert!(!hover_due(last, p + egui::vec2(20.0, 0.0), 1.01));
        // A move of a few pixels after the interval: pick.
        assert!(hover_due(
            last,
            p + egui::vec2(HOVER_MIN_MOVE_PX, 0.0),
            1.0 + HOVER_MIN_INTERVAL_S
        ));
        assert!(hover_due(last, p + egui::vec2(0.0, -30.0), 1.5));
    }

    #[test]
    fn the_hover_tints_objects_that_are_not_selected_in_a_lighter_colour() {
        let (p, wall) = one_wall();
        let scene = scene_of(&p);
        let sel = Selection::default();
        let hover = overlay_with_hover(&scene, Vec::new(), &sel, Some(wall), false);
        assert!(!hover.is_empty());
        assert!(hover
            .iter()
            .all(|m| m.material == Material::Selection && m.color == Some(HOVER_RGB)));
        // A selected object keeps the selection tint alone.
        let mut sel = Selection::default();
        sel.set(ObjectRef::Wall(wall));
        let both = overlay_with_hover(&scene, Vec::new(), &sel, Some(wall), false);
        assert_eq!(both.len(), hover.len());
        assert!(both.iter().all(|m| m.color.is_none()));
        // The hover is part of the overlay's identity.
        assert_ne!(
            overlay_key_with_hover(&Selection::default(), Some(wall), &[], false),
            empty_overlay_key()
        );
        assert_eq!(
            overlay_key_with_hover(&Selection::default(), None, &[], false),
            empty_overlay_key()
        );
        // Rooms and the terrain have no meshes to tint.
        assert_eq!(hover_id(Some((0, ObjectRef::Room(0)))), None);
        assert_eq!(hover_id(Some((0, ObjectRef::Wall(wall)))), Some(wall));
        assert_eq!(hover_id(None), None);
    }

    #[test]
    fn the_surface_point_is_where_the_nearest_surface_is_hit() {
        let (p, _) = one_wall();
        let scene = scene_of(&p);
        let cam = camera_for(CameraMode::Orbit, &scene);
        let on_wall: Vec3 = [120.0, 96.0, 0.0];
        let at = pixel_of(&cam, on_wall);
        let got = surface_point(&cam, rect(), at, &scene, &[]).expect("a hit");
        let off = math::length(math::sub(got, on_wall));
        assert!(off < 1.0, "{got:?} is {off} from {on_wall:?}");
        // Surfaces without an object count (the slab under a wall would).
        let mut floor = Scene::default();
        floor.meshes.push(quad(Material::Floor, None, 50.0, 0.0));
        let cam = Camera {
            mode: CameraMode::FullCamera,
            position: [0.0, 100.0, 0.0],
            yaw: 0.0,
            pitch: -std::f32::consts::FRAC_PI_2 + 0.01,
            ..Camera::default()
        };
        let hit = surface_point(&cam, rect(), rect().center(), &floor, &[]).expect("the slab");
        assert!(hit[1].abs() < 1e-3);
        // The sky has no point.
        assert!(surface_point(&cam, rect(), rect().center(), &Scene::default(), &[]).is_none());
    }
}
