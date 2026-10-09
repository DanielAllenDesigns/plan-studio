//! Default Settings > Dimension, next to the Dimensions list of saved
//! default sets (Primary Format, Setup Automatic, Extensions, Arrow, Text
//! Style, Locate Objects live there): the General and Setup Temporary panels
//! of Chief's Dimension Defaults dialog and the Auto Story Pole Dimension
//! Defaults dialog, with Chief's panel and field names in Chief's order
//! (manual pages 478 to 496, in our own words).
//!
//! A field the model has (`DimensionDefaults`: baseline separation, reach,
//! leader style) edits the active default set and is read by the dimension
//! tools. The rest is stored with the defaults (`dimension.<panel>.<field>`),
//! marked with `*` on the page, and no tool reads it yet
//! (docs/integration-queue.md).

use super::page::{Bind, Field as F, PageSpec};
use plan_core::defaults::{PageValue, PlanDefaults};

/// The dimension pages in tree order: `(slug, title)`.
pub const LEAVES: &[(&str, &str)] = &[
    ("general", "General"),
    ("setup_temporary", "Setup Temporary"),
    ("auto_story_pole", "Auto Story Pole Dimensions"),
];

/// Copies the edited dimension defaults into the active saved set, which
/// mirrors them.
fn sync(d: &mut PlanDefaults) {
    let name = d.active_dimension_set.clone();
    let auto = d.dimensions.clone();
    if let Some(set) = d.dimension_sets.iter_mut().find(|s| s.name == name) {
        set.auto = auto;
    }
}

macro_rules! dim_num {
    ($f:ident) => {
        Bind {
            get: |d| PageValue::Num(d.dimensions.$f),
            set: |d, v| {
                d.dimensions.$f = v.num();
                sync(d);
            },
        }
    };
}

macro_rules! dim_text {
    ($f:ident) => {
        Bind {
            get: |d| PageValue::Text(d.dimensions.$f.clone()),
            set: |d, v| {
                d.dimensions.$f = v.text();
                sync(d);
            },
        }
    };
}

/// The page of dimension panel `slug`.
pub fn page(slug: &str) -> Option<PageSpec> {
    let (_, title) = LEAVES.iter().find(|(s, _)| *s == slug)?;
    let id = format!("dimension.{slug}");
    let p = id.as_str();
    let spec = PageSpec::new(p, title);
    let k = |name: &str| format!("{p}.{name}");
    Some(match slug {
        // Dimension Defaults > General panel.
        "general" => spec
            .note("Baseline Line Separation, Reach and Leader Style edit the active dimension default set.")
            .section(
                "General",
                vec![
                    F::len(&k("baseline_separation"), "Baseline Line Separation", 12.0)
                        .bound(dim_num!(baseline_separation)),
                    F::len(&k("reach"), "Reach", 24.0).bound(dim_num!(reach)),
                ],
            )
            .section(
                "Rounded Value Indicators",
                vec![
                    F::flag(&k("plus_minus_after"), "+ or - After Number", false),
                    F::flag(&k("tilde_before"), "~ Before Number", false),
                ],
            )
            .section(
                "Rounding Method",
                vec![F::pick(
                    &k("rounding"),
                    "Rounding Method",
                    &["Grid Rounding", "Distance Rounding"],
                    "Grid Rounding",
                )],
            )
            .section(
                "Dimension Text Position and Orientation",
                vec![
                    F::pick(
                        &k("text_position"),
                        "Position",
                        &[
                            "Centered on Dimension Line",
                            "Above Dimension Line",
                            "Below Dimension Line",
                        ],
                        "Above Dimension Line",
                    ),
                    F::flag(&k("angle_automatic"), "Automatic", true),
                    F::deg(&k("angle"), "Angle", 0.0),
                ],
            )
            .section(
                "Leader Line",
                vec![
                    F::pick(
                        &k("leader_style"),
                        "Leader Style",
                        &["None", "Square Corner", "Round Corner", "Diagonal"],
                        "Square Corner",
                    )
                    .bound(dim_text!(leader_style)),
                    F::flag(&k("second_segment"), "Include Second Segment", false),
                    F::len(&k("second_segment_length"), "Second Segment Length", 12.0),
                ],
            )
            .section(
                "Include Arrow",
                vec![
                    F::flag(&k("leader_arrow"), "Include Arrow", false),
                    F::pick(
                        &k("leader_arrow_style"),
                        "Style",
                        &["Match Dimension", "Arrow", "Tick", "Dot"],
                        "Match Dimension",
                    ),
                    F::flag(&k("leader_arrow_match_size"), "Match Dimension", true),
                    F::len(&k("leader_arrow_size"), "Size", 6.0),
                ],
            )
            .section(
                "3D Display",
                vec![
                    F::flag(&k("extend_extensions"), "Extend Extensions to Mark", true),
                    F::flag(&k("label_faces_camera"), "Label Faces Camera", false),
                ],
            ),
        // Dimension Defaults > Setup Temporary panel.
        "setup_temporary" => spec
            .section(
                "Options",
                vec![
                    F::int(&k("row_limit"), "Dimension Row Limit", 2, (1, 20), ""),
                    F::len(&k("reach"), "Reach", 48.0),
                ],
            )
            .section(
                "Walls",
                vec![F::pick(
                    &k("walls"),
                    "Locate Walls At",
                    &["Surfaces", "Wall Dimension Layer"],
                    "Surfaces",
                )],
            )
            .section(
                "Wall Options: Exterior Walls",
                vec![
                    F::flag(&k("exterior_primary"), "Primary Side", true),
                    F::flag(&k("exterior_secondary"), "Secondary Side", false),
                ],
            )
            .section(
                "Wall Options: Interior Walls",
                vec![
                    F::flag(&k("interior_primary"), "Primary Side", true),
                    F::flag(&k("interior_secondary"), "Secondary Side", true),
                    F::flag(&k("interior_centers"), "Centers", false),
                ],
            )
            .section(
                "Locate Objects Inside",
                vec![
                    F::flag(&k("inside_cad"), "CAD Objects", true),
                    F::flag(&k("inside_terrain"), "Terrain Objects", true),
                ],
            ),
        // Auto Story Pole Dimension Defaults dialog.
        "auto_story_pole" => spec
            .note("General, Inner Format, Outer Format and Marker Format panels. Locate Objects, Locate Elevations and Layer are not built yet.")
            .section(
                "Position",
                vec![
                    F::flag(&k("left"), "Dimension on Left", true),
                    F::int(&k("left_reach"), "Left Reach", 100, (1, 100), "%"),
                    F::flag(&k("right"), "Dimension on Right", false),
                    F::int(&k("right_reach"), "Right Reach", 100, (1, 100), "%"),
                    F::len(&k("line_separation"), "Line Separation", 12.0),
                    F::len(&k("first_line_offset"), "First Line Offset", 24.0),
                ],
            )
            .section(
                "Options",
                vec![
                    F::flag(&k("inner"), "Inner Dimension", true),
                    F::flag(&k("between_markers"), "Dimensions Between Elevation Markers", true),
                    F::flag(&k("primary_ridges"), "Primary Ridge Marks Only", true),
                    F::flag(&k("primary_heights"), "Primary Height Marks Only", true),
                ],
            )
            .section(
                "Inner Format",
                vec![F::flag(&k("inner_default"), "Use Default Formatting", true)],
            )
            .section(
                "Outer Format",
                vec![F::flag(&k("outer_default"), "Use Default Formatting", true)],
            )
            .section(
                "Marker Format",
                vec![F::flag(&k("marker_default"), "Use Default Formatting", true)],
            ),
        _ => return None,
    })
}
