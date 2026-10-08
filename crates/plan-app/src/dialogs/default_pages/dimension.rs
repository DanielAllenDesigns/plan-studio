//! Default Settings > Dimension: one page per kind of dimension (Auto
//! Exterior, Auto Interior, NKBA, Baseline, Story Pole, Angular, Point to
//! Point, Temporary), next to the Dimensions list of saved default sets.
//!
//! The values the model has (`DimensionDefaults`: line separation, extension
//! lines, locate-interior-surfaces) edit the active default set; the rest is
//! stored with the defaults (`dimension.<kind>.<field>`).

use super::page::{Bind, Field as F, PageSpec};
use plan_core::defaults::{PageValue, PlanDefaults};

/// The dimension pages in tree order: `(slug, title)`.
pub const LEAVES: &[(&str, &str)] = &[
    ("auto_exterior", "Auto Exterior"),
    ("auto_interior", "Auto Interior"),
    ("nkba", "NKBA"),
    ("baseline", "Baseline"),
    ("story_pole", "Story Pole"),
    ("angular", "Angular"),
    ("point_to_point", "Point to Point"),
    ("temporary", "Temporary"),
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

macro_rules! dim_flag {
    ($f:ident) => {
        Bind {
            get: |d| PageValue::Bool(d.dimensions.$f),
            set: |d, v| {
                d.dimensions.$f = v.flag();
                sync(d);
            },
        }
    };
}

/// The page of dimension kind `slug`.
pub fn page(slug: &str) -> Option<PageSpec> {
    let (_, title) = LEAVES.iter().find(|(s, _)| *s == slug)?;
    let id = format!("dimension.{slug}");
    let p = id.as_str();
    let spec = PageSpec::new(p, title);
    Some(match slug {
        "auto_exterior" => spec
            .note("Offsets and spacing edit the active dimension default set.")
            .section(
                "Strings",
                vec![
                    F::len(&format!("{p}.offset"), "Distance From the Wall", 24.0)
                        .bound(dim_num!(auto_exterior_offset)),
                    F::len(&format!("{p}.separation"), "Line Separation", 18.0)
                        .bound(dim_num!(auto_line_separation)),
                    F::len(&format!("{p}.reach"), "Reach", 0.0).bound(dim_num!(reach)),
                ],
            )
            .section(
                "Include",
                vec![
                    F::flag(&format!("{p}.openings"), "Openings", true),
                    F::flag(&format!("{p}.wall_to_wall"), "Wall to Wall", true),
                    F::flag(&format!("{p}.overall"), "Overall", true),
                ],
            ),
        "auto_interior" => spec
            .note("Spacing edits the active dimension default set.")
            .section(
                "Interior",
                vec![
                    F::flag(&format!("{p}.interior_surfaces"), "Locate Interior Wall Surfaces", true)
                        .bound(dim_flag!(interior_locates_interior_surfaces)),
                    F::len(&format!("{p}.offset"), "Distance From the Wall", 12.0),
                    F::flag(&format!("{p}.openings"), "Include Openings", true),
                    F::flag(&format!("{p}.cabinets"), "Include Cabinets", false),
                ],
            ),
        "nkba" => spec
            .note("Saved with the plan defaults.")
            .section(
                "NKBA",
                vec![
                    F::flag(&format!("{p}.show_clearances"), "Show Clearance Dimensions", true),
                    F::len(&format!("{p}.aisle"), "Work Aisle", 42.0),
                    F::len(&format!("{p}.walkway"), "Walkway", 36.0),
                    F::flag(&format!("{p}.locate_appliances"), "Locate Appliances", true),
                ],
            ),
        "baseline" => spec
            .note("Separation edits the active dimension default set.")
            .section(
                "Baseline",
                vec![
                    F::len(&format!("{p}.separation"), "Line Separation", 12.0)
                        .bound(dim_num!(baseline_separation)),
                    F::len(&format!("{p}.start_offset"), "First Line Offset", 18.0),
                    F::flag(&format!("{p}.cumulative"), "Show Cumulative Distances", true),
                ],
            ),
        "story_pole" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Story Pole",
                vec![
                    F::len(&format!("{p}.pole_width"), "Pole Width", 6.0),
                    F::len(&format!("{p}.tick_length"), "Tick Length", 4.0),
                    F::flag(&format!("{p}.label_floors"), "Label the Floors", true),
                    F::flag(&format!("{p}.show_plates"), "Mark Plate Heights", true),
                ],
            ),
        "angular" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Angular",
                vec![
                    F::len(&format!("{p}.radius"), "Arc Radius", 24.0),
                    F::int(&format!("{p}.decimals"), "Decimal Places", 1, (0, 4), ""),
                    F::flag(&format!("{p}.show_degrees_sign"), "Show the Degree Sign", true),
                ],
            ),
        "point_to_point" => spec
            .note("Extension lines edit the active dimension default set.")
            .section(
                "Extension Lines",
                vec![
                    F::len(&format!("{p}.gap"), "Gap to the Object", 2.0)
                        .bound(dim_num!(extension_gap)),
                    F::len(&format!("{p}.past"), "Extension Past the Line", 2.0)
                        .bound(dim_num!(extension_past)),
                    F::len(&format!("{p}.arrow_size"), "Arrow Size", 6.0)
                        .bound(dim_num!(arrow_size)),
                ],
            )
            .section(
                "Text",
                vec![F::flag(&format!("{p}.text_above"), "Text Above the Line", true)
                    .bound(dim_flag!(text_above_line))],
            ),
        "temporary" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Temporary Dimensions",
                vec![
                    F::flag(&format!("{p}.show"), "Show While Drawing and Moving", true),
                    F::flag(&format!("{p}.show_selected"), "Show for Selected Objects", true),
                    F::len(&format!("{p}.offset"), "Distance From the Object", 12.0),
                    F::int(&format!("{p}.hide_after"), "Hide After", 0, (0, 60), "s"),
                ],
            ),
        _ => return None,
    })
}
