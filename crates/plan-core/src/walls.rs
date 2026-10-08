//! Wall extensions for Chief parity (`docs/parity/walls.md`, `roofs.md`).
//!
//! * W-21..W-24: exterior/interior side ([`Side`]) and the wall option flags
//!   ([`WallFlags`]).
//! * W-26..W-30: the reference line a thickness change holds fixed
//!   ([`ResizeAbout`]) and [`Project::set_wall_thickness_about`] /
//!   [`Project::set_wall_type`].
//! * W-46..W-49: the wall-type registry on the project and the per-wall
//!   `wall_type` name.
//! * W-52..W-56: foundation, pony, half, divider and railing walls (flags).
//! * W-64..W-68: curved walls ([`WallCurve`]).
//! * RF-18..RF-25: per-wall roof directives ([`WallRoofDirective`]).
//! * Editing: split/break, join collinear walls and junction queries.

use crate::defaults::{RoofWallKind, WallTypeDef};
use crate::geometry::{project_on_segment, Point};
use crate::joins::{self, wall_layer_bands};
use crate::model::{Id, Project, Wall, WallEnd, WallKind, DEFAULT_CEILING_HEIGHT};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Smallest allowed wall thickness, inches (W-30).
pub const MIN_WALL_THICKNESS: f64 = 0.125;
/// Chief's default facet angle for curved walls, degrees (W-65).
pub const DEFAULT_FACET_ANGLE_DEG: f64 = 7.5;
/// Tolerance used by the editing helpers when matching wall ends, inches.
const EDIT_TOL: f64 = 0.5;

/// One side of a wall, looking from start to end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Side {
    /// The +normal side (counter-clockwise from the direction).
    #[default]
    Left,
    Right,
}

impl Side {
    /// `+1` for [`Side::Left`] (along `Wall::normal`), `-1` for [`Side::Right`].
    pub fn sign(self) -> f64 {
        match self {
            Side::Left => 1.0,
            Side::Right => -1.0,
        }
    }
    pub fn opposite(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

/// Which reference a thickness or wall-type change holds fixed (W-26, W-27).
/// The default (used for walls from old files) is `WallCenter`, which keeps
/// the previous "grow equally on both sides" behaviour;
/// [`ResizeAbout::default_for`] gives Chief's per-kind default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ResizeAbout {
    /// The exterior side of the main (structural) layer.
    MainLayerOutside,
    /// The interior side of the main layer.
    MainLayerInside,
    #[default]
    WallCenter,
    /// The exterior face of the wall.
    OuterSurface,
    /// The interior face of the wall.
    InnerSurface,
}

impl ResizeAbout {
    /// Chief's default: main layer outside for exterior walls, wall center
    /// for interior walls.
    pub fn default_for(kind: WallKind) -> ResizeAbout {
        match kind {
            WallKind::Exterior => ResizeAbout::MainLayerOutside,
            WallKind::Interior => ResizeAbout::WallCenter,
        }
    }
}

/// Pony wall: the lower part uses another wall type up to `lower_height` (W-53).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PonyWall {
    pub lower_type: String,
    /// Elevation of the lower wall top, inches.
    pub lower_height: f64,
}

/// Wall options and variants (W-24, W-52..W-58, R-3..R-5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WallFlags {
    /// Not drawn in plan or 3D.
    pub invisible: bool,
    /// Never defines a room boundary.
    pub no_room_definition: bool,
    /// Dimensions skip this wall.
    pub no_locate: bool,
    /// Invisible zero-height wall whose only job is to close a room (R-4).
    pub room_divider: bool,
    pub railing: bool,
    /// Lowered top, typically 42" (W-54).
    pub half_wall: bool,
    pub pony: Option<PonyWall>,
    pub foundation: bool,
    pub attic: bool,
}

impl WallFlags {
    /// Whether the wall closes rooms. A room divider always does; otherwise
    /// `no_room_definition` and `invisible` walls are skipped (R-3..R-5).
    pub fn defines_rooms(&self) -> bool {
        self.room_divider || !(self.no_room_definition || self.invisible)
    }
}

/// A curved wall stored as a true arc through `start`, the apex and `end`
/// (W-64..W-67). `bulge` is the signed sagitta: the distance from the chord
/// midpoint to the arc apex. Positive bulges toward the left (+normal) of
/// start-to-end. For curved walls, opening offsets are measured along the arc.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WallCurve {
    pub bulge: f64,
}

impl WallCurve {
    pub fn is_straight(&self) -> bool {
        self.bulge.abs() < 1e-9
    }

    /// Half the swept angle, radians, in `(0, PI]`.
    fn half_sweep(&self, chord: f64) -> f64 {
        2.0 * (2.0 * self.bulge.abs() / chord).atan()
    }

    /// Signed sweep from start to end, radians (negative = clockwise).
    pub fn sweep(&self, start: Point, end: Point) -> f64 {
        let chord = start.dist(end);
        if chord < 1e-9 || self.is_straight() {
            return 0.0;
        }
        -self.bulge.signum() * 2.0 * self.half_sweep(chord)
    }

    /// Arc center and radius, or `None` for a straight or degenerate wall.
    pub fn arc_center_radius(&self, start: Point, end: Point) -> Option<(Point, f64)> {
        let chord = start.dist(end);
        if chord < 1e-9 || self.is_straight() {
            return None;
        }
        let s = self.bulge;
        let r = (chord * chord / 4.0 + s * s) / (2.0 * s.abs());
        let n = end.sub(start).normalized().perp();
        let mid = Point::lerp(start, end, 0.5);
        Some((mid + n * (s - s.signum() * r), r))
    }

    /// Arc length, inches (the chord for a straight wall).
    pub fn arc_length(&self, start: Point, end: Point) -> f64 {
        match self.arc_center_radius(start, end) {
            Some((_, r)) => r * self.sweep(start, end).abs(),
            None => start.dist(end),
        }
    }

    /// `n + 1` points along the arc from start to end (`n` is clamped to at
    /// least 1). A straight wall returns evenly spaced chord points.
    pub fn sample_points(&self, start: Point, end: Point, n: usize) -> Vec<Point> {
        let n = n.max(1);
        let Some((c, r)) = self.arc_center_radius(start, end) else {
            return (0..=n)
                .map(|i| Point::lerp(start, end, i as f64 / n as f64))
                .collect();
        };
        let a0 = start.sub(c).angle();
        let sweep = self.sweep(start, end);
        (0..=n)
            .map(|i| {
                if i == 0 {
                    return start;
                }
                if i == n {
                    return end;
                }
                let a = a0 + sweep * i as f64 / n as f64;
                Point::new(c.x + r * a.cos(), c.y + r * a.sin())
            })
            .collect()
    }

    /// Number of facets for [`DEFAULT_FACET_ANGLE_DEG`] (at least 1).
    pub fn facet_count(&self, start: Point, end: Point) -> usize {
        let deg = self.sweep(start, end).abs() * 180.0 / PI;
        ((deg / DEFAULT_FACET_ANGLE_DEG).ceil() as usize).max(1)
    }

    /// The curve for the sub-arc between two points of this arc, given the
    /// sub-sweep (radians, magnitude) and the radius. Keeps the bulge sign.
    fn sub_curve(&self, radius: f64, sub_sweep: f64) -> WallCurve {
        WallCurve {
            bulge: self.bulge.signum() * radius * (1.0 - (sub_sweep.abs() * 0.5).cos()),
        }
    }
}

/// Roof directive of an exterior wall (RF-18..RF-25).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallRoofDirective {
    pub kind: RoofWallKind,
    /// Rise per 12 of run; `None` follows the roof defaults.
    pub pitch_in_12: Option<f64>,
    /// Second pitch: `(rise in 12, starts at height above floor)` (RF-25).
    pub upper_pitch: Option<(f64, f64)>,
    pub overhang: Option<f64>,
    pub auto_roof_return: bool,
}

impl Default for WallRoofDirective {
    fn default() -> Self {
        Self {
            kind: RoofWallKind::Hip,
            pitch_in_12: None,
            upper_pitch: None,
            overhang: None,
            auto_roof_return: false,
        }
    }
}

/// How another wall meets one end of a wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallConnection {
    pub other: Id,
    /// The end of the queried wall that is joined.
    pub at: WallEnd,
    pub kind: joins::ConnectionKind,
}

impl Wall {
    pub fn new(start: Point, end: Point, thickness: f64, height: f64, kind: WallKind) -> Wall {
        Wall {
            id: 0,
            start,
            end,
            thickness,
            height,
            kind,
            layer: crate::model::DEFAULT_WALL_LAYER.to_string(),
            flags: WallFlags::default(),
            wall_type: None,
            resize_about: ResizeAbout::default_for(kind),
            curve: None,
            roof: WallRoofDirective::default(),
            exterior_side: Side::Left,
            extras: crate::extras::WallExtras::default(),
        }
    }

    /// Whether the wall is a real (non-straight) arc.
    pub fn is_curved(&self) -> bool {
        self.curve.is_some_and(|c| !c.is_straight())
    }

    /// Arc center and radius of a curved wall (W-65).
    pub fn arc_center_radius(&self) -> Option<(Point, f64)> {
        self.curve?.arc_center_radius(self.start, self.end)
    }

    /// `n + 1` centerline points from start to end (chord points when straight).
    pub fn sample_points(&self, n: usize) -> Vec<Point> {
        match self.curve {
            Some(c) => c.sample_points(self.start, self.end, n),
            None => WallCurve { bulge: 0.0 }.sample_points(self.start, self.end, n),
        }
    }

    /// Length along the centerline: the arc length for curved walls, the
    /// chord (same as [`Wall::length`]) otherwise.
    pub fn path_length(&self) -> f64 {
        match self.curve {
            Some(c) => c.arc_length(self.start, self.end),
            None => self.length(),
        }
    }

    /// Unit normal pointing to the exterior side (W-21).
    pub fn exterior_normal(&self) -> Point {
        self.normal() * self.exterior_side.sign()
    }
}

impl Default for Wall {
    fn default() -> Self {
        Wall::new(
            Point::ZERO,
            Point::new(120.0, 0.0),
            crate::model::DEFAULT_INTERIOR_THICKNESS,
            DEFAULT_CEILING_HEIGHT,
            WallKind::Interior,
        )
    }
}

/// Lateral position (along the wall's +normal) of the reference line.
fn reference_lateral(wall: &Wall, ty: Option<&WallTypeDef>, about: ResizeAbout) -> f64 {
    let ext = wall.exterior_side.sign();
    let half = wall.thickness * 0.5;
    let bands = wall_layer_bands(wall, ty);
    let main = bands.iter().find(|b| b.is_main);
    match about {
        ResizeAbout::WallCenter => 0.0,
        ResizeAbout::OuterSurface => ext * half,
        ResizeAbout::InnerSurface => -ext * half,
        ResizeAbout::MainLayerOutside => main.map_or(ext * half, |b| b.outer),
        ResizeAbout::MainLayerInside => main.map_or(-ext * half, |b| b.inner),
    }
}

impl Project {
    /// The registered wall type called `name` (W-47).
    pub fn wall_type_def(&self, name: &str) -> Option<&WallTypeDef> {
        self.wall_types.iter().find(|t| t.name == name)
    }

    /// Add or replace (by name) a wall type in the project registry.
    pub fn register_wall_type(&mut self, ty: WallTypeDef) {
        match self.wall_types.iter_mut().find(|t| t.name == ty.name) {
            Some(slot) => *slot = ty,
            None => self.wall_types.push(ty),
        }
    }

    /// Move the centerline so the reference line `about` keeps its place when
    /// `old` is replaced by `new` (same position, different thickness/type).
    fn shifted_for_reference(
        &self,
        old: &Wall,
        new: &Wall,
        new_ty: Option<&WallTypeDef>,
        about: ResizeAbout,
    ) -> Point {
        let old_ty = old.wall_type.as_deref().and_then(|n| self.wall_type_def(n));
        let r_old = reference_lateral(old, old_ty, about);
        let r_new = reference_lateral(new, new_ty, about);
        old.normal() * (r_old - r_new)
    }

    /// Change a wall's total thickness, growing or shrinking it on the side
    /// the wall's [`ResizeAbout`] keeps fixed (W-27..W-29): the centerline
    /// shifts accordingly. A thickness edit changes the main layer only, so
    /// for a layered wall the main-layer references coincide with the surface
    /// references. Returns `false` for an unknown wall or a thickness below
    /// [`MIN_WALL_THICKNESS`].
    pub fn set_wall_thickness_about(&mut self, floor: usize, id: Id, new_thickness: f64) -> bool {
        if new_thickness.is_nan() || new_thickness < MIN_WALL_THICKNESS {
            return false;
        }
        let Some(old) = self.floors[floor].wall(id).cloned() else {
            return false;
        };
        let mut new = old.clone();
        new.thickness = new_thickness;
        let ty = old
            .wall_type
            .as_deref()
            .and_then(|n| self.wall_type_def(n))
            .cloned();
        let shift = self.shifted_for_reference(&old, &new, ty.as_ref(), old.resize_about);
        if let Some(w) = self.floors[floor].wall_mut(id) {
            w.thickness = new_thickness;
            w.start = w.start + shift;
            w.end = w.end + shift;
        }
        true
    }

    /// Give a wall a new wall type (registering it in the project) and set the
    /// thickness to the type's total, holding the `about` reference line fixed
    /// (W-27, W-49). `about` becomes the wall's stored reference. Returns
    /// `false` for an unknown wall or a type thinner than [`MIN_WALL_THICKNESS`].
    pub fn set_wall_type(
        &mut self,
        floor: usize,
        id: Id,
        ty: &WallTypeDef,
        about: ResizeAbout,
    ) -> bool {
        let t = ty.thickness();
        if t.is_nan() || t < MIN_WALL_THICKNESS {
            return false;
        }
        let Some(old) = self.floors[floor].wall(id).cloned() else {
            return false;
        };
        let mut new = old.clone();
        new.thickness = t;
        new.wall_type = Some(ty.name.clone());
        // Resolve the old stack before the registry entry may be replaced.
        let shift = self.shifted_for_reference(&old, &new, Some(ty), about);
        self.register_wall_type(ty.clone());
        if let Some(w) = self.floors[floor].wall_mut(id) {
            w.thickness = t;
            w.wall_type = Some(ty.name.clone());
            w.resize_about = about;
            w.start = w.start + shift;
            w.end = w.end + shift;
        }
        true
    }

    /// Split a wall at the point nearest `point` into two walls. The first
    /// piece (start side) keeps `id`; the second gets a fresh id. Openings
    /// move to the piece that holds them; flags, type and roof directive are
    /// preserved. Returns `None` if the wall is unknown, the point is within
    /// 0.01" of an end, or it falls inside an opening. Curved walls split
    /// into two arcs of the same radius.
    pub fn split_wall_at(&mut self, floor: usize, id: Id, point: Point) -> Option<(Id, Id)> {
        let wall = self.floors[floor].wall(id)?.clone();
        let curved = wall.is_curved();
        // Split position: arc length from the start, split point on the centerline.
        let (d, split_pt, curve_parts) = if curved {
            let curve = wall.curve?;
            let (c, r) = curve.arc_center_radius(wall.start, wall.end)?;
            let sweep = curve.sweep(wall.start, wall.end);
            let a0 = wall.start.sub(c).angle();
            let ap = point.sub(c).angle();
            // Signed angular travel from the start toward the point.
            let mut da = ap - a0;
            while da > PI {
                da -= 2.0 * PI;
            }
            while da < -PI {
                da += 2.0 * PI;
            }
            if da.signum() != sweep.signum() && da.abs() > 1e-9 {
                da += 2.0 * PI * sweep.signum();
            }
            let frac = (da / sweep).clamp(0.0, 1.0);
            let a = a0 + sweep * frac;
            let p = Point::new(c.x + r * a.cos(), c.y + r * a.sin());
            let s1 = sweep * frac;
            let s2 = sweep - s1;
            (
                r * s1.abs(),
                p,
                Some((curve.sub_curve(r, s1), curve.sub_curve(r, s2))),
            )
        } else {
            let (t, q) = project_on_segment(point, wall.start, wall.end);
            (t * wall.length(), q, None)
        };
        let total = wall.path_length();
        if d < 0.01 || total - d < 0.01 {
            return None;
        }
        let f = &self.floors[floor];
        if f.openings_on(id)
            .any(|o| o.start_offset() < d && o.end_offset() > d)
        {
            return None;
        }
        let new_id = self.alloc_id();
        let f = &mut self.floors[floor];
        let mut second = wall.clone();
        second.id = new_id;
        second.start = split_pt;
        second.end = wall.end;
        if let Some(w) = f.wall_mut(id) {
            w.end = split_pt;
        }
        if let Some((c1, c2)) = curve_parts {
            if let Some(w) = f.wall_mut(id) {
                w.curve = Some(c1);
            }
            second.curve = Some(c2);
        }
        f.walls.push(second);
        for o in f.openings.iter_mut().filter(|o| o.wall_id == id) {
            if o.center_offset > d {
                o.wall_id = new_id;
                o.center_offset -= d;
            }
        }
        for g in f.groups.iter_mut() {
            let had = g.members.contains(&crate::groups::ObjectRef::Wall(id));
            if had {
                g.members.push(crate::groups::ObjectRef::Wall(new_id));
            }
        }
        Some((id, new_id))
    }

    /// Alias of [`Project::split_wall_at`] (Chief's Break Wall).
    pub fn break_wall(&mut self, floor: usize, id: Id, point: Point) -> Option<(Id, Id)> {
        self.split_wall_at(floor, id, point)
    }

    /// Merge two straight walls that share an end and run in the same line
    /// with the same thickness, height, kind, layer, type and flags. The
    /// merged wall keeps `a`'s id and direction; `b`'s openings move onto it.
    /// Returns the merged id, or `None` if the walls cannot be merged.
    pub fn join_collinear_walls(&mut self, floor: usize, a: Id, b: Id) -> Option<Id> {
        if a == b {
            return None;
        }
        let wa = self.floors[floor].wall(a)?.clone();
        let wb = self.floors[floor].wall(b)?.clone();
        if wa.is_curved()
            || wb.is_curved()
            || (wa.thickness - wb.thickness).abs() > 1e-9
            || (wa.height - wb.height).abs() > 1e-9
            || wa.kind != wb.kind
            || wa.layer != wb.layer
            || wa.wall_type != wb.wall_type
            || wa.flags != wb.flags
            || wa.exterior_side != wb.exterior_side
        {
            return None;
        }
        if wa.direction().cross(wb.direction()).abs() > 1e-3 {
            return None;
        }
        let la = wa.length();
        let lb = wb.length();
        // Which ends touch.
        let ends = [
            (wa.end, wb.start, true, true),
            (wa.end, wb.end, true, false),
            (wa.start, wb.end, false, true),
            (wa.start, wb.start, false, false),
        ];
        let &(_, _, a_end, b_start) = ends
            .iter()
            .find(|(pa, pb, _, _)| pa.dist(*pb) <= EDIT_TOL)?;
        // Offsets are re-based onto a's frame.
        let (new_start, new_end, a_shift, b_map): (Point, Point, f64, Box<dyn Fn(f64) -> f64>) =
            match (a_end, b_start) {
                // a.end -- b.start: continue forward.
                (true, true) => (wa.start, wb.end, 0.0, Box::new(move |o| o + la)),
                // a.end -- b.end: b runs backward.
                (true, false) => (wa.start, wb.start, 0.0, Box::new(move |o| la + (lb - o))),
                // b.end -- a.start: b precedes a.
                (false, true) => (wb.start, wa.end, lb, Box::new(|o| o)),
                // b.start -- a.start: b precedes a, reversed.
                (false, false) => (wb.end, wa.end, lb, Box::new(move |o| lb - o)),
            };
        let f = &mut self.floors[floor];
        for o in f.openings.iter_mut() {
            if o.wall_id == a {
                o.center_offset += a_shift;
            } else if o.wall_id == b {
                o.wall_id = a;
                o.center_offset = b_map(o.center_offset);
            }
        }
        if let Some(w) = f.wall_mut(a) {
            w.start = new_start;
            w.end = new_end;
        }
        f.walls.retain(|w| w.id != b);
        for g in f.groups.iter_mut() {
            g.members
                .retain(|m| *m != crate::groups::ObjectRef::Wall(b));
        }
        Some(a)
    }

    /// Walls joined to the ends of wall `id` (corner, collinear continuation
    /// or T-junction where this wall butts into another), using the same
    /// matching as the join geometry in [`crate::joins`].
    pub fn wall_connections(&self, floor: usize, id: Id) -> Vec<WallConnection> {
        let walls = &self.floors[floor].walls;
        let Some(i) = walls.iter().position(|w| w.id == id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for at in [WallEnd::Start, WallEnd::End] {
            for (j, kind) in joins::wall_end_joins(walls, i, at, EDIT_TOL) {
                out.push(WallConnection {
                    other: walls[j].id,
                    at,
                    kind,
                });
            }
        }
        out
    }

    /// Walls whose end butts into the interior of wall `id` (the through wall
    /// side of a T-junction), with the butting end.
    pub fn walls_butting_into(&self, floor: usize, id: Id) -> Vec<(Id, WallEnd)> {
        let walls = &self.floors[floor].walls;
        let mut out = Vec::new();
        for (j, w) in walls.iter().enumerate() {
            if w.id == id {
                continue;
            }
            for at in [WallEnd::Start, WallEnd::End] {
                for (k, kind) in joins::wall_end_joins(walls, j, at, EDIT_TOL) {
                    if kind == joins::ConnectionKind::Tee && walls[k].id == id {
                        out.push((w.id, at));
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::OpeningKind;

    fn proj_with_wall(t: f64, kind: WallKind) -> (Project, Id) {
        let mut p = Project::new("t");
        let id = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            t,
            DEFAULT_CEILING_HEIGHT,
            kind,
        );
        (p, id)
    }

    #[test]
    fn wall_new_and_default() {
        let w = Wall::new(
            Point::ZERO,
            Point::new(10.0, 0.0),
            6.5,
            96.0,
            WallKind::Exterior,
        );
        assert_eq!(w.resize_about, ResizeAbout::MainLayerOutside);
        assert_eq!(w.exterior_side, Side::Left);
        let i = Wall::new(
            Point::ZERO,
            Point::new(10.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        assert_eq!(i.resize_about, ResizeAbout::WallCenter);
        let d = Wall::default();
        assert_eq!(d.kind, WallKind::Interior);
        assert!(d.length() > 0.0);
    }

    #[test]
    fn thickness_about_main_layer_shifts_center_by_half_delta() {
        // Exterior side is Left (+y), so the outside face is at y = +3.25.
        let (mut p, id) = proj_with_wall(6.5, WallKind::Exterior);
        assert_eq!(
            p.floors[0].wall(id).unwrap().resize_about,
            ResizeAbout::MainLayerOutside
        );
        assert!(p.set_wall_thickness_about(0, id, 8.5));
        let w = p.floors[0].wall(id).unwrap();
        assert!((w.thickness - 8.5).abs() < 1e-9);
        // Outside face fixed at +3.25, so the center moves to the interior side by 1".
        assert!((w.start.y - -1.0).abs() < 1e-9 && (w.end.y - -1.0).abs() < 1e-9);
        assert!((w.start.y + w.thickness * 0.5 - 3.25).abs() < 1e-9);

        // Interior face fixed.
        let (mut p, id) = proj_with_wall(6.5, WallKind::Exterior);
        p.floors[0].wall_mut(id).unwrap().resize_about = ResizeAbout::MainLayerInside;
        assert!(p.set_wall_thickness_about(0, id, 4.5));
        let w = p.floors[0].wall(id).unwrap();
        assert!((w.start.y - -1.0).abs() < 1e-9 || (w.start.y - 1.0).abs() < 1e-9);
        assert!((w.start.y - w.thickness * 0.5 - -3.25).abs() < 1e-9);

        // Wall center stays put; too-thin and unknown walls are refused.
        let (mut p, id) = proj_with_wall(4.5, WallKind::Interior);
        assert!(p.set_wall_thickness_about(0, id, 6.5));
        assert_eq!(p.floors[0].wall(id).unwrap().start.y, 0.0);
        assert!(!p.set_wall_thickness_about(0, id, 0.0));
        assert!(!p.set_wall_thickness_about(0, 999, 6.0));
    }

    #[test]
    fn set_wall_type_holds_reference_line() {
        let d = crate::defaults::PlanDefaults::chief_x18_daniel();
        let i4 = d.wall_type("Interior-4").unwrap().clone();
        let i6 = d.wall_type("Interior-6").unwrap().clone();

        // WallCenter: centerline unchanged.
        let (mut p, id) = proj_with_wall(4.5, WallKind::Interior);
        assert!(p.set_wall_type(0, id, &i4, ResizeAbout::WallCenter));
        assert!(p.set_wall_type(0, id, &i6, ResizeAbout::WallCenter));
        let w = p.floors[0].wall(id).unwrap();
        assert_eq!(w.start.y, 0.0);
        assert!((w.thickness - 6.5).abs() < 1e-9);
        assert_eq!(w.wall_type.as_deref(), Some("Interior-6"));

        // MainLayerOutside: the main layer's outside line (y = 2.25 - 0.5 = 1.75
        // for Interior-4) stays; Interior-6's main outside is at start.y + 3.25 - 0.5.
        let (mut p, id) = proj_with_wall(4.5, WallKind::Interior);
        assert!(p.set_wall_type(0, id, &i4, ResizeAbout::WallCenter));
        let before = p.floors[0].wall(id).unwrap().start.y + 2.25 - 0.5;
        assert!(p.set_wall_type(0, id, &i6, ResizeAbout::MainLayerOutside));
        let w = p.floors[0].wall(id).unwrap();
        let after = w.start.y + w.thickness * 0.5 - 0.5;
        assert!((before - after).abs() < 1e-9, "{before} vs {after}");
        assert!((w.start.y - -1.0).abs() < 1e-9);
        // The registry holds both definitions.
        assert!(p.wall_type_def("Interior-4").is_some());
    }

    #[test]
    fn curve_geometry() {
        let c = WallCurve { bulge: 2.0 };
        let (a, b) = (Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let (center, r) = c.arc_center_radius(a, b).unwrap();
        assert!((r - 7.25).abs() < 1e-9);
        assert!(center.dist(Point::new(5.0, -5.25)) < 1e-9);
        let pts = c.sample_points(a, b, 2);
        assert!(pts[1].dist(Point::new(5.0, 2.0)) < 1e-9);
        for p in c.sample_points(a, b, 8) {
            assert!((p.dist(center) - r).abs() < 1e-9);
        }
        // Negative bulge goes to the right.
        let n = WallCurve { bulge: -2.0 }.sample_points(a, b, 2);
        assert!(n[1].dist(Point::new(5.0, -2.0)) < 1e-9);
        // Semicircle.
        let semi = WallCurve { bulge: 5.0 };
        assert!((semi.arc_length(a, b) - 5.0 * PI).abs() < 1e-9);
        assert!(WallCurve { bulge: 0.0 }.arc_center_radius(a, b).is_none());
        let w = Wall {
            curve: Some(c),
            ..Wall::new(a, b, 4.5, 96.0, WallKind::Interior)
        };
        assert!(w.is_curved() && w.arc_center_radius().is_some());
        assert!(w.path_length() > w.length());
    }

    #[test]
    fn split_keeps_openings_on_the_right_piece() {
        let (mut p, id) = proj_with_wall(4.5, WallKind::Interior);
        let d1 = p.add_opening(0, id, 50.0, OpeningKind::Door).unwrap();
        let d2 = p.add_opening(0, id, 190.0, OpeningKind::Window).unwrap();
        p.floors[0].wall_mut(id).unwrap().flags.no_locate = true;
        p.floors[0].wall_mut(id).unwrap().wall_type = Some("Interior-4".into());
        // Splitting through an opening is refused.
        assert!(p.split_wall_at(0, id, Point::new(50.0, 3.0)).is_none());
        // Ends are refused.
        assert!(p.split_wall_at(0, id, Point::new(0.0, 0.0)).is_none());
        let (a, b) = p.split_wall_at(0, id, Point::new(120.0, 7.0)).unwrap();
        assert_eq!(a, id);
        assert_ne!(a, b);
        let f = &p.floors[0];
        assert!((f.wall(a).unwrap().length() - 120.0).abs() < 1e-9);
        assert!((f.wall(b).unwrap().length() - 120.0).abs() < 1e-9);
        assert_eq!(f.wall(b).unwrap().start, Point::new(120.0, 0.0));
        assert!(f.wall(b).unwrap().flags.no_locate);
        assert_eq!(f.wall(b).unwrap().wall_type.as_deref(), Some("Interior-4"));
        let o1 = f.openings.iter().find(|o| o.id == d1).unwrap();
        let o2 = f.openings.iter().find(|o| o.id == d2).unwrap();
        assert_eq!(o1.wall_id, a);
        assert_eq!(o2.wall_id, b);
        assert!((o2.center_offset - 70.0).abs() < 1e-9);
        // break_wall is an alias; join puts everything back.
        let merged = p.join_collinear_walls(0, a, b).unwrap();
        assert_eq!(merged, a);
        let f = &p.floors[0];
        assert_eq!(f.walls.len(), 1);
        assert!((f.wall(a).unwrap().length() - 240.0).abs() < 1e-9);
        let o2 = f.openings.iter().find(|o| o.id == d2).unwrap();
        assert_eq!(o2.wall_id, a);
        assert!((o2.center_offset - 190.0).abs() < 1e-9);
        assert!(p.break_wall(0, a, Point::new(100.0, 0.0)).is_some());
    }

    #[test]
    fn split_curved_wall_into_two_arcs() {
        let mut p = Project::new("c");
        let id = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        p.floors[0].wall_mut(id).unwrap().curve = Some(WallCurve { bulge: 20.0 });
        let (a, b) = p.split_wall_at(0, id, Point::new(50.0, 20.0)).unwrap();
        let f = &p.floors[0];
        let (wa, wb) = (f.wall(a).unwrap(), f.wall(b).unwrap());
        assert!(wa.end.dist(Point::new(50.0, 20.0)) < 1e-6);
        let (_, r0) = Wall {
            curve: Some(WallCurve { bulge: 20.0 }),
            ..Wall::new(
                Point::ZERO,
                Point::new(100.0, 0.0),
                4.5,
                96.0,
                WallKind::Interior,
            )
        }
        .arc_center_radius()
        .unwrap();
        assert!((wa.arc_center_radius().unwrap().1 - r0).abs() < 1e-6);
        assert!((wb.arc_center_radius().unwrap().1 - r0).abs() < 1e-6);
        assert!((wa.path_length() - wb.path_length()).abs() < 1e-6);
    }

    #[test]
    fn join_refuses_mismatched_walls() {
        let mut p = Project::new("j");
        let a = p.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let b = p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(200.0, 0.0),
            6.5,
            96.0,
            WallKind::Interior,
        );
        let c = p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(100.0, 90.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        assert!(p.join_collinear_walls(0, a, b).is_none());
        assert!(p.join_collinear_walls(0, a, c).is_none());
        assert!(p.join_collinear_walls(0, a, a).is_none());
    }

    #[test]
    fn connections_and_flags() {
        let mut p = Project::new("k");
        let a = p.add_wall(
            0,
            Point::ZERO,
            Point::new(120.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let b = p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 100.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let c = p.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(240.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let t = p.add_wall(
            0,
            Point::new(60.0, 80.0),
            Point::new(60.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        use joins::ConnectionKind::*;
        let ca = p.wall_connections(0, a);
        assert!(ca
            .iter()
            .any(|x| x.other == b && x.at == WallEnd::End && x.kind == Corner));
        assert!(ca.iter().any(|x| x.other == c && x.kind == Through));
        let ct = p.wall_connections(0, t);
        assert_eq!(ct.len(), 1);
        assert_eq!((ct[0].other, ct[0].at, ct[0].kind), (a, WallEnd::End, Tee));
        assert_eq!(p.walls_butting_into(0, a), vec![(t, WallEnd::End)]);
        assert!(p.wall_connections(0, 999).is_empty());

        let mut f = WallFlags::default();
        assert!(f.defines_rooms());
        f.invisible = true;
        assert!(!f.defines_rooms());
        f.room_divider = true;
        assert!(f.defines_rooms());
    }

    #[test]
    fn old_wall_json_defaults() {
        let w: Wall = serde_json::from_str(
            r#"{"id":1,"start":{"x":0.0,"y":0.0},"end":{"x":10.0,"y":0.0},
                "thickness":6.5,"height":96.0,"kind":"Exterior"}"#,
        )
        .unwrap();
        assert_eq!(w.flags, WallFlags::default());
        assert_eq!(w.roof, WallRoofDirective::default());
        assert!(w.curve.is_none() && w.wall_type.is_none());
        let s = serde_json::to_string(&w).unwrap();
        let back: Wall = serde_json::from_str(&s).unwrap();
        assert_eq!(back.roof, w.roof);
    }
}
