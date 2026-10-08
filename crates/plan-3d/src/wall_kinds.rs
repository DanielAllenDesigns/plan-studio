//! The non-standard wall classes in 3D (`plan_core::WallClass`) and curved
//! walls: foundation, pony, glass, glass pony, half-wall, room divider,
//! railings, deck edge and fencing.
//!
//! A wall of one of these classes (or any curved wall) is built here instead
//! of by [`crate::wall::build_wall`] alone; the standard builder is reused
//! for every solid part.

use crate::builder::MeshSet;
use crate::clip::TopProfile;
use crate::cover::FloorCover;
use crate::mesh::{Material, Mesh};
use crate::railing;
use crate::wall::{hole_for, solid_rects, Hole, InteriorSign, WallFrame, WallLook};
use crate::SceneOptions;
use plan_core::{Floor, Opening, Wall, WallClass};

pub use crate::wall::EndCuts;

/// Wall type facts the builder needs, resolved by the scene builder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeInfo {
    pub thickness: f64,
    /// Material of the type's exterior layer, when it maps to one.
    pub exterior: Option<Material>,
}

/// Glass panel thickness, inches.
const GLASS_THICKNESS: f64 = 0.5;
/// Width of the trim frame around a glass panel.
const FRAME_WIDTH: f64 = 2.0;
/// Rim board thickness.
const RIM_THICKNESS: f64 = 1.5;
/// Fence post spacing (at most) and size.
const FENCE_POST_SPACING: f64 = 96.0;
const FENCE_POST: f64 = 3.5;
/// Picket width and pitch.
const PICKET_WIDTH: f64 = 3.5;
const PICKET_PITCH: f64 = 5.0;

/// Whether this wall is built here: any non-standard class, or a curve.
pub fn handles(wall: &Wall) -> bool {
    !wall.class.is_standard() || wall.is_curved()
}

type Lookup<'a> = &'a dyn Fn(&str) -> Option<TypeInfo>;

/// The height the roof can cut a wall of this class down to, if it can: a
/// half wall's own height, a foundation wall's reach below the floor, the
/// whole height of the solid classes. Railings, fences and deck edges are
/// not cut.
fn cuttable_height(wall: &Wall) -> Option<f64> {
    match &wall.class {
        WallClass::Standard
        | WallClass::Pony { .. }
        | WallClass::GlassPony { .. }
        | WallClass::Glass => Some(wall.height),
        WallClass::HalfWall { height } => Some(height.min(wall.height).max(0.0)),
        WallClass::Foundation => Some(wall.foundation_height),
        _ => None,
    }
}

/// The top the roof gives a wall of this class standing on `elevation`
/// (RF-16): cut by the roof planes above it like a standard wall, never
/// raised to them. `None` for a level top.
fn class_top(cover: Option<&FloorCover>, wall: &Wall, elevation: f64) -> Option<TopProfile> {
    let nominal = cuttable_height(wall)?;
    let base = match wall.class {
        WallClass::Foundation => elevation - wall.foundation_height,
        _ => elevation,
    } + wall.bottom_offset;
    cover?.wall_top_clipped(wall, base, nominal)
}

/// Builds the wall and the openings it hosts into `out`. `cover` is the roof
/// above the floor, which cuts the wall tops.
pub fn add_wall(
    floor: &Floor,
    wall: &Wall,
    interior: InteriorSign,
    opts: &SceneOptions,
    lookup: Lookup,
    cover: Option<&FloorCover>,
    out: &mut Vec<Mesh>,
) {
    if wall.class == WallClass::RoomDivider || wall.length() <= 1e-6 {
        return;
    }
    if wall.is_curved() {
        add_curved(floor, wall, interior, opts, lookup, cover, out);
        return;
    }
    // The wall as its openings see it: a half-wall is lower, a foundation
    // wall is only as tall as it reaches below the floor.
    let basis = opening_basis(wall);
    let hosted: Vec<(&Opening, Hole)> = floor
        .openings_on(wall.id)
        .filter_map(|o| hole_for(&basis, o).map(|h| (o, h)))
        .collect();
    let holes: Vec<Hole> = hosted.iter().map(|(_, h)| *h).collect();
    let top = class_top(cover, wall, floor.elevation);
    out.extend(build_class_with_top(
        wall,
        floor.elevation,
        &holes,
        interior,
        lookup,
        top.as_ref(),
    ));
    let base = match wall.class {
        WallClass::Foundation => floor.elevation - wall.foundation_height,
        _ => floor.elevation,
    };
    for (opening, hole) in &hosted {
        out.extend(crate::opening::build_opening(
            &basis, opening, hole, base, interior, opts,
        ));
    }
}

fn opening_basis(wall: &Wall) -> Wall {
    let mut w = wall.clone();
    match wall.class {
        WallClass::HalfWall { height } => w.height = height.min(wall.height).max(0.0),
        WallClass::Foundation => w.height = wall.foundation_height,
        _ => {}
    }
    w
}

/// The meshes of a straight wall of its class with `holes` cut in it.
pub fn build_class(
    wall: &Wall,
    elevation: f64,
    holes: &[Hole],
    interior: InteriorSign,
    lookup: Lookup,
) -> Vec<Mesh> {
    build_class_with_top(wall, elevation, holes, interior, lookup, None)
}

/// [`build_class`] whose top follows `top` (heights above the wall's bottom).
pub fn build_class_with_top(
    wall: &Wall,
    elevation: f64,
    holes: &[Hole],
    interior: InteriorSign,
    lookup: Lookup,
    top: Option<&TopProfile>,
) -> Vec<Mesh> {
    build_class_cut(
        wall,
        elevation,
        holes,
        interior,
        lookup,
        top,
        &EndCuts::NONE,
    )
}

/// [`build_class_with_top`] for a wall that may be curved. The solid classes
/// (standard, half, foundation and the pony wall's boxes) follow the arc, with
/// `s` in `holes` and `top` the arc length and the ends mitred by `cuts`; the
/// glass, railing, deck edge and fencing classes are built as one straight
/// piece per facet of the arc.
pub fn build_class_cut(
    wall: &Wall,
    elevation: f64,
    holes: &[Hole],
    interior: InteriorSign,
    lookup: Lookup,
    top: Option<&TopProfile>,
    cuts: &EndCuts,
) -> Vec<Mesh> {
    // A wall that starts above the floor is built as an ordinary wall of its
    // class standing on a higher base, with its holes re-measured from it.
    if wall.bottom_offset != 0.0 {
        let mut grounded = wall.clone();
        grounded.bottom_offset = 0.0;
        return build_class_cut(
            &grounded,
            elevation + wall.bottom_offset,
            &shift_holes(holes, wall.bottom_offset),
            interior,
            lookup,
            top,
            cuts,
        );
    }
    let look_of = |name: Option<&str>| WallLook {
        exterior: name
            .and_then(lookup)
            .and_then(|t| t.exterior)
            .unwrap_or(Material::WallExterior),
    };
    let thickness_of = |name: &str| lookup(name).map_or(wall.thickness, |t| t.thickness);
    match &wall.class {
        WallClass::RoomDivider => Vec::new(),
        WallClass::Standard => crate::wall::build_wall_with_top_cut(
            wall,
            elevation,
            holes,
            interior,
            look_of(wall.wall_type.as_deref()),
            top,
            cuts,
        ),
        WallClass::HalfWall { height } => {
            let mut w = wall.clone();
            w.height = height.min(wall.height).max(0.0);
            let holes = clip_holes(holes, 0.0, w.height);
            crate::wall::build_wall_with_top_cut(
                &w,
                elevation,
                &holes,
                interior,
                look_of(wall.wall_type.as_deref()),
                top,
                cuts,
            )
        }
        WallClass::Foundation => {
            let mut w = wall.clone();
            w.height = wall.foundation_height;
            let holes = clip_holes(holes, 0.0, w.height);
            let mut meshes = crate::wall::build_wall_with_top_cut(
                &w,
                elevation - wall.foundation_height,
                &holes,
                interior,
                WallLook {
                    exterior: Material::Concrete,
                },
                top,
                cuts,
            );
            for m in &mut meshes {
                if matches!(m.material, Material::WallExterior | Material::WallInterior) {
                    m.material = Material::Concrete;
                }
            }
            meshes
        }
        WallClass::Pony {
            upper_type,
            lower_type,
            split_height,
            ..
        } => {
            let split = split_height.clamp(0.0, wall.height);
            let mut meshes = pony_lower(
                wall,
                elevation,
                holes,
                interior,
                split,
                thickness_of(lower_type),
                look_of(Some(lower_type)),
                top,
                cuts,
            );
            let mut upper = wall.clone();
            upper.thickness = thickness_of(upper_type);
            upper.height = wall.height - split;
            let upper_holes = clip_holes(holes, split, wall.height);
            let upper_top = top.map(|t| t.lowered(split));
            meshes.extend(crate::wall::build_wall_with_top_cut(
                &upper,
                elevation + split,
                &shift_holes(&upper_holes, split),
                interior,
                look_of(Some(upper_type)),
                upper_top.as_ref(),
                cuts,
            ));
            meshes
        }
        WallClass::GlassPony {
            lower_type,
            split_height,
        } => {
            let split = split_height.clamp(0.0, wall.height);
            let mut meshes = pony_lower(
                wall,
                elevation,
                holes,
                interior,
                split,
                thickness_of(lower_type),
                look_of(Some(lower_type)),
                top,
                cuts,
            );
            let upper_holes = shift_holes(&clip_holes(holes, split, wall.height), split);
            meshes.extend(glass_panel(
                wall,
                elevation + split,
                wall.height - split,
                &upper_holes,
                false,
                cuts,
            ));
            meshes
        }
        WallClass::Glass => glass_panel(wall, elevation, wall.height, holes, true, cuts),
        // A railing is built a straight piece at a time along an arc.
        WallClass::Railing | WallClass::DeckRailing => {
            on_facets(wall, &[], |w, _| railing::build_railing(w, elevation))
        }
        WallClass::DeckEdge => rim_board(wall, elevation),
        WallClass::Fencing { style } => fence(wall, elevation, *style),
    }
}

/// The solid lower box of a pony wall (cut to `top` when the roof comes
/// down below the split).
#[allow(clippy::too_many_arguments)]
fn pony_lower(
    wall: &Wall,
    elevation: f64,
    holes: &[Hole],
    interior: InteriorSign,
    split: f64,
    thickness: f64,
    look: WallLook,
    top: Option<&TopProfile>,
    cuts: &EndCuts,
) -> Vec<Mesh> {
    let mut lower = wall.clone();
    lower.thickness = thickness;
    lower.height = split;
    let top = top.map(|t| t.clamped(split));
    crate::wall::build_wall_with_top_cut(
        &lower,
        elevation,
        &clip_holes(holes, 0.0, split),
        interior,
        look,
        top.as_ref(),
        cuts,
    )
}

/// The holes clipped to the vertical band `lo..hi` (those outside it drop).
fn clip_holes(holes: &[Hole], lo: f64, hi: f64) -> Vec<Hole> {
    holes
        .iter()
        .filter_map(|h| {
            let (h0, h1) = (h.h0.max(lo), h.h1.min(hi));
            (h1 - h0 > 1e-6).then_some(Hole { h0, h1, ..*h })
        })
        .collect()
}

fn shift_holes(holes: &[Hole], down: f64) -> Vec<Hole> {
    holes
        .iter()
        .map(|h| Hole {
            h0: h.h0 - down,
            h1: h.h1 - down,
            ..*h
        })
        .collect()
}

/// A glass wall: a [`Material::Glass`] panel (cut by the holes) in a
/// [`Material::Trim`] frame. `bottom_rail` is off when a solid wall sits below.
fn glass_panel(
    wall: &Wall,
    elevation: f64,
    height: f64,
    holes: &[Hole],
    bottom_rail: bool,
    cuts: &EndCuts,
) -> Vec<Mesh> {
    let frame = WallFrame::new(wall, elevation, cuts);
    let length = frame.length();
    if height <= 1e-6 || length <= 1e-6 {
        return Vec::new();
    }
    let mut set = MeshSet::default();
    let g = GLASS_THICKNESS.min(wall.thickness).max(0.1) * 0.5;
    for (s0, s1, h0, h1) in solid_rects(length, height, holes) {
        frame.cuboid(set.material(Material::Glass), (s0, s1), (-g, g), (h0, h1));
    }
    let f = (wall.thickness * 0.5).max(g);
    let t = (-f, f);
    let trim = set.material(Material::Trim);
    let fw = FRAME_WIDTH.min(length * 0.5).min(height * 0.5);
    let bottom = if bottom_rail { fw } else { 0.0 };
    frame.cuboid(trim, (0.0, length), t, (height - fw, height));
    if bottom_rail {
        frame.cuboid(trim, (0.0, length), t, (0.0, bottom));
    }
    frame.cuboid(trim, (0.0, fw), t, (bottom, height - fw));
    frame.cuboid(trim, (length - fw, length), t, (bottom, height - fw));
    set.finish(Some(wall.id))
}

/// A deck edge: a rim board hanging below the deck surface, no railing.
fn rim_board(wall: &Wall, elevation: f64) -> Vec<Mesh> {
    let frame = WallFrame::new(wall, elevation, &EndCuts::NONE);
    let mut set = MeshSet::default();
    let half = RIM_THICKNESS * 0.5;
    let drop = wall.height.max(1.0);
    frame.cuboid(
        set.material(Material::Trim),
        (0.0, frame.length()),
        (-half, half),
        (-drop, 0.0),
    );
    set.finish(Some(wall.id))
}

/// Fencing: posts at most 96" apart with pickets, boards or rails between.
fn fence(wall: &Wall, elevation: f64, style: plan_core::FenceStyle) -> Vec<Mesh> {
    use plan_core::FenceStyle;
    let frame = WallFrame::new(wall, elevation, &EndCuts::NONE);
    let length = frame.length();
    let height = wall.height.max(12.0);
    let mut set = MeshSet::default();
    let post = FENCE_POST * 0.5;
    let bays = (length / FENCE_POST_SPACING).ceil().max(1.0) as usize;
    let centers: Vec<f64> = (0..=bays)
        .map(|i| (length * i as f64 / bays as f64).clamp(post, (length - post).max(post)))
        .collect();
    let wood = set.material(Material::Trim);
    for &c in &centers {
        frame.cuboid(wood, (c - post, c + post), (-post, post), (0.0, height));
    }
    let board = (wall.thickness * 0.5).clamp(0.375, 1.0);
    let rail_t = (-board * 0.8, board * 0.8);
    match style {
        FenceStyle::Privacy => {
            frame.cuboid(wood, (0.0, length), (-board, board), (2.0, height - 4.0));
            frame.cuboid(
                wood,
                (0.0, length),
                (-post, post),
                (height - 4.0, height - 2.0),
            );
        }
        FenceStyle::Picket | FenceStyle::Rail => {
            let rails: &[f64] = if style == FenceStyle::Rail {
                &[height * 0.25, height * 0.5, height * 0.75]
            } else {
                &[8.0, height - 12.0]
            };
            for &y in rails {
                frame.cuboid(wood, (0.0, length), rail_t, (y, y + 3.5));
            }
            if style == FenceStyle::Picket {
                let w = PICKET_WIDTH * 0.5;
                let mut s = PICKET_PITCH * 0.5;
                while s + w <= length {
                    if centers.iter().all(|c| (c - s).abs() >= post + w) {
                        frame.cuboid(wood, (s - w, s + w), (-board, board), (1.0, height));
                    }
                    s += PICKET_PITCH;
                }
            }
        }
    }
    set.finish(Some(wall.id))
}

/// One straight facet of a curved wall: a straight clone of the wall along a
/// chord of its arc, and the span `s0..s1` of arc length it covers.
struct Facet {
    wall: Wall,
    s0: f64,
    s1: f64,
}

/// The facets of curved `wall` (none for a straight wall).
fn facets(wall: &Wall) -> Vec<Facet> {
    let Some(curve) = wall.curve.filter(|c| !c.is_straight()) else {
        return Vec::new();
    };
    let n = curve.facet_count(wall.start, wall.end).max(2);
    let pts = curve.sample_points(wall.start, wall.end, n);
    let total = wall.path_length();
    pts.windows(2)
        .enumerate()
        .filter_map(|(k, pair)| {
            let mut w = wall.clone();
            w.curve = None;
            w.start = pair[0];
            w.end = pair[1];
            (w.length() > 1e-6).then(|| Facet {
                wall: w,
                s0: total * k as f64 / n as f64,
                s1: total * (k + 1) as f64 / n as f64,
            })
        })
        .collect()
}

/// `holes` (in arc length) as one facet sees them (in its chord length).
fn facet_holes(holes: &[Hole], f: &Facet) -> Vec<Hole> {
    let scale = f.wall.length() / (f.s1 - f.s0).max(1e-9);
    holes
        .iter()
        .filter_map(|h| {
            let (a, b) = (h.s0.max(f.s0), h.s1.min(f.s1));
            (b - a > 1e-6).then_some(Hole {
                s0: (a - f.s0) * scale,
                s1: (b - f.s0) * scale,
                ..*h
            })
        })
        .collect()
}

/// Builds with `build` on the wall itself, or on every facet of a curved wall
/// with the holes that fall on it.
fn on_facets(
    wall: &Wall,
    holes: &[Hole],
    build: impl Fn(&Wall, &[Hole]) -> Vec<Mesh>,
) -> Vec<Mesh> {
    if !wall.is_curved() {
        return build(wall, holes);
    }
    facets(wall)
        .iter()
        .flat_map(|f| build(&f.wall, &facet_holes(holes, f)))
        .collect()
}

/// The top the roof gives a curved wall, facet by facet: each facet is cut by
/// the roof planes over it (a standard wall also follows its roof directive,
/// so a gable end rises to the roof) and the pieces are laid along the arc.
/// `None` when no facet is shaped.
fn curved_top(cover: Option<&FloorCover>, wall: &Wall, elevation: f64) -> Option<TopProfile> {
    let nominal = cuttable_height(wall)?;
    let cover = cover?;
    let base = match wall.class {
        WallClass::Foundation => elevation - wall.foundation_height,
        _ => elevation,
    } + wall.bottom_offset;
    let mut pieces = Vec::new();
    let mut shaped = false;
    for f in facets(wall) {
        let profile = if wall.class == WallClass::Standard {
            cover.wall_top(&f.wall, base, nominal)
        } else {
            cover.wall_top_clipped(&f.wall, base, nominal)
        };
        let scale = (f.s1 - f.s0) / f.wall.length();
        match profile {
            Some(p) => {
                shaped = true;
                pieces.extend(p.pieces.iter().map(|q| crate::clip::Piece {
                    s0: f.s0 + q.s0 * scale,
                    s1: f.s0 + q.s1 * scale,
                    ..*q
                }));
            }
            None => pieces.push(crate::clip::Piece {
                s0: f.s0,
                s1: f.s1,
                h0: nominal,
                h1: nominal,
            }),
        }
    }
    shaped.then_some(TopProfile { pieces })
}

/// A curved wall of its class: the solid classes follow the arc with ends
/// mitred against the walls joined to it, the roof cuts its top facet by
/// facet, and its openings are cut through it (jambs and head follow the arc;
/// the door or window unit stands square to the arc's tangent at its center).
fn add_curved(
    floor: &Floor,
    wall: &Wall,
    interior: InteriorSign,
    opts: &SceneOptions,
    lookup: Lookup,
    cover: Option<&FloorCover>,
    out: &mut Vec<Mesh>,
) {
    let top = curved_top(cover, wall, floor.elevation);
    let mut basis = opening_basis(wall);
    if let (Some(t), WallClass::Standard) = (&top, &wall.class) {
        // A gable rises above the plate; openings in it need the taller wall.
        basis.height = basis.height.max(t.max_height());
    }
    let hosted: Vec<(&Opening, Hole)> = floor
        .openings_on(wall.id)
        .filter_map(|o| hole_for(&basis, o).map(|h| (o, h)))
        .collect();
    let holes: Vec<Hole> = hosted.iter().map(|(_, h)| *h).collect();
    let cuts = EndCuts::of(&floor.walls, wall);
    out.extend(build_class_cut(
        wall,
        floor.elevation,
        &holes,
        interior,
        lookup,
        top.as_ref(),
        &cuts,
    ));
    let base = match wall.class {
        WallClass::Foundation => floor.elevation - wall.foundation_height,
        _ => floor.elevation,
    };
    for (opening, hole) in &hosted {
        let local = crate::opening::tangent_wall(&basis, hole);
        out.extend(crate::opening::build_opening_in_wall(
            &local, opening, hole, base, interior, opts, &hosted,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{Point, Project, WallKind};

    fn wall_of(class: WallClass, bottom: f64) -> (Project, Wall) {
        let mut p = Project::new("t");
        let id = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            60.0,
            WallKind::Exterior,
        );
        let mut w = p.floors[0].wall(id).unwrap().clone();
        w.class = class;
        w.bottom_offset = bottom;
        (p, w)
    }

    fn y_range(meshes: &[Mesh]) -> (f32, f32) {
        let ys: Vec<f32> = meshes
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .collect();
        (
            ys.iter().copied().fold(f32::MAX, f32::min),
            ys.iter().copied().fold(f32::MIN, f32::max),
        )
    }

    #[test]
    fn every_straight_class_honors_the_bottom_offset() {
        let classes = [
            WallClass::Standard,
            WallClass::Glass,
            WallClass::HalfWall { height: 36.0 },
            WallClass::Pony {
                upper_type: "A".into(),
                lower_type: "B".into(),
                split_height: 24.0,
                upper_sets_plan_display: false,
            },
        ];
        for class in classes {
            let none = |_: &str| None;
            let (_, flat) = wall_of(class.clone(), 0.0);
            let (_, raised) = wall_of(class.clone(), 40.0);
            let a = y_range(&build_class(&flat, 0.0, &[], 1.0, &none));
            let b = y_range(&build_class(&raised, 0.0, &[], 1.0, &none));
            assert_eq!((b.0 - a.0, b.1 - a.1), (40.0, 40.0), "{class:?}");
        }
    }

    #[test]
    fn raised_wall_holes_are_measured_from_the_floor() {
        use plan_core::Opening;
        let (_, w) = wall_of(WallClass::Standard, 40.0);
        let mut win = Opening::default_window(2, w.id, 60.0);
        win.sill_height = 50.0;
        win.height = 24.0;
        let hole = hole_for(&opening_basis(&w), &win).unwrap();
        assert_eq!((hole.h0, hole.h1), (50.0, 74.0));
        let none = |_: &str| None;
        let meshes = build_class(&w, 0.0, &[hole], 1.0, &none);
        // Reveals of the hole at its sill and head, in scene height.
        let at = |y: f32| {
            meshes
                .iter()
                .any(|m| m.vertices.iter().any(|v| (v.position[1] - y).abs() < 1e-3))
        };
        assert!(at(50.0) && at(74.0));
    }
}
