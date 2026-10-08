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
mod mesh3d;
mod symbol;

pub use cabinet::{
    auto_label, run_along_wall, Backsplash, Cabinet, CabinetKind, Countertop, DoorStyle,
    DrawerStyle, HandleStyle, Overlay, ToeKick,
};
pub use face::{FaceCell, FaceItem, FaceLayout, ResolvedFace};
pub use mesh3d::meshes;
pub use symbol::{plan_symbol, Stroke};
