//! Lesson 24, Driveways, Sidewalks and Roads (pp. 420-433). Real: a road,
//! a driveway and a sidewalk are each one undo step and one strip. Open:
//! curb cuts, flare, stripes and Auto Generate Sidewalks.
use super::support_b::*;
use crate::editor::site_view::load_terrain;
use crate::scenarios::tutorials_support::*;
use crate::tools::terrain::TerrainVariant as V;

#[test]
fn road_driveway_and_sidewalk_each_add_one_strip() {
    let mut sim = cottage_lot();
    let strips = |s: &crate::scenarios::Sim| {
        load_terrain(&s.app.cx.project).map_or(0, |r| r.terrain.roads.len())
    };
    let base = strips(&sim);
    assert_one_undo_step(&mut sim, "road", |s| {
        draw_terrain(s, V::Road, &[(-250.0, 700.0), (850.0, 700.0)], &["20'"])
    });
    assert_eq!(strips(&sim), base + 1);
    assert_one_undo_step(&mut sim, "driveway", |s| {
        draw_terrain(s, V::Driveway, &[(540.0, 480.0), (540.0, 700.0)], &[""])
    });
    assert_one_undo_step(&mut sim, "sidewalk", |s| {
        draw_terrain(s, V::Sidewalk, &[(100.0, 520.0), (400.0, 520.0)], &[""])
    });
    assert!(strips(&sim) >= base + 3, "three strips");
}

#[test]
#[ignore = "T7-24: CB-629 (driveway 3 ft end flare, Blacktop)"]
fn driveway_flare() {
    assert_ignored_break("CB-629");
}

#[test]
#[ignore = "T7-24: CB-588 (Road Stripe 6 ft from the edge)"]
fn road_stripe() {
    assert_ignored_break("CB-588");
}

fn drawn_road_and_driveway() -> plan_terrain::Terrain {
    let mut sim = cottage_lot();
    draw_terrain(
        &mut sim,
        V::Road,
        &[(-250.0, 700.0), (850.0, 700.0)],
        &["20'"],
    );
    draw_terrain(
        &mut sim,
        V::Driveway,
        &[(540.0, 480.0), (540.0, 700.0)],
        &[""],
    );
    load_terrain(&sim.app.cx.project).unwrap().terrain
}

#[test]
fn the_road_curb_is_cut_where_the_driveway_crosses() {
    use plan_terrain::{build_terrain, road_meshes};
    let mut t = drawn_road_and_driveway();
    let road = t
        .roads
        .iter()
        .position(|r| r.kind == plan_terrain::RoadKind::Road)
        .unwrap();
    t.roads[road].curb = true;
    let tris = |t: &plan_terrain::Terrain| {
        let surface = build_terrain(t);
        road_meshes(t, &surface)
            .iter()
            .map(|m| m.indices.len())
            .sum::<usize>()
    };
    t.roads[road].cut_curb = true;
    let cut = tris(&t);
    t.roads[road].cut_curb = false;
    let whole = tris(&t);
    assert!(
        cut != whole,
        "the cut changes the curb geometry: {cut} vs {whole}"
    );
}

#[test]
fn auto_generated_sidewalks_sit_sixty_inches_off_the_road() {
    use plan_terrain::{auto_sidewalks, AutoSidewalk};
    let t = drawn_road_and_driveway();
    let road = t
        .roads
        .iter()
        .position(|r| r.kind == plan_terrain::RoadKind::Road)
        .unwrap();
    let walks = auto_sidewalks(
        &t,
        road,
        &AutoSidewalk {
            all_connected: false,
            right: false,
            offset: 60.0,
            ..AutoSidewalk::default()
        },
    );
    assert_eq!(walks.len(), 1);
    let gap = (walks[0].centerline[0].y - t.roads[road].centerline[0].y).abs();
    assert!((gap - (t.roads[road].width / 2.0 + walks[0].width / 2.0 + 60.0)).abs() < 1e-6);
}
