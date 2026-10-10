//! Tutorial Guide lessons 15 to 28 replayed headlessly (round 16, brief 35).
//! Helpers shared by these lessons live in `support_b`; the shared ones in
//! `scenarios/tutorials_support.rs`.
mod support_b;
mod tut15_cabinet_layout;
mod tut16_appliances_fixtures;
mod tut17_light_fixtures;
mod tut18_electrical_objects;
mod tut19_floor_framing;
mod tut20_wall_framing;
mod tut21_roof_ceiling_framing;
mod tut22_plot_plans;
mod tut23_terrain_elevation;
mod tut24_driveways_roads;
mod tut25_landscaping;
mod tut26_layout_page_templates;
mod tut27_title_blocks_borders;
mod tut28_send_to_layout;

macro_rules! lessons {
    ($($n:literal => $f:literal),* $(,)?) => {
        &[$(($n, include_str!(concat!($f, ".rs")))),*]
    };
}

/// Lists the ignored tutorial tests by lesson (prints, asserts nothing).
#[test]
fn tutorial_b_replays_ignored_by_lesson() {
    let all: &[(u32, &str)] = lessons!(
        15 => "tut15_cabinet_layout", 16 => "tut16_appliances_fixtures",
        17 => "tut17_light_fixtures", 18 => "tut18_electrical_objects",
        19 => "tut19_floor_framing", 20 => "tut20_wall_framing",
        21 => "tut21_roof_ceiling_framing", 22 => "tut22_plot_plans",
        23 => "tut23_terrain_elevation", 24 => "tut24_driveways_roads",
        25 => "tut25_landscaping", 26 => "tut26_layout_page_templates",
        27 => "tut27_title_blocks_borders", 28 => "tut28_send_to_layout",
    );
    for (lesson, src) in all {
        let green = src.matches("#[test]").count();
        let ids: Vec<&str> = src
            .lines()
            .filter_map(|l| l.trim().strip_prefix("#[ignore = \""))
            .map(|l| l.trim_end_matches("\")]").trim_end_matches("\"]"))
            .collect();
        println!(
            "lesson {lesson:>2}: {} tests, {} ignored {ids:?}",
            green,
            ids.len()
        );
    }
}
