//! Scene clipping of section and elevation cameras (`docs/parity/3d-views-cameras.md`
//! C-136..C-138, C-152, C-157; manual pp. 1171 to 1173, 1196, 1197).
//!
//! A cross section camera keeps a box of the model. Its front face is the
//! Cross Section Line (a straight line, or a stepped cutting plane made with
//! Add Break), its back face the Back Clip, its sides the Clip Width and its
//! top and bottom the Clip Elevation. Everything here is in the view frame of
//! the camera: `x` runs along the cut line from its left end to its right end
//! as the viewer sees it (0 at the line's centre), `depth` is the distance
//! ahead of the cut line and `y` is the height above the first floor. The
//! frame is the one `plan-elevation`'s `FreeView` uses, so a drawing's X is
//! this `x` and its Y is this `y`.

use crate::camera::{CameraKind, CameraObject};
use crate::geometry::Point;
use crate::model::Project;
use serde::{Deserialize, Serialize};

/// Name of the layer that holds the Clip Lines of a clipped view.
pub const CLIP_LINES_LAYER: &str = "CAD, Clip Lines";
/// Name of the locked layer that holds the Cross Section Lines.
pub const CROSS_SECTION_LAYER: &str = "Cross Section Lines";
/// Closest two breaks of a stepped plane may sit, inches.
pub const MIN_BREAK_GAP: f64 = 2.0;
/// The step a break gets from Make Perpendicular when it had none, inches.
pub const DEFAULT_STEP: f64 = 12.0;
/// Back Clip Framing After for a new view, inches.
pub const DEFAULT_FRAMING_BACK: f64 = 120.0;
/// How far a clip to room reaches past the room's far surface (through the
/// wall it looks at), inches. Matches `INTERIOR_BACK_EXTRA`.
pub const ROOM_BACK_EXTRA: f64 = crate::camera::INTERIOR_BACK_EXTRA;

fn yes() -> bool {
    true
}

fn default_top() -> f64 {
    120.0
}

fn default_framing() -> f64 {
    DEFAULT_FRAMING_BACK
}

/// A stepped cutting plane (C-137): the Cross Section Line with breaks in it.
/// `breaks` are positions along the line (view `x`, ascending); `offsets` has
/// one entry more than `breaks`: how far ahead of the base line each piece of
/// the plane stands. A straight line has no breaks and one zero offset.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StepPlane {
    pub breaks: Vec<f64>,
    pub offsets: Vec<f64>,
}

impl StepPlane {
    /// Does the line have any break?
    pub fn is_stepped(&self) -> bool {
        !self.breaks.is_empty()
    }

    /// Number of straight pieces.
    pub fn pieces(&self) -> usize {
        self.breaks.len() + 1
    }

    /// Puts the lists in order and gives every piece an offset.
    pub fn normalize(&mut self) {
        self.breaks.sort_by(f64::total_cmp);
        self.offsets.resize(self.breaks.len() + 1, 0.0);
    }

    /// The piece that `x` falls in (a point on a break belongs to the piece
    /// after it).
    pub fn piece_at(&self, x: f64) -> usize {
        self.breaks.iter().take_while(|b| **b <= x).count()
    }

    /// How far ahead of the base line the plane stands at `x`.
    pub fn depth_at(&self, x: f64) -> f64 {
        self.offsets.get(self.piece_at(x)).copied().unwrap_or(0.0)
    }

    /// Add Break (C-137): splits the piece under `x` in two; both keep its
    /// offset, so nothing moves until a handle is dragged. Returns the index
    /// of the new break, or `None` when `x` is within [`MIN_BREAK_GAP`] of an
    /// existing break.
    pub fn add_break(&mut self, x: f64) -> Option<usize> {
        self.normalize();
        if self.breaks.iter().any(|b| (b - x).abs() < MIN_BREAK_GAP) {
            return None;
        }
        let at = self.piece_at(x);
        let keep = self.offsets[at];
        self.breaks.insert(at, x);
        self.offsets.insert(at, keep);
        Some(at)
    }

    /// Takes a break out; the pieces on both sides become one piece at the
    /// offset of the piece before it.
    pub fn remove_break(&mut self, i: usize) {
        if i < self.breaks.len() {
            self.breaks.remove(i);
            self.offsets.remove(i + 1);
        }
    }

    /// Moves break `i` along the line, between its neighbours (and `lo`, `hi`
    /// the ends of the line).
    pub fn move_break(&mut self, i: usize, x: f64, lo: f64, hi: f64) {
        if i >= self.breaks.len() {
            return;
        }
        let min = if i == 0 { lo } else { self.breaks[i - 1] } + MIN_BREAK_GAP;
        let max = if i + 1 == self.breaks.len() {
            hi
        } else {
            self.breaks[i + 1]
        } - MIN_BREAK_GAP;
        if min <= max {
            self.breaks[i] = x.clamp(min, max);
        }
    }

    /// Drags the handle of piece `piece` perpendicular to the plane: the
    /// piece stands `offset` inches ahead of the base line.
    pub fn set_offset(&mut self, piece: usize, offset: f64) {
        self.normalize();
        if let Some(o) = self.offsets.get_mut(piece) {
            *o = offset;
        }
    }

    /// Make Parallel on break `i`: the piece after it lines up with the piece
    /// before it (the step is flattened; the break stays).
    pub fn make_parallel(&mut self, i: usize) {
        self.normalize();
        if i < self.breaks.len() {
            self.offsets[i + 1] = self.offsets[i];
        }
    }

    /// Make Perpendicular on break `i`: the step at the break is square to
    /// the line (always so in this model); a break with no step gets one of
    /// [`DEFAULT_STEP`] so the handle has something to drag.
    pub fn make_perpendicular(&mut self, i: usize) {
        self.normalize();
        if i < self.breaks.len() && (self.offsets[i + 1] - self.offsets[i]).abs() < 1e-9 {
            self.offsets[i + 1] = self.offsets[i] + DEFAULT_STEP;
        }
    }

    /// The pieces cut to `lo..hi`: `(x0, x1, offset)`, ascending, none empty.
    pub fn spans(&self, lo: f64, hi: f64) -> Vec<(f64, f64, f64)> {
        let mut out = Vec::new();
        let mut from = lo;
        for k in 0..self.pieces() {
            let to = self.breaks.get(k).copied().unwrap_or(hi).min(hi);
            let off = self.offsets.get(k).copied().unwrap_or(0.0);
            if to > from + 1e-9 {
                out.push((from, to, off));
            }
            from = from.max(to);
        }
        out
    }

    /// The plane as a polyline `(x, offset)` from `lo` to `hi`: a horizontal
    /// run per piece and a vertical step at every break whose neighbours
    /// differ.
    pub fn path(&self, lo: f64, hi: f64) -> Vec<(f64, f64)> {
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for (x0, x1, off) in self.spans(lo, hi) {
            pts.push((x0, off));
            pts.push((x1, off));
        }
        if pts.is_empty() {
            pts = vec![(lo, 0.0), (hi, 0.0)];
        }
        pts
    }
}

/// The Scene Clipping group of a cross section/elevation specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SectionClip {
    /// Poche: a dark fill on the clipped edges of walls, platforms and roof
    /// planes (C-152).
    #[serde(default = "yes")]
    pub poche: bool,
    /// Framing Back Clip: framing objects reach only this far behind the
    /// camera.
    pub framing_back_clip: bool,
    #[serde(default = "default_framing")]
    pub framing_back_after: f64,
    /// Clip Sides: the view is as wide as the Clip Width (the length of the
    /// cut line). Unchecked, the whole width of the model is drawn.
    pub clip_sides: bool,
    /// Clip Elevation: only heights between `bottom` and `top`.
    pub clip_elevation: bool,
    pub bottom: f64,
    #[serde(default = "default_top")]
    pub top: f64,
    /// Clip to Room: the view is the room the camera stands in.
    pub clip_to_room: bool,
    /// Railings and invisible walls do not close the room.
    #[serde(default = "yes")]
    pub ignore_railings: bool,
    /// Walls above the room's ceiling are left out.
    pub ignore_walls_above: bool,
    /// The cut line, with its breaks.
    pub plane: StepPlane,
}

impl Default for SectionClip {
    fn default() -> Self {
        Self {
            poche: true,
            framing_back_clip: false,
            framing_back_after: DEFAULT_FRAMING_BACK,
            clip_sides: false,
            clip_elevation: false,
            bottom: 0.0,
            top: default_top(),
            clip_to_room: false,
            ignore_railings: true,
            ignore_walls_above: false,
            plane: StepPlane::default(),
        }
    }
}

impl SectionClip {
    /// Chief's settings for a new camera of `kind`: a Wall Elevation clips to
    /// its room (C-138), everything else shows the whole model.
    pub fn for_kind(kind: CameraKind) -> Self {
        Self {
            clip_to_room: kind == CameraKind::WallElevation,
            ..Self::default()
        }
    }

    /// The Set as Default rule (manual p. 1173): which defaults a view's
    /// clipping updates.
    pub fn default_target(&self, back_clipped: bool) -> ClipDefaults {
        if self.clip_to_room {
            ClipDefaults::WallElevation
        } else if back_clipped {
            ClipDefaults::BackClipped
        } else {
            ClipDefaults::CrossSection
        }
    }
}

/// The three defaults pages a view's Set as Default can update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipDefaults {
    WallElevation,
    BackClipped,
    CrossSection,
}

impl ClipDefaults {
    pub fn label(self) -> &'static str {
        match self {
            ClipDefaults::WallElevation => "Wall Elevation",
            ClipDefaults::BackClipped => "Back Clipped Cross Section",
            ClipDefaults::CrossSection => "Cross Section/Elevation",
        }
    }
}

/// What a room gives a camera that clips to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomClip {
    /// Distance from the camera to the far surface plus [`ROOM_BACK_EXTRA`].
    pub back: f64,
    /// The room's extent along the cut line, view `x` left and right.
    pub x: (f64, f64),
    /// Floor and ceiling of the room's floor, inches above the first floor.
    pub y: (f64, f64),
}

/// The region a section camera keeps, in the view frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipVolume {
    /// Left and right limits (`None` keeps the whole width).
    pub x: Option<(f64, f64)>,
    /// Bottom and top limits (`None` keeps the whole height).
    pub y: Option<(f64, f64)>,
    /// The front plane.
    pub plane: StepPlane,
    /// Back clip distance behind the front plane.
    pub back: Option<f64>,
    /// Framing reaches this far behind the front plane.
    pub framing_back: Option<f64>,
}

impl ClipVolume {
    /// Is the point `(x, depth, y)` kept? `depth` is measured from the base
    /// cut line.
    pub fn contains(&self, x: f64, depth: f64, y: f64) -> bool {
        if self.x.is_some_and(|(lo, hi)| x < lo - 1e-6 || x > hi + 1e-6) {
            return false;
        }
        if self.y.is_some_and(|(lo, hi)| y < lo - 1e-6 || y > hi + 1e-6) {
            return false;
        }
        let front = self.plane.depth_at(x);
        depth >= front - 1e-6 && self.back.is_none_or(|b| depth <= front + b + 1e-6)
    }

    /// Is framing at this point kept? Framing has its own back limit.
    pub fn contains_framing(&self, x: f64, depth: f64, y: f64) -> bool {
        let front = self.plane.depth_at(x);
        self.contains(x, depth, y) && self.framing_back.is_none_or(|b| depth <= front + b + 1e-6)
    }

    /// The pieces of the front plane across `lo..hi` (the view's width).
    pub fn spans(&self, lo: f64, hi: f64) -> Vec<(f64, f64, f64)> {
        let (lo, hi) = self.x.map_or((lo, hi), |(a, b)| (lo.max(a), hi.min(b)));
        self.plane.spans(lo, hi)
    }
}

/// The vertical plane through a camera's cut line seen from above: `(centre,
/// tangent, forward)`. `None` for a camera without a cut line.
pub fn line_frame(c: &CameraObject) -> Option<(Point, Point, Point)> {
    let s = c.section?;
    let v = s.b - s.a;
    let len = v.length();
    if len < 1e-9 {
        return None;
    }
    let t = v * (1.0 / len);
    // The view looks to the left of A to B, the tangent turned a quarter
    // clockwise from the viewing direction.
    let fwd = Point::new(-t.y, t.x);
    Some((Point::lerp(s.a, s.b, 0.5), t, fwd))
}

/// Distance along `dir` from `from` to the nearest edge of `ring` ahead, or
/// `None`.
fn ray_hit(ring: &[Point], from: Point, dir: Point) -> Option<f64> {
    let n = ring.len();
    let mut best: Option<f64> = None;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let e = b - a;
        let den = dir.cross(e);
        if den.abs() < 1e-12 {
            continue;
        }
        let w = a - from;
        let t = w.cross(e) / den;
        let u = w.cross(dir) / den;
        if t > 1e-6 && (-1e-9..=1.0 + 1e-9).contains(&u) {
            best = Some(best.map_or(t, |b: f64| b.min(t)));
        }
    }
    best
}

impl Project {
    /// The room a camera that clips to its room is confined to: the room of
    /// the camera's floor that contains the camera, found with or without
    /// the railings and invisible walls (C-138).
    pub fn room_clip(&self, c: &CameraObject) -> Option<RoomClip> {
        let f = self.floors.get(c.floor)?;
        let (centre, tangent, fwd) = line_frame(c).unwrap_or_else(|| {
            let d = c.direction();
            (c.position, Point::new(d.y, -d.x), d)
        });
        let ignore = c.view.clip.ignore_railings;
        let walls: Vec<_> = f
            .walls
            .iter()
            .filter(|w| !(ignore && (w.flags.railing || w.flags.invisible)))
            .cloned()
            .collect();
        let rooms = crate::rooms::detect_rooms(&walls, 1.0);
        // Stand just in front of the cut line so a line lying on a wall face
        // still finds the room.
        let probe = centre + fwd * crate::camera::INTERIOR_CUT_INSET.max(2.0);
        let room = rooms
            .iter()
            .filter(|r| r.contains(probe) || r.contains(centre))
            .min_by(|a, b| a.area_sq_in.total_cmp(&b.area_sq_in))?;
        let ring = if room.inner_polygon.len() >= 3 {
            &room.inner_polygon
        } else {
            &room.polygon
        };
        let from = if room.contains(probe) { probe } else { centre };
        let ahead = ray_hit(ring, from, fwd)?;
        let right = ray_hit(ring, from, tangent).unwrap_or(0.0);
        let left = ray_hit(ring, from, tangent * -1.0).unwrap_or(0.0);
        let x0 = (from - centre).dot(tangent);
        Some(RoomClip {
            back: (from - centre).dot(fwd) + ahead + ROOM_BACK_EXTRA,
            x: (x0 - left, x0 + right),
            y: (f.elevation, f.elevation + f.ceiling_height),
        })
    }

    /// The clip volume of a section or elevation camera (C-136..C-138): its
    /// Scene Clipping settings, the length of its cut line and, for Clip to
    /// Room, the room it stands in.
    pub fn clip_volume(&self, c: &CameraObject) -> ClipVolume {
        let clip = &c.view.clip;
        let mut plane = clip.plane.clone();
        plane.normalize();
        let half = c.section.map(|s| s.a.dist(s.b) * 0.5);
        let mut v = ClipVolume {
            x: clip.clip_sides.then_some(()).and(half).map(|h| (-h, h)),
            y: clip.clip_elevation.then_some((clip.bottom, clip.top)),
            plane,
            back: crate::camera::back_clip_of(c),
            framing_back: clip.framing_back_clip.then_some(clip.framing_back_after),
        };
        if clip.clip_to_room {
            if let Some(r) = self.room_clip(c) {
                v.x = Some(r.x);
                v.back = Some(r.back);
                v.framing_back = Some(0.0);
                v.y = clip.ignore_walls_above.then_some(r.y).or(v.y);
                v.plane = StepPlane::default();
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stepped() -> StepPlane {
        let mut p = StepPlane::default();
        p.add_break(-20.0).unwrap();
        p.add_break(30.0).unwrap();
        p.set_offset(1, 24.0);
        p
    }

    #[test]
    fn a_straight_line_has_one_flat_piece() {
        let p = StepPlane::default();
        assert!(!p.is_stepped());
        assert_eq!(p.depth_at(5.0), 0.0);
        assert_eq!(p.spans(-50.0, 50.0), vec![(-50.0, 50.0, 0.0)]);
        assert_eq!(p.path(-50.0, 50.0), vec![(-50.0, 0.0), (50.0, 0.0)]);
    }

    #[test]
    fn add_break_splits_a_piece_and_keeps_its_offset() {
        let mut p = StepPlane::default();
        p.offsets = vec![6.0];
        assert_eq!(p.add_break(10.0), Some(0));
        assert_eq!(p.offsets, vec![6.0, 6.0]);
        assert_eq!(p.add_break(11.0), None, "too close to the first break");
        assert_eq!(p.add_break(-5.0), Some(0));
        assert_eq!(p.breaks, vec![-5.0, 10.0]);
        assert_eq!(p.offsets, vec![6.0, 6.0, 6.0]);
    }

    #[test]
    fn dragging_a_piece_makes_a_step() {
        let p = stepped();
        assert_eq!(p.breaks, vec![-20.0, 30.0]);
        assert_eq!(p.offsets, vec![0.0, 24.0, 0.0]);
        assert_eq!(p.depth_at(-40.0), 0.0);
        assert_eq!(p.depth_at(0.0), 24.0);
        assert_eq!(p.depth_at(30.0), 0.0, "a break belongs to the piece after it");
        assert_eq!(
            p.spans(-50.0, 50.0),
            vec![(-50.0, -20.0, 0.0), (-20.0, 30.0, 24.0), (30.0, 50.0, 0.0)]
        );
        // The polyline steps up at the first break and down at the second.
        assert_eq!(
            p.path(-50.0, 50.0),
            vec![
                (-50.0, 0.0),
                (-20.0, 0.0),
                (-20.0, 24.0),
                (30.0, 24.0),
                (30.0, 0.0),
                (50.0, 0.0)
            ]
        );
    }

    #[test]
    fn moving_a_break_keeps_it_between_its_neighbours() {
        let mut p = stepped();
        p.move_break(0, 100.0, -50.0, 50.0);
        assert!((p.breaks[0] - (30.0 - MIN_BREAK_GAP)).abs() < 1e-9);
        p.move_break(1, -100.0, -50.0, 50.0);
        assert!((p.breaks[1] - (p.breaks[0] + MIN_BREAK_GAP)).abs() < 1e-9);
        p.move_break(1, 500.0, -50.0, 50.0);
        assert!((p.breaks[1] - (50.0 - MIN_BREAK_GAP)).abs() < 1e-9);
    }

    #[test]
    fn make_parallel_flattens_and_make_perpendicular_restores_a_step() {
        let mut p = stepped();
        p.make_parallel(0);
        assert_eq!(p.offsets, vec![0.0, 0.0, 0.0]);
        p.make_perpendicular(0);
        assert_eq!(p.offsets[1], DEFAULT_STEP);
        // A break that already steps is left alone.
        p.set_offset(1, 7.0);
        p.make_perpendicular(0);
        assert_eq!(p.offsets[1], 7.0);
        p.remove_break(0);
        assert_eq!(p.breaks.len(), 1);
        assert_eq!(p.offsets.len(), 2);
    }

    #[test]
    fn the_volume_keeps_what_is_between_the_planes() {
        let v = ClipVolume {
            x: Some((-40.0, 40.0)),
            y: Some((0.0, 96.0)),
            plane: stepped(),
            back: Some(60.0),
            framing_back: Some(30.0),
        };
        // Sides, height, front and back.
        assert!(v.contains(0.0, 30.0, 50.0));
        assert!(!v.contains(41.0, 30.0, 50.0), "past the right clip line");
        assert!(!v.contains(0.0, 30.0, 97.0), "above the top clip");
        assert!(!v.contains(0.0, 10.0, 50.0), "in front of the stepped plane");
        assert!(v.contains(-30.0, 10.0, 50.0), "the left piece is not stepped");
        assert!(v.contains(0.0, 24.0 + 60.0, 50.0), "back plane follows the step");
        assert!(!v.contains(0.0, 24.0 + 61.0, 50.0));
        // Framing stops sooner.
        assert!(v.contains_framing(0.0, 24.0 + 30.0, 50.0));
        assert!(!v.contains_framing(0.0, 24.0 + 31.0, 50.0));
        // Unclipped sides keep the whole width.
        let open = ClipVolume { x: None, y: None, ..v };
        assert!(open.contains(900.0, 30.0, 500.0));
    }

    #[test]
    fn old_files_load_with_chiefs_defaults() {
        let c: SectionClip = serde_json::from_str("{}").unwrap();
        assert_eq!(c, SectionClip::default());
        assert!(c.poche && !c.clip_sides && !c.clip_elevation && c.ignore_railings);
        assert!(SectionClip::for_kind(CameraKind::WallElevation).clip_to_room);
        assert!(!SectionClip::for_kind(CameraKind::Elevation).clip_to_room);
        let mut c = SectionClip::default();
        c.plane = stepped();
        c.clip_elevation = true;
        let back: SectionClip = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn set_as_default_follows_the_boxes_that_are_checked() {
        let mut c = SectionClip::default();
        assert_eq!(c.default_target(false), ClipDefaults::CrossSection);
        assert_eq!(c.default_target(true), ClipDefaults::BackClipped);
        c.clip_to_room = true;
        assert_eq!(c.default_target(true), ClipDefaults::WallElevation);
    }
}
