//! Lesson 23, Terrain Elevation (pp. 403-420). Real: elevation lines drawn
//! with the tool at two heights give the slope the data says; each is one
//! undo step. Open: the reference point, curved lines and the crossing check.
use super::support_b::*;
use crate::editor::site_view::load_terrain;
use crate::scenarios::tutorials_support::*;
use crate::tools::terrain::TerrainVariant as V;

#[test]
fn two_elevation_lines_give_the_stated_slope() {
    let mut sim = cottage_lot();
    assert_one_undo_step(&mut sim, "first line", |s| {
        draw_terrain(
            s,
            V::ElevationLine,
            &[(-250.0, -200.0), (-250.0, 700.0)],
            &["0"],
        )
    });
    assert_one_undo_step(&mut sim, "second line", |s| {
        draw_terrain(
            s,
            V::ElevationLine,
            &[(850.0, -200.0), (850.0, 700.0)],
            &["-96"],
        )
    });
    let t = load_terrain(&sim.app.cx.project)
        .expect("a terrain")
        .terrain;
    assert_eq!(t.elevation_lines.len(), 2);
    let run = (t.elevation_lines[1].points[0].x - t.elevation_lines[0].points[0].x).abs();
    let slope = (t.elevation_lines[1].z - t.elevation_lines[0].z) / run;
    assert!((slope + 96.0 / 1100.0).abs() < 1e-9, "{slope}");
}

#[test]
#[ignore = "T7-23: CB-530 (Terrain Specification: Absolute Elevation -28 at the Reference Point on the garage door)"]
fn reference_point_absolute_elevation() {
    assert_ignored_break("CB-530");
}

#[test]
#[ignore = "T7-23: CAD-22 (Change Line/Arc: curved elevation line radius 450 ft)"]
fn curved_elevation_line() {
    assert_ignored_break("CAD-22");
}

#[test]
#[ignore = "T7-23: CB-512 (elevation lines at different heights never cross: warning)"]
fn crossing_lines_warn() {
    assert_ignored_break("CB-512");
}
