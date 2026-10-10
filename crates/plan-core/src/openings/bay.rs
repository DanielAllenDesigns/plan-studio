//! Bay, box and bow windows as wall-section units (DW-48, DW-155, DW-167;
//! manual pp. 604, 605, 634 to 642).
//!
//! A bay window is three wall sections, each with one component window, set at
//! an angle to the main wall; a box window is a bay with 90 degree sides; a bow
//! is two to twenty identical sections on a curve. The unit is one
//! [`Opening`](crate::model::Opening) whose width is the opening in the main
//! wall and whose [`BayUnit`] (kept in `OpeningSpec::bay`) holds what the
//! Bay/Box and Bow Specification dialogs set. [`bay_shape`] turns that into the
//! footprint every view draws: positions are `(s, d)` with `s` along the wall
//! from the unit's start jamb and `d` outward from the wall's exterior face.
//!
//! The model side of the rest of the unit lives here too: the depth handle
//! ([`Project::set_bay_depth`]), the automatic width and radius dimensions
//! ([`bay_dimensions`]), the foundation under a unit on the first floor
//! ([`Project::bay_foundations`]), Explode into walls, windows and a room
//! ([`Project::explode_bay`]) and the count of roof planes a roof option gives
//! ([`roof_plane_count`]).

use super::mull::MulledLabel;
use super::spec::{BayRoof, BayRoofKind};
use super::OpeningStyle;
use crate::geometry::Point;
use crate::model::{Id, Opening, OpeningKind, Project, RoomName, Wall, WallKind};
use crate::opening_symbol::inset_polyline;
use serde::{Deserialize, Serialize};

/// Narrowest a bay, box or bow window may be placed, inches (manual p. 604).
pub const MIN_UNIT_WIDTH: f64 = 30.0;
/// Narrowest front of a bay, inches.
const MIN_FRONT: f64 = 6.0;
/// Depth of the foundation wall under a unit, inches.
pub const FOUNDATION_DEPTH: f64 = 24.0;
/// Thickness of that foundation wall, inches.
pub const FOUNDATION_THICKNESS: f64 = 8.0;
/// One standard trimmer beside a component window, inches.
pub const TRIMMER: f64 = 1.5;
/// Fewest and most sections of a bow.
pub const BOW_MIN: u32 = 2;
pub const BOW_MAX: u32 = 20;

/// The lowered ceiling of a unit (Ceiling box of the General panel).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LoweredCeiling {
    /// Lowered from the ceiling of the adjacent room, inches.
    pub height: f64,
    pub finish: f64,
    pub structure: f64,
}

impl Default for LoweredCeiling {
    fn default() -> Self {
        Self {
            height: 6.0,
            finish: 0.5,
            structure: 5.5,
        }
    }
}

/// The raised floor of a unit (Floor box of the General panel): a bench seat
/// or a garden window.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RaisedFloor {
    /// Raised from the floor of the adjacent room, inches.
    pub height: f64,
    /// Measure the height from the floor finish instead of the subfloor.
    pub use_floor_finish: bool,
    pub finish: f64,
    pub structure: f64,
}

impl Default for RaisedFloor {
    fn default() -> Self {
        Self {
            height: 18.0,
            use_floor_finish: false,
            finish: 0.75,
            structure: 7.25,
        }
    }
}

/// The roof choices of the Options panel (manual pp. 636 to 641).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BayRoofOptions {
    /// The standard roof ignores the unit, which tucks under the eave.
    pub use_existing: bool,
    /// The main roof comes down over the unit and follows its shape.
    pub extend_existing: bool,
    /// The roof over the unit is square across the end instead of following
    /// the profile.
    pub rectangular: bool,
}

/// Which wall layer the components are recessed to (Components Recessed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BayRecess {
    /// To the main layer of the wall.
    #[default]
    Main,
    /// To the sheathing layer.
    Sheathing,
}

/// Everything the Bay/Box and Bow Specification dialogs set that is not on the
/// opening itself. `depth` of 0 means the style's own depth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BayUnit {
    /// Bay Angle: the angle of the side components to the main wall, degrees
    /// (45 for a bay, 90 for a box).
    pub angle_deg: f64,
    /// How far the unit projects, inches; 0 takes the style's.
    pub depth: f64,
    /// Quantity: the sections of a bow.
    pub segments: u32,
    /// Wall type of the embedded walls; `None` is Use Main Wall Type.
    pub wall_type: Option<String>,
    pub lowered_ceiling: Option<LoweredCeiling>,
    pub raised_floor: Option<RaisedFloor>,
    pub roof: BayRoofOptions,
    /// Display Standard Dimension: the width and depth dimensions.
    pub standard_dimension: bool,
    /// Display Dimensions to Center: the radial dimension of a bow.
    pub center_dimension: bool,
    /// Has Component Windows.
    pub has_components: bool,
    /// No Trimmers for Components.
    pub no_trimmers: bool,
    /// No Framing Between Components.
    pub no_framing_between: bool,
    /// Connect Outer Casing: one exterior casing around a bow's components.
    pub connect_outer_casing: bool,
    /// Components Recessed, and to which layer.
    pub recess: Option<BayRecess>,
    pub label: MulledLabel,
    /// A foundation wall is built under the unit on the first floor unless it
    /// is raised from the floor.
    pub foundation: bool,
}

impl Default for BayUnit {
    fn default() -> Self {
        Self {
            angle_deg: 45.0,
            depth: 0.0,
            segments: 5,
            wall_type: None,
            lowered_ceiling: None,
            raised_floor: None,
            roof: BayRoofOptions::default(),
            standard_dimension: true,
            center_dimension: false,
            has_components: true,
            no_trimmers: false,
            no_framing_between: false,
            connect_outer_casing: false,
            recess: None,
            label: MulledLabel::Single,
            foundation: true,
        }
    }
}

impl BayUnit {
    /// The depth the unit projects for `style`, inches (manual pp. 604, 605):
    /// 1 ft for a bay, 1 ft 6 in for a box, 11 1/2 in for a bow.
    pub fn depth_for(&self, style: OpeningStyle) -> f64 {
        if self.depth > 0.0 {
            return self.depth;
        }
        match style {
            OpeningStyle::BoxWindow => 18.0,
            OpeningStyle::BowWindow => 11.5,
            _ => 12.0,
        }
    }

    /// The angle of the side components: 90 for a box, the set angle (45
    /// unless changed) for a bay.
    pub fn angle_for(&self, style: OpeningStyle) -> f64 {
        match style {
            OpeningStyle::BoxWindow => 90.0,
            _ => self.angle_deg.clamp(10.0, 90.0),
        }
    }

    /// The number of sections: three for a bay or box, [`Self::segments`]
    /// for a bow.
    pub fn sections_for(&self, style: OpeningStyle) -> usize {
        match style {
            OpeningStyle::BowWindow => self.segments.clamp(BOW_MIN, BOW_MAX) as usize,
            _ => 3,
        }
    }

    /// The Bay Angle a new bay window starts with.
    pub fn for_style(style: OpeningStyle) -> BayUnit {
        BayUnit {
            angle_deg: if style == OpeningStyle::BoxWindow {
                90.0
            } else {
                45.0
            },
            center_dimension: style == OpeningStyle::BowWindow,
            ..BayUnit::default()
        }
    }

    /// The unit sits on the ground: not raised, so a foundation is built.
    pub fn on_grade(&self) -> bool {
        self.raised_floor.is_none_or(|r| r.height <= 0.0)
    }

    /// Height the unit's floor is raised above the adjacent room's, inches.
    pub fn floor_raise(&self) -> f64 {
        self.raised_floor.map_or(0.0, |r| r.height.max(0.0))
    }

    /// Height the unit's ceiling is lowered below the room's, inches.
    pub fn ceiling_drop(&self) -> f64 {
        self.lowered_ceiling.map_or(0.0, |c| c.height.max(0.0))
    }

    /// Whether a roof is built over the unit from its own options: not when the
    /// standard roof is told to use the existing one.
    pub fn builds_own_roof(&self) -> bool {
        !self.roof.use_existing
    }
}

/// The width a new bay, box or bow window has in its wall (manual pp. 604,
/// 605): 4 ft 2 in for a bay and a box, 5 ft 10 in for a bow.
pub fn default_unit_width(style: OpeningStyle) -> f64 {
    match style {
        OpeningStyle::BowWindow => 70.0,
        _ => 50.0,
    }
}

/// The stock width plans saved before the manual's sizes carry.
pub fn legacy_unit_width(style: OpeningStyle) -> f64 {
    match style {
        OpeningStyle::BoxWindow => 60.0,
        _ => 96.0,
    }
}

/// Whether `style` is a bay, box or bow window.
pub fn is_unit(style: OpeningStyle) -> bool {
    style.projects()
}

// ----- footprint -----

/// One wall section of a unit, from `a` to `b` in `(s, d)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Section {
    pub a: (f64, f64),
    pub b: (f64, f64),
}

impl Section {
    pub fn length(&self) -> f64 {
        (self.b.0 - self.a.0).hypot(self.b.1 - self.a.1)
    }

    /// The angle of the section to the main wall, degrees, `0..=90`.
    pub fn angle_deg(&self) -> f64 {
        (self.b.1 - self.a.1)
            .abs()
            .atan2((self.b.0 - self.a.0).abs())
            .to_degrees()
    }
}

/// The footprint of a unit in `(s, d)`.
#[derive(Debug, Clone, PartialEq)]
pub struct BayShape {
    /// The outer face, from the start jamb out and back to the end jamb.
    pub outline: Vec<(f64, f64)>,
    pub sections: Vec<Section>,
    /// How far the front stands from the wall face.
    pub depth: f64,
    /// Across the front of a bay or box (the flat), or the chord of a bow.
    pub front_width: f64,
    /// The radius and center `(s, d)` of a bow's arc (`None` for a bay or box).
    pub arc: Option<(f64, (f64, f64))>,
}

/// The footprint of a unit `width` wide in its wall (the opening in the main
/// wall), for `style` and `bay`.
pub fn bay_shape(style: OpeningStyle, width: f64, bay: &BayUnit) -> BayShape {
    let depth = bay.depth_for(style).max(1.0);
    let outline: Vec<(f64, f64)>;
    let mut front = width;
    let mut arc = None;
    match style {
        OpeningStyle::BowWindow => {
            let n = bay.sections_for(style);
            let c = width * 0.5;
            // The circle through both jambs with the depth for its sagitta.
            let r = (c * c + depth * depth) / (2.0 * depth);
            let (sc, dc) = (width * 0.5, depth - r);
            let phi0 = c.atan2(r - depth);
            let mut pts: Vec<(f64, f64)> = (0..=n)
                .map(|k| {
                    let phi = -phi0 + 2.0 * phi0 * k as f64 / n as f64;
                    (sc + r * phi.sin(), dc + r * phi.cos())
                })
                .collect();
            // No vertex sits at the apex of an odd bow; rescale so the unit
            // projects exactly `depth`.
            let max = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max).max(1e-9);
            let k = depth / max;
            for p in &mut pts {
                p.1 *= k;
            }
            // The jambs sit on the wall face, at the ends of the opening.
            pts[0] = (0.0, 0.0);
            pts[n] = (width, 0.0);
            arc = Some((r, (sc, dc)));
            outline = pts;
        }
        _ => {
            let angle = bay.angle_for(style).to_radians();
            // The sides run out at `angle` to the main wall.
            let raw = if angle >= std::f64::consts::FRAC_PI_2 - 1e-9 {
                0.0
            } else {
                depth / angle.tan()
            };
            let dx = raw.min(((width - MIN_FRONT) * 0.5).max(0.0));
            front = width - 2.0 * dx;
            outline = vec![(0.0, 0.0), (dx, depth), (width - dx, depth), (width, 0.0)];
        }
    }
    let sections = outline
        .windows(2)
        .map(|w| Section { a: w[0], b: w[1] })
        .collect();
    BayShape {
        outline,
        sections,
        depth,
        front_width: front,
        arc,
    }
}

/// The outline of the roof over a unit (`rectangular` squares it off across
/// the end: manual p. 638).
pub fn roof_footprint(shape: &BayShape, bay: &BayUnit) -> Vec<(f64, f64)> {
    if bay.roof.rectangular {
        let (lo, hi) = shape
            .outline
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p.0), h.max(p.0)));
        vec![(lo, 0.0), (lo, shape.depth), (hi, shape.depth), (hi, 0.0)]
    } else {
        shape.outline.clone()
    }
}

/// How many roof planes the roof over a unit has (manual pp. 636 to 638): a hip
/// has one plane per section and, over a bay or bow with angled sides, two more
/// that make the ridge at the back (the California ridge). A rectangular roof
/// is a hip over a rectangle; a shed or flat roof is one plane; using or
/// extending the existing roof adds none of its own.
pub fn roof_plane_count(style: OpeningStyle, bay: &BayUnit, roof: &BayRoof) -> usize {
    if bay.roof.use_existing || bay.roof.extend_existing {
        return 0;
    }
    match roof.kind.resolved(style == OpeningStyle::BoxWindow) {
        BayRoofKind::None => 0,
        BayRoofKind::Flat | BayRoofKind::Shed => 1,
        _ => {
            if bay.roof.rectangular {
                3
            } else {
                let sections = bay.sections_for(style);
                let angled = style != OpeningStyle::BoxWindow && bay.angle_for(style) < 90.0 - 1e-9;
                sections
                    + if angled || style == OpeningStyle::BowWindow {
                        2
                    } else {
                        0
                    }
            }
        }
    }
}

// ----- component windows -----

/// The span `(start, end)` along a section that its component window fills,
/// leaving a trimmer at each end (none when the unit has no trimmers; none
/// between sections when there is no framing between them).
pub fn component_span(
    section: &Section,
    first: bool,
    last: bool,
    bay: &BayUnit,
) -> Option<(f64, f64)> {
    if !bay.has_components {
        return None;
    }
    let len = section.length();
    let end_trim = if bay.no_trimmers { 0.0 } else { TRIMMER };
    let (a, b) = (
        if first || !bay.no_framing_between {
            end_trim
        } else {
            0.0
        },
        if last || !bay.no_framing_between {
            end_trim
        } else {
            0.0
        },
    );
    (len - a - b >= 4.0).then_some((a, len - b))
}

/// The width of the component window in `section`, inches.
pub fn component_width(section: &Section, first: bool, last: bool, bay: &BayUnit) -> Option<f64> {
    component_span(section, first, last, bay).map(|(a, b)| b - a)
}

// ----- dimensions -----

/// What an automatic dimension of a unit measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BayDimKind {
    /// Across the opening in the wall.
    Width,
    /// From the wall face to the front.
    Depth,
    /// From the center of a bow's arc to a corner.
    Radius,
}

/// One automatic dimension, in `(s, d)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BayDim {
    pub kind: BayDimKind,
    pub a: (f64, f64),
    pub b: (f64, f64),
    pub value: f64,
}

/// The width and radius dimensions the unit shows in plan (Options panel:
/// Display Standard Dimension, Display Dimensions to Center).
pub fn bay_dimensions(style: OpeningStyle, width: f64, bay: &BayUnit) -> Vec<BayDim> {
    let shape = bay_shape(style, width, bay);
    let mut v = Vec::new();
    if bay.standard_dimension {
        v.push(BayDim {
            kind: BayDimKind::Width,
            a: (0.0, shape.depth),
            b: (width, shape.depth),
            value: width,
        });
        v.push(BayDim {
            kind: BayDimKind::Depth,
            a: (width * 0.5, 0.0),
            b: (width * 0.5, shape.depth),
            value: shape.depth,
        });
    }
    if bay.center_dimension && style == OpeningStyle::BowWindow {
        if let Some((r, center)) = shape.arc {
            // To the first corner past the start jamb.
            let corner = shape.outline.get(1).copied().unwrap_or((0.0, 0.0));
            v.push(BayDim {
                kind: BayDimKind::Radius,
                a: center,
                b: corner,
                value: r,
            });
        }
    }
    v
}

// ----- the model side -----

/// The result of exploding a unit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Exploded {
    pub walls: Vec<Id>,
    pub windows: Vec<Id>,
    /// The pass-through left in the main wall where the unit opened it.
    pub opening: Option<Id>,
    /// The new room, when the unit had a lowered ceiling or a raised floor.
    pub room: Option<usize>,
}

impl Project {
    /// The depth handle (manual p. 634): sets how far the unit `id` projects.
    /// Returns `false` for an opening that is not a unit.
    pub fn set_bay_depth(&mut self, floor: usize, id: Id, depth: f64) -> bool {
        let Some(o) = self.floors[floor].openings.iter_mut().find(|o| o.id == id) else {
            return false;
        };
        if !o.style.projects() {
            return false;
        }
        o.extras.spec.bay.depth = depth.clamp(6.0, 96.0);
        true
    }

    /// Sets Bay Angle of a bay window (45 and 90 are the usual).
    pub fn set_bay_angle(&mut self, floor: usize, id: Id, angle_deg: f64) -> bool {
        let Some(o) = self.floors[floor].openings.iter_mut().find(|o| o.id == id) else {
            return false;
        };
        if o.style != OpeningStyle::BayWindow {
            return false;
        }
        o.extras.spec.bay.angle_deg = angle_deg.clamp(10.0, 90.0);
        true
    }

    /// The footprints of the foundation walls under the units of `floor`
    /// (first floor only, and not for a unit raised from the floor), as
    /// polygons in world coordinates: the unit's outline with the foundation's
    /// thickness. `exterior(wall)` is the side of a wall that is outside.
    pub fn bay_foundations(
        &self,
        floor: usize,
        exterior: &dyn Fn(&Wall) -> f64,
    ) -> Vec<(Id, Vec<Point>)> {
        if floor != 0 {
            return Vec::new();
        }
        let f = &self.floors[floor];
        f.openings
            .iter()
            .filter(|o| o.style.projects() && o.extras.spec.bay.foundation)
            .filter(|o| o.extras.spec.bay.on_grade())
            .filter_map(|o| {
                let wall = f.wall(o.wall_id)?;
                let sign = unit_side(o, exterior(wall));
                let pts = unit_world_outline(wall, o, sign);
                Some((o.id, pts))
            })
            .collect()
    }

    /// Explode Bay/Bow Window (manual p. 636): the unit becomes walls, one
    /// window in each, and — when it had a lowered ceiling or a raised floor — a
    /// room of its own with Raised Floor For Bump Out set. The main wall keeps a
    /// pass-through where the unit opened it. `exterior` is the side of the main
    /// wall that is outside. One undo step for the caller.
    pub fn explode_bay(&mut self, floor: usize, id: Id, exterior: f64) -> Result<Exploded, String> {
        let Some(o) = self.floors[floor]
            .openings
            .iter()
            .find(|o| o.id == id)
            .cloned()
        else {
            return Err("That window is gone".into());
        };
        if !o.style.projects() {
            return Err("Only a bay, box or bow window can be exploded".into());
        }
        let Some(wall) = self.floors[floor].wall(o.wall_id).cloned() else {
            return Err("The window has no wall".into());
        };
        let sign = unit_side(&o, exterior);
        let bay = &o.extras.spec.bay;
        let shape = bay_shape(o.style, o.width, bay);
        // The centerline of each new wall: the outer face moved in by half the
        // wall thickness.
        let thickness = wall.thickness;
        let inner = inset_polyline(&shape.outline, thickness * 0.5);
        let world = |p: (f64, f64)| -> Point {
            let s = o.start_offset() + p.0;
            wall.point_along(s)
                .add(wall.normal_along(s).scale(sign * (thickness * 0.5 + p.1)))
        };
        let raise = bay.floor_raise();
        let drop = bay.ceiling_drop();
        let height = (wall.height - raise - drop).max(12.0);
        let n = shape.sections.len();
        let mut out = Exploded::default();
        let mut corners: Vec<Point> = Vec::new();
        for (i, sec) in shape.sections.iter().enumerate() {
            let (a, b) = (world(inner[i]), world(inner[i + 1]));
            if a.dist(b) < 1.0 {
                continue;
            }
            let wid = self.alloc_id();
            let mut w = Wall::new(a, b, thickness, height, WallKind::Exterior);
            w.id = wid;
            w.bottom_offset = raise;
            w.wall_type = bay.wall_type.clone().or_else(|| wall.wall_type.clone());
            // Outside is away from the unit's middle.
            let mid = Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
            let toward = world((o.width * 0.5, shape.depth * 0.4));
            w.exterior_side = if (b.sub(a)).cross(toward.sub(mid)) > 0.0 {
                crate::walls::Side::Right
            } else {
                crate::walls::Side::Left
            };
            self.floors[floor].walls.push(w);
            out.walls.push(wid);
            corners.push(a);
            if i + 1 == n {
                corners.push(b);
            }
            // One component window in the section.
            if let Some(width) = component_width(sec, i == 0, i + 1 == n, bay) {
                let win_id = self.alloc_id();
                let mut win = Opening::new(
                    wid,
                    sec.length() * 0.5,
                    OpeningKind::Window,
                    width,
                    o.height.min(height - 2.0).max(6.0),
                    o.sill_height.max(raise),
                );
                win.id = win_id;
                win.extras.spec = o.extras.spec.clone();
                win.extras.spec.bay = BayUnit::default();
                win.extras.spec.mulled = None;
                win.style = OpeningStyle::Window;
                win.extras.spec.window_type = super::types::WindowType::DoubleHung;
                win.casing = o.casing;
                self.floors[floor].openings.push(win);
                out.windows.push(win_id);
            }
        }
        // The main wall stays open where the unit was.
        let host_height = wall.height;
        let mut gap = Opening::new(
            o.wall_id,
            o.center_offset,
            OpeningKind::Window,
            o.width,
            host_height - drop,
            0.0,
        );
        gap.id = self.alloc_id();
        gap.style = OpeningStyle::PassThrough;
        out.opening = Some(gap.id);
        self.floors[floor].openings.push(gap);
        self.floors[floor].openings.retain(|x| x.id != id);
        if raise > 0.0 || drop > 0.0 {
            let mut c = Point::ZERO;
            for p in &corners {
                c = c.add(*p);
            }
            c = c.scale(1.0 / corners.len().max(1) as f64);
            let mut room = RoomName::new(c, "Bay", "Bay");
            room.floor_height_offset = raise;
            room.ceiling_height = Some(wall.height - drop);
            room.options.raised_floor_bump_out = true;
            self.floors[floor].room_names.push(room);
            out.room = Some(self.floors[floor].room_names.len() - 1);
        }
        Ok(out)
    }
}

/// The side of the main wall the unit projects toward: the exterior, or the
/// other one when Reverse Side flipped it.
pub fn unit_side(o: &Opening, exterior: f64) -> f64 {
    if o.swing_flipped {
        -exterior
    } else {
        exterior
    }
}

/// The footprint of the foundation under a unit in world coordinates: the
/// outline plus the foundation thickness toward the inside, closed along the
/// wall face.
fn unit_world_outline(wall: &Wall, o: &Opening, sign: f64) -> Vec<Point> {
    let shape = bay_shape(o.style, o.width, &o.extras.spec.bay);
    let half = wall.thickness * 0.5;
    let world = |p: (f64, f64)| -> Point {
        let s = o.start_offset() + p.0;
        wall.point_along(s)
            .add(wall.normal_along(s).scale(sign * (half + p.1)))
    };
    shape.outline.iter().map(|p| world(*p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bay_starts_26_in_across_the_front_and_12_deep() {
        // 4 ft 2 in at the back, 2 ft 2 in across the front, 1 ft deep (p. 604).
        let bay = BayUnit::for_style(OpeningStyle::BayWindow);
        let s = bay_shape(OpeningStyle::BayWindow, 50.0, &bay);
        assert_eq!(s.depth, 12.0);
        assert!((s.front_width - 26.0).abs() < 1e-9);
        assert_eq!(s.sections.len(), 3);
        assert_eq!(s.outline.len(), 4);
        // The sides run out at 45 degrees; the front is parallel to the wall.
        assert!((s.sections[0].angle_deg() - 45.0).abs() < 1e-6);
        assert!(s.sections[1].angle_deg().abs() < 1e-9);
        assert!((s.sections[2].angle_deg() - 45.0).abs() < 1e-6);
    }

    #[test]
    fn the_bay_angle_sets_the_side_sections() {
        let mut bay = BayUnit::for_style(OpeningStyle::BayWindow);
        bay.angle_deg = 60.0;
        let s = bay_shape(OpeningStyle::BayWindow, 60.0, &bay);
        assert!((s.sections[0].angle_deg() - 60.0).abs() < 1e-6);
        // Steeper sides leave a wider front: 60 - 2 * 12 / tan(60).
        assert!((s.front_width - (60.0 - 2.0 * 12.0 / 60f64.to_radians().tan())).abs() < 1e-9);
        // A deeper bay at the same width narrows the front, down to a floor.
        bay.depth = 40.0;
        bay.angle_deg = 45.0;
        let s = bay_shape(OpeningStyle::BayWindow, 60.0, &bay);
        assert!((s.front_width - MIN_FRONT).abs() < 1e-9);
        assert_eq!(s.depth, 40.0);
    }

    #[test]
    fn a_box_window_is_a_bay_with_90_degree_sides() {
        let bay = BayUnit::for_style(OpeningStyle::BoxWindow);
        let s = bay_shape(OpeningStyle::BoxWindow, 50.0, &bay);
        assert_eq!(s.depth, 18.0);
        assert_eq!(s.front_width, 50.0);
        assert_eq!(
            s.outline,
            vec![(0.0, 0.0), (0.0, 18.0), (50.0, 18.0), (50.0, 0.0)]
        );
        assert!((s.sections[0].angle_deg() - 90.0).abs() < 1e-9);
        // Even a bay angle typed into the data does not change a box.
        let mut b2 = bay.clone();
        b2.angle_deg = 30.0;
        assert_eq!(bay_shape(OpeningStyle::BoxWindow, 50.0, &b2), s);
    }

    #[test]
    fn a_bow_has_between_two_and_twenty_identical_sections() {
        let mut bay = BayUnit::for_style(OpeningStyle::BowWindow);
        let s = bay_shape(OpeningStyle::BowWindow, 70.0, &bay);
        assert_eq!(s.sections.len(), 5);
        assert!((s.depth - 11.5).abs() < 1e-9);
        let apex = s.outline.iter().map(|p| p.1).fold(f64::MIN, f64::max);
        assert!((apex - 11.5).abs() < 1e-9, "{apex}");
        // The outer sections are identical to each other, as are the inner two.
        let l: Vec<f64> = s.sections.iter().map(|x| x.length()).collect();
        assert!(
            (l[0] - l[4]).abs() < 0.5 && (l[1] - l[3]).abs() < 0.5,
            "{l:?}"
        );
        // The jambs sit on the wall face.
        assert_eq!(s.outline[0], (0.0, 0.0));
        assert_eq!(s.outline[5], (70.0, 0.0));
        for n in [2u32, 7, 20, 40] {
            bay.segments = n;
            let s = bay_shape(OpeningStyle::BowWindow, 70.0, &bay);
            assert_eq!(s.sections.len(), n.clamp(2, 20) as usize);
        }
        // The radial dimension runs from the center of the arc to a corner.
        bay.segments = 5;
        bay.center_dimension = true;
        let dims = bay_dimensions(OpeningStyle::BowWindow, 70.0, &bay);
        let radius = dims.iter().find(|d| d.kind == BayDimKind::Radius).unwrap();
        let (r, c) = bay_shape(OpeningStyle::BowWindow, 70.0, &bay).arc.unwrap();
        assert_eq!(radius.a, c);
        assert_eq!(radius.value, r);
        // The center is inside the wall, a few feet behind the face.
        assert!(c.1 < 0.0);
    }

    #[test]
    fn standard_dimensions_show_width_and_depth_unless_switched_off() {
        let mut bay = BayUnit::for_style(OpeningStyle::BayWindow);
        let d = bay_dimensions(OpeningStyle::BayWindow, 50.0, &bay);
        assert_eq!(
            d.iter().map(|x| (x.kind, x.value)).collect::<Vec<_>>(),
            vec![(BayDimKind::Width, 50.0), (BayDimKind::Depth, 12.0)]
        );
        bay.standard_dimension = false;
        assert!(bay_dimensions(OpeningStyle::BayWindow, 50.0, &bay).is_empty());
    }

    #[test]
    fn a_hip_roof_has_a_plane_per_section_and_two_for_the_california_ridge() {
        let hip = BayRoof::default();
        let bay = BayUnit::for_style(OpeningStyle::BayWindow);
        assert_eq!(roof_plane_count(OpeningStyle::BayWindow, &bay, &hip), 5);
        let box_ = BayUnit::for_style(OpeningStyle::BoxWindow);
        assert_eq!(
            roof_plane_count(OpeningStyle::BoxWindow, &box_, &hip),
            1,
            "box default is shed"
        );
        let hip_box = BayRoof {
            kind: BayRoofKind::Hip,
            ..hip
        };
        assert_eq!(
            roof_plane_count(OpeningStyle::BoxWindow, &box_, &hip_box),
            3
        );
        let bow = BayUnit::for_style(OpeningStyle::BowWindow);
        assert_eq!(roof_plane_count(OpeningStyle::BowWindow, &bow, &hip), 7);
        // Rectangular roof over: a hip over the bounding rectangle.
        let mut rect = bay.clone();
        rect.roof.rectangular = true;
        assert_eq!(roof_plane_count(OpeningStyle::BayWindow, &rect, &hip), 3);
        // Use or extend the existing roof: none of its own.
        let mut ex = bay.clone();
        ex.roof.use_existing = true;
        assert_eq!(roof_plane_count(OpeningStyle::BayWindow, &ex, &hip), 0);
        ex.roof.use_existing = false;
        ex.roof.extend_existing = true;
        assert_eq!(roof_plane_count(OpeningStyle::BayWindow, &ex, &hip), 0);
        // None, flat and shed.
        for (k, n) in [
            (BayRoofKind::None, 0),
            (BayRoofKind::Flat, 1),
            (BayRoofKind::Shed, 1),
        ] {
            let r = BayRoof { kind: k, ..hip };
            assert_eq!(roof_plane_count(OpeningStyle::BayWindow, &bay, &r), n);
        }
    }

    #[test]
    fn the_rectangular_roof_squares_off_the_outline() {
        let mut bay = BayUnit::for_style(OpeningStyle::BayWindow);
        let s = bay_shape(OpeningStyle::BayWindow, 50.0, &bay);
        assert_eq!(roof_footprint(&s, &bay), s.outline);
        bay.roof.rectangular = true;
        assert_eq!(
            roof_footprint(&s, &bay),
            vec![(0.0, 0.0), (0.0, 12.0), (50.0, 12.0), (50.0, 0.0)]
        );
    }

    #[test]
    fn components_leave_room_for_trimmers() {
        let mut bay = BayUnit::for_style(OpeningStyle::BayWindow);
        let s = bay_shape(OpeningStyle::BoxWindow, 50.0, &bay);
        let w0 = component_width(&s.sections[0], true, false, &bay).unwrap();
        assert!((w0 - (18.0 - 3.0)).abs() < 1e-9);
        bay.no_trimmers = true;
        assert!((component_width(&s.sections[0], true, false, &bay).unwrap() - 18.0).abs() < 1e-9);
        bay.no_trimmers = false;
        bay.no_framing_between = true;
        // The studs between sections go; the outer ones stay.
        let mid = component_width(&s.sections[1], false, false, &bay).unwrap();
        assert!((mid - 50.0).abs() < 1e-9);
        bay.has_components = false;
        assert_eq!(component_width(&s.sections[1], false, false, &bay), None);
    }

    fn house() -> (Project, Id) {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        (p, w)
    }

    fn place_bay(p: &mut Project, w: Id, style: OpeningStyle) -> Id {
        let id = p.alloc_id();
        let mut o = Opening::new(w, 120.0, OpeningKind::Window, 50.0, 60.0, 24.0);
        o.id = id;
        o.style = style;
        o.extras.spec.bay = BayUnit::for_style(style);
        p.floors[0].openings.push(o);
        id
    }

    #[test]
    fn the_depth_handle_and_bay_angle_edit_the_unit() {
        let (mut p, w) = house();
        let id = place_bay(&mut p, w, OpeningStyle::BayWindow);
        assert!(p.set_bay_depth(0, id, 20.0));
        assert!(p.set_bay_angle(0, id, 60.0));
        let o = &p.floors[0].openings[0];
        assert_eq!(o.extras.spec.bay.depth_for(o.style), 20.0);
        // A plain window has no depth.
        let plain = p.alloc_id();
        let mut win = Opening::default_window(plain, w, 40.0);
        win.width = 24.0;
        p.floors[0].openings.push(win);
        assert!(!p.set_bay_depth(0, plain, 20.0));
    }

    #[test]
    fn a_unit_on_the_first_floor_gets_a_foundation_unless_raised() {
        let (mut p, w) = house();
        let id = place_bay(&mut p, w, OpeningStyle::BayWindow);
        // Outside is the left (+y) side of this +x wall.
        let ext = |_: &Wall| 1.0;
        let f = p.bay_foundations(0, &ext);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].0, id);
        // The outline starts on the wall face and projects 12 in.
        let ys: Vec<f64> = f[0].1.iter().map(|q| q.y).collect();
        assert!(ys.iter().cloned().fold(f64::MIN, f64::max) > 14.9);
        // A bench seat raises it from the ground: no foundation.
        p.floors[0].openings[0].extras.spec.bay.raised_floor = Some(RaisedFloor::default());
        assert!(p.bay_foundations(0, &ext).is_empty());
        // Nor on an upper floor.
        p.floors[0].openings[0].extras.spec.bay.raised_floor = None;
        let up = p.insert_floor_above(0).unwrap();
        assert!(p.bay_foundations(up, &ext).is_empty());
    }

    #[test]
    fn exploding_a_bay_leaves_walls_windows_and_a_pass_through() {
        let (mut p, w) = house();
        let id = place_bay(&mut p, w, OpeningStyle::BayWindow);
        let walls = p.floors[0].walls.len();
        let r = p.explode_bay(0, id, 1.0).unwrap();
        assert_eq!(r.walls.len(), 3);
        assert_eq!(r.windows.len(), 3);
        assert_eq!(p.floors[0].walls.len(), walls + 3);
        assert!(p.floors[0].openings.iter().all(|o| o.id != id));
        let gap = p.floors[0]
            .openings
            .iter()
            .find(|o| Some(o.id) == r.opening)
            .unwrap();
        assert_eq!(gap.style, OpeningStyle::PassThrough);
        assert_eq!((gap.center_offset, gap.width), (120.0, 50.0));
        // No lowered ceiling or raised floor: no room.
        assert!(r.room.is_none());
        // The new walls project out of the main wall face (y > 3).
        let front = p.floors[0].wall(r.walls[1]).unwrap();
        assert!(front.start.y > 3.0 && front.end.y > 3.0);
        assert!((front.start.y - front.end.y).abs() < 1e-6);
    }

    #[test]
    fn exploding_a_raised_unit_makes_a_bump_out_room() {
        let (mut p, w) = house();
        let id = place_bay(&mut p, w, OpeningStyle::BoxWindow);
        {
            let b = &mut p.floors[0].openings[0].extras.spec.bay;
            b.raised_floor = Some(RaisedFloor::default());
            b.lowered_ceiling = Some(LoweredCeiling::default());
        }
        let r = p.explode_bay(0, id, 1.0).unwrap();
        let room = &p.floors[0].room_names[r.room.unwrap()];
        assert!(room.options.raised_floor_bump_out);
        assert_eq!(room.floor_height_offset, 18.0);
        assert_eq!(room.ceiling_height, Some(90.0));
        // The walls start at the raised floor and stop at the lowered ceiling.
        let wall = p.floors[0].wall(r.walls[0]).unwrap();
        assert_eq!(wall.bottom_offset, 18.0);
        assert_eq!(wall.height, 96.0 - 18.0 - 6.0);
        // Only units explode.
        let plain = p.alloc_id();
        p.floors[0]
            .openings
            .push(Opening::default_window(plain, w, 40.0));
        assert!(p.explode_bay(0, plain, 1.0).is_err());
    }
}
