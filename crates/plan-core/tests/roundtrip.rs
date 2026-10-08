//! Robustness sweep, plan-core side: a plan must survive save and load without
//! losing or changing anything.
//!
//! The fixture is a plan with at least one of every object kind and every
//! slot of `Project` and `Floor` filled with something that is not the
//! default (the cabinet, stair, roof, electrical, framing, terrain and
//! layout slots are opaque JSON here; `plan-app`'s `s40_roundtrips` builds the
//! same kinds with the real tools and crates).
//!
//! Tests marked `#[ignore = "QA-nn"]` expose a defect, see `docs/qa-findings.md`.

use plan_core::cad::{ArrowStyle, CadAttrs, CadBlockInfo, FillAttr, PolyArc};
use plan_core::camera::PlanLight;
use plan_core::details::{DetailsLayer, MaterialRegion, Quoin, WallHatch};
use plan_core::dim_assoc::{AnchorAxis, AnchorTarget, DimAnchor, DimAttach};
use plan_core::dimension::DimOverrides;
use plan_core::foundation::{FoundationLayer, Slab};
use plan_core::images::{DistKind, Distribution, ImageSpec, RegionPattern};
use plan_core::layer_sets::{LayerEdit, LayerSetDef};
use plan_core::object_materials::{ClassMaterial, ObjectMaterial, PartMaterial};
use plan_core::schedules::{Schedule, ScheduleKind, ScheduleLayer};
use plan_core::underlay::Underlay;
use plan_core::{
    CadItem, CameraKind, CameraObject, Dimension, DimensionKind, Floor, FloorKind, FoundationKind,
    ObjectGroup, ObjectRef, Opening, OpeningKind, OpeningStyle, PlacedSymbol, Point, Project,
    RoomName, SavedPlanView, Wall, WallClass, WallCurve, WallKind,
};
use serde_json::{json, Value};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn json_of(p: &Project) -> String {
    p.to_json().expect("a plan serializes")
}

/// Every top-level key of the plan JSON and of the first floor's JSON.
fn keys(v: &Value) -> Vec<String> {
    v.as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

/// A plan with one of everything. Three floors (foundation, main, attic).
fn rich_project() -> Project {
    let mut p = Project::new("Rich fixture \u{2014} d\u{e9}j\u{e0} vu \"quoted\" \\ back");
    p.floors = vec![
        Floor::new("Foundation", -96.0),
        Floor::new("1st Floor", 0.0),
        Floor::new("Attic", 120.0),
    ];
    p.floors[0].kind = FloorKind::Foundation;
    p.floors[2].kind = FloorKind::Attic;
    p.floors[0].settings.foundation = Some(plan_core::floors::FoundationBuild {
        kind: FoundationKind::StemWall { height: 36.0 },
        footing_width: 16.0,
        footing_depth: 8.0,
    });
    p.floors[1].ceiling_height = 120.0;
    p.floors[1].settings.default_room_type = "Kitchen".into();
    p.floors[1].settings.floor_material = "Oak".into();

    // ----- walls: a closed shell, a curved wall, one wall of every class -----
    let corners = [
        pt(0.0, 0.0),
        pt(1200.0, 0.0),
        pt(1200.0, 600.0),
        pt(0.0, 600.0),
    ];
    let mut shell = Vec::new();
    for i in 0..4 {
        shell.push(p.add_wall(
            1,
            corners[i],
            corners[(i + 1) % 4],
            6.5,
            109.125,
            WallKind::Exterior,
        ));
    }
    let curved = p.add_wall(
        1,
        pt(300.0, 700.0),
        pt(600.0, 700.0),
        4.5,
        96.0,
        WallKind::Interior,
    );
    p.floors[1].wall_mut(curved).unwrap().curve = Some(WallCurve { bulge: 60.0 });
    let classes = vec![
        WallClass::Foundation,
        WallClass::Pony {
            upper_type: "Upper".into(),
            lower_type: "Lower".into(),
            split_height: 36.0,
            upper_sets_plan_display: true,
        },
        WallClass::Glass,
        WallClass::GlassPony {
            lower_type: "Lower".into(),
            split_height: 30.0,
        },
        WallClass::HalfWall { height: 42.0 },
        WallClass::RoomDivider,
        WallClass::Railing,
        WallClass::DeckRailing,
        WallClass::DeckEdge,
        WallClass::Fencing {
            style: plan_core::FenceStyle::default(),
        },
    ];
    for (i, class) in classes.into_iter().enumerate() {
        let y = 800.0 + 40.0 * i as f64;
        let id = p.add_wall(1, pt(0.0, y), pt(200.0, y), 4.5, 96.0, WallKind::Interior);
        let w = p.floors[1].wall_mut(id).unwrap();
        w.class = class;
        w.flags.invisible = i % 2 == 0;
        w.flags.no_locate = i % 3 == 0;
        w.flags.half_wall = i == 4;
        w.wall_type = Some(format!("Type {i}"));
        w.bottom_offset = i as f64;
        w.foundation_height = 20.0 + i as f64;
        w.is_deck_edge = i == 8;
        w.layer = "Walls, Normal".into();
    }
    // A long wall that hosts one opening of every style.
    let long = p.add_wall(
        1,
        pt(0.0, 2000.0),
        pt(4000.0, 2000.0),
        6.5,
        109.125,
        WallKind::Exterior,
    );
    let mut offset = 60.0;
    let mut n = 0u32;
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        for style in OpeningStyle::for_kind_list(kind) {
            if let Some(id) = p.add_opening(1, long, offset, kind) {
                let o = p.floors[1]
                    .openings
                    .iter_mut()
                    .find(|o| o.id == id)
                    .unwrap();
                o.style = *style;
                o.swing_flipped = n % 2 == 0;
                o.hinge_at_end = n % 3 == 0;
                o.label_override = (n % 4 == 0).then(|| format!("L{n}"));
                o.schedule_number = Some(format!("S{n}"));
                o.lites = (n % 3 + 1, n % 2 + 1);
                o.egress = n % 5 == 0;
                o.tempered = n % 2 == 1;
                o.mull_group = (n % 6 == 0).then_some(900 + u64::from(n));
                o.casing = Some(plan_core::Casing::default());
                n += 1;
            }
            offset += 110.0;
        }
    }
    assert!(n >= 20, "only {n} openings fit");
    // A door on the curved wall.
    let _ = p.add_opening(1, curved, 150.0, OpeningKind::Door);
    // Openings on the other floors' walls are not needed; the attic gets a wall.
    p.add_wall(
        2,
        pt(0.0, 0.0),
        pt(400.0, 0.0),
        4.5,
        60.0,
        WallKind::Interior,
    );

    // ----- rooms -----
    let mut rn = RoomName::new(pt(600.0, 300.0), "Great Room", "Living");
    rn.ceiling_height = Some(144.0);
    rn.floor_finish = Some("Hardwood".into());
    rn.ceiling_finish = Some("Paint".into());
    rn.include_in_living_area = Some(true);
    rn.rough_ceiling = Some(110.0);
    rn.conditioned = Some(true);
    rn.stem_wall_height = Some(24.0);
    rn.floor_height_offset = 3.0;
    rn.has_ceiling = false;
    rn.moldings.push(plan_core::extras::MoldingRef {
        kind: plan_core::extras::MoldingKind::Crown,
        profile: "Crown 3".into(),
        height: 3.5,
    });
    rn.misc = Some(plan_core::extras::RoomMisc::default());
    rn.fill_style = Some(plan_core::RoomFill {
        color: [10, 20, 30],
        pattern: "Hatch".into(),
        alpha: 0.5,
    });
    p.floors[1].room_names.push(rn);
    p.floors[1]
        .room_names
        .push(RoomName::new(pt(100.0, 100.0), "Pantry", "Pantry"));

    // ----- dimensions of every kind -----
    for (i, kind) in [
        DimensionKind::Manual,
        DimensionKind::AutoExterior,
        DimensionKind::Temporary,
    ]
    .into_iter()
    .enumerate()
    {
        let mut d = Dimension::new(
            0,
            kind,
            pt(0.0, 0.0),
            pt(100.0 + 10.0 * i as f64, 0.0),
            12.0,
        );
        d.text_override = (i == 0).then(|| "OVERRIDE".to_string());
        d.hide_ext = [i == 1, i == 2];
        d.text_style = Some("Dimension Text Style".into());
        d.anchors = [
            Some(DimAnchor {
                wall: shell[0],
                target: AnchorTarget::Wall,
                at: DimAttach::Along(0.25),
                side: 3.0,
                last: pt(1.0, 2.0),
                axis: AnchorAxis::X,
                ..sample_anchor()
            }),
            Some(DimAnchor {
                target: AnchorTarget::Opening,
                at: DimAttach::Local { u: 1.5, v: 2.5 },
                ..sample_anchor()
            }),
        ];
        d.look = DimOverrides {
            fraction: Some(16),
            decimals: Some(3),
            arrow_size: Some(5.0),
            ext_gap: Some(2.0),
            ..DimOverrides::default()
        };
        p.add_dimension(1, d);
    }

    // ----- CAD: every primitive, blocks, attributes, groups -----
    let items = vec![
        CadItem::Line {
            a: pt(0.0, 0.0),
            b: pt(10.0, 10.0),
        },
        CadItem::Arc {
            center: pt(5.0, 5.0),
            radius: 5.0,
            start_angle: 0.25,
            end_angle: 2.5,
        },
        CadItem::Circle {
            center: pt(50.0, 50.0),
            radius: 7.5,
        },
        CadItem::Polyline {
            points: vec![pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0), pt(5.0, 14.0)],
            closed: true,
        },
        CadItem::Text {
            pos: pt(20.0, 20.0),
            text: "Note \u{2014} \"q\" \\ \u{e9}\n2nd line".into(),
            height: 6.0,
            angle: 0.5,
        },
    ];
    let mut cad_ids = Vec::new();
    for it in items {
        cad_ids.push(p.add_cad(1, "CAD, Default", it));
    }
    let poly = cad_ids[3];
    p.floors[1].cad_attrs.push(CadAttrs {
        target: poly,
        color: Some([1, 2, 3]),
        weight: Some(35),
        dash: Some(plan_core::LineStyle::Dashed),
        fill: Some(FillAttr {
            lines: vec![cad_ids[0]],
            ..FillAttr::default()
        }),
        arrow_start: ArrowStyle::Filled,
        arrow_end: ArrowStyle::Dot,
        arrow_size: 4.0,
        text_style: Some("Default Text Style".into()),
        arc_edges: vec![PolyArc {
            from: 0,
            to: 1,
            bulge: 0.25,
        }],
        ..CadAttrs::default()
    });
    let gid = p.alloc_id();
    p.floors[1].groups.push(ObjectGroup {
        id: gid,
        members: vec![
            ObjectRef::Cad(cad_ids[0]),
            ObjectRef::Cad(cad_ids[1]),
            ObjectRef::Wall(shell[0]),
        ],
    });
    p.floors[1].cad_blocks.push(CadBlockInfo {
        group: gid,
        name: "Block A".into(),
        insertion: Some(pt(1.0, 1.0)),
        backoff: Some(pt(2.0, 2.0)),
    });

    // ----- placed symbols, an image and a distribution -----
    let sym = p.add_symbol(
        1,
        PlacedSymbol::new("cat.chair", pt(400.0, 400.0), 20.0, 20.0, 36.0),
    );
    let mut pic = PlacedSymbol::new("image.tree", pt(500.0, 400.0), 40.0, 1.0, 80.0);
    pic.image = Some(ImageSpec::new("/tmp/does-not-exist.png", 64, 64));
    pic.solid = true;
    pic.owner = Some(sym);
    let mut dist = PlacedSymbol::new("cat.plant", pt(600.0, 400.0), 12.0, 12.0, 30.0);
    dist.distribution = Some(Distribution::new(
        DistKind::Region,
        false,
        vec![pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 100.0)],
        "cat.plant",
        [12.0, 12.0, 30.0],
    ));
    if let Some(d) = dist.distribution.as_mut() {
        d.pattern = RegionPattern::Random;
        d.seed = 99;
    }
    p.add_symbol(1, pic);
    p.add_symbol(1, dist);

    // ----- opaque slots -----
    let f = &mut p.floors[1];
    f.cabinets = vec![json!({"id": 7001, "kind": "Base", "x": 1.5, "name": "caf\u{e9}"})];
    f.stairs = vec![json!({"id": 7002, "shape": "LShaped", "risers": 14})];
    f.roofs = vec![
        json!({"kind": "plane", "id": 7003, "pitch": 8.0}),
        json!({"kind": "settings", "overhang": 12.0}),
    ];
    f.electrical = Some(json!({"devices": [{"id": 7004, "kind": "Outlet110"}]}));
    f.framing = vec![json!({"id": 7005, "member": "Joist"})];
    f.foundation = None;
    let mut layer = FoundationLayer::default();
    let mut slab = Slab::new(7006, vec![pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 100.0)]);
    slab.holes = vec![vec![pt(10.0, 10.0), pt(20.0, 10.0), pt(20.0, 20.0)]];
    layer.slabs.push(slab);
    layer.store(f);
    let mut det = DetailsLayer::default();
    det.quoins.push(Quoin {
        id: 7007,
        ..Quoin::default()
    });
    det.regions.push(MaterialRegion {
        id: 7008,
        outline: vec![pt(0.0, 0.0), pt(5.0, 0.0), pt(5.0, 5.0)],
        ..MaterialRegion::default()
    });
    det.hatches.push(WallHatch {
        id: 7009,
        wall_id: shell[0],
        ..WallHatch::default()
    });
    det.store(f);
    let mut sched = ScheduleLayer::default();
    sched.add(Schedule::new(ScheduleKind::Door, pt(1300.0, 0.0)));
    sched.add(Schedule::new(ScheduleKind::Window, pt(1300.0, 200.0)));
    sched.store(f);
    f.underlays.push(Underlay::new(
        "Survey",
        "/tmp/survey.png",
        800,
        600,
        pt(0.0, 0.0),
    ));
    f.detail = Some(plan_core::details::CadDetailInfo::default());
    f.settings.ceiling_structure_thickness = 11.0;

    // ----- project level -----
    let cam_kinds = [
        CameraKind::FullCamera,
        CameraKind::PerspectiveOverview,
        CameraKind::DollHouse,
        CameraKind::CrossSection {
            back_clip: Some(120.0),
        },
        CameraKind::WallElevation,
        CameraKind::Orthographic,
        CameraKind::Elevation,
        CameraKind::Walkthrough,
        CameraKind::FloorCamera,
        CameraKind::GlassHouse,
        CameraKind::FramingOverview,
    ];
    for (i, k) in cam_kinds.into_iter().enumerate() {
        let mut c = CameraObject::new(k, pt(10.0 * i as f64, 5.0), 45.0, format!("Cam {i}"), 1);
        if i == 7 {
            c.path = vec![pt(0.0, 0.0), pt(50.0, 0.0), pt(50.0, 50.0)];
        }
        p.add_camera(c);
    }
    let mut light = PlanLight::new(pt(300.0, 300.0), 96.0);
    light.cast_shadows = true;
    p.add_light(1, light);
    p.light_options.use_electrical = false;
    p.lighting.sun_azimuth_deg = 123.0;
    p.lighting.from_date = Some(plan_core::camera_view::SunDate {
        month: 6,
        day: 21,
        hours: 14.5,
        latitude: 33.7,
    });
    let mut set = LayerSetDef::new("Electrical");
    set.apply("Walls, Normal", &LayerEdit::Display(false));
    set.apply("Walls, Normal", &LayerEdit::Color([9, 8, 7]));
    set.apply("Walls, Normal", &LayerEdit::Reference(true));
    p.layer_sets.sets.push(set);
    p.layer_sets.active = "Electrical".into();
    p.plan_views
        .push(SavedPlanView::new("Electrical Plan", "Electrical"));
    p.plan_views[0].camera = Some((pt(5.0, 6.0), 0.75));
    p.plan_views[0].floor = Some(1);
    p.active_plan_view = "Electrical Plan".into();
    p.text_styles.styles.push(plan_core::TextStyle {
        name: "Custom".into(),
        italic: true,
        ..plan_core::TextStyle::default()
    });
    p.text_macros.add("job", "Job %plan.name%");
    p.note_types.types.push(plan_core::text_styles::NoteType {
        name: "Extra".into(),
        prefix: "X".into(),
        style: "Custom".into(),
    });
    p.info.client_name = "Client \u{c9}".into();
    p.info.client_address = vec!["1 Main St".into(), "Atlanta".into()];
    p.info.revisions = vec![("1".into(), "2026-10-01".into(), "Issued".into())];
    p.info.custom = vec![("k".into(), "v".into())];
    p.object_materials.push(ObjectMaterial {
        object: shell[0],
        parts: vec![PartMaterial {
            part: "Exterior Wall Surface".into(),
            material: "Brick".into(),
        }],
    });
    p.material_defaults.push(ClassMaterial {
        class: "Wall".into(),
        part: String::new(),
        material: "Stucco".into(),
    });
    p.wall_types.push(plan_core::WallTypeDef {
        name: "Custom wall".into(),
        layers: vec![
            plan_core::WallLayer::new("Siding", 1.0, false, "Wood"),
            plan_core::WallLayer::new("Frame", 5.5, true, "Lumber"),
        ],
        kind: WallKind::Exterior,
    });
    p.opening_display =
        serde_json::from_value(json!({"casing": false, "doors_open": true})).unwrap();
    p.terrain =
        Some(json!({"points": [[0.0, 0.0, 1.0]], "features": [], "roads": [], "plants": []}));
    p.layout = Some(json!({"name": "Layout", "pages": [{"boxes": [{"kind": "Plan"}]}]}));
    p.layout_files = vec![json!({"name": "Second"})];
    p.electrical_defaults = Some(json!({"outlet_height": 16.0}));
    p
}

fn sample_anchor() -> DimAnchor {
    DimAnchor {
        wall: 1,
        target: AnchorTarget::Wall,
        at: DimAttach::Start,
        side: 0.0,
        last: pt(0.0, 0.0),
        axis: AnchorAxis::Both,
        extra: 1.25,
    }
}

#[test]
fn the_fixture_has_every_slot_filled() {
    let p = rich_project();
    let v = serde_json::to_value(&p).unwrap();
    let fresh = serde_json::to_value(Project::new("x")).unwrap();
    // Every project field differs from a new plan's, including the ones a new
    // plan leaves out of its JSON (`skip_serializing_if`).
    let mut missing: Vec<String> = Vec::new();
    for k in keys(&fresh) {
        // `name`, `floors` and `next_id` are set by every plan; the layers and
        // opening display have no non-default case worth the check.
        if ["layers"].contains(&k.as_str()) {
            continue;
        }
        if v[&k] == fresh[&k] {
            missing.push(k);
        }
    }
    for k in ["layout_files", "material_defaults", "electrical_defaults"] {
        if v.get(k).is_none() {
            missing.push(k.into());
        }
    }
    assert!(
        missing.is_empty(),
        "project slots at their default: {missing:?}"
    );
    let f = &v["floors"][1];
    let fresh_floor = serde_json::to_value(Floor::new("1st Floor", 0.0)).unwrap();
    let mut missing: Vec<String> = Vec::new();
    for k in keys(&fresh_floor) {
        // `kind` is set on the foundation and attic floors instead.
        if ["name", "elevation", "kind"].contains(&k.as_str()) {
            continue;
        }
        if f[&k] == fresh_floor[&k] {
            missing.push(k);
        }
    }
    assert!(
        missing.is_empty(),
        "floor slots at their default: {missing:?}"
    );
    assert!(v["floors"][2]["kind"] != fresh_floor["kind"]);
    assert!(v["floors"][0]["settings"]["foundation"].is_object());
    assert!(v["floors"][1].get("detail").is_some());
    // Every opening style is on the fixture's long wall.
    let styles: std::collections::BTreeSet<String> = p.floors[1]
        .openings
        .iter()
        .map(|o| format!("{:?}", o.style))
        .collect();
    for s in OpeningStyle::DOORS
        .iter()
        .chain(OpeningStyle::WINDOWS.iter())
    {
        assert!(styles.contains(&format!("{s:?}")), "no {s:?} opening");
    }
}

#[test]
fn save_load_save_is_byte_identical() {
    let p = rich_project();
    let a = json_of(&p);
    let q = Project::from_json(&a).expect("the saved plan loads");
    let b = json_of(&q);
    assert_eq!(a, b, "second save differs from the first");
    // And a third generation, so nothing creeps.
    let r = Project::from_json(&b).unwrap();
    assert_eq!(b, json_of(&r));
    // Semantically equal: the typed values, not just the text.
    assert_eq!(format!("{p:?}"), format!("{q:?}"));
    assert_eq!(
        serde_json::to_value(&p).unwrap(),
        serde_json::to_value(&q).unwrap()
    );
}

#[test]
fn save_load_through_the_file_functions_is_byte_identical() {
    let dir = std::env::temp_dir().join(format!("plan-core-roundtrip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = rich_project();
    let f1 = dir.join("a.psplan");
    let f2 = dir.join("b.psplan");
    plan_core::io::save_project(&p, &f1).unwrap();
    let q = plan_core::io::load_project(&f1).unwrap();
    plan_core::io::save_project(&q, &f2).unwrap();
    assert_eq!(std::fs::read(&f1).unwrap(), std::fs::read(&f2).unwrap());
    // No temporary file is left next to them.
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ids_survive_and_new_ids_do_not_collide_after_a_reload() {
    let p = rich_project();
    let mut q = Project::from_json(&json_of(&p)).unwrap();
    let used: std::collections::BTreeSet<u64> = q
        .floors
        .iter()
        .flat_map(|f| {
            f.walls
                .iter()
                .map(|w| w.id)
                .chain(f.openings.iter().map(|o| o.id))
                .chain(f.dimensions.iter().map(|d| d.id))
                .chain(f.cad.iter().map(|c| c.id))
                .chain(f.symbols.iter().map(|s| s.id))
                .chain(f.groups.iter().map(|g| g.id))
        })
        .chain(q.cameras.iter().map(|c| c.id))
        .collect();
    for _ in 0..50 {
        let id = q.alloc_id();
        assert!(!used.contains(&id), "alloc_id handed out the used id {id}");
    }
}

#[test]
fn an_empty_plan_round_trips() {
    let p = Project::new("");
    let a = json_of(&p);
    let q = Project::from_json(&a).unwrap();
    assert_eq!(a, json_of(&q));
}

#[test]
fn unicode_and_awkward_text_survive() {
    let mut p =
        Project::new("\u{1F3E0} \u{4F4F}\u{5B85} \u{202E}rtl\u{202C} \0 nul \t tab \r\n crlf");
    p.info.client_name = "Zo\u{eb} \"O'Brien\" \\ / \u{2028}".into();
    p.floors[0].name = "\u{1F9F1} floor / 1".into();
    p.add_cad(
        0,
        "CAD, Default",
        CadItem::Text {
            pos: Point::ZERO,
            text: "a\u{0}b\u{7f}c\u{85}d".into(),
            height: 6.0,
            angle: 0.0,
        },
    );
    let a = json_of(&p);
    let q = Project::from_json(&a).unwrap();
    assert_eq!(q.name, p.name);
    assert_eq!(q.info.client_name, p.info.client_name);
    assert_eq!(a, json_of(&q));
}

#[test]
fn extreme_but_finite_numbers_survive_exactly() {
    let mut p = Project::new("n");
    let vals = [
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
        5e-324,
        -0.0,
        0.1 + 0.2,
        1.0 / 3.0,
        123_456_789.012_345_68,
        1e21,
        1e-7,
    ];
    for (i, v) in vals.iter().enumerate() {
        p.add_wall(0, pt(*v, -*v), pt(i as f64, *v), *v, *v, WallKind::Interior);
    }
    let q = Project::from_json(&json_of(&p)).unwrap();
    for (a, b) in p.floors[0].walls.iter().zip(&q.floors[0].walls) {
        assert_eq!(a.start.x.to_bits(), b.start.x.to_bits(), "{:?}", a.start);
        assert_eq!(a.start.y.to_bits(), b.start.y.to_bits());
        assert_eq!(a.thickness.to_bits(), b.thickness.to_bits());
    }
}

// ----- legacy and forward compatibility -----

#[test]
fn the_oldest_wall_only_file_loads_with_every_new_slot_defaulted() {
    // The shape of phase 0: a wall had no layer, flags, curve or roof, an
    // opening no style.
    let old = r#"{"name":"phase0","floors":[{"name":"1st Floor","elevation":0.0,
        "ceiling_height":109.125,
        "walls":[{"id":1,"start":{"x":0.0,"y":0.0},"end":{"x":100.0,"y":0.0},
                  "thickness":6.5,"height":109.125,"kind":"Exterior"}],
        "openings":[{"id":2,"wall_id":1,"center_offset":50.0,"width":36.0,"height":60.0,
                     "sill_height":24.0,"kind":"Window"},
                    {"id":3,"wall_id":1,"center_offset":20.0,"width":30.0,"height":80.0,
                     "sill_height":0.0,"kind":"Door"}]}],"next_id":4}"#;
    let p = Project::from_json(old).expect("phase 0 file loads");
    let f = &p.floors[0];
    assert_eq!(f.walls[0].layer, "Walls, Normal");
    assert_eq!(f.openings[0].style, OpeningStyle::Window);
    assert_eq!(f.openings[1].style, OpeningStyle::Hinged);
    assert_eq!(f.kind, FloorKind::Normal);
    assert!(p.layers.get("Doors").is_some());
    assert!(!p.plan_views.is_empty());
    // And once loaded it is stable under save and load.
    let a = json_of(&p);
    assert_eq!(a, json_of(&Project::from_json(&a).unwrap()));
}

#[test]
fn a_file_with_an_unknown_wall_class_cannot_be_read_and_says_so() {
    // A plan saved by a newer build with a wall class this build lacks must
    // fail with a message, never panic.
    let mut v = serde_json::to_value(rich_project()).unwrap();
    v["floors"][1]["walls"][0]["class"] = json!({"FutureClass": {"x": 1}});
    let r = Project::from_json(&v.to_string());
    let e = r.expect_err("an unknown enum variant is an error");
    assert!(e.to_string().contains("FutureClass") || e.to_string().contains("variant"));
}

#[test]
fn unknown_extra_keys_do_not_stop_a_file_from_loading() {
    let mut v = serde_json::to_value(rich_project()).unwrap();
    v["from_the_future"] = json!({"a": [1, 2, 3]});
    v["floors"][1]["future_slot"] = json!("x");
    v["floors"][1]["walls"][0]["future_wall_field"] = json!(1.5);
    v["floors"][1]["openings"][0]["future_opening_field"] = json!(null);
    v["floors"][1]["dimensions"][0]["future_dim_field"] = json!([]);
    v["cameras"][0]["future_camera_field"] = json!(true);
    let p = Project::from_json(&v.to_string()).expect("unknown keys are ignored, not an error");
    assert_eq!(p.name, rich_project().name);
    assert_eq!(
        p.floors[1].walls.len(),
        rich_project().floors[1].walls.len()
    );
}

#[test]
#[ignore = "QA-20"]
fn unknown_extra_keys_are_kept_when_the_plan_is_saved_again() {
    // Forward compatibility: open a newer build's plan, save it, and the
    // newer build's data must still be there.
    let mut v = serde_json::to_value(rich_project()).unwrap();
    v["from_the_future"] = json!({"a": [1, 2, 3]});
    v["floors"][1]["future_slot"] = json!("x");
    v["floors"][1]["walls"][0]["future_wall_field"] = json!(1.5);
    let p = Project::from_json(&v.to_string()).unwrap();
    let saved: Value = serde_json::from_str(&json_of(&p)).unwrap();
    assert_eq!(saved["from_the_future"], v["from_the_future"]);
    assert_eq!(
        saved["floors"][1]["future_slot"],
        v["floors"][1]["future_slot"]
    );
    assert_eq!(
        saved["floors"][1]["walls"][0]["future_wall_field"],
        v["floors"][1]["walls"][0]["future_wall_field"]
    );
}

#[test]
#[ignore = "QA-21"]
fn an_empty_json_object_loads_as_an_empty_plan() {
    // `name`, `floors` and `next_id` have no serde default, so `{}` (an empty
    // or truncated-to-braces file) is an error rather than an empty plan.
    let p = Project::from_json("{}").expect("`{}` loads as an empty plan");
    assert!(!p.floors.is_empty(), "a plan always has a floor");
}

#[test]
fn a_file_missing_its_floors_is_an_error_not_a_panic() {
    for text in [
        "",
        "null",
        "[]",
        "{\"name\":\"x\"}",
        "{\"floors\":[]}",
        "\u{feff}{}",
        "{",
    ] {
        let _ = Project::from_json(text);
    }
}

#[test]
#[ignore = "QA-22"]
fn a_stale_next_id_does_not_hand_out_an_id_that_is_in_use() {
    // Files written by an older build, or edited by hand, can carry a
    // `next_id` at or below the ids already used.
    let mut v = serde_json::to_value(rich_project()).unwrap();
    v["next_id"] = json!(1);
    let mut p = Project::from_json(&v.to_string()).unwrap();
    let used: Vec<u64> = p.floors[1].walls.iter().map(|w| w.id).collect();
    for _ in 0..100 {
        let id = p.alloc_id();
        assert!(!used.contains(&id), "alloc_id reused wall id {id}");
    }
}

#[test]
#[ignore = "QA-23"]
fn a_non_finite_number_never_makes_the_saved_plan_unreadable() {
    // serde_json writes NaN and infinity as `null`, and `null` is not a valid
    // f64 on the way back in: one stray NaN (a zero-length division in a tool)
    // makes the whole saved plan unloadable, and the atomic save has already
    // replaced the good file with it.
    let mut p = Project::new("nan");
    p.add_wall(
        0,
        pt(0.0, 0.0),
        pt(f64::NAN, 5.0),
        6.5,
        109.0,
        WallKind::Exterior,
    );
    p.add_wall(
        0,
        pt(0.0, 0.0),
        pt(f64::INFINITY, 5.0),
        6.5,
        109.0,
        WallKind::Exterior,
    );
    // Either saving refuses (an Err), or what it writes loads again.
    if let Ok(text) = p.to_json() {
        Project::from_json(&text).expect("what to_json wrote must load again");
    }
}

#[test]
fn save_project_refuses_or_keeps_the_old_file_when_the_plan_cannot_be_written() {
    let dir = std::env::temp_dir().join(format!("plan-core-rt-bad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("keep.psplan");
    plan_core::io::save_project(&rich_project(), &f).unwrap();
    let before = std::fs::read(&f).unwrap();
    // A folder that does not exist: the save fails and the file is untouched.
    let bad = dir.join("nope").join("x.psplan");
    assert!(plan_core::io::save_project(&rich_project(), &bad).is_err());
    assert_eq!(std::fs::read(&f).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_truncated_file_is_an_error_and_never_a_partial_plan() {
    let text = json_of(&rich_project());
    // Cut at a spread of byte offsets (on char boundaries).
    let mut cuts = 0;
    for frac in 1..40 {
        let mut at = text.len() * frac / 40;
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        assert!(
            Project::from_json(&text[..at]).is_err(),
            "a plan cut at byte {at} of {} loaded",
            text.len()
        );
        cuts += 1;
    }
    assert!(cuts > 30);
}

#[test]
fn a_wall_that_lost_its_opening_host_still_loads_and_saves() {
    // Dangling references (an opening whose wall is gone, a group member that
    // does not exist, a dimension anchored to a deleted wall) must load.
    let mut p = rich_project();
    let gone = 999_999;
    p.floors[1]
        .openings
        .push(Opening::default_door(888_888, gone, 30.0));
    p.floors[1].groups.push(ObjectGroup {
        id: 888_889,
        members: vec![ObjectRef::Wall(gone), ObjectRef::Cabinet(gone)],
    });
    let a = json_of(&p);
    let q = Project::from_json(&a).unwrap();
    assert_eq!(a, json_of(&q));
}

#[test]
fn wall_helpers_do_not_misbehave_on_a_zero_length_wall() {
    // A degenerate wall can come from a damaged file; its derived numbers
    // must be finite so a save of the plan stays loadable.
    let w = Wall::new(pt(5.0, 5.0), pt(5.0, 5.0), 4.5, 96.0, WallKind::Interior);
    for v in [
        w.length(),
        w.direction().x,
        w.direction().y,
        w.normal().x,
        w.normal().y,
        w.point_at(10.0).x,
        w.path_length(),
    ] {
        assert!(v.is_finite(), "{v}");
    }
}
