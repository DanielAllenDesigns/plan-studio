//! The output of the hidden-line pipeline: weighted 2D line segments.

use plan_3d::Material;
use plan_core::{CadItem, CadObject, Id, Point};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Pen weight of a line, lightest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LineWeight {
    /// Material and surface boundaries.
    Light,
    /// Feature (crease) edges.
    Medium,
    /// Silhouettes and section cut lines.
    Heavy,
}

impl LineWeight {
    /// Display name, also used as a layer suffix.
    pub fn name(self) -> &'static str {
        match self {
            LineWeight::Light => "Light",
            LineWeight::Medium => "Medium",
            LineWeight::Heavy => "Heavy",
        }
    }
}

/// Why an edge is drawn. Declared strongest first: when overlapping segments
/// are merged the strongest kind survives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    /// Intersection of a section plane with the model.
    Cut,
    /// Outline of the visible shape (also free mesh borders).
    Silhouette,
    /// Fold between two faces sharper than the crease angle.
    Crease,
    /// Seam between different materials or objects on the same surface.
    Material,
    /// A material hatch stroke inside a [`RegionKind::Face`] region.
    Hatch,
    /// Drafting annotation: grade line, pitch triangles ([`crate::annotate`]).
    Annotation,
    /// A hidden edge, kept only when dashed hidden lines are requested.
    Hidden,
}

/// One 2D segment in drawing space (inches of the building, Y up).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Line2 {
    pub a: Point,
    pub b: Point,
    pub weight: LineWeight,
    pub kind: EdgeKind,
}

impl Line2 {
    /// Segment length in drawing units.
    pub fn length(&self) -> f64 {
        self.a.dist(self.b)
    }

    /// Whether the segment is drawn dashed (hidden lines).
    pub fn is_dashed(&self) -> bool {
        self.kind == EdgeKind::Hidden
    }

    /// Layer suffix: the weight name, or `Hidden` for dashed lines.
    fn layer_suffix(&self) -> &'static str {
        if self.is_dashed() {
            "Hidden"
        } else {
            self.weight.name()
        }
    }

    /// Merge priority, 0 (hidden) to 4 (cut): a line hides collinear ones of lower tier.
    fn tier(&self) -> u8 {
        match self.kind {
            EdgeKind::Hidden => 0,
            EdgeKind::Cut => 4,
            _ => self.weight as u8 + 1,
        }
    }
}

/// What a [`Region`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RegionKind {
    /// A visible surface of one material (hatched when [`crate::Options::hatch`] is set).
    Face,
    /// A section cut face: fill solid or gray (poche).
    Cut,
    /// Cast shadow on a visible surface; `material` is the surface it falls on.
    Shadow,
}

/// A filled area of the drawing.
///
/// `polygon` is one closed ring in drawing space (no repeated first point).
/// Holes (window openings in a wall face...) are joined to the outer ring by
/// a zero-width slit, so the ring is weakly simple and fills correctly under
/// both the even-odd and non-zero rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub polygon: Vec<Point>,
    #[serde(with = "material_serde")]
    pub material: Material,
    /// The wall or opening the surface belongs to, if known.
    pub object_id: Option<Id>,
    pub kind: RegionKind,
}

impl Region {
    /// Absolute enclosed area, square drawing units (slits cancel out).
    pub fn area(&self) -> f64 {
        plan_core::geometry::polygon_area(&self.polygon).abs()
    }
}

/// Serialise [`Material`] by name (plan-3d's enum has no serde impls).
mod material_serde {
    use plan_3d::Material;
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(m: &Material, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(m.name())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Material, D::Error> {
        let name = String::deserialize(d)?;
        Material::ALL
            .iter()
            .copied()
            .find(|m| m.name() == name)
            .ok_or_else(|| D::Error::custom(format!("unknown material {name}")))
    }
}

/// A finished 2D drawing: line segments, filled regions and text, plus bounds.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Drawing {
    pub lines: Vec<Line2>,
    /// `(min, max)` corners of all line endpoints; zero for an empty drawing.
    pub bounds: (Point, Point),
    /// Visible-surface, section-cut and shadow areas (see [`Region`]).
    #[serde(default)]
    pub regions: Vec<Region>,
    /// Annotations from [`crate::annotate`]: `(anchor, text)`. The anchor is
    /// the left end of the text baseline, in drawing space.
    #[serde(default)]
    pub texts: Vec<(Point, String)>,
}

impl Drawing {
    /// Wrap `lines`, computing the bounds.
    pub fn new(lines: Vec<Line2>) -> Drawing {
        let mut d = Drawing {
            lines,
            bounds: (Point::ZERO, Point::ZERO),
            regions: Vec::new(),
            texts: Vec::new(),
        };
        d.update_bounds();
        d
    }

    /// Recompute [`Drawing::bounds`] from the current lines.
    pub fn update_bounds(&mut self) {
        let mut pts = self.lines.iter().flat_map(|l| [l.a, l.b]);
        self.bounds = match pts.next() {
            None => (Point::ZERO, Point::ZERO),
            Some(first) => pts.fold((first, first), |(lo, hi), p| {
                (
                    Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                    Point::new(hi.x.max(p.x), hi.y.max(p.y)),
                )
            }),
        };
    }

    /// The section cut faces (poche): regions of kind [`RegionKind::Cut`].
    pub fn cut_regions(&self) -> impl Iterator<Item = &Region> + '_ {
        self.regions_of(RegionKind::Cut)
    }

    /// Regions of one kind, in drawing order.
    pub fn regions_of(&self, kind: RegionKind) -> impl Iterator<Item = &Region> + '_ {
        self.regions.iter().filter(move |r| r.kind == kind)
    }

    /// Width and height of the bounds.
    pub fn size(&self) -> (f64, f64) {
        (
            self.bounds.1.x - self.bounds.0.x,
            self.bounds.1.y - self.bounds.0.y,
        )
    }

    /// Sum of all segment lengths.
    pub fn total_length(&self) -> f64 {
        self.lines.iter().map(Line2::length).sum()
    }

    /// Convert to CAD line objects on layers named `"{layer_prefix}, {Heavy|Medium|Light|Hidden}"`.
    ///
    /// Ids are sequential from 1; the caller re-keys them when adding to a project.
    pub fn to_cad(&self, layer_prefix: &str) -> Vec<CadObject> {
        self.lines
            .iter()
            .zip(1u64..)
            .map(|(l, id)| CadObject {
                id,
                layer: format!("{layer_prefix}, {}", l.layer_suffix()),
                item: CadItem::Line { a: l.a, b: l.b },
            })
            .collect()
    }

    /// Join collinear segments that touch or overlap into single segments.
    ///
    /// Segments merge only within the same tier (hidden < Light < Medium <
    /// Heavy < cut). A segment lying under a higher-tier collinear one is
    /// trimmed away, so no stroke is drawn twice.
    pub fn merge_collinear(&mut self) {
        self.lines = merge::merge_lines(&self.lines);
        self.update_bounds();
    }

    /// Bounds of lines, regions and text together (text width is estimated).
    fn extent(&self) -> (Point, Point) {
        let (mut lo, mut hi) = self.bounds;
        let mut grow = |p: Point| {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        };
        for r in &self.regions {
            r.polygon.iter().for_each(|&p| grow(p));
        }
        for (p, t) in &self.texts {
            grow(*p);
            grow(Point::new(
                p.x + TEXT_CHAR_W * t.chars().count() as f64,
                p.y + TEXT_H,
            ));
        }
        (lo, hi)
    }

    /// A small SVG rendering for debugging and tests, Y flipped.
    ///
    /// Paint order: face regions (white), section poche (gray), shadows
    /// (translucent gray), then one `<line>` per segment (hatch strokes
    /// included) and one `<text>` per annotation.
    pub fn svg(&self) -> String {
        let (lo, hi) = self.extent();
        let (w, h) = (hi.x - lo.x, hi.y - lo.y);
        let unit = w.max(h).max(1.0) / 800.0;
        let margin = 8.0 * unit;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{:.3} {:.3} {:.3} {:.3}\" width=\"{:.0}\" height=\"{:.0}\">",
            -margin,
            -margin,
            w + 2.0 * margin,
            h + 2.0 * margin,
            (w + 2.0 * margin) / unit,
            (h + 2.0 * margin) / unit,
        );
        for kind in [RegionKind::Face, RegionKind::Cut, RegionKind::Shadow] {
            let paint = match kind {
                RegionKind::Face => "fill=\"#ffffff\"",
                RegionKind::Cut => "fill=\"#8c8c8c\"",
                RegionKind::Shadow => "fill=\"#808080\" fill-opacity=\"0.35\"",
            };
            for r in self.regions_of(kind) {
                let mut d = String::new();
                for (i, p) in r.polygon.iter().enumerate() {
                    let _ = write!(
                        d,
                        "{}{:.3} {:.3} ",
                        if i == 0 { "M" } else { "L" },
                        p.x - lo.x,
                        hi.y - p.y
                    );
                }
                let _ = writeln!(
                    s,
                    "<path d=\"{d}Z\" {paint} fill-rule=\"evenodd\" stroke=\"none\"/>"
                );
            }
        }
        for l in &self.lines {
            let (stroke, width) = match l.weight {
                LineWeight::Heavy => ("#000000", 2.5),
                LineWeight::Medium => ("#333333", 1.4),
                LineWeight::Light => ("#777777", 0.8),
            };
            let dash = if l.is_dashed() {
                format!(" stroke-dasharray=\"{:.3} {:.3}\"", 6.0 * unit, 4.0 * unit)
            } else {
                String::new()
            };
            let _ = writeln!(
                s,
                "<line x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\" stroke=\"{stroke}\" stroke-width=\"{:.3}\"{dash}/>",
                l.a.x - lo.x,
                hi.y - l.a.y,
                l.b.x - lo.x,
                hi.y - l.b.y,
                width * unit,
            );
        }
        for (p, t) in &self.texts {
            let escaped = t
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let _ = writeln!(
                s,
                "<text x=\"{:.3}\" y=\"{:.3}\" font-family=\"sans-serif\" font-size=\"{:.3}\">{escaped}</text>",
                p.x - lo.x,
                hi.y - p.y,
                TEXT_H,
            );
        }
        s.push_str("</svg>\n");
        s
    }
}

/// Nominal text height of annotations, drawing units (inches of the building).
pub(crate) const TEXT_H: f64 = 8.0;
/// Estimated advance width of one character of annotation text.
pub(crate) const TEXT_CHAR_W: f64 = 0.6 * TEXT_H;

mod merge {
    //! Collinear merging: cluster by direction and offset, then merge intervals per tier.

    use super::{EdgeKind, Line2, LineWeight};
    use plan_core::Point;
    use std::f64::consts::FRAC_PI_2;

    /// Directions closer than this (radians) are parallel.
    const ANGLE_TOL: f64 = 1e-4;
    /// Parallel lines closer than this (inches) are the same line.
    const OFFSET_TOL: f64 = 0.02;
    /// Segments separated by less than this (inches) are joined.
    const JOIN_TOL: f64 = 0.05;
    /// Pieces shorter than this (inches) are dropped.
    const MIN_LEN: f64 = 0.05;

    /// A segment normalised so that `a` precedes `b` along a canonical direction.
    struct Seg {
        a: Point,
        b: Point,
        theta: f64,
        offset: f64,
        tier: u8,
        kind: EdgeKind,
    }

    /// An interval along a cluster's reference line.
    #[derive(Clone, Copy)]
    struct Iv {
        t0: f64,
        t1: f64,
        a: Point,
        b: Point,
        kind: EdgeKind,
    }

    fn normalise(l: &Line2) -> Option<Seg> {
        let len = l.length();
        if len <= 1e-9 {
            return None;
        }
        let (mut a, mut b) = (l.a, l.b);
        let mut theta = b.sub(a).angle();
        if theta <= -FRAC_PI_2 || theta > FRAC_PI_2 {
            std::mem::swap(&mut a, &mut b);
            theta = b.sub(a).angle();
        }
        // Keep near-vertical lines on one side of the angle wrap.
        if theta < -FRAC_PI_2 + ANGLE_TOL {
            std::mem::swap(&mut a, &mut b);
            theta = b.sub(a).angle();
        }
        let d = b.sub(a).normalized();
        Some(Seg {
            a,
            b,
            theta,
            offset: d.cross(a),
            tier: l.tier(),
            kind: l.kind,
        })
    }

    /// Split a sorted list into runs where consecutive keys differ by at most `tol`.
    fn chains<T>(items: Vec<T>, key: impl Fn(&T) -> f64, tol: f64) -> Vec<Vec<T>> {
        let mut out: Vec<Vec<T>> = Vec::new();
        let mut last = f64::NEG_INFINITY;
        for item in items {
            let k = key(&item);
            if out.is_empty() || k - last > tol {
                out.push(Vec::new());
            }
            last = k;
            if let Some(chain) = out.last_mut() {
                chain.push(item);
            }
        }
        out
    }

    pub fn merge_lines(lines: &[Line2]) -> Vec<Line2> {
        let mut segs: Vec<Seg> = lines.iter().filter_map(normalise).collect();
        segs.sort_by(|p, q| p.theta.total_cmp(&q.theta));
        let mut out = Vec::new();
        for mut by_angle in chains(segs, |s| s.theta, ANGLE_TOL) {
            by_angle.sort_by(|p, q| p.offset.total_cmp(&q.offset));
            for cluster in chains(by_angle, |s| s.offset, OFFSET_TOL) {
                merge_cluster(&cluster, &mut out);
            }
        }
        out
    }

    /// Union of sorted intervals, joining those closer than `JOIN_TOL`.
    fn union(mut ivs: Vec<Iv>) -> Vec<Iv> {
        ivs.sort_by(|p, q| p.t0.total_cmp(&q.t0));
        let mut out: Vec<Iv> = Vec::new();
        for iv in ivs {
            match out.last_mut() {
                Some(cur) if iv.t0 <= cur.t1 + JOIN_TOL => {
                    if iv.t1 > cur.t1 {
                        cur.t1 = iv.t1;
                        cur.b = iv.b;
                    }
                    cur.kind = cur.kind.min(iv.kind);
                }
                _ => out.push(iv),
            }
        }
        out
    }

    fn merge_cluster(cluster: &[Seg], out: &mut Vec<Line2>) {
        let origin = cluster[0].a;
        let dir = cluster[0].b.sub(origin).normalized();
        let along = |p: Point| dir.dot(p.sub(origin));
        let at = |t: f64| origin.add(dir.scale(t));
        let mut covered: Vec<Iv> = Vec::new();
        for tier in (0..=4u8).rev() {
            let ivs: Vec<Iv> = cluster
                .iter()
                .filter(|s| s.tier == tier)
                .map(|s| {
                    let (ta, tb) = (along(s.a), along(s.b));
                    if ta <= tb {
                        Iv {
                            t0: ta,
                            t1: tb,
                            a: s.a,
                            b: s.b,
                            kind: s.kind,
                        }
                    } else {
                        Iv {
                            t0: tb,
                            t1: ta,
                            a: s.b,
                            b: s.a,
                            kind: s.kind,
                        }
                    }
                })
                .collect();
            let merged = union(ivs);
            for iv in &merged {
                let mut cursor = iv.t0;
                let mut start = iv.a;
                let mut pieces: Vec<(Point, Point, f64, f64)> = Vec::new();
                for c in &covered {
                    if c.t1 <= cursor || c.t0 >= iv.t1 {
                        continue;
                    }
                    if c.t0 > cursor {
                        pieces.push((start, at(c.t0), cursor, c.t0));
                    }
                    cursor = cursor.max(c.t1);
                    start = at(cursor);
                }
                if cursor < iv.t1 {
                    pieces.push((start, iv.b, cursor, iv.t1));
                }
                for (a, b, t0, t1) in pieces {
                    if t1 - t0 >= MIN_LEN {
                        out.push(Line2 {
                            a,
                            b,
                            weight: weight_of(tier),
                            kind: iv.kind,
                        });
                    }
                }
            }
            covered = union(covered.into_iter().chain(merged).collect());
        }
    }

    fn weight_of(tier: u8) -> LineWeight {
        match tier {
            3 | 4 => LineWeight::Heavy,
            2 => LineWeight::Medium,
            _ => LineWeight::Light,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(ax: f64, ay: f64, bx: f64, by: f64, weight: LineWeight) -> Line2 {
        Line2 {
            a: Point::new(ax, ay),
            b: Point::new(bx, by),
            weight,
            kind: EdgeKind::Silhouette,
        }
    }

    #[test]
    fn three_collinear_touching_segments_become_one() {
        let mut d = Drawing::new(vec![
            line(20.0, 0.0, 30.0, 0.0, LineWeight::Heavy),
            line(0.0, 0.0, 10.0, 0.0, LineWeight::Heavy),
            line(10.0, 0.0, 20.0, 0.0, LineWeight::Heavy),
        ]);
        d.merge_collinear();
        assert_eq!(d.lines.len(), 1);
        let l = d.lines[0];
        assert!((l.length() - 30.0).abs() < 1e-9);
        assert_eq!(d.bounds, (Point::new(0.0, 0.0), Point::new(30.0, 0.0)));
    }

    #[test]
    fn vertical_segments_in_either_direction_merge() {
        let mut d = Drawing::new(vec![
            line(5.0, 10.0, 5.0, 0.0, LineWeight::Medium),
            line(5.0, 10.0, 5.0, 20.0, LineWeight::Medium),
        ]);
        d.merge_collinear();
        assert_eq!(d.lines.len(), 1);
        assert!((d.lines[0].length() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn non_collinear_and_separated_segments_stay_apart() {
        let mut d = Drawing::new(vec![
            line(0.0, 0.0, 10.0, 0.0, LineWeight::Heavy),
            line(11.0, 0.0, 20.0, 0.0, LineWeight::Heavy),
            line(0.0, 1.0, 10.0, 1.0, LineWeight::Heavy),
        ]);
        d.merge_collinear();
        assert_eq!(d.lines.len(), 3);
    }

    #[test]
    fn lighter_line_under_a_heavier_one_is_trimmed() {
        let mut d = Drawing::new(vec![
            line(0.0, 0.0, 30.0, 0.0, LineWeight::Light),
            line(10.0, 0.0, 20.0, 0.0, LineWeight::Heavy),
        ]);
        d.merge_collinear();
        assert_eq!(d.lines.len(), 3);
        let heavy: f64 = d
            .lines
            .iter()
            .filter(|l| l.weight == LineWeight::Heavy)
            .map(Line2::length)
            .sum();
        assert!((heavy - 10.0).abs() < 1e-9);
        assert!((d.total_length() - 30.0).abs() < 1e-9);
    }

    #[test]
    fn svg_has_one_line_element_per_segment() {
        let d = Drawing::new(vec![
            line(0.0, 0.0, 10.0, 0.0, LineWeight::Heavy),
            line(0.0, 0.0, 0.0, 5.0, LineWeight::Light),
        ]);
        let svg = d.svg();
        assert!(svg.starts_with("<svg"));
        assert_eq!(svg.matches("<line").count(), 2);
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn to_cad_emits_lines_on_weight_layers() {
        let d = Drawing::new(vec![line(0.0, 0.0, 10.0, 0.0, LineWeight::Medium)]);
        let cad = d.to_cad("Elev Front");
        assert_eq!(cad.len(), 1);
        assert_eq!(cad[0].layer, "Elev Front, Medium");
        assert!(matches!(cad[0].item, CadItem::Line { .. }));
    }
}
