//! Plot Lines: the edge and pattern lines of a cross section, elevation or
//! camera view sent to layout as a semi-dynamic Vector View (manual pp. 1403
//! and 1407 to 1409).
//!
//! [`art_from_drawing`] turns the vector drawing the hidden-line pipeline made
//! (the same one a Live View prints) into the lines a [`ViewArt`] keeps. The
//! lines are then the box's own: Edit Layout Lines can change, add and delete
//! them, and only an update replaces them again. How a line is drawn comes
//! from its own settings, else from the view's Edge / Pattern Line Defaults
//! when those are on, else from the line's weight class or material.

use crate::boxview::{FillRole, LineType, PlotFill, PlotLine, PlotOptions, Tier, ViewArt};
use crate::hatch::HatchStroke;
use plan_core::geometry::{dist_to_segment, point_in_polygon};
use plan_core::{Id, LineStyle, Point};
use plan_elevation::{Drawing, EdgeKind, LineWeight, RegionKind};

/// Pen weights of the vector view's weight classes, points.
pub(crate) const HEAVY_PT: f64 = 0.7;
pub(crate) const MEDIUM_PT: f64 = 0.35;
pub(crate) const LIGHT_PT: f64 = 0.18;
/// A material hatch stroke.
pub(crate) const HATCH_PT: f64 = 0.15;
/// The line of a section cut.
pub(crate) const CUT_PT: f64 = 1.0;
/// Gray of the poche of a cut and of a cast shadow.
pub(crate) const CUT_RGB: [u8; 3] = [140, 140, 140];
pub(crate) const SHADOW_RGB: [u8; 3] = [217, 217, 217];
/// Gray of a pattern line whose material gave no color.
const PATTERN_GRAY: [u8; 3] = [90, 90, 90];

/// The tier a drawing line starts with.
fn tier_of(kind: EdgeKind, weight: LineWeight) -> Tier {
    match kind {
        EdgeKind::Hidden => Tier::Hidden,
        EdgeKind::Cut => Tier::Cut,
        _ => match weight {
            LineWeight::Heavy => Tier::Heavy,
            LineWeight::Medium => Tier::Medium,
            LineWeight::Light => Tier::Light,
        },
    }
}

fn rgb_of(c: [f32; 4]) -> [u8; 3] {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [f(c[0]), f(c[1]), f(c[2])]
}

/// Hatch strokes are drawn in this fraction of their material's color.
const HATCH_DARKEN: f32 = 0.6;

/// The picture a Plot Lines (or Update on Demand) view keeps, made from the
/// view's vector drawing: every line is an edge line, except the strokes of a
/// material hatch (the drawing's own and the wall-face `hatch` of the scene),
/// which are pattern lines in the color of their material.
pub fn art_from_drawing(d: &Drawing, hatch: &[HatchStroke]) -> ViewArt {
    let mut art = ViewArt::default();
    // Face regions give the pattern lines their material's color.
    let faces: Vec<&plan_elevation::Region> = d
        .regions
        .iter()
        .filter(|r| r.kind == RegionKind::Face)
        .collect();
    for l in &d.lines {
        let hatch = l.kind == EdgeKind::Hatch;
        let base_color = hatch
            .then(|| {
                let mid = Point::lerp(l.a, l.b, 0.5);
                faces
                    .iter()
                    .find(|r| point_in_polygon(mid, &r.polygon))
                    .map(|r| rgb_of(r.material.color()))
            })
            .flatten();
        art.next_id += 1;
        art.lines.push(PlotLine {
            id: art.next_id,
            a: l.a,
            b: l.b,
            kind: if hatch {
                LineType::Pattern
            } else {
                LineType::Edge
            },
            tier: if hatch {
                Tier::Light
            } else {
                tier_of(l.kind, l.weight)
            },
            base_color,
            weight_pt: None,
            style: None,
            color: None,
            added: false,
        });
    }
    for h in hatch {
        let c = h.material.color();
        art.next_id += 1;
        art.lines.push(PlotLine {
            id: art.next_id,
            a: h.a,
            b: h.b,
            kind: LineType::Pattern,
            tier: Tier::Light,
            base_color: Some(rgb_of([
                c[0] * HATCH_DARKEN,
                c[1] * HATCH_DARKEN,
                c[2] * HATCH_DARKEN,
                1.0,
            ])),
            weight_pt: None,
            style: None,
            color: None,
            added: false,
        });
    }
    for r in &d.regions {
        let (rgb, role) = match r.kind {
            RegionKind::Cut => (CUT_RGB, FillRole::Cut),
            RegionKind::Shadow => (SHADOW_RGB, FillRole::Shadow),
            RegionKind::Face => (rgb_of(r.material.color()), FillRole::Material),
        };
        art.fills.push(PlotFill {
            polygon: r.polygon.clone(),
            rgb,
            role,
        });
    }
    art.texts = d.texts.clone();
    art.update_bounds();
    art.rev = 1;
    art
}

/// How one plot line is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotPen {
    pub weight_pt: f64,
    pub color: [u8; 3],
    pub style: LineStyle,
}

/// The pen of `line` under the view's `opts`: the line's own settings win,
/// then the view's Edge / Pattern Line Defaults when they are on, then the
/// vector view's weight class (edge lines, black) or the material's pattern
/// (pattern lines).
pub fn effective_pen(line: &PlotLine, opts: &PlotOptions) -> PlotPen {
    let (use_defaults, def_w, def_c) = match line.kind {
        LineType::Edge => (opts.use_edge_defaults, opts.edge_weight_pt, opts.edge_color),
        LineType::Pattern => (
            opts.use_pattern_defaults,
            opts.pattern_weight_pt,
            opts.pattern_color,
        ),
    };
    let tier_w = match line.tier {
        Tier::Light | Tier::Hidden => LIGHT_PT,
        Tier::Medium => MEDIUM_PT,
        Tier::Heavy => HEAVY_PT,
        Tier::Cut => CUT_PT,
    };
    let natural_w = match line.kind {
        LineType::Edge => tier_w,
        LineType::Pattern => HATCH_PT,
    };
    let natural_c = match line.kind {
        LineType::Edge => [0, 0, 0],
        LineType::Pattern => line.base_color.unwrap_or(PATTERN_GRAY),
    };
    let natural_s = if line.tier == Tier::Hidden {
        LineStyle::Dashed
    } else {
        LineStyle::Solid
    };
    PlotPen {
        weight_pt: line
            .weight_pt
            .unwrap_or(if use_defaults { def_w } else { natural_w }),
        color: line
            .color
            .unwrap_or(if use_defaults { def_c } else { natural_c }),
        style: line.style.unwrap_or(natural_s),
    }
}

// ------------------------------------------------------------- editing --

/// What the Layout Line Specification changes. `None` is "No Change"; a
/// `Some(None)` is the "Use Default" check box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LineSpec {
    pub kind: Option<LineType>,
    pub weight_pt: Option<Option<f64>>,
    pub style: Option<Option<LineStyle>>,
    pub color: Option<Option<[u8; 3]>>,
}

impl ViewArt {
    fn touch(&mut self) {
        self.rev += 1;
        self.update_bounds();
    }

    /// Applies `spec` to the lines `ids`; returns how many changed.
    pub fn set_spec(&mut self, ids: &[Id], spec: &LineSpec) -> usize {
        let mut n = 0;
        for l in self.lines.iter_mut().filter(|l| ids.contains(&l.id)) {
            let before = l.clone();
            if let Some(k) = spec.kind {
                l.kind = k;
            }
            if let Some(w) = spec.weight_pt {
                l.weight_pt = w.map(|w| w.clamp(0.01, 12.0));
            }
            if let Some(s) = spec.style {
                l.style = s;
            }
            if let Some(c) = spec.color {
                l.color = c;
            }
            n += usize::from(*l != before);
        }
        if n > 0 {
            self.touch();
        }
        n
    }

    /// Draws a new line (an edge line unless `kind` says pattern). Returns
    /// its id.
    pub fn add_line(&mut self, a: Point, b: Point, kind: LineType) -> Id {
        self.next_id += 1;
        let id = self.next_id;
        self.lines.push(PlotLine {
            id,
            a,
            b,
            kind,
            tier: Tier::Medium,
            base_color: None,
            weight_pt: None,
            style: None,
            color: None,
            added: true,
        });
        self.touch();
        id
    }

    /// Deletes the lines `ids`; returns how many went.
    pub fn delete_lines(&mut self, ids: &[Id]) -> usize {
        let before = self.lines.len();
        self.lines.retain(|l| !ids.contains(&l.id));
        let n = before - self.lines.len();
        if n > 0 {
            self.touch();
        }
        n
    }

    /// Moves the ends of line `id`.
    pub fn move_line(&mut self, id: Id, a: Point, b: Point) -> bool {
        let Some(l) = self.lines.iter_mut().find(|l| l.id == id) else {
            return false;
        };
        if l.a == a && l.b == b {
            return false;
        }
        l.a = a;
        l.b = b;
        self.touch();
        true
    }

    /// Moves every line `ids` by `d` (drawing inches).
    pub fn nudge_lines(&mut self, ids: &[Id], d: Point) -> usize {
        let mut n = 0;
        for l in self.lines.iter_mut().filter(|l| ids.contains(&l.id)) {
            l.a = l.a + d;
            l.b = l.b + d;
            n += 1;
        }
        if n > 0 {
            self.touch();
        }
        n
    }

    /// The line nearest `p` within `tol` drawing inches (the later-drawn line
    /// wins a tie, so a line you added is picked over the one under it).
    pub fn hit_line(&self, p: Point, tol: f64) -> Option<Id> {
        let mut best: Option<(f64, Id)> = None;
        for l in &self.lines {
            let d = dist_to_segment(p, l.a, l.b);
            if d <= tol && best.is_none_or(|(bd, _)| d <= bd) {
                best = Some((d, l.id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// The lines wholly inside the rectangle `lo`..`hi` (a marquee select).
    pub fn lines_in_rect(&self, lo: Point, hi: Point) -> Vec<Id> {
        let (x0, x1) = (lo.x.min(hi.x), lo.x.max(hi.x));
        let (y0, y1) = (lo.y.min(hi.y), lo.y.max(hi.y));
        let inside = |p: Point| p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1;
        self.lines
            .iter()
            .filter(|l| inside(l.a) && inside(l.b))
            .map(|l| l.id)
            .collect()
    }

    /// Counts `(edge lines, pattern lines)`.
    pub fn counts(&self) -> (usize, usize) {
        let pattern = self
            .lines
            .iter()
            .filter(|l| l.kind == LineType::Pattern)
            .count();
        (self.lines.len() - pattern, pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_3d::Material;
    use plan_elevation::{Line2, Region};

    fn line(a: (f64, f64), b: (f64, f64), weight: LineWeight, kind: EdgeKind) -> Line2 {
        Line2 {
            a: Point::new(a.0, a.1),
            b: Point::new(b.0, b.1),
            weight,
            kind,
        }
    }

    /// A 120 x 96 wall with a brick face, a heavy outline, one hidden edge,
    /// one hatch stroke and a cut poche.
    fn drawing() -> Drawing {
        let mut d = Drawing::new(vec![
            line(
                (0.0, 0.0),
                (120.0, 0.0),
                LineWeight::Heavy,
                EdgeKind::Silhouette,
            ),
            line(
                (120.0, 0.0),
                (120.0, 96.0),
                LineWeight::Medium,
                EdgeKind::Crease,
            ),
            line(
                (0.0, 50.0),
                (120.0, 50.0),
                LineWeight::Light,
                EdgeKind::Hidden,
            ),
            line(
                (0.0, 10.0),
                (120.0, 10.0),
                LineWeight::Light,
                EdgeKind::Hatch,
            ),
            line((0.0, 96.0), (120.0, 96.0), LineWeight::Heavy, EdgeKind::Cut),
        ]);
        let square = |x: f64, y: f64, w: f64, h: f64| {
            vec![
                Point::new(x, y),
                Point::new(x + w, y),
                Point::new(x + w, y + h),
                Point::new(x, y + h),
            ]
        };
        d.regions.push(Region {
            polygon: square(0.0, 0.0, 120.0, 96.0),
            material: Material::Brick,
            object_id: None,
            kind: RegionKind::Face,
        });
        d.regions.push(Region {
            polygon: square(0.0, 90.0, 120.0, 6.0),
            material: Material::Framing,
            object_id: None,
            kind: RegionKind::Cut,
        });
        d.texts.push((Point::new(2.0, 2.0), "NOTE".into()));
        d
    }

    #[test]
    fn a_drawing_becomes_edge_and_pattern_lines_with_fills() {
        let art = art_from_drawing(&drawing(), &[]);
        assert_eq!(art.counts(), (4, 1));
        let tiers: Vec<Tier> = art.lines.iter().map(|l| l.tier).collect();
        assert_eq!(
            tiers,
            [
                Tier::Heavy,
                Tier::Medium,
                Tier::Hidden,
                Tier::Light,
                Tier::Cut
            ]
        );
        // The pattern line takes the brick color from the face it lies in.
        let pat = art
            .lines
            .iter()
            .find(|l| l.kind == LineType::Pattern)
            .unwrap();
        assert_eq!(pat.base_color, Some(rgb_of(Material::Brick.color())));
        assert_eq!(art.fills.len(), 2);
        assert!(art.fills.iter().any(|f| f.role == FillRole::Cut));
        assert!(art.fills.iter().any(|f| f.role == FillRole::Material));
        assert_eq!(art.texts.len(), 1);
        assert_eq!(art.bounds, (Point::new(0.0, 0.0), Point::new(120.0, 96.0)));
        assert_eq!(art.lines.iter().map(|l| l.id).max(), Some(art.next_id));
    }

    #[test]
    fn pens_follow_the_line_then_the_defaults_then_the_weight_class() {
        let art = art_from_drawing(&drawing(), &[]);
        let opts = PlotOptions::default();
        let heavy = effective_pen(&art.lines[0], &opts);
        assert_eq!((heavy.weight_pt, heavy.color), (HEAVY_PT, [0, 0, 0]));
        let hidden = effective_pen(&art.lines[2], &opts);
        assert_eq!(hidden.style, LineStyle::Dashed);
        let pat = effective_pen(&art.lines[3], &opts);
        assert_eq!(pat.weight_pt, HATCH_PT);
        assert_eq!(pat.color, rgb_of(Material::Brick.color()));
        // Edge Line Defaults on: every edge line takes them, a pattern line
        // does not.
        let on = PlotOptions {
            use_edge_defaults: true,
            edge_weight_pt: 0.5,
            edge_color: [10, 20, 30],
            ..PlotOptions::default()
        };
        assert_eq!(effective_pen(&art.lines[1], &on).weight_pt, 0.5);
        assert_eq!(effective_pen(&art.lines[1], &on).color, [10, 20, 30]);
        assert_eq!(effective_pen(&art.lines[3], &on).weight_pt, HATCH_PT);
        // A line's own weight beats the defaults; "Use Default" gives it back.
        let mut art = art;
        let id = art.lines[1].id;
        art.set_spec(
            &[id],
            &LineSpec {
                weight_pt: Some(Some(2.0)),
                ..LineSpec::default()
            },
        );
        assert_eq!(effective_pen(art.line(id).unwrap(), &on).weight_pt, 2.0);
        art.set_spec(
            &[id],
            &LineSpec {
                weight_pt: Some(None),
                ..LineSpec::default()
            },
        );
        assert_eq!(effective_pen(art.line(id).unwrap(), &on).weight_pt, 0.5);
    }

    #[test]
    fn editing_lines_selects_adds_changes_and_deletes() {
        let mut art = art_from_drawing(&drawing(), &[]);
        let rev = art.rev;
        // Hit test: the heavy bottom edge, then nothing far away.
        let bottom = art.lines[0].id;
        assert_eq!(art.hit_line(Point::new(60.0, 0.5), 1.0), Some(bottom));
        assert_eq!(art.hit_line(Point::new(60.0, 30.0), 1.0), None);
        // Marquee: the right edge and nothing else fit in x 100..130, y -1..97
        // wholly... the bottom edge starts at 0, so it does not.
        let ids = art.lines_in_rect(Point::new(100.0, -1.0), Point::new(130.0, 97.0));
        assert_eq!(ids, vec![art.lines[1].id]);
        // Type change moves a line between the counts.
        assert_eq!(
            art.set_spec(
                &[bottom],
                &LineSpec {
                    kind: Some(LineType::Pattern),
                    ..LineSpec::default()
                }
            ),
            1
        );
        assert_eq!(art.counts(), (3, 2));
        assert!(art.rev > rev);
        // A new line is marked as drawn by you and gets the next id.
        let n = art.add_line(
            Point::new(0.0, 20.0),
            Point::new(10.0, 20.0),
            LineType::Edge,
        );
        assert!(art.line(n).unwrap().added);
        assert_eq!(art.hit_line(Point::new(5.0, 20.0), 0.5), Some(n));
        // Deleting reports the count and a second delete finds nothing.
        assert_eq!(art.delete_lines(&[n, bottom]), 2);
        assert_eq!(art.delete_lines(&[n]), 0);
        // Setting the same value again changes nothing (no new revision).
        let rev = art.rev;
        let id = art.lines[0].id;
        let spec = LineSpec {
            style: Some(Some(LineStyle::Dotted)),
            ..LineSpec::default()
        };
        assert_eq!(art.set_spec(&[id], &spec), 1);
        assert_eq!(art.set_spec(&[id], &spec), 0);
        assert_eq!(art.rev, rev + 1);
    }
}
