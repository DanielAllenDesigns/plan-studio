//! Default Settings > CAD: the CAD Defaults page (General CAD) and the line
//! style, fill style and arrow defaults of the CAD objects that have a
//! specification dialog in Chief (Arcs, Boxes, Circles, Lines, Polylines,
//! Splines), with Chief's panel and field names (manual pages 319 to 349, in
//! our own words).
//!
//! No CAD tool reads these yet: every field is stored with the defaults
//! (`PlanDefaults::pages`, keys `cad.<object>.<field>`), marked with `*` on
//! the page, and queued in `docs/integration-queue.md`. Chief's Markers,
//! Callouts and Leaders defaults are on the Text pages.

use super::page::{Field as F, PageSpec};

/// The CAD pages in tree order: `(slug, title)`.
pub const LEAVES: &[(&str, &str)] = &[
    ("general", "General CAD"),
    ("arcs", "Arcs"),
    ("boxes", "Boxes"),
    ("circles", "Circles"),
    ("lines", "Lines"),
    ("polylines", "Polylines"),
    ("splines", "Splines"),
];

const LINE_STYLES: &[&str] = &["Solid", "Dashed", "Dotted", "Dash-Dot", "Center", "Hidden"];
const ARROWS: &[&str] = &["None", "Start", "End", "Both"];

/// The Line Style panel: layer, colour, style and weight.
fn line_style(p: &str) -> Vec<F> {
    vec![
        F::text(&format!("{p}.layer"), "Layer", "CAD, Default"),
        F::color(&format!("{p}.color"), "Line Color", "#000000"),
        F::pick(
            &format!("{p}.line_style"),
            "Line Style",
            LINE_STYLES,
            "Solid",
        ),
        F::num(&format!("{p}.line_weight"), "Line Weight", 0.5, "pt"),
    ]
}

/// The Fill Style panel.
fn fill_style(p: &str) -> Vec<F> {
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

/// The Arrow panel.
fn arrow(p: &str) -> Vec<F> {
    vec![
        F::pick(&format!("{p}.arrows"), "Arrows", ARROWS, "None"),
        F::pick(
            &format!("{p}.arrow_style"),
            "Arrow Style",
            &["Open", "Closed", "Filled", "Tick", "Dot"],
            "Open",
        ),
        F::len(&format!("{p}.arrow_size"), "Arrow Size", 6.0),
    ]
}

/// The page of CAD object `slug`.
pub fn page(slug: &str) -> Option<PageSpec> {
    let (_, title) = LEAVES.iter().find(|(s, _)| *s == slug)?;
    let id = format!("cad.{slug}");
    let p = id.as_str();
    let spec = PageSpec::new(p, title);
    Some(match slug {
        // CAD Defaults dialog.
        "general" => spec
            .section(
                "CAD Defaults",
                vec![
                    F::text(
                        &format!("{p}.current_layer"),
                        "Current CAD Layer",
                        "CAD, Default",
                    ),
                    F::pick(
                        &format!("{p}.length_format"),
                        "Displayed Line Length Format",
                        &[
                            "Feet and Inches",
                            "Feet",
                            "Inches",
                            "Meters",
                            "Centimeters",
                            "Millimeters",
                        ],
                        "Feet and Inches",
                    ),
                    F::pick(
                        &format!("{p}.length_accuracy"),
                        "Accuracy",
                        &["1", "1/2", "1/4", "1/8", "1/16", "1/32", "1/64"],
                        "1/16",
                    ),
                    F::pick(
                        &format!("{p}.angle_format"),
                        "Display Line Angles As",
                        &["Degrees", "Bearing", "Azimuth"],
                        "Degrees",
                    ),
                ],
            )
            .section(
                "Options",
                vec![F::flag(
                    &format!("{p}.show_arc_centers"),
                    "Show Arc Centers and Ends",
                    false,
                )],
            ),
        "arcs" | "lines" => spec
            .section("Line Style", line_style(p))
            .section("Arrow", arrow(p)),
        "boxes" => spec
            .section(
                "General",
                vec![F::pick(
                    &format!("{p}.box_style"),
                    "Box Style",
                    &["Normal", "Cross", "Insulation"],
                    "Normal",
                )],
            )
            .section("Line Style", line_style(p))
            .section("Fill Style", fill_style(p)),
        "circles" | "splines" => spec
            .section("Line Style", line_style(p))
            .section("Fill Style", fill_style(p)),
        "polylines" => spec
            .section("Line Style", line_style(p))
            .section("Fill Style", fill_style(p))
            .section("Arrow", arrow(p)),
        _ => return None,
    })
}
