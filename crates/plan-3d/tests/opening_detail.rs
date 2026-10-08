//! Openings in the editor's 3D view: casing, jambs, window stools and aprons,
//! thresholds, doors shown open, shutter color and a transom over a door
//! (docs/parity/doors-windows.md DW-52, DW-79..DW-85).

use plan_3d::{build_scene_with, Material, Scene, SceneOptions};
use plan_core::openings::{CasingProfile, ShutterStyle};
use plan_core::{Casing, Id, OpeningKind, OpeningStyle, Point, Project, WallKind};

/// One 10' wall along +x, 4 1/2" thick and 100" high, with a single opening
/// in its middle: scene x is the wall offset, y the height, z the negated
/// wall-local side.
fn project(wall: WallKind, kind: OpeningKind, style: OpeningStyle, width: f64) -> (Project, Id) {
    let mut p = Project::new("t");
    let w = p.add_wall(0, Point::ZERO, Point::new(120.0, 0.0), 4.5, 100.0, wall);
    let id = p.add_opening(0, w, 60.0, kind).unwrap();
    let o = &mut p.floors[0].openings[0];
    o.style = style;
    o.width = width;
    (p, id)
}

fn door() -> (Project, Id) {
    project(
        WallKind::Exterior,
        OpeningKind::Door,
        OpeningStyle::Hinged,
        36.0,
    )
}

fn window() -> (Project, Id) {
    project(
        WallKind::Exterior,
        OpeningKind::Window,
        OpeningStyle::Window,
        48.0,
    )
}

fn casing_on() -> SceneOptions {
    SceneOptions {
        show_casing: true,
        ..Default::default()
    }
}

/// Bounding box `[x0, x1, y0, y1, z0, z1]` of every box (36 indices) of an
/// object's meshes in `material`.
fn boxes(scene: &Scene, id: Id, material: Material) -> Vec<[f32; 6]> {
    let mut out = Vec::new();
    for m in scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == material)
    {
        for chunk in m.indices.chunks(36) {
            let mut b = [f32::MAX, f32::MIN, f32::MAX, f32::MIN, f32::MAX, f32::MIN];
            for &i in chunk {
                let v = m.vertices[i as usize].position;
                for a in 0..3 {
                    b[2 * a] = b[2 * a].min(v[a]);
                    b[2 * a + 1] = b[2 * a + 1].max(v[a]);
                }
            }
            out.push(b);
        }
    }
    out
}

fn near(a: f32, b: f64) -> bool {
    (f64::from(a) - b).abs() < 1e-3
}

/// A box with these x and y ranges exists.
fn has_box(boxes: &[[f32; 6]], x: (f64, f64), y: (f64, f64)) -> bool {
    boxes
        .iter()
        .any(|b| near(b[0], x.0) && near(b[1], x.1) && near(b[2], y.0) && near(b[3], y.1))
}

fn extent(scene: &Scene, id: Id, material: Material, axis: usize) -> (f32, f32) {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == material)
        .flat_map(|m| m.vertices.iter().map(move |v| v.position[axis]))
        .fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
}

#[test]
fn a_36_inch_exterior_door_gets_casing_jambs_and_a_threshold() {
    let (p, id) = door();
    let scene = build_scene_with(&p, &casing_on());
    let trim = boxes(&scene, id, Material::Trim);
    // 6 casing boards (two legs and a head on each face), 6 jamb boards (the
    // legs and the head on each side of the leaf) and the threshold.
    assert_eq!(trim.len(), 13);
    // The door hole is 42..78 across and 80 high; casing is 3 1/2" wide with
    // a 1/4" reveal: legs 38.25..41.75 and 78.25..81.75, head 80.25..83.75.
    assert!(has_box(&trim, (38.25, 41.75), (0.0, 80.25)));
    assert!(has_box(&trim, (78.25, 81.75), (0.0, 80.25)));
    assert!(has_box(&trim, (38.25, 81.75), (80.25, 83.75)));
    // The threshold spans the hole, 3/4" high, through the whole wall.
    let threshold: Vec<_> = trim
        .iter()
        .filter(|b| near(b[0], 42.0) && near(b[1], 78.0) && near(b[3], 0.75))
        .collect();
    assert_eq!(threshold.len(), 1);
    assert!(near(threshold[0][4], -2.25) && near(threshold[0][5], 2.25));
    // Jamb boards line the hole through the wall thickness, clear of the
    // 1 3/8" leaf: 3/4" wide, from the face to the leaf.
    let jambs: Vec<_> = trim
        .iter()
        .filter(|b| near(b[0], 42.0) && near(b[1], 42.75) && near(b[3], 80.0))
        .collect();
    assert_eq!(jambs.len(), 2);
    assert!(jambs
        .iter()
        .all(|b| (near(b[5], 2.25) && near(b[4], 0.6875))
            || (near(b[4], -2.25) && near(b[5], -0.6875))));
    // The casing stands proud of the faces by its depth (3/4").
    let (z0, z1) = extent(&scene, id, Material::Trim, 2);
    assert!(near(z0, -3.0) && near(z1, 3.0), "{z0} {z1}");
    // Without casing nothing of it is built.
    assert!(boxes(
        &build_scene_with(&p, &SceneOptions::default()),
        id,
        Material::Trim
    )
    .is_empty());
}

#[test]
fn an_interior_door_has_no_threshold() {
    let (p, id) = project(
        WallKind::Interior,
        OpeningKind::Door,
        OpeningStyle::Hinged,
        36.0,
    );
    let trim = boxes(&build_scene_with(&p, &casing_on()), id, Material::Trim);
    assert_eq!(trim.len(), 12);
    assert!(!trim.iter().any(|b| near(b[3], 0.75)));
}

#[test]
fn a_48_inch_window_gets_casing_apron_stools_and_jambs() {
    let (p, id) = window();
    let scene = build_scene_with(&p, &casing_on());
    let trim = boxes(&scene, id, Material::Trim);
    // Hole 36..84 across, 24..84 high. Per face: two legs, the head and the
    // apron (4); a stool on each face of an exterior wall (2); jamb boards
    // outside the 3 1/2" frame: legs, head and sill on each side (8).
    assert_eq!(trim.len(), 4 * 2 + 2 + 8);
    // Legs run the height of the opening, the head and apron the width of
    // the casing: 32.25..87.75.
    assert!(has_box(&trim, (32.25, 35.75), (24.0, 84.25)));
    assert!(has_box(&trim, (84.25, 87.75), (24.0, 84.25)));
    assert!(has_box(&trim, (32.25, 87.75), (84.25, 87.75)));
    assert!(has_box(&trim, (32.25, 87.75), (19.75, 23.25)));
    // The stools sit just under the opening, 3/4" thick.
    let stools: Vec<_> = trim
        .iter()
        .filter(|b| near(b[0], 32.25) && near(b[1], 87.75) && near(b[2], 23.25) && near(b[3], 24.0))
        .collect();
    assert_eq!(stools.len(), 2);
    // One projects 1 1/4" past the room face, the other past the exterior
    // casing (3/4" deep plus 1 1/4"); the walls are 4 1/2" thick.
    let mut reach: Vec<f32> = stools.iter().map(|b| b[4].abs().max(b[5].abs())).collect();
    reach.sort_by(f32::total_cmp);
    assert!(near(reach[0], 3.5) && near(reach[1], 4.25), "{reach:?}");
}

#[test]
fn exterior_casing_has_its_own_size() {
    let (mut p, id) = window();
    let o = &mut p.floors[0].openings[0];
    o.casing = Some(Casing {
        width: 3.5,
        depth: 0.75,
        reveal: 0.25,
    });
    o.extras.spec.casing_exterior_size = Some(Casing {
        width: 5.0,
        depth: 1.5,
        reveal: 0.5,
    });
    let scene = build_scene_with(&p, &casing_on());
    let trim = boxes(&scene, id, Material::Trim);
    // One face carries the 3 1/2" legs, the other the 5" ones.
    assert!(has_box(&trim, (32.25, 35.75), (24.0, 84.25)));
    assert!(has_box(&trim, (30.5, 35.5), (24.0, 84.5)));
    let (z0, z1) = extent(&scene, id, Material::Trim, 2);
    assert!(
        z0.min(z1).abs().max(z0.max(z1).abs()) > 2.25 + 1.5,
        "{z0} {z1}"
    );
    // Turning one face's casing off leaves the other.
    p.floors[0].openings[0].extras.spec.casing_exterior = false;
    let one = boxes(&build_scene_with(&p, &casing_on()), id, Material::Trim);
    assert!(!has_box(&one, (30.5, 35.5), (24.0, 84.5)));
    assert!(has_box(&one, (32.25, 35.75), (24.0, 84.25)));
}

#[test]
fn casing_profiles_add_a_head_cap_or_plinth_blocks() {
    let (mut p, id) = door();
    let flat = boxes(&build_scene_with(&p, &casing_on()), id, Material::Trim).len();
    p.floors[0].openings[0].extras.spec.casing_profile = CasingProfile::Cap;
    let cap = boxes(&build_scene_with(&p, &casing_on()), id, Material::Trim);
    // A cap board on top of each head.
    assert_eq!(cap.len(), flat + 2);
    assert!(cap.iter().any(|b| near(b[2], 83.75) && near(b[3], 84.75)));
    p.floors[0].openings[0].extras.spec.casing_profile = CasingProfile::Plinth;
    let plinth = boxes(&build_scene_with(&p, &casing_on()), id, Material::Trim);
    // A block at the foot of each leg and at each head corner, on both faces.
    assert_eq!(plinth.len(), flat + 8);
    assert!(plinth.iter().any(|b| near(b[2], 0.0) && near(b[3], 6.0)));
}

#[test]
fn the_plan_decides_whether_the_view_builds_casing_and_opens_doors() {
    let (mut p, id) = door();
    let built = |p: &Project| {
        let scene = build_scene_with(p, &SceneOptions::for_project(p));
        let trim = boxes(&scene, id, Material::Trim).len();
        let (z0, z1) = extent(&scene, id, Material::DoorPanel, 2);
        (trim, z0.abs().max(z1.abs()))
    };
    let (trim, reach) = built(&p);
    assert_eq!(trim, 13, "casing is on in a new plan");
    assert!(reach < 1.0);
    p.opening_display.doors_open = true;
    let (_, reach) = built(&p);
    assert!(reach > 35.0, "{reach}");
    p.opening_display.casing = false;
    assert_eq!(built(&p).0, 0);
    // It is saved with the plan.
    let back = Project::from_json(&p.to_json().unwrap()).unwrap();
    assert!(back.opening_display.doors_open && !back.opening_display.casing);
}

/// The farthest a door's panels reach from the wall plane, inches.
fn swing_reach(p: &Project, id: Id, opts: &SceneOptions) -> f32 {
    let scene = build_scene_with(p, opts);
    let (z0, z1) = extent(&scene, id, Material::DoorPanel, 2);
    z0.abs().max(z1.abs())
}

#[test]
fn a_door_opens_by_its_own_slider() {
    let (mut p, id) = door();
    let closed = SceneOptions::default();
    assert!(swing_reach(&p, id, &closed) < 1.0);
    // Open all the way: the 36" leaf stands square to the wall.
    p.floors[0].openings[0].extras.spec.show_open_in_3d = true;
    let full = swing_reach(&p, id, &closed);
    assert!((full - 36.0).abs() < 0.05, "{full}");
    // Half way is 45 degrees: 36 sin 45 plus half the 1 3/8" slab's cos 45.
    p.floors[0].openings[0].extras.spec.open_fraction = 0.5;
    let half = swing_reach(&p, id, &closed);
    let want = 36.0 * 45.0_f64.to_radians().sin() + 0.6875 * 45.0_f64.to_radians().cos();
    assert!((f64::from(half) - want).abs() < 0.05, "{half} vs {want}");
    // The slider scales the door's own Swing Angle.
    p.floors[0].openings[0].extras.swing_angle_deg = Some(60.0);
    p.floors[0].openings[0].extras.spec.open_fraction = 1.0;
    let sixty = swing_reach(&p, id, &closed);
    let want = 36.0 * 60.0_f64.to_radians().sin() + 0.6875 * 60.0_f64.to_radians().cos();
    assert!((f64::from(sixty) - want).abs() < 0.05, "{sixty} vs {want}");
    // Closed again with the checkbox off.
    p.floors[0].openings[0].extras.spec.show_open_in_3d = false;
    assert!(swing_reach(&p, id, &closed) < 1.0);
    // The plan-wide switch opens every door at the plan's angle.
    let all = SceneOptions {
        doors_open: true,
        open_angle_deg: 90.0,
        ..Default::default()
    };
    assert!((swing_reach(&p, id, &all) - 36.0).abs() < 0.05);
}

#[test]
fn sliding_pocket_bifold_and_garage_doors_have_open_states() {
    let open = SceneOptions {
        doors_open: true,
        ..Default::default()
    };
    let x_extent = |p: &Project, id| {
        let scene = build_scene_with(p, &open);
        let (a, b) = extent(&scene, id, Material::DoorPanel, 0);
        let closed = build_scene_with(p, &SceneOptions::default());
        let (c, d) = extent(&closed, id, Material::DoorPanel, 0);
        ((a, b), (c, d))
    };
    // A pocket door slides into the wall beside it.
    let (p, id) = project(
        WallKind::Interior,
        OpeningKind::Door,
        OpeningStyle::Pocket,
        36.0,
    );
    let ((a, _), (c, d)) = x_extent(&p, id);
    assert!(near(c, 42.0) && near(d, 78.0));
    assert!(a < 10.0, "{a}");
    // A bifold folds its panels toward the jambs and out of the plane.
    let (p, id) = project(
        WallKind::Interior,
        OpeningKind::Door,
        OpeningStyle::Bifold,
        36.0,
    );
    assert!(swing_reach(&p, id, &open) > 8.0);
    assert!(swing_reach(&p, id, &SceneOptions::default()) < 1.0);
    // A sliding glass door's panel slides over its neighbour.
    let (p, id) = project(
        WallKind::Exterior,
        OpeningKind::Door,
        OpeningStyle::Sliding,
        72.0,
    );
    let scene_open = build_scene_with(&p, &open);
    let scene_closed = build_scene_with(&p, &SceneOptions::default());
    let sum = |s: &Scene| -> f32 {
        s.meshes
            .iter()
            .filter(|m| m.object_id == Some(id) && m.material == Material::DoorPanel)
            .flat_map(|m| m.vertices.iter().map(|v| v.position[0]))
            .sum()
    };
    assert!((sum(&scene_open) - sum(&scene_closed)).abs() > 100.0);
    // A garage door rolls up and back overhead into the room.
    let (p, id) = project(
        WallKind::Exterior,
        OpeningKind::Door,
        OpeningStyle::Garage,
        108.0,
    );
    assert!(swing_reach(&p, id, &SceneOptions::default()) < 2.0);
    let rolled = swing_reach(&p, id, &open);
    assert!(rolled > 60.0, "{rolled}");
    let (_, top) = extent(&build_scene_with(&p, &open), id, Material::DoorPanel, 1);
    assert!(top > 80.0, "the sections lie above the head: {top}");
}

#[test]
fn shutters_carry_their_paint_color() {
    let (mut p, id) = window();
    p.floors[0].openings[0].extras.spec.shutters.style = ShutterStyle::Panel;
    p.floors[0].openings[0].extras.spec.shutters.color = [10, 90, 40];
    let scene = build_scene_with(&p, &SceneOptions::default());
    let painted: Vec<_> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.color.is_some())
        .collect();
    assert_eq!(painted.len(), 1);
    assert_eq!(painted[0].color, Some([10, 90, 40]));
    assert_eq!(painted[0].material, Material::Trim);
    // Everything else keeps its material's own color.
    assert!(scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .any(|m| m.color.is_none()));
}

#[test]
fn a_transom_over_a_door_shares_one_opening_and_one_casing() {
    let (mut p, door) = door();
    let transom = p.add_transom(0, door, 18.0).unwrap();
    let o = p.floors[0]
        .openings
        .iter()
        .find(|o| o.id == transom)
        .unwrap();
    assert_eq!((o.sill_height, o.height, o.width), (80.0, 18.0, 36.0));
    let scene = build_scene_with(&p, &casing_on());
    // The wall is open from the floor to 98": no face closes the door's head
    // or the window's sill across the hole.
    let wall = p.floors[0].walls[0].id;
    let closes = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(wall))
        .any(|m| {
            m.indices.chunks(3).any(|t| {
                t.iter().all(|&i| {
                    let v = m.vertices[i as usize].position;
                    (v[1] - 80.0).abs() < 1e-4 && v[0] > 42.5 && v[0] < 77.5
                })
            })
        });
    assert!(!closes, "the head of the door is not a face of the wall");
    // One casing around the unit: the legs rise from the floor to the top of
    // the transom (98") and a single head on each face, the transom's own
    // casing is not built.
    let unit_trim = boxes(&scene, door, Material::Trim);
    assert!(has_box(&unit_trim, (38.25, 41.75), (0.0, 98.25)));
    assert!(has_box(&unit_trim, (78.25, 81.75), (0.0, 98.25)));
    let heads = unit_trim.iter().filter(|b| near(b[2], 98.25)).count();
    assert_eq!(heads, 2);
    // The door: 6 casing boards, 4 jamb legs (its head is the transom's
    // sill), the threshold.
    assert_eq!(unit_trim.len(), 6 + 4 + 1);
    // The transom: only jamb boards (two legs and a head on each side).
    let top = boxes(&scene, transom, Material::Trim);
    assert_eq!(top.len(), 6);
    assert!(top.iter().all(|b| b[2] >= 79.99 && b[3] <= 98.01));
    // The transom is glazed.
    assert!(!boxes(&scene, transom, Material::WindowGlass).is_empty());
}

#[test]
fn casing_and_threshold_of_a_door_on_a_curved_wall_stand_at_its_arc_position() {
    let mut p = Project::new("arc");
    let w = p.add_wall(
        0,
        Point::ZERO,
        Point::new(240.0, 0.0),
        6.0,
        96.0,
        WallKind::Exterior,
    );
    p.floors[0].walls[0].curve = Some(plan_core::walls::WallCurve { bulge: 60.0 });
    let id = p.add_opening(0, w, 100.0, OpeningKind::Door).unwrap();
    let scene = build_scene_with(&p, &casing_on());
    let trim = boxes(&scene, id, Material::Trim);
    assert_eq!(trim.len(), 13);
    // The threshold is the box 3/4" high: its middle is the door's center on
    // the arc (scene z is the negated plan y).
    let threshold = trim.iter().find(|b| near(b[3], 0.75)).unwrap();
    let at = p.floors[0].walls[0].point_along(100.0);
    let mid = Point::new(
        f64::from(threshold[0] + threshold[1]) * 0.5,
        -f64::from(threshold[4] + threshold[5]) * 0.5,
    );
    assert!(mid.dist(at) < 0.5, "{mid:?} vs {at:?}");
    // The casing legs flank it, 3 1/2" wide, one each side.
    let legs = trim
        .iter()
        .filter(|b| near(b[2], 0.0) && near(b[3], 80.25))
        .count();
    assert_eq!(legs, 4);
}
