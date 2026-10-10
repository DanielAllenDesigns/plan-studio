//! Default Settings pages of the plan itself: General Plan Defaults, Plan
//! Defaults, Layout, Schedules (one page per schedule kind), the Text pages
//! (Rich Text, Arrows, Leader Lines, Callouts, Markers), Default Sets and the
//! Floors and Rooms pages (Floor Levels, Floor/Ceiling Platform, Rooms, Room
//! Label).
//!
//! Units, grid, snaps, room and floor heights edit the slots of
//! `PlanDefaults`; everything else is stored with the defaults
//! (`PlanDefaults::pages`).

use super::page::{Bind, Field as F, ListSource, PageSpec};
use crate::{bind_flag, bind_num, bind_text};
use plan_core::defaults::PageValue;

const NOTE: &str = "Saved with the plan defaults.";

/// Default Settings > General > General Plan Defaults.
pub fn general() -> PageSpec {
    PageSpec::new("general", "General Plan Defaults")
        .section(
            "Units",
            vec![
                F::flag("general.imperial", "Imperial Units (Feet and Inches)", true)
                    .bound(bind_flag!(units.imperial)),
            ],
        )
        .section(
            "Grid and Snaps",
            vec![
                F::len("general.grid_spacing", "Reference Grid Spacing", 12.0)
                    .bound(bind_num!(grid.spacing)),
                F::len("general.grid_snap", "Grid Snap Spacing", 1.0).bound(bind_num!(grid.snap)),
                F::deg("general.angle_snap", "Angle Snap Increment", 15.0)
                    .bound(bind_num!(editing.angle_snap_deg)),
                F::num("general.snap_distance", "Snap Distance", 10.0, "px")
                    .bound(bind_num!(editing.snap_distance_px)),
                F::flag("general.bumping", "Bumping", true).bound(bind_flag!(editing.bumping)),
                F::len("general.bumping_distance", "Bumping Distance", 5.0)
                    .bound(bind_num!(editing.bumping_distance)),
            ],
        )
        .section(
            "Undo",
            vec![F::int(
                "general.undo_levels",
                "Undo Levels",
                100,
                (1, 1000),
                "",
            )],
        )
        .section(
            "Living Area",
            vec![
                F::pick(
                    plan_core::living::LIVING_TO_KEY,
                    "Living Area to",
                    &["Outside Surface", "Outside of Main Layer"],
                    "Outside Surface",
                ),
                F::flag(
                    plan_core::living::SHOW_LIVING_LABEL_KEY,
                    "Show Living Area Label",
                    true,
                ),
            ],
        )
        .section(
            "Walls",
            vec![
                F::flag(
                    "general.split_on_tee",
                    "Split a Wall Where Another Meets It",
                    true,
                )
                .bound(bind_flag!(walls_connect.split_on_tee)),
                F::len("general.connect_distance", "Connect Walls Within", 1.0)
                    .bound(bind_num!(walls_connect.connect_distance_min)),
            ],
        )
}

/// Default Settings > Plan > Plan Defaults.
pub fn plan_defaults() -> PageSpec {
    PageSpec::new("plan", "Plan Defaults")
        .note(NOTE)
        .section(
            "Drawing",
            vec![
                F::pick(
                    "plan.scale",
                    "Drawing Scale",
                    &[
                        "1/8\" = 1'",
                        "3/16\" = 1'",
                        "1/4\" = 1'",
                        "3/8\" = 1'",
                        "1/2\" = 1'",
                        "3/4\" = 1'",
                        "1\" = 1'",
                    ],
                    "1/4\" = 1'",
                ),
                F::deg("plan.north", "North Direction", 90.0),
                F::flag("plan.show_grid", "Show the Reference Grid", false),
            ],
        )
        .section(
            "Text",
            vec![
                F::text("plan.font", "Plan Font", "Arial").bound(bind_text!(text.font)),
                F::len("plan.text_height", "Text Height", 6.0).bound(bind_num!(text.height)),
                F::len("plan.label_height", "Label Text Height", 4.5)
                    .bound(bind_num!(text.label_style_height)),
            ],
        )
}

/// Default Settings > Layout.
pub fn layout() -> PageSpec {
    PageSpec::new("layout", "Layout")
        .note(NOTE)
        .section(
            "Page",
            vec![
                F::pick(
                    "layout.page_size",
                    "Page Size",
                    &["ARCH C 18x24", "ARCH D 24x36", "ANSI B 11x17", "US Letter"],
                    "ARCH C 18x24",
                ),
                F::pick(
                    "layout.orientation",
                    "Orientation",
                    &["Landscape", "Portrait"],
                    "Landscape",
                ),
                F::len("layout.margin", "Margin", 0.5),
                F::flag("layout.border", "Page Border", true),
            ],
        )
        .section(
            "Box",
            vec![
                F::flag("layout.box_border", "Layout Box Border", true),
                F::len("layout.box_gap", "Gap Between Boxes", 0.25),
                F::flag("layout.box_title", "Box Title", true),
            ],
        )
        .section(
            "Text",
            vec![
                F::list(
                    "layout.text_style",
                    "Layout Text Style",
                    ListSource::TextStyles,
                    "Default Text Style",
                ),
                F::text("layout.title_block", "Title Block", ""),
            ],
        )
}

/// The schedule kinds with a page: `(slug, title)`.
pub const SCHEDULES: &[(&str, &str)] = &[
    ("door", "Door Schedule"),
    ("window", "Window Schedule"),
    ("room", "Room Schedule"),
    ("wall", "Wall Schedule"),
    ("cabinet", "Cabinet Schedule"),
    ("electrical", "Electrical Schedule"),
    ("framing", "Framing Schedule"),
    ("fixture", "Fixture Schedule"),
    ("furniture", "Furniture Schedule"),
    ("plant", "Plant Schedule"),
    ("stair", "Stair Schedule"),
    ("room_finish", "Room Finish Schedule"),
    ("note", "Note Schedule"),
    ("general", "General Schedule"),
];

/// Default Settings > Schedules > one kind.
pub fn schedule(slug: &str) -> Option<PageSpec> {
    let (_, title) = SCHEDULES.iter().find(|(s, _)| *s == slug)?;
    let id = format!("schedules.{slug}");
    let p = id.as_str();
    Some(
        PageSpec::new(p, title)
            .note(NOTE)
            .section(
                "Table",
                vec![
                    F::text(&format!("{p}.title"), "Title", title),
                    F::flag(&format!("{p}.show_title"), "Show the Title", true),
                    F::flag(&format!("{p}.border"), "Border", true),
                    F::flag(&format!("{p}.grid_lines"), "Grid Lines", true),
                    F::len(&format!("{p}.row_height"), "Row Height", 0.0),
                    F::list(
                        &format!("{p}.text_style"),
                        "Text Style",
                        ListSource::TextStyles,
                        "Default Text Style",
                    ),
                ],
            )
            .section(
                "Rows",
                vec![
                    F::flag(&format!("{p}.number_rows"), "Number the Rows", true),
                    F::flag(&format!("{p}.sort"), "Sort by Mark", true),
                    F::flag(&format!("{p}.callouts"), "Show Callouts in the Plan", true),
                ],
            ),
    )
}

/// The Text pages: `(slug, title)`.
pub const TEXT_PAGES: &[(&str, &str)] = &[
    ("rich", "Rich Text"),
    ("arrows", "Arrows"),
    ("leader_lines", "Leader Lines"),
    ("callouts", "Callouts"),
    ("markers", "Markers"),
];

/// Default Settings > Text > one of the Text pages (Text Styles is its own
/// list).
pub fn text_page(slug: &str) -> Option<PageSpec> {
    let (_, title) = TEXT_PAGES.iter().find(|(s, _)| *s == slug)?;
    let id = format!("text.{slug}");
    let p = id.as_str();
    let spec = PageSpec::new(p, title).note(NOTE);
    Some(match slug {
        "rich" => spec.section(
            "Rich Text",
            vec![
                F::list(
                    &format!("{p}.style"),
                    "Default Rich Text Style",
                    ListSource::TextStyles,
                    "Default Text Style",
                ),
                F::pick(
                    &format!("{p}.align"),
                    "Alignment",
                    &["Left", "Center", "Right"],
                    "Left",
                ),
                F::flag(&format!("{p}.border"), "Display Border", false),
                F::len(&format!("{p}.margin"), "Margins", 0.375),
                F::flag(&format!("{p}.fill"), "Opaque Background", false),
            ],
        ),
        "arrows" => spec.section(
            "Arrows",
            vec![
                F::pick(
                    &format!("{p}.style"),
                    "Arrow Style",
                    &["Open", "Closed", "Filled", "Tick", "Dot"],
                    "Open",
                ),
                F::len(&format!("{p}.size"), "Arrow Size", 6.0),
                F::deg(&format!("{p}.angle"), "Arrow Head Angle", 20.0),
            ],
        ),
        "leader_lines" => spec.section(
            "Leader Lines",
            vec![
                F::pick(
                    &format!("{p}.style"),
                    "Line Style",
                    &["Solid", "Dashed", "Dotted"],
                    "Solid",
                ),
                F::num(&format!("{p}.weight"), "Line Weight", 0.5, "pt"),
                F::len(&format!("{p}.landing"), "Landing Length", 6.0),
                F::flag(&format!("{p}.elbow"), "Elbow", true),
            ],
        ),
        "callouts" => spec.section(
            "Callouts",
            vec![
                F::pick(
                    &format!("{p}.border"),
                    "Border",
                    &["None", "Box", "Circle", "Rounded Box"],
                    "None",
                ),
                F::list(
                    &format!("{p}.style"),
                    "Text Style",
                    ListSource::TextStyles,
                    "Default Text Style",
                ),
                F::flag(&format!("{p}.leader"), "Leader Line", true),
            ],
        ),
        "markers" => spec.section(
            "Markers",
            vec![
                F::pick(
                    &format!("{p}.style"),
                    "Marker Style",
                    &["Circle", "Hexagon", "Diamond", "Square"],
                    "Circle",
                ),
                F::len(&format!("{p}.size"), "Size", 12.0),
                F::flag(&format!("{p}.number"), "Show a Number or Letter", true),
            ],
        ),
        _ => return None,
    })
}

fn active_set() -> Bind {
    Bind {
        get: |d| PageValue::Text(d.active_dimension_set.clone()),
        set: |d, v| {
            let name = v.text();
            if !d.set_active_dimension_set(&name) {
                d.active_dimension_set = name;
            }
        },
    }
}

/// Default Settings > Default Sets.
pub fn default_sets() -> PageSpec {
    PageSpec::new("default_sets", "Default Sets")
        .note("Pick the saved dimension default set new dimensions use; the Dimensions page edits the sets.")
        .section(
            "Active Sets",
            vec![F::list(
                "default_sets.dimension",
                "Dimension Defaults",
                ListSource::DimensionSets,
                "1/4\" Scale",
            )
            .bound(active_set())],
        )
}

/// Default Settings > Floors and Rooms > Floor Levels.
pub fn floor_levels() -> PageSpec {
    PageSpec::new("floor_levels", "Floor Levels")
        .section(
            "New Floors",
            vec![
                F::len("floor_levels.ceiling", "Ceiling Height", 108.0)
                    .bound(bind_num!(rooms.ceiling_height)),
                F::len(
                    "floor_levels.foundation_height",
                    "Foundation Wall Height",
                    96.0,
                )
                .bound(bind_num!(foundation_wall.height)),
                F::len("floor_levels.wall_height", "Exterior Wall Height", 109.125)
                    .bound(bind_num!(exterior_wall.height)),
            ],
        )
        .section(
            "Names",
            vec![
                F::text("floor_levels.first", "First Floor Name", "First Floor"),
                F::text("floor_levels.next", "Upper Floor Name", "Second Floor"),
            ],
        )
}

/// Default Settings > Floors and Rooms > Floor/Ceiling Platform.
pub fn platforms() -> PageSpec {
    PageSpec::new("platforms", "Floor/Ceiling Platform").section(
        "Platforms",
        vec![
            F::len("platforms.floor", "Floor Platform Thickness", 11.875)
                .bound(bind_num!(rooms.floor.floor_structure_thickness)),
            F::len("platforms.ceiling", "Ceiling Platform Thickness", 0.0)
                .bound(bind_num!(rooms.floor.ceiling_structure_thickness)),
            F::len("platforms.floor_finish", "Floor Finish Thickness", 0.75)
                .bound(bind_num!(rooms.floor.floor_finish_thickness)),
            F::len(
                "platforms.ceiling_finish",
                "Ceiling Finish Thickness",
                0.625,
            )
            .bound(bind_num!(rooms.floor.ceiling_finish_thickness)),
        ],
    )
}

/// Default Settings > Floors and Rooms > Rooms.
pub fn rooms() -> PageSpec {
    PageSpec::new("rooms", "Rooms")
        .section(
            "New Rooms",
            vec![
                F::list("rooms.type", "Default Room Type", ListSource::RoomTypes, "")
                    .bound(bind_text!(rooms.floor.default_room_type)),
                F::text("rooms.floor_material", "Floor Material", "")
                    .bound(bind_text!(rooms.floor.floor_material)),
                F::text("rooms.ceiling_material", "Ceiling Material", "")
                    .bound(bind_text!(rooms.floor.ceiling_material)),
                F::text("rooms.wall_material", "Wall Material", "")
                    .bound(bind_text!(rooms.floor.wall_material)),
            ],
        )
        .section(
            "Detection",
            vec![
                F::flag("rooms.auto_detect", "Detect Rooms From Walls", true),
                F::len("rooms.min_size", "Smallest Room Dimension", 12.0),
            ],
        )
}

/// Default Settings > Floors and Rooms > Room Label.
pub fn room_label() -> PageSpec {
    PageSpec::new("room_label", "Room Label")
        .note(NOTE)
        .section(
            "Content",
            vec![
                F::flag("room_label.name", "Show the Room Name", true),
                F::flag("room_label.area", "Show the Area", true),
                F::flag("room_label.dimensions", "Show the Dimensions", false),
                F::flag("room_label.ceiling", "Show the Ceiling Height", false),
            ],
        )
        .section(
            "Appearance",
            vec![
                F::list(
                    "room_label.text_style",
                    "Text Style",
                    ListSource::TextStyles,
                    "Default Text Style",
                ),
                F::flag("room_label.border", "Display Border", false),
                F::pick(
                    "room_label.align",
                    "Alignment",
                    &["Left", "Center", "Right"],
                    "Center",
                ),
            ],
        )
}
