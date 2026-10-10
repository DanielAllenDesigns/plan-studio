//! Round 16 layered platforms in 3D (brief 13; R-27..R-29, R-122, R-123): a
//! two-layer floor, a floor finish of tile over backerboard, a dropped
//! ceiling and a floor that follows the plan-wide definition.

use plan_3d::{build_scene, Material, Mesh};
use plan_core::assemblies::{
    Assembly, AssemblyKind, AssemblyLayer, AssemblySlot, LayerRole, Source,
};
use plan_core::{Point, Project, RoomName, WallKind};

const CEIL: f64 = 108.0;

fn one_room() -> Project {
    let mut p = Project::new("room");
    p.floors[0].ceiling_height = CEIL;
    let c = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 120.0),
        Point::new(0.0, 120.0),
    ];
    for i in 0..4 {
        p.add_wall(0, c[i], c[(i + 1) % 4], 6.0, CEIL, WallKind::Exterior);
    }
    p
}

fn meshes(p: &Project, material: Material) -> Vec<Mesh> {
    build_scene(p)
        .meshes
        .into_iter()
        .filter(|m| m.material == material)
        .collect()
}

fn span(ms: &[Mesh]) -> (f32, f32) {
    let ys: Vec<f32> = ms
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
        .collect();
    (
        ys.iter().cloned().fold(f32::MAX, f32::min),
        ys.iter().cloned().fold(f32::MIN, f32::max),
    )
}

fn layer(name: &str, role: LayerRole, t: f64) -> AssemblyLayer {
    AssemblyLayer::new(name, role, t)
}

fn floor_12() -> Assembly {
    Assembly::new(vec![
        layer("3/4 OSB", LayerRole::Sheathing, 0.75),
        layer("2x12", LayerRole::Framing, 11.25),
    ])
}

#[test]
fn a_two_layer_floor_is_two_slices_twelve_inches_deep() {
    let mut p = one_room();
    // Before: the single 1 in block.
    let (lo, hi) = span(&meshes(&p, Material::Floor));
    assert!((hi - lo - 1.0).abs() < 0.01, "{lo}..{hi}");
    assert!(meshes(&p, Material::Framing).is_empty());

    p.floors[0]
        .settings
        .platform
        .set(AssemblyKind::FloorStructure, AssemblySlot::Own(floor_12()));
    let framing = meshes(&p, Material::Framing);
    assert_eq!(framing.len(), 2, "one mesh per layer");
    let (lo, hi) = span(&framing);
    assert!(
        (hi - 0.0).abs() < 0.01,
        "the top is the platform datum, {hi}"
    );
    assert!((hi - lo - 12.0).abs() < 0.01, "12 in deep, {lo}..{hi}");
    // The slices stack without a gap: 0..-0.75 over -0.75..-12.
    let mut bands: Vec<(f32, f32)> = framing
        .iter()
        .map(|m| span(std::slice::from_ref(m)))
        .collect();
    bands.sort_by(|a, b| b.1.total_cmp(&a.1));
    assert!((bands[0].0 - -0.75).abs() < 0.01 && (bands[1].1 - -0.75).abs() < 0.01);
    // The older 3/4 in finish stands on the datum, as one block.
    let finish = span(&meshes(&p, Material::Floor));
    assert!(
        finish.0.abs() < 0.01 && (finish.1 - 0.75).abs() < 0.01,
        "{finish:?}"
    );
}

#[test]
fn tile_over_backerboard_stands_on_the_platform_datum() {
    let mut p = one_room();
    p.floors[0].settings.platform.set(
        AssemblyKind::FloorFinish,
        AssemblySlot::Own(Assembly::new(vec![
            layer("Ceramic Tile", LayerRole::Finish, 0.375),
            layer("Backerboard", LayerRole::Standard, 0.5),
        ])),
    );
    p.sync_platform_mirrors();
    assert!((p.floors[0].settings.floor_finish_thickness - 0.875).abs() < 1e-9);
    let tile = span(&meshes(&p, Material::Stone));
    let backer = span(&meshes(&p, Material::Concrete));
    assert!(
        (tile.0 - 0.5).abs() < 0.01 && (tile.1 - 0.875).abs() < 0.01,
        "{tile:?}"
    );
    assert!(
        (backer.0 - 0.0).abs() < 0.01 && (backer.1 - 0.5).abs() < 0.01,
        "{backer:?}"
    );
}

#[test]
fn a_dropped_ceiling_hangs_below_the_ceiling_height_under_the_same_wall_tops() {
    let mut p = one_room();
    let plain_ceiling = span(&meshes(&p, Material::Ceiling));
    assert!((plain_ceiling.0 - CEIL as f32).abs() < 0.01);
    let walls = span(&meshes(&p, Material::WallExterior));
    p.floors[0].settings.platform.set(
        AssemblyKind::CeilingFinish,
        AssemblySlot::Own(Assembly::new(vec![
            layer("Plenum", LayerRole::AirGap, 11.125),
            layer("Hat Channel", LayerRole::Framing, 0.875),
            layer("Drywall", LayerRole::Finish, 0.5),
        ])),
    );
    let ceiling = span(&meshes(&p, Material::Ceiling));
    // Drywall at the hung ceiling, the platform above the plenum.
    assert!(
        (ceiling.0 - (CEIL as f32 - 12.0)).abs() < 0.01,
        "{ceiling:?}"
    );
    assert!(
        (ceiling.1 - (CEIL as f32 + 1.5)).abs() < 0.01,
        "{ceiling:?}"
    );
    let hat = span(&meshes(&p, Material::Framing));
    assert!(
        (hat.0 - (CEIL as f32 - 11.5)).abs() < 0.01,
        "hat channel above the drywall {hat:?}"
    );
    assert!((hat.1 - hat.0 - 0.875).abs() < 0.01);
    // The plenum is empty: nothing between the hat channel and the platform.
    assert_eq!(
        span(&meshes(&p, Material::WallExterior)),
        walls,
        "wall tops stay"
    );
}

#[test]
fn a_room_with_its_own_definition_differs_from_the_floor_default() {
    let mut p = one_room();
    // A partition makes two rooms; only the east one gets the 12 in floor.
    p.add_wall(
        0,
        Point::new(120.0, 0.0),
        Point::new(120.0, 120.0),
        4.5,
        CEIL,
        WallKind::Interior,
    );
    let mut n = RoomName::new(Point::new(180.0, 60.0), "East", "Standard");
    let mut misc = plan_core::extras::RoomMisc::default();
    misc.assemblies
        .set(AssemblyKind::FloorStructure, AssemblySlot::Own(floor_12()));
    n.misc = Some(misc);
    p.floors[0].room_names.push(n);
    p.sync_platform_mirrors();
    let framing = meshes(&p, Material::Framing);
    assert_eq!(framing.len(), 2);
    for m in &framing {
        assert!(
            m.vertices.iter().all(|v| v.position[0] >= 119.0),
            "the layers are the east room's only"
        );
    }
    // The west room keeps the single block.
    assert!(meshes(&p, Material::Floor)
        .iter()
        .any(|m| m.vertices.iter().any(|v| v.position[0] < 10.0)));
}

#[test]
fn a_floor_following_the_plan_wide_definition_is_built_from_it() {
    let mut p = one_room();
    p.set_plan_wide_assembly(AssemblyKind::FloorStructure, Some(floor_12()));
    let f = &p.floors[0].settings;
    assert_eq!(f.platform.floor_structure, AssemblySlot::Default);
    let r = plan_core::assemblies::resolve_floor(AssemblyKind::FloorStructure, f);
    assert_eq!(r.source, Source::PlanWide);
    let (lo, hi) = span(&meshes(&p, Material::Framing));
    assert!((hi - lo - 12.0).abs() < 0.01);
    // The upper floor sits above a 12 in platform of its own.
    let idx = p.build_new_floor(true);
    assert_eq!(p.floors[idx].settings.floor_structure_thickness, 12.0);
    let f0 = &p.floors[0];
    assert!(
        (p.floors[idx].elevation - (f0.elevation + f0.ceiling_height + 12.0)).abs() < 1e-9,
        "{}",
        p.floors[idx].elevation
    );
}
