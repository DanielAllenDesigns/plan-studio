//! Scenario 18: doors and windows from the flyouts into the 3D scene: the
//! parts each variant builds, the selection after placement, window labels,
//! the Sash, Arch and Shutters tabs in 3D, a door jamb handle and a typed
//! width, and a door mulled with its sidelite (DW-8, DW-12, DW-24, DW-26,
//! DW-38..DW-52, DW-59..DW-63, DW-98, DW-107).

use super::{draw_shell, Sim};
use crate::editor::handles::{self, HandleKind};
use crate::editor::opening_edit;
use crate::editor::opening_view::opening_labels;
use crate::editor::tempdim::{self, TempDimKind};
use crate::editor::ObjectRef;
use crate::shell::view3d_panel::{build_view_scene, ViewScope};
use crate::toolbar;
use crate::tools::ToolId;
use crate::ActiveDialog;
use plan_3d::{Material, Mesh, Scene};
use plan_core::geometry::Point;
use plan_core::openings::{ArchType, ShutterStyle};
use plan_core::schedules::ScheduleKind;
use plan_core::{Id, Opening, OpeningKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

fn scene(sim: &Sim) -> Scene {
    build_view_scene(&sim.app.cx.project, &ViewScope::default())
}

fn tris(scene: &Scene, id: Id, m: Material) -> usize {
    scene
        .meshes
        .iter()
        .filter(|x| x.object_id == Some(id) && x.material == m)
        .map(Mesh::triangle_count)
        .sum()
}

fn all_tris(scene: &Scene, id: Id) -> usize {
    scene
        .meshes
        .iter()
        .filter(|x| x.object_id == Some(id))
        .map(Mesh::triangle_count)
        .sum()
}

fn pick(sim: &mut Sim, name: &str, flyout: &toolbar::Flyout) {
    let it = flyout
        .entries
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no flyout entry {name}"));
    assert!(it.enabled, "{name} is dimmed");
    sim.action(it.action);
}

fn opening(sim: &Sim, id: Id) -> Opening {
    sim.app
        .cx
        .floor()
        .openings
        .iter()
        .find(|o| o.id == id)
        .unwrap()
        .clone()
}

fn newest(sim: &Sim) -> Opening {
    sim.app
        .cx
        .floor()
        .openings
        .iter()
        .max_by_key(|o| o.id)
        .unwrap()
        .clone()
}

/// (flyout entry, materials its 3D parts must include, parts it must not)
type Parts = (&'static str, &'static [Material], &'static [Material]);

const DOOR_PARTS: &[Parts] = &[
    ("Hinged Door", &[Material::DoorPanel], &[Material::Glass]),
    ("Double Door", &[Material::DoorPanel], &[Material::Glass]),
    // An open doorway has no leaf: only the wall opening (and any casing).
    ("Doorway", &[], &[Material::DoorPanel, Material::Glass]),
    ("Sliding Door", &[Material::DoorPanel, Material::Glass], &[]),
    ("Pocket Door", &[Material::DoorPanel], &[Material::Glass]),
    ("Bifold Door", &[Material::DoorPanel], &[Material::Glass]),
    ("Barn Door", &[Material::DoorPanel, Material::Metal], &[]),
    ("Garage Door", &[Material::DoorPanel, Material::Metal], &[]),
    ("Shower Door", &[Material::Glass, Material::Metal], &[]),
];

const WINDOW_PARTS: &[Parts] = &[
    (
        "Window",
        &[Material::WindowFrame, Material::WindowGlass],
        &[Material::DoorPanel],
    ),
    (
        "Bay Window",
        &[Material::WindowFrame, Material::WindowGlass, Material::Roof],
        &[],
    ),
    (
        "Bow Window",
        &[Material::WindowFrame, Material::WindowGlass, Material::Roof],
        &[],
    ),
    (
        "Box Window",
        &[Material::WindowFrame, Material::WindowGlass, Material::Roof],
        &[],
    ),
    // Pass-throughs and niches are holes: no glass.
    ("Pass-Through", &[], &[Material::WindowGlass]),
    ("Wall Niche", &[], &[Material::WindowGlass]),
    (
        "Casement Window",
        &[Material::WindowFrame, Material::WindowGlass],
        &[],
    ),
    (
        "Fixed Window",
        &[Material::WindowFrame, Material::WindowGlass],
        &[],
    ),
    (
        "Sliding Window",
        &[Material::WindowFrame, Material::WindowGlass],
        &[],
    ),
    (
        "Awning Window",
        &[
            Material::WindowFrame,
            Material::WindowGlass,
            Material::Metal,
        ],
        &[],
    ),
    (
        "Hopper Window",
        &[
            Material::WindowFrame,
            Material::WindowGlass,
            Material::Metal,
        ],
        &[],
    ),
];

fn check_parts(flyout: toolbar::Flyout, cases: &[Parts]) {
    for (name, must, must_not) in cases {
        let mut sim = house();
        pick(&mut sim, name, &flyout);
        sim.click(240.0, 0.0);
        let o = newest(&sim);
        let s = scene(&sim);
        for m in *must {
            assert!(tris(&s, o.id, *m) > 0, "{name}: no {m:?} in 3D");
        }
        for m in *must_not {
            assert_eq!(tris(&s, o.id, *m), 0, "{name}: unexpected {m:?} in 3D");
        }
        // Every variant but the plain holes builds something of its own.
        if !must.is_empty() {
            assert!(all_tris(&s, o.id) >= 12, "{name}");
        }
        // The wall behind it is cut: the wall meshes differ from a plain wall.
        let wall_tris: usize = s
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(o.wall_id))
            .map(Mesh::triangle_count)
            .sum();
        let plain = house();
        let wid = plain.app.cx.floor().walls[0].id;
        let plain_tris: usize = scene(&plain)
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(wid))
            .map(Mesh::triangle_count)
            .sum();
        assert!(
            wall_tris > plain_tris,
            "{name}: the wall has no opening cut"
        );
    }
}

#[test]
fn every_door_flavor_builds_its_3d_parts() {
    check_parts(toolbar::door(), DOOR_PARTS);
}

#[test]
fn every_window_flavor_builds_its_3d_parts() {
    check_parts(toolbar::window(), WINDOW_PARTS);
}

#[test]
fn bigger_flavors_have_more_geometry_than_the_plain_ones() {
    let count = |flyout: &toolbar::Flyout, name: &str| {
        let mut sim = house();
        pick(&mut sim, name, flyout);
        sim.click(240.0, 0.0);
        all_tris(&scene(&sim), newest(&sim).id)
    };
    let doors = toolbar::door();
    assert!(
        count(&doors, "Double Door") > count(&doors, "Hinged Door"),
        "two leaves"
    );
    assert!(
        count(&doors, "Bifold Door") > count(&doors, "Hinged Door"),
        "folded panels"
    );
    assert!(
        count(&doors, "Sliding Door") > count(&doors, "Hinged Door"),
        "two panels and glass"
    );
    assert!(
        count(&doors, "Garage Door") > count(&doors, "Hinged Door"),
        "sections"
    );
    let windows = toolbar::window();
    let plain = count(&windows, "Window");
    assert!(count(&windows, "Bay Window") > plain);
    assert!(count(&windows, "Bow Window") > count(&windows, "Bay Window"));
    assert!(count(&windows, "Sliding Window") > plain, "two sashes");
    assert!(count(&windows, "Fixed Window") < plain, "no sash");
}

#[test]
fn the_opening_just_placed_is_the_selection_for_every_flavor() {
    for (flyout, kind) in [
        (toolbar::door(), OpeningKind::Door),
        (toolbar::window(), OpeningKind::Window),
    ] {
        for e in &flyout.entries {
            let mut sim = house();
            sim.action(e.action);
            sim.click(120.0, 0.0);
            let first = newest(&sim);
            assert_eq!(first.kind, kind, "{}", e.name);
            assert_eq!(
                sim.app.cx.selection.items,
                vec![ObjectRef::Opening(first.id)],
                "{}",
                e.name
            );
            // A second one takes over the selection; the first is let go.
            sim.click(360.0, 0.0);
            let second = newest(&sim);
            assert_ne!(first.id, second.id);
            assert_eq!(
                sim.app.cx.selection.items,
                vec![ObjectRef::Opening(second.id)],
                "{}",
                e.name
            );
        }
    }
}

fn label_of(sim: &mut Sim, id: Id) -> Option<(String, bool)> {
    sim.app.cx.refresh();
    opening_labels(&sim.app.cx)
        .into_iter()
        .find(|l| l.opening == id)
        .map(|l| (l.text, l.is_mark))
}

#[test]
fn a_window_label_shows_its_size_then_its_mark_and_follows_a_resize() {
    let mut sim = house();
    sim.tool(ToolId::Window);
    sim.click(240.0, 0.0);
    let id = newest(&sim).id;
    // 32" x 72" default window with a 24" sill: 2'-8" x 6'-0".
    let (w, h) = (opening(&sim, id).width, opening(&sim, id).height);
    let (text, is_mark) = label_of(&mut sim, id).expect("a label");
    assert!(!is_mark);
    assert!(text.chars().all(|c| c.is_ascii_digit()), "{text}");
    assert_eq!(text.len(), 4, "{text} for {w} x {h}");

    // A resize through the dialog changes the size text.
    sim.tool(ToolId::Select);
    sim.double_click(240.0, 0.0);
    let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() else {
        panic!("the Window Specification opens")
    };
    d.draft_mut().width = 60.0;
    sim.ok();
    let (wider, _) = label_of(&mut sim, id).unwrap();
    assert_ne!(wider, text, "the label follows the new width");
    assert!(wider.starts_with("50"), "5'-0\" wide: {wider}");

    // A Window Schedule numbers it; the label turns into the mark.
    let sid = crate::editor::schedule_view::add(
        &mut sim.app.cx,
        ScheduleKind::Window,
        Point::new(0.0, -90.0),
    );
    let mut def = crate::editor::schedule_view::find(&sim.app.cx, sid).unwrap();
    def.show_labels = true;
    assert!(crate::editor::schedule_view::replace(
        &mut sim.app.cx,
        0,
        def
    ));
    let (mark, is_mark) = label_of(&mut sim, id).unwrap();
    assert!(is_mark, "{mark}");
    assert!(mark.starts_with('W'), "{mark}");
    // Door labels keep their size: the Window Schedule does not number doors.
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    let door = newest(&sim).id;
    let (dl, door_is_mark) = label_of(&mut sim, door).unwrap();
    assert!(!door_is_mark, "{dl}");
}

#[test]
fn the_sash_arch_and_shutters_tabs_change_the_3d_geometry_and_undo_restores_it() {
    let mut sim = house();
    sim.tool(ToolId::Window);
    sim.click(240.0, 0.0);
    let id = newest(&sim).id;
    sim.tool(ToolId::Select);
    let base = scene(&sim);
    let (frame0, glass0, all0) = (
        tris(&base, id, Material::WindowFrame),
        tris(&base, id, Material::WindowGlass),
        all_tris(&base, id),
    );
    let edit = |sim: &mut Sim, f: &dyn Fn(&mut Opening)| {
        assert!(sim.open_spec(ObjectRef::Opening(id)));
        let Some(ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() else {
            panic!("the Window Specification opens")
        };
        f(d.draft_mut());
        sim.ok();
        assert_eq!(sim.app.cx.undo_label(), Some("Opening Specification"));
    };

    // Shutters: louvered ones add parts beside the window.
    edit(&mut sim, &|o| {
        o.extras.spec.shutters.style = ShutterStyle::Louver
    });
    let louver = all_tris(&scene(&sim), id);
    assert!(louver > all0, "louvered shutters: {all0} -> {louver}");
    edit(&mut sim, &|o| {
        o.extras.spec.shutters.style = ShutterStyle::Panel
    });
    let panel = all_tris(&scene(&sim), id);
    assert!(panel > all0 && panel != louver, "panel shutters {panel}");
    edit(&mut sim, &|o| {
        o.extras.spec.shutters.style = ShutterStyle::None
    });
    assert_eq!(all_tris(&scene(&sim), id), all0);

    // Arch: a round top replaces the flat head with a curve (more facets).
    edit(&mut sim, &|o| o.extras.spec.arch.kind = ArchType::RoundTop);
    let arched = scene(&sim);
    assert!(
        tris(&arched, id, Material::WindowGlass) > glass0,
        "arched glass has more triangles"
    );
    assert!(tris(&arched, id, Material::WindowFrame) > frame0);
    // The hole in the wall stays square; the opening's own mesh gains the
    // spandrel that squares off the corners above the curve.
    assert!(
        all_tris(&arched, id) > all0 + 12,
        "{all0} -> {}",
        all_tris(&arched, id)
    );
    edit(&mut sim, &|o| o.extras.spec.arch.kind = ArchType::None);

    // Sash off: the sash frame parts go and the glass stays.
    edit(&mut sim, &|o| o.extras.spec.has_sash = false);
    let no_sash = scene(&sim);
    assert!(
        tris(&no_sash, id, Material::WindowFrame) < frame0,
        "frame without sashes"
    );
    assert_eq!(tris(&no_sash, id, Material::WindowGlass), glass0);

    // Undo steps back through every edit to the original geometry.
    for _ in 0..6 {
        assert_eq!(sim.undo().as_deref(), Some("Opening Specification"));
    }
    let back = scene(&sim);
    assert_eq!(all_tris(&back, id), all0);
}

#[test]
fn a_door_jamb_handle_and_a_typed_width_resize_the_leaf_in_3d() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(240.0, 0.0);
    let id = newest(&sim).id;
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Opening(id));
    let leaf_width = |sim: &Sim| -> f32 {
        let s = scene(sim);
        let (lo, hi) = s
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(id) && m.material == Material::DoorPanel)
            .filter_map(Mesh::bounds)
            .fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), (l, h)| {
                (
                    [lo[0].min(l[0]), lo[1].min(l[1]), lo[2].min(l[2])],
                    [hi[0].max(h[0]), hi[1].max(h[1]), hi[2].max(h[2])],
                )
            });
        // The leaf is thin: its long horizontal extent is the width.
        (hi[0] - lo[0]).max(hi[2] - lo[2])
    };
    let before = opening(&sim, id);
    let w0 = leaf_width(&sim);
    // Drag the end jamb handle 12" out.
    let end = handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in)
        .into_iter()
        .find(|h| h.kind == HandleKind::ResizeEnd)
        .expect("a jamb handle on the door")
        .pos;
    sim.drag((end.x, end.y), (end.x + 12.0, end.y));
    let dragged = opening(&sim, id);
    assert_eq!(dragged.start_offset(), before.start_offset());
    assert!(
        (dragged.width - before.width - 12.0).abs() <= 1.0,
        "{} -> {}",
        before.width,
        dragged.width
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Resize Opening"));
    let w1 = leaf_width(&sim);
    assert!(w1 > w0 + 8.0, "the leaf widened in 3D: {w0} -> {w1}");

    // The width temp dimension: type 42 and the door resizes about its center.
    sim.app.cx.selection.set(ObjectRef::Opening(id));
    sim.app.cx.refresh();
    let i = sim
        .app
        .cx
        .temp
        .dims
        .iter()
        .position(|d| d.kind == TempDimKind::OpeningWidth)
        .expect("a width temporary dimension");
    let centre = opening(&sim, id).center_offset;
    assert!(sim.app.cx.temp.begin_edit(i));
    sim.app.cx.temp.editing.as_mut().unwrap().text = "42".into();
    assert_eq!(
        tempdim::commit_edit(&mut sim.app.cx).unwrap(),
        "Resize Opening"
    );
    let typed = opening(&sim, id);
    assert_eq!(typed.width, 42.0);
    assert!((typed.center_offset - centre).abs() < 1e-9);
    assert!(
        (leaf_width(&sim) - 42.0).abs() < 4.0,
        "{}",
        leaf_width(&sim)
    );
}

#[test]
fn a_door_and_the_window_beside_it_mull_into_one_unit_and_unmull_apart() {
    let mut sim = house();
    sim.tool(ToolId::Door);
    sim.click(200.0, 0.0);
    let door = newest(&sim).id;
    sim.tool(ToolId::Window);
    sim.click(240.0, 0.0);
    let side = newest(&sim).id;
    let d = opening(&sim, door);
    let w = opening(&sim, side);
    assert!(w.start_offset() - d.end_offset() < 24.0 && w.start_offset() > d.end_offset());
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Opening(door));
    // A door with a swing mulls with the window beside it: the Edit toolbar says so.
    let labels: Vec<_> = sim
        .app
        .tools
        .active()
        .edit_toolbar(&sim.app.cx)
        .into_iter()
        .map(|b| b.label)
        .collect();
    assert!(labels.contains(&"Mull"), "{labels:?}");
    sim.app.cx.run_custom(opening_edit::MULL);
    let (d2, w2) = (opening(&sim, door), opening(&sim, side));
    assert!(d2.mull_group.is_some() && d2.mull_group == w2.mull_group);
    assert_eq!(
        d2.end_offset(),
        w2.start_offset(),
        "the mullion closes the gap"
    );
    // Both still have their own 3D parts.
    let s = scene(&sim);
    assert!(tris(&s, door, Material::DoorPanel) > 0);
    assert!(tris(&s, side, Material::WindowGlass) > 0);
    sim.app.cx.run_custom(opening_edit::UNMULL);
    assert!(opening(&sim, door).mull_group.is_none());
    assert!(opening(&sim, side).mull_group.is_none());
    // One undo per command.
    assert_eq!(sim.undo().as_deref(), Some("Unmull Windows"));
    assert!(opening(&sim, door).mull_group.is_some());
}
