//! plan-cabinets: the parametric cabinet engine, mirroring Chief Architect's
//! cabinet model.
//!
//! A [`Cabinet`] is a box with an optional countertop, backsplash and toe
//! kick, plus a [`FaceLayout`] (Chief's "Front/Sides/Back" tab): a vertical
//! stack of separations, drawers, doors, openings and appliances, any of which
//! may be a side-by-side `HorizontalLayout`. That one description drives both
//! the 2D plan symbol ([`plan_symbol`]) and the 3D geometry ([`meshes`]).
//!
//! Units are inches. In the cabinet's **local frame** X runs along the width,
//! the back sits at `y = 0` and the front at `y = depth` (so the front faces
//! +Y), and Z measures up from the cabinet bottom. The local origin is the
//! back-left corner and is placed at [`Cabinet::position`], rotated by
//! [`Cabinet::angle`] (counter-clockwise radians). The 3D scene frame matches
//! `plan-3d`: X = plan x, Y = up, Z = -plan y.

mod cabinet;
mod dress;
mod face;
mod filler;
mod geom;
mod item;
mod label;
mod mesh3d;
mod mesh_extra;
mod options;
mod push;
mod runs;
mod special;
mod symbol;
mod top;
mod topedit;

#[cfg(test)]
mod extras_tests;

pub use cabinet::{
    auto_label, expand_label, run_along_wall, type_code, ApplianceBay, Backsplash, BlindSide,
    BlindSpec, Cabinet, CabinetKind, CabinetPreset, CornerSpec, CornerStyle, Countertop,
    DoorProfile, DoorStyle, DrawerStyle, FaceSide, HandleStyle, HingeStyle, MaterialChoice,
    Molding, MoldingKind, Overlay, PartMaterials, SideFace, SideKind, Stiles, ToeKick,
    ToeOptions, FULL_HEIGHT_TO,
};
pub use dress::{
    components, fmt_in, shelf_count, Accessories, Component, FillPattern, FootStyle, ObjectInfo,
    PanelStyle, PilasterStyle, PlanFill, MAX_SHELVES, SHELF_SPACING,
};
pub use face::{
    Divider, DividerHandle, DoorPlan, FaceCell, FaceItem, FaceLayout, ItemKind, ResolvedFace,
    AUTO_DOOR_THRESHOLD, MIN_ITEM,
};
pub use item::{
    HardwareSize, InsertOrder, ItemProps, Shelf, ShelfDepth, ShelfPlacement, ShelfSpec,
    SHELF_THICKNESS,
};
pub use filler::{
    auto_fillers, fit_between, fit_to_gap, run_bounds, run_class, run_mates, same_height,
    wall_polygon, FillerOptions, RunClass, AUTO_FILLER_REACH, MAX_FILLER_GAP,
};
pub use label::{key_of, label_is_blank};
pub use options::{
    AutoOnOff, BoxConstruction, EdgeMolding, Ends, Manufacturer, ShowOpen, TopEdge, TopSpec,
};
pub use geom::{
    area as ring_area, bbox as ring_bbox, ccw as ring_ccw, free_span, offset_ring, thicken_path,
    triangulate, union_polygons, InsideObstacle,
};
pub use mesh3d::meshes;
pub use push::push_run;
pub use runs::{
    bottom_over_appliance, cabinet_appliance_tops, link, merge_reach, merge_runs, plan_strokes,
    run_display, schedule_category, schedule_counts, width_for_space, Link, ModuleLine,
    PlanOptions, Run, RunDisplay, ScheduleCategory, MODULE_LINES_LAYER,
};
pub use special::{
    apply_blind_corners, apply_exposures, blind_corners, exposures, CabinetStyle, Special,
    SpecialShape,
};
pub use symbol::{plan_symbol, Stroke};
pub use top::{
    fit_full_height_backsplashes, generate_countertops, join_touching_countertops,
    release_joined_top, treat_corners, CornerTreatment, CustomTop, Cutout, CutoutKind, EdgeProfile,
    GeneratedTop, JoinedSource,
};
