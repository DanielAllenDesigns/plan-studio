//! Default Settings > CAD: General CAD and one page per CAD object (Arcs,
//! Boxes, Circles, Lines, Polylines, Splines, Points, Markers, Callouts,
//! Leaders, Insert Point). The values are stored with the defaults
//! (`PlanDefaults::pages`, keys `cad.<object>.<field>`); the CAD tools start
//! new objects from them where the tool reads the page (see
//! `docs/integration-queue.md`, "Default Settings pages").

use super::page::{Field as F, ListSource, PageSpec};

/// The CAD pages in tree order: `(slug, title)`.
pub const LEAVES: &[(&str, &str)] = &[
    ("general", "General CAD"),
    ("arcs", "Arcs"),
    ("boxes", "Boxes"),
    ("circles", "Circles"),
    ("lines", "Lines"),
    ("polylines", "Polylines"),
    ("splines", "Splines"),
    ("points", "Points"),
    ("markers", "Markers"),
    ("callouts", "Callouts"),
    ("leaders", "Leaders"),
    ("insert_point", "Insert Point"),
];

const LINE_STYLES: &[&str] = &["Solid", "Dashed", "Dotted", "Dash-Dot", "Center", "Hidden"];
const ARROWS: &[&str] = &["None", "Start", "End", "Both"];
const ARROW_STYLES: &[&str] = &["Open", "Closed", "Filled", "Tick", "Dot"];
const MARKERS: &[&str] = &["Cross", "X", "Circle", "Circle Cross", "Square", "Diamond"];

/// Line style, weight and colour: the look every CAD object shares.
fn look(p: &str) -> Vec<F> {
    vec![
        F::pick(&format!("{p}.line_style"), "Line Style", LINE_STYLES, "Solid"),
        F::num(&format!("{p}.line_weight"), "Line Weight", 0.5, "pt"),
        F::color(&format!("{p}.color"), "Color", "#000000"),
    ]
}

fn fill(p: &str) -> Vec<F> {
    vec![
        F::pick(
            &format!("{p}.fill"),
            "Fill Style",
            &["None", "Solid", "Hatch", "Cross Hatch"],
            "None",
        ),
        F::color(&format!("{p}.fill_color"), "Fill Color", "#CCCCCC"),
    ]
}

fn layer(p: &str, default: &str) -> Vec<F> {
    vec![F::text(&format!("{p}.layer"), "Layer", default)]
}

/// The page of CAD object `slug`.
pub fn page(slug: &str) -> Option<PageSpec> {
    let (_, title) = LEAVES.iter().find(|(s, _)| *s == slug)?;
    let id = format!("cad.{slug}");
    let p = id.as_str();
    let spec = PageSpec::new(p, title)
        .note("Saved with the plan defaults; new CAD objects of this kind start from them.");
    Some(match slug {
        "general" => spec
            .section("Look", look(p))
            .section(
                "Text",
                vec![F::list(
                    &format!("{p}.text_style"),
                    "Text Style",
                    ListSource::TextStyles,
                    "Default Text Style",
                )],
            )
            .section("Layer", layer(p, "CAD, Default"))
            .section(
                "Display",
                vec![
                    F::flag(&format!("{p}.show_arc_centers"), "Show Arc Centers and Ends", false),
                    F::flag(&format!("{p}.line_weights"), "Show Line Weights", true),
                ],
            ),
        "arcs" => spec
            .section("Look", look(p))
            .section(
                "Arc",
                vec![
                    F::pick(&format!("{p}.arrows"), "Arrows", ARROWS, "None"),
                    F::pick(&format!("{p}.arrow_style"), "Arrow Style", ARROW_STYLES, "Open"),
                    F::flag(&format!("{p}.show_center"), "Show Center Marker", false),
                ],
            )
            .section("Layer", layer(p, "CAD, Default")),
        "boxes" => spec
            .section("Look", look(p))
            .section("Fill", fill(p))
            .section(
                "Box",
                vec![
                    F::len(&format!("{p}.corner_radius"), "Corner Radius", 0.0),
                    F::flag(&format!("{p}.from_center"), "Draw From Center", false),
                ],
            )
            .section("Layer", layer(p, "CAD, Default")),
        "circles" => spec
            .section("Look", look(p))
            .section("Fill", fill(p))
            .section(
                "Circle",
                vec![
                    F::flag(&format!("{p}.show_center"), "Show Center Marker", false),
                    F::pick(&format!("{p}.size_by"), "Size By", &["Radius", "Diameter"], "Radius"),
                ],
            )
            .section("Layer", layer(p, "CAD, Default")),
        "lines" => spec
            .section("Look", look(p))
            .section(
                "Arrows",
                vec![
                    F::pick(&format!("{p}.arrows"), "Arrows", ARROWS, "None"),
                    F::pick(&format!("{p}.arrow_style"), "Arrow Style", ARROW_STYLES, "Open"),
                    F::len(&format!("{p}.arrow_size"), "Arrow Size", 6.0),
                ],
            )
            .section("Layer", layer(p, "CAD, Default")),
        "polylines" => spec
            .section("Look", look(p))
            .section("Fill", fill(p))
            .section(
                "Polyline",
                vec![
                    F::flag(&format!("{p}.closed"), "Closed", false),
                    F::pick(
                        &format!("{p}.corners"),
                        "Corners",
                        &["Sharp", "Rounded", "Chamfered"],
                        "Sharp",
                    ),
                    F::pick(&format!("{p}.arrows"), "Arrows", ARROWS, "None"),
                ],
            )
            .section("Layer", layer(p, "CAD, Default")),
        "splines" => spec
            .section("Look", look(p))
            .section(
                "Spline",
                vec![
                    F::flag(&format!("{p}.closed"), "Closed", false),
                    F::num(&format!("{p}.tension"), "Tension", 0.5, ""),
                    F::flag(&format!("{p}.show_control_points"), "Show Control Points", true),
                ],
            )
            .section("Layer", layer(p, "CAD, Default")),
        "points" => spec
            .section(
                "Point",
                vec![
                    F::pick(&format!("{p}.style"), "Point Style", MARKERS, "Cross"),
                    F::len(&format!("{p}.size"), "Size", 2.0),
                    F::color(&format!("{p}.color"), "Color", "#000000"),
                    F::flag(&format!("{p}.label"), "Number the Points", false),
                ],
            )
            .section("Layer", layer(p, "CAD, Default")),
        "markers" => spec
            .section(
                "Marker",
                vec![
                    F::pick(&format!("{p}.style"), "Marker Style", MARKERS, "Circle Cross"),
                    F::len(&format!("{p}.size"), "Size", 4.0),
                    F::color(&format!("{p}.color"), "Color", "#D2691E"),
                    F::flag(&format!("{p}.snap"), "Objects Snap to Markers", true),
                ],
            )
            .section("Layer", layer(p, "CAD, Default")),
        "callouts" => spec
            .section(
                "Callout",
                vec![
                    F::list(
                        &format!("{p}.text_style"),
                        "Text Style",
                        ListSource::TextStyles,
                        "Default Text Style",
                    ),
                    F::pick(
                        &format!("{p}.border"),
                        "Border",
                        &["None", "Box", "Circle", "Rounded Box"],
                        "None",
                    ),
                    F::pick(&format!("{p}.arrow_style"), "Arrow Style", ARROW_STYLES, "Open"),
                    F::flag(&format!("{p}.leader"), "Leader Line", true),
                ],
            )
            .section("Look", look(p))
            .section("Layer", layer(p, "CAD, Default")),
        "leaders" => spec
            .section(
                "Leader",
                vec![
                    F::pick(&format!("{p}.arrow_style"), "Arrow Style", ARROW_STYLES, "Open"),
                    F::len(&format!("{p}.arrow_size"), "Arrow Size", 6.0),
                    F::len(&format!("{p}.landing"), "Landing Length", 6.0),
                    F::flag(&format!("{p}.text_above"), "Text Above the Landing", true),
                ],
            )
            .section("Look", look(p))
            .section("Layer", layer(p, "CAD, Default")),
        "insert_point" => spec
            .section(
                "Insert Point",
                vec![
                    F::pick(&format!("{p}.style"), "Point Style", MARKERS, "Cross"),
                    F::len(&format!("{p}.size"), "Size", 3.0),
                    F::flag(&format!("{p}.snap"), "Snap to Insert Points", true),
                    F::flag(&format!("{p}.show"), "Show Insert Points of Selected Symbols", true),
                ],
            )
            .section("Layer", layer(p, "CAD, Default")),
        _ => return None,
    })
}
