//! Default Settings pages of the architectural objects that have no
//! specification dialog of their own: 3D Solid, Corner Trim, Distributed
//! Objects, Image, Dormer, Foundation, Slab, Stairs, Railing and Deck, the
//! Garage Door and the special walls (Railing, Fence, Pony, Half, Glass,
//! Attic), Materials List and Molding Polylines.
//!
//! Where the model already holds a value (the code minimums of the stairs,
//! the footing, the wall variants) the field edits that slot; the rest is
//! stored with the defaults (`PlanDefaults::pages`). Chief's own values for
//! the stored ones are not captured: every stored default below is marked
//! "verify in Chief" in `DECISIONS.md`.

use super::page::{Field as F, ListSource, PageSpec};
use crate::{bind_num, bind_text};

const LAYER_NOTE: &str =
    "Saved with the plan defaults (File > Templates > Save Current Defaults as My Template).";

pub fn solid_3d() -> PageSpec {
    PageSpec::new("solid3d", "3D Solid")
        .note(LAYER_NOTE)
        .section(
            "Size",
            vec![
                F::len("solid3d.width", "Width", 24.0),
                F::len("solid3d.depth", "Depth", 24.0),
                F::len("solid3d.height", "Height", 24.0),
                F::len("solid3d.elevation", "Elevation", 0.0),
            ],
        )
        .section(
            "Appearance",
            vec![
                F::text("solid3d.material", "Material", "Default"),
                F::flag("solid3d.cast_shadows", "Cast Shadows", true),
                F::text("solid3d.layer", "Layer", "3D Solids"),
            ],
        )
}

pub fn corner_trim() -> PageSpec {
    PageSpec::new("corner_trim", "Corner Trim")
        .note(LAYER_NOTE)
        .section(
            "Corner Board",
            vec![
                F::len("corner_trim.board_width", "Board Width", 3.5),
                F::len("corner_trim.board_thickness", "Board Thickness", 0.75),
                F::text("corner_trim.board_material", "Material", "Trim - White"),
            ],
        )
        .section(
            "Quoin",
            vec![
                F::len("corner_trim.quoin_width", "Quoin Width", 12.0),
                F::len("corner_trim.quoin_height", "Quoin Height", 8.0),
                F::len("corner_trim.quoin_projection", "Projection", 0.75),
                F::flag(
                    "corner_trim.quoin_alternate",
                    "Alternate Long and Short",
                    true,
                ),
                F::text("corner_trim.quoin_material", "Material", "Stone"),
            ],
        )
}

pub fn distributed() -> PageSpec {
    PageSpec::new("distributed", "Distributed Objects")
        .note(LAYER_NOTE)
        .section(
            "Distribution",
            vec![
                F::len("distributed.spacing", "Spacing", 24.0),
                F::pick(
                    "distributed.spacing_mode",
                    "Spacing Is",
                    &["Center to Center", "Edge to Edge"],
                    "Center to Center",
                ),
                F::len("distributed.start_offset", "Start Offset", 0.0),
                F::len("distributed.end_offset", "End Offset", 0.0),
                F::flag("distributed.align_to_path", "Align Objects to Path", true),
                F::flag("distributed.fit_ends", "Space Evenly Between Ends", false),
            ],
        )
        .section(
            "Object",
            vec![
                F::text("distributed.symbol", "Symbol", ""),
                F::text("distributed.layer", "Layer", "Distributed Objects"),
            ],
        )
}

pub fn image() -> PageSpec {
    PageSpec::new("image", "Image")
        .note(LAYER_NOTE)
        .section(
            "Size",
            vec![
                F::len("image.width", "Width", 36.0),
                F::len("image.height", "Height", 24.0),
                F::flag("image.lock_aspect", "Lock Aspect Ratio", true),
                F::len("image.elevation", "Elevation", 0.0),
            ],
        )
        .section(
            "Display",
            vec![
                F::int("image.transparency", "Transparency", 0, (0, 100), "%"),
                F::flag(
                    "image.billboard",
                    "Billboard (Always Face the Camera)",
                    false,
                ),
                F::flag("image.show_in_3d", "Show in 3D Views", true),
                F::text("image.layer", "Layer", "Images"),
            ],
        )
}

pub fn dormer() -> PageSpec {
    PageSpec::new("dormer", "Dormer")
        .note(LAYER_NOTE)
        .section(
            "Dormer",
            vec![
                F::pick(
                    "dormer.kind",
                    "Roof Type",
                    &["Gable", "Shed", "Hip"],
                    "Gable",
                ),
                F::len("dormer.width", "Width", 72.0),
                F::len("dormer.wall_height", "Wall Height", 48.0),
                F::len("dormer.setback", "Setback From Eave", 24.0),
            ],
        )
        .section(
            "Roof",
            vec![
                F::num("dormer.pitch", "Pitch", 8.0, "in 12"),
                F::len("dormer.overhang", "Overhang", 12.0),
                F::flag("dormer.fascia", "Include Fascia", true),
                F::flag("dormer.window", "Place a Window in the Front Wall", true),
            ],
        )
}

pub fn foundation() -> PageSpec {
    PageSpec::new("foundation", "Foundation")
        .section(
            "Footing",
            vec![
                F::len("foundation.footing_width", "Footing Width", 16.0)
                    .bound(bind_num!(code.footing_width)),
                F::len("foundation.footing_thickness", "Footing Thickness", 8.0)
                    .bound(bind_num!(code.footing_thickness)),
                F::len("foundation.footing_offset", "Footing Offset", 0.0),
            ],
        )
        .section(
            "Foundation Wall",
            vec![
                F::len("foundation.wall_height", "Wall Height", 96.0)
                    .bound(bind_num!(foundation_wall.height)),
                F::len("foundation.stem_height", "Stem Wall Above Grade", 12.0),
                F::flag("foundation.sill_plate", "Sill Plate", true),
                F::len("foundation.anchor_spacing", "Anchor Bolt Spacing", 72.0),
            ],
        )
        .section(
            "Slab",
            vec![
                F::len("foundation.slab_thickness", "Slab Thickness", 4.0),
                F::flag("foundation.monolithic", "Monolithic Slab", false),
                F::len("foundation.chamfer", "Chamfer", 4.0),
            ],
        )
}

pub fn slab() -> PageSpec {
    PageSpec::new("slab", "Slab")
        .note(LAYER_NOTE)
        .section(
            "Slab",
            vec![
                F::len("slab.thickness", "Thickness", 4.0),
                F::len("slab.footing_width", "Footing Width", 16.0),
                F::len("slab.footing_height", "Footing Height", 12.0),
                F::flag("slab.monolithic", "Monolithic Pour", false),
            ],
        )
        .section(
            "Under the Slab",
            vec![
                F::len("slab.fill_thickness", "Gravel Fill Thickness", 4.0),
                F::flag("slab.vapor_barrier", "Vapor Barrier", true),
                F::flag("slab.edge_insulation", "Edge Insulation", false),
            ],
        )
        .section(
            "Finish",
            vec![
                F::text("slab.material", "Material", "Concrete"),
                F::text("slab.layer", "Layer", "Slabs"),
            ],
        )
}

pub fn stairs() -> PageSpec {
    PageSpec::new("stairs", "Stairs")
        .section(
            "Stair Sizes",
            vec![
                F::len("stairs.riser", "Riser Height", 7.0).bound(bind_num!(code.stair_riser)),
                F::len("stairs.tread", "Tread Depth", 11.0).bound(bind_num!(code.stair_tread)),
                F::len("stairs.width", "Stair Width", 36.0).bound(bind_num!(code.stair_width)),
                F::len("stairs.headroom", "Headroom", 80.0).bound(bind_num!(code.stair_headroom)),
            ],
        )
        .section(
            "Railing",
            vec![
                F::len("stairs.guard", "Guard Height", 36.0).bound(bind_num!(code.guard_height)),
                F::len("stairs.handrail", "Handrail Height", 34.0)
                    .bound(bind_num!(code.handrail_height)),
                F::len("stairs.baluster_gap", "Largest Baluster Opening", 4.0)
                    .bound(bind_num!(code.baluster_opening)),
            ],
        )
        .section(
            "Construction",
            vec![
                F::len("stairs.nosing", "Nosing", 1.0),
                F::len("stairs.tread_thickness", "Tread Thickness", 1.0),
                F::len("stairs.riser_thickness", "Riser Thickness", 0.75),
                F::int("stairs.stringers", "Stringers", 3, (2, 8), ""),
                F::len("stairs.landing_depth", "Landing Depth", 36.0),
            ],
        )
}

pub fn railing_deck() -> PageSpec {
    PageSpec::new("railing_deck", "Railing and Deck")
        .section(
            "Deck",
            vec![
                F::list(
                    "railing_deck.edge_type",
                    "Deck Edge Wall Type",
                    ListSource::WallTypes,
                    "Deck Edge-2",
                )
                .bound(bind_text!(wall_variants.deck_edge_type)),
                F::len("railing_deck.edge_height", "Deck Edge Height", 9.25)
                    .bound(bind_num!(wall_variants.deck_edge_height)),
                F::list(
                    "railing_deck.railing_type",
                    "Deck Railing Wall Type",
                    ListSource::WallTypes,
                    "Deck Railing-4",
                )
                .bound(bind_text!(wall_variants.deck_railing_type)),
                F::len("railing_deck.board_width", "Deck Board Width", 5.5),
                F::len("railing_deck.board_gap", "Gap Between Boards", 0.25),
                F::len("railing_deck.joist_spacing", "Joist Spacing", 16.0),
            ],
        )
        .section(
            "Railing",
            vec![
                F::list(
                    "railing_deck.rail_type",
                    "Railing Wall Type",
                    ListSource::WallTypes,
                    "Railing-4",
                )
                .bound(bind_text!(wall_variants.railing_type)),
                F::len("railing_deck.rail_height", "Railing Height", 36.0)
                    .bound(bind_num!(wall_variants.railing_height)),
                F::len("railing_deck.post_spacing", "Post Spacing", 72.0),
                F::len("railing_deck.baluster_spacing", "Baluster Spacing", 4.0),
            ],
        )
}

pub fn garage_door() -> PageSpec {
    PageSpec::new("garage_door", "Garage Door")
        .note(LAYER_NOTE)
        .section(
            "Size",
            vec![
                F::len("garage_door.width", "Width", 108.0),
                F::len("garage_door.height", "Height", 84.0),
                F::len("garage_door.sill", "Sill Height", 0.0),
            ],
        )
        .section(
            "Style",
            vec![
                F::pick(
                    "garage_door.style",
                    "Door Style",
                    &["Sectional", "Carriage House", "Roll-Up", "Tilt-Up"],
                    "Sectional",
                ),
                F::int("garage_door.panels", "Panels", 4, (1, 8), ""),
                F::flag("garage_door.windows", "Window Row", false),
                F::flag("garage_door.casing", "Casing", true),
                F::flag("garage_door.opener", "Opener Symbol", false),
            ],
        )
}

/// A wall-variant page: `slot` is the variant (`railing`, `fence`, ...).
pub fn special_wall(slot: &str) -> Option<PageSpec> {
    let types = ListSource::WallTypes;
    Some(match slot {
        "railing" => PageSpec::new("walls.railing", "Railing Wall").section(
            "Railing Wall",
            vec![
                F::list("walls.railing.type", "Wall Type", types, "Railing-4")
                    .bound(bind_text!(wall_variants.railing_type)),
                F::len("walls.railing.height", "Height", 36.0)
                    .bound(bind_num!(wall_variants.railing_height)),
            ],
        ),
        "fence" => PageSpec::new("walls.fence", "Fence").section(
            "Fence",
            vec![
                F::list("walls.fence.type", "Wall Type", types, "Fence-Wood-2")
                    .bound(bind_text!(wall_variants.fencing_type)),
                F::len("walls.fence.height", "Height", 72.0)
                    .bound(bind_num!(wall_variants.fencing_height)),
            ],
        ),
        "pony" => PageSpec::new("walls.pony", "Pony Wall").section(
            "Pony Wall",
            vec![
                F::list("walls.pony.upper", "Upper Wall Type", types, "Stucco-6")
                    .bound(bind_text!(wall_variants.pony_upper_type)),
                F::list("walls.pony.lower", "Lower Wall Type", types, "Foundation-8")
                    .bound(bind_text!(wall_variants.pony_lower_type)),
                F::len("walls.pony.split", "Elevation of Lower Wall Top", 36.0)
                    .bound(bind_num!(wall_variants.pony_split_height)),
            ],
        ),
        "half" => PageSpec::new("walls.half", "Half Wall").section(
            "Half Wall",
            vec![F::len("walls.half.height", "Top Height", 36.0)
                .bound(bind_num!(wall_variants.half_wall_height))],
        ),
        "glass" => PageSpec::new("walls.glass", "Glass Wall").section(
            "Glass Wall",
            vec![F::list("walls.glass.type", "Wall Type", types, "Glass-1")
                .bound(bind_text!(wall_variants.glass_type))],
        ),
        "attic" => PageSpec::new("walls.attic", "Attic Wall").section(
            "Attic Wall",
            vec![F::list("walls.attic.type", "Wall Type", types, "")
                .bound(bind_text!(roof_detail.attic_wall_type))],
        ),
        _ => return None,
    })
}

pub fn materials_list() -> PageSpec {
    PageSpec::new("materials_list", "Materials List")
        .note(LAYER_NOTE)
        .section(
            "Quantities",
            vec![
                F::int("materials_list.waste", "Waste Factor", 10, (0, 100), "%"),
                F::flag(
                    "materials_list.round_up",
                    "Round Counts Up to Whole Units",
                    true,
                ),
                F::flag(
                    "materials_list.group_by_category",
                    "Group by Category",
                    true,
                ),
            ],
        )
        .section(
            "Pricing",
            vec![
                F::flag("materials_list.show_prices", "Show Prices", true),
                F::flag("materials_list.show_labor", "Include Labor", true),
                F::int("materials_list.markup", "Markup", 0, (0, 500), "%"),
            ],
        )
}

pub fn molding_polylines() -> PageSpec {
    PageSpec::new("molding_polylines", "Molding Polylines")
        .note(LAYER_NOTE)
        .section(
            "Profile",
            vec![
                F::text("molding_polylines.profile", "Profile", "Crown 4 1/4"),
                F::len("molding_polylines.width", "Width", 4.25),
                F::len("molding_polylines.height", "Height", 4.25),
                F::len("molding_polylines.offset", "Elevation", 0.0),
            ],
        )
        .section(
            "Options",
            vec![
                F::flag("molding_polylines.miter", "Miter Corners", true),
                F::flag(
                    "molding_polylines.count_in_list",
                    "Count in Materials List",
                    true,
                ),
                F::text("molding_polylines.material", "Material", "Trim - White"),
                F::text("molding_polylines.layer", "Layer", "Moldings"),
            ],
        )
}
