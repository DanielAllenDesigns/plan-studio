//! Default Settings > Dimension, next to the Dimensions list of saved
//! default sets (Primary Format, Setup Automatic, Extensions, Arrow, Text
//! Style, Locate Objects live there): the General, Setup Automatic, Setup
//! Temporary, Secondary Format, Extensions (centerlines), Layer and Locate
//! panels of Chief's Dimension Defaults dialog and the Auto Story Pole
//! Dimension Defaults dialog, with Chief's panel and field names in Chief's
//! order (manual pages 478 to 496, in our own words).
//!
//! Every field edits the active dimension default set through the typed
//! slots of `DimensionDefaults` and its `DimSetup`, `ToolLocates` and
//! `PoleSetup` (`plan_core::dimension::settings`), which the dimension tools
//! read. The Locate panels of the dimension tools are one page each.

use super::page::{Bind, Field as F, PageSpec};
use plan_core::defaults::{PageValue, PlanDefaults};
use plan_core::dimension::{
    LocateTool, MarkKind, ObjectLocate, OffsetFrom, OpeningLocate, PoleMark, RoundMethod,
    TempWalls, TextPos, TolMode, WallLocate,
};
use plan_core::units::LengthUnit;

/// The dimension pages in tree order: `(slug, title)`.
pub const LEAVES: &[(&str, &str)] = &[
    ("general", "General"),
    ("setup_automatic", "Setup Automatic"),
    ("setup_temporary", "Setup Temporary"),
    ("secondary", "Secondary Format"),
    ("centerlines", "Extensions: Centerlines"),
    ("layer", "Layer"),
    ("locate_manual", "Locate Manual"),
    ("locate_end_to_end", "Locate End to End"),
    ("locate_centerline", "Locate Centerline"),
    ("locate_interior", "Locate Interior"),
    ("locate_auto_exterior", "Locate Auto Exterior"),
    ("locate_auto_room", "Locate Auto Room"),
    ("locate_auto_elevation", "Locate Auto Elevation"),
    ("locate_elevations", "Locate Elevations"),
    ("auto_story_pole", "Auto Story Pole Dimensions"),
    ("pole_elevations", "Story Pole: Locate Elevations"),
];

/// Copies the edited dimension defaults into the active saved set, which
/// mirrors them (and refreshes the set's text format from the setup).
fn sync(d: &mut PlanDefaults) {
    let name = d.active_dimension_set.clone();
    let auto = d.dimensions.clone();
    if let Some(set) = d.dimension_sets.iter_mut().find(|s| s.name == name) {
        set.format.label = auto.setup.label_options(auto.text_above_line);
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

macro_rules! setup_flag {
    ($($f:ident).+) => {
        Bind {
            get: |d| PageValue::Bool(d.dimensions.setup.$($f).+),
            set: |d, v| {
                d.dimensions.setup.$($f).+ = v.flag();
                sync(d);
            },
        }
    };
}

macro_rules! setup_num {
    ($($f:ident).+) => {
        Bind {
            get: |d| PageValue::Num(d.dimensions.setup.$($f).+),
            set: |d, v| {
                d.dimensions.setup.$($f).+ = v.num();
                sync(d);
            },
        }
    };
}

macro_rules! setup_text {
    ($($f:ident).+) => {
        Bind {
            get: |d| PageValue::Text(d.dimensions.setup.$($f).+.clone()),
            set: |d, v| {
                d.dimensions.setup.$($f).+ = v.text();
                sync(d);
            },
        }
    };
}

macro_rules! setup_int {
    ($($f:ident).+, $t:ty) => {
        Bind {
            get: |d| PageValue::Int(i64::from(d.dimensions.setup.$($f).+)),
            set: |d, v| {
                d.dimensions.setup.$($f).+ = v.int().max(0) as $t;
                sync(d);
            },
        }
    };
}

/// The names of `items` by `label`.
#[cfg(test)]
fn labels<T: Copy>(items: &[T], label: fn(T) -> &'static str) -> Vec<&'static str> {
    items.iter().map(|i| label(*i)).collect()
}

fn by_label<T: Copy>(items: &[T], label: fn(T) -> &'static str, name: &str) -> Option<T> {
    items.iter().copied().find(|i| label(*i) == name)
}

const UNIT_NAMES: [(LengthUnit, &str); 6] = [
    (LengthUnit::FeetInches, "Feet and Inches"),
    (LengthUnit::Inches, "Inches"),
    (LengthUnit::DecimalFeet, "Decimal Feet"),
    (LengthUnit::Millimeters, "Millimeters"),
    (LengthUnit::Centimeters, "Centimeters"),
    (LengthUnit::Meters, "Meters"),
];

fn unit_name(u: LengthUnit) -> &'static str {
    UNIT_NAMES
        .iter()
        .find(|(x, _)| *x == u)
        .map_or("Feet and Inches", |(_, n)| n)
}

// ----- the Locate panel of a tool -----

/// A Locate panel's text for the walls choice.
const WALL_CHOICES: [&str; 4] = ["None", "Surfaces", "Wall Dimension Layer", "Wall Center"];
const OPENING_CHOICES: [&str; 3] = ["None", "Sides", "Centers"];

fn wall_choice(t: &plan_core::dimension::ToolLocate) -> &'static str {
    if t.walls_none {
        "None"
    } else {
        match t.group.walls {
            WallLocate::Surfaces => "Surfaces",
            WallLocate::MainLayer => "Wall Dimension Layer",
            WallLocate::Centers => "Wall Center",
        }
    }
}

fn opening_choice(o: OpeningLocate) -> &'static str {
    match o {
        OpeningLocate::None => "None",
        OpeningLocate::Sides => "Sides",
        OpeningLocate::Centers => "Centers",
    }
}

/// One flag of a Locate panel bound to a field of its `ToolLocate`.
macro_rules! loc_flag {
    ($tool:expr, $($f:ident).+) => {
        Bind {
            get: |d| PageValue::Bool(d.dimensions.tool_locate($tool).$($f).+),
            set: |d, v| {
                let mut t = d.dimensions.tool_locate($tool);
                t.$($f).+ = v.flag();
                d.dimensions.set_tool_locate($tool, t);
                sync(d);
            },
        }
    };
}

/// One object mark of a Locate panel (`<category>.<mark>`).
macro_rules! loc_mark {
    ($tool:expr, $key:literal) => {
        Bind {
            get: |d| PageValue::Bool(d.dimensions.tool_locate($tool).mark($key)),
            set: |d, v| {
                let mut t = d.dimensions.tool_locate($tool);
                t.set_mark($key, v.flag());
                d.dimensions.set_tool_locate($tool, t);
                sync(d);
            },
        }
    };
}

/// One entry of the Outer or Inner string list of Auto Room and Auto
/// Elevation.
macro_rules! loc_list {
    ($tool:expr, $list:ident, $kind:expr) => {
        Bind {
            get: |d| PageValue::Bool(d.dimensions.tool_locate($tool).$list.contains(&$kind)),
            set: |d, v| {
                let mut t = d.dimensions.tool_locate($tool);
                t.$list.retain(|k| *k != $kind);
                if v.flag() {
                    t.$list.push($kind);
                    t.$list.sort();
                }
                d.dimensions.set_tool_locate($tool, t);
                sync(d);
            },
        }
    };
}

/// The page of one tool's Locate panel (manual pp. 484 to 489).
macro_rules! locate_page {
    ($spec:expr, $p:expr, $tool:expr) => {{
        let k = |name: &str| format!("{}.{}", $p, name);
        let walls =
            F::pick(&k("walls"), "Walls", &WALL_CHOICES, "Wall Dimension Layer").bound(Bind {
                get: |d| PageValue::Text(wall_choice(&d.dimensions.tool_locate($tool)).into()),
                set: |d, v| {
                    let mut t = d.dimensions.tool_locate($tool);
                    match v.text().as_str() {
                        "None" => t.walls_none = true,
                        "Surfaces" => {
                            t.walls_none = false;
                            t.group.walls = WallLocate::Surfaces;
                        }
                        "Wall Center" => {
                            t.walls_none = false;
                            t.group.walls = WallLocate::Centers;
                        }
                        _ => {
                            t.walls_none = false;
                            t.group.walls = WallLocate::MainLayer;
                        }
                    }
                    d.dimensions.set_tool_locate($tool, t);
                    sync(d);
                },
            });
        let openings =
            F::pick(&k("openings"), "Locate Openings", &OPENING_CHOICES, "Sides").bound(Bind {
                get: |d| {
                    PageValue::Text(
                        opening_choice(d.dimensions.tool_locate($tool).group.openings).into(),
                    )
                },
                set: |d, v| {
                    let mut t = d.dimensions.tool_locate($tool);
                    t.group.openings = match v.text().as_str() {
                        "None" => OpeningLocate::None,
                        "Centers" => OpeningLocate::Centers,
                        _ => OpeningLocate::Sides,
                    };
                    d.dimensions.set_tool_locate($tool, t);
                    sync(d);
                },
            });
        let cabinets = F::flag(&k("cabinets"), "Locate Cabinets", true).bound(Bind {
            get: |d| {
                PageValue::Bool(
                    d.dimensions.tool_locate($tool).group.cabinets == ObjectLocate::Sides,
                )
            },
            set: |d, v| {
                let mut t = d.dimensions.tool_locate($tool);
                t.group.cabinets = if v.flag() {
                    ObjectLocate::Sides
                } else {
                    ObjectLocate::None
                };
                d.dimensions.set_tool_locate($tool, t);
                sync(d);
            },
        });
        let fixtures =
            F::flag(&k("fixtures"), "Locate Fixtures and Appliances", true).bound(Bind {
                get: |d| {
                    PageValue::Bool(
                        d.dimensions.tool_locate($tool).group.fixtures == ObjectLocate::Sides,
                    )
                },
                set: |d, v| {
                    let mut t = d.dimensions.tool_locate($tool);
                    t.group.fixtures = if v.flag() {
                        ObjectLocate::Sides
                    } else {
                        ObjectLocate::None
                    };
                    d.dimensions.set_tool_locate($tool, t);
                    sync(d);
                },
            });
        $spec
            .section("Walls", vec![walls])
            .section(
                "Wall Options: Exterior Walls",
                vec![
                    F::flag(&k("exterior_primary"), "Primary Side", true)
                        .bound(loc_flag!($tool, exterior_primary)),
                    F::flag(&k("exterior_secondary"), "Secondary Side", false)
                        .bound(loc_flag!($tool, exterior_secondary)),
                ],
            )
            .section(
                "Wall Options: Interior Walls",
                vec![
                    F::flag(&k("interior_primary"), "Primary Side", true)
                        .bound(loc_flag!($tool, interior_primary)),
                    F::flag(&k("interior_secondary"), "Secondary Side", false)
                        .bound(loc_flag!($tool, interior_secondary)),
                    F::flag(&k("interior_centers"), "Centers", false)
                        .bound(loc_flag!($tool, interior_centers)),
                ],
            )
            .section(
                "Wall Features",
                vec![
                    F::flag(&k("wall_steps"), "Wall Steps", false)
                        .bound(loc_flag!($tool, wall_steps)),
                    F::flag(&k("brick_ledge"), "Brick Ledge Lines", false)
                        .bound(loc_flag!($tool, brick_ledge_lines)),
                    F::flag(&k("wall_widths"), "Display Wall Widths", true)
                        .bound(loc_flag!($tool, display_wall_widths)),
                ],
            )
            .section(
                "Openings",
                vec![
                    openings,
                    F::flag(&k("openings_casing"), "Casing", false)
                        .bound(loc_mark!($tool, "openings.casing")),
                    F::flag(&k("openings_rough"), "Rough Opening", false)
                        .bound(loc_mark!($tool, "openings.rough")),
                ],
            )
            .section(
                "Cabinets",
                vec![
                    cabinets,
                    F::flag(&k("cabinets_sides"), "Sides", true)
                        .bound(loc_mark!($tool, "cabinets.sides")),
                    F::flag(&k("cabinets_corners"), "Corners", false)
                        .bound(loc_mark!($tool, "cabinets.corners")),
                    F::flag(&k("cabinets_centers"), "Centers", false)
                        .bound(loc_mark!($tool, "cabinets.centers")),
                    F::flag(&k("cabinets_moldings"), "Moldings", false)
                        .bound(loc_mark!($tool, "cabinets.moldings")),
                    F::flag(&k("cabinets_countertop"), "Countertop", false)
                        .bound(loc_mark!($tool, "cabinets.countertop")),
                    F::flag(&k("cabinets_backsplash"), "Backsplash", false)
                        .bound(loc_mark!($tool, "cabinets.backsplash")),
                    F::flag(&k("cabinets_toe_kick"), "Toe Kick", false)
                        .bound(loc_mark!($tool, "cabinets.toe_kick")),
                    F::flag(&k("cabinets_openings"), "Openings", false)
                        .bound(loc_mark!($tool, "cabinets.openings")),
                    F::flag(&k("cabinets_doors"), "Doors/Drawers/Panels", false)
                        .bound(loc_mark!($tool, "cabinets.doors")),
                ],
            )
            .section(
                "Fixtures, Appliances and Furniture",
                vec![
                    fixtures,
                    F::flag(&k("fixtures_sides"), "Fixtures: Sides/Corners", true)
                        .bound(loc_mark!($tool, "fixtures.sides")),
                    F::flag(&k("fixtures_centers"), "Fixtures: Centers", false)
                        .bound(loc_mark!($tool, "fixtures.centers")),
                    F::flag(&k("furniture_sides"), "Furniture: Sides/Corners", true)
                        .bound(loc_mark!($tool, "furniture.sides")),
                    F::flag(&k("furniture_centers"), "Furniture: Centers", false)
                        .bound(loc_mark!($tool, "furniture.centers")),
                ],
            )
            .section(
                "CAD Objects",
                vec![
                    F::flag(&k("cad_lines"), "Line/Sides", true)
                        .bound(loc_mark!($tool, "cad.lines")),
                    F::flag(&k("cad_ends"), "Ends/Corners", true)
                        .bound(loc_mark!($tool, "cad.ends")),
                    F::flag(&k("cad_callouts"), "Callouts/Markers", false)
                        .bound(loc_mark!($tool, "cad.callouts")),
                    F::flag(&k("cad_clip"), "Clip Lines", false)
                        .bound(loc_mark!($tool, "cad.clip_lines")),
                    F::flag(&k("cad_text"), "Text", false).bound(loc_mark!($tool, "cad.text")),
                    F::flag(&k("cad_construction"), "Construction Lines", false)
                        .bound(loc_mark!($tool, "cad.construction")),
                ],
            )
            .section(
                "3D Solids",
                vec![
                    F::flag(&k("solids_sides"), "Sides", true)
                        .bound(loc_mark!($tool, "solids.sides")),
                    F::flag(&k("solids_corners"), "Corners", false)
                        .bound(loc_mark!($tool, "solids.corners")),
                    F::flag(&k("solids_centers"), "Centers", false)
                        .bound(loc_mark!($tool, "solids.centers")),
                ],
            )
            .section(
                "Framing",
                vec![
                    F::flag(&k("framing_single"), "Single Side", false)
                        .bound(loc_mark!($tool, "framing.single")),
                    F::flag(&k("framing_both"), "Both Sides", true)
                        .bound(loc_mark!($tool, "framing.both")),
                    F::flag(&k("framing_centers"), "Centers", false)
                        .bound(loc_mark!($tool, "framing.centers")),
                ],
            )
            .section(
                "Electrical",
                vec![
                    F::flag(&k("electrical_lights"), "Lights", true)
                        .bound(loc_mark!($tool, "electrical.lights")),
                    F::flag(&k("electrical_outlets"), "Outlets", true)
                        .bound(loc_mark!($tool, "electrical.outlets")),
                    F::flag(&k("electrical_switches"), "Switches", true)
                        .bound(loc_mark!($tool, "electrical.switches")),
                    F::flag(&k("electrical_other"), "Other", false)
                        .bound(loc_mark!($tool, "electrical.other")),
                ],
            )
            .section(
                "Other Objects",
                vec![
                    F::flag(&k("other_blocks"), "Architectural Blocks", false)
                        .bound(loc_mark!($tool, "other.blocks")),
                    F::flag(&k("other_plants"), "Plants and Images", false)
                        .bound(loc_mark!($tool, "other.plants")),
                    F::flag(&k("other_newel_centers"), "Newel Centers", false)
                        .bound(loc_mark!($tool, "other.newel_centers")),
                    F::flag(&k("other_newel_sides"), "Newel Sides", false)
                        .bound(loc_mark!($tool, "other.newel_sides")),
                ],
            )
    }};
}

/// The Outer and Inner string lists of Auto Room and Auto Elevation.
macro_rules! string_lists {
    ($spec:expr, $p:expr, $tool:expr) => {{
        let k = |name: &str| format!("{}.{}", $p, name);
        $spec
            .section(
                "Outer String",
                vec![
                    F::flag(&k("outer_grade"), "Grade", false).bound(loc_list!(
                        $tool,
                        outer,
                        MarkKind::Grade
                    )),
                    F::flag(&k("outer_subfloor"), "Top of Subfloor", false).bound(loc_list!(
                        $tool,
                        outer,
                        MarkKind::TopOfSubfloor
                    )),
                    F::flag(&k("outer_plate"), "Top of Plate", false).bound(loc_list!(
                        $tool,
                        outer,
                        MarkKind::TopOfPlate
                    )),
                    F::flag(&k("outer_ceiling"), "Ceiling", false).bound(loc_list!(
                        $tool,
                        outer,
                        MarkKind::Ceiling
                    )),
                    F::flag(&k("outer_eave"), "Eave", false).bound(loc_list!(
                        $tool,
                        outer,
                        MarkKind::Eave
                    )),
                    F::flag(&k("outer_ridge"), "Ridge", false).bound(loc_list!(
                        $tool,
                        outer,
                        MarkKind::Ridge
                    )),
                    F::flag(&k("outer_sill"), "Sill", false).bound(loc_list!(
                        $tool,
                        outer,
                        MarkKind::OpeningSill
                    )),
                    F::flag(&k("outer_head"), "Head", false).bound(loc_list!(
                        $tool,
                        outer,
                        MarkKind::OpeningHead
                    )),
                ],
            )
            .section(
                "Inner String",
                vec![
                    F::flag(&k("inner_grade"), "Grade", false).bound(loc_list!(
                        $tool,
                        inner,
                        MarkKind::Grade
                    )),
                    F::flag(&k("inner_subfloor"), "Top of Subfloor", false).bound(loc_list!(
                        $tool,
                        inner,
                        MarkKind::TopOfSubfloor
                    )),
                    F::flag(&k("inner_plate"), "Top of Plate", false).bound(loc_list!(
                        $tool,
                        inner,
                        MarkKind::TopOfPlate
                    )),
                    F::flag(&k("inner_ceiling"), "Ceiling", false).bound(loc_list!(
                        $tool,
                        inner,
                        MarkKind::Ceiling
                    )),
                    F::flag(&k("inner_eave"), "Eave", false).bound(loc_list!(
                        $tool,
                        inner,
                        MarkKind::Eave
                    )),
                    F::flag(&k("inner_ridge"), "Ridge", false).bound(loc_list!(
                        $tool,
                        inner,
                        MarkKind::Ridge
                    )),
                    F::flag(&k("inner_sill"), "Sill", false).bound(loc_list!(
                        $tool,
                        inner,
                        MarkKind::OpeningSill
                    )),
                    F::flag(&k("inner_head"), "Head", false).bound(loc_list!(
                        $tool,
                        inner,
                        MarkKind::OpeningHead
                    )),
                ],
            )
    }};
}

// ----- the Auto Story Pole Locate Elevations list -----

/// A mark of the pole's Locate Elevations panel: included, on the outer
/// string, and renamed.
macro_rules! pole_mark {
    ($p:expr, $kind:expr, $name:literal) => {{
        let k = |s: &str| format!("{}.{}_{}", $p, $name, s);
        // Inclusion is the mark's own flag: the name and the outer-string
        // switch never add or remove the mark (DECISIONS QA-32).
        let on_by_default = PoleMark::default_marks()
            .iter()
            .any(|m| m.kind == $kind && m.included);
        [
            F::flag(&k("included"), concat!($name, ": Locate"), on_by_default).bound(Bind {
                get: |d| PageValue::Bool(d.dimensions.setup.pole.locates($kind)),
                set: |d, v| {
                    d.dimensions.setup.pole.entry_mut($kind).included = v.flag();
                    sync(d);
                },
            }),
            F::flag(&k("outer"), concat!($name, ": On the Outer String"), false).bound(Bind {
                get: |d| {
                    PageValue::Bool(
                        d.dimensions
                            .setup
                            .pole
                            .marks
                            .iter()
                            .any(|m| m.kind == $kind && m.outer),
                    )
                },
                set: |d, v| {
                    d.dimensions.setup.pole.entry_mut($kind).outer = v.flag();
                    sync(d);
                },
            }),
            F::text(&k("name"), concat!($name, ": Name"), "").bound(Bind {
                get: |d| {
                    PageValue::Text(
                        d.dimensions
                            .setup
                            .pole
                            .marks
                            .iter()
                            .find(|m| m.kind == $kind)
                            .map(|m| m.name.clone())
                            .unwrap_or_default(),
                    )
                },
                set: |d, v| {
                    d.dimensions.setup.pole.entry_mut($kind).name = v.text().trim().to_string();
                    sync(d);
                },
            }),
        ]
    }};
}

/// The leader-arrow style names of the General panel.
const LEADER_ARROWS: [&str; 5] = ["Match Dimension", "Arrow", "Tick", "Slash", "Dot"];

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
            .note("These edit the active dimension default set.")
            .section(
                "General",
                vec![
                    F::len(&k("baseline_separation"), "Baseline Line Separation", 12.0)
                        .bound(dim_num!(baseline_separation)),
                    F::len(&k("reach"), "Reach", 24.0).bound(dim_num!(reach)),
                    F::len(&k("layout_reach"), "Reach (Layouts)", 1.0)
                        .bound(setup_num!(layout_reach)),
                    F::len(&k("snap_line_separation"), "Snap Line Separation (Layouts)", 1.0)
                        .bound(setup_num!(snap_line_separation)),
                ],
            )
            .section(
                "Rounded Value Indicators",
                vec![
                    F::flag(&k("plus_minus_after"), "+ or - After Number", false)
                        .bound(setup_flag!(label.plus_minus_after)),
                    F::flag(&k("tilde_before"), "~ Before Number", false)
                        .bound(setup_flag!(label.tilde_before)),
                ],
            )
            .section(
                "Rounding Method",
                vec![F::pick(
                    &k("rounding"),
                    "Rounding Method",
                    &["Grid Rounding", "Distance Rounding"],
                    "Grid Rounding",
                )
                .bound(Bind {
                    get: |d| PageValue::Text(d.dimensions.setup.label.rounding.label().into()),
                    set: |d, v| {
                        d.dimensions.setup.label.rounding =
                            by_label(&RoundMethod::ALL, RoundMethod::label, &v.text())
                                .unwrap_or_default();
                        sync(d);
                    },
                })],
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
                    )
                    .bound(Bind {
                        get: |d| {
                            PageValue::Text(
                                d.dimensions
                                    .setup
                                    .position(d.dimensions.text_above_line)
                                    .label()
                                    .into(),
                            )
                        },
                        set: |d, v| {
                            let pos = by_label(&TextPos::ALL, TextPos::label, &v.text())
                                .unwrap_or_default();
                            d.dimensions.setup.text_position = Some(pos);
                            d.dimensions.text_above_line = pos == TextPos::Above;
                            sync(d);
                        },
                    }),
                    F::flag(&k("angle_automatic"), "Automatic", true).bound(Bind {
                        get: |d| PageValue::Bool(d.dimensions.setup.label.angle.is_none()),
                        set: |d, v| {
                            let kept = d.dimensions.setup.label_angle_value;
                            let l = &mut d.dimensions.setup.label;
                            if v.flag() {
                                l.angle = None;
                            } else if l.angle.is_none() {
                                l.angle = Some(kept);
                            }
                            sync(d);
                        },
                    }),
                    F::deg(&k("angle"), "Angle", 0.0).bound(Bind {
                        get: |d| {
                            let s = &d.dimensions.setup;
                            PageValue::Num(s.label.angle.unwrap_or(s.label_angle_value))
                        },
                        set: |d, v| {
                            d.dimensions.setup.label_angle_value = v.num();
                            let l = &mut d.dimensions.setup.label;
                            if l.angle.is_some() {
                                l.angle = Some(v.num());
                            }
                            sync(d);
                        },
                    }),
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
                    F::flag(&k("second_segment"), "Include Second Segment", false)
                        .bound(setup_flag!(leader_second_segment)),
                    F::len(&k("second_segment_length"), "Second Segment Length", 12.0)
                        .bound(setup_num!(leader_second_length)),
                ],
            )
            .section(
                "Include Arrow",
                vec![
                    F::flag(&k("leader_arrow"), "Include Arrow", false)
                        .bound(setup_flag!(leader_arrow)),
                    F::pick(
                        &k("leader_arrow_style"),
                        "Style",
                        &LEADER_ARROWS,
                        "Match Dimension",
                    )
                    .bound(setup_text!(leader_arrow_style)),
                    F::flag(&k("leader_arrow_match_size"), "Match Dimension", true)
                        .bound(setup_flag!(leader_arrow_match_size)),
                    F::len(&k("leader_arrow_size"), "Size", 6.0)
                        .bound(setup_num!(leader_arrow_size)),
                ],
            )
            .section(
                "3D Display",
                vec![
                    F::flag(&k("extend_extensions"), "Extend Extensions to Mark", true)
                        .bound(setup_flag!(extend_extensions_3d)),
                    F::flag(&k("label_faces_camera"), "Label Faces Camera", false)
                        .bound(setup_flag!(label_faces_camera)),
                ],
            ),
        // Dimension Defaults > Setup Automatic panel.
        "setup_automatic" => spec
            .note("Exterior, room and elevation options of the automatic dimension tools.")
            .section(
                "Exterior",
                vec![
                    F::len(&k("exterior_offset"), "First Line Offset", 24.0)
                        .bound(dim_num!(auto_exterior_offset)),
                    F::pick(
                        &k("offset_from"),
                        "Offset From",
                        &["Center", "Dimension Layer", "Surface"],
                        "Dimension Layer",
                    )
                    .bound(Bind {
                        get: |d| PageValue::Text(d.dimensions.setup.offset_from.label().into()),
                        set: |d, v| {
                            d.dimensions.setup.offset_from =
                                by_label(&OffsetFrom::ALL, OffsetFrom::label, &v.text())
                                    .unwrap_or_default();
                            sync(d);
                        },
                    }),
                    F::len(&k("line_separation"), "Line Separation", 18.0)
                        .bound(dim_num!(auto_line_separation)),
                    F::len(&k("exterior_reach"), "Reach", 48.0)
                        .bound(setup_num!(exterior_reach)),
                    F::num(&k("exterior_min_area"), "Minimum Area", 0.0, "sq ft")
                        .bound(setup_num!(exterior_min_area)),
                    F::len(&k("exterior_height_3d"), "3D Height Above Floor", 36.0)
                        .bound(setup_num!(exterior_height_3d)),
                    F::flag(&k("exterior_vertical_labels"), "Vertical Labels", false)
                        .bound(setup_flag!(exterior_vertical_labels)),
                    F::flag(&k("exterior_overall"), "Overall Dimension", true)
                        .bound(setup_flag!(exterior_overall)),
                    F::flag(&k("exterior_inner"), "Inner Dimensions", true)
                        .bound(setup_flag!(exterior_inner)),
                    F::flag(&k("exterior_auto_refresh"), "Auto Refresh", false)
                        .bound(setup_flag!(exterior_auto_refresh)),
                ],
            )
            .section(
                "Room",
                vec![
                    F::num(&k("room_min_area"), "Minimum Area", 0.0, "sq ft")
                        .bound(setup_num!(room_min_area)),
                    F::len(&k("room_height_3d"), "3D Height Above Floor", 36.0)
                        .bound(setup_num!(room_height_3d)),
                    F::flag(&k("room_vertical_labels"), "Vertical Labels", false)
                        .bound(setup_flag!(room_vertical_labels)),
                    F::flag(&k("room_overall"), "Overall Dimension", true)
                        .bound(setup_flag!(room_overall)),
                    F::flag(&k("room_outer"), "Outer Dimension", false)
                        .bound(setup_flag!(room_outer)),
                    F::flag(&k("room_auto_refresh"), "Auto Refresh", false)
                        .bound(setup_flag!(room_auto_refresh)),
                    F::flag(&k("room_allow_duplicates"), "Allow Duplicates", false)
                        .bound(setup_flag!(room_allow_duplicates)),
                    F::flag(&k("room_inside"), "Line Position: Inside the Room", true)
                        .bound(setup_flag!(room_inside)),
                ],
            )
            .section(
                "Elevation",
                vec![
                    F::flag(&k("elevation_overall"), "Overall Dimension", true)
                        .bound(setup_flag!(elevation_overall)),
                    F::flag(&k("elevation_outer"), "Outer Dimension", false)
                        .bound(setup_flag!(elevation_outer)),
                    F::flag(&k("elevation_auto_refresh"), "Auto Refresh", false)
                        .bound(setup_flag!(elevation_auto_refresh)),
                    F::flag(&k("elevation_left"), "Dimension on Left", true)
                        .bound(setup_flag!(elevation_left)),
                    F::flag(&k("elevation_right"), "Dimension on Right", false)
                        .bound(setup_flag!(elevation_right)),
                    F::flag(&k("elevation_top"), "Dimension Across Top", false)
                        .bound(setup_flag!(elevation_top)),
                    F::flag(&k("elevation_bottom"), "Dimension Across Bottom", false)
                        .bound(setup_flag!(elevation_bottom)),
                ],
            ),
        // Dimension Defaults > Setup Temporary panel.
        "setup_temporary" => spec
            .section(
                "Options",
                vec![
                    F::int(&k("row_limit"), "Dimension Row Limit", 2, (1, 20), "")
                        .bound(setup_int!(temp_row_limit, u32)),
                    F::len(&k("reach"), "Reach", 48.0).bound(setup_num!(temp_reach)),
                ],
            )
            .section(
                "Walls",
                vec![F::pick(
                    &k("walls"),
                    "Locate Walls At",
                    &["Surfaces", "Wall Dimension Layer"],
                    "Surfaces",
                )
                .bound(Bind {
                    get: |d| PageValue::Text(d.dimensions.setup.temp_walls.label().into()),
                    set: |d, v| {
                        d.dimensions.setup.temp_walls =
                            by_label(&TempWalls::ALL, TempWalls::label, &v.text())
                                .unwrap_or_default();
                        sync(d);
                    },
                })],
            )
            .section(
                "Wall Options: Exterior Walls",
                vec![
                    F::flag(&k("exterior_primary"), "Primary Side", true)
                        .bound(setup_flag!(temp_exterior_primary)),
                    F::flag(&k("exterior_secondary"), "Secondary Side", true)
                        .bound(setup_flag!(temp_exterior_secondary)),
                ],
            )
            .section(
                "Wall Options: Interior Walls",
                vec![
                    F::flag(&k("interior_primary"), "Primary Side", true)
                        .bound(setup_flag!(temp_interior_primary)),
                    F::flag(&k("interior_secondary"), "Secondary Side", true)
                        .bound(setup_flag!(temp_interior_secondary)),
                    F::flag(&k("interior_centers"), "Centers", false)
                        .bound(setup_flag!(temp_interior_centers)),
                ],
            )
            .section(
                "Locate Objects Inside",
                vec![
                    F::flag(&k("inside_cad"), "CAD Objects", true)
                        .bound(setup_flag!(temp_inside_cad)),
                    F::flag(&k("inside_terrain"), "Terrain Objects", true)
                        .bound(setup_flag!(temp_inside_terrain)),
                ],
            ),
        // Dimension Defaults > Secondary Format panel and tolerance.
        "secondary" => spec
            .note("A second number for every distance, in another unit or accuracy, and a tolerance.")
            .section(
                "Secondary Format",
                vec![
                    F::flag(&k("include"), "Include Second Format", false)
                        .bound(setup_flag!(label.second.include)),
                    F::pick(
                        &k("units"),
                        "Units",
                        &UNIT_NAMES.map(|(_, n)| n),
                        "Millimeters",
                    )
                    .bound(Bind {
                        get: |d| {
                            PageValue::Text(
                                unit_name(d.dimensions.setup.label.second.format.unit).into(),
                            )
                        },
                        set: |d, v| {
                            if let Some((u, _)) =
                                UNIT_NAMES.iter().find(|(_, n)| *n == v.text())
                            {
                                d.dimensions.setup.label.second.format.unit = *u;
                            }
                            sync(d);
                        },
                    }),
                    F::int(&k("fraction"), "Smallest Fraction (1/n)", 16, (1, 64), "")
                        .bound(Bind {
                            get: |d| {
                                PageValue::Int(i64::from(
                                    d.dimensions.setup.label.second.format.fraction_denominator,
                                ))
                            },
                            set: |d, v| {
                                d.dimensions.setup.label.second.format.fraction_denominator =
                                    v.int().clamp(1, 64) as u32;
                                sync(d);
                            },
                        }),
                    F::int(&k("decimals"), "Decimal Places", 0, (0, 6), "")
                        .bound(Bind {
                            get: |d| {
                                PageValue::Int(i64::from(
                                    d.dimensions.setup.label.second.format.decimals,
                                ))
                            },
                            set: |d, v| {
                                d.dimensions.setup.label.second.format.decimals =
                                    v.int().clamp(0, 6) as u32;
                                sync(d);
                            },
                        }),
                    F::flag(&k("indicators"), "Show Unit Indicators", true)
                        .bound(setup_flag!(label.second.format.unit_indicators)),
                    F::flag(&k("zeroes"), "Show Trailing Zeroes", false)
                        .bound(setup_flag!(label.second.format.trailing_zeroes)),
                ],
            )
            .section(
                "Tolerance",
                vec![
                    F::pick(
                        &k("tolerance"),
                        "Style",
                        &["None", "Plus or Minus", "Separate Plus and Minus", "Limits"],
                        "None",
                    )
                    .bound(Bind {
                        get: |d| {
                            PageValue::Text(d.dimensions.setup.label.tolerance.mode.label().into())
                        },
                        set: |d, v| {
                            d.dimensions.setup.label.tolerance.mode =
                                by_label(&TolMode::ALL, TolMode::label, &v.text())
                                    .unwrap_or_default();
                            sync(d);
                        },
                    }),
                    F::len(&k("plus"), "Plus", 0.25).bound(setup_num!(label.tolerance.plus)),
                    F::len(&k("minus"), "Minus", 0.25).bound(setup_num!(label.tolerance.minus)),
                ],
            ),
        // Dimension Defaults > Extensions panel, centerline options.
        "centerlines" => spec
            .note("The gap, length and fixed proximity of extension lines are on the Dimensions list's Extensions tab.")
            .section(
                "Centerlines",
                vec![
                    F::flag(&k("auto_mark"), "Auto Mark Centerlines", true)
                        .bound(setup_flag!(auto_mark_centerlines)),
                    F::flag(&k("same_angle"), "Same Angle as Dimension", true)
                        .bound(setup_flag!(centerline_same_angle)),
                    F::len(&k("offset"), "Offset From Extension", 3.0)
                        .bound(setup_num!(centerline_offset)),
                ],
            ),
        // Dimension Defaults > Layer panel.
        "layer" => spec
            .note("Empty is Chief's layer: Dimensions, Manual and Dimensions, Automatic.")
            .section(
                "Layers",
                vec![
                    F::text(&k("manual"), "Manual Dimensions", "")
                        .bound(setup_text!(layer_manual)),
                    F::text(&k("automatic"), "Automatic Dimensions", "")
                        .bound(setup_text!(layer_automatic)),
                ],
            ),
        // The Locate panel of each dimension tool.
        "locate_manual" => locate_page!(spec, p, LocateTool::Manual),
        "locate_end_to_end" => locate_page!(spec, p, LocateTool::EndToEnd),
        "locate_centerline" => locate_page!(spec, p, LocateTool::Centerline),
        "locate_interior" => locate_page!(spec, p, LocateTool::Interior),
        "locate_auto_exterior" => locate_page!(spec, p, LocateTool::AutoExterior),
        "locate_auto_room" => {
            string_lists!(locate_page!(spec, p, LocateTool::AutoRoom), p, LocateTool::AutoRoom)
        }
        "locate_auto_elevation" => string_lists!(
            locate_page!(spec, p, LocateTool::AutoElevation),
            p,
            LocateTool::AutoElevation
        ),
        "locate_elevations" => locate_page!(spec, p, LocateTool::Elevations),
        // Auto Story Pole Dimension Defaults dialog.
        "auto_story_pole" => spec
            .note("General panel of the Auto Story Pole Dimension Defaults; the marks are on the Locate Elevations page.")
            .section(
                "Position",
                vec![
                    F::flag(&k("left"), "Dimension on Left", true)
                        .bound(setup_flag!(pole.left)),
                    F::int(&k("left_reach"), "Left Reach", 100, (1, 100), "%")
                        .bound(setup_int!(pole.left_reach, u32)),
                    F::flag(&k("right"), "Dimension on Right", false)
                        .bound(setup_flag!(pole.right)),
                    F::int(&k("right_reach"), "Right Reach", 100, (1, 100), "%")
                        .bound(setup_int!(pole.right_reach, u32)),
                    F::len(&k("line_separation"), "Line Separation", 12.0)
                        .bound(setup_num!(pole.line_separation)),
                    F::len(&k("first_line_offset"), "First Line Offset", 24.0)
                        .bound(setup_num!(pole.first_line_offset)),
                ],
            )
            .section(
                "Options",
                vec![
                    F::flag(&k("inner"), "Inner Dimension", true).bound(setup_flag!(pole.inner)),
                    F::flag(&k("between_markers"), "Dimensions Between Elevation Markers", true)
                        .bound(setup_flag!(pole.between_markers)),
                    F::flag(&k("primary_ridges"), "Primary Ridge Marks Only", true)
                        .bound(setup_flag!(pole.primary_ridges_only)),
                    F::flag(&k("primary_heights"), "Primary Height Marks Only", true)
                        .bound(setup_flag!(pole.primary_heights_only)),
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
        "pole_elevations" => {
            let mut s = spec.note("Which elevations a story pole locates, which sit on the outer string, and the names it shows.");
            for fields in [
                pole_mark!(p, MarkKind::Grade, "Grade"),
                pole_mark!(p, MarkKind::TopOfSubfloor, "Top of Subfloor"),
                pole_mark!(p, MarkKind::TopOfPlate, "Top of Plate"),
                pole_mark!(p, MarkKind::Ceiling, "Ceiling"),
                pole_mark!(p, MarkKind::Eave, "Eave"),
                pole_mark!(p, MarkKind::Ridge, "Ridge"),
                pole_mark!(p, MarkKind::OpeningSill, "Sill"),
                pole_mark!(p, MarkKind::OpeningHead, "Head"),
            ] {
                s = s.section("", fields.to_vec());
            }
            s
        }
        _ => return None,
    })
}

/// The names of the choices, for the tests.
#[cfg(test)]
pub fn choice_names() -> (Vec<&'static str>, Vec<&'static str>) {
    (
        labels(&TextPos::ALL, TextPos::label),
        labels(&OffsetFrom::ALL, OffsetFrom::label),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> PlanDefaults {
        crate::plan_defaults::embedded()
    }

    fn set_field(d: &mut PlanDefaults, slug: &str, name: &str, v: PageValue) {
        let spec = page(slug).unwrap();
        let key = format!("dimension.{slug}.{name}");
        let f = spec.field(&key).unwrap_or_else(|| panic!("no field {key}"));
        f.write(d, &v);
    }

    fn get_field(d: &PlanDefaults, slug: &str, name: &str) -> PageValue {
        let spec = page(slug).unwrap();
        let key = format!("dimension.{slug}.{name}");
        spec.field(&key)
            .unwrap_or_else(|| panic!("no field {key}"))
            .read(d)
    }

    #[test]
    fn every_dimension_page_builds_and_reads_without_stored_fields() {
        let d = defaults();
        for (slug, _) in LEAVES {
            let spec = page(slug).unwrap_or_else(|| panic!("page {slug}"));
            assert!(
                !spec.has_stored() || *slug == "auto_story_pole",
                "{slug} has stored fields"
            );
            for f in spec.fields() {
                let _ = f.read(&d);
            }
        }
        assert!(page("nope").is_none());
    }

    #[test]
    fn general_fields_write_the_setup_the_tools_read() {
        let mut d = defaults();
        set_field(
            &mut d,
            "general",
            "text_position",
            PageValue::Text("Below Dimension Line".into()),
        );
        assert_eq!(d.dimensions.setup.position(true), TextPos::Below);
        assert!(!d.dimensions.text_above_line);
        assert_eq!(d.dim_format().label.position, TextPos::Below);
        set_field(&mut d, "general", "tilde_before", PageValue::Bool(true));
        assert!(d.dim_format().label.tilde_before);
        set_field(&mut d, "general", "angle_automatic", PageValue::Bool(false));
        set_field(&mut d, "general", "angle", PageValue::Num(30.0));
        assert_eq!(d.dimensions.setup.label.angle, Some(30.0));
        set_field(&mut d, "general", "angle_automatic", PageValue::Bool(true));
        assert_eq!(d.dimensions.setup.label.angle, None);
        set_field(
            &mut d,
            "general",
            "rounding",
            PageValue::Text("Distance Rounding".into()),
        );
        assert_eq!(d.dim_format().label.rounding, RoundMethod::Distance);
        // The active saved set mirrors it.
        let active = d.active_dimension().unwrap();
        assert_eq!(active.auto.setup.label.rounding, RoundMethod::Distance);
    }

    #[test]
    fn secondary_format_and_tolerance_reach_the_label_options() {
        let mut d = defaults();
        set_field(&mut d, "secondary", "include", PageValue::Bool(true));
        set_field(
            &mut d,
            "secondary",
            "units",
            PageValue::Text("Meters".into()),
        );
        set_field(&mut d, "secondary", "decimals", PageValue::Int(3));
        set_field(
            &mut d,
            "secondary",
            "tolerance",
            PageValue::Text("Plus or Minus".into()),
        );
        let o = d.dim_format().label;
        assert!(o.second.include);
        assert_eq!(o.second.format.unit, LengthUnit::Meters);
        assert_eq!(o.second.format.decimals, 3);
        assert_eq!(o.tolerance.mode, TolMode::Symmetric);
    }

    #[test]
    fn setup_automatic_and_temporary_write_the_dimension_setup() {
        let mut d = defaults();
        set_field(
            &mut d,
            "setup_automatic",
            "offset_from",
            PageValue::Text("Center".into()),
        );
        set_field(
            &mut d,
            "setup_automatic",
            "exterior_overall",
            PageValue::Bool(false),
        );
        set_field(
            &mut d,
            "setup_automatic",
            "room_min_area",
            PageValue::Num(25.0),
        );
        assert_eq!(d.dimensions.setup.offset_from, OffsetFrom::Center);
        assert!(!d.dimensions.setup.exterior_overall);
        assert_eq!(d.dimensions.setup.room_min_area, 25.0);
        set_field(&mut d, "setup_temporary", "row_limit", PageValue::Int(5));
        set_field(
            &mut d,
            "setup_temporary",
            "walls",
            PageValue::Text("Wall Dimension Layer".into()),
        );
        assert_eq!(d.dimensions.setup.temp_row_limit, 5);
        assert_eq!(d.dimensions.setup.temp_walls, TempWalls::DimensionLayer);
    }

    #[test]
    fn a_locate_page_edits_only_its_tools_panel() {
        let mut d = defaults();
        set_field(
            &mut d,
            "locate_end_to_end",
            "walls",
            PageValue::Text("Wall Center".into()),
        );
        set_field(
            &mut d,
            "locate_end_to_end",
            "cabinets_centers",
            PageValue::Bool(true),
        );
        let t = d.dimensions.tool_locate(LocateTool::EndToEnd);
        assert_eq!(t.group.walls, WallLocate::Centers);
        assert!(t.cabinet_centers());
        // Manual is untouched.
        let m = d.dimensions.tool_locate(LocateTool::Manual);
        assert_ne!(m.group.walls, WallLocate::Centers);
        assert!(!m.cabinet_centers());
        // The page reads back what was stored.
        assert_eq!(
            get_field(&d, "locate_end_to_end", "walls").text(),
            "Wall Center"
        );
        // The Manual page writes the typed fields the older code reads.
        set_field(
            &mut d,
            "locate_manual",
            "openings",
            PageValue::Text("Centers".into()),
        );
        assert_eq!(d.dimensions.opening_locate(), OpeningLocate::Centers);
        set_field(
            &mut d,
            "locate_manual",
            "walls",
            PageValue::Text("None".into()),
        );
        assert!(d.dimensions.tool_locate(LocateTool::Manual).walls_none);
        // The room and elevation panels have outer and inner string lists.
        set_field(
            &mut d,
            "locate_auto_room",
            "outer_plate",
            PageValue::Bool(true),
        );
        assert_eq!(
            d.dimensions.tool_locate(LocateTool::AutoRoom).outer,
            vec![MarkKind::TopOfPlate]
        );
        set_field(
            &mut d,
            "locate_auto_room",
            "outer_plate",
            PageValue::Bool(false),
        );
        assert!(d
            .dimensions
            .tool_locate(LocateTool::AutoRoom)
            .outer
            .is_empty());
    }

    #[test]
    fn the_story_pole_pages_edit_the_pole_setup() {
        let mut d = defaults();
        set_field(&mut d, "auto_story_pole", "right", PageValue::Bool(true));
        set_field(&mut d, "auto_story_pole", "right_reach", PageValue::Int(60));
        set_field(
            &mut d,
            "auto_story_pole",
            "line_separation",
            PageValue::Num(20.0),
        );
        let p = &d.dimensions.setup.pole;
        assert!(p.right);
        assert_eq!(p.right_reach, 60);
        assert_eq!(p.line_separation, 20.0);
        // Locate Elevations: add the sill, put the eave on the outer string,
        // rename the ridge.
        set_field(
            &mut d,
            "pole_elevations",
            "Sill_included",
            PageValue::Bool(true),
        );
        set_field(
            &mut d,
            "pole_elevations",
            "Eave_outer",
            PageValue::Bool(true),
        );
        set_field(
            &mut d,
            "pole_elevations",
            "Ridge_name",
            PageValue::Text("Ridge Line".into()),
        );
        let marks = &d.dimensions.setup.pole.marks;
        assert!(d.dimensions.setup.pole.locates(MarkKind::OpeningSill));
        assert!(marks.iter().any(|m| m.kind == MarkKind::Eave && m.outer));
        assert_eq!(
            marks
                .iter()
                .find(|m| m.kind == MarkKind::Ridge)
                .unwrap()
                .display(),
            "Ridge Line"
        );
        set_field(
            &mut d,
            "pole_elevations",
            "Sill_included",
            PageValue::Bool(false),
        );
        assert!(!d.dimensions.setup.pole.locates(MarkKind::OpeningSill));
        // Naming or moving a mark that is not located does not locate it.
        set_field(
            &mut d,
            "pole_elevations",
            "Sill_name",
            PageValue::Text("Sill Line".into()),
        );
        set_field(
            &mut d,
            "pole_elevations",
            "Sill_outer",
            PageValue::Bool(true),
        );
        assert!(!d.dimensions.setup.pole.locates(MarkKind::OpeningSill));
        let (positions, offsets) = choice_names();
        assert_eq!(positions.len(), 3);
        assert_eq!(offsets.len(), 3);
    }
}
