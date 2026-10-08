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
mod face;
mod filler;
mod geom;
mod mesh3d;
mod mesh_extra;
mod symbol;
mod top;

#[cfg(test)]
mod extras_tests;

pub use cabinet::{
    auto_label, expand_label, run_along_wall, type_code, ApplianceBay, Backsplash, BlindSide,
    BlindSpec, Cabinet, CabinetKind, CabinetPreset, CornerSpec, CornerStyle, Countertop,
    DoorProfile, DoorStyle, DrawerStyle, FaceSide, HandleStyle, HingeStyle, MaterialChoice,
    Molding, MoldingKind, Overlay, PartMaterials, SideFace, SideKind, ToeKick, FULL_HEIGHT_TO,
};
pub use face::{Divider, DividerHandle, FaceCell, FaceItem, FaceLayout, ResolvedFace, MIN_ITEM};
pub use filler::{fit_between, fit_to_gap, wall_polygon, MAX_FILLER_GAP};
pub use geom::{
    area as ring_area, bbox as ring_bbox, ccw as ring_ccw, free_span, offset_ring, thicken_path,
    triangulate, union_polygons, InsideObstacle,
};
pub use mesh3d::meshes;
pub use symbol::{plan_symbol, Stroke};
pub use top::{
    fit_full_height_backsplashes, generate_countertops, join_touching_countertops,
    release_joined_top, treat_corners, CornerTreatment, CustomTop, Cutout, CutoutKind, EdgeProfile,
    GeneratedTop, JoinedSource,
};
