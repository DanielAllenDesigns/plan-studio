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
        lift: 0.0,
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
