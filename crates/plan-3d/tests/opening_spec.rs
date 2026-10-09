//! The Specification tabs in 3D: sash, frame, lites, lintel, arch, hardware,
//! shutters, mulled units and the niche depth (docs/parity/doors-windows.md).

use plan_3d::{build_scene, build_scene_with, Material, Mesh, Scene, SceneOptions};
use plan_core::openings::{
    Arch, ArchType, HandleStyle, LintelStyle, LiteStyle, ShutterSides, ShutterStyle,
};
use plan_core::{Id, OpeningKind, OpeningStyle, Point, Project, WallKind};

/// One 10' wall along +x, 4 1/2" thick, with a single opening in its middle.
fn project(wall: WallKind, kind: OpeningKind, style: OpeningStyle, width: f64) -> (Project, Id) {
    let mut p = Project::new("t");
    let w = p.add_wall(0, Point::ZERO, Point::new(120.0, 0.0), 4.5, 100.0, wall);
    let id = p.add_opening(0, w, 60.0, kind).unwrap();
    let o = &mut p.floors[0].openings[0];
    o.style = style;
    o.width = width;
    (p, id)
}

fn window() -> (Project, Id) {
    project(
        WallKind::Exterior,
        OpeningKind::Window,
        OpeningStyle::Window,
        48.0,
    )
}

fn door() -> (Project, Id) {
    project(
        WallKind::Exterior,
        OpeningKind::Door,
        OpeningStyle::Hinged,
        36.0,
    )
}

fn edit(p: &mut Project, f: impl FnOnce(&mut plan_core::Opening)) {
    f(&mut p.floors[0].openings[0]);
}

fn tris(scene: &Scene, id: Id, material: Material) -> usize {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == material)
        .map(Mesh::triangle_count)
        .sum()
}

fn extent(scene: &Scene, id: Id, material: Material, axis: usize) -> (f32, f32) {
    let v = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == material)
        .flat_map(|m| m.vertices.iter().map(|v| v.position[axis]));
    v.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
}

/// The distinct x values of an object's vertices of one material, ascending.
fn xs(scene: &Scene, id: Id, material: Material) -> Vec<f32> {
    let mut v: Vec<f32> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == material)
        .flat_map(|m| m.vertices.iter().map(|v| v.position[0]))
        .collect();
    v.sort_by(f32::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    v
}

/// Smallest and largest gap between consecutive distinct x values.
fn gaps(scene: &Scene, id: Id, material: Material) -> (f32, f32) {
    let v = xs(scene, id, material);
    v.windows(2)
        .map(|w| w[1] - w[0])
        .fold((f32::MAX, f32::MIN), |(lo, hi), g| (lo.min(g), hi.max(g)))
}

fn casing_on() -> SceneOptions {
    SceneOptions {
        show_casing: true,
        ..Default::default()
    }
}

#[test]
fn sash_and_frame_widths_set_the_glass() {
    let (mut p, id) = window();
    let glass_w = |p: &Project| {
        let (lo, hi) = extent(&build_scene(p), id, Material::WindowGlass, 0);
        hi - lo
    };
    // Defaults: 3/4" frame and 1 1/2" sash on each side of a 48" window.
    assert!((glass_w(&p) - (48.0 - 1.5 - 3.0)).abs() < 1e-3);
    edit(&mut p, |o| {
        o.extras.frame_width = Some(2.0);
        o.extras.sash_width = Some(2.5);
    });
    assert!((glass_w(&p) - (48.0 - 4.0 - 5.0)).abs() < 1e-3);
    // No sash: the glass runs to the frame, and four boxes of frame go.
    let with_sash = tris(&build_scene(&p), id, Material::WindowFrame);
    edit(&mut p, |o| o.extras.spec.has_sash = false);
    assert!((glass_w(&p) - (48.0 - 4.0)).abs() < 1e-3);
    assert_eq!(
        tris(&build_scene(&p), id, Material::WindowFrame),
        with_sash - 4 * 12
    );
    // Top and bottom sash rails are their own widths.
    edit(&mut p, |o| {
        o.extras.spec.has_sash = true;
        o.extras.spec.sash_top = 3.0;
        o.extras.spec.sash_bottom = 0.5;
    });
    let (lo, hi) = extent(&build_scene(&p), id, Material::WindowGlass, 1);
    let h = p.floors[0].openings[0].height as f32;
    assert!(
        (hi - lo - (h - 4.0 - 3.0 - 0.5)).abs() < 1e-3,
        "{}",
        hi - lo
    );
}

#[test]
fn the_mullion_width_is_the_post_of_a_double_casement() {
    let (mut p, id) = project(
        WallKind::Exterior,
        OpeningKind::Window,
        OpeningStyle::Casement,
        60.0,
    );
    edit(&mut p, |o| o.extras.spec.has_sash = false);
    // Two sashes of glass; the post is the space between them, the widest
    // gap in the glass's x values less the panes themselves.
    let gap = |p: &Project| {
        let v = xs(&build_scene(p), id, Material::WindowGlass);
        assert_eq!(v.len(), 4, "{v:?}");
        v[2] - v[1]
    };
    assert!((gap(&p) - 1.5).abs() < 1e-3);
    edit(&mut p, |o| o.extras.spec.mullion_width = 4.0);
    assert!((gap(&p) - 4.0).abs() < 1e-3);
}

#[test]
fn lite_styles_change_the_panes_and_muntins() {
    let (mut p, id) = window();
    let counts = |p: &Project| {
        let s = build_scene(p);
        (
            tris(&s, id, Material::WindowGlass),
            tris(&s, id, Material::WindowFrame),
        )
    };
    let (g1, f1) = counts(&p);
    assert_eq!(g1, 12);
    edit(&mut p, |o| o.lites = (3, 2));
    let (g, f) = counts(&p);
    assert_eq!(g, 6 * 12);
    // Two vertical and one horizontal muntin.
    assert_eq!(f, f1 + 3 * 12);

    // Diamond: one pane behind crossing bars; (3 + 2 - 1) bars each way.
    edit(&mut p, |o| o.extras.spec.lite_style = LiteStyle::Diamond);
    let (g, f) = counts(&p);
    assert_eq!(g, 12);
    assert_eq!(f, f1 + 8 * 12);

    // Prairie: a border of small lites round one big pane: a 3 x 3 grid.
    edit(&mut p, |o| o.extras.spec.lite_style = LiteStyle::Prairie);
    let (g, f) = counts(&p);
    assert_eq!(g, 9 * 12);
    assert_eq!(f, f1 + 4 * 12);

    // Custom: dividers where the tab puts them.
    edit(&mut p, |o| {
        o.extras.spec.lite_style = LiteStyle::Custom;
        o.extras.spec.custom_across = vec![0.25, 0.5, 0.75];
        o.extras.spec.custom_up = vec![];
    });
    let (g, f) = counts(&p);
    assert_eq!(g, 4 * 12);
    assert_eq!(f, f1 + 3 * 12);

    // The muntin width is the gap between panes.
    let (before, _) = gaps(&build_scene(&p), id, Material::WindowGlass);
    assert!((before - 0.875).abs() < 1e-3, "{before}");
    edit(&mut p, |o| o.extras.spec.muntin_width = 3.0);
    let (after, _) = gaps(&build_scene(&p), id, Material::WindowGlass);
    assert!((after - 3.0).abs() < 1e-3, "{after}");
}

#[test]
fn doors_take_lite_styles_too() {
    let (mut p, id) = door();
    assert_eq!(tris(&build_scene(&p), id, Material::WindowGlass), 0);
    edit(&mut p, |o| o.extras.spec.lite_style = LiteStyle::Diamond);
    assert_eq!(tris(&build_scene(&p), id, Material::WindowGlass), 12);
    edit(&mut p, |o| {
        o.extras.spec.lite_style = LiteStyle::Standard;
        o.lites = (2, 2);
    });
    assert_eq!(tris(&build_scene(&p), id, Material::WindowGlass), 4 * 12);
}

#[test]
fn a_lintel_and_an_exterior_sill_add_trim_on_the_outside() {
    let (mut p, id) = window();
    let trim = |p: &Project| tris(&build_scene(p), id, Material::Trim);
    assert_eq!(trim(&p), 0);
    edit(&mut p, |o| {
        o.extras.spec.lintel.exterior = true;
    });
    assert_eq!(trim(&p), 12);
    let flat_z = extent(&build_scene(&p), id, Material::Trim, 2);
    // It stands 1 1/2" off the wall face, on the outside (left = -z here).
    assert!((flat_z.1 - flat_z.0 - 1.5).abs() < 1e-3);
    edit(&mut p, |o| o.extras.spec.lintel.style = LintelStyle::Cap);
    assert_eq!(trim(&p), 24);
    edit(&mut p, |o| {
        o.extras.spec.lintel.style = LintelStyle::Keystone
    });
    assert_eq!(trim(&p), 24);
    // Interior side only: the same board, on the other face.
    edit(&mut p, |o| {
        o.extras.spec.lintel.style = LintelStyle::Flat;
        o.extras.spec.lintel.exterior = false;
        o.extras.spec.lintel.interior = true;
    });
    let inner_z = extent(&build_scene(&p), id, Material::Trim, 2);
    assert!(inner_z != flat_z);
    edit(&mut p, |o| o.extras.spec.lintel.interior = false);
    // Exterior sill under the window, projecting 2" past the face.
    edit(&mut p, |o| o.extras.spec.sill.enabled = true);
    assert_eq!(trim(&p), 12);
    let z = extent(&build_scene(&p), id, Material::Trim, 2);
    assert!((z.1 - z.0 - (2.0 + 0.5)).abs() < 1e-3, "{z:?}");
    // Lintel and sill do not need the casing switch.
    assert!(tris(&build_scene_with(&p, &casing_on()), id, Material::Trim) > 12);
}

#[test]
fn every_arch_shapes_a_window_a_door_and_a_doorway() {
    for kind in [
        ArchType::RoundTop,
        ArchType::Segmental,
        ArchType::Tudor,
        ArchType::Gothic,
        ArchType::Eyebrow,
    ] {
        for (k, style) in [
            (OpeningKind::Window, OpeningStyle::Window),
            (OpeningKind::Window, OpeningStyle::Casement),
            (OpeningKind::Window, OpeningStyle::Fixed),
            (OpeningKind::Door, OpeningStyle::Hinged),
            (OpeningKind::Door, OpeningStyle::DoubleDoor),
            (OpeningKind::Door, OpeningStyle::Doorway),
        ] {
            let (mut p, id) = project(WallKind::Exterior, k, style, 36.0);
            let square = build_scene(&p);
            assert_eq!(tris(&square, id, Material::WallExterior), 0, "{style:?}");
            edit(&mut p, |o| o.extras.spec.arch = Arch { kind, height: 0.0 });
            let scene = build_scene(&p);
            // The corners over the curve are filled with wall, both faces.
            assert!(
                tris(&scene, id, Material::WallExterior) > 0,
                "{kind:?} {style:?}"
            );
            assert!(
                tris(&scene, id, Material::WallInterior) > 0,
                "{kind:?} {style:?}"
            );
            for m in &scene.meshes {
                assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
                assert!(m
                    .vertices
                    .iter()
                    .all(|v| v.position.iter().all(|c| c.is_finite())));
            }
            // Nothing pokes above the hole.
            let o = &p.floors[0].openings[0];
            let top = (o.sill_height + o.height) as f32 + 1e-3;
            let (_, hi) = extent(&scene, id, Material::WindowGlass, 1);
            assert!(
                hi <= top || hi == f32::MIN,
                "{kind:?} {style:?} {hi} > {top}"
            );
        }
    }
}

#[test]
fn an_arched_window_follows_the_curve_with_its_glass() {
    let (mut p, id) = window();
    edit(&mut p, |o| {
        o.width = 36.0;
        o.height = 60.0;
        o.sill_height = 24.0;
    });
    let flat = build_scene(&p);
    edit(&mut p, |o| {
        o.extras.spec.arch = Arch {
            kind: ArchType::RoundTop,
            height: 0.0,
        }
    });
    let arched = build_scene(&p);
    // The glass reaches the top at the middle of the arch only: at the
    // corner it stops at the springline (rise 18").
    let corner_top = arched
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == Material::WindowGlass)
        .flat_map(|m| m.vertices.iter())
        .filter(|v| (v.position[0] - 42.0).abs() < 1.2)
        .map(|v| v.position[1])
        .fold(f32::MIN, f32::max);
    let (_, apex) = extent(&arched, id, Material::WindowGlass, 1);
    assert!(apex > 84.0 - 1.0 - 1e-3 && apex <= 84.0, "apex {apex}");
    assert!(corner_top < 70.0, "corner {corner_top}");
    // Frame, glass and wall-fill pieces all changed the mesh.
    assert_ne!(
        tris(&flat, id, Material::WindowFrame),
        tris(&arched, id, Material::WindowFrame)
    );
    // A rise typed in the tab moves the springline.
    edit(&mut p, |o| {
        o.extras.spec.arch = Arch {
            kind: ArchType::Segmental,
            height: 6.0,
        }
    });
    let shallow = build_scene(&p);
    let corner = shallow
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == Material::WindowGlass)
        .flat_map(|m| m.vertices.iter())
        .filter(|v| (v.position[0] - 42.0).abs() < 1.2)
        .map(|v| v.position[1])
        .fold(f32::MIN, f32::max);
    assert!(corner > corner_top + 5.0, "{corner} vs {corner_top}");
}

#[test]
fn hardware_is_drawn_only_when_asked_and_by_style() {
    let (mut p, id) = door();
    let metal = |p: &Project| tris(&build_scene(p), id, Material::Metal);
    assert_eq!(metal(&p), 0);
    edit(&mut p, |o| o.extras.spec.hardware.enabled = true);
    // Lever on both faces + 3 hinges.
    let lever = metal(&p);
    assert_eq!(lever, (2 * 2 + 3) * 12);
    edit(&mut p, |o| {
        o.extras.spec.hardware.handle = HandleStyle::Knob
    });
    assert_eq!(metal(&p), (2 * 2 + 3) * 12);
    edit(&mut p, |o| {
        o.extras.spec.hardware.handle = HandleStyle::Pull
    });
    assert_eq!(metal(&p), (2 * 3 + 3) * 12);
    edit(&mut p, |o| {
        o.extras.spec.hardware.handle = HandleStyle::None
    });
    assert_eq!(metal(&p), 3 * 12);
    edit(&mut p, |o| {
        o.extras.spec.hardware.handle = HandleStyle::Lever;
        o.extras.spec.hardware.hinges = 0;
    });
    assert_eq!(metal(&p), 2 * 2 * 12);
    // The handle sits at the chosen height, in from the latch edge.
    edit(&mut p, |o| {
        o.extras.spec.hardware.hinges = 0;
        o.extras.spec.hardware.handle_height = 40.0;
        o.extras.spec.hardware.in_from_edge = 3.0;
    });
    let (lo, hi) = extent(&build_scene(&p), id, Material::Metal, 1);
    assert!(
        (lo - 39.0).abs() < 1e-3 && (hi - 41.0).abs() < 1e-3,
        "{lo} {hi}"
    );
    // Hinges on the hinge edge: hinge at the start jamb, x = 60 - 18.
    edit(&mut p, |o| {
        o.extras.spec.hardware.handle = HandleStyle::None;
        o.extras.spec.hardware.hinges = 2;
    });
    let (xlo, xhi) = extent(&build_scene(&p), id, Material::Metal, 0);
    assert!((xlo - 42.0).abs() < 1e-3 && xhi < 43.0, "{xlo} {xhi}");
    // A doorway has no leaf to hang anything on.
    edit(&mut p, |o| o.style = OpeningStyle::Doorway);
    assert_eq!(metal(&p), 0);
}

#[test]
fn shutters_stand_beside_or_over_the_opening_on_the_outside() {
    let (mut p, id) = window();
    edit(&mut p, |o| {
        o.width = 36.0;
        o.extras.spec.shutters.style = ShutterStyle::Panel;
    });
    let trim = |p: &Project| tris(&build_scene(p), id, Material::Trim);
    // Back board + two raised panels each side.
    assert_eq!(trim(&p), 2 * 3 * 12);
    // Beside the window: outside 42..78, 18" each, half an inch off the jamb.
    let (lo, hi) = extent(&build_scene(&p), id, Material::Trim, 0);
    assert!((lo - (42.0 - 0.5 - 18.0)).abs() < 1e-3 && (hi - (78.0 + 0.5 + 18.0)).abs() < 1e-3);
    // Outside the wall face (left = -z in scene space).
    let (zlo, zhi) = extent(&build_scene(&p), id, Material::Trim, 2);
    assert!(zhi - zlo > 0.99 && zhi - zlo < 1.01);
    assert!(
        zlo >= -3.25 - 1e-3 && zhi <= -2.25 + 1e-3 || zlo >= 2.25 - 1e-3,
        "{zlo} {zhi}"
    );
    // Louvers carry slats: more triangles than panels, and fewer with a
    // bigger louver size.
    edit(&mut p, |o| {
        o.extras.spec.shutters.style = ShutterStyle::Louver
    });
    let slats = trim(&p);
    assert!(slats > 2 * 3 * 12);
    edit(&mut p, |o| o.extras.spec.shutters.louver_size = 2.0);
    assert!(trim(&p) < slats);
    // One side only.
    edit(&mut p, |o| {
        o.extras.spec.shutters.sides = ShutterSides::Left
    });
    assert!(trim(&p) < slats);
    // Closed: over the opening.
    edit(&mut p, |o| {
        o.extras.spec.shutters.sides = ShutterSides::Both;
        o.extras.spec.shutters.closed = true;
    });
    let (lo, hi) = extent(&build_scene(&p), id, Material::Trim, 0);
    assert!((lo - 42.0).abs() < 1e-3 && (hi - 78.0).abs() < 1e-3);
    // Gone when the style is.
    edit(&mut p, |o| {
        o.extras.spec.shutters.style = ShutterStyle::None
    });
    assert_eq!(trim(&p), 0);
}

#[test]
fn shutters_appear_in_the_elevation_because_it_is_cut_from_the_scene() {
    // The elevation drawing is made from these meshes, so a shutter that is
    // in the scene is in the elevation: it stands in front of the wall.
    let (mut p, id) = window();
    edit(&mut p, |o| {
        o.extras.spec.shutters.style = ShutterStyle::Panel
    });
    let scene = build_scene(&p);
    let shutter: Vec<&Mesh> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == Material::Trim)
        .collect();
    assert!(!shutter.is_empty());
    let (zlo, _) = extent(&scene, id, Material::Trim, 2);
    assert!(zlo.abs() > 2.0, "in front of the wall face");
}

/// A 10' interior wall with a door and a sidelite window mulled with it.
fn door_and_sidelite(wall: WallKind) -> (Project, Id, Id) {
    let mut p = Project::new("t");
    let w = p.add_wall(0, Point::ZERO, Point::new(120.0, 0.0), 4.5, 100.0, wall);
    let door = p.add_opening(0, w, 40.0, OpeningKind::Door).unwrap();
    let side = p.add_opening(0, w, 80.0, OpeningKind::Window).unwrap();
    for o in &mut p.floors[0].openings {
        if o.id == side {
            o.width = 18.0;
            o.center_offset = 71.0;
            o.sill_height = 0.0;
            o.style = OpeningStyle::Fixed;
        }
        if o.id == door {
            o.width = 36.0;
        }
    }
    (p, door, side)
}

/// Triangles of the casing boards of an object: the trim boxes standing
/// proud of the wall faces (the jamb boards lie inside the wall).
fn casing_tris(scene: &Scene, id: Id) -> usize {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id) && m.material == Material::Trim)
        .map(|m| {
            m.indices
                .chunks(36)
                .filter(|c| {
                    c.iter()
                        .any(|&i| m.vertices[i as usize].position[2].abs() > 2.3)
                })
                .count()
                * 12
        })
        .sum()
}

#[test]
fn a_door_and_its_sidelite_share_one_casing_loop() {
    let (mut p, door, side) = door_and_sidelite(WallKind::Interior);
    let trim = |p: &Project| {
        let s = build_scene_with(p, &casing_on());
        (casing_tris(&s, door), casing_tris(&s, side))
    };
    let (d0, s0) = trim(&p);
    // 4 in apart their casings already touch, so they mull on their own
    // (Round 16) and share one casing: a leg at each end, a strip filling the
    // gap and the head across both, on each face (8 boards in all).
    assert_eq!(d0 + s0, 8 * 12, "door {d0} sidelite {s0}");
    let group = p.mull_openings(0, &[door, side]).unwrap();
    assert_eq!(p.floors[0].openings[0].mull_group, Some(group));
    let (d1, s1) = trim(&p);
    // Mulled: one loop around the unit: a leg at each end and a head across
    // the unit on each face: 6 boxes in all, none between the members.
    assert_eq!(d1 + s1, 6 * 12, "door {d1} sidelite {s1}");
    assert!(d1 + s1 < d0 + s0);
    // The door owns the head; the sidelite only its end leg, one per face.
    assert_eq!(s1, 2 * 12);
    // The casing spans the whole unit: from 3 3/4" left of the door to the
    // same right of the sidelite.
    let (xs_lo, _) = extent(&build_scene_with(&p, &casing_on()), door, Material::Trim, 0);
    let (_, xs_hi) = extent(&build_scene_with(&p, &casing_on()), side, Material::Trim, 0);
    let unit = p.unit_span(0, door).unwrap();
    assert!((xs_lo - (unit.0 as f32 - 3.75)).abs() < 1e-3, "{xs_lo}");
    assert!((xs_hi - (unit.1 as f32 + 3.75)).abs() < 1e-3, "{xs_hi}");
    // The head reaches the top of the door, not the shorter sidelite.
    let (_, head_top) = extent(&build_scene_with(&p, &casing_on()), door, Material::Trim, 1);
    assert!((head_top - (80.0 + 0.25 + 3.5)).abs() < 1e-3, "{head_top}");
    // Unmulled again they still touch, so the casing is still shared.
    p.unmull_openings(0, door);
    assert_eq!(trim(&p).0, 4 * 12);
    // Apart, each is on its own.
    for o in &mut p.floors[0].openings {
        if o.id == side {
            o.center_offset = 100.0;
        }
    }
    assert_eq!(trim(&p).0, 6 * 12);
}

#[test]
fn a_mulled_unit_shares_one_post_between_its_windows() {
    let mut p = Project::new("t");
    let w = p.add_wall(
        0,
        Point::ZERO,
        Point::new(240.0, 0.0),
        4.5,
        100.0,
        WallKind::Exterior,
    );
    let a = p.add_opening(0, w, 60.0, OpeningKind::Window).unwrap();
    let b = p.add_opening(0, w, 100.0, OpeningKind::Window).unwrap();
    for o in &mut p.floors[0].openings {
        o.extras.spec.has_sash = false;
        o.extras.spec.mullion_width = 3.0;
    }
    let edge = |p: &Project, id: Id, hi: bool| {
        let (lo, h) = extent(&build_scene(p), id, Material::WindowGlass, 0);
        if hi {
            h
        } else {
            lo
        }
    };
    // Alone, each has a 3/4" leg: glass stops 3/4" in from the jamb.
    assert!((edge(&p, a, true) - (78.0 - 0.75)).abs() < 1e-3);
    p.mull_openings(0, &[a, b]).unwrap();
    // Mulled: half of the 3" post each side of the joint at 78.
    assert!((edge(&p, a, true) - (78.0 - 1.5)).abs() < 1e-3);
    assert!((edge(&p, b, false) - (78.0 + 1.5)).abs() < 1e-3);
    // The outer ends keep the frame leg.
    assert!((edge(&p, a, false) - (42.0 + 0.75)).abs() < 1e-3);
}

#[test]
fn a_calculated_door_is_double_when_wide() {
    let (mut p, id) = door();
    edit(&mut p, |o| {
        o.width = 60.0;
        o.extras.spec.calc_panels = true;
    });
    let slabs = |p: &Project| {
        build_scene(p)
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(id) && m.material == Material::DoorPanel)
            .map(Mesh::triangle_count)
            .sum::<usize>()
            / 12
    };
    assert_eq!(slabs(&p), 2);
    edit(&mut p, |o| o.width = 30.0);
    assert_eq!(slabs(&p), 1);
}

#[test]
fn the_niche_depth_is_read_from_the_opening() {
    let (mut p, id) = project(
        WallKind::Interior,
        OpeningKind::Window,
        OpeningStyle::WallNiche,
        24.0,
    );
    let _ = id;
    // The recess is cut from the left face; its back plane sits `depth` in.
    let back = |p: &Project| {
        let zs: Vec<f32> = build_scene(p)
            .meshes
            .iter()
            .filter(|m| matches!(m.material, Material::WallInterior | Material::WallExterior))
            .flat_map(|m| m.vertices.iter().map(|v| v.position[2]))
            .collect();
        let mut inner: Vec<f32> = zs.into_iter().filter(|z| z.abs() < 2.25 - 1e-3).collect();
        inner.sort_by(f32::total_cmp);
        inner.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
        inner
    };
    let default = back(&p);
    assert_eq!(default.len(), 1);
    edit(&mut p, |o| o.extras.spec.niche_depth = 2.0);
    let shallow = back(&p);
    assert_eq!(shallow.len(), 1);
    assert!(
        (shallow[0] - default[0]).abs() > 1.0,
        "{shallow:?} {default:?}"
    );
}
