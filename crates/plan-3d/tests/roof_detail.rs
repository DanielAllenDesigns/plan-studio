//! Roof detail: eave cuts, rafter tails, Roof Cuts Wall at Bottom, the lower
//! wall type split, half and pony walls under a roof, the baseline at the top
//! plate, garage stem walls (RF-14, RF-15, RF-16, RF-28, R-26, R-40).

use plan_3d::{
    build_scene, build_scene_covered, eave_elements, rafter_count, EaveKind, EaveOverrides,
    EavePlane, FloorRoofInput, Material, Mesh, RoofCover, RoofDetail, Scene, SceneOptions,
};
use plan_core::defaults::{EaveCut, RoofDetailDefaults, RoofWallKind};
use plan_core::rooms::{apply_function_defaults, function_defaults};
use plan_core::walls::PonyWall;
use plan_core::{
    Floor, Id, Opening, OpeningStyle, Point, Project, RoomName, WallClass, WallKind, WallLayer,
    WallTypeDef,
};
use plan_roof::{build_roof, build_roof_at_plate, EdgeKind, EdgeRoof, EdgeRoofSpec, Roof};
use serde_json::{json, Value};

const W: f64 = 480.0;
const D: f64 = 360.0;
const PLATE: f64 = 96.0;
const THICK: f64 = 6.0;
const OVER: f64 = 16.0 + THICK * 0.5;

fn corners() -> [Point; 4] {
    [
        Point::new(0.0, 0.0),
        Point::new(W, 0.0),
        Point::new(W, D),
        Point::new(0.0, D),
    ]
}

fn house() -> (Project, [Id; 4]) {
    let mut p = Project::new("house");
    let c = corners();
    let mut ids = [0; 4];
    for i in 0..4 {
        ids[i] = p.add_wall(0, c[i], c[(i + 1) % 4], THICK, PLATE, WallKind::Exterior);
    }
    (p, ids)
}

fn edge(kind: EdgeKind) -> EdgeRoof {
    EdgeRoof {
        pitch_in_12: 8.0,
        kind,
        overhang: OVER,
    }
}

fn gable_roof(baseline: f64) -> Roof {
    let e = [
        edge(EdgeKind::Hip),
        edge(EdgeKind::Gable),
        edge(EdgeKind::Hip),
        edge(EdgeKind::Gable),
    ];
    build_roof(&corners(), &e, baseline)
}

fn hip_roof(baseline: f64) -> Roof {
    build_roof(&corners(), &[edge(EdgeKind::Hip); 4], baseline)
}

fn plane_json(plane: &plan_roof::RoofPlane, id: Id) -> Value {
    json!({
        "kind": "plane",
        "id": id,
        "polygon3d": plane.polygon3d,
        "pitch": plane.pitch_in_12,
        "baseline": [plane.baseline.0, plane.baseline.1],
        "overhang": 16.0,
        "ridge_caps": false,
    })
}

fn store_roof(floor: &mut Floor, roof: &Roof) {
    floor.roofs = roof
        .planes
        .iter()
        .enumerate()
        .map(|(i, pl)| plane_json(pl, 9000 + i as u64))
        .collect();
}

fn settings(detail: &RoofDetailDefaults) -> Value {
    json!({ "kind": "settings", "detail": detail })
}

fn eave_planes() -> Vec<EavePlane> {
    gable_roof(PLATE)
        .planes
        .into_iter()
        .map(EavePlane::bare)
        .map(|mut e| {
            e.overhang = 16.0;
            e
        })
        .collect()
}

fn of(scene: &Scene, id: Id) -> Vec<&Mesh> {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .collect()
}

fn ys(meshes: &[&Mesh]) -> Vec<f64> {
    meshes
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| f64::from(v.position[1])))
        .collect()
}

fn max_y(meshes: &[&Mesh]) -> f64 {
    ys(meshes).into_iter().fold(f64::MIN, f64::max)
}

fn min_y(meshes: &[&Mesh]) -> f64 {
    ys(meshes).into_iter().fold(f64::MAX, f64::min)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

// ---------------------------------------------------------------- eave cuts

/// The fascia element of the south plane (the first plane) and the facts a
/// cut test needs.
struct Fascia {
    faces: Vec<([[f64; 3]; 4], [f64; 3])>,
    edge_dir: [f64; 3],
    plane_normal: [f64; 3],
    eave_y: f64,
}

fn fascia_with(cut: EaveCut) -> Fascia {
    let planes = eave_planes();
    let detail = RoofDetail {
        eave_cut: cut,
        ..RoofDetail::default()
    };
    let els = eave_elements(&planes, &detail);
    let fascia = els
        .iter()
        .find(|e| e.kind == EaveKind::Fascia && e.plane == 0)
        .expect("fascia on the first plane");
    let p = &planes[0].plane;
    let (a, b) = (p.polygon3d[0], p.polygon3d[1]);
    let len = ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
    Fascia {
        faces: fascia.faces.clone(),
        edge_dir: [(b[0] - a[0]) / len, 0.0, (b[2] - a[2]) / len],
        plane_normal: p.normal(),
        eave_y: a[1],
    }
}

/// Faces that run along the eave (their normal is square to it): the board's
/// big faces, not its two ends.
fn long_faces(f: &Fascia) -> Vec<[f64; 3]> {
    f.faces
        .iter()
        .map(|(_, n)| *n)
        .filter(|n| dot(*n, f.edge_dir).abs() < 1e-6)
        .collect()
}

fn lowest(f: &Fascia) -> f64 {
    f.faces
        .iter()
        .flat_map(|(q, _)| q.iter().map(|v| v[1]))
        .fold(f64::MAX, f64::min)
}

#[test]
fn a_plumb_cut_hangs_the_fascia_vertically() {
    let f = fascia_with(EaveCut::Plumb);
    let d = RoofDetail::default();
    assert!((f.eave_y - lowest(&f) - d.fascia_height).abs() < 1e-9);
    let n = long_faces(&f);
    assert!(
        n.iter()
            .any(|n| n[1].abs() < 1e-9 && dot(*n, f.plane_normal) < 0.0),
        "an outer face that is vertical, facing out"
    );
}

#[test]
fn a_level_cut_lays_the_fascia_flat_under_the_structure() {
    let f = fascia_with(EaveCut::Level);
    let d = RoofDetail::default();
    let drop_v = d.thickness / f.plane_normal[1];
    assert!(
        (lowest(&f) - (f.eave_y - drop_v - d.fascia_thickness)).abs() < 1e-9,
        "lowest at {}",
        lowest(&f)
    );
    assert!(
        long_faces(&f).iter().any(|n| (n[1] + 1.0).abs() < 1e-9),
        "a face looks straight down"
    );
}

#[test]
fn a_square_cut_stands_the_fascia_square_to_the_plane() {
    let f = fascia_with(EaveCut::Square);
    let d = RoofDetail::default();
    let n = long_faces(&f);
    assert!(
        n.iter()
            .any(|n| dot(*n, f.plane_normal).abs() < 1e-9 && n[1] < -0.3),
        "an end face square to the rafter, leaning down"
    );
    // The board is `fascia_height` long square to the plane: its lowest
    // point is that far along -normal from the eave top.
    let want = f.eave_y - d.fascia_height * f.plane_normal[1];
    assert!((lowest(&f) - want).abs() < 1e-9, "lowest {}", lowest(&f));
    // And it is not the plumb board.
    let plumb = fascia_with(EaveCut::Plumb);
    assert!(lowest(&f) > lowest(&plumb));
}

#[test]
fn a_plane_can_choose_its_own_cut_tails_and_gutters() {
    let mut planes = eave_planes();
    planes[0].opts = EaveOverrides {
        eave_cut: Some(EaveCut::Level),
        gutters: Some(true),
        fascia: Some(false),
        ..EaveOverrides::default()
    };
    let d = RoofDetail::default();
    let els = eave_elements(&planes, &d);
    let on = |k: EaveKind, plane: usize| {
        els.iter()
            .filter(|e| e.kind == k && e.plane == plane)
            .count()
    };
    assert_eq!(
        on(EaveKind::Fascia, 0),
        0,
        "the first plane turned fascia off"
    );
    assert_eq!(on(EaveKind::Fascia, 1), 1);
    assert_eq!(on(EaveKind::Gutter, 0), 1);
    assert_eq!(
        on(EaveKind::Gutter, 1),
        0,
        "gutters follow the roof's default"
    );
    assert!(EaveOverrides::default().is_default());
}

// ------------------------------------------------------------ rafter tails

#[test]
fn rafter_tails_stand_at_the_spacing_along_each_eave() {
    let planes = eave_planes();
    let tails = |d: &RoofDetail| {
        eave_elements(&planes, d)
            .iter()
            .filter(|e| e.kind == EaveKind::RafterTail)
            .count()
    };
    assert_eq!(tails(&RoofDetail::default()), 0, "tails are optional");
    let d = RoofDetail {
        rafter_tails: true,
        rafter_spacing: 24.0,
        rafter_width: 1.5,
        ..RoofDetail::default()
    };
    // Each plane's first edge is its eave.
    let expect: usize = planes
        .iter()
        .map(|p| {
            let (a, b) = (p.plane.polygon3d[0], p.plane.polygon3d[1]);
            let len = ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            rafter_count(len, 1.5, 24.0)
        })
        .sum();
    assert!(expect >= 40, "two 40' eaves at 24\" on center: {expect}");
    assert_eq!(tails(&d), expect);
    // Closer spacing, more tails.
    let close = RoofDetail {
        rafter_spacing: 16.0,
        ..d
    };
    assert!(tails(&close) > tails(&d));
    // Exposed tails replace the soffit on the eaves.
    let soffits = |d: &RoofDetail| {
        eave_elements(&planes, d)
            .iter()
            .filter(|e| e.kind == EaveKind::Soffit)
            .count()
    };
    assert_eq!(soffits(&RoofDetail::default()), 2);
    assert_eq!(soffits(&d), 0);
    // Counting helper: a 10' run at 16" holds 8 rafters, first flush.
    assert_eq!(rafter_count(120.0, 1.5, 16.0), 8);
}

// -------------------------------------------------- roof cuts wall at bottom

/// Floor 0 holds a low shed roof rising 6:12 to the north; a floor-1 wall
/// runs north-south over it, starting where the roof is below the wall's
/// bottom and ending where it is above.
fn wall_over_a_rising_roof(detail: Option<RoofDetailDefaults>) -> (Project, Id) {
    let mut p = Project::new("over-roof");
    p.floors.push(Floor::new("Second", 120.0));
    let sq = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 240.0),
        Point::new(0.0, 240.0),
    ];
    let mut e = [edge(EdgeKind::Shed); 4];
    e[0] = EdgeRoof {
        pitch_in_12: 6.0,
        kind: EdgeKind::Hip,
        overhang: OVER,
    };
    let roof = build_roof(&sq, &e, 60.0);
    assert_eq!(roof.planes.len(), 1);
    store_roof(&mut p.floors[0], &roof);
    if let Some(d) = detail {
        p.floors[0].roofs.push(settings(&d));
    }
    let wall = p.add_wall(
        1,
        Point::new(100.0, 40.0),
        Point::new(100.0, 200.0),
        THICK,
        96.0,
        WallKind::Exterior,
    );
    (p, wall)
}

/// Lowest scene height of the wall's vertices lying within 1" of plan `y`.
fn lowest_at(scene: &Scene, id: Id, y: f64) -> f64 {
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(id))
        .flat_map(|m| m.vertices.iter())
        .filter(|v| (f64::from(-v.position[2]) - y).abs() < 1.0)
        .map(|v| f64::from(v.position[1]))
        .fold(f64::MAX, f64::min)
}

#[test]
fn a_roof_below_a_wall_cuts_the_wall_bottom() {
    let (p, wall) = wall_over_a_rising_roof(None);
    let scene = build_scene(&p);
    // Surface at plan y: 60 + (y + 22) * 6 / 12; it passes the floor-1 wall
    // bottom (120) at y = 98.
    let surface = |y: f64| 60.0 + (y + OVER) * 0.5;
    assert!(
        (lowest_at(&scene, wall, 40.0) - 120.0).abs() < 1e-3,
        "where the roof is lower the wall keeps its own bottom"
    );
    let at_end = lowest_at(&scene, wall, 200.0);
    assert!(
        (at_end - surface(200.0)).abs() < 1.0,
        "the wall stands on the roof: {at_end} vs {}",
        surface(200.0)
    );
    assert!(at_end > 120.0 + 40.0);
    assert!((max_y(&of(&scene, wall)) - (120.0 + 96.0)).abs() < 1e-3);
}

#[test]
fn roof_cuts_wall_at_bottom_can_be_turned_off() {
    let off = RoofDetailDefaults {
        roof_cuts_wall_at_bottom: false,
        ..RoofDetailDefaults::default()
    };
    let (p, wall) = wall_over_a_rising_roof(Some(off));
    let scene = build_scene(&p);
    assert!((min_y(&of(&scene, wall)) - 120.0).abs() < 1e-3);
    assert!((lowest_at(&scene, wall, 200.0) - 120.0).abs() < 1e-3);
}

#[test]
fn without_attic_walls_the_wall_reaches_down_to_the_roof_below() {
    let no_attic = RoofDetailDefaults {
        auto_attic_walls: false,
        ..RoofDetailDefaults::default()
    };
    let (p, wall) = wall_over_a_rising_roof(Some(no_attic));
    let scene = build_scene(&p);
    // At the south end the roof surface is under the wall's 120" bottom: the
    // wall stands on it instead of hanging.
    let low = lowest_at(&scene, wall, 40.0);
    let surface = 60.0 + (40.0 + OVER) * 0.5;
    assert!(
        (low - surface).abs() < 1.0,
        "reaches {low}, roof at {surface}"
    );
}

// ----------------------------------------------------------- wall type split

fn typed(name: &str, layer: &str) -> WallTypeDef {
    WallTypeDef {
        props: Default::default(),
        name: name.into(),
        layers: vec![WallLayer::new(layer, 4.0, true, layer)],
        kind: WallKind::Exterior,
    }
}

fn material_ys(scene: &Scene, id: Id, m: Material) -> Vec<f64> {
    scene
        .meshes
        .iter()
        .filter(|x| x.object_id == Some(id) && x.material == m)
        .flat_map(|x| x.vertices.iter().map(|v| f64::from(v.position[1])))
        .collect()
}

#[test]
fn the_part_of_a_gable_above_the_plate_takes_the_attic_wall_type() {
    let (mut p, ids) = house();
    p.wall_types.push(typed("Brick-4", "Brick"));
    store_roof(&mut p.floors[0], &gable_roof(PLATE));
    p.floors[0].roofs.push(settings(&RoofDetailDefaults {
        attic_wall_type: "Brick-4".into(),
        ..RoofDetailDefaults::default()
    }));
    for i in [1, 3] {
        p.floors[0].walls[i].roof.kind = RoofWallKind::FullGable;
    }
    let scene = build_scene(&p);
    let brick = material_ys(&scene, ids[1], Material::Brick);
    assert!(
        !brick.is_empty(),
        "the upper part is marked with the attic type"
    );
    let lowest_brick = brick.iter().copied().fold(f64::MAX, f64::min);
    assert!(
        (lowest_brick - PLATE).abs() < 1e-3,
        "brick starts at the plate, not {lowest_brick}"
    );
    let plain = material_ys(&scene, ids[1], Material::WallExterior);
    assert!(plain.iter().copied().fold(f64::MIN, f64::max) <= max_y(&of(&scene, ids[1])) + 1e-6);
    // The wall that does not rise above its plate has no brick at all.
    assert!(material_ys(&scene, ids[0], Material::Brick).is_empty());
    // Without the setting the gable is one material.
    let (mut q, qid) = house();
    q.wall_types.push(typed("Brick-4", "Brick"));
    store_roof(&mut q.floors[0], &gable_roof(PLATE));
    for i in [1, 3] {
        q.floors[0].walls[i].roof.kind = RoofWallKind::FullGable;
    }
    let none = build_scene(&q);
    assert!(material_ys(&none, qid[1], Material::Brick).is_empty());
}

#[test]
fn the_part_of_a_wall_below_a_butting_roof_takes_the_lower_wall_type() {
    let (mut p, wall) = wall_over_a_rising_roof(Some(RoofDetailDefaults {
        roof_cuts_wall_at_bottom: false,
        lower_wall_type: "Stucco-6".into(),
        ..RoofDetailDefaults::default()
    }));
    p.wall_types.push(typed("Stucco-6", "Stucco"));
    let scene = build_scene(&p);
    let stucco = material_ys(&scene, wall, Material::Stucco);
    assert!(!stucco.is_empty(), "the lower part is marked");
    // It stays under the roof surface (about 60 + 222 * 0.5 = 171 at the
    // north end, 90 + ... at the south).
    let top = stucco.iter().copied().fold(f64::MIN, f64::max);
    assert!(top < 175.0 && top > 125.0, "stucco reaches {top}");
    // The wall above the roof line keeps the wall's own material.
    let own = material_ys(&scene, wall, Material::WallExterior);
    assert!(own.iter().copied().fold(f64::MIN, f64::max) > 200.0);
}

// ------------------------------------------------ half and pony under a roof

#[test]
fn half_and_pony_walls_are_cut_by_the_roof_like_standard_walls() {
    // A low hip roof (eave tip at 10"): its underside at the walls is about
    // 17" above the floor, under a half wall's 36".
    let roof = hip_roof(10.0);
    let underside = 10.0 + OVER * (8.0 / 12.0) - 6.0 * (1.0 + (8.0f64 / 12.0).powi(2)).sqrt();
    assert!(underside > 10.0 && underside < 30.0, "{underside}");
    let (mut p, ids) = house();
    store_roof(&mut p.floors[0], &roof);
    p.floors[0].walls[0].class = WallClass::HalfWall { height: 36.0 };
    p.floors[0].walls[1].class = WallClass::Pony {
        upper_type: "A".into(),
        lower_type: "B".into(),
        split_height: 12.0,
        upper_sets_plan_display: false,
    };
    // Flagged (not classed) half and pony walls take the same route.
    p.floors[0].walls[2].flags.half_wall = true;
    p.floors[0].walls[3].flags.pony = Some(PonyWall {
        lower_type: "B".into(),
        lower_height: 36.0,
    });
    let scene = build_scene(&p);
    for (i, id) in ids.iter().enumerate() {
        let top = max_y(&of(&scene, *id));
        assert!(
            top < 36.0 - 1.0,
            "wall {i} stops under the roof, not at 36: {top}"
        );
        assert!(
            top >= underside - 3.0,
            "wall {i} is not cut below the roof: {top}"
        );
    }
    // Without the roof the half wall is its full 36".
    let (mut q, qids) = house();
    q.floors[0].walls[0].class = WallClass::HalfWall { height: 36.0 };
    let flat = build_scene(&q);
    assert!((max_y(&of(&flat, qids[0])) - 36.0).abs() < 1e-3);
}

#[test]
fn a_curved_wall_is_cut_by_the_roof_facet_by_facet() {
    let roof = hip_roof(10.0);
    let (mut p, ids) = house();
    store_roof(&mut p.floors[0], &roof);
    p.floors[0].walls[0].curve = Some(plan_core::WallCurve { bulge: 20.0 });
    let scene = build_scene(&p);
    let top = max_y(&of(&scene, ids[0]));
    assert!(
        top < 40.0,
        "the curved wall stops under the low roof: {top}"
    );
    assert!(top > 5.0);
    // Without the roof it is the full 96" high.
    let (mut q, qids) = house();
    q.floors[0].walls[0].curve = Some(plan_core::WallCurve { bulge: 20.0 });
    assert!((max_y(&of(&build_scene(&q), qids[0])) - PLATE).abs() < 1e-3);
}

#[test]
fn a_foundation_wall_keeps_its_flat_top_under_the_roof() {
    // Below the floor, away from the roof planes: clipping leaves it alone.
    let roof = hip_roof(10.0);
    let (mut p, ids) = house();
    store_roof(&mut p.floors[0], &roof);
    p.floors[0].walls[0].class = WallClass::Foundation;
    p.floors[0].walls[0].foundation_height = 36.0;
    let scene = build_scene(&p);
    let w = of(&scene, ids[0]);
    assert!(max_y(&w).abs() < 1e-3, "top at the floor: {}", max_y(&w));
    assert!((min_y(&w) + 36.0).abs() < 1e-3);
}

#[test]
fn a_half_wall_is_never_raised_to_a_gable() {
    let (mut p, ids) = house();
    store_roof(&mut p.floors[0], &gable_roof(PLATE));
    p.floors[0].walls[1].roof.kind = RoofWallKind::FullGable;
    p.floors[0].walls[1].class = WallClass::HalfWall { height: 36.0 };
    let scene = build_scene(&p);
    assert!((max_y(&of(&scene, ids[1])) - 36.0).abs() < 1e-3);
}

// --------------------------------------------------------- baseline at plate

fn plate_specs() -> Vec<EdgeRoofSpec> {
    let hip = EdgeRoofSpec {
        pitch: 8.0,
        overhang: OVER,
        ..EdgeRoofSpec::default()
    };
    let gable = EdgeRoofSpec { gable: true, ..hip };
    vec![hip, gable, hip, gable]
}

/// Scene height of the gable wall's lowest top vertex: its corner.
fn gable_corner(roof: &Roof) -> f64 {
    let (mut p, ids) = house();
    store_roof(&mut p.floors[0], roof);
    for i in [1, 3] {
        p.floors[0].walls[i].roof.kind = RoofWallKind::FullGable;
    }
    let scene = build_scene(&p);
    // The corner is the wall's end at plan y = 0; its top is the lowest
    // vertex at the wall's face line above the plate region.
    scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(ids[1]))
        .flat_map(|m| m.vertices.iter())
        .filter(|v| f64::from(-v.position[2]) < 0.5 && f64::from(v.position[1]) > PLATE - 20.0)
        .map(|v| f64::from(v.position[1]))
        .fold(f64::MIN, f64::max)
}

#[test]
fn the_baseline_at_the_plate_closes_the_gap_at_the_gable_corner() {
    let specs = plate_specs();
    // The eave tip at the plate: the gable corner stands proud of it.
    let tip = plan_roof::build_roof_with_specs(&corners(), &specs, PLATE);
    let proud = gable_corner(&tip);
    assert!(
        proud > PLATE + 3.0,
        "the eave-tip rule leaves a gap of {}",
        proud - PLATE
    );
    // The underside at the plate: the corner is at plate height.
    let at_plate = build_roof_at_plate(&corners(), &specs, PLATE, RoofDetail::default().thickness);
    let corner = gable_corner(&at_plate);
    assert!(
        (corner - PLATE).abs() < 0.05,
        "gable corner at {corner}, plate {PLATE}"
    );
    // And the plane's underside meets the plate at the wall face line.
    let south = at_plate.planes.iter().find(|p| p.source_edge == 0).unwrap();
    let under = south
        .underside_at(Point::new(W * 0.5, 0.0), RoofDetail::default().thickness)
        .unwrap();
    assert!((under - PLATE).abs() < 1e-6);
}

// ------------------------------------------------------------ stem walls

fn garage_house(drop: bool) -> (Project, [Id; 4]) {
    let (mut p, ids) = house();
    let mut name = RoomName::new(Point::new(W * 0.5, D * 0.5), "Garage", "Garage");
    if drop {
        apply_function_defaults(&mut name, &function_defaults("Garage", "Garage"), 0.75);
    }
    p.floors[0].room_names.push(name);
    (p, ids)
}

#[test]
fn a_dropped_garage_gets_stem_walls_under_its_exterior_walls() {
    let (p, ids) = garage_house(true);
    let scene = build_scene(&p);
    let stems = |id: Id| -> Vec<&Mesh> {
        scene
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(id) && m.material == Material::Concrete)
            .collect()
    };
    for id in ids {
        let s = stems(id);
        assert_eq!(s.len(), 1, "one stem run under wall {id}");
        // From the underside of the 4" slab, 24" down, up to the datum.
        assert!((min_y(&s) + 28.0).abs() < 1e-3, "bottom {}", min_y(&s));
        assert!(max_y(&s).abs() < 1e-3, "top {}", max_y(&s));
    }
    // A garage whose floor is not dropped has none.
    let (flat, fids) = garage_house(false);
    let scene = build_scene(&flat);
    assert!(scene
        .meshes
        .iter()
        .all(|m| !(fids.contains(&m.object_id.unwrap_or(0)) && m.material == Material::Concrete)));
}

#[test]
fn stem_walls_stop_at_a_garage_door() {
    let (mut p, ids) = garage_house(true);
    let id = p.alloc_id();
    let mut door = Opening::default_door(id, ids[0], W * 0.5);
    door.style = OpeningStyle::Garage;
    door.width = 108.0;
    p.floors[0].openings.push(door);
    let scene = build_scene(&p);
    let runs: Vec<&Mesh> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(ids[0]) && m.material == Material::Concrete)
        .collect();
    assert_eq!(runs.len(), 2, "the stem is split around the door");
    let total: f64 = runs
        .iter()
        .map(|m| {
            let xs: Vec<f64> = m
                .vertices
                .iter()
                .map(|v| f64::from(v.position[0]))
                .collect();
            xs.iter().copied().fold(f64::MIN, f64::max)
                - xs.iter().copied().fold(f64::MAX, f64::min)
        })
        .sum();
    assert!((total - (W - 108.0)).abs() < 1.0, "stem length {total}");
}

#[test]
fn a_room_stem_wall_height_reaches_that_far_below_the_datum() {
    let (mut p, ids) = house();
    let mut name = RoomName::new(Point::new(W * 0.5, D * 0.5), "Den", "Den");
    name.stem_wall_height = Some(18.0);
    p.floors[0].room_names.push(name);
    let scene = build_scene(&p);
    let stem: Vec<&Mesh> = scene
        .meshes
        .iter()
        .filter(|m| m.object_id == Some(ids[2]) && m.material == Material::Concrete)
        .collect();
    assert_eq!(stem.len(), 1);
    assert!((min_y(&stem) + 18.0).abs() < 1e-3);
}

// ------------------------------------------------------- stored roof detail

#[test]
fn the_detail_stored_with_the_roof_settings_drives_the_eave_meshes() {
    let (mut p, _) = house();
    store_roof(&mut p.floors[0], &gable_roof(PLATE));
    let stock = RoofCover::from_project(&p);
    assert_eq!(stock.floor(0).unwrap().detail.eave_cut, EaveCut::Plumb);
    let tails = stock.eave_meshes().len();
    p.floors[0].roofs.push(settings(&RoofDetailDefaults {
        eave_cut: EaveCut::Square,
        rafter_tails: true,
        ..RoofDetailDefaults::default()
    }));
    let cover = RoofCover::from_project(&p);
    let d = cover.floor(0).unwrap().detail;
    assert_eq!(d.eave_cut, EaveCut::Square);
    assert!(d.rafter_tails);
    assert!(
        cover
            .eave_meshes()
            .iter()
            .map(|m| m.vertices.len())
            .sum::<usize>()
            > stock
                .eave_meshes()
                .iter()
                .map(|m| m.vertices.len())
                .sum::<usize>(),
        "tails add geometry (stock had {tails} meshes)"
    );
    // A plane's own choice is read from its record.
    let mut q = p.clone();
    if let Some(Value::Object(m)) = q.floors[0].roofs.first_mut() {
        m.insert(
            "eave".into(),
            json!({ "eave_cut": "Level", "gutters": true }),
        );
    }
    let cover = RoofCover::from_project(&q);
    let first = &cover.floor(0).unwrap().eaves[0];
    assert_eq!(first.opts.eave_cut, Some(EaveCut::Level));
    assert_eq!(first.opts.gutters, Some(true));
}

#[test]
fn a_cover_made_by_hand_has_the_roof_given() {
    // The scene builder takes the cover as given, planes and detail.
    let (p, ids) = house();
    let input = vec![FloorRoofInput {
        planes: eave_planes(),
        ..FloorRoofInput::default()
    }];
    let cover = RoofCover::new(&p, input, RoofDetail::default());
    let scene = build_scene_covered(&p, &SceneOptions::default(), &[], &cover);
    assert!(!of(&scene, ids[0]).is_empty());
}
