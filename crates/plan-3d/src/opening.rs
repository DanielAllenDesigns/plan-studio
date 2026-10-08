//! Door panels, window units and casing that fill wall holes, by opening style.

use crate::builder::MeshSet;
use crate::casing;
use crate::doors;
use crate::frame::Frame;
use crate::mesh::Mesh;
use crate::wall::Hole;
use crate::windows;
use crate::SceneOptions;
use plan_core::{Opening, OpeningStyle, Wall, WallKind};

/// Everything an opening builder needs, in wall-local terms.
pub struct Ctx<'a> {
    pub frame: Frame,
    pub wall: &'a Wall,
    pub opening: &'a Opening,
    pub hole: Hole,
    /// Side of the wall facing a room: `1.0` left (+t), `-1.0` right.
    pub interior: f64,
    pub opts: &'a SceneOptions,
}

impl Ctx<'_> {
    /// Half the wall thickness.
    pub fn half(&self) -> f64 {
        self.wall.thickness * 0.5
    }

    /// Wall-local `t` sign of the side a door swings toward / a unit projects to.
    pub fn swing_sign(&self) -> f64 {
        if self.opening.swing_flipped {
            -1.0
        } else {
            1.0
        }
    }

    /// `(hinge s, direction sign along s)` for hinged leaves.
    pub fn hinge(&self) -> (f64, f64) {
        if self.opening.hinge_at_end {
            (self.hole.s1, -1.0)
        } else {
            (self.hole.s0, 1.0)
        }
    }
}

/// Build the meshes that fill `hole` for `opening`.
pub fn build_opening(
    wall: &Wall,
    opening: &Opening,
    hole: &Hole,
    elevation: f64,
    interior: f64,
    opts: &SceneOptions,
) -> Vec<Mesh> {
    let ctx = Ctx {
        frame: Frame::new(wall, elevation),
        wall,
        opening,
        hole: *hole,
        interior,
        opts,
    };
    let mut set = MeshSet::default();
    match opening.style {
        OpeningStyle::Hinged
        | OpeningStyle::Sliding
        | OpeningStyle::Pocket
        | OpeningStyle::Bifold
        | OpeningStyle::Garage
        | OpeningStyle::Barn
        | OpeningStyle::Shower
        | OpeningStyle::Doorway => doors::build(&ctx, &mut set),
        OpeningStyle::Fixed if opening.kind == plan_core::OpeningKind::Door => {
            doors::build(&ctx, &mut set)
        }
        OpeningStyle::Fixed
        | OpeningStyle::Window
        | OpeningStyle::BayWindow
        | OpeningStyle::BowWindow
        | OpeningStyle::BoxWindow => windows::build(&ctx, &mut set),
        OpeningStyle::PassThrough | OpeningStyle::WallNiche => {}
    }
    if opts.show_casing && opening.style != OpeningStyle::WallNiche {
        casing::add_casing(&ctx, &mut set);
        if wall.kind == WallKind::Exterior {
            casing::add_threshold(&ctx, &mut set);
        }
    }
    set.finish(Some(opening.id))
}
