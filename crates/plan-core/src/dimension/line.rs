//! A dimension line as one object: N numbered extension lines and N - 1
//! segments (manual pp. 501 to 510, DECISIONS 81, 82 and DM17).
//!
//! The plan stores every segment as a [`Dimension`] of two measured points,
//! and the segments of one line share a [`super::DimSeg::string`] id (more
//! than twenty places read `Floor::dimensions`, so the stored form stays).
//! Everything a user does goes through the line, not the segment:
//! [`Floor::dimension_line`] reads the line as its extension lines and
//! segments, and the operations here insert, move, delete and append
//! extension lines, set what one extension line carries, move or turn the
//! whole line, and keep a fixed-proximity extension at its distance. Files
//! from before the strings were recorded load into one line per run of
//! segments ([`migrate_dimension_strings`]).

use super::{AutoGroup, Dimension, DimensionKind};
use crate::dim_assoc::{AnchorTarget, DimAnchor, Targets};
use crate::geometry::Point;
use crate::model::{Floor, Id, Project};
use serde::{Deserialize, Serialize};

/// Chief's layers for manual and automatic dimensions.
pub const MANUAL_LAYER: &str = "Dimensions, Manual";
pub const AUTO_LAYER: &str = "Dimensions, Automatic";

/// The shortest segment an edit may leave.
pub const MIN_SEG_LEN: f64 = 0.5;

/// Where the length of an extension line is counted from (Extensions
/// panel): a fixed length toward the marked object, or the gap that
/// stays between its end and the mark.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ExtReach {
    /// This much of the line reaches from the dimension line toward the
    /// object, however far the line is moved.
    Towards(f64),
    /// The line stops this far short of the mark.
    Gap(f64),
}

/// One extension line's own length settings ("Use Default" off).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExtLen {
    /// How far it runs past the dimension line, away from the object.
    pub away: f64,
    pub reach: ExtReach,
}

/// An elevation marker drawn on an extension line of a vertical dimension
/// (manual pp. 503, 518).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ElevMark {
    /// The text above the marker's line; empty takes the name of the mark
    /// the extension locates.
    pub text: String,
    /// The Saved Marker Defaults the marker inherits from.
    pub defaults: String,
}

/// What one end of a segment carries for its extension line. The line
/// writes it to both segments that meet at an extension line.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtProps {
    pub len: Option<ExtLen>,
    /// A fixed distance from the dimension line to the marked object: the
    /// line follows the object (one extension per dimension line).
    pub fixed_proximity: Option<f64>,
    pub marker: Option<ElevMark>,
}

impl ExtProps {
    pub fn is_default(&self) -> bool {
        *self == ExtProps::default()
    }
}

/// One extension line of a [`DimLine`].
#[derive(Debug, Clone, PartialEq)]
pub struct ExtLine {
    /// The number shown at its end, from 1.
    pub number: usize,
    /// The measured point it locates.
    pub point: Point,
    pub anchor: Option<DimAnchor>,
    pub hidden: bool,
    pub centerline: bool,
    pub props: ExtProps,
    /// The segment ends that carry it: `(segment id, end index)`.
    pub refs: Vec<(Id, usize)>,
}

/// A dimension line read as one object.
#[derive(Debug, Clone, PartialEq)]
pub struct DimLine {
    /// The segments in order along the line; `extensions.len() - 1` of them
    /// when the line is whole.
    pub segments: Vec<Id>,
    pub extensions: Vec<ExtLine>,
    /// The unit direction of the measuring line.
    pub dir: Point,
    /// The signed offset of the dimension line from the measured line.
    pub offset: f64,
    pub kind: DimensionKind,
    /// A radius, arc length or angle: one segment, no extension table.
    pub curved: bool,
    /// Segments run in both directions (after Reverse Dimension on part of
    /// the line); the extension edits refuse such a line.
    pub mixed: bool,
}

impl DimLine {
    /// The number of segments (extension lines less one).
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
}

/// The end index of a segment that faces back along `dir` (the low side).
fn lo_end(d: &Dimension, dir: Point) -> usize {
    usize::from(d.end.sub(d.start).dot(dir) < 0.0)
}

impl Floor {
    /// The dimension line `id` belongs to, read as extension lines and
    /// segments.
    pub fn dimension_line(&self, id: Id) -> Option<DimLine> {
        let ids = self.string_members(id);
        let mut segs: Vec<&Dimension> = ids
            .iter()
            .filter_map(|i| self.dimensions.iter().find(|d| d.id == *i))
            .collect();
        let first = *segs.first()?;
        let dir = first.end.sub(first.start).normalized();
        let curved = first.curve().is_some();
        let origin = first.start;
        let mixed = segs.iter().any(|s| s.end.sub(s.start).dot(dir) < 0.0);
        segs.sort_by(|a, b| {
            let ta = a.start.sub(origin).dot(dir).min(a.end.sub(origin).dot(dir));
            let tb = b.start.sub(origin).dot(dir).min(b.end.sub(origin).dot(dir));
            ta.total_cmp(&tb)
        });
        let mut exts: Vec<ExtLine> = Vec::new();
        for s in &segs {
            let lo = lo_end(s, dir);
            let pts = [(lo, true), (1 - lo, false)];
            for (end, is_lo) in pts {
                let point = if end == 0 { s.start } else { s.end };
                let seg = &s.look.seg;
                let merge = is_lo && exts.last().is_some_and(|e| e.point.dist(point) < 0.05);
                if merge {
                    let e = exts.last_mut().expect("checked");
                    e.hidden &= s.hide_ext[end];
                    e.centerline |= seg.centerline[end];
                    if e.anchor.is_none() {
                        e.anchor = s.anchors[end];
                    }
                    if e.props.is_default() {
                        e.props = seg.ext[end].clone();
                    }
                    e.refs.push((s.id, end));
                } else {
                    exts.push(ExtLine {
                        number: exts.len() + 1,
                        point,
                        anchor: s.anchors[end],
                        hidden: s.hide_ext[end],
                        centerline: seg.centerline[end],
                        props: seg.ext[end].clone(),
                        refs: vec![(s.id, end)],
                    });
                }
            }
        }
        Some(DimLine {
            segments: segs.iter().map(|s| s.id).collect(),
            extensions: exts,
            dir,
            offset: first.offset,
            kind: first.kind,
            curved,
            mixed,
        })
    }

    /// Sets what extension line `number` of the line carries; the changes
    /// are written to both segments that meet there. Returns whether the
    /// extension line exists.
    pub fn set_extension(
        &mut self,
        id: Id,
        number: usize,
        hidden: Option<bool>,
        centerline: Option<bool>,
        props: Option<ExtProps>,
    ) -> bool {
        let Some(line) = self.dimension_line(id) else {
            return false;
        };
        let Some(ext) = number.checked_sub(1).and_then(|n| line.extensions.get(n)) else {
            return false;
        };
        for (sid, end) in &ext.refs {
            if let Some(d) = self.dimensions.iter_mut().find(|d| d.id == *sid) {
                if let Some(h) = hidden {
                    d.hide_ext[*end] = h;
                }
                if let Some(c) = centerline {
                    d.look.seg.centerline[*end] = c;
                }
                if let Some(p) = &props {
                    d.look.seg.ext[*end] = p.clone();
                }
            }
        }
        true
    }

    /// The segment of the line that holds `at` strictly between its two
    /// measured points, and the distance of `at` from the segment's start.
    fn segment_holding(&self, id: Id, at: Point) -> Option<(Id, f64)> {
        let line = self.dimension_line(id)?;
        line.segments.iter().find_map(|sid| {
            let d = self.dimensions.iter().find(|d| d.id == *sid)?;
            let len = d.length();
            let u = d.end.sub(d.start).normalized();
            let t = at.sub(d.start).dot(u);
            (t > MIN_SEG_LEN && t < len - MIN_SEG_LEN).then_some((*sid, t))
        })
    }

    /// Moves the dimension line of `id` (every segment) by `d`.
    pub fn translate_line(&mut self, id: Id, d: Point) -> usize {
        self.move_string(id, d)
    }

    /// Turns the dimension line of `id` about `pivot`.
    pub fn rotate_line(&mut self, id: Id, pivot: Point, angle: f64) -> usize {
        self.rotate_string(id, pivot, angle)
    }

    /// Joins every run of segments that continue each other on one
    /// dimension line, belong to no string and were not taken out of one on
    /// purpose, into a string each. Returns how many strings were made.
    pub fn adopt_dimension_strings(&mut self) -> usize {
        let chains: Vec<Vec<Id>> = self
            .dimension_chains()
            .into_iter()
            .map(|c| c.into_iter().map(|i| self.dimensions[i].id).collect())
            .collect();
        let mut made = 0;
        for chain in chains {
            let same = {
                let ds: Vec<&Dimension> = chain
                    .iter()
                    .filter_map(|i| self.dimensions.iter().find(|d| d.id == *i))
                    .collect();
                let (k, g) = (ds[0].kind, ds[0].auto_group);
                ds.iter().all(|d| {
                    d.string_id().is_none()
                        && !d.look.seg.separate
                        && d.kind == k
                        && d.auto_group == g
                        && d.kind != DimensionKind::Temporary
                })
            };
            if same && self.join_string(&chain).is_some() {
                made += 1;
            }
        }
        made
    }

    /// Does any segment of the line of `id` hold a fixed-proximity
    /// extension line? Such a line cannot be moved by hand.
    pub fn line_is_fixed(&self, id: Id) -> bool {
        self.string_members(id).iter().any(|m| {
            self.dimensions
                .iter()
                .find(|d| d.id == *m)
                .is_some_and(|d| d.look.seg.ext.iter().any(|e| e.fixed_proximity.is_some()))
        })
    }

    /// Puts every line with a fixed-proximity extension at its distance
    /// from the object that extension locates (the line follows the
    /// object). Returns whether any line moved.
    pub fn enforce_fixed_proximity(&mut self) -> bool {
        let mut fixes: Vec<(Id, f64)> = Vec::new();
        {
            let t = Targets::of(self);
            let mut seen: Vec<Id> = Vec::new();
            for d in &self.dimensions {
                if seen.contains(&d.id)
                    || !d.look.seg.ext.iter().any(|e| e.fixed_proximity.is_some())
                {
                    continue;
                }
                let Some(line) = self.dimension_line(d.id) else {
                    continue;
                };
                seen.extend(line.segments.iter().copied());
                let Some(ext) = line
                    .extensions
                    .iter()
                    .find(|e| e.props.fixed_proximity.is_some())
                else {
                    continue;
                };
                let dist = ext.props.fixed_proximity.unwrap_or(0.0).abs();
                // Where the marked object really is, not just its place
                // on the measuring line.
                let obj = ext
                    .anchor
                    .and_then(|a| a.resolve_in(&t))
                    .unwrap_or(ext.point);
                let n = line.dir.perp();
                let side = if line.offset < 0.0 { -1.0 } else { 1.0 };
                let want = obj.add(n.scale(side * dist));
                let off = want.sub(ext.point).dot(n);
                if (off - line.offset).abs() > 1e-9 {
                    fixes.push((line.segments[0], off));
                }
            }
        }
        let mut changed = false;
        for (sid, off) in fixes {
            for m in self.string_members(sid) {
                if let Some(d) = self.dimensions.iter_mut().find(|d| d.id == m) {
                    if (d.offset - off).abs() > 1e-9 {
                        d.offset = off;
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// Removes extension line `number`: the two segments that meet there
    /// become one, or (an end extension of a line with more than two) the
    /// end segment goes. A line needs two extension lines.
    pub fn remove_extension(&mut self, id: Id, number: usize) -> Result<(), &'static str> {
        let line = self.dimension_line(id).ok_or("That dimension is gone")?;
        if line.curved {
            return Err("A curved dimension has no extension lines to delete");
        }
        if line.mixed {
            return Err("The segments run in different directions");
        }
        let n = line.extensions.len();
        if n <= 2 {
            return Err("A dimension line needs two extension lines");
        }
        if number == 0 || number > n {
            return Err("There is no such extension line");
        }
        let segs = &line.segments;
        if number == 1 || number == n {
            let gone = if number == 1 {
                segs[0]
            } else {
                *segs.last().expect("n > 2")
            };
            self.dimensions.retain(|d| d.id != gone);
        } else {
            // Interior: segment number - 2 absorbs segment number - 1.
            let (a_id, b_id) = (segs[number - 2], segs[number - 1]);
            let b = self
                .dimensions
                .iter()
                .find(|d| d.id == b_id)
                .cloned()
                .ok_or("That dimension is gone")?;
            let dir = line.dir;
            let b_hi = 1 - lo_end(&b, dir);
            let a = self
                .dimensions
                .iter_mut()
                .find(|d| d.id == a_id)
                .ok_or("That dimension is gone")?;
            let a_hi = 1 - lo_end(a, dir);
            if a_hi == 0 {
                a.start = if b_hi == 0 { b.start } else { b.end };
            } else {
                a.end = if b_hi == 0 { b.start } else { b.end };
            }
            a.anchors[a_hi] = b.anchors[b_hi];
            a.hide_ext[a_hi] = b.hide_ext[b_hi];
            a.look.seg.centerline[a_hi] = b.look.seg.centerline[b_hi];
            a.look.seg.ext[a_hi] = b.look.seg.ext[b_hi].clone();
            a.text_override = None;
            a.look.seg.shown = None;
            self.dimensions.retain(|d| d.id != b_id);
        }
        // A line left with one segment is no string.
        let rest = self.string_members(line.segments[usize::from(number == 1)]);
        if rest.len() == 1 {
            if let Some(d) = self.dimensions.iter_mut().find(|d| d.id == rest[0]) {
                d.look.seg.string = None;
            }
        }
        Ok(())
    }

    /// Moves extension line `number` along the measuring line to the
    /// projection of `to`, tied to `anchor` (None: the point is free). It
    /// stays between its neighbours.
    pub fn move_extension(
        &mut self,
        id: Id,
        number: usize,
        to: Point,
        anchor: Option<DimAnchor>,
    ) -> Result<(), &'static str> {
        let line = self.dimension_line(id).ok_or("That dimension is gone")?;
        if line.curved {
            return Err("A curved dimension has no extension lines to move");
        }
        if line.mixed {
            return Err("The segments run in different directions");
        }
        let n = line.extensions.len();
        if number == 0 || number > n {
            return Err("There is no such extension line");
        }
        let ext = &line.extensions[number - 1];
        // The new place on the measuring line.
        let t_old = ext.point.dot(line.dir);
        let t_new = to.sub(ext.point).dot(line.dir) + t_old;
        let lo = number.checked_sub(2).map_or(f64::NEG_INFINITY, |k| {
            line.extensions[k].point.dot(line.dir) + MIN_SEG_LEN
        });
        let hi = line
            .extensions
            .get(number)
            .map_or(f64::INFINITY, |e| e.point.dot(line.dir) - MIN_SEG_LEN);
        if t_new < lo || t_new > hi {
            return Err("An extension line stays between its neighbours");
        }
        let point = ext.point.add(line.dir.scale(t_new - t_old));
        for (sid, end) in &ext.refs {
            if let Some(d) = self.dimensions.iter_mut().find(|d| d.id == *sid) {
                if *end == 0 {
                    d.start = point;
                } else {
                    d.end = point;
                }
                d.anchors[*end] = anchor.map(|a| DimAnchor { last: point, ..a });
                d.text_override = None;
                d.look.seg.shown = None;
            }
        }
        Ok(())
    }
}

impl Project {
    /// Inserts an extension line into the line of `id` at the projection of
    /// `at`: the segment that holds it splits in two (a new segment is
    /// added; every other segment keeps its value and text). Returns the id
    /// of the new segment.
    pub fn insert_dimension_extension(
        &mut self,
        floor: usize,
        id: Id,
        at: Point,
        anchor: Option<DimAnchor>,
    ) -> Result<Id, &'static str> {
        let f = &self.floors[floor];
        let line = f.dimension_line(id).ok_or("That dimension is gone")?;
        if line.curved {
            return Err("A curved dimension has no extension lines to add");
        }
        if line.mixed {
            return Err("The segments run in different directions");
        }
        let (sid, t) = f
            .segment_holding(id, at)
            .ok_or("Click between two extension lines")?;
        let orig = f
            .dimensions
            .iter()
            .find(|d| d.id == sid)
            .cloned()
            .ok_or("That dimension is gone")?;
        let u = orig.end.sub(orig.start).normalized();
        let mid = orig.start.add(u.scale(t));
        let anchor = anchor.map(|a| DimAnchor { last: mid, ..a });
        let string = orig.string_id().unwrap_or(orig.id);
        let mut second = orig.clone();
        second.start = mid;
        second.anchors = [anchor, orig.anchors[1]];
        second.hide_ext = [false, orig.hide_ext[1]];
        second.text_override = None;
        let mut seg = super::DimSeg {
            string: Some(string),
            ..Default::default()
        };
        seg.centerline = [false, orig.look.seg.centerline[1]];
        seg.ext = [ExtProps::default(), orig.look.seg.ext[1].clone()];
        second.look.seg = seg;
        let new_id = self.add_dimension(floor, second);
        let first = self.floors[floor]
            .dimensions
            .iter_mut()
            .find(|d| d.id == sid)
            .expect("found above");
        first.end = mid;
        first.anchors = [orig.anchors[0], anchor];
        first.hide_ext = [orig.hide_ext[0], false];
        first.text_override = None;
        first.look.seg.string = Some(string);
        first.look.seg.shown = None;
        first.look.seg.centerline[1] = false;
        first.look.seg.ext[1] = ExtProps::default();
        Ok(new_id)
    }

    /// Adds a segment past one end of the line of `id` (the Add Segments
    /// handle): from the last extension line (`at_end`) or the first, out to
    /// the projection of `to`. Returns the id of the new segment.
    pub fn append_dimension_segment(
        &mut self,
        floor: usize,
        id: Id,
        at_end: bool,
        to: Point,
        anchor: Option<DimAnchor>,
    ) -> Result<Id, &'static str> {
        let f = &self.floors[floor];
        let line = f.dimension_line(id).ok_or("That dimension is gone")?;
        if line.curved {
            return Err("A curved dimension has no segments to add");
        }
        if line.mixed {
            return Err("The segments run in different directions");
        }
        let edge_seg = if at_end {
            *line.segments.last().ok_or("That dimension is gone")?
        } else {
            line.segments[0]
        };
        let edge = f
            .dimensions
            .iter()
            .find(|d| d.id == edge_seg)
            .cloned()
            .ok_or("That dimension is gone")?;
        let forward = lo_end(&edge, line.dir) == 0;
        // The end of the edge segment that lies at the shared point.
        let edge_idx = usize::from(at_end == forward);
        let from = if edge_idx == 0 { edge.start } else { edge.end };
        let along = to.sub(from).dot(line.dir);
        let ok = if at_end {
            along >= MIN_SEG_LEN
        } else {
            along <= -MIN_SEG_LEN
        };
        if !ok {
            return Err("Drag the handle away from the line to add a segment");
        }
        let far = from.add(line.dir.scale(along));
        let anchor = anchor.map(|a| DimAnchor { last: far, ..a });
        let string = edge.string_id().unwrap_or(edge.id);
        let mut d = edge.clone();
        // Same orientation as the edge segment.
        let (start, end) = if at_end == forward {
            (from, far)
        } else {
            (far, from)
        };
        d.start = start;
        d.end = end;
        let (shared, free) = if at_end == forward { (0, 1) } else { (1, 0) };
        d.anchors = [None, None];
        d.anchors[shared] = edge.anchors[edge_idx];
        d.anchors[free] = anchor;
        d.hide_ext = [false, false];
        d.text_override = None;
        let mut seg = super::DimSeg {
            string: Some(string),
            ..Default::default()
        };
        seg.ext = [ExtProps::default(), ExtProps::default()];
        d.look.seg = seg;
        let new_id = self.add_dimension(floor, d);
        // The edge segment and the new one are one string.
        if let Some(e) = self.floors[floor]
            .dimensions
            .iter_mut()
            .find(|x| x.id == edge_seg)
        {
            e.look.seg.string = Some(string);
        }
        Ok(new_id)
    }
}

/// Project-load step: lines recorded before strings existed (Round 14 kept
/// the segments of a dimension line as separate dimensions) become one
/// string each. Returns whether anything changed.
pub fn migrate_dimension_strings(project: &mut Project) -> bool {
    let mut made = 0;
    for f in &mut project.floors {
        made += f.adopt_dimension_strings();
    }
    made > 0
}

impl Dimension {
    /// Does this segment measure across one wall, from one of its surfaces
    /// to the other (Display Wall Widths)?
    pub fn is_wall_width(&self) -> bool {
        match self.anchors {
            [Some(a), Some(b)] => {
                a.target == AnchorTarget::Wall
                    && b.target == AnchorTarget::Wall
                    && a.wall == b.wall
                    && (a.side - b.side).abs() > 0.01
            }
            _ => false,
        }
    }

    /// Does the segment show? A wall width hides when the dimension line
    /// does not display wall widths (the Interior Dimension default).
    pub fn segment_shows(&self) -> bool {
        self.look.wall_widths != Some(false) || !self.is_wall_width()
    }

    /// The layer the dimension is drawn on: its own (Layer panel), else
    /// `fallback`, the default for its kind.
    pub fn layer_or<'a>(&'a self, fallback: &'a str) -> &'a str {
        self.look
            .layer
            .as_deref()
            .filter(|l| !l.is_empty())
            .unwrap_or(fallback)
    }

    /// [`Dimension::layer_or`] with the default taken from the Dimension
    /// Defaults' Layer panel (Manual and Automatic), else Chief's names.
    pub fn layer_name(&self, setup: &super::DimSetup) -> String {
        let (named, plain) = match self.kind {
            DimensionKind::AutoExterior => (&setup.layer_automatic, AUTO_LAYER),
            _ => (&setup.layer_manual, MANUAL_LAYER),
        };
        let default = if named.is_empty() { plain } else { named };
        self.layer_or(default).to_string()
    }

    /// Did an automatic run make this segment and is it still automatic?
    pub fn is_automatic(&self) -> bool {
        self.kind == DimensionKind::AutoExterior && self.auto_group != AutoGroup::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dim_assoc::{DimAttach, DimHint};
    use crate::model::WallKind;

    /// A string of three segments (0..100..250..400) on y = 0, 24 inches off.
    fn string() -> (Project, Vec<Id>) {
        let mut p = Project::new("t");
        let mut ids = Vec::new();
        for (a, b) in [(0.0, 100.0), (100.0, 250.0), (250.0, 400.0)] {
            ids.push(p.add_dimension(
                0,
                Dimension::new(
                    0,
                    DimensionKind::Manual,
                    Point::new(a, 0.0),
                    Point::new(b, 0.0),
                    24.0,
                ),
            ));
        }
        p.floors[0].join_string(&ids);
        (p, ids)
    }

    #[test]
    fn a_string_reads_as_n_extension_lines_and_n_minus_one_segments() {
        let (p, ids) = string();
        let line = p.floors[0].dimension_line(ids[1]).unwrap();
        assert_eq!(line.extensions.len(), 4);
        assert_eq!(line.segments, ids);
        let xs: Vec<f64> = line.extensions.iter().map(|e| e.point.x).collect();
        assert_eq!(xs, vec![0.0, 100.0, 250.0, 400.0]);
        assert_eq!(
            line.extensions.iter().map(|e| e.number).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        // The shared extension lines name both segments that carry them.
        assert_eq!(line.extensions[1].refs.len(), 2);
        assert_eq!(line.extensions[0].refs.len(), 1);
        assert!(!line.mixed && !line.curved);
        assert_eq!(line.offset, 24.0);
    }

    #[test]
    fn inserting_an_extension_line_splits_one_segment_and_keeps_the_rest() {
        let (mut p, ids) = string();
        p.floors[0].dimensions[0].look.seg.leading = "(".into();
        p.floors[0].dimensions[2].look.seg.trailing = " TYP".into();
        let before: Vec<f64> = ids
            .iter()
            .map(|i| {
                p.floors[0]
                    .dimensions
                    .iter()
                    .find(|d| d.id == *i)
                    .unwrap()
                    .length()
            })
            .collect();
        let new = p
            .insert_dimension_extension(0, ids[1], Point::new(180.0, 7.0), None)
            .unwrap();
        let line = p.floors[0].dimension_line(ids[0]).unwrap();
        assert_eq!(line.extensions.len(), 5);
        assert_eq!(line.segments.len(), 4);
        let len = |id: Id| {
            p.floors[0]
                .dimensions
                .iter()
                .find(|d| d.id == id)
                .unwrap()
                .length()
        };
        // The untouched segments keep their values and text.
        assert_eq!(len(ids[0]), before[0]);
        assert_eq!(len(ids[2]), before[2]);
        assert_eq!(
            p.floors[0]
                .dimensions
                .iter()
                .find(|d| d.id == ids[0])
                .unwrap()
                .look
                .seg
                .leading,
            "("
        );
        assert_eq!(
            p.floors[0]
                .dimensions
                .iter()
                .find(|d| d.id == ids[2])
                .unwrap()
                .look
                .seg
                .trailing,
            " TYP"
        );
        // The split segment's halves add up to its old value.
        assert!((len(ids[1]) + len(new) - before[1]).abs() < 1e-9);
        assert!((len(ids[1]) - 80.0).abs() < 1e-9 && (len(new) - 70.0).abs() < 1e-9);
        // All four are still one line.
        assert_eq!(p.floors[0].string_members(new).len(), 4);
        // Outside any segment, nothing happens.
        assert!(p
            .insert_dimension_extension(0, ids[1], Point::new(500.0, 0.0), None)
            .is_err());
        assert!(p
            .insert_dimension_extension(0, ids[1], Point::new(100.2, 0.0), None)
            .is_err());
    }

    #[test]
    fn deleting_an_extension_line_merges_or_drops_and_keeps_two() {
        let (mut p, ids) = string();
        // Interior: segments 1 and 2 become one.
        p.floors[0].remove_extension(ids[0], 2).unwrap();
        let line = p.floors[0].dimension_line(ids[0]).unwrap();
        assert_eq!(line.extensions.len(), 3);
        let xs: Vec<f64> = line.extensions.iter().map(|e| e.point.x).collect();
        assert_eq!(xs, vec![0.0, 250.0, 400.0]);
        // An end extension of a line with three: the end segment goes.
        p.floors[0].remove_extension(ids[0], 3).unwrap();
        let line = p.floors[0].dimension_line(ids[0]).unwrap();
        assert_eq!(line.extensions.len(), 2);
        // The last segment is no string, and two extension lines stay.
        assert_eq!(p.floors[0].string_members(ids[0]).len(), 1);
        assert_eq!(
            p.floors[0].remove_extension(ids[0], 1),
            Err("A dimension line needs two extension lines")
        );
    }

    #[test]
    fn an_extension_line_moves_between_its_neighbours() {
        let (mut p, ids) = string();
        p.floors[0]
            .move_extension(ids[0], 2, Point::new(160.0, 50.0), None)
            .unwrap();
        let line = p.floors[0].dimension_line(ids[0]).unwrap();
        assert_eq!(line.extensions[1].point, Point::new(160.0, 0.0));
        let lens: Vec<f64> = line
            .segments
            .iter()
            .map(|i| {
                p.floors[0]
                    .dimensions
                    .iter()
                    .find(|d| d.id == *i)
                    .unwrap()
                    .length()
            })
            .collect();
        assert_eq!(lens, vec![160.0, 90.0, 150.0]);
        // Past a neighbour is refused and nothing moves.
        assert!(p.floors[0]
            .move_extension(ids[0], 2, Point::new(300.0, 0.0), None)
            .is_err());
        assert_eq!(
            p.floors[0].dimension_line(ids[0]).unwrap().extensions[1]
                .point
                .x,
            160.0
        );
        // The end extension line moves too.
        p.floors[0]
            .move_extension(ids[0], 4, Point::new(450.0, 0.0), None)
            .unwrap();
        assert_eq!(
            p.floors[0].dimension_line(ids[0]).unwrap().extensions[3]
                .point
                .x,
            450.0
        );
    }

    #[test]
    fn the_add_segments_handle_adds_a_segment_past_either_end() {
        let (mut p, ids) = string();
        let last = p
            .append_dimension_segment(0, ids[0], true, Point::new(520.0, 9.0), None)
            .unwrap();
        let first = p
            .append_dimension_segment(0, ids[0], false, Point::new(-60.0, 0.0), None)
            .unwrap();
        let line = p.floors[0].dimension_line(ids[1]).unwrap();
        assert_eq!(line.extensions.len(), 6);
        let xs: Vec<f64> = line.extensions.iter().map(|e| e.point.x).collect();
        assert_eq!(xs, vec![-60.0, 0.0, 100.0, 250.0, 400.0, 520.0]);
        assert!(line.segments.contains(&last) && line.segments.contains(&first));
        assert_eq!(p.floors[0].string_members(first).len(), 5);
        // Towards the line is not a new segment.
        assert!(p
            .append_dimension_segment(0, ids[0], true, Point::new(300.0, 0.0), None)
            .is_err());
    }

    #[test]
    fn what_an_extension_carries_is_written_to_both_segments() {
        let (mut p, ids) = string();
        let props = ExtProps {
            fixed_proximity: Some(30.0),
            marker: Some(ElevMark {
                text: "T.O. PLATE".into(),
                defaults: "Elevation".into(),
            }),
            ..ExtProps::default()
        };
        assert!(p.floors[0].set_extension(ids[1], 2, Some(false), Some(true), Some(props.clone())));
        let line = p.floors[0].dimension_line(ids[0]).unwrap();
        assert!(line.extensions[1].centerline);
        assert_eq!(line.extensions[1].props, props);
        // Both segments that meet there carry it.
        let a = p.floors[0]
            .dimensions
            .iter()
            .find(|d| d.id == ids[0])
            .unwrap();
        let b = p.floors[0]
            .dimensions
            .iter()
            .find(|d| d.id == ids[1])
            .unwrap();
        assert!(a.look.seg.centerline[1] && b.look.seg.centerline[0]);
        assert!(p.floors[0].line_is_fixed(ids[2]));
        assert!(!p.floors[0].set_extension(ids[1], 9, None, None, None));
    }

    #[test]
    fn old_runs_of_separate_dimensions_load_as_one_string_each() {
        let mut p = Project::new("t");
        let mut mk = |a: f64, b: f64, off: f64, y: f64| {
            p.add_dimension(
                0,
                Dimension::new(
                    0,
                    DimensionKind::Manual,
                    Point::new(a, y),
                    Point::new(b, y),
                    off,
                ),
            )
        };
        let run = [mk(0.0, 100.0, 24.0, 0.0), mk(100.0, 220.0, 24.0, 0.0)];
        let other_line = mk(220.0, 300.0, 48.0, 0.0);
        let lone = mk(0.0, 90.0, 24.0, 200.0);
        assert!(migrate_dimension_strings(&mut p));
        let f = &p.floors[0];
        assert_eq!(f.string_members(run[0]), run.to_vec());
        assert_eq!(f.string_members(other_line), vec![other_line]);
        assert_eq!(f.string_members(lone), vec![lone]);
        // Idempotent, and a segment taken out on purpose is left alone.
        assert!(!migrate_dimension_strings(&mut p));
        p.floors[0].leave_string(run[1]);
        p.floors[0]
            .dimensions
            .iter_mut()
            .for_each(|d| d.look.seg.separate = true);
        assert!(!migrate_dimension_strings(&mut p));
        assert_eq!(p.floors[0].string_members(run[0]), vec![run[0]]);
    }

    #[test]
    fn a_fixed_proximity_extension_keeps_the_line_at_its_distance_from_the_object() {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let id = p.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(0.0, 0.0),
                Point::new(240.0, 0.0),
                -30.0,
            ),
        );
        p.floors[0].attach_dimension_hinted(
            id,
            [
                Some(DimHint {
                    target: AnchorTarget::Wall,
                    id: w,
                    point: Point::new(0.0, 0.0),
                }),
                None,
            ],
        );
        assert!(p.floors[0].dimensions[0].anchors[0].is_some());
        let props = ExtProps {
            fixed_proximity: Some(30.0),
            ..ExtProps::default()
        };
        assert!(p.floors[0].set_extension(id, 1, None, None, Some(props)));
        assert!(!p.floors[0].enforce_fixed_proximity(), "already at 30 in");
        // The wall moves up 20 inches; the line follows it.
        let wm = p.floors[0].wall_mut(w).unwrap();
        wm.start = Point::new(0.0, 20.0);
        wm.end = Point::new(240.0, 20.0);
        p.floors[0].sync_dimension_anchors();
        assert!(p.floors[0].enforce_fixed_proximity());
        let d = &p.floors[0].dimensions[0];
        let (a, _) = d.line_points();
        assert!((a.y - (20.0 - 30.0)).abs() < 1e-6, "{a:?}");
        assert!(!p.floors[0].enforce_fixed_proximity(), "idempotent");
        let _ = DimAttach::Start;
    }

    #[test]
    fn a_wall_width_segment_hides_unless_the_line_displays_wall_widths() {
        let mut d = Dimension::new(
            0,
            DimensionKind::Manual,
            Point::ZERO,
            Point::new(6.0, 0.0),
            12.0,
        );
        let at = |side: f64| DimAnchor {
            wall: 9,
            target: AnchorTarget::Wall,
            at: DimAttach::Start,
            side,
            last: Point::ZERO,
            axis: Default::default(),
            extra: 0.0,
        };
        d.anchors = [Some(at(-3.0)), Some(at(3.0))];
        assert!(d.is_wall_width());
        assert!(d.segment_shows(), "wall widths show by default");
        d.look.wall_widths = Some(false);
        assert!(!d.segment_shows());
        // A segment between two different walls always shows.
        let mut other = at(3.0);
        other.wall = 10;
        d.anchors[1] = Some(other);
        assert!(d.segment_shows());
    }
}
