//! What one dimension segment keeps beyond its two measured points: the
//! string it belongs to, additional text, a moved or turned label, its
//! leader line, centerline marks, a grid-rounded value, rich label text and
//! the curved forms (radius, arc length and angle dimensions). Also the
//! plan-wide helpers that act on strings of segments.
//!
//! In Chief a dimension line is one object with numbered extension lines and
//! one label per segment. Here every segment is a [`Dimension`]; the segments
//! of one string share a [`DimSeg::string`] id (the id of the first), the
//! editor selects, moves, copies and deletes them together, and the
//! Dimension Specification's Segments panel edits them in one table.

use super::label::{angle_text, grid_round, round_to_step, step_inches, LabelParts};
use super::{DimFormat, Dimension};
use crate::geometry::Point;
use crate::model::{Floor, Id, Wall};
use crate::text_styles::RichRun;
use serde::{Deserialize, Serialize};

/// The line that joins an offset label to its segment (Leader Lines, manual
/// p. 503).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LeaderStyle {
    None,
    #[default]
    SquareCorner,
    RoundCorner,
    Diagonal,
}

impl LeaderStyle {
    pub const ALL: [LeaderStyle; 4] = [
        LeaderStyle::None,
        LeaderStyle::SquareCorner,
        LeaderStyle::RoundCorner,
        LeaderStyle::Diagonal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LeaderStyle::None => "None",
            LeaderStyle::SquareCorner => "Square Corner",
            LeaderStyle::RoundCorner => "Round Corner",
            LeaderStyle::Diagonal => "Diagonal",
        }
    }

    pub fn from_name(name: &str) -> LeaderStyle {
        LeaderStyle::ALL
            .into_iter()
            .find(|s| s.label().eq_ignore_ascii_case(name.trim()))
            .unwrap_or_default()
    }
}

/// What a curved dimension measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveKind {
    /// The radius of an arc: from its center to the arc.
    Radius,
    /// The length along an arc.
    ArcLength,
    /// The angle between two edges, drawn as an arc about their corner.
    Angle,
}

/// A curved dimension's geometry. The dimension's `start` and `end` are the
/// points the arc's measured ends (or, for a radius, the center and the point
/// on the arc) are at, so handles and picking keep working.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimCurve {
    pub kind: CurveKind,
    pub center: Point,
    /// The radius of the measured arc (for an angle: of the drawn arc).
    pub radius: f64,
    /// Where the arc starts, radians counter-clockwise from east.
    pub start: f64,
    /// Signed sweep, radians (negative: clockwise).
    pub sweep: f64,
    /// The walls measured. An angle is between `walls[0]` and `walls[1]`;
    /// the other kinds measure `walls[0]`, a curved wall.
    pub walls: [Option<Id>; 2],
    /// Angle: whether the arm runs toward the end of the wall (otherwise
    /// toward its start).
    pub toward_end: [bool; 2],
    /// Arc kinds: how far to the left of the wall's centerline the measured
    /// arc lies (the located surface).
    pub lateral: f64,
}

impl DimCurve {
    /// The measured value: the radius or the arc length in inches, the angle
    /// in degrees.
    pub fn value(&self) -> f64 {
        match self.kind {
            CurveKind::Radius => self.radius,
            CurveKind::ArcLength => self.radius * self.sweep.abs(),
            CurveKind::Angle => self.sweep.abs().to_degrees(),
        }
    }

    /// The point at angle `a` (radians) on the measured arc.
    pub fn point_at(&self, a: f64) -> Point {
        Point::new(
            self.center.x + self.radius * a.cos(),
            self.center.y + self.radius * a.sin(),
        )
    }

    /// The arc's end angles `(start, end)`.
    pub fn angles(&self) -> (f64, f64) {
        (self.start, self.start + self.sweep)
    }

    /// The arc's middle point.
    pub fn mid_point(&self) -> Point {
        self.point_at(self.start + self.sweep * 0.5)
    }

    /// The arc as `n + 1` points, drawn at `radius_add` more than the measured
    /// radius.
    pub fn sample(&self, radius_add: f64, n: usize) -> Vec<Point> {
        let n = n.max(1);
        let r = (self.radius + radius_add).max(0.0);
        (0..=n)
            .map(|i| {
                let a = self.start + self.sweep * i as f64 / n as f64;
                Point::new(self.center.x + r * a.cos(), self.center.y + r * a.sin())
            })
            .collect()
    }

    /// Does the point lie on the drawn arc (within `tol`)? `radius_add` is the
    /// dimension arc's offset from the measured one.
    pub fn near(&self, p: Point, radius_add: f64, tol: f64) -> bool {
        let r = (self.radius + radius_add).max(0.0);
        if (p.dist(self.center) - r).abs() > tol {
            return false;
        }
        let slack = tol / r.max(1e-6);
        let rel = (p.sub(self.center).angle() - self.start).rem_euclid(std::f64::consts::TAU);
        if self.sweep >= 0.0 {
            rel <= self.sweep + slack || rel >= std::f64::consts::TAU - slack
        } else {
            rel >= std::f64::consts::TAU + self.sweep - slack || rel <= slack
        }
    }
}

/// The per-segment settings of a [`Dimension`] (a field of its
/// [`super::DimOverrides`]); every field starts at "none of this".
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DimSeg {
    /// The string this segment is part of: the id of its first segment.
    pub string: Option<Id>,
    /// Text before and after the number (Additional Text).
    pub leading: String,
    pub trailing: String,
    /// Leave the number out and keep only the additional text.
    pub suppress_value: bool,
    /// No label at all.
    pub blank: bool,
    /// The label's center moved off its default spot, as `(along, across)`
    /// plan inches from the middle of the dimension line in the line's own
    /// frame.
    pub label_move: Option<Point>,
    /// The label turned to this angle (degrees); `None` is automatic.
    pub label_angle: Option<f64>,
    /// This segment's leader line (`None` follows the Dimension Defaults).
    pub leader: Option<LeaderStyle>,
    pub leader_second_segment: Option<bool>,
    /// The two extension lines carry a Centerline mark.
    pub centerline: [bool; 2],
    /// The grid-rounded length shown instead of the true one.
    pub shown: Option<f64>,
    /// Rich label text (bold, italic, size, colour per run); when set it
    /// replaces the number.
    pub runs: Vec<RichRun>,
    pub curve: Option<DimCurve>,
}

impl DimSeg {
    pub fn is_default(&self) -> bool {
        *self == DimSeg::default()
    }

    /// The text of the rich label, if it has one.
    pub fn rich_text(&self) -> Option<String> {
        (!self.runs.is_empty()).then(|| self.runs.iter().map(|r| r.text.as_str()).collect())
    }
}

impl Dimension {
    /// The measured curve, if this is a radius, arc length or angle.
    pub fn curve(&self) -> Option<&DimCurve> {
        self.look.seg.curve.as_ref()
    }

    /// The string this segment belongs to.
    pub fn string_id(&self) -> Option<Id> {
        self.look.seg.string
    }

    /// The measured value before formatting: the distance between the
    /// measured points in inches, or a curved dimension's radius, arc length
    /// (inches) or angle (degrees).
    pub fn measure(&self) -> f64 {
        match self.curve() {
            Some(c) => c.value(),
            None => self.length(),
        }
    }

    /// The label as its lines: the primary number and the second format's.
    pub fn label_parts(&self, fmt: &DimFormat) -> LabelParts {
        let seg = &self.look.seg;
        if seg.blank {
            return LabelParts::default();
        }
        let (primary, second) = self.core_text(fmt);
        let rich = seg.rich_text();
        let overridden = rich.is_some() || self.text_override.is_some();
        let text = match (rich, &self.text_override) {
            (Some(r), _) => r,
            (None, Some(t)) => t.clone(),
            (None, None) => {
                let body = if seg.suppress_value {
                    String::new()
                } else {
                    primary
                };
                format!("{}{}{}", seg.leading, body, seg.trailing)
            }
        };
        LabelParts {
            primary: text,
            second: second.filter(|_| !overridden && !seg.suppress_value),
        }
    }

    /// The number and the second format's, before additional text.
    fn core_text(&self, fmt: &DimFormat) -> (String, Option<String>) {
        let opts = self.look.label_options(fmt);
        let Some(c) = self.curve() else {
            let len = self.length();
            let shown = self.look.seg.shown.unwrap_or(len);
            let lf = self.look.effective_format(fmt);
            let mut text = self.look.format_len(fmt, shown);
            let step = step_inches(&lf);
            let (pre, post) = super::label::indicators(
                len,
                self.look.seg.shown.unwrap_or_else(|| round_to_step(len, step)),
                step,
                opts.plus_minus_after,
                opts.tilde_before,
            );
            text = opts.tolerance.apply(&format!("{pre}{text}{post}"), len, &lf);
            let second = opts
                .second
                .include
                .then(|| crate::units::format_length(len, &opts.second.format));
            return (text, second);
        };
        match c.kind {
            CurveKind::Angle => (
                angle_text(c.value(), opts.angle_decimals, opts.angle_dms),
                None,
            ),
            CurveKind::Radius | CurveKind::ArcLength => {
                let v = c.value();
                let lf = self.look.effective_format(fmt);
                let mut text = self.look.format_len(fmt, v);
                if c.kind == CurveKind::Radius {
                    text = format!("R {text}");
                }
                text = opts.tolerance.apply(&text, v, &lf);
                let second = opts
                    .second
                    .include
                    .then(|| crate::units::format_length(v, &opts.second.format));
                (text, second)
            }
        }
    }

    /// A curved dimension from its curve. `offset` is the radial distance of
    /// the dimension line from the measured arc (arc length), or ignored.
    pub fn curved(kind: super::DimensionKind, curve: DimCurve, offset: f64) -> Dimension {
        let (a0, a1) = curve.angles();
        let (start, end) = match curve.kind {
            CurveKind::Radius => (curve.center, curve.point_at(a0 + curve.sweep * 0.5)),
            _ => (curve.point_at(a0), curve.point_at(a1)),
        };
        let mut d = Dimension::new(0, kind, start, end, offset);
        d.look.seg.curve = Some(curve);
        d
    }
}

impl Floor {
    /// The ids of every segment of the string that `id` belongs to, in
    /// drawing order (just `[id]` when it is a lone segment).
    pub fn string_members(&self, id: Id) -> Vec<Id> {
        let Some(d) = self.dimensions.iter().find(|d| d.id == id) else {
            return Vec::new();
        };
        match d.string_id() {
            Some(s) => self
                .dimensions
                .iter()
                .filter(|x| x.string_id() == Some(s))
                .map(|x| x.id)
                .collect(),
            None => vec![id],
        }
    }

    /// Joins `ids` into one string (Chief's dimension line with several
    /// segments). Returns the string id, or `None` for fewer than two.
    pub fn join_string(&mut self, ids: &[Id]) -> Option<Id> {
        if ids.len() < 2 {
            return None;
        }
        let head = ids[0];
        for d in &mut self.dimensions {
            if ids.contains(&d.id) {
                d.look.seg.string = Some(head);
            }
        }
        Some(head)
    }

    /// Takes one segment out of its string. A string left with one segment
    /// stops being one.
    pub fn leave_string(&mut self, id: Id) {
        let Some(s) = self
            .dimensions
            .iter()
            .find(|d| d.id == id)
            .and_then(Dimension::string_id)
        else {
            return;
        };
        if let Some(d) = self.dimensions.iter_mut().find(|d| d.id == id) {
            d.look.seg.string = None;
        }
        let rest: Vec<Id> = self
            .dimensions
            .iter()
            .filter(|d| d.string_id() == Some(s))
            .map(|d| d.id)
            .collect();
        if rest.len() == 1 {
            if let Some(d) = self.dimensions.iter_mut().find(|d| d.id == rest[0]) {
                d.look.seg.string = None;
            }
        } else if let Some(first) = rest.first().copied().filter(|f| *f != s) {
            // The head left: the next segment names the string.
            for d in &mut self.dimensions {
                if d.string_id() == Some(s) {
                    d.look.seg.string = Some(first);
                }
            }
        }
    }

    /// Chains of dimensions that continue each other on one dimension line
    /// (each starts where the previous ended): the runs Grid Rounding keeps
    /// adding up. Returns indices into `dimensions`.
    pub fn dimension_chains(&self) -> Vec<Vec<usize>> {
        let ds = &self.dimensions;
        let continues = |a: &Dimension, b: &Dimension| {
            if a.curve().is_some() || b.curve().is_some() {
                return false;
            }
            if a.end.dist(b.start) > 0.01 || a.length() < 1e-9 || b.length() < 1e-9 {
                return false;
            }
            let (ua, ub) = (
                a.end.sub(a.start).normalized(),
                b.end.sub(b.start).normalized(),
            );
            if ua.dot(ub) < 1.0 - 1e-6 {
                return false;
            }
            let off_a = ua.perp().scale(a.offset);
            let off_b = ub.perp().scale(b.offset);
            off_a.dist(off_b) < 0.01
        };
        let next: Vec<Option<usize>> = (0..ds.len())
            .map(|i| (0..ds.len()).find(|&j| j != i && continues(&ds[i], &ds[j])))
            .collect();
        let mut has_prev = vec![false; ds.len()];
        for n in next.iter().flatten() {
            has_prev[*n] = true;
        }
        let mut chains = Vec::new();
        for head in (0..ds.len()).filter(|&i| !has_prev[i]) {
            let mut chain = vec![head];
            let mut cur = head;
            while let Some(n) = next[cur] {
                if chain.contains(&n) {
                    break;
                }
                chain.push(n);
                cur = n;
            }
            if chain.len() > 1 {
                chains.push(chain);
            }
        }
        chains
    }

    /// Applies the rounding method to the strings of the floor: with Grid
    /// Rounding every chain of segments gets shown lengths that add up to the
    /// whole; with Distance Rounding they are cleared. Returns whether any
    /// dimension changed.
    pub fn regrid_dimensions(&mut self, fmt: &DimFormat) -> bool {
        let mut shown: Vec<Option<f64>> = vec![None; self.dimensions.len()];
        if fmt.label.rounding == super::RoundMethod::Grid {
            for chain in self.dimension_chains() {
                let first = &self.dimensions[chain[0]];
                if first.text_override.is_some() {
                    continue;
                }
                let step = step_inches(&first.look.effective_format(fmt));
                let lens: Vec<f64> = chain.iter().map(|&i| self.dimensions[i].length()).collect();
                let rounded = grid_round(&lens, step);
                for (k, &i) in chain.iter().enumerate() {
                    let plain = round_to_step(lens[k], step);
                    if (rounded[k] - plain).abs() > 1e-9 {
                        shown[i] = Some(rounded[k]);
                    }
                }
            }
        }
        let mut changed = false;
        for (d, s) in self.dimensions.iter_mut().zip(shown) {
            if d.look.seg.shown != s {
                d.look.seg.shown = s;
                changed = true;
            }
        }
        changed
    }

    /// Brings the curved dimensions that follow walls up to date: an angle
    /// between two walls turns with them, a radius or arc length follows its
    /// curved wall. A wall that is gone, or one that is no longer curved,
    /// lets the dimension go (it keeps its last geometry). Returns whether
    /// anything changed.
    pub fn sync_dimension_curves(&mut self) -> bool {
        let walls = self.walls.clone();
        let mut changed = false;
        for d in &mut self.dimensions {
            let Some(c) = d.look.seg.curve else { continue };
            if c.walls.iter().all(Option::is_none) {
                continue;
            }
            let find = |id: Option<Id>| id.and_then(|i| walls.iter().find(|w| w.id == i));
            let new = match c.kind {
                CurveKind::Angle => angle_from_walls(&c, find(c.walls[0]), find(c.walls[1])),
                _ => arc_from_wall(&c, find(c.walls[0])),
            };
            match new {
                Some(n) if n != c => {
                    let kind = d.kind;
                    let id = d.id;
                    let offset = d.offset;
                    let was = d.look.clone();
                    let mut nd = Dimension::curved(kind, n, offset);
                    nd.id = id;
                    nd.look = was;
                    nd.look.seg.curve = Some(n);
                    nd.text_override = d.text_override.clone();
                    nd.text_style = d.text_style.clone();
                    nd.auto_group = d.auto_group;
                    nd.hide_ext = d.hide_ext;
                    *d = nd;
                    changed = true;
                }
                Some(_) => {}
                None => {
                    // The wall is gone or no longer curved: the dimension
                    // keeps what it measured.
                    if let Some(c) = d.look.seg.curve.as_mut() {
                        c.walls = [None, None];
                    }
                    changed = true;
                }
            }
        }
        changed
    }
}

/// The angle curve between two walls' lines, keeping the arm directions and
/// the arc radius.
fn angle_from_walls(c: &DimCurve, a: Option<&Wall>, b: Option<&Wall>) -> Option<DimCurve> {
    let (a, b) = (a?, b?);
    let dir = |w: &Wall, toward_end: bool| {
        let d = w.end.sub(w.start).normalized();
        if toward_end {
            d
        } else {
            d.scale(-1.0)
        }
    };
    let (da, db) = (dir(a, c.toward_end[0]), dir(b, c.toward_end[1]));
    let denom = da.cross(db);
    if denom.abs() < 1e-9 {
        return None;
    }
    // Intersection of the two infinite lines.
    let t = b.start.sub(a.start).cross(db) / a.end.sub(a.start).normalized().cross(db);
    let vertex = a.start.add(a.end.sub(a.start).normalized().scale(t));
    let start = da.angle();
    let mut sweep = (db.angle() - start).rem_euclid(std::f64::consts::TAU);
    if sweep > std::f64::consts::PI {
        sweep -= std::f64::consts::TAU;
    }
    Some(DimCurve {
        center: vertex,
        start,
        sweep,
        ..*c
    })
}

/// The radius or arc length curve of a curved wall at the located surface.
fn arc_from_wall(c: &DimCurve, w: Option<&Wall>) -> Option<DimCurve> {
    let w = w?;
    let (center, r) = w.arc_center_radius()?;
    let curve = w.curve?;
    let sweep = curve.sweep(w.start, w.end);
    let start = w.start.sub(center).angle();
    // The surface `lateral` to the left of the centerline: the radius there.
    let at = w.point_offset(w.path_length() * 0.5, c.lateral);
    let radius = center.dist(at).max(1e-6);
    let _ = r;
    Some(DimCurve {
        center,
        radius,
        start,
        sweep,
        ..*c
    })
}

#[cfg(test)]
mod tests {
    use super::super::{DimensionKind, RoundMethod};
    use super::*;
    use crate::model::{Project, WallKind};
    use crate::units::LengthFormat;

    fn dim(a: f64, b: f64, off: f64) -> Dimension {
        Dimension::new(
            0,
            DimensionKind::Manual,
            Point::new(a, 0.0),
            Point::new(b, 0.0),
            off,
        )
    }

    #[test]
    fn additional_text_wraps_the_number_and_blank_hides_the_label() {
        let fmt = DimFormat::default();
        let mut d = dim(0.0, 120.0, 24.0);
        assert_eq!(d.label_parts(&fmt).primary, "10'-0\"");
        d.look.seg.leading = "(".into();
        d.look.seg.trailing = ") TYP".into();
        assert_eq!(d.label(&fmt), "(10'-0\") TYP");
        d.look.seg.suppress_value = true;
        assert_eq!(d.label(&fmt), "() TYP");
        d.look.seg.blank = true;
        assert_eq!(d.label(&fmt), "");
    }

    #[test]
    fn a_text_override_replaces_the_whole_label() {
        let fmt = DimFormat::default();
        let mut d = dim(0.0, 120.0, 24.0);
        d.text_override = Some("EQ".into());
        d.look.seg.leading = "x".into();
        assert_eq!(d.label(&fmt), "EQ");
    }

    #[test]
    fn rich_runs_replace_the_number_and_keep_their_text() {
        let fmt = DimFormat::default();
        let mut d = dim(0.0, 120.0, 24.0);
        d.look.seg.runs = vec![
            RichRun {
                text: "CLR ".into(),
                bold: true,
                ..RichRun::default()
            },
            RichRun {
                text: "10'".into(),
                ..RichRun::default()
            },
        ];
        assert_eq!(d.label(&fmt), "CLR 10'");
    }

    #[test]
    fn the_second_format_adds_a_line_and_a_parenthesized_number() {
        let mut fmt = DimFormat::default();
        fmt.label.second.include = true;
        let d = dim(0.0, 120.0, 24.0);
        let parts = d.label_parts(&fmt);
        assert_eq!(parts.primary, "10'-0\"");
        assert_eq!(parts.second.as_deref(), Some("3048 mm"));
        assert_eq!(d.label(&fmt), "10'-0\" (3048 mm)");
        // A text override has no second number.
        let mut o = d.clone();
        o.text_override = Some("EQ".into());
        assert_eq!(o.label_parts(&fmt).second, None);
    }

    #[test]
    fn tolerance_and_rounded_value_indicators_show_in_the_label() {
        let mut fmt = DimFormat {
            smallest_fraction: 8,
            ..DimFormat::default()
        };
        fmt.length = Some(LengthFormat {
            fraction_denominator: 8,
            ..LengthFormat::default()
        });
        fmt.label.plus_minus_after = true;
        fmt.label.tilde_before = true;
        // 120 and 1/32 inch is shown as 10'-0" at eighths: the truth is higher.
        let d = Dimension::new(
            0,
            DimensionKind::Manual,
            Point::ZERO,
            Point::new(120.0 + 1.0 / 32.0, 0.0),
            12.0,
        );
        assert_eq!(d.label(&fmt), "~10'-0\"+");
        let exact = dim(0.0, 120.0, 12.0);
        assert_eq!(exact.label(&fmt), "10'-0\"");
        fmt.label.tolerance.mode = super::super::TolMode::Symmetric;
        fmt.label.tolerance.plus = 0.125;
        assert!(exact.label(&fmt).ends_with("\u{b1}0'-0 1/8\""), "{}", exact.label(&fmt));
    }

    #[test]
    fn grid_rounding_keeps_a_string_adding_up() {
        let mut f = Floor::new("t", 0.0);
        // Three segments of 4.4" on a 1" grid: 4, 5, 4 instead of 4, 4, 4.
        let mut fmt = DimFormat {
            length: Some(LengthFormat {
                unit: crate::units::LengthUnit::Inches,
                fraction_denominator: 1,
                ..LengthFormat::default()
            }),
            ..DimFormat::default()
        };
        for i in 0..3u32 {
            let a = 4.4 * f64::from(i);
            f.dimensions.push(Dimension {
                id: u64::from(i) + 1,
                ..dim(a, a + 4.4, 12.0)
            });
        }
        assert!(f.regrid_dimensions(&fmt));
        let shown: Vec<Option<f64>> = f.dimensions.iter().map(|d| d.look.seg.shown).collect();
        assert_eq!(shown, vec![None, Some(5.0), None]);
        assert_eq!(f.dimensions[1].label(&fmt), "5\"");
        assert_eq!(f.dimensions[0].label(&fmt), "4\"");
        // Distance Rounding takes it back.
        fmt.label.rounding = RoundMethod::Distance;
        assert!(f.regrid_dimensions(&fmt));
        assert!(f.dimensions.iter().all(|d| d.look.seg.shown.is_none()));
        assert_eq!(f.dimensions[1].label(&fmt), "4\"");
    }

    #[test]
    fn strings_join_and_leave() {
        let mut f = Floor::new("t", 0.0);
        for i in 1..=3u64 {
            let a = 100.0 * i as f64;
            f.dimensions.push(Dimension {
                id: i,
                ..dim(a, a + 100.0, 12.0)
            });
        }
        assert_eq!(f.string_members(2), vec![2]);
        assert_eq!(f.join_string(&[1, 2, 3]), Some(1));
        assert_eq!(f.string_members(3), vec![1, 2, 3]);
        f.leave_string(1);
        assert_eq!(f.string_members(2), vec![2, 3], "the next names the string");
        f.leave_string(2);
        assert_eq!(f.string_members(3), vec![3], "a lone segment is no string");
        assert_eq!(f.join_string(&[1]), None);
    }

    #[test]
    fn an_angle_between_two_walls_turns_with_them() {
        let mut p = Project::new("t");
        let a = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let b = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(0.0, 120.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let curve = DimCurve {
            kind: CurveKind::Angle,
            center: Point::ZERO,
            radius: 36.0,
            start: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            walls: [Some(a), Some(b)],
            toward_end: [true, true],
            lateral: 0.0,
        };
        let id = p.add_dimension(0, Dimension::curved(DimensionKind::Manual, curve, 0.0));
        let fmt = DimFormat::default();
        assert_eq!(p.floors[0].dimensions[0].label(&fmt), "90.0\u{b0}");
        // Turn the second wall to 45 degrees.
        let w = p.floors[0].wall_mut(b).unwrap();
        w.end = Point::new(85.0, 85.0);
        assert!(p.floors[0].sync_dimension_curves());
        let d = p.floors[0].dimensions.iter().find(|d| d.id == id).unwrap();
        assert_eq!(d.label(&fmt), "45.0\u{b0}");
        // Remove the wall: the dimension keeps what it measured.
        p.floors[0].walls.retain(|w| w.id != b);
        assert!(p.floors[0].sync_dimension_curves());
        let d = p.floors[0].dimensions.iter().find(|d| d.id == id).unwrap();
        assert_eq!(d.label(&fmt), "45.0\u{b0}");
        assert!(d.curve().unwrap().walls.iter().all(Option::is_none));
    }

    #[test]
    fn radius_and_arc_length_follow_a_curved_wall() {
        let mut p = Project::new("t");
        let id = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        p.floors[0].wall_mut(id).unwrap().curve = Some(crate::walls::WallCurve { bulge: 60.0 });
        let w = p.floors[0].wall(id).unwrap().clone();
        let (center, r) = w.arc_center_radius().unwrap();
        let sweep = w.curve.unwrap().sweep(w.start, w.end);
        let base = DimCurve {
            kind: CurveKind::ArcLength,
            center,
            radius: r,
            start: w.start.sub(center).angle(),
            sweep,
            walls: [Some(id), None],
            toward_end: [true; 2],
            lateral: 0.0,
        };
        let fmt = DimFormat::default();
        let arc = Dimension::curved(DimensionKind::Manual, base, 12.0);
        assert!(
            (arc.measure() - w.path_length()).abs() < 1e-6,
            "{} vs {}",
            arc.measure(),
            w.path_length()
        );
        let rad = Dimension::curved(
            DimensionKind::Manual,
            DimCurve {
                kind: CurveKind::Radius,
                ..base
            },
            0.0,
        );
        assert!(rad.label(&fmt).starts_with("R "), "{}", rad.label(&fmt));
        // Bend the wall further: a sync follows it.
        let a = p.add_dimension(0, arc);
        p.floors[0].wall_mut(id).unwrap().curve = Some(crate::walls::WallCurve { bulge: 90.0 });
        assert!(p.floors[0].sync_dimension_curves());
        let w2 = p.floors[0].wall(id).unwrap().clone();
        let d = p.floors[0].dimensions.iter().find(|d| d.id == a).unwrap();
        assert!((d.measure() - w2.path_length()).abs() < 1e-6);
        assert!(d.measure() > w.path_length());
    }
}
