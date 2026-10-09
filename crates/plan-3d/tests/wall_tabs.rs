//! The Wall Specification tabs in 3D: Wall Covering bands from the molding
//! library, and the Newels/Balusters and Rails tabs of a railing wall.

use plan_3d::{build_scene_with_types, Material, Mesh, Scene, SceneOptions};
use plan_core::walls::{BalusterStyle, NewelStyle, RailFill};
use plan_core::{Id, OpeningKind, PlanDefaults, Point, Project, WallClass, WallKind};

fn wall_project(kind: WallKind) -> (Project, Id) {
    let mut p = Project::new("t");
    let id = p.add_wall(0, Point::ZERO, Point::new(240.0, 0.0), 6.0, 96.0, kind);
    (p, id)
}

fn scene(p: &Project) -> Scene {
    let d = PlanDefaults::chief_x18_daniel();
    build_scene_with_types(p, &SceneOptions::default(), &d.wall_types)
}

fn of(scene: &Scene, id: Id) -> Vec<&Mesh> {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .collect()
}

fn tris(scene: &Scene, id: Id) -> usize {
    of(scene, id).iter().map(|m| m.indices.len() / 3).sum()
}

/// How far the wall's meshes reach past the faces of a 6" wall on the +z and
/// -z sides.
fn reach(scene: &Scene, id: Id) -> (f32, f32) {
    let mut r = (0.0_f32, 0.0_f32);
    for m in of(scene, id) {
        for v in &m.vertices {
            r.0 = r.0.max(v.position[2] - 3.0);
            r.1 = r.1.max(-v.position[2] - 3.0);
        }
    }
    r
}

#[test]
fn a_wainscot_and_moldings_add_bands_on_the_chosen_face_only() {
    let (mut p, id) = wall_project(WallKind::Interior);
    let s = scene(&p);
    let bare = tris(&s, id);
    let (a, b) = reach(&s, id);
    assert!(a < 0.01 && b < 0.01, "a bare wall stays inside its faces");
    {
        let c = &mut p.floors[0].wall_mut(id).unwrap().spec.covering.interior;
        c.wainscot = "Beadboard".into();
        c.wainscot_height = 36.0;
        c.base = "Base 5 1/4".into();
        c.crown = "Crown 3 5/8".into();
        c.chair_rail = "Chair Rail".into();
    }
    let s = scene(&p);
    assert!(tris(&s, id) > bare);
    let (a, b) = reach(&s, id);
    // Everything stands on one face.
    assert!((a > 0.1) != (b > 0.1), "{a} {b}");
}

#[test]
fn the_exterior_and_interior_coverings_stand_on_opposite_faces() {
    let (mut p, id) = wall_project(WallKind::Exterior);
    p.floors[0]
        .wall_mut(id)
        .unwrap()
        .spec
        .covering
        .exterior
        .wainscot = "Stone".into();
    let ext = reach(&scene(&p), id);
    p.floors[0]
        .wall_mut(id)
        .unwrap()
        .spec
        .covering
        .exterior
        .wainscot
        .clear();
    p.floors[0]
        .wall_mut(id)
        .unwrap()
        .spec
        .covering
        .interior
        .wainscot = "Stone".into();
    let int = reach(&scene(&p), id);
    assert!((ext.0 > 0.1) != (ext.1 > 0.1), "{ext:?}");
    assert!((int.0 > 0.1) != (int.1 > 0.1), "{int:?}");
    assert_eq!(ext.0 > 0.1, int.1 > 0.1, "{ext:?} {int:?}");
}

#[test]
fn a_door_opens_the_wainscot() {
    let (mut p, id) = wall_project(WallKind::Interior);
    p.floors[0]
        .wall_mut(id)
        .unwrap()
        .spec
        .covering
        .interior
        .wainscot = "Beadboard".into();
    let covered = tris(&scene(&p), id);
    // The same wall with a door: the wainscot stops at the jambs, so the
    // slab is cut in two (more triangles than one whole slab).
    p.add_opening(0, id, 120.0, OpeningKind::Door).unwrap();
    let with_door = tris(&scene(&p), id);
    assert!(with_door > covered);
}

fn railing() -> (Project, Id) {
    let (mut p, id) = wall_project(WallKind::Exterior);
    p.floors[0]
        .wall_mut(id)
        .unwrap()
        .set_class(WallClass::Railing);
    p.floors[0].wall_mut(id).unwrap().height = 36.0;
    (p, id)
}

#[test]
fn the_default_railing_spec_matches_the_old_railing() {
    let (p, id) = railing();
    // 240" at 96" spacing: 4 posts.
    assert_eq!(plan_3d::railing_post_count(240.0), 4);
    let s = scene(&p);
    assert!((s.bounds().unwrap().1[1] - 36.0).abs() < 1e-3);
    assert!(tris(&s, id) > 0);
}

#[test]
fn newel_spacing_and_style_change_the_railing() {
    let (mut p, id) = railing();
    let base = tris(&scene(&p), id);
    p.floors[0].wall_mut(id).unwrap().spec.railing.newel_spacing = 48.0;
    let tight = tris(&scene(&p), id);
    assert_ne!(tight, base, "more newels change the railing");
    p.floors[0].wall_mut(id).unwrap().spec.railing.newel_style = NewelStyle::Round;
    assert_ne!(tris(&scene(&p), id), tight);
}

#[test]
fn balusters_panels_and_rail_height_follow_the_tabs() {
    let (mut p, id) = railing();
    let spaced = tris(&scene(&p), id);
    p.floors[0]
        .wall_mut(id)
        .unwrap()
        .spec
        .railing
        .baluster_spacing = 8.0;
    assert!(tris(&scene(&p), id) < spaced, "fewer balusters");
    p.floors[0]
        .wall_mut(id)
        .unwrap()
        .spec
        .railing
        .baluster_style = BalusterStyle::Round;
    assert!(tris(&scene(&p), id) > 0);
    // Glass panels have no balusters and a glass mesh.
    p.floors[0].wall_mut(id).unwrap().spec.railing.fill = RailFill::GlassPanel;
    let s = scene(&p);
    assert!(of(&s, id).iter().any(|m| m.material == Material::Glass));
    // A taller top rail raises the top of the railing.
    p.floors[0].wall_mut(id).unwrap().spec.railing.top_rail_top = Some(42.0);
    p.floors[0].wall_mut(id).unwrap().height = 42.0;
    assert!((scene(&p).bounds().unwrap().1[1] - 42.0).abs() < 0.6);
}

#[test]
fn posts_run_down_to_the_beam_when_asked() {
    let (mut p, _id) = railing();
    let before = scene(&p).bounds().unwrap().0[1];
    p.floors[0].wall_mut(_id).unwrap().spec.railing.post_to_beam = true;
    let after = scene(&p).bounds().unwrap().0[1];
    assert!(after < before - 1.0, "{after} vs {before}");
}
