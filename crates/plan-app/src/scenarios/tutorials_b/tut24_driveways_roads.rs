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
#[ignore = "T7-24: CB-628 (road curbs cut where a driveway crosses)"]
fn curb_cut_at_driveway() {
    assert_ignored_break("CB-628");
}

#[test]
#[ignore = "T7-24: CB-629 (driveway 3 ft end flare, Blacktop)"]
fn driveway_flare() {
    assert_ignored_break("CB-629");
}

#[test]
#[ignore = "T7-24: CB-630 (Auto Generate Sidewalks offset 60 in from the road)"]
fn auto_sidewalks() {
    assert_ignored_break("CB-630");
}

#[test]
#[ignore = "T7-24: CB-588 (Road Stripe 6 ft from the edge)"]
fn road_stripe() {
    assert_ignored_break("CB-588");
}
