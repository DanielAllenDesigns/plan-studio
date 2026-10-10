//! Annotations saved with a camera view (`docs/parity/3d-views-cameras.md`
//! C-129, C-130, C-142..C-144; manual pp. 1166, 1173, 1177 to 1179).
//!
//! Text, Rich Text, Note, Leader Line, linear Dimensions and CAD lines, boxes
//! and polylines can be drawn on a cross section, an elevation or a camera
//! view. They belong to the view, not to a floor: they are stored in
//! [`super::CameraView::annotations`], come back when the saved view is
//! opened and are drawn again when it is sent to layout.
//!
//! Every annotation lies on a [`DrawSurface`], a plane with its own 2D
//! coordinates. A section's surface is the drawing itself (X along the cut
//! line, Y up); in a camera view it is a side of an object's bounding box or
//! one of its faces, moved out by the Offset from Draw Surface.

use crate::geometry::Point;
use crate::model::Id;
use serde::{Deserialize, Serialize};

/// Reference to an edge of what a section plane cuts (a Cross Section Line),
/// so a dimension keeps locating it when the lines are made again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CutRef {
    /// The wall that is cut.
    pub object: Id,
    /// 0 left face, 1 right face, 2 bottom, 3 top; plus 4 for each piece of
    /// a stepped plane after the first.
    pub edge: u8,
}

impl CutRef {
    /// Does the reference locate a position along the cut line (a vertical
    /// face) rather than a height?
    pub fn is_vertical_face(self) -> bool {
        self.edge % 4 < 2
    }
}

/// A plane to draw on: `origin + s * u + t * v`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DrawSurface {
    pub origin: [f64; 3],
    pub u: [f64; 3],
    pub v: [f64; 3],
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: [f64; 3]) -> [f64; 3] {
    let l = dot3(a, a).sqrt();
    if l < 1e-12 {
        [0.0, 0.0, 0.0]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

/// The six sides of a bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoxSide {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

impl BoxSide {
    pub const ALL: [BoxSide; 6] = [
        BoxSide::Front,
        BoxSide::Back,
        BoxSide::Left,
        BoxSide::Right,
        BoxSide::Top,
        BoxSide::Bottom,
    ];
}

impl DrawSurface {
    /// The drawing of a section or elevation: `s` is the drawing's X, `t` its
    /// Y, on the plane `z = 0`.
    pub fn drawing() -> Self {
        Self {
            origin: [0.0; 3],
            u: [1.0, 0.0, 0.0],
            v: [0.0, 1.0, 0.0],
        }
    }

    /// Scene position of the surface point `(s, t)`.
    pub fn point(&self, s: f64, t: f64) -> [f64; 3] {
        [
            self.origin[0] + self.u[0] * s + self.v[0] * t,
            self.origin[1] + self.u[1] * s + self.v[1] * t,
            self.origin[2] + self.u[2] * s + self.v[2] * t,
        ]
    }

    /// Unit normal, `u x v`.
    pub fn normal(&self) -> [f64; 3] {
        unit3(cross3(self.u, self.v))
    }

    /// The surface moved `d` inches along its normal (Offset from Draw
    /// Surface).
    pub fn offset(mut self, d: f64) -> Self {
        let n = self.normal();
        for (o, n) in self.origin.iter_mut().zip(n) {
            *o += n * d;
        }
        self
    }

    /// The surface coordinates of the scene point `p` projected onto the
    /// plane (the two axes are unit and square to each other).
    pub fn project(&self, p: [f64; 3]) -> (f64, f64) {
        let d = sub3(p, self.origin);
        (dot3(d, self.u), dot3(d, self.v))
    }

    /// A side of the box `min..max` (scene space, X east, Y up, Z south),
    /// seen from outside: `u` runs to the viewer's right, `v` up (or, for
    /// the top and bottom, away from the viewer). Draw on Bounding Box.
    pub fn box_side(min: [f64; 3], max: [f64; 3], side: BoxSide) -> Self {
        let (w, h, d) = (max[0] - min[0], max[1] - min[1], max[2] - min[2]);
        let _ = (w, h, d);
        match side {
            // Looking at the south face (+Z) from outside: right is +X.
            BoxSide::Front => Self {
                origin: [min[0], min[1], max[2]],
                u: [1.0, 0.0, 0.0],
                v: [0.0, 1.0, 0.0],
            },
            BoxSide::Back => Self {
                origin: [max[0], min[1], min[2]],
                u: [-1.0, 0.0, 0.0],
                v: [0.0, 1.0, 0.0],
            },
            BoxSide::Left => Self {
                origin: [min[0], min[1], min[2]],
                u: [0.0, 0.0, 1.0],
                v: [0.0, 1.0, 0.0],
            },
            BoxSide::Right => Self {
                origin: [max[0], min[1], max[2]],
                u: [0.0, 0.0, -1.0],
                v: [0.0, 1.0, 0.0],
            },
            BoxSide::Top => Self {
                origin: [min[0], max[1], max[2]],
                u: [1.0, 0.0, 0.0],
                v: [0.0, 0.0, -1.0],
            },
            BoxSide::Bottom => Self {
                origin: [min[0], min[1], min[2]],
                u: [1.0, 0.0, 0.0],
                v: [0.0, 0.0, 1.0],
            },
        }
    }

    /// The plane of a face through three corners (Draw on Surface); `u` runs
    /// along the face's horizontal direction and `v` up it (or, for a
    /// horizontal face, toward -Z). The normal points toward `toward` (the
    /// camera) so the surface faces the viewer.
    pub fn face(a: [f64; 3], b: [f64; 3], c: [f64; 3], toward: [f64; 3]) -> Option<Self> {
        let mut n = unit3(cross3(sub3(b, a), sub3(c, a)));
        if dot3(n, n) < 0.5 {
            return None;
        }
        if dot3(n, sub3(toward, a)) < 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        let up = [0.0, 1.0, 0.0];
        let (u, v) = if n[1].abs() > 0.999 {
            // Horizontal: u east, v away from the viewer along -Z.
            let u = [1.0, 0.0, 0.0];
            (u, unit3(cross3(n, u)))
        } else {
            let u = unit3(cross3(up, n));
            (u, unit3(cross3(n, u)))
        };
        Some(Self { origin: a, u, v })
    }
}

/// What an annotation is. Coordinates are surface coordinates, inches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnnotKind {
    /// Text or Rich Text (`rich`): `at` is the left end of the baseline.
    Text {
        at: [f64; 2],
        text: String,
        size: f64,
        rich: bool,
        /// Turns to face the camera (camera views only).
        faces_camera: bool,
    },
    /// A Note: a text that is also a schedule entry of a note type.
    Note {
        at: [f64; 2],
        text: String,
        note_type: String,
        size: f64,
    },
    /// A Leader Line: the arrow tip `tip`, the text at `at`.
    Leader {
        tip: [f64; 2],
        at: [f64; 2],
        text: String,
        size: f64,
    },
    /// A linear dimension from `a` to `b`. `offset` is the distance of the
    /// dimension line from the points (its component square to `a`-`b`) and
    /// the slide of the text along it (the component along `a`-`b`): the two
    /// Move handles of a dimension in a camera view.
    Dimension {
        a: [f64; 2],
        b: [f64; 2],
        offset: [f64; 2],
        a_cut: Option<CutRef>,
        b_cut: Option<CutRef>,
        text: Option<String>,
    },
    /// A CAD line.
    Line { a: [f64; 2], b: [f64; 2] },
    /// A CAD box, `a` and `b` opposite corners.
    Rect { a: [f64; 2], b: [f64; 2] },
    /// A CAD polyline.
    Polyline { pts: Vec<[f64; 2]>, closed: bool },
    /// A Point Marker that a dimension to a Cross Section Line left behind.
    PointMarker { at: [f64; 2], cut: CutRef },
}

/// One annotation of a view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewAnnotation {
    pub id: Id,
    pub kind: AnnotKind,
    pub surface: DrawSurface,
    /// The display layer, from the view's Selected Defaults.
    pub layer: String,
    /// Line weight in points; `None` uses the layer's.
    #[serde(default)]
    pub weight: Option<f64>,
}

/// An annotation turned into plain marks, in surface coordinates.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Marks {
    pub lines: Vec<([f64; 2], [f64; 2])>,
    /// `(centre of the text, text, height)`.
    pub texts: Vec<([f64; 2], String, f64)>,
}

/// Height of the figures of a dimension, inches.
pub const DIM_TEXT: f64 = 6.0;
/// Gap between a dimension's extension line and the point it locates, inches.
const EXT_GAP: f64 = 2.0;
/// How far an extension line passes the dimension line, inches.
const EXT_OVER: f64 = 3.0;
/// Half length of a dimension tick, inches.
const TICK: f64 = 3.0;
/// Width of the arrowhead of a leader line, inches.
const ARROW: f64 = 5.0;

fn p2(a: [f64; 2]) -> Point {
    Point::new(a[0], a[1])
}

fn a2(p: Point) -> [f64; 2] {
    [p.x, p.y]
}

impl AnnotKind {
    /// The points a user drags (end points, corners, anchors).
    pub fn handles(&self) -> Vec<[f64; 2]> {
        match self {
            AnnotKind::Text { at, .. } | AnnotKind::Note { at, .. } => vec![*at],
            AnnotKind::Leader { tip, at, .. } => vec![*at, *tip],
            AnnotKind::Dimension { a, b, offset, .. } => {
                let (n, d) = dim_axes(*a, *b);
                let mid = p2(*a) + (p2(*b) - p2(*a)) * 0.5;
                vec![*a, *b, a2(mid + d * offset[0] + n * offset[1])]
            }
            AnnotKind::Line { a, b } | AnnotKind::Rect { a, b } => vec![*a, *b],
            AnnotKind::Polyline { pts, .. } => pts.clone(),
            AnnotKind::PointMarker { at, .. } => vec![*at],
        }
    }

    /// Moves handle `i` to `to`. A dimension's third handle sets its offset
    /// in both directions.
    pub fn set_handle(&mut self, i: usize, to: [f64; 2]) {
        match self {
            AnnotKind::Text { at, .. } | AnnotKind::Note { at, .. } => *at = to,
            AnnotKind::Leader { tip, at, .. } => {
                if i == 0 {
                    *at = to;
                } else {
                    *tip = to;
                }
            }
            AnnotKind::Dimension {
                a,
                b,
                offset,
                a_cut,
                b_cut,
                ..
            } => match i {
                0 => {
                    *a = to;
                    *a_cut = None;
                }
                1 => {
                    *b = to;
                    *b_cut = None;
                }
                _ => {
                    let (n, d) = dim_axes(*a, *b);
                    let mid = p2(*a) + (p2(*b) - p2(*a)) * 0.5;
                    let rel = p2(to) - mid;
                    *offset = [rel.dot(d), rel.dot(n)];
                }
            },
            AnnotKind::Line { a, b } | AnnotKind::Rect { a, b } => {
                if i == 0 {
                    *a = to;
                } else {
                    *b = to;
                }
            }
            AnnotKind::Polyline { pts, .. } => {
                if let Some(p) = pts.get_mut(i) {
                    *p = to;
                }
            }
            AnnotKind::PointMarker { at, .. } => *at = to,
        }
    }

    /// Moves the whole annotation by `d`.
    pub fn translate(&mut self, d: [f64; 2]) {
        let mv = |p: &mut [f64; 2]| {
            p[0] += d[0];
            p[1] += d[1];
        };
        match self {
            AnnotKind::Text { at, .. } | AnnotKind::Note { at, .. } => mv(at),
            AnnotKind::Leader { tip, at, .. } => {
                mv(tip);
                mv(at);
            }
            AnnotKind::Dimension {
                a, b, a_cut, b_cut, ..
            } => {
                mv(a);
                mv(b);
                *a_cut = None;
                *b_cut = None;
            }
            AnnotKind::Line { a, b } | AnnotKind::Rect { a, b } => {
                mv(a);
                mv(b);
            }
            AnnotKind::Polyline { pts, .. } => pts.iter_mut().for_each(mv),
            AnnotKind::PointMarker { at, .. } => mv(at),
        }
    }

    /// Moves the points that locate a Cross Section Line to where the line
    /// stands now (`position_of` gives its position: `x` for a face, `y` for
    /// a level edge). A line that is gone leaves the point where it was.
    pub fn relocate(&mut self, position_of: &dyn Fn(CutRef) -> Option<f64>) {
        let place = |p: &mut [f64; 2], cut: Option<CutRef>| {
            if let Some((c, v)) = cut.and_then(|c| position_of(c).map(|v| (c, v))) {
                p[usize::from(!c.is_vertical_face())] = v;
            }
        };
        match self {
            AnnotKind::Dimension {
                a, b, a_cut, b_cut, ..
            } => {
                place(a, *a_cut);
                place(b, *b_cut);
            }
            AnnotKind::PointMarker { at, cut } => place(at, Some(*cut)),
            _ => {}
        }
    }

    /// A dimension's measured length, inches.
    pub fn dimension_value(&self) -> Option<f64> {
        match self {
            AnnotKind::Dimension { a, b, .. } => Some(p2(*a).dist(p2(*b))),
            _ => None,
        }
    }

    /// Short name for lists and the status line.
    pub fn label(&self) -> &'static str {
        match self {
            AnnotKind::Text { rich: true, .. } => "Rich Text",
            AnnotKind::Text { .. } => "Text",
            AnnotKind::Note { .. } => "Note",
            AnnotKind::Leader { .. } => "Leader Line",
            AnnotKind::Dimension { .. } => "Dimension",
            AnnotKind::Line { .. } => "CAD Line",
            AnnotKind::Rect { .. } => "CAD Box",
            AnnotKind::Polyline { .. } => "CAD Polyline",
            AnnotKind::PointMarker { .. } => "Point Marker",
        }
    }

    /// The text of a text-like annotation.
    pub fn text_mut(&mut self) -> Option<&mut String> {
        match self {
            AnnotKind::Text { text, .. }
            | AnnotKind::Note { text, .. }
            | AnnotKind::Leader { text, .. } => Some(text),
            _ => None,
        }
    }

    /// Is this a CAD object (line, box, polyline), as opposed to text and
    /// dimensions? CAD Detail from View ignores text and dimensions of a
    /// camera view (manual p. 1178).
    pub fn is_cad(&self) -> bool {
        matches!(
            self,
            AnnotKind::Line { .. } | AnnotKind::Rect { .. } | AnnotKind::Polyline { .. }
        )
    }

    /// Distance from the surface point `p` to the annotation, inches.
    pub fn distance(&self, p: [f64; 2]) -> f64 {
        let marks = self.marks();
        let q = p2(p);
        let mut best = f64::INFINITY;
        for (a, b) in &marks.lines {
            best = best.min(crate::geometry::dist_to_segment(q, p2(*a), p2(*b)));
        }
        for (c, t, h) in &marks.texts {
            let w = text_width(t, *h);
            let (dx, dy) = ((q.x - c[0]).abs() - w * 0.5, (q.y - c[1]).abs() - h * 0.5);
            best = best.min(dx.max(0.0).hypot(dy.max(0.0)));
        }
        best
    }

    /// The marks the annotation is drawn with, in surface coordinates.
    pub fn marks(&self) -> Marks {
        let mut m = Marks::default();
        match self {
            AnnotKind::Text { at, text, size, .. } | AnnotKind::Note { at, text, size, .. } => {
                let w = text_width(text, *size);
                m.texts
                    .push(([at[0] + w * 0.5, at[1] + size * 0.5], text.clone(), *size));
            }
            AnnotKind::Leader {
                tip,
                at,
                text,
                size,
            } => {
                let w = text_width(text, *size);
                let c = [at[0] + w * 0.5, at[1] + size * 0.5];
                m.texts.push((c, text.clone(), *size));
                // The line runs from the tip to the nearest end of the text.
                let end = if tip[0] < at[0] {
                    *at
                } else {
                    [at[0] + w, at[1] + size * 0.5]
                };
                let end = [end[0], at[1] + size * 0.5];
                m.lines.push((*tip, end));
                let dir = (p2(end) - p2(*tip)).normalized();
                let n = dir.perp();
                let back = p2(*tip) + dir * ARROW;
                m.lines.push((*tip, a2(back + n * (ARROW * 0.3))));
                m.lines.push((*tip, a2(back - n * (ARROW * 0.3))));
            }
            AnnotKind::Dimension {
                a, b, offset, text, ..
            } => {
                let (n, d) = dim_axes(*a, *b);
                let len = p2(*a).dist(p2(*b));
                let off = offset[1];
                let sign = if off < 0.0 { -1.0 } else { 1.0 };
                let (pa, pb) = (p2(*a) + n * off, p2(*b) + n * off);
                for (from, to) in [(p2(*a), pa), (p2(*b), pb)] {
                    m.lines.push((
                        a2(from + n * (sign * EXT_GAP)),
                        a2(to + n * (sign * EXT_OVER)),
                    ));
                }
                m.lines.push((a2(pa), a2(pb)));
                for p in [pa, pb] {
                    m.lines.push((
                        a2(p - (d + n) * (TICK * std::f64::consts::FRAC_1_SQRT_2)),
                        a2(p + (d + n) * (TICK * std::f64::consts::FRAC_1_SQRT_2)),
                    ));
                }
                let value = text
                    .clone()
                    .unwrap_or_else(|| crate::units::fmt_ft_in_frac(len, 8));
                let mid = pa + (pb - pa) * 0.5 + d * offset[0];
                let at = mid + n * (sign * (DIM_TEXT * 0.6 + 1.0));
                m.texts.push((a2(at), value, DIM_TEXT));
            }
            AnnotKind::Line { a, b } => m.lines.push((*a, *b)),
            AnnotKind::Rect { a, b } => {
                let c = [[a[0], a[1]], [b[0], a[1]], [b[0], b[1]], [a[0], b[1]]];
                for i in 0..4 {
                    m.lines.push((c[i], c[(i + 1) % 4]));
                }
            }
            AnnotKind::Polyline { pts, closed } => {
                for w in pts.windows(2) {
                    m.lines.push((w[0], w[1]));
                }
                if *closed && pts.len() > 2 {
                    m.lines.push((pts[pts.len() - 1], pts[0]));
                }
            }
            AnnotKind::PointMarker { at, .. } => {
                let r = 2.0;
                m.lines.push(([at[0] - r, at[1]], [at[0] + r, at[1]]));
                m.lines.push(([at[0], at[1] - r], [at[0], at[1] + r]));
            }
        }
        m
    }
}

/// Estimated width of `text` set `height` inches tall.
pub fn text_width(text: &str, height: f64) -> f64 {
    0.6 * height * text.chars().count() as f64
}

/// Unit normal and direction of a dimension from `a` to `b`.
fn dim_axes(a: [f64; 2], b: [f64; 2]) -> (Point, Point) {
    let v = p2(b) - p2(a);
    let d = if v.length() < 1e-9 {
        Point::new(1.0, 0.0)
    } else {
        v.normalized()
    };
    (d.perp(), d)
}

impl ViewAnnotation {
    /// The marks in scene space, for a surface in a camera view.
    pub fn scene_marks(&self) -> Vec<([f64; 3], [f64; 3])> {
        self.kind
            .marks()
            .lines
            .iter()
            .map(|(a, b)| {
                (
                    self.surface.point(a[0], a[1]),
                    self.surface.point(b[0], b[1]),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(a: [f64; 2], b: [f64; 2], off: [f64; 2]) -> AnnotKind {
        AnnotKind::Dimension {
            a,
            b,
            offset: off,
            a_cut: None,
            b_cut: None,
            text: None,
        }
    }

    #[test]
    fn a_dimension_measures_and_writes_feet_and_inches() {
        let d = dim([0.0, 0.0], [102.0, 0.0], [0.0, 12.0]);
        assert_eq!(d.dimension_value(), Some(102.0));
        let m = d.marks();
        assert_eq!(m.texts.len(), 1);
        assert_eq!(m.texts[0].1, "8'-6\"");
        // Two extension lines, the dimension line and two ticks.
        assert_eq!(m.lines.len(), 5);
        // The dimension line stands 12 inches off the points.
        assert!(m.lines.iter().any(|(a, b)| a[1] == 12.0 && b[1] == 12.0));
    }

    #[test]
    fn the_offset_handle_moves_the_line_and_the_text() {
        let mut d = dim([0.0, 0.0], [100.0, 0.0], [0.0, 10.0]);
        let h = d.handles();
        assert_eq!(h.len(), 3);
        assert!((h[2][1] - 10.0).abs() < 1e-9 && (h[2][0] - 50.0).abs() < 1e-9);
        d.set_handle(2, [70.0, -20.0]);
        let AnnotKind::Dimension { offset, .. } = &d else {
            unreachable!()
        };
        assert!((offset[0] - 20.0).abs() < 1e-9 && (offset[1] + 20.0).abs() < 1e-9);
        // Moving an end point frees it from a Cross Section Line.
        let mut e = AnnotKind::Dimension {
            a: [0.0, 0.0],
            b: [50.0, 0.0],
            offset: [0.0, 6.0],
            a_cut: Some(CutRef { object: 3, edge: 1 }),
            b_cut: None,
            text: Some("custom".into()),
        };
        e.set_handle(0, [4.0, 0.0]);
        let AnnotKind::Dimension { a_cut, .. } = &e else {
            unreachable!()
        };
        assert!(a_cut.is_none());
        assert_eq!(e.marks().texts[0].1, "custom");
    }

    #[test]
    fn text_and_leaders_draw_where_they_were_put() {
        let t = AnnotKind::Text {
            at: [10.0, 20.0],
            text: "SIDING".into(),
            size: 6.0,
            rich: false,
            faces_camera: false,
        };
        let m = t.marks();
        assert_eq!(m.texts[0].1, "SIDING");
        assert!(t.distance([20.0, 23.0]) < 1.0, "inside the text box");
        assert!(t.distance([200.0, 200.0]) > 100.0);
        let l = AnnotKind::Leader {
            tip: [0.0, 0.0],
            at: [40.0, 30.0],
            text: "FLASHING".into(),
            size: 6.0,
        };
        // The leader line plus the two barbs of its arrow.
        assert_eq!(l.marks().lines.len(), 3);
        assert_eq!(l.handles(), vec![[40.0, 30.0], [0.0, 0.0]]);
    }

    #[test]
    fn cad_objects_have_their_corners_as_handles() {
        let mut r = AnnotKind::Rect {
            a: [0.0, 0.0],
            b: [10.0, 5.0],
        };
        assert_eq!(r.marks().lines.len(), 4);
        r.set_handle(1, [20.0, 8.0]);
        assert_eq!(r.handles(), vec![[0.0, 0.0], [20.0, 8.0]]);
        let mut p = AnnotKind::Polyline {
            pts: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]],
            closed: true,
        };
        assert_eq!(p.marks().lines.len(), 3);
        p.translate([5.0, 5.0]);
        assert_eq!(p.handles()[0], [5.0, 5.0]);
        assert!(p.is_cad() && !dim([0.0; 2], [1.0, 0.0], [0.0; 2]).is_cad());
    }

    #[test]
    fn surfaces_project_points_back() {
        let s = DrawSurface::box_side([0.0, 0.0, 0.0], [100.0, 80.0, 60.0], BoxSide::Front);
        assert_eq!(s.point(10.0, 20.0), [10.0, 20.0, 60.0]);
        assert_eq!(s.normal(), [0.0, 0.0, 1.0]);
        let off = s.offset(2.0);
        assert_eq!(off.point(0.0, 0.0)[2], 62.0);
        let (a, b) = off.project([30.0, 40.0, 99.0]);
        assert!((a - 30.0).abs() < 1e-9 && (b - 40.0).abs() < 1e-9);
        // Every side of a box faces away from its middle.
        let (lo, hi) = ([0.0; 3], [100.0, 80.0, 60.0]);
        let mid = [50.0, 40.0, 30.0];
        for side in BoxSide::ALL {
            let f = DrawSurface::box_side(lo, hi, side);
            let n = f.normal();
            let out = dot3(n, sub3(f.origin, mid));
            assert!(out > 0.0, "{side:?} faces outward");
        }
    }

    #[test]
    fn a_face_turns_toward_the_camera() {
        // A wall face in the plane z = 10, seen from +Z and from -Z.
        let (a, b, c) = ([0.0, 0.0, 10.0], [10.0, 0.0, 10.0], [10.0, 5.0, 10.0]);
        let from_front = DrawSurface::face(a, b, c, [0.0, 0.0, 100.0]).unwrap();
        let from_back = DrawSurface::face(a, b, c, [0.0, 0.0, -100.0]).unwrap();
        assert!(from_front.normal()[2] > 0.99 && from_back.normal()[2] < -0.99);
        assert!(from_front.v[1] > 0.99, "up is up on a vertical face");
        // A floor face keeps u east.
        let floor = DrawSurface::face(
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [10.0, 0.0, 10.0],
            [0.0, 50.0, 0.0],
        )
        .unwrap();
        assert!(floor.normal()[1] > 0.99 && floor.u[0] > 0.99);
        assert!(DrawSurface::face(a, a, a, [0.0; 3]).is_none());
    }

    #[test]
    fn annotations_round_trip_through_json() {
        let a = ViewAnnotation {
            id: 7,
            kind: dim([0.0, 0.0], [10.0, 0.0], [0.0, 4.0]),
            surface: DrawSurface::drawing(),
            layer: "Dimensions".into(),
            weight: Some(0.5),
        };
        let back: ViewAnnotation =
            serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn a_dimension_to_a_cut_line_follows_the_line() {
        let cut = CutRef { object: 3, edge: 1 };
        let mut dim = AnnotKind::Dimension {
            a: [100.0, 40.0],
            b: [160.0, 40.0],
            offset: [0.0, 12.0],
            a_cut: Some(cut),
            b_cut: None,
            text: None,
        };
        // The wall moved: its right face now stands at x = 112.
        dim.relocate(&|c| (c == cut).then_some(112.0));
        let AnnotKind::Dimension { a, b, .. } = &dim else {
            unreachable!()
        };
        assert_eq!((a[0], a[1]), (112.0, 40.0));
        assert_eq!(b[0], 160.0, "the free end stays where it is");
        // A level edge moves the height instead.
        let top = CutRef { object: 3, edge: 3 };
        let mut m = AnnotKind::PointMarker {
            at: [5.0, 90.0],
            cut: top,
        };
        m.relocate(&|_| Some(96.0));
        assert_eq!(m.handles()[0], [5.0, 96.0]);
        // A line that is gone leaves the point alone.
        m.relocate(&|_| None);
        assert_eq!(m.handles()[0], [5.0, 96.0]);
    }
}
