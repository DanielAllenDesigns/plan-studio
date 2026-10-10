//! Scenario 40: the robustness sweep. A plan must never lose data.
//!
//! 1. A rich fixture, built with the real tools (every tool id gets the
//!    gestures of `CANDS` on one house) plus the objects no gesture reaches,
//!    holds at least one of every object kind ([`required_kinds`]).
//! 2. Save, load, save gives byte-identical JSON and an equal plan, through
//!    the file functions and through the application's own Save / Open.
//! 3. Every tool gesture and every Edit toolbar command is exactly one undo
//!    step, and do, undo, redo, undo gives back the plan it started from.
//! 4. Legacy files (hidden-layer records, the oldest slot shapes, the sample
//!    plans and their earlier versions in git) load and migrate once.
//! 5. 2000 seeded random gestures never panic, never write a plan that cannot
//!    be read back, and every undo unwind returns to the plan it began from.
//!
//! Defects found are written to `docs/qa-findings.md` (QA-20 onward); the
//! minimal tests that show them are named in each finding and now pass.

use super::{draw_shell, Sim};
use crate::editor::{selection, ObjectRef};
use crate::shell::hotkeys::collect_commands;
use crate::toolbar::Action;
use crate::tools::cad::CadMode;
use crate::tools::dimension::DimMode;
use crate::tools::electrical::ElecVariant;
use crate::tools::foundation::FoundationVariant;
use crate::tools::terrain::TerrainVariant;
use crate::tools::text::TextMode;
use crate::tools::{registry, KeyEvent, ToolId};
use eframe::egui::Key;
use plan_core::{Point, Project, WallKind};
use serde_json::Value;
use std::collections::BTreeSet;
use std::panic::{catch_unwind, AssertUnwindSafe};

const W: f64 = 480.0;
const H: f64 = 360.0;

// ===================================================================
// Gestures
// ===================================================================

/// Every tool id the registry, toolbars, flyouts and menus can pick.
fn all_tool_ids() -> Vec<ToolId> {
    let mut ids: Vec<ToolId> = Vec::new();
    let mut add = |id: ToolId| {
        if !ids.contains(&id) {
            ids.push(id);
        }
    };
    for t in registry() {
        add(t.id());
    }
    for c in collect_commands() {
        if let Action::SetTool(id) = c.action {
            add(id);
        }
    }
    for m in DimMode::ALL {
        add(ToolId::DimensionVariant(m));
    }
    for m in CadMode::ALL {
        add(ToolId::CadVariant(m));
    }
    for m in TextMode::ALL {
        add(ToolId::TextVariant(m));
    }
    for k in crate::editor::stairs_view::StairKind::ALL {
        add(ToolId::StairsVariant(k));
    }
    for v in FoundationVariant::ALL {
        add(ToolId::FoundationVariant(v));
    }
    for k in crate::tools::cabinet::KINDS {
        add(ToolId::CabinetVariant(k));
    }
    for v in ElecVariant::ALL {
        add(ToolId::ElectricalVariant(v));
    }
    for v in TerrainVariant::ALL {
        add(ToolId::TerrainVariant(v));
    }
    for v in crate::tools::details::DetailsVariant::ALL {
        add(ToolId::DetailsVariant(v));
    }
    for v in crate::tools::framing::FramingVariant::ALL {
        add(ToolId::FramingVariant(v));
    }
    for v in crate::tools::images::ImageMode::ALL {
        add(ToolId::ImagesVariant(v));
    }
    for k in crate::tools::schedule::FLYOUT_KINDS {
        add(ToolId::ScheduleVariant(k));
    }
    ids
}

#[derive(Clone, Copy, Debug)]
enum End {
    None,
    Enter,
    Double,
}

#[derive(Clone, Copy, Debug)]
enum G {
    Click(f64, f64),
    Drag((f64, f64), (f64, f64)),
    Clicks(&'static [(f64, f64)], End),
    Text(f64, f64),
}

const TWO: &[(f64, f64)] = &[(100.0, 100.0), (300.0, 100.0)];
const THREE: &[(f64, f64)] = &[(100.0, 100.0), (300.0, 100.0), (300.0, 250.0)];
const FOUR: &[(f64, f64)] = &[
    (100.0, 100.0),
    (300.0, 100.0),
    (300.0, 250.0),
    (100.0, 250.0),
];

const CANDS: &[G] = &[
    G::Click(240.0, 180.0),
    G::Click(200.0, 2.0),
    G::Drag((100.0, 100.0), (300.0, 200.0)),
    G::Drag((100.0, 100.0), (300.0, 100.0)),
    G::Clicks(TWO, End::None),
    G::Clicks(TWO, End::Enter),
    G::Clicks(THREE, End::Enter),
    G::Clicks(THREE, End::Double),
    G::Clicks(FOUR, End::Enter),
    G::Clicks(FOUR, End::Double),
    G::Text(100.0, 200.0),
];

fn perform(sim: &mut Sim, g: G) {
    match g {
        G::Click(x, y) => {
            sim.click(x, y);
        }
        G::Drag(a, b) => {
            sim.drag(a, b);
        }
        G::Clicks(pts, end) => {
            for (x, y) in pts {
                sim.click(*x, *y);
            }
            match end {
                End::None => {}
                End::Enter => {
                    sim.key(KeyEvent::key(Key::Enter));
                }
                End::Double => {
                    let (x, y) = pts[pts.len() - 1];
                    sim.double_click(x, y);
                }
            }
        }
        G::Text(x, y) => {
            sim.click(x, y);
            sim.key(KeyEvent::text("Den"));
            sim.key(KeyEvent::key(Key::Enter));
        }
    }
}

/// What a user does after a gesture: closes a dialog with OK (or Cancel when
/// OK leaves it open), ends the tool and clears the selection.
fn settle(sim: &mut Sim) {
    if sim.app.has_dialog() {
        sim.ok();
        if sim.app.has_dialog() {
            sim.cancel();
        }
    }
    sim.app.dialog = None;
    sim.esc();
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    frame(sim);
}

/// What the shell does at the top of every frame: the derived data is
/// refreshed and the automatic roofs are rebuilt (`PlanApp::update`).
fn frame(sim: &mut Sim) {
    sim.app.cx.refresh();
    if crate::editor::roof_view::auto_rebuild(&mut sim.app.cx) {
        sim.app.cx.refresh();
    }
}

// ===================================================================
// State digests
// ===================================================================

/// The plan as compact JSON text. Two plans are the same when this is.
fn digest(sim: &Sim) -> String {
    serde_json::to_string(&sim.app.cx.project).expect("a plan always serializes")
}

/// The plan as the file holds it (pretty-printed JSON).
fn pretty(sim: &Sim) -> String {
    sim.app
        .cx
        .project
        .to_json()
        .expect("a plan always serializes")
}

/// How many undo steps there are.
fn depth(sim: &Sim) -> usize {
    sim.app.cx.action_history().0.len()
}

/// How many redo steps there are.
fn redo_depth(sim: &Sim) -> usize {
    sim.app.cx.action_history().1.len()
}

/// Puts the plan back to `json` without an undo step (after a defect, so the
/// sweep can go on).
fn restore(sim: &mut Sim, json: &str) {
    let p = Project::from_json(json).expect("a digest loads");
    sim.app.cx.set_project(p);
    sim.app.cx.selection.clear();
    sim.app.cx.refresh();
}

// ===================================================================
// Which object kinds a plan holds
// ===================================================================

/// The variant name of a serialized enum: the string, or the only key.
fn variant(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) if o.len() == 1 => o.keys().next().cloned(),
        _ => None,
    }
}

fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map_or(&[], |a| a.as_slice())
}

/// A set of labels such as `wall`, `wall.curved`, `opening.Garage`,
/// `stair.LShaped`, read from the plan's JSON so every slot (typed or opaque)
/// is looked at the same way.
fn kinds(p: &Project) -> BTreeSet<String> {
    let v = serde_json::to_value(p).expect("serializes");
    let mut out = BTreeSet::new();
    let mut put = |s: String| {
        out.insert(s);
    };
    for f in arr(&v, "floors") {
        put(format!("floor.{}", variant(&f["kind"]).unwrap_or_default()));
        for (label, field) in [
            ("wall", "walls"),
            ("opening", "openings"),
            ("dimension", "dimensions"),
            ("cad", "cad"),
            ("symbol", "symbols"),
            ("cabinet", "cabinets"),
            ("stair", "stairs"),
            ("framing", "framing"),
            ("group", "groups"),
            ("room_name", "room_names"),
            ("cad_attrs", "cad_attrs"),
            ("cad_block", "cad_blocks"),
            ("underlay", "underlays"),
        ] {
            for o in arr(f, field) {
                put(label.to_string());
                for key in ["kind", "style", "class", "item"] {
                    if let Some(k) = o.get(key).and_then(variant) {
                        put(format!("{label}.{k}"));
                    }
                }
                if let Some(k) = o.pointer("/params/shape").and_then(variant) {
                    put(format!("{label}.{k}"));
                }
                if let Some(k) = o.pointer("/stair/params/shape").and_then(variant) {
                    put(format!("{label}.{k}"));
                }
                if o.get("curve").is_some_and(|c| !c.is_null()) {
                    put(format!("{label}.curved"));
                }
                if o.get("image").is_some_and(|c| !c.is_null()) {
                    put(format!("{label}.image"));
                }
                if o.get("distribution").is_some_and(|c| !c.is_null()) {
                    put(format!("{label}.distribution"));
                }
            }
        }
        for o in arr(f, "roofs") {
            if let Some(k) = o.get("kind").and_then(variant) {
                put(format!("roof.{k}"));
            }
            for h in arr(o, "holes") {
                put(format!(
                    "roof.hole.{}",
                    variant(&h["kind"]).unwrap_or_default()
                ));
            }
        }
        if let Some(e) = f.get("electrical").filter(|e| !e.is_null()) {
            for d in arr(e, "devices") {
                put(format!(
                    "device.{}",
                    variant(&d["kind"]).unwrap_or_default()
                ));
            }
            if !arr(e, "connections").is_empty() {
                put("device.connection".into());
            }
        }
        for (label, slot, lists) in [
            (
                "foundation",
                "foundation",
                &["slabs", "holes", "pads", "piers", "platform_holes"][..],
            ),
            (
                "detail",
                "details",
                &[
                    "corner_boards",
                    "quoins",
                    "moldings",
                    "regions",
                    "hatches",
                    "decks",
                    "solids",
                ][..],
            ),
        ] {
            if let Some(e) = f.get(slot).filter(|e| !e.is_null()) {
                for l in lists {
                    if !arr(e, l).is_empty() {
                        put(format!("{label}.{l}"));
                    }
                }
            }
        }
        if let Some(e) = f.get("schedules").filter(|e| !e.is_null()) {
            for s in arr(e, "schedules") {
                put(format!(
                    "schedule.{}",
                    variant(&s["kind"]).unwrap_or_default()
                ));
            }
        }
        if f.get("detail").is_some_and(|d| !d.is_null()) {
            put("floor.cad_detail".into());
        }
    }
    if let Some(t) = v.get("terrain").filter(|t| !t.is_null()) {
        let t = t.get("terrain").unwrap_or(t);
        for l in [
            "elevation_points",
            "elevation_lines",
            "elevation_regions",
            "modifiers",
            "features",
            "roads",
            "breaks",
            "walls",
            "landscape",
        ] {
            if !arr(t, l).is_empty() {
                put(format!("terrain.{l}"));
            }
        }
        if !arr(t, "perimeter").is_empty() {
            put("terrain.perimeter".into());
        }
    }
    for (label, field) in [
        ("camera", "cameras"),
        ("light", "lights"),
        ("object_material", "object_materials"),
        ("material_default", "material_defaults"),
        ("wall_type", "wall_types"),
        ("layer_set", "layer_sets.sets"),
        ("plan_view", "plan_views"),
        ("text_style", "text_styles.styles"),
        ("text_macro", "text_macros.macros"),
        ("note_type", "note_types.types"),
        ("layout_file", "layout_files"),
    ] {
        let node = if let Some((a, b)) = field.split_once('.') {
            v.get(a).and_then(|x| x.get(b))
        } else {
            v.get(field)
        };
        for o in node.and_then(Value::as_array).into_iter().flatten() {
            put(label.to_string());
            if let Some(k) = o.get("kind").and_then(variant) {
                put(format!("{label}.{k}"));
            }
        }
    }
    if let Some(l) = v.get("layout").filter(|l| !l.is_null()) {
        put("layout".into());
        for page in arr(l, "pages") {
            for b in arr(page, "boxes") {
                if let Some(k) = b.get("source").and_then(variant) {
                    put(format!("layout.box.{k}"));
                }
            }
            for key in ["cad", "leaders", "clouds"] {
                if !arr(page, key).is_empty() {
                    put(format!("layout.{key}"));
                }
            }
        }
    }
    for key in ["info", "lighting", "light_options", "opening_display"] {
        put(format!("project.{key}"));
    }
    if v["info"] != serde_json::to_value(Project::new("x").info).unwrap() {
        put("project.info.filled".into());
    }
    if !arr(&v, "electrical_defaults").is_empty() || v.get("electrical_defaults").is_some() {
        put("project.electrical_defaults".into());
    }
    out
}

// ===================================================================
// The rich fixture
// ===================================================================

/// A shell with a door, a window, a partition and a roof.
fn base_house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::Window);
    sim.click(300.0, 0.0);
    sim.tool(ToolId::Wall {
        kind: WallKind::Interior,
    });
    sim.drag((240.0, 0.0), (240.0, H + 1.0));
    sim.tool(ToolId::RoofVariant(crate::tools::roof::RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    settle(&mut sim);
    sim
}

/// Tools the sweep does not drive with gestures: they do not edit the plan.
fn skip_tool(id: ToolId) -> bool {
    matches!(id, ToolId::Select | ToolId::Pan)
}

/// Gives `id` the gestures of `CANDS` one after another; returns the ones
/// that changed the plan. A tool that panics is reported, not fatal.
fn run_tool_on_fixture(sim: &mut Sim, id: ToolId, panics: &mut Vec<String>) -> Vec<G> {
    let mut worked = Vec::new();
    for g in CANDS {
        let d0 = depth(sim);
        let r = catch_unwind(AssertUnwindSafe(|| {
            sim.tool(id);
            perform(sim, *g);
            settle(sim);
        }));
        match r {
            Ok(()) => {
                if depth(sim) > d0 {
                    worked.push(*g);
                    // Two objects per tool keep the fixture (and the sweeps) a size.
                    if worked.len() >= 2 {
                        break;
                    }
                }
            }
            Err(_) => {
                panics.push(format!("{id:?} {g:?}"));
                sim.app.dialog = None;
                sim.tool(ToolId::Select);
            }
        }
    }
    worked
}

// ----- what no gesture reaches -----

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Hole, skylight, dormer, floating dormer and a ceiling plane on the roof
/// of [`base_house`].
fn roof_features(sim: &mut Sim) {
    use crate::editor::roof_view;
    use crate::tools::roof::RoofMode;
    let set = roof_view::load(sim.app.cx.floor());
    let planes: Vec<_> = set.planes.iter().map(|p| (p.id, p.centroid())).collect();
    if let Some((_, c)) = planes.first() {
        sim.tool(ToolId::RoofVariant(RoofMode::Hole));
        sim.drag((c.x - 40.0, c.y - 10.0), (c.x - 20.0, c.y + 10.0));
        sim.tool(ToolId::RoofVariant(RoofMode::Skylight));
        sim.drag((c.x + 20.0, c.y - 12.0), (c.x + 44.0, c.y + 12.0));
    }
    for (i, mode) in [(1, RoofMode::Dormer), (2, RoofMode::FloatingDormer)] {
        if let Some((_, c)) = planes.get(i) {
            sim.tool(ToolId::RoofVariant(mode));
            sim.click(c.x, c.y);
            sim.dialog_frame(true);
            sim.dialog_frame(false);
            settle(sim);
        }
    }
    let _ = roof_view::add_ceiling(
        &mut sim.app.cx.project,
        0,
        (pt(60.0, 60.0), pt(420.0, 60.0), pt(240.0, 180.0)),
        100.0,
        4.0,
    );
    sim.app.cx.refresh();
}

/// Everything the tools cannot be driven into making headless: a wall with
/// one opening of every style, rooms, cameras of every kind, library
/// symbols, CAD blocks and attributes, underlays, material overrides, a
/// layout with every box kind, terrain roads and lines, extra floors.
fn add_direct(sim: &mut Sim) {
    use plan_core::camera::PlanLight;
    use plan_core::images::{DistKind, Distribution, ImageSpec};
    use plan_core::object_materials::{ClassMaterial, ObjectMaterial, PartMaterial};
    use plan_core::{
        CadItem, CameraKind, CameraObject, Floor, FloorKind, ObjectGroup, ObjectRef as CoreRef,
        OpeningKind, OpeningStyle, PlacedSymbol, RoomName,
    };
    let p = &mut sim.app.cx.project;

    // A long wall with one opening of every style.
    let long = p.add_wall(
        0,
        pt(0.0, 1000.0),
        pt(4600.0, 1000.0),
        6.5,
        109.125,
        WallKind::Exterior,
    );
    let mut at = 60.0;
    let mut n = 0u32;
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        for style in OpeningStyle::for_kind_list(kind) {
            if let Some(id) = p.add_opening(0, long, at, kind) {
                if let Some(o) = p.floors[0].openings.iter_mut().find(|o| o.id == id) {
                    o.style = *style;
                    o.swing_flipped = n.is_multiple_of(2);
                    o.hinge_at_end = n.is_multiple_of(3);
                    o.label_override = n.is_multiple_of(4).then(|| format!("L{n}"));
                    o.schedule_number = Some(format!("S{n}"));
                    o.lites = (n % 3 + 1, n % 2 + 1);
                    o.egress = n.is_multiple_of(5);
                    o.tempered = n % 2 == 1;
                    o.mull_group = n.is_multiple_of(6).then_some(9000 + u64::from(n));
                }
                n += 1;
            }
            at += 120.0;
        }
    }

    // Rooms with their extra values.
    let mut rn = RoomName::new(pt(120.0, 180.0), "Great Room", "Living");
    rn.ceiling_height = Some(144.0);
    rn.floor_finish = Some("Hardwood".into());
    rn.include_in_living_area = Some(true);
    rn.has_ceiling = false;
    rn.rough_ceiling = Some(110.0);
    rn.stem_wall_height = Some(24.0);
    p.floors[0].room_names.push(rn);
    p.floors[0]
        .room_names
        .push(RoomName::new(pt(360.0, 180.0), "Pantry", "Pantry"));

    // Cameras of every kind, and a light.
    let kinds = [
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
    for (i, k) in kinds.into_iter().enumerate() {
        let mut c = CameraObject::new(k, pt(20.0 * i as f64, -60.0), 90.0, format!("Cam {i}"), 0);
        if matches!(k, CameraKind::Walkthrough) {
            c.path = vec![pt(0.0, -80.0), pt(200.0, -80.0), pt(200.0, -20.0)];
        }
        p.add_camera(c);
    }
    p.add_light(0, PlanLight::new(pt(240.0, 180.0), 96.0));

    // Library symbols, a picture, a distribution.
    let mut chair = PlacedSymbol::new("cat.chair", pt(100.0, 100.0), 20.0, 20.0, 36.0);
    chair.label = "Chair".into();
    let chair_id = p.add_symbol(0, chair);
    let mut pic = PlacedSymbol::new("image.tree", pt(160.0, 100.0), 40.0, 1.0, 80.0);
    pic.image = Some(ImageSpec::new("/tmp/does-not-exist.png", 64, 64));
    pic.owner = Some(chair_id);
    p.add_symbol(0, pic);
    let mut scatter = PlacedSymbol::new("cat.plant", pt(300.0, 100.0), 12.0, 12.0, 30.0);
    scatter.distribution = Some(Distribution::new(
        DistKind::Path,
        false,
        vec![pt(0.0, 0.0), pt(100.0, 0.0)],
        "cat.plant",
        [12.0, 12.0, 30.0],
    ));
    scatter.solid = true;
    p.add_symbol(0, scatter);

    // CAD: every primitive, an arc and a circle among them, attributes, a block.
    let ids: Vec<_> = [
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
            points: vec![pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0)],
            closed: true,
        },
        CadItem::Text {
            pos: pt(20.0, 20.0),
            text: "Note \u{2014} \"q\" \\ \u{e9}\nline 2".into(),
            height: 6.0,
            angle: 0.5,
        },
    ]
    .into_iter()
    .map(|it| p.add_cad(0, "CAD, Default", it))
    .collect();
    p.floors[0].cad_attrs.push(plan_core::cad::CadAttrs {
        target: ids[3],
        color: Some([1, 2, 3]),
        weight: Some(35),
        arrow_end: plan_core::cad::ArrowStyle::Dot,
        ..plan_core::cad::CadAttrs::default()
    });
    let gid = p.alloc_id();
    p.floors[0].groups.push(ObjectGroup {
        id: gid,
        members: vec![CoreRef::Cad(ids[0]), CoreRef::Cad(ids[1])],
    });
    p.floors[0].cad_blocks.push(plan_core::cad::CadBlockInfo {
        group: gid,
        name: "Block A".into(),
        insertion: Some(pt(1.0, 1.0)),
        backoff: None,
    });
    p.floors[0]
        .underlays
        .push(plan_core::underlay::Underlay::new(
            "Survey",
            "/tmp/survey.png",
            800,
            600,
            pt(0.0, 0.0),
        ));

    // Project-level data.
    let wall0 = p.floors[0].walls[0].id;
    p.object_materials.push(ObjectMaterial {
        object: wall0,
        parts: vec![PartMaterial {
            part: String::new(),
            material: "Brick".into(),
        }],
    });
    p.material_defaults.push(ClassMaterial {
        class: "Wall".into(),
        part: String::new(),
        material: "Stucco".into(),
    });
    p.text_macros.add("job", "Job %plan.name%");
    p.info.client_name = "Client \u{c9}".into();
    p.info.client_address = vec!["1 Main St".into(), "Atlanta".into()];
    p.info.revisions = vec![("1".into(), "2026-10-01".into(), "Issued".into())];
    p.lighting.sun_azimuth_deg = 123.0;
    p.electrical_defaults = Some(serde_json::json!({"outlet_height": 16.0}));
    let mut set = plan_core::LayerSetDef::new("Electrical Only");
    set.apply(
        "Walls, Normal",
        &plan_core::layer_sets::LayerEdit::Display(false),
    );
    p.layer_sets.sets.push(set);
    p.plan_views.push(plan_core::SavedPlanView::new(
        "Electrical Plan",
        "Electrical Only",
    ));

    // Terrain: roads, lines, regions on top of what the tools made.
    {
        use crate::editor::site_view;
        use plan_terrain::{ElevationLine, ElevationRegion, RoadKind, RoadStrip};
        let mut rec = site_view::load_terrain(p).unwrap_or_default();
        for (i, kind) in [
            RoadKind::Road,
            RoadKind::Driveway,
            RoadKind::Sidewalk,
            RoadKind::Marking,
        ]
        .into_iter()
        .enumerate()
        {
            rec.terrain.roads.push(RoadStrip {
                kind,
                centerline: vec![
                    pt(-250.0, -200.0 + 30.0 * i as f64),
                    pt(700.0, -200.0 + 30.0 * i as f64),
                ],
                width: 60.0,
                ..RoadStrip::default()
            });
        }
        rec.terrain.elevation_lines.push(ElevationLine {
            points: vec![pt(-200.0, 400.0), pt(600.0, 450.0)],
            z: 24.0,
            ..ElevationLine::default()
        });
        rec.terrain.elevation_regions.push(ElevationRegion {
            polygon: vec![pt(-200.0, 500.0), pt(-100.0, 500.0), pt(-100.0, 600.0)],
            z: 12.0,
        });
        site_view::save_terrain(p, &rec);
    }

    // Layout: the construction set plus a page with a box of every kind.
    {
        use plan_docs::{Scale, SheetSize};
        use plan_layout::{BoxSource, Layout, LayoutBox, PageLeader, RevisionCloud, ScheduleKind};
        let mut layout = plan_layout::default_construction_set(p, p.floors.len());
        let page = layout.add_page(90, "Every box");
        let sources = vec![
            BoxSource::PlanView {
                floor: 0,
                layer_set: "All".into(),
            },
            BoxSource::Elevation {
                dir: plan_elevation::ViewDir::Front,
            },
            BoxSource::Section {
                cut: plan_elevation::SectionCut {
                    plane_normal: plan_elevation::ViewDir::Left,
                    offset: 120.0,
                },
            },
            BoxSource::Camera { camera_id: 1 },
            BoxSource::Schedule {
                kind: ScheduleKind::Door,
            },
            BoxSource::PlacedSchedule { floor: 0, id: 1 },
            BoxSource::CadDetail {
                name: "Detail".into(),
                items: vec![plan_core::CadObject {
                    id: 1,
                    layer: "CAD, Default".into(),
                    item: CadItem::Line {
                        a: pt(0.0, 0.0),
                        b: pt(9.0, 9.0),
                    },
                }],
            },
            BoxSource::Image {
                path: "x.png".into(),
            },
            BoxSource::ImageData {
                width: 2,
                height: 2,
                rgba: vec![255; 16],
            },
            BoxSource::text("Hello", 12.0),
            BoxSource::Perspective { camera_id: 1 },
            BoxSource::Materials {
                floor: None,
                category: Some("Lumber".into()),
            },
            BoxSource::SheetIndex,
        ];
        for (i, source) in sources.into_iter().enumerate() {
            let x = 1.0 + i as f64;
            page.boxes.push(LayoutBox::new(
                500 + i as u64,
                (pt(x, 1.0), pt(x + 0.8, 1.8)),
                source,
                Scale::QuarterInch,
            ));
        }
        page.add_text(pt(1.0, 5.0), "Paper text", 0.125);
        page.add_rect(pt(1.0, 6.0), pt(2.0, 7.0));
        let lid = page.next_cad_id();
        page.leaders.push(PageLeader {
            id: lid,
            tip: pt(3.0, 3.0),
            elbow: pt(4.0, 4.0),
            text: "See detail".into(),
            height_in: 0.125,
            arrow: true,
            bends: vec![pt(3.5, 3.5)],
        });
        let cid = page.next_cad_id();
        page.clouds.push(RevisionCloud {
            id: cid,
            rect: (pt(5.0, 5.0), pt(6.0, 6.0)),
            revision: "1".into(),
        });
        p.layout = Some(serde_json::to_value(&layout).expect("layout serializes"));
        p.layout_files = vec![
            serde_json::to_value(Layout::new("Second", SheetSize::ArchD))
                .expect("layout serializes"),
        ];
    }

    // One schedule of every kind (the Schedule flyout lacks some).
    {
        use plan_core::schedules::{Schedule, ScheduleKind, ScheduleLayer};
        let mut layer = ScheduleLayer::load(&p.floors[0]);
        for (i, k) in ScheduleKind::ALL.into_iter().enumerate() {
            if !layer.schedules.iter().any(|s| s.kind == k) {
                layer.add(Schedule::new(k, pt(700.0, 20.0 * i as f64)));
            }
        }
        layer.store(&mut p.floors[0]);
    }

    // Foundation and attic floors; a second floor through the menu.
    let mut found = Floor::new("Foundation", -96.0);
    found.kind = FloorKind::Foundation;
    let mut attic = Floor::new("Attic", 240.0);
    attic.kind = FloorKind::Attic;
    attic.detail = Some(plan_core::details::CadDetailInfo::default());
    p.floors.push(found);
    p.floors.push(attic);
    sim.app.cx.refresh();
}

/// The fixture: [`base_house`], every tool's gestures, then [`add_direct`].
/// The history starts empty. Returns the sim and the tools that panicked.
fn rich_sim() -> (Sim, Vec<String>) {
    super::s21_layout_print::isolate_home();
    let mut sim = base_house();
    roof_features(&mut sim);
    let mut panics = Vec::new();
    for id in all_tool_ids() {
        if skip_tool(id) {
            continue;
        }
        run_tool_on_fixture(&mut sim, id, &mut panics);
    }
    add_direct(&mut sim);
    // The first floor is the active one; the history starts here.
    frame(&mut sim);
    let json = digest(&sim);
    restore(&mut sim, &json);
    (sim, panics)
}

/// What the fixture must hold: one of every kind the sweep can name.
fn required_kinds() -> Vec<String> {
    let mut v: Vec<String> = [
        "floor.Normal",
        "floor.Foundation",
        "floor.Attic",
        "floor.cad_detail",
        "wall",
        "wall.curved",
        "wall.Exterior",
        "wall.Interior",
        "wall.Foundation",
        "wall.Pony",
        "wall.Glass",
        "wall.GlassPony",
        "wall.HalfWall",
        "wall.RoomDivider",
        "wall.Railing",
        "wall.DeckRailing",
        "wall.DeckEdge",
        "wall.Fencing",
        "opening",
        "room_name",
        "dimension.Manual",
        "dimension.AutoExterior",
        "cad.Line",
        "cad.Arc",
        "cad.Circle",
        "cad.Polyline",
        "cad.Text",
        "cad_attrs",
        "cad_block",
        "group",
        "symbol",
        "symbol.image",
        "symbol.distribution",
        "underlay",
        "framing",
        "stair.Straight",
        "stair.LShaped",
        "stair.UShaped",
        "stair.Winder",
        "stair.Curved",
        "stair.Landing",
        "stair.Ramp",
        "roof.plane",
        "roof.settings",
        "roof.dormer",
        "roof.ceiling",
        "foundation.slabs",
        "foundation.holes",
        "foundation.pads",
        "foundation.piers",
        "foundation.platform_holes",
        "detail.corner_boards",
        "detail.quoins",
        "detail.moldings",
        "detail.regions",
        "detail.hatches",
        "detail.decks",
        "detail.solids",
        "device.Outlet110",
        "device.Switch",
        "device.CeilingLight",
        "device.connection",
        "terrain.perimeter",
        "terrain.elevation_points",
        "terrain.elevation_lines",
        "terrain.elevation_regions",
        "terrain.features",
        "terrain.roads",
        "terrain.walls",
        "terrain.landscape",
        "terrain.modifiers",
        "camera",
        "light",
        "object_material",
        "material_default",
        "wall_type",
        "layer_set",
        "plan_view",
        "text_style",
        "text_macro",
        "note_type",
        "layout",
        "layout_file",
        "layout.cad",
        "layout.leaders",
        "layout.clouds",
        "project.info.filled",
        "project.electrical_defaults",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for s in plan_core::OpeningStyle::DOORS
        .iter()
        .chain(plan_core::OpeningStyle::WINDOWS.iter())
    {
        v.push(format!("opening.{s:?}"));
    }
    for k in [
        "FullCamera",
        "PerspectiveOverview",
        "DollHouse",
        "CrossSection",
        "WallElevation",
        "Orthographic",
        "Elevation",
        "Walkthrough",
        "FloorCamera",
        "GlassHouse",
        "FramingOverview",
    ] {
        v.push(format!("camera.{k}"));
    }
    for k in crate::tools::cabinet::KINDS {
        let name = format!("{k:?}");
        // Tool-only kinds are never stored.
        if !["CounterHole", "SoffitPolygon"].contains(&name.as_str()) {
            v.push(format!("cabinet.{name}"));
        }
    }
    for k in plan_core::schedules::ScheduleKind::ALL {
        v.push(format!("schedule.{k:?}"));
    }
    for k in [
        "PlanView",
        "Elevation",
        "Section",
        "Camera",
        "Schedule",
        "PlacedSchedule",
        "CadDetail",
        "Image",
        "ImageData",
        "Text",
        "Perspective",
        "Materials",
        "SheetIndex",
    ] {
        v.push(format!("layout.box.{k}"));
    }
    v
}

#[test]
fn the_fixture_holds_at_least_one_of_every_object_kind() {
    let (sim, panics) = rich_sim();
    assert!(panics.is_empty(), "tools panicked building it: {panics:?}");
    let have = kinds(&sim.app.cx.project);
    let missing: Vec<String> = required_kinds()
        .into_iter()
        .filter(|k| !have.contains(k))
        .collect();
    if let Ok(path) = std::env::var("S40_DUMP") {
        let _ = std::fs::write(&path, digest(&sim));
        eprintln!("kinds: {have:?}");
    }
    assert!(missing.is_empty(), "the fixture lacks {missing:?}");
}

// ===================================================================
// 2. Save, load, save
// ===================================================================

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("plan-s40-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp dir");
    d
}

#[test]
fn the_fixture_saves_loads_and_saves_byte_identical() {
    let (sim, _) = rich_sim();
    let dir = temp_dir("roundtrip");
    let (a, b) = (dir.join("a.psplan"), dir.join("b.psplan"));
    plan_core::io::save_project(&sim.app.cx.project, &a).expect("saves");
    let loaded = plan_core::io::load_project(&a).expect("loads");
    plan_core::io::save_project(&loaded, &b).expect("saves again");
    let (bytes_a, bytes_b) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    assert_eq!(bytes_a.len(), bytes_b.len());
    assert!(bytes_a == bytes_b, "the second save differs from the first");
    // Semantically equal, not only the same text.
    assert_eq!(
        format!("{:?}", sim.app.cx.project),
        format!("{loaded:?}"),
        "the loaded plan differs from the saved one"
    );
    // A third generation changes nothing either.
    let third = Project::from_json(&loaded.to_json().unwrap()).unwrap();
    assert_eq!(third.to_json().unwrap().as_bytes(), bytes_b.as_slice());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_the_saved_fixture_in_the_editor_changes_nothing() {
    let (sim, _) = rich_sim();
    let json = digest(&sim);
    let mut other = Sim::new();
    other
        .app
        .cx
        .set_project(Project::from_json(&json).expect("loads"));
    other.app.cx.refresh();
    assert!(
        digest(&other) == json,
        "loading the plan into the editor (storage migration) altered it"
    );
    // The migration finds nothing left to move.
    let mut again = Project::from_json(&json).unwrap();
    assert!(
        !crate::editor::site_view::migrate_legacy_storage(&mut again),
        "migration changed a plan already in the current format"
    );
}

#[test]
fn the_applications_own_save_and_open_keep_the_fixture_byte_identical() {
    let (mut sim, _) = rich_sim();
    let dir = temp_dir("app-save");
    let path = dir.join("fixture.psplan");
    sim.app.path = Some(path.clone());
    sim.action(Action::FileSave);
    assert!(
        sim.app.cx.status.starts_with("Saved"),
        "{}",
        sim.app.cx.status
    );
    let saved = std::fs::read_to_string(&path).expect("the plan was written");
    assert!(
        saved == pretty(&sim),
        "Save wrote something else than the plan"
    );
    let mut other = Sim::new();
    other.app.open_path(path.clone());
    assert!(
        other.app.cx.status.starts_with("Opened"),
        "{}",
        other.app.cx.status
    );
    assert!(
        pretty(&other) == saved,
        "the plan opened is not the plan saved"
    );
    // Save it again from the second editor: the file does not change.
    other.app.path = Some(path.clone());
    other.action(Action::FileSave);
    assert!(std::fs::read_to_string(&path).unwrap() == saved);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_damaged_file_is_refused_with_a_message_and_the_open_plan_stays() {
    let dir = temp_dir("damaged");
    let (sim, _) = rich_sim();
    let good = digest(&sim);
    let mut with_no_floors: Value = serde_json::from_str(&good).unwrap();
    with_no_floors["floors"] = serde_json::json!([]);
    let cases: Vec<(&str, String)> = vec![
        ("empty", String::new()),
        ("array", "[]".into()),
        ("garbage", "not a plan \u{0} \u{fffd}".into()),
        ("truncated", good[..good.len() / 2].to_string()),
        ("no floors", with_no_floors.to_string()),
    ];
    for (name, text) in cases {
        let path = dir.join(format!("{}.psplan", name.replace(' ', "-")));
        std::fs::write(&path, &text).unwrap();
        let mut other = Sim::new();
        let before = digest(&other);
        other.app.open_path(path);
        assert!(
            !other.app.cx.status.starts_with("Opened"),
            "{name}: {}",
            other.app.cx.status
        );
        assert!(digest(&other) == before, "{name}: the open plan changed");
    }
    // `{}` is a plan with every slot at its default (QA-21), not a damaged file.
    let path = dir.join("braces.psplan");
    std::fs::write(&path, "{}").unwrap();
    let mut other = Sim::new();
    other.app.open_path(path);
    assert!(
        other.app.cx.status.starts_with("Opened"),
        "braces: {}",
        other.app.cx.status
    );
    assert_eq!(other.app.cx.project.floors.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

// ===================================================================
// 3. Undo: one step per action, do / undo / redo / undo
// ===================================================================

/// Closes whatever dialog an action left up: OK, or Cancel when OK keeps it.
fn settle_dialog(sim: &mut Sim) {
    if sim.app.has_dialog() {
        sim.ok();
        if sim.app.has_dialog() {
            sim.cancel();
        }
    }
    sim.app.dialog = None;
}

/// A few of the places where two plans' JSON differ.
fn diff_note(a: &str, b: &str) -> String {
    let (Ok(x), Ok(y)) = (
        serde_json::from_str::<Value>(a),
        serde_json::from_str::<Value>(b),
    ) else {
        return "unreadable".into();
    };
    let mut out = Vec::new();
    first_difference(&x, &y, "", &mut out);
    let n = out.len();
    let shown: Vec<String> = out.into_iter().take(3).collect();
    format!("{n} differences, e.g. {}", shown.join("; "))
}

/// Runs one user event (a click, a key, a command, a dialog OK) and notes it
/// when it pushed more than one undo step.
fn event(sim: &mut Sim, name: &str, over: &mut Vec<String>, f: impl FnOnce(&mut Sim)) {
    let d = depth(sim);
    f(sim);
    let n = depth(sim).saturating_sub(d);
    if n > 1 {
        over.push(format!("{name} pushed {n} undo steps"));
    }
}

/// [`perform`] with every event (click, drag, key, double-click) measured.
fn perform_counting(sim: &mut Sim, g: G, over: &mut Vec<String>) {
    match g {
        G::Click(x, y) => event(sim, "click", over, |s| {
            s.click(x, y);
        }),
        G::Drag(a, b) => event(sim, "drag", over, |s| {
            s.drag(a, b);
        }),
        G::Clicks(pts, end) => {
            for (i, (x, y)) in pts.iter().enumerate() {
                event(sim, &format!("click {}", i + 1), over, |s| {
                    s.click(*x, *y);
                });
            }
            match end {
                End::None => {}
                End::Enter => event(sim, "Enter", over, |s| {
                    s.key(KeyEvent::key(Key::Enter));
                }),
                End::Double => {
                    let (x, y) = pts[pts.len() - 1];
                    event(sim, "double-click", over, |s| {
                        s.double_click(x, y);
                    });
                }
            }
        }
        G::Text(x, y) => {
            event(sim, "click", over, |s| {
                s.click(x, y);
            });
            event(sim, "typed text", over, |s| {
                s.key(KeyEvent::text("Den"));
            });
            event(sim, "Enter", over, |s| {
                s.key(KeyEvent::key(Key::Enter));
            });
        }
    }
}

/// Runs `act` (a user action made of events) and returns what is wrong with
/// it: an event with more than one undo step; a change with no undo step; a
/// step left behind by a change that was not made; undo that does not give
/// back the plan; redo that does not reproduce it. `s0` is the plan's digest
/// before. The plan is left as it was.
fn check_action(
    sim: &mut Sim,
    s0: &str,
    label: &str,
    act: impl FnOnce(&mut Sim, &mut Vec<String>),
) -> Vec<String> {
    let d0 = depth(sim);
    let mut over = Vec::new();
    act(sim, &mut over);
    let s1 = digest(sim);
    let d1 = depth(sim);
    let mut out: Vec<String> = over.into_iter().map(|o| format!("{label}: {o}")).collect();
    if s1 == s0 {
        if d1 != d0 {
            out.push(format!(
                "{label}: changed nothing but left {} undo step(s)",
                d1 as i64 - d0 as i64
            ));
            for _ in d0..d1 {
                sim.undo();
            }
        }
        return out;
    }
    let steps = d1.saturating_sub(d0);
    if steps == 0 {
        out.push(format!(
            "{label}: changed the plan without an undo step ({})",
            diff_note(s0, &s1)
        ));
        restore(sim, s0);
        return out;
    }
    for _ in 0..steps {
        sim.undo();
    }
    let u = digest(sim);
    if u != s0 {
        out.push(format!(
            "{label}: undo does not restore the plan ({})",
            diff_note(s0, &u)
        ));
        restore(sim, s0);
        return out;
    }
    if depth(sim) != d0 {
        out.push(format!(
            "{label}: undo left {} steps instead of {d0}",
            depth(sim)
        ));
    }
    for _ in 0..steps {
        sim.redo();
    }
    if digest(sim) != s1 {
        out.push(format!("{label}: redo does not reproduce the plan"));
    }
    for _ in 0..steps {
        sim.undo();
    }
    let u = digest(sim);
    if u != s0 {
        out.push(format!(
            "{label}: the second undo does not restore the plan ({})",
            diff_note(s0, &u)
        ));
        restore(sim, s0);
    }
    out
}

/// [`check_action`] that turns a panic into a problem. Keeps `s0` current.
fn guarded(
    sim: &mut Sim,
    s0: &mut String,
    label: &str,
    act: impl FnOnce(&mut Sim, &mut Vec<String>),
) -> Vec<String> {
    let r = catch_unwind(AssertUnwindSafe(|| check_action(sim, s0, label, act)));
    let out = match r {
        Ok(p) => p,
        Err(_) => {
            sim.app.dialog = None;
            restore(sim, s0);
            sim.tool(ToolId::Select);
            vec![format!("{label}: panicked")]
        }
    };
    if !out.is_empty() {
        *s0 = digest(sim);
    }
    out
}

/// The text a failing sweep prints: every problem is a failure (QA-24 to
/// QA-27 are fixed, nothing is filtered any more).
fn report(problems: &[String]) -> String {
    if problems.is_empty() {
        String::new()
    } else {
        format!(
            "{} problems:\n{}",
            problems.len(),
            problems
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

#[test]
fn every_tool_gesture_is_one_undo_step_and_undoes_cleanly() {
    let (mut sim, panics) = rich_sim();
    assert!(panics.is_empty(), "{panics:?}");
    let mut s0 = digest(&sim);
    let t = std::time::Instant::now();
    let mut problems = Vec::new();
    let mut n = 0;
    for id in all_tool_ids() {
        if skip_tool(id) {
            continue;
        }
        for g in CANDS {
            n += 1;
            let label = format!("{id:?} {g:?}");
            problems.extend(guarded(&mut sim, &mut s0, &label, |s, over| {
                s.tool(id);
                perform_counting(s, *g, over);
                event(s, "dialog", over, settle);
            }));
        }
    }
    eprintln!("{n} tool gestures checked in {:?}", t.elapsed());
    let text = report(&problems);
    assert!(text.is_empty(), "{text}");
}

/// The objects of the fixture to try the Edit commands on: the first few of
/// each kind (all of them would take too long).
fn sample_objects(sim: &Sim) -> Vec<ObjectRef> {
    let cx = &sim.app.cx;
    let mut all = selection::all_selectable(cx);
    all.extend((0..cx.rooms.len()).map(ObjectRef::Room));
    if crate::editor::site_view::load_terrain(&cx.project).is_some() {
        all.push(ObjectRef::Terrain);
    }
    let mut seen: std::collections::BTreeMap<&'static str, usize> = Default::default();
    all.into_iter()
        .filter(|o| {
            let n = seen.entry(o.type_name()).or_default();
            *n += 1;
            *n <= 2
        })
        .collect()
}

#[test]
fn every_edit_toolbar_command_is_one_undo_step_and_undoes_cleanly() {
    use crate::editor::actions::EditActionKind;
    let (mut sim, _) = rich_sim();
    let mut s0 = digest(&sim);
    let mut problems = Vec::new();
    let mut ran = 0;
    for o in sample_objects(&sim) {
        sim.tool(ToolId::Select);
        sim.app.cx.selection.set(o);
        sim.app.cx.refresh();
        let actions = sim.app.tools.active().edit_toolbar(&sim.app.cx);
        for a in actions {
            if !a.enabled || a.kind == EditActionKind::OpenObject {
                continue;
            }
            let label = format!("{} on {} {:?}", a.label, o.type_name(), o);
            ran += 1;
            problems.extend(guarded(&mut sim, &mut s0, &label, |s, over| {
                s.tool(ToolId::Select);
                s.app.cx.selection.set(o);
                event(s, a.label, over, |s| {
                    s.app.cx.apply_edit_action(a.kind);
                    s.app.process_requests();
                    s.app.cx.refresh();
                    settle_dialog(s);
                });
            }));
        }
    }
    assert!(ran > 50, "only {ran} Edit toolbar commands were tried");
    let text = report(&problems);
    assert!(text.is_empty(), "{text}");
}

#[test]
fn every_edit_menu_command_and_floor_command_is_one_undo_step_and_undoes_cleanly() {
    use crate::editor::edit_commands::ids;
    let (mut sim, _) = rich_sim();
    let mut s0 = digest(&sim);
    let mut problems = Vec::new();
    let edit_ids = [
        ids::CUT,
        ids::COPY,
        ids::PASTE,
        ids::PASTE_HOLD,
        ids::PASTE_GROUP,
        ids::COPY_PASTE_IN_PLACE,
        ids::DUPLICATE,
        ids::DELETE,
        ids::DELETE_OBJECTS,
        ids::SELECT_ALL,
        ids::SELECT_SAME,
        ids::GROUP,
        ids::UNGROUP,
        ids::EXPLODE,
        ids::LOCK,
        ids::UNLOCK,
        ids::TRANSFORM,
        ids::ROTATE,
        ids::REFLECT,
        ids::REFLECT_COPY,
        ids::CENTER,
        ids::PARALLEL,
        ids::PERPENDICULAR,
        ids::DISTRIBUTE_H,
        ids::DISTRIBUTE_V,
        ids::FRONT,
        ids::BACK,
        ids::REVERSE_SWING,
        ids::FLIP_HINGE,
        ids::REVERSE_LAYERS,
        ids::FIX_WALLS,
    ];
    let objects = sample_objects(&sim);
    let mut ran = 0;
    for id in edit_ids {
        for o in &objects {
            let label = format!("menu {id} on {o:?}");
            ran += 1;
            problems.extend(guarded(&mut sim, &mut s0, &label, |s, over| {
                s.tool(ToolId::Select);
                s.app.cx.selection.set(*o);
                event(s, id, over, |s| {
                    s.action(Action::Custom(id));
                    settle_dialog(s);
                });
            }));
        }
        // On the whole selection too.
        let label = format!("menu {id} on everything");
        problems.extend(guarded(&mut sim, &mut s0, &label, |s, over| {
            s.tool(ToolId::Select);
            let all = selection::all_selectable(&s.app.cx);
            s.app.cx.selection.clear();
            for o in all {
                s.app.cx.selection.add(o);
            }
            event(s, id, over, |s| {
                s.action(Action::Custom(id));
                settle_dialog(s);
            });
        }));
    }
    // Floor and plan-wide commands.
    let plan_wide: Vec<(&str, Action)> = vec![
        ("Build New Floor", Action::BuildNewFloor),
        ("Insert Floor", Action::InsertFloor),
        ("Insert Floor Below", Action::InsertFloorBelow),
        ("Delete Floor", Action::DeleteFloor),
        ("Delete Foundation", Action::DeleteFoundation),
        ("Exchange Floor Above", Action::ExchangeFloorAbove),
        ("Exchange Floor Below", Action::ExchangeFloorBelow),
        ("Build Foundation", Action::BuildFoundation),
        ("Rebuild All", Action::RebuildAll),
        (
            "Terrain Clear",
            Action::Terrain(crate::toolbar::TerrainCommand::Clear),
        ),
        (
            "Terrain Hole Around Building",
            Action::Terrain(crate::toolbar::TerrainCommand::HoleAroundBuilding),
        ),
        (
            "Framing Build",
            Action::Framing(crate::toolbar::FramingCommand::Build),
        ),
        (
            "Framing Build All",
            Action::Framing(crate::toolbar::FramingCommand::BuildAll),
        ),
        (
            "Framing Delete",
            Action::Framing(crate::toolbar::FramingCommand::Delete),
        ),
        (
            "Delete Roof Planes",
            Action::Custom(crate::editor::dispatch::cmd::ROOF_DELETE_ALL),
        ),
        (
            "Delete Ceiling Planes",
            Action::Custom(crate::editor::dispatch::cmd::ROOF_DELETE_CEILINGS),
        ),
        (
            "Rebuild Roofs",
            Action::Custom(crate::editor::dispatch::cmd::ROOF_REBUILD),
        ),
        (
            "Renumber Doors",
            Action::Custom(crate::editor::opening_edit::RENUMBER_DOORS),
        ),
        (
            "Renumber Windows",
            Action::Custom(crate::editor::opening_edit::RENUMBER_WINDOWS),
        ),
        (
            "Doors Open",
            Action::Custom(crate::editor::opening_edit::DOORS_OPEN),
        ),
        (
            "Casing in 3D",
            Action::Custom(crate::editor::opening_edit::CASING_3D),
        ),
        (
            "Save Plan View",
            Action::Custom(crate::dialogs::plan_views::SAVE),
        ),
        (
            "Reset Plan View",
            Action::Custom(crate::dialogs::plan_views::RESET),
        ),
        (
            "Add Template Plan Views",
            Action::Custom(crate::dialogs::plan_views::SEED),
        ),
    ];
    for (name, action) in plan_wide {
        for floor in 0..sim.app.cx.project.floors.len().min(3) {
            let label = format!("{name} (floor {floor})");
            ran += 1;
            problems.extend(guarded(&mut sim, &mut s0, &label, |s, over| {
                s.app.cx.floor = floor.min(s.app.cx.project.floors.len() - 1);
                s.tool(ToolId::Select);
                event(s, name, over, |s| {
                    s.action(action);
                    settle_dialog(s);
                });
                s.app.cx.floor = 0;
            }));
        }
    }
    assert!(ran > 100, "only {ran} commands were tried");
    let text = report(&problems);
    assert!(text.is_empty(), "{text}");
}

// ===================================================================
// 4. Legacy files
// ===================================================================

const LEGACY_LAYERS: [&str; 6] = [
    "Roof Planes, Data",
    "Foundation, Data",
    "Lights, Data",
    "CAD, Data",
    "Electrical, Data",
    "Terrain, Data",
];

fn legacy_record(
    p: &mut Project,
    floor: usize,
    id: Option<plan_core::Id>,
    layer: &str,
    text: String,
) {
    if p.layers.get(layer).is_none() {
        let mut l = plan_core::Layer::new(layer, [128, 128, 128], 13);
        l.display = false;
        l.locked = true;
        p.layers.add(l);
    }
    let id = id.unwrap_or_else(|| p.alloc_id());
    p.floors[floor].cad.push(plan_core::CadObject {
        id,
        layer: layer.to_string(),
        item: plan_core::CadItem::Text {
            pos: Point::ZERO,
            text,
            height: 0.1,
            angle: 0.0,
        },
    });
}

/// The plan as the builds before the typed slots stored it: roof planes and
/// settings, the slab layer, lights, CAD attributes, blocks, macros and note
/// types, electrical and terrain as tagged text records on hidden layers.
fn downgrade(modern: &Project) -> Project {
    let mut old = modern.clone();
    for fi in 0..old.floors.len() {
        // Roof planes and the Build Roof settings (the old form held no more).
        let roofs = std::mem::take(&mut old.floors[fi].roofs);
        for r in &roofs {
            match r.get("kind").and_then(Value::as_str) {
                Some("plane") => {
                    let id = r["id"].as_u64().expect("plane id");
                    let outline = old.alloc_id();
                    old.floors[fi].cad.push(plan_core::CadObject {
                        id: outline,
                        layer: "Roof Planes".into(),
                        item: plan_core::CadItem::Polyline {
                            points: vec![Point::ZERO, pt(5.0, 0.0), pt(5.0, 5.0)],
                            closed: true,
                        },
                    });
                    let mut v = r.clone();
                    let o = v.as_object_mut().unwrap();
                    o.remove("kind");
                    o.insert("outline_id".into(), serde_json::json!(outline));
                    legacy_record(
                        &mut old,
                        fi,
                        Some(id),
                        "Roof Planes, Data",
                        format!("RFP1:{v}"),
                    );
                }
                Some("settings") => {
                    legacy_record(&mut old, fi, None, "Roof Planes, Data", format!("RFS1:{r}"));
                }
                _ => {}
            }
        }
        // The slab layer.
        if let Some(v) = old.floors[fi].foundation.take() {
            legacy_record(&mut old, fi, None, "Foundation, Data", format!("FND1:{v}"));
        }
        // Electrical.
        if let Some(v) = old.floors[fi].electrical.take() {
            legacy_record(&mut old, fi, None, "Electrical, Data", v.to_string());
        }
        // CAD attributes and blocks.
        for a in std::mem::take(&mut old.floors[fi].cad_attrs) {
            let j = serde_json::to_string(&a).unwrap();
            legacy_record(&mut old, fi, None, "CAD, Data", format!("cad-attrs:{j}"));
        }
        for b in std::mem::take(&mut old.floors[fi].cad_blocks) {
            let j = serde_json::to_string(&b).unwrap();
            legacy_record(&mut old, fi, None, "CAD, Data", format!("cad-block:{j}"));
        }
    }
    for l in std::mem::take(&mut old.lights) {
        let j = serde_json::to_string(&l).unwrap();
        let floor = l.floor.min(old.floors.len() - 1);
        legacy_record(
            &mut old,
            floor,
            Some(l.id),
            "Lights, Data",
            format!("plan-light:{j}"),
        );
    }
    let j = serde_json::to_string(&old.light_options).unwrap();
    legacy_record(
        &mut old,
        0,
        None,
        "Lights, Data",
        format!("plan-lightset:{j}"),
    );
    if !old.text_macros.macros.is_empty() {
        let j = serde_json::to_string(&old.text_macros).unwrap();
        old.text_macros = Default::default();
        legacy_record(
            &mut old,
            0,
            None,
            "CAD, Data",
            format!("cad-blob:text-macros={j}"),
        );
    }
    if old.note_types != plan_core::text_styles::NoteTypes::default() {
        let j = serde_json::to_string(&old.note_types).unwrap();
        old.note_types = Default::default();
        legacy_record(
            &mut old,
            0,
            None,
            "CAD, Data",
            format!("cad-blob:note-types={j}"),
        );
    }
    if let Some(t) = old.terrain.take() {
        legacy_record(&mut old, 0, None, "Terrain, Data", t.to_string());
    }
    old
}

/// The plan's JSON with what the old form could not hold taken out: only
/// roof planes and settings, lights in id order.
fn comparable(p: &Project) -> Value {
    let mut v = serde_json::to_value(p).unwrap();
    for f in v["floors"].as_array_mut().unwrap() {
        if let Some(r) = f["roofs"].as_array_mut() {
            r.retain(|x| matches!(x["kind"].as_str(), Some("plane" | "settings")));
        }
    }
    if let Some(l) = v["lights"].as_array_mut() {
        l.sort_by_key(|x| x["id"].as_u64());
    }
    // The old form took ids from the same counter for its records.
    v["next_id"] = serde_json::json!(0);
    v
}

fn first_difference(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for k in keys {
                first_difference(
                    x.get(k).unwrap_or(&Value::Null),
                    y.get(k).unwrap_or(&Value::Null),
                    &format!("{path}/{k}"),
                    out,
                );
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                first_difference(p, q, &format!("{path}[{i}]"), out);
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            out.push(format!("{path}: {} entries became {}", x.len(), y.len()))
        }
        _ => {
            let (a, b) = (a.to_string(), b.to_string());
            let cut = |s: &str| s.chars().take(80).collect::<String>();
            out.push(format!("{path}: {} became {}", cut(&a), cut(&b)));
        }
    }
}

#[test]
fn a_plan_stored_the_old_hidden_layer_way_migrates_to_the_same_plan_once() {
    let (sim, _) = rich_sim();
    let modern = sim.app.cx.project.clone();
    let old = downgrade(&modern);
    for l in LEGACY_LAYERS {
        assert!(old.layers.get(l).is_some(), "the old plan lacks layer {l}");
    }
    // As a file, then opened in the editor.
    let text = old.to_json().expect("serializes");
    let mut s = Sim::new();
    s.app
        .cx
        .set_project(Project::from_json(&text).expect("the old plan loads"));
    s.app.cx.refresh();
    let migrated = s.app.cx.project.clone();
    for l in LEGACY_LAYERS {
        assert!(migrated.layers.get(l).is_none(), "layer {l} is still there");
        for (fi, f) in migrated.floors.iter().enumerate() {
            assert!(
                f.cad.iter().all(|c| c.layer != l),
                "floor {fi} still has CAD objects on {l}"
            );
        }
    }
    let mut diffs = Vec::new();
    first_difference(&comparable(&modern), &comparable(&migrated), "", &mut diffs);
    assert!(
        diffs.is_empty(),
        "migration changed {} things:\n{}",
        diffs.len(),
        diffs
            .iter()
            .take(30)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    // Nothing is left to migrate, and the migrated plan saves and loads stably.
    let mut again = migrated.clone();
    assert!(!crate::editor::site_view::migrate_legacy_storage(
        &mut again
    ));
    let a = migrated.to_json().unwrap();
    let b = Project::from_json(&a).unwrap().to_json().unwrap();
    assert!(a == b, "the migrated plan does not save stably");
}

/// Plans written by earlier builds: the samples in the tree, and every
/// version of them in git (skipped without git or history).
fn sample_plans() -> Vec<(String, String)> {
    use std::process::Command;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen: BTreeSet<u64> = BTreeSet::new();
    let mut add = |name: String, text: String, out: &mut Vec<(String, String)>| {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut h);
        if seen.insert(h.finish()) {
            out.push((name, text));
        }
    };
    if let Ok(rd) = std::fs::read_dir(root.join("samples")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "psplan") {
                if let Ok(t) = std::fs::read_to_string(&p) {
                    add(
                        format!("tree:{}", p.file_name().unwrap().to_string_lossy()),
                        t,
                        &mut out,
                    );
                }
            }
        }
    }
    let git = |args: &[&str]| -> Option<String> {
        let o = Command::new("git")
            .current_dir(&root)
            .args(args)
            .output()
            .ok()?;
        o.status
            .success()
            .then(|| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    if let Some(log) = git(&["log", "--format=%h", "--", "samples"]) {
        for c in log.lines() {
            let Some(files) = git(&["ls-tree", "-r", "--name-only", c, "samples"]) else {
                continue;
            };
            for f in files.lines().filter(|f| f.ends_with(".psplan")) {
                if let Some(t) = git(&["show", &format!("{c}:{f}")]) {
                    add(format!("git {c}:{f}"), t, &mut out);
                }
            }
        }
    }
    out
}

#[test]
fn the_sample_plans_and_their_earlier_versions_load_migrate_and_resave_stably() {
    super::s21_layout_print::isolate_home();
    let plans = sample_plans();
    assert!(!plans.is_empty(), "no sample plans found");
    eprintln!("{} sample plan versions", plans.len());
    let mut problems = Vec::new();
    for (name, text) in plans {
        let p = match Project::from_json(&text) {
            Ok(p) => p,
            Err(e) => {
                problems.push(format!("{name}: does not load: {e}"));
                continue;
            }
        };
        let r = catch_unwind(AssertUnwindSafe(|| {
            let mut sim = Sim::new();
            sim.app.cx.set_project(p);
            let mut out = Vec::new();
            // Every floor draws and detects its rooms without a panic.
            for fi in 0..sim.app.cx.project.floors.len() {
                sim.app.cx.floor = fi;
                sim.app.cx.refresh();
                let _ = sim.plan_shapes();
            }
            sim.app.cx.floor = 0;
            let a = digest(&sim);
            let b = Project::from_json(&a).map(|q| serde_json::to_string(&q).unwrap());
            match b {
                Ok(b) if b == a => {}
                Ok(_) => out.push("saving the loaded plan twice differs".to_string()),
                Err(e) => out.push(format!("its own save does not load: {e}")),
            }
            let mut q = sim.app.cx.project.clone();
            if crate::editor::site_view::migrate_legacy_storage(&mut q) {
                out.push("migration was not finished by loading".to_string());
            }
            out
        }));
        match r {
            Ok(v) => problems.extend(v.into_iter().map(|m| format!("{name}: {m}"))),
            Err(_) => problems.push(format!("{name}: panicked on open")),
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

// ===================================================================
// 4b. Records this build cannot read
// ===================================================================

/// Gives `id` the gestures of `CANDS` until one changes the plan.
fn edit_with(sim: &mut Sim, id: ToolId) -> bool {
    for g in CANDS {
        let d0 = depth(sim);
        sim.tool(id);
        perform(sim, *g);
        settle(sim);
        if depth(sim) > d0 {
            return true;
        }
    }
    false
}

/// A shell with the tools' own records of one family made by `prepare`, then
/// a record this build cannot read added to the slot, then an edit of the
/// same family. `check` must still hold afterwards.
fn survives(
    name: &str,
    prepare: &dyn Fn(&mut Sim),
    inject: &dyn Fn(&mut plan_core::Floor),
    edit: &dyn Fn(&mut Sim) -> bool,
    check: &dyn Fn(&plan_core::Floor) -> bool,
) -> Option<String> {
    let shell = || {
        let mut sim = Sim::new();
        draw_shell(&mut sim, W, H);
        prepare(&mut sim);
        sim
    };
    // The edit works on a plan without the foreign record.
    let mut clean = shell();
    if !edit(&mut clean) {
        return Some(format!(
            "{name}: (harness) the edit does nothing on a clean plan"
        ));
    }
    let mut sim = shell();
    inject(&mut sim.app.cx.project.floors[0]);
    let json = digest(&sim);
    restore(&mut sim, &json);
    if !check(&sim.app.cx.project.floors[0]) {
        return Some(format!(
            "{name}: the record did not even load into the plan"
        ));
    }
    if !edit(&mut sim) {
        // The tool refuses to edit a family it cannot read completely: the
        // record is safe, but the user's click does nothing and says nothing.
        return Some(format!(
            "{name}: the edit was refused while the unreadable record is there (nothing is said)"
        ));
    }
    (!check(&sim.app.cx.project.floors[0]))
        .then(|| format!("{name}: the record was dropped by an edit of its family"))
}

/// Does the slot hold a record with this id?
fn has_id(v: &[Value], id: u64) -> bool {
    v.iter().any(|r| r["id"] == serde_json::json!(id))
}

fn foreign_records() -> Vec<Option<String>> {
    use serde_json::json;
    let no_prepare = |_: &mut Sim| {};
    let mut out = Vec::new();
    out.push(survives(
        "cabinets",
        &no_prepare,
        &|f| {
            f.cabinets
                .push(json!({"id": 880001, "kind": "FutureKind", "x": 1}))
        },
        &|s| edit_with(s, ToolId::CabinetVariant(plan_cabinets::CabinetKind::Base)),
        &|f| has_id(&f.cabinets, 880001),
    ));
    out.push(survives(
        "stairs",
        &no_prepare,
        &|f| f.stairs.push(json!({"id": 880002, "future": true})),
        &|s| {
            edit_with(
                s,
                ToolId::StairsVariant(crate::editor::stairs_view::StairKind::Draw),
            )
        },
        &|f| has_id(&f.stairs, 880002),
    ));
    out.push(survives(
        "roofs",
        &|s| {
            s.tool(ToolId::RoofVariant(crate::tools::roof::RoofMode::Build));
            s.click(240.0, 180.0);
            s.ok();
            settle(s);
        },
        &|f| f.roofs.push(json!({"kind": "future", "id": 880003})),
        &|s| {
            // A skylight on the first roof plane, as `roof_features` does.
            let set = crate::editor::roof_view::load(s.app.cx.floor());
            let Some(c) = set.planes.first().map(|p| p.centroid()) else {
                return false;
            };
            let d0 = depth(s);
            s.tool(ToolId::RoofVariant(crate::tools::roof::RoofMode::Skylight));
            s.drag((c.x + 20.0, c.y - 12.0), (c.x + 44.0, c.y + 12.0));
            settle(s);
            depth(s) > d0
        },
        &|f| has_id(&f.roofs, 880003),
    ));
    out.push(survives(
        "framing",
        &no_prepare,
        &|f| f.framing.push(json!({"id": 880004, "future": true})),
        &|s| {
            crate::tools::framing::FramingVariant::ALL
                .into_iter()
                .any(|v| edit_with(s, ToolId::FramingVariant(v)))
        },
        &|f| has_id(&f.framing, 880004),
    ));
    out
}

#[test]
fn the_foreign_record_checks_work() {
    // Only "was dropped" lines are findings (QA-29, below); the harness must
    // be able to make the edits and see the records.
    let problems: Vec<String> = foreign_records().into_iter().flatten().collect();
    let broken: Vec<&String> = problems
        .iter()
        .filter(|p| !p.contains("was dropped") && !p.contains("was refused"))
        .collect();
    assert!(broken.is_empty(), "{broken:?}");
    eprintln!("dropped by an edit: {problems:?}");
}

/// Electrical devices of the layer: the readable ones, by id.
fn device_ids(f: &plan_core::Floor) -> Vec<u64> {
    f.electrical
        .as_ref()
        .and_then(|e| e["devices"].as_array())
        .map(|d| d.iter().filter_map(|v| v["id"].as_u64()).collect())
        .unwrap_or_default()
}

/// Two devices, then one of a kind this build does not know, then a third
/// device. Returns the ids before and after.
fn electrical_with_a_foreign_device() -> (Vec<u64>, Vec<u64>) {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    for v in [
        ElecVariant::Outlet110,
        ElecVariant::Light,
        ElecVariant::Switch,
        ElecVariant::Gfci,
        ElecVariant::RecessedLight,
        ElecVariant::DataJack,
    ] {
        if device_ids(&sim.app.cx.project.floors[0]).len() >= 2 {
            break;
        }
        edit_with(&mut sim, ToolId::ElectricalVariant(v));
    }
    let readable = device_ids(&sim.app.cx.project.floors[0]);
    assert!(readable.len() >= 2, "{readable:?}");
    let e = sim.app.cx.project.floors[0]
        .electrical
        .as_mut()
        .expect("electrical slot");
    e["devices"]
        .as_array_mut()
        .expect("devices")
        .push(serde_json::json!({"id": 880005, "kind": "FutureDevice"}));
    let json = digest(&sim);
    restore(&mut sim, &json);
    // Any device kind that finds a free spot will do.
    let placed = [
        ElecVariant::Gfci,
        ElecVariant::Switch,
        ElecVariant::RecessedLight,
        ElecVariant::DataJack,
        ElecVariant::Outlet110,
        ElecVariant::Light,
    ]
    .into_iter()
    .any(|v| edit_with(&mut sim, ToolId::ElectricalVariant(v)));
    assert!(
        placed,
        "no device was placed next to the foreign one; status {:?}, devices {:?}",
        sim.app.cx.status,
        device_ids(&sim.app.cx.project.floors[0])
    );
    (readable, device_ids(&sim.app.cx.project.floors[0]))
}

// ===================================================================
// 5. Random gestures
// ===================================================================

/// xorshift64: the same gestures on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next() >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
    }
}

/// The menu and toolbar commands the random run may press (nothing that
/// opens a native file dialog or replaces the plan).
fn fuzz_actions() -> Vec<Action> {
    use crate::editor::edit_commands::ids;
    use crate::toolbar::{FramingCommand, TerrainCommand};
    let mut v = vec![
        Action::FloorUp,
        Action::FloorDown,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::FillWindow,
        Action::BuildNewFloor,
        Action::InsertFloor,
        Action::InsertFloorBelow,
        Action::DeleteFloor,
        Action::DeleteFoundation,
        Action::ExchangeFloorAbove,
        Action::ExchangeFloorBelow,
        Action::BuildFoundation,
        Action::RebuildAll,
        Action::Terrain(TerrainCommand::Clear),
        Action::Terrain(TerrainCommand::HoleAroundBuilding),
        Action::Framing(FramingCommand::Build),
        Action::Framing(FramingCommand::BuildAll),
        Action::Framing(FramingCommand::Delete),
        Action::Custom(crate::editor::dispatch::cmd::ROOF_DELETE_ALL),
        Action::Custom(crate::editor::dispatch::cmd::ROOF_REBUILD),
        Action::Custom(crate::editor::opening_edit::RENUMBER_DOORS),
    ];
    for id in [
        ids::CUT,
        ids::COPY,
        ids::PASTE,
        ids::DUPLICATE,
        ids::DELETE,
        ids::SELECT_ALL,
        ids::SELECT_SAME,
        ids::GROUP,
        ids::UNGROUP,
        ids::EXPLODE,
        ids::LOCK,
        ids::UNLOCK,
        ids::ROTATE,
        ids::REFLECT_COPY,
        ids::CENTER,
        ids::FRONT,
        ids::BACK,
        ids::FIX_WALLS,
    ] {
        v.push(Action::Custom(id));
    }
    v
}

/// One random user gesture; returns what it was.
fn fuzz_op(sim: &mut Sim, rng: &mut Rng, ids: &[ToolId], acts: &[Action]) -> String {
    if sim.app.has_dialog() {
        let key = [None, Some(Key::Enter), Some(Key::Escape)][rng.below(3)];
        sim.dialog_frame_key(key);
        return format!("dialog frame {key:?}");
    }
    // A point: usually near the house, sometimes on one of its walls.
    let point = |sim: &Sim, rng: &mut Rng| -> (f64, f64) {
        let walls = &sim.app.cx.floor().walls;
        if !walls.is_empty() && rng.below(5) == 0 {
            let w = &walls[rng.below(walls.len())];
            let t = rng.range(0.0, 1.0);
            let q = w.start.add(w.end.sub(w.start).scale(t));
            return (q.x, q.y);
        }
        (rng.range(-150.0, W + 150.0), rng.range(-150.0, H + 150.0))
    };
    match rng.below(100) {
        0..=11 => {
            let id = ids[rng.below(ids.len())];
            sim.tool(id);
            format!("tool {id:?}")
        }
        12..=44 => {
            let (x, y) = point(sim, rng);
            sim.click(x, y);
            format!("click {x:.1},{y:.1}")
        }
        45..=59 => {
            let a = point(sim, rng);
            let b = point(sim, rng);
            sim.drag(a, b);
            format!("drag {a:?} to {b:?}")
        }
        60..=64 => {
            let (x, y) = point(sim, rng);
            sim.double_click(x, y);
            format!("double-click {x:.1},{y:.1}")
        }
        65..=76 => {
            let texts = ["12", "3'6\"", "-5", "abc", "0", ".5", "10'"];
            let keys = [
                Key::Escape,
                Key::Enter,
                Key::Delete,
                Key::Backspace,
                Key::Tab,
                Key::ArrowLeft,
                Key::ArrowRight,
                Key::ArrowUp,
                Key::ArrowDown,
            ];
            if rng.below(3) == 0 {
                let t = texts[rng.below(texts.len())];
                sim.key(KeyEvent::text(t));
                format!("type {t:?}")
            } else {
                let k = keys[rng.below(keys.len())];
                sim.key(KeyEvent::key(k));
                format!("key {k:?}")
            }
        }
        77..=82 => {
            let (x, y) = point(sim, rng);
            sim.move_to(x, y);
            format!("move {x:.1},{y:.1}")
        }
        83..=89 => {
            let a = acts[rng.below(acts.len())];
            sim.action(a);
            format!("menu {a:?}")
        }
        90..=96 => {
            // Select something and press one of its Edit toolbar buttons.
            let all = selection::all_selectable(&sim.app.cx);
            if all.is_empty() {
                return "nothing to select".into();
            }
            let o = all[rng.below(all.len())];
            sim.app.cx.selection.set(o);
            let actions = sim.app.tools.active().edit_toolbar(&sim.app.cx);
            let usable: Vec<_> = actions.into_iter().filter(|a| a.enabled).collect();
            if usable.is_empty() {
                return format!("selected {o:?}");
            }
            let a = &usable[rng.below(usable.len())];
            sim.app.cx.apply_edit_action(a.kind);
            sim.app.process_requests();
            sim.app.cx.refresh();
            format!("edit button {:?} on {o:?}", a.label)
        }
        _ => {
            let all = selection::all_selectable(&sim.app.cx);
            if all.is_empty() {
                return "nothing to select".into();
            }
            let o = all[rng.below(all.len())];
            sim.app.cx.selection.set(o);
            format!("select {o:?}")
        }
    }
}

#[test]
fn two_thousand_random_gestures_never_panic_and_every_unwind_returns_to_the_start() {
    const OPS: usize = 2000;
    const BATCH: usize = 50;
    let (mut sim, built) = rich_sim();
    assert!(built.is_empty(), "{built:?}");
    let ids: Vec<ToolId> = all_tool_ids();
    let acts = fuzz_actions();
    let mut rng = Rng(0x5EED_0040_1234_5678);
    let mut problems: Vec<String> = Vec::new();
    let mut done = 0;
    while done < OPS {
        settle(&mut sim);
        // The history keeps 100 steps; the batches before this one left their
        // steps in it (they are redone after the unwind check), so without
        // this the depth would stop growing after a dozen batches and every
        // later gesture would look like it left no undo step.
        sim.app.cx.forget_history();
        let base = digest(&sim);
        let base_depth = depth(&sim);
        let mut log: Vec<String> = Vec::new();
        let mut tainted = false;
        let mut now = base.clone();
        for _ in 0..BATCH {
            let d_before = depth(&sim);
            let r = catch_unwind(AssertUnwindSafe(|| {
                let what = fuzz_op(&mut sim, &mut rng, &ids, &acts);
                frame(&mut sim);
                what
            }));
            done += 1;
            let what = match r {
                Ok(w) => w,
                Err(_) => {
                    problems.push(format!(
                        "panic at gesture {done}; the last ones: {:?}",
                        log.iter().rev().take(4).collect::<Vec<_>>()
                    ));
                    sim.app.dialog = None;
                    sim.tool(ToolId::Select);
                    tainted = true;
                    "(panicked)".to_string()
                }
            };
            log.push(what.clone());
            // Each gesture on its own: one undo step that gives the plan back.
            let after = digest(&sim);
            let d_after = depth(&sim);
            let steps = d_after.saturating_sub(d_before);
            let label = format!("gesture {done} ({what})");
            if after == now {
                if d_after != d_before {
                    problems.push(format!(
                        "{label}: changed nothing but left {} undo step(s)",
                        d_after as i64 - d_before as i64
                    ));
                }
            } else if steps == 0 {
                problems.push(format!(
                    "{label}: changed the plan without an undo step ({})",
                    diff_note(&now, &after)
                ));
                tainted = true;
            } else {
                for _ in 0..steps {
                    sim.undo();
                }
                let back = digest(&sim);
                if back != now {
                    problems.push(format!(
                        "{label}: undo does not restore the plan ({})",
                        diff_note(&now, &back)
                    ));
                    tainted = true;
                    restore(&mut sim, &after);
                } else {
                    for _ in 0..steps {
                        sim.redo();
                    }
                    if digest(&sim) != after {
                        problems.push(format!("{label}: redo does not reproduce the plan"));
                        tainted = true;
                    }
                }
            }
            now = digest(&sim);
        }
        // End of the batch: dialogs closed, tool ended.
        let _ = catch_unwind(AssertUnwindSafe(|| settle_dialog(&mut sim)));
        let _ = catch_unwind(AssertUnwindSafe(|| settle(&mut sim)));
        let after = digest(&sim);
        // What was written can be read back, and is the same plan.
        match Project::from_json(&after) {
            Ok(q) => {
                if serde_json::to_string(&q).map_or(true, |t| t != after) {
                    problems.push(format!(
                        "gesture {done}: the saved plan does not reload equal"
                    ));
                }
            }
            Err(e) => problems.push(format!(
                "gesture {done}: the saved plan cannot be read back ({e}); gestures: {log:?}"
            )),
        }
        // The whole batch unwinds to where it began, and comes back.
        let steps = depth(&sim).saturating_sub(base_depth);
        if tainted || steps == 0 || steps >= 90 || depth(&sim) < base_depth {
            continue;
        }
        for _ in 0..steps {
            sim.undo();
        }
        if digest(&sim) != base {
            problems.push(format!(
                "gesture {done}: undoing the {steps} steps of the last {BATCH} gestures does not \
                 return to their start ({}); gestures: {log:?}",
                diff_note(&base, &digest(&sim))
            ));
            restore(&mut sim, &after);
            continue;
        }
        for _ in 0..steps {
            sim.redo();
        }
        if digest(&sim) != after {
            problems.push(format!(
                "gesture {done}: redoing {steps} steps does not reproduce the plan"
            ));
        }
    }
    let text = report(&problems);
    assert!(text.is_empty(), "{text}");
}

// ===================================================================
// Findings (QA-24 onward): the smallest test that shows each one
// ===================================================================

fn small_house() -> Sim {
    super::s21_layout_print::isolate_home();
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    settle(&mut sim);
    sim
}

/// A door, a base cabinet and a stair on the small house.
fn furnished_house() -> Sim {
    let mut sim = small_house();
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::CabinetVariant(plan_cabinets::CabinetKind::Base));
    sim.click(100.0, 10.0);
    sim.tool(ToolId::StairsVariant(
        crate::editor::stairs_view::StairKind::Draw,
    ));
    sim.drag((300.0, 100.0), (420.0, 100.0));
    settle(&mut sim);
    sim
}

#[test]
fn deleting_a_mixed_selection_is_one_undo_step() {
    use crate::editor::edit_commands::ids;
    let mut sim = furnished_house();
    let all = selection::all_selectable(&sim.app.cx);
    sim.app.cx.selection.clear();
    for o in all {
        sim.app.cx.selection.add(o);
    }
    let d = depth(&sim);
    sim.action(Action::Custom(ids::DELETE));
    let steps = depth(&sim) - d;
    assert_eq!(
        steps, 1,
        "one Delete of walls, a door, a cabinet and a stair is {steps} undo steps"
    );
}

#[test]
fn reversing_the_swing_of_a_door_and_a_cabinet_is_one_undo_step() {
    use crate::editor::edit_commands::ids;
    let mut sim = furnished_house();
    let door = sim.app.cx.floor().openings[0].id;
    let cab = crate::editor::placed::load_cabinets(sim.app.cx.floor())[0].id;
    sim.app.cx.selection.clear();
    sim.app.cx.selection.add(ObjectRef::Opening(door));
    sim.app.cx.selection.add(ObjectRef::Cabinet(cab));
    let before = digest(&sim);
    let d = depth(&sim);
    sim.action(Action::Custom(ids::REVERSE_SWING));
    assert!(digest(&sim) != before, "nothing was reversed");
    let steps = depth(&sim) - d;
    assert_eq!(steps, 1, "one Reverse Swing is {steps} undo steps");
}

#[test]
fn rebuild_all_leaves_an_undo_step_when_it_changes_the_plan() {
    let mut sim = small_house();
    sim.app
        .cx
        .project
        .floors
        .push(plan_core::Floor::new("2nd Floor", 240.0));
    let before = digest(&sim);
    let d = depth(&sim);
    sim.action(Action::RebuildAll);
    let after = digest(&sim);
    assert!(
        after != before,
        "Rebuild All changed nothing: the test needs a stale plan"
    );
    assert!(
        depth(&sim) > d,
        "Rebuild All changed the plan ({}) and left no undo step",
        diff_note(&before, &after)
    );
}

#[test]
fn a_command_that_changes_nothing_leaves_no_undo_step() {
    use crate::editor::edit_commands::ids;
    let mut sim = small_house();
    let wall = sim.app.cx.floor().walls[0].id;
    sim.app.cx.selection.set(ObjectRef::Wall(wall));
    let before = digest(&sim);
    let d = depth(&sim);
    // The wall is not locked: Unlock Selection has nothing to do.
    sim.action(Action::Custom(ids::UNLOCK));
    assert!(digest(&sim) == before, "Unlock changed an unlocked wall");
    assert_eq!(
        depth(&sim),
        d,
        "Unlock Selection on an unlocked wall left an undo step"
    );
}

#[test]
fn locking_a_terrain_object_locks_it_or_says_it_cannot() {
    use crate::editor::edit_commands::ids;
    let mut sim = small_house();
    assert!(edit_with(
        &mut sim,
        ToolId::TerrainVariant(TerrainVariant::Perimeter)
    ));
    edit_with(
        &mut sim,
        ToolId::TerrainVariant(TerrainVariant::ElevationPoint),
    );
    let pts = selection::all_selectable(&sim.app.cx)
        .into_iter()
        .find(|o| matches!(o, ObjectRef::TerrainObject(_)))
        .expect("an elevation point");
    sim.app.cx.selection.set(pts);
    let before = digest(&sim);
    let d = depth(&sim);
    sim.action(Action::Custom(ids::LOCK));
    assert!(
        digest(&sim) != before || depth(&sim) == d,
        "Lock on a terrain object changed nothing but left an undo step"
    );
}

#[test]
fn undo_after_a_foundation_tool_gives_back_exactly_the_plan() {
    let mut sim = small_house();
    let before = digest(&sim);
    sim.tool(ToolId::FoundationVariant(FoundationVariant::SquarePad));
    sim.click(240.0, 180.0);
    settle(&mut sim);
    assert!(digest(&sim) != before, "no pad was placed");
    sim.undo();
    assert!(
        digest(&sim) == before,
        "undo left the plan different: {}",
        diff_note(&before, &digest(&sim))
    );
}

#[test]
fn one_unreadable_electrical_device_does_not_take_the_readable_ones_with_it() {
    let (before, after) = electrical_with_a_foreign_device();
    let lost: Vec<&u64> = before.iter().filter(|i| !after.contains(i)).collect();
    assert!(
        lost.is_empty(),
        "devices {lost:?} of {before:?} are gone; now {after:?}"
    );
}

#[test]
fn records_this_build_cannot_read_survive_an_edit_of_their_family() {
    // A refused edit keeps the record (the cabinets do that); a dropped one
    // is the finding.
    let problems: Vec<String> = foreign_records()
        .into_iter()
        .flatten()
        .filter(|p| p.contains("was dropped"))
        .collect();
    assert!(problems.is_empty(), "{problems:?}");
}
