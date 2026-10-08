//! Door panels, window units and casing that fill wall holes, by opening style.

use crate::builder::MeshSet;
use crate::casing;
use crate::doors;
use crate::frame::Frame;
use crate::leaf::Leaf;
use crate::mesh::{Material, Mesh};
use crate::wall::Hole;
use crate::windows;
use crate::SceneOptions;
use plan_core::{Opening, OpeningKind, OpeningStyle, Wall, WallKind};

/// The mulled unit an opening belongs to (DW-51, DW-52): a window beside a
/// window, or a door beside its sidelites. The members share one frame post
/// between them and one casing around the whole unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unit {
    /// A member stands against this one on the start side / the end side.
    pub left: bool,
    pub right: bool,
    /// Wall offsets `(start, end)` of the whole unit.
    pub span: (f64, f64),
    /// Lowest bottom and highest top of the members, wall-local height.
    pub h: (f64, f64),
    /// A door is one of the members.
    pub door: bool,
}

impl Unit {
    /// The unit of an opening that stands alone in `hole`.
    pub fn alone(opening: &Opening, hole: &Hole) -> Self {
        Self {
            left: false,
            right: false,
            span: (hole.s0, hole.s1),
            h: (hole.h0, hole.h1),
            door: opening.kind == OpeningKind::Door,
        }
    }

    /// The unit of `opening` among the openings of its wall (`siblings`, each
    /// with its hole). Members are those with the same mull group.
    pub fn among(opening: &Opening, hole: &Hole, siblings: &[(&Opening, Hole)]) -> Self {
        let Some(group) = opening.mull_group else {
            return Self::alone(opening, hole);
        };
        let mut unit = Self::alone(opening, hole);
        for (o, h) in siblings {
            if o.id == opening.id || o.mull_group != Some(group) {
                continue;
            }
            unit.span = (unit.span.0.min(h.s0), unit.span.1.max(h.s1));
            unit.h = (unit.h.0.min(h.h0), unit.h.1.max(h.h1));
            unit.door |= o.kind == OpeningKind::Door;
            if h.s1 <= hole.s0 + 1e-6 {
                unit.left = true;
            }
            if h.s0 >= hole.s1 - 1e-6 {
                unit.right = true;
            }
        }
        unit
    }

    /// Whether this opening owns the head, the sill and the apron of the unit
    /// casing: the first member from the wall start.
    pub fn draws_head(&self) -> bool {
        !self.left
    }
}

/// Everything an opening builder needs, in wall-local terms.
pub struct Ctx<'a> {
    pub frame: Frame,
    pub wall: &'a Wall,
    pub opening: &'a Opening,
    pub hole: Hole,
    /// Side of the wall facing a room: `1.0` left (+t), `-1.0` right.
    pub interior: f64,
    pub opts: &'a SceneOptions,
    pub unit: Unit,
}

/// An arched head laid over the hole (Arch tab), in wall-local `(s, h)`.
#[derive(Debug, Clone)]
pub struct ArchGeom {
    /// Height of the springline.
    pub spring: f64,
    /// The outer curve from the start jamb to the end jamb.
    pub outer: Vec<(f64, f64)>,
    /// The curve `fw` inside it (the inside of the frame), same point count.
    pub inner: Vec<(f64, f64)>,
}

/// Whether the arch of an `opening` of this style shapes its 3D unit.
pub fn arch_applies(kind: OpeningKind, style: OpeningStyle) -> bool {
    match kind {
        OpeningKind::Window => matches!(
            style,
            OpeningStyle::Window | OpeningStyle::Fixed | OpeningStyle::Casement
        ),
        OpeningKind::Door => matches!(
            style,
            OpeningStyle::Hinged
                | OpeningStyle::DoubleDoor
                | OpeningStyle::Fixed
                | OpeningStyle::Doorway
        ),
    }
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

    /// The arched head of the hole, with the frame band `fw` wide inside the
    /// outer curve; `None` for a square head, a style an arch does not shape,
    /// or an arch too small to draw.
    pub fn arch(&self, fw: f64) -> Option<ArchGeom> {
        let o = self.opening;
        if !arch_applies(o.kind, self.style()) {
            return None;
        }
        let h = self.hole;
        let (w, ht) = (h.s1 - h.s0, h.h1 - h.h0);
        let arch = o.extras.spec.arch;
        let rise = arch.rise(w, ht);
        if rise < 1.0 || w < 4.0 {
            return None;
        }
        let spring = h.h1 - rise;
        let profile = arch.profile(w, rise);
        let fw = fw.min(w * 0.25).min(rise * 0.5);
        let rise_in = (rise - fw).max(0.25);
        let outer: Vec<(f64, f64)> = profile.iter().map(|p| (h.s0 + p.0, spring + p.1)).collect();
        let inner = profile
            .iter()
            .map(|p| {
                (
                    h.s0 + fw + p.0 / w * (w - 2.0 * fw),
                    spring + p.1 / rise * rise_in,
                )
            })
            .collect();
        Some(ArchGeom {
            spring,
            outer,
            inner,
        })
    }

    /// The style drawn: a window's sliding door style is the sliding window,
    /// and calculated door panels become single or double by the width.
    pub fn style(&self) -> OpeningStyle {
        match (self.opening.kind, self.opening.effective_style()) {
            (OpeningKind::Window, OpeningStyle::Sliding) => OpeningStyle::SlidingWindow,
            (_, s) => s,
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

/// The wall fill that squares off the corners of the hole over an arched head
/// (the hole itself stays rectangular): vertical strips between the curve and
/// the top of the hole, in the exterior and interior wall materials.
fn spandrel(ctx: &Ctx, set: &mut MeshSet, arch: &ArchGeom) {
    let top = ctx.hole.h1;
    let half = ctx.half();
    let leaf = Leaf::new((0.0, 0.0), (1.0, 0.0));
    let exterior = -ctx.interior;
    for (side, material) in [
        (exterior, Material::WallExterior),
        (ctx.interior, Material::WallInterior),
    ] {
        let t = if side > 0.0 {
            (0.0, half)
        } else {
            (-half, 0.0)
        };
        let mesh = set.material(material);
        for pair in arch.outer.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if top - a.1 < 1e-6 && top - b.1 < 1e-6 {
                continue;
            }
            leaf.slab(&ctx.frame, mesh, &[a, b, (b.0, top), (a.0, top)], t);
        }
    }
}

/// The straight wall an opening on a curved wall is built against: `wall`
/// unrolled along the arc's tangent at the center of `hole`, with `s` still
/// the arc length, so the hole's span lands where it is on the arc. A door
/// leaf or window unit is flat; this stands it square to the local tangent
/// at the middle of the opening. A straight wall comes back unchanged.
pub fn tangent_wall(wall: &Wall, hole: &Hole) -> Wall {
    if !wall.is_curved() {
        return wall.clone();
    }
    let center = (hole.s0 + hole.s1) * 0.5;
    let (at, tangent) = wall.frame_at(center);
    let mut w = wall.clone();
    w.curve = None;
    w.start = at.sub(tangent.scale(center));
    w.end = w.start.add(tangent.scale(wall.path_length()));
    w
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
    build_opening_in_wall(wall, opening, hole, elevation, interior, opts, &[])
}

/// [`build_opening`] knowing the other openings of the wall with their holes:
/// a mulled unit shares one frame post and one casing (DW-51, DW-52).
pub fn build_opening_in_wall(
    wall: &Wall,
    opening: &Opening,
    hole: &Hole,
    elevation: f64,
    interior: f64,
    opts: &SceneOptions,
    siblings: &[(&Opening, Hole)],
) -> Vec<Mesh> {
    let ctx = Ctx {
        frame: Frame::new(wall, elevation),
        wall,
        opening,
        hole: *hole,
        interior,
        opts,
        unit: Unit::among(opening, hole, siblings),
    };
    let mut set = MeshSet::default();
    match ctx.style() {
        OpeningStyle::Hinged
        | OpeningStyle::Sliding
        | OpeningStyle::Pocket
        | OpeningStyle::Bifold
        | OpeningStyle::Garage
        | OpeningStyle::Barn
        | OpeningStyle::Shower
        | OpeningStyle::DoubleDoor
        | OpeningStyle::Doorway => doors::build(&ctx, &mut set),
        OpeningStyle::Fixed if opening.kind == OpeningKind::Door => doors::build(&ctx, &mut set),
        OpeningStyle::Fixed
        | OpeningStyle::Window
        | OpeningStyle::Casement
        | OpeningStyle::SlidingWindow
        | OpeningStyle::Awning
        | OpeningStyle::Hopper
        | OpeningStyle::BayWindow
        | OpeningStyle::BowWindow
        | OpeningStyle::BoxWindow => windows::build(&ctx, &mut set),
        OpeningStyle::PassThrough | OpeningStyle::WallNiche => {}
    }
    if let Some(arch) = ctx.arch(opening.frame_width()) {
        spandrel(&ctx, &mut set, &arch);
    }
    if opts.show_casing && opening.style != OpeningStyle::WallNiche {
        casing::add_casing(&ctx, &mut set);
        if wall.kind == WallKind::Exterior {
            casing::add_threshold(&ctx, &mut set);
        }
    }
    casing::add_lintel(&ctx, &mut set);
    casing::add_exterior_sill(&ctx, &mut set);
    casing::add_shutters(&ctx, &mut set);
    set.finish(Some(opening.id))
}
