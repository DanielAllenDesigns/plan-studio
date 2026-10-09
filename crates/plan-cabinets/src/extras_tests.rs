//! Tests of the later cabinet additions: library types, label macros, edge
//! profiles, full-height backsplashes, hardware and the 3D opening
//! indicators.

use crate::*;
use plan_3d::{Material, Mesh};
use plan_core::geometry::Point;

fn triangles(ms: &[Mesh]) -> usize {
    ms.iter().map(|m| m.indices.len() / 3).sum()
}

fn of_material(ms: &[Mesh], m: Material) -> Vec<&Mesh> {
    ms.iter().filter(|x| x.material == m).collect()
}

fn top_triangles(edge: EdgeProfile) -> usize {
    let mut c = Cabinet::base(36.0);
    let t = c.countertop.as_mut().unwrap();
    t.edge = edge;
    t.edge_size = 0.75;
    triangles(
        &of_material(&meshes(&c), Material::Floor)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>(),
    )
}

#[test]
fn edge_profiles_build_more_triangles_the_rounder_they_are() {
    let square = top_triangles(EdgeProfile::Square);
    let bevel = top_triangles(EdgeProfile::Beveled);
    let bull = top_triangles(EdgeProfile::Bullnose);
    let ogee = top_triangles(EdgeProfile::Ogee);
    assert_eq!(square, 12, "a plain slab is a box");
    assert!(bevel > square, "{bevel} vs {square}");
    assert!(bull > bevel, "{bull} vs {bevel}");
    assert!(ogee > bull, "{ogee} vs {bull}");
    assert_eq!(EdgeProfile::ALL.len(), 5);
}

#[test]
fn a_waterfall_edge_drops_a_slab_to_the_floor_at_both_ends() {
    let mut c = Cabinet::base(36.0);
    let plain = meshes(&c);
    c.countertop.as_mut().unwrap().edge = EdgeProfile::Waterfall;
    let wf = meshes(&c);
    assert_eq!(wf.len(), plain.len() + 2);
    let tops = of_material(&wf, Material::Floor);
    assert_eq!(tops.len(), 3);
    // The two added slabs reach from the floor to the top surface.
    let tall: Vec<_> = tops
        .iter()
        .filter(|m| {
            let (lo, hi) = m.bounds().unwrap();
            hi[1] - lo[1] > 30.0
        })
        .collect();
    assert_eq!(tall.len(), 2);
    for m in tall {
        let (lo, hi) = m.bounds().unwrap();
        assert!((lo[1] - 0.0).abs() < 1e-3 && (hi[1] - 36.0).abs() < 1e-3);
    }
    // Round-trips through JSON.
    let back: Cabinet = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
    assert_eq!(back.countertop.unwrap().edge, EdgeProfile::Waterfall);
}

#[test]
fn label_macros_expand_to_chief_text() {
    let c = Cabinet::base(36.0);
    assert_eq!(auto_label(&c), "B36");
    assert_eq!(c.display_label(), "B36");
    assert_eq!(expand_label("<L>", &c), "B36");
    assert_eq!(expand_label("<T><W>", &c), "B36");
    assert_eq!(expand_label("<WxDxH>", &c), "36x24x36");
    assert_eq!(expand_label("<WxD>/<H>", &c), "36x24/36");
    assert_eq!(expand_label("<N>", &c), "Base Cabinet");
    assert_eq!(expand_label("<S>", &c), "Lincoln Door");
    assert_eq!(expand_label("<HW>", &c), "Knob");
    let mut w = Cabinet::wall(30.0);
    w.height = 30.0;
    assert_eq!(auto_label(&w), "W3030");
    w.label = "<T>-<W>x<H>".into();
    assert_eq!(w.display_label(), "W-30x30");
    let dw = Cabinet::dishwasher_opening();
    assert_eq!(expand_label("<A>", &dw), "Dishwasher");
}

#[test]
fn the_library_types_have_chiefs_sizes_and_labels() {
    for p in CabinetPreset::ALL {
        let (w, d, h) = p.size();
        let c = Cabinet::from_preset(p, w);
        assert_eq!((c.width, c.depth, c.height), (w, d, h), "{p:?}");
        assert_eq!(c.kind, p.kind(), "{p:?}");
        assert_eq!(c.preset, Some(p));
        assert!(!meshes(&c).is_empty(), "{p:?}");
        let back: Cabinet = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c, "{p:?}");
    }
    let v = Cabinet::vanity(30.0);
    assert_eq!((v.depth, v.height), (21.0, 34.5));
    assert_eq!(v.display_label(), "VB30");
    assert!(v.countertop.is_some() && v.toe_kick.is_some());
    // 30" and wider vanities have a pair of doors under the drawer.
    assert!(v
        .face
        .items
        .iter()
        .any(|i| matches!(i, FaceItem::DoubleDoor { .. })));
    let p = Cabinet::pantry(24.0);
    assert_eq!(p.display_label(), "PN2484");
    assert_eq!(p.height, 84.0);
    let f = Cabinet::refrigerator(36.0);
    assert!(f.toe_kick.is_none() && f.countertop.is_none());
    assert_eq!(f.display_label(), "REF3684");
    assert_eq!(CabinetPreset::TallOven.size(), (30.0, 24.0, 84.0));
}

#[test]
fn the_refrigerator_and_oven_towers_have_appliance_bays() {
    let f = Cabinet::refrigerator(36.0);
    let bays = f.appliance_bays();
    assert_eq!(bays.len(), 1);
    assert_eq!(bays[0].name, "Refrigerator");
    assert!((bays[0].z.1 - bays[0].z.0 - 70.0).abs() < 1e-6);
    assert!(
        (bays[0].z.0 - 0.0).abs() < 1e-6,
        "the bay opens at the floor"
    );
    let o = Cabinet::tall_oven(30.0);
    let names: Vec<_> = o.appliance_bays().into_iter().map(|b| b.name).collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"Oven".to_string()) && names.contains(&"Microwave".to_string()));
    let dw = Cabinet::dishwasher_opening();
    let b = dw.appliance_bays();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].name, "Dishwasher");
    assert!(Cabinet::base(24.0).appliance_bays().is_empty());
}

fn base_with_wall_over(wall_bottom: Option<f64>) -> Vec<Cabinet> {
    let mut b = Cabinet::base(48.0);
    b.id = 1;
    b.backsplash = Some(Backsplash {
        height: 4.0,
        thickness: 0.5,
        full_height: true,
        ..Backsplash::new(4.0, 0.5)
    });
    let mut v = vec![b];
    if let Some(z) = wall_bottom {
        let mut w = Cabinet::wall(36.0);
        w.id = 2;
        w.elevation = z;
        v.push(w);
    }
    v
}

#[test]
fn a_full_height_backsplash_rises_to_the_wall_cabinet_above() {
    let mut cabs = base_with_wall_over(Some(54.0));
    assert_eq!(fit_full_height_backsplashes(&mut cabs), 1);
    assert!((cabs[0].backsplash.unwrap().height - 18.0).abs() < 1e-9);
    assert_eq!(fit_full_height_backsplashes(&mut cabs), 0, "settled");
    // Raise the wall cabinet and it follows.
    cabs[1].elevation = 60.0;
    fit_full_height_backsplashes(&mut cabs);
    assert!((cabs[0].backsplash.unwrap().height - 24.0).abs() < 1e-9);
    // Nothing above: up to the standard 54".
    let mut alone = base_with_wall_over(None);
    fit_full_height_backsplashes(&mut alone);
    assert!((alone[0].backsplash.unwrap().height - (FULL_HEIGHT_TO - 36.0)).abs() < 1e-9);
    // A plain backsplash is left alone.
    let mut plain = base_with_wall_over(Some(54.0));
    plain[0].backsplash.as_mut().unwrap().full_height = false;
    assert_eq!(fit_full_height_backsplashes(&mut plain), 0);
    assert_eq!(plain[0].backsplash.unwrap().height, 4.0);
}

#[test]
fn the_backsplash_mesh_follows_its_height() {
    let mut c = Cabinet::base(48.0);
    c.backsplash = Some(Backsplash::new(4.0, 0.5));
    let h = |c: &Cabinet| {
        let ms = meshes(c);
        let tops = of_material(&ms, Material::Floor);
        tops.iter()
            .map(|m| m.bounds().unwrap().1[1])
            .fold(0.0f32, f32::max)
    };
    assert!((h(&c) - 40.0).abs() < 1e-3);
    c.backsplash.as_mut().unwrap().height = 18.0;
    assert!((h(&c) - 54.0).abs() < 1e-3);
}

#[test]
fn every_handle_style_builds_and_a_pull_is_measured_to_its_near_end() {
    for style in HandleStyle::ALL {
        let mut c = Cabinet::base(30.0);
        c.door_style.handle = style;
        c.drawer_style.handle = style;
        let hw = of_material(&meshes(&c), Material::WindowFrame).len();
        if style == HandleStyle::None {
            assert_eq!(hw, 0);
        } else {
            assert!(hw >= 2, "{style:?} on a door and a drawer");
        }
        assert_eq!(HandleStyle::from_name(style.name()), Some(style));
    }
    assert_eq!(HandleStyle::from_name("bar"), Some(HandleStyle::Pull));
    assert_eq!(HandleStyle::from_name("nonsense"), None);
    // A 4" pull on a wall door starts 2.5" above the door's bottom edge (the
    // door begins above the 1 1/2" rail) and runs up.
    let mut w = Cabinet::wall(15.0);
    w.door_style.handle = HandleStyle::Pull;
    w.door_style.handle_from_top = 2.5;
    w.door_style.handle_length = 4.0;
    let ms = meshes(&w);
    let pull = of_material(&ms, Material::WindowFrame);
    let lo = pull
        .iter()
        .map(|m| m.bounds().unwrap().0[1])
        .fold(f32::MAX, f32::min);
    assert!((lo - (54.0 + 1.5 + 2.5)).abs() < 0.2, "{lo}");
    // Tall doors carry the handle at 38".
    let mut t = Cabinet::pantry(24.0);
    t.door_style.handle = HandleStyle::Knob;
    let ms = meshes(&t);
    let knobs = of_material(&ms, Material::WindowFrame);
    let at38 = knobs.iter().any(|m| {
        let (lo, hi) = m.bounds().unwrap();
        ((lo[1] + hi[1]) / 2.0 - 38.0).abs() < 0.1
    });
    assert!(at38, "a knob at 38 inches on the lower door");
}

#[test]
fn opening_indicators_in_3d_open_the_doors_and_pull_out_the_drawers() {
    let mut c = Cabinet::base(36.0);
    let closed = meshes(&c);
    c.indicators_3d = true;
    let open = meshes(&c);
    assert!(open.len() > closed.len(), "shelves and a drawer box show");
    // The open door leaves the front plane: something reaches past the
    // cabinet's front (depth 24 plus the drawer's pull-out).
    let reach = |ms: &[Mesh]| {
        ms.iter()
            .map(|m| -m.bounds().unwrap().0[2])
            .fold(f32::MIN, f32::max)
    };
    assert!(
        reach(&open) > reach(&closed) + 5.0,
        "{} {}",
        reach(&open),
        reach(&closed)
    );
    // A shelf is a 3/4" board inside the carcass.
    let inside: Vec<_> = open
        .iter()
        .filter(|m| m.material == Material::WallInterior)
        .filter(|m| {
            let (lo, hi) = m.bounds().unwrap();
            (hi[1] - lo[1] - 0.75).abs() < 1e-3 && hi[0] - lo[0] > 20.0 && hi[2] - lo[2] > 15.0
        })
        .collect();
    assert!(!inside.is_empty());
    // Off again, the meshes come back.
    c.indicators_3d = false;
    assert_eq!(meshes(&c).len(), closed.len());
    // A double door opens as two leaves, hinges mirrored.
    let mut s = Cabinet::sink_base(36.0);
    s.indicators_3d = true;
    assert!(meshes(&s).len() > meshes(&Cabinet::sink_base(36.0)).len());
}

#[test]
fn old_cabinets_without_the_new_fields_still_load() {
    let c = Cabinet::base(24.0);
    let mut v = serde_json::to_value(&c).unwrap();
    let o = v.as_object_mut().unwrap();
    for k in ["label_offset", "indicators_3d", "preset"] {
        o.remove(k);
    }
    let back: Cabinet = serde_json::from_value(v).unwrap();
    assert_eq!(back, c);
    // A backsplash stored before the full-height option.
    let b: Backsplash = serde_json::from_str(r#"{"height":4.0,"thickness":0.5}"#).unwrap();
    assert!(!b.full_height);
    let d: DoorStyle = serde_json::from_str(r#"{"name":"Shaker Door"}"#).unwrap();
    assert_eq!(d.handle_length, 4.0);
}

#[test]
fn the_label_offset_moves_the_plan_label() {
    let mut c = Cabinet::base(24.0);
    let at = |c: &Cabinet| match plan_symbol(c).last() {
        Some(Stroke::Text { at, .. }) => *at,
        _ => panic!("label last"),
    };
    let home = at(&c);
    c.label_offset = Point::new(6.0, -3.0);
    let moved = at(&c);
    assert!((moved.x - home.x - 6.0).abs() < 1e-9 && (moved.y - home.y + 3.0).abs() < 1e-9);
}

// ----- Round 16, brief 24: face items, special shapes, exposed ends, tops -----

fn bounds_all(ms: &[Mesh]) -> ([f32; 3], [f32; 3]) {
    ms.iter().filter_map(Mesh::bounds).fold(
        ([f32::MAX; 3], [f32::MIN; 3]),
        |(mut lo, mut hi), (l, h)| {
            for k in 0..3 {
                lo[k] = lo[k].min(l[k]);
                hi[k] = hi[k].max(h[k]);
            }
            (lo, hi)
        },
    )
}

/// The farthest the cabinet reaches towards its front (plan +y), inches.
fn reach_front(c: &Cabinet) -> f64 {
    -f64::from(bounds_all(&meshes(c)).0[2])
}

fn face_of(items: Vec<FaceItem>) -> FaceLayout {
    FaceLayout {
        items,
        frame_width: 1.5,
    }
}

#[test]
fn every_item_kind_resolves_in_a_face_and_reports_its_kind() {
    assert_eq!(ItemKind::ALL.len(), 18);
    let mut labels: Vec<&str> = ItemKind::ALL.iter().map(|k| k.label()).collect();
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(labels.len(), 18);
    for kind in ItemKind::ALL {
        let item = kind.make(0.0);
        assert_eq!(item.kind(), kind, "{}", kind.label());
        if !kind.is_leaf() {
            continue;
        }
        // Each leaf kind between two separations takes the face height left.
        let layout = face_of(vec![
            FaceItem::Separation { height: 1.5 },
            kind.make(0.0),
            FaceItem::Separation { height: 1.5 },
        ]);
        let r = layout.resolve(30.0, 24.0).unwrap();
        let sum: f64 = r.iter().map(|f| f.rect.3).sum();
        assert!((sum - 30.0).abs() < 1e-9, "{}: {sum}", kind.label());
        if kind != ItemKind::Separation {
            assert!(r.iter().any(|f| f.item.kind() == kind), "{}", kind.label());
        }
        // And the cabinet builds and keeps its meshes outward.
        let mut c = Cabinet::base(24.0);
        c.face = layout;
        assert!(!meshes(&c).is_empty());
    }
}

#[test]
fn layouts_nest_horizontal_inside_vertical_and_back() {
    let drawer = |h| FaceItem::FalseDrawer { height: h };
    let layout = face_of(vec![FaceItem::VerticalLayout {
        height: 0.0,
        items: vec![
            drawer(6.0),
            FaceItem::HorizontalLayout {
                height: 0.0,
                cells: vec![
                    FaceCell {
                        item: FaceItem::Rollout { height: 0.0 },
                        width: Some(10.0),
                    },
                    FaceCell {
                        item: FaceItem::DoorLeft { height: 0.0 },
                        width: None,
                    },
                ],
            },
        ],
    }]);
    let r = layout.resolve(30.0, 40.0).unwrap();
    assert_eq!(r.len(), 3);
    assert_eq!(r[0].rect, (0.0, 24.0, 40.0, 6.0));
    assert_eq!(r[1].rect, (0.0, 0.0, 10.0, 24.0));
    assert_eq!(r[2].rect, (10.0, 0.0, 30.0, 24.0));
    assert_eq!(r[0].item.kind(), ItemKind::FalseDrawer);
    assert!(layout.has_appliance("none") == false);
}

#[test]
fn auto_doors_split_above_the_threshold() {
    let right = FaceItem::DoorAuto { height: 0.0 };
    let left = FaceItem::DoorAutoLeft { height: 0.0 };
    assert_eq!(right.door_plan(18.0, 24.0), DoorPlan::Single { left: false });
    assert_eq!(right.door_plan(24.0, 24.0), DoorPlan::Single { left: false });
    assert_eq!(right.door_plan(24.5, 24.0), DoorPlan::Pair);
    assert_eq!(left.door_plan(12.0, 24.0), DoorPlan::Single { left: true });
    assert_eq!(left.door_plan(36.0, 24.0), DoorPlan::Pair);
    // Explicit doors ignore the width.
    assert_eq!(
        FaceItem::DoorLeft { height: 0.0 }.door_plan(60.0, 24.0),
        DoorPlan::Single { left: true }
    );
    assert_eq!(FaceItem::DoubleDoor { height: 0.0 }.door_plan(10.0, 24.0), DoorPlan::Pair);
    assert_eq!(FaceItem::Drawer { height: 0.0 }.door_plan(10.0, 24.0), DoorPlan::None);
    // The threshold is the cabinet's: more doors on a 48" cabinet with a
    // lower one.
    let mut c = Cabinet::wall(36.0);
    let pair = meshes(&c).len();
    c.auto_door_threshold = 40.0;
    assert!(meshes(&c).len() < pair);
}

#[test]
fn custom_items_wrap_and_unwrap_and_reach_the_resolved_face() {
    let mut item = FaceItem::DoorLeft { height: 12.0 };
    assert!(item.props().is_none());
    item.props_mut().unwrap().locked = true;
    assert!(matches!(item, FaceItem::Custom { .. }));
    assert_eq!(item.height(), 12.0);
    assert_eq!(item.kind(), ItemKind::LeftDoor);
    let layout = face_of(vec![item.clone(), FaceItem::Drawer { height: 0.0 }]);
    let r = layout.resolve(30.0, 24.0).unwrap();
    assert!(r[0].props.as_ref().is_some_and(|p| p.locked));
    assert!(r[1].props.is_none());
    assert!(matches!(r[0].item, FaceItem::DoorLeft { .. }));
    item.props_mut().unwrap().locked = false;
    item.tidy();
    assert!(matches!(item, FaceItem::DoorLeft { .. }));
    // A layout takes no settings.
    let mut grid = FaceItem::HorizontalLayout {
        height: 0.0,
        cells: Vec::new(),
    };
    assert!(grid.props_mut().is_none());
    let wrapped = FaceItem::Drawer { height: 0.0 }.with_props(ItemProps {
        percent_open: Some(40.0),
        ..ItemProps::default()
    });
    assert_eq!(wrapped.props().unwrap().percent_open, Some(40.0));
    // It survives the plan file.
    let json = serde_json::to_string(&wrapped).unwrap();
    assert_eq!(serde_json::from_str::<FaceItem>(&json).unwrap(), wrapped);
}

#[test]
fn per_item_style_and_percent_open_change_the_meshes() {
    let mut c = Cabinet::base(24.0);
    c.face = face_of(vec![
        FaceItem::Separation { height: 1.5 },
        FaceItem::Drawer { height: 6.0 },
        FaceItem::Separation { height: 1.5 },
        FaceItem::DoorLeft { height: 0.0 },
        FaceItem::Separation { height: 1.5 },
    ]);
    let closed = triangles(&meshes(&c));
    // The drawer shown half open grows a drawer box and moves out.
    c.face.items[1].props_mut().unwrap().percent_open = Some(50.0);
    let half = bounds_all(&meshes(&c));
    let open = triangles(&meshes(&c));
    assert!(open > closed);
    c.face.items[1].props_mut().unwrap().percent_open = Some(100.0);
    let full = bounds_all(&meshes(&c));
    assert!(full.0[2] < half.0[2] - 1.0, "pulled further out");
    // A door of its own style: a shaker door on one door only.
    let mut d = Cabinet::base(24.0);
    let base = triangles(&meshes(&d));
    let mut shaker = d.door_style.clone();
    shaker.apply_builtin("Shaker Door");
    let door = d
        .face
        .items
        .iter_mut()
        .find(|i| i.is_door())
        .expect("a door");
    door.props_mut().unwrap().door = Some(shaker);
    assert!(triangles(&meshes(&d)) > base);
    // A door swung 60 degrees stands off the front.
    let mut e = Cabinet::base(24.0);
    let door = e.face.items.iter_mut().find(|i| i.is_door()).unwrap();
    door.props_mut().unwrap().swing_angle = Some(60.0);
    assert!(reach_front(&e) > reach_front(&Cabinet::base(24.0)) + 5.0);
}

#[test]
fn shelves_per_item_follow_the_specification() {
    let mut c = Cabinet::wall(24.0);
    c.indicators_3d = true;
    let auto = meshes(&c).len();
    // Manual: five thin shelves.
    let door = c.face.items.iter_mut().find(|i| i.is_door()).unwrap();
    door.props_mut().unwrap().shelves = ShelfSpec::manual_of(5);
    assert!(meshes(&c).len() > auto);
    // None at all.
    let door = c.face.items.iter_mut().find(|i| i.is_door()).unwrap();
    door.props_mut().unwrap().shelves = ShelfSpec::manual_of(0);
    assert!(meshes(&c).len() < auto);
}

#[test]
fn rollouts_and_false_drawers_build_and_roll_out_when_shown_open() {
    let mut c = Cabinet::base(24.0);
    c.face = face_of(vec![
        FaceItem::Separation { height: 1.5 },
        FaceItem::FalseDrawer { height: 6.0 },
        FaceItem::Separation { height: 1.5 },
        FaceItem::Rollout { height: 0.0 },
        FaceItem::Separation { height: 1.5 },
    ]);
    let closed = bounds_all(&meshes(&c));
    c.show_open.rollouts = true;
    let open = bounds_all(&meshes(&c));
    // The false drawer never opens, the roll-out shelves come forward.
    assert!(open.0[2] < closed.0[2] - 1.0);
    c.show_open = ShowOpen::all();
    c.face.items[3].props_mut().unwrap().shelves = {
        let mut s = ShelfSpec::manual_of(2);
        s.shelves[0].rollout = true;
        s.shelves[0].rollout_amount = 6.0;
        s
    };
    assert!(!meshes(&c).is_empty());
}

#[test]
fn absorbing_height_changes_follows_the_lowest_item() {
    let mut l = FaceLayout::base_default(34.5);
    // The door is the lowest non-separation item and auto: nothing to do.
    l.fit_height(34.5, 40.0).unwrap();
    assert!(l.resolve(40.0, 24.0).is_ok());
    // Fixed items: the lowest takes the change.
    let mut f = face_of(vec![
        FaceItem::Separation { height: 1.5 },
        FaceItem::Drawer { height: 6.0 },
        FaceItem::Separation { height: 1.5 },
        FaceItem::DoorLeft { height: 25.5 },
        FaceItem::Separation { height: 1.5 },
    ]);
    f.fit_height(36.0, 40.0).unwrap();
    assert_eq!(f.items[3].height(), 29.5);
    assert_eq!(f.items[1].height(), 6.0);
    // Locked: the drawer above takes it instead.
    f.items[3].props_mut().unwrap().locked = true;
    f.fit_height(40.0, 42.0).unwrap();
    assert_eq!(f.items[3].height(), 29.5);
    assert_eq!(f.items[1].height(), 8.0);
    // Nothing can take it.
    let mut g = face_of(vec![FaceItem::Separation { height: 1.5 }]);
    assert!(g.fit_height(1.5, 9.0).is_err());
    assert_eq!(g.items.len(), 1);
}

#[test]
fn setting_an_item_height_moves_the_difference_to_the_lowest_item() {
    let mut f = face_of(vec![
        FaceItem::Separation { height: 1.5 },
        FaceItem::Drawer { height: 6.0 },
        FaceItem::Separation { height: 1.5 },
        FaceItem::DoorLeft { height: 0.0 },
        FaceItem::Separation { height: 1.5 },
    ]);
    f.set_item_height(34.5, 1, 9.0).unwrap();
    let r = f.resolve(34.5, 24.0).unwrap();
    assert!((r[1].rect.3 - 9.0).abs() < 1e-9);
    assert!((r[3].rect.3 - 21.0).abs() < 1e-9);
    // Shortening the lowest item leaves a separation and a blank area.
    f.set_item_height(34.5, 3, 15.0).unwrap();
    let kinds: Vec<ItemKind> = f.items.iter().map(FaceItem::kind).collect();
    assert_eq!(&kinds[kinds.len() - 2..], &[ItemKind::Separation, ItemKind::BlankArea]);
    let total: f64 = f.resolve(34.5, 24.0).unwrap().iter().map(|x| x.rect.3).sum();
    assert!((total - 34.5).abs() < 1e-9);
    // Too small is refused and changes nothing.
    let before = f.clone();
    assert!(f.set_item_height(34.5, 1, 0.5).is_err());
    assert_eq!(f, before);
    // Growing the lowest item takes from the one above it.
    let mut g = face_of(vec![
        FaceItem::Drawer { height: 6.0 },
        FaceItem::DoorLeft { height: 10.0 },
    ]);
    g.set_item_height(16.0, 1, 12.0).unwrap();
    assert_eq!(g.items[0].height(), 4.0);
}

#[test]
fn the_bottom_item_decides_the_appliance_garage() {
    assert!(FaceLayout::base_default(34.5).bottom_is_closed());
    let garage = face_of(vec![
        FaceItem::Separation { height: 1.5 },
        FaceItem::DoorAuto { height: 0.0 },
    ]);
    assert!(!garage.bottom_is_closed());
    let mut c = Cabinet::base(24.0);
    c.face = garage;
    assert!(!c.box_has_bottom());
    c.box_construction.bottom = AutoOnOff::On;
    assert!(c.box_has_bottom());
    assert!(!Cabinet::base(24.0).box_has_top());
    assert!(Cabinet::wall(24.0).box_has_top());
    let mut b = Cabinet::base(24.0);
    b.box_construction.top = AutoOnOff::On;
    assert!(b.box_has_top());
}

fn with_special(shape: SpecialShape, amount: f64, w: f64, d: f64) -> Cabinet {
    let mut c = Cabinet::base(w);
    c.depth = d;
    c.special = Some(Special { shape, amount });
    c
}

#[test]
fn a_bow_front_reaches_forward_by_its_bow_and_an_inside_bow_does_not() {
    let plain = reach_front(&Cabinet::base(36.0));
    let bow = reach_front(&with_special(SpecialShape::BowFront, 4.0, 36.0, 24.0));
    let inside = reach_front(&with_special(SpecialShape::BowFront, -4.0, 36.0, 24.0));
    assert!((bow - plain - 4.0).abs() < 0.6, "{bow} vs {plain}");
    assert!((inside - plain).abs() < 0.6, "{inside} vs {plain}");
    // The countertop follows the curve: its outline reaches 4" further.
    let c = with_special(SpecialShape::BowFront, 4.0, 36.0, 24.0);
    let top = c.top_local().unwrap();
    let deepest = top.iter().fold(0.0_f64, |m, p| m.max(p.y));
    assert!((deepest - (24.0 + 4.0 + 1.0)).abs() < 0.5, "{deepest}");
    // The doors follow it too: one chord per item, each a bit forward of
    // the straight front.
    assert!(meshes(&c).len() > meshes(&Cabinet::base(36.0)).len() / 2);
    // The bow depth is limited to half the width.
    let mut d = Cabinet::base(36.0);
    d.convert(CabinetStyle::Special(SpecialShape::BowFront)).unwrap();
    assert!(d.set_special_amount(18.0).is_ok());
    assert!(d.set_special_amount(19.0).is_err());
    assert_eq!(d.special.unwrap().amount, 18.0);
}

#[test]
fn angled_front_cabinets_have_two_depths() {
    let mut c = with_special(SpecialShape::AngledFront, 12.0, 30.0, 24.0);
    let ring = c.footprint_local();
    assert_eq!(ring.len(), 4);
    assert!(ring.iter().any(|p| (p.y - 12.0).abs() < 1e-9 && (p.x - 30.0).abs() < 1e-9));
    let ms = meshes(&c);
    let (lo, hi) = bounds_all(&ms);
    assert!((f64::from(hi[0]) - f64::from(lo[0]) - 30.0).abs() < 1.5, "width");
    // The deepest part is the left side: 24" plus the handle.
    assert!(reach_front(&c) > 24.0 && reach_front(&c) < 26.5);
    c.special.as_mut().unwrap().amount = 24.0;
    assert!((c.footprint_local().iter().map(|p| p.y).fold(0.0, f64::max) - 24.0).abs() < 1e-9);
}

#[test]
fn end_radius_and_peninsula_cabinets_clip_or_round_the_exposed_front() {
    for shape in [
        SpecialShape::LeftEnd,
        SpecialShape::RightEnd,
        SpecialShape::LeftRadiusEnd,
        SpecialShape::RightRadiusEnd,
        SpecialShape::PeninsulaRadius,
    ] {
        let c = with_special(shape, 0.0, 12.0, 24.0);
        let ring = c.footprint_local();
        let area = plan_core::geometry::polygon_area(&ring).abs();
        assert!(area < 12.0 * 24.0 - 5.0, "{shape:?} loses its corner");
        assert!(area > 12.0 * 24.0 - 80.0, "{shape:?}");
        let ms = meshes(&c);
        assert!(!ms.is_empty());
        // Everything stays inside the bounding box of the straight cabinet
        // (plus the handle and the top overhang).
        let (lo, hi) = bounds_all(&ms);
        assert!(f64::from(lo[0]) >= -1.01 && f64::from(hi[0]) <= 13.01, "{shape:?}");
        assert!(reach_front(&c) <= 26.01, "{shape:?}");
    }
    // The right end is cut on the right.
    let r = with_special(SpecialShape::RightEnd, 0.0, 12.0, 24.0);
    let l = with_special(SpecialShape::LeftEnd, 0.0, 12.0, 24.0);
    let corner_cut = |c: &Cabinet, x: f64| c.special.unwrap().depth_at(c.width, c.depth, x);
    assert!(corner_cut(&r, 12.0) < 24.0 && corner_cut(&r, 0.0) == 24.0);
    assert!(corner_cut(&l, 0.0) < 24.0 && corner_cut(&l, 12.0) == 24.0);
}

#[test]
fn converting_a_standard_cabinet_follows_the_manual() {
    let mut c = Cabinet::base(36.0);
    // Width must be greater than depth for a corner.
    c.width = 20.0;
    let err = c.convert(CabinetStyle::Corner).unwrap_err();
    assert!(err.contains("greater than its depth"), "{err}");
    assert_eq!(c.style(), CabinetStyle::Standard);
    c.width = 36.0;
    c.convert(CabinetStyle::Corner).unwrap();
    assert_eq!(c.kind, CabinetKind::CornerBase);
    assert_eq!(c.style(), CabinetStyle::Corner);
    assert!(c.corner.is_some());
    assert!(c.footprint_local().len() >= 5);
    // Back to standard.
    c.convert(CabinetStyle::Standard).unwrap();
    assert_eq!(c.kind, CabinetKind::Base);
    assert!(c.corner.is_none());
    // End cabinets need width <= depth.
    assert!(c.convert(CabinetStyle::Special(SpecialShape::LeftEnd)).is_err());
    c.width = 12.0;
    c.convert(CabinetStyle::Special(SpecialShape::LeftEnd)).unwrap();
    assert_eq!(c.style(), CabinetStyle::Special(SpecialShape::LeftEnd));
    // Fillers and appliance bays do not change style.
    let mut f = Cabinet::filler(CabinetKind::BaseFiller, 3.0);
    assert!(f.convert(CabinetStyle::Special(SpecialShape::BowFront)).is_err());
    let mut dw = Cabinet::dishwasher_opening();
    assert!(dw.convert(CabinetStyle::Corner).is_err());
    // A bowed corner diagonal bulges outward.
    let mut k = Cabinet::corner_base(36.0);
    let straight = k.footprint_local().len();
    k.corner_bow = 3.0;
    assert!(k.footprint_local().len() > straight);
    assert!(!meshes(&k).is_empty());
    assert!(k.top_local().unwrap().len() > straight);
    let far = |c: &Cabinet| {
        c.footprint_local()
            .iter()
            .map(|p| p.x + p.y)
            .fold(f64::MIN, f64::max)
    };
    assert!(far(&k) > far(&Cabinet::corner_base(36.0)) + 1.0);
    k.corner_bow = -3.0;
    assert!(far(&k) <= far(&Cabinet::corner_base(36.0)) + 0.01);
}

#[test]
fn special_shapes_survive_the_plan_file() {
    let mut c = with_special(SpecialShape::BowFront, 3.0, 36.0, 24.0);
    c.box_construction.corner = CornerTreatment::Rounded;
    c.stiles.left = Some(4.0);
    c.top_spec.mitre_waterfall = false;
    let json = serde_json::to_string(&c).unwrap();
    let back: Cabinet = serde_json::from_str(&json).unwrap();
    assert_eq!(back, c);
    // A plain cabinet writes none of the new fields.
    let plain = serde_json::to_value(Cabinet::base(24.0)).unwrap();
    for key in ["special", "box_construction", "top_spec", "stiles", "ends", "show_open"] {
        assert!(plain.get(key).is_none(), "{key} is written for a plain cabinet");
    }
}

#[test]
fn box_corner_treatment_clips_the_exposed_corners() {
    let mut c = Cabinet::base(36.0);
    c.box_construction.corner = CornerTreatment::Clipped;
    c.box_construction.corner_size = 3.0;
    assert!(c.is_shaped());
    // All four corners of a free cabinet are clipped.
    assert_eq!(c.footprint_local().len(), 8);
    // Against a wall at the back and a cabinet at the left only the right
    // front corner is.
    c.ends = Some(Ends {
        left: true,
        right: false,
        back: true,
        back_wall: true,
    });
    assert_eq!(c.footprint_local().len(), 5);
    // Not automatic: the chosen corners only.
    c.box_construction.auto_corners = false;
    c.box_construction.corners = [true, false, false, true];
    assert_eq!(c.footprint_local().len(), 6);
    assert!(!meshes(&c).is_empty());
    assert!(!Cabinet::base(24.0).is_shaped());
}

fn run_of(n: usize, w: f64) -> Vec<Cabinet> {
    (0..n)
        .map(|i| {
            let mut c = Cabinet::base(w);
            c.id = i as u64 + 1;
            c.position = Point::new(w * i as f64, 0.0);
            c.countertop.as_mut().unwrap().overhang_sides = 1.0;
            c
        })
        .collect()
}

#[test]
fn exposed_ends_carry_the_overhang_and_mated_ends_do_not() {
    let mut cabs = run_of(3, 24.0);
    // A wall behind the whole run.
    let wall = vec![
        Point::new(-10.0, -6.0),
        Point::new(100.0, -6.0),
        Point::new(100.0, -0.1),
        Point::new(-10.0, -0.1),
    ];
    let ex = exposures(&cabs, &[wall.clone()]);
    assert_eq!(ex.len(), 3);
    let e = |id: u64| ex.iter().find(|(i, _)| *i == id).unwrap().1;
    assert_eq!(
        (e(1).left, e(1).right, e(1).back, e(1).back_wall),
        (false, true, true, true)
    );
    assert_eq!((e(2).left, e(2).right), (true, true));
    assert_eq!((e(3).left, e(3).right), (true, false));
    assert_eq!(apply_exposures(&mut cabs, &[wall.clone()]), 3);
    assert_eq!(apply_exposures(&mut cabs, &[wall]), 0);
    // The run's outer ends keep the 1" side overhang; the joints and the
    // wall side lose theirs.
    let top = |c: &Cabinet| {
        let t = c.top_local().unwrap();
        (
            t.iter().map(|p| p.x).fold(f64::MAX, f64::min),
            t.iter().map(|p| p.x).fold(f64::MIN, f64::max),
        )
    };
    assert_eq!(top(&cabs[0]), (-1.0, 24.0));
    assert_eq!(top(&cabs[1]), (0.0, 24.0));
    assert_eq!(top(&cabs[2]), (0.0, 25.0));
    // The back overhang is gone against the wall.
    let mut lone = Cabinet::base(24.0);
    lone.countertop.as_mut().unwrap().overhang_back = 1.0;
    let back_y = |c: &Cabinet| c.top_local().unwrap().iter().map(|p| p.y).fold(f64::MAX, f64::min);
    assert_eq!(back_y(&lone), -1.0);
    lone.ends = Some(Ends {
        back: true,
        back_wall: true,
        ..Ends::default()
    });
    assert_eq!(back_y(&lone), 0.0);
}

#[test]
fn closed_toe_and_flat_sides_close_the_toe_space_beside_an_exposed_end() {
    let mut c = Cabinet::base(24.0);
    c.ends = Some(Ends::default());
    let base = meshes(&c).len();
    c.toe_options.closed_toe = true;
    // Both ends are exposed: a closing panel each.
    assert_eq!(meshes(&c).len(), base + 2);
    // Mated on the left: only the right gets one, unless Always Present.
    c.ends = Some(Ends {
        left: true,
        ..Ends::default()
    });
    assert_eq!(meshes(&c).len(), base + 1);
    c.toe_options.closed_toe_always = true;
    assert_eq!(meshes(&c).len(), base + 2);
    // A free cabinet also gets toe kick on its exposed ends and back; Flat
    // Sides and Flat Back take them away.
    let mut free = Cabinet::base(24.0);
    let plain = meshes(&free).len();
    free.ends = Some(Ends::default());
    assert_eq!(meshes(&free).len(), plain + 3);
    free.toe_options.flat_sides = true;
    free.toe_options.flat_back = true;
    assert_eq!(meshes(&free).len(), plain + 2);
}

#[test]
fn back_to_back_cabinets_make_an_island() {
    let mut a = Cabinet::base(48.0);
    a.id = 1;
    a.position = Point::new(0.0, 0.0);
    let mut b = Cabinet::base(48.0);
    b.id = 2;
    // Turned half a turn, its back to a's back.
    b.angle = std::f64::consts::PI;
    b.position = Point::new(48.0, 48.0);
    let ex = exposures(&[a.clone(), b.clone()], &[]);
    assert!(ex.iter().all(|(_, e)| e.back && !e.back_wall && !e.left && !e.right));
    let mut cabs = vec![a, b];
    apply_exposures(&mut cabs, &[]);
    for c in &cabs {
        let t = c.top_local().unwrap();
        assert_eq!(t.iter().map(|p| p.y).fold(f64::MAX, f64::min), 0.0);
    }
}

#[test]
fn a_custom_top_waterfall_drops_to_the_floor() {
    let mut t = Cabinet::custom_countertop(
        &[
            Point::ZERO,
            Point::new(96.0, 0.0),
            Point::new(96.0, 42.0),
            Point::new(0.0, 42.0),
        ],
        1.5,
        36.0,
    )
    .unwrap();
    let plain = meshes(&t);
    assert_eq!(plain.len(), 1);
    t.set_waterfall(1, true);
    t.set_waterfall(3, true);
    let wf = meshes(&t);
    assert_eq!(wf.len(), 3);
    // Each panel runs from the floor to the underside of the top and is as
    // thick as the top.
    for m in &wf[1..] {
        let (lo, hi) = m.bounds().unwrap();
        assert!(lo[1].abs() < 1e-4 && (hi[1] - 34.5).abs() < 1e-4, "{lo:?} {hi:?}");
        let thick = (hi[0] - lo[0]).min(hi[2] - lo[2]);
        assert!((thick - 1.5).abs() < 1e-4, "{thick}");
    }
    // A set height instead.
    t.top_spec.waterfall_auto_height = false;
    t.top_spec.waterfall_height = 10.0;
    let (lo, _) = meshes(&t)[1].bounds().unwrap();
    assert!((lo[1] - 24.5).abs() < 1e-4, "{lo:?}");
}

#[test]
fn islands_peninsulas_and_waterfall_tops_survive_a_json_round_trip() {
    let mut t = Cabinet::custom_countertop(
        &[Point::ZERO, Point::new(60.0, 0.0), Point::new(60.0, 30.0), Point::new(0.0, 30.0)],
        1.5,
        36.0,
    )
    .unwrap();
    t.set_waterfall(2, true);
    t.set_top_edge_molding(0, EdgeMolding::NoMolding, false);
    t.top_spec.selected = 2;
    let back: Cabinet = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
    assert_eq!(back, t);
    assert_eq!(back.waterfall_edges(), vec![2]);
}

#[test]
fn zz_debug_bounds() {
    let c = with_special(SpecialShape::RightRadiusEnd, 0.0, 12.0, 24.0);
    let ms = meshes(&c);
    for m in &ms {
        let (lo, hi) = m.bounds().unwrap();
        println!("{:?} {:?} {:?}", m.material, lo, hi);
    }
}
