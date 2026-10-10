//! The Default Settings pages of the kinds that keep multiple saved defaults
//! as page values (`plan_core::defaults::saved`): Revision Clouds, Room
//! Functions, Structural Member Reporting and Framing Types. Arrows keep
//! their own page in `plan.rs`. The keys start with the kind's prefix, which
//! is how a saved default finds its values among `PlanDefaults::pages`.

use super::page::{Field as F, PageSpec};

const NOTE: &str = "A saved default: the Saved Defaults dialog lists and switches them.";

/// The ids of these pages, in tree order.
pub const IDS: [&str; 4] = [
    "revision_clouds",
    "saved.room_functions",
    "saved.structural_reporting",
    "saved.framing_types",
];

/// The page with id `id`, if it is one of ours.
pub fn page(id: &str) -> Option<PageSpec> {
    let p = id;
    Some(match id {
        "revision_clouds" => PageSpec::new(p, "Revision Clouds")
            .note(NOTE)
            .section(
                "Cloud",
                vec![
                    F::len("revision_clouds.arc_size", "Arc Size", 6.0),
                    F::pick(
                        "revision_clouds.arc_direction",
                        "Arcs Bulge",
                        &["Outward", "Inward"],
                        "Outward",
                    ),
                    F::flag("revision_clouds.closed", "Closed Cloud", true),
                ],
            )
            .section(
                "Line and Layer",
                vec![
                    F::pick(
                        "revision_clouds.line_style",
                        "Line Style",
                        &["Solid", "Dashed", "Dotted"],
                        "Solid",
                    ),
                    F::num("revision_clouds.line_weight", "Line Weight", 0.5, "pt"),
                    F::text("revision_clouds.layer", "Layer", "Revision Clouds"),
                ],
            ),
        "saved.room_functions" => PageSpec::new(p, "Room Functions").note(NOTE).section(
            "Function",
            vec![
                F::pick(
                    "saved.room_functions.function",
                    "Function",
                    &[
                        "Standard",
                        "Living",
                        "Utility",
                        "Deck",
                        "Garage",
                        "Porch",
                        "Open Below",
                        "Attic",
                        "Slab",
                        "Balcony",
                        "Court",
                    ],
                    "Standard",
                ),
                F::flag(
                    "saved.room_functions.living_area",
                    "Include in Living Area",
                    true,
                ),
                F::flag(
                    "saved.room_functions.conditioned",
                    "Include in Conditioned Area",
                    true,
                ),
                F::flag("saved.room_functions.flat_ceiling", "Flat Ceiling", true),
                F::flag(
                    "saved.room_functions.generates_roof",
                    "Generate a Roof Above",
                    true,
                ),
            ],
        ),
        "saved.structural_reporting" => PageSpec::new(p, "Structural Member Reporting")
            .note(NOTE)
            .section(
                "Reporting",
                vec![
                    F::pick(
                        "saved.structural_reporting.method",
                        "Reporting Method",
                        &["Buy List", "Cut List", "Linear Length", "Mixed Reporting"],
                        "Mixed Reporting",
                    ),
                    F::text(
                        "saved.structural_reporting.board_lengths",
                        "Board Lengths (Buy List)",
                        "8', 10', 12', 14', 16', 20'",
                    ),
                ],
            ),
        "saved.framing_types" => PageSpec::new(p, "Framing Types").note(NOTE).section(
            "Type",
            vec![
                F::pick(
                    "saved.framing_types.composition",
                    "Composition",
                    &[
                        "Lumber",
                        "I-Joist",
                        "Glulam",
                        "Engineered Lumber",
                        "LVL",
                        "PSL",
                        "VSL",
                        "Steel I",
                        "Steel Box",
                        "C Channel",
                        "U Channel",
                        "Concrete Rectangular",
                        "Concrete Circular",
                        "Other",
                    ],
                    "Lumber",
                ),
                F::flag(
                    "saved.framing_types.nominal_sizes",
                    "Display Nominal Sizes",
                    true,
                ),
                F::flag(
                    "saved.framing_types.name_in_labels",
                    "Include Name in Labels",
                    false,
                ),
            ],
        ),
        _ => return None,
    })
}
