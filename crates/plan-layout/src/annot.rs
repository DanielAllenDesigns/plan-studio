//! Page annotations: layout CAD, leaders and revision clouds.
//!
//! All of them live on a [`LayoutPage`], in paper inches, and share one id
//! space (see [`LayoutPage::next_cad_id`]) so the layout window can select,
//! move, resize and delete any of them the same way. The geometry here is
//! pure: the window, the PDF and the tests all read the same outlines.

use crate::model::LayoutPage;
use plan_core::{CadItem, CadObject, Id, Point};
use plan_docs::PdfDoc;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// Default text height of a leader, paper inches.
pub const LEADER_TEXT_IN: f64 = 0.125;
/// Bump width of a revision cloud, paper inches.
pub const CLOUD_BUMP_IN: f64 = 0.3;
/// Smallest side of a revision cloud, paper inches.
pub const MIN_CLOUD_IN: f64 = 0.3;

/// A leader: text with a line and an arrowhead pointing at what it names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageLeader {
    pub id: Id,
    /// The point the arrow touches.
    pub tip: Point,
    /// Where the line meets the text (the text sits on a landing line from here).
    pub elbow: Point,
    pub text: String,
    /// Text height, paper inches.
    pub height_in: f64,
    /// Draw an arrowhead at the tip.
    pub arrow: bool,
}

/// A revision cloud around a rectangle, tagged with a revision mark.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevisionCloud {
    pub id: Id,
    /// Two opposite corners, paper inches.
    pub rect: (Point, Point),
    /// The revision it belongs to (`1`, `A`): drawn in a triangle at the cloud's corner. Empty = no tag.
    pub revision: String,
}

/// What kind of annotation an id names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKind {
    Cad,
    Leader,
    Cloud,
}

fn norm(a: Point, b: Point) -> [f64; 4] {
    [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]
}

impl PageLeader {
    /// Width of the text on the page, paper inches.
    pub fn text_width_in(&self) -> f64 {
        self.text
            .lines()
            .map(|l| PdfDoc::text_width(l, self.height_in * 72.0))
            .fold(0.0, f64::max)
            / 72.0
    }

    /// Where the text's bottom-left corner sits (above the landing line).
    pub fn text_pos(&self) -> Point {
        Point::new(
            self.elbow.x + self.height_in * 0.25,
            self.elbow.y + self.height_in * 0.2,
        )
    }

    /// The line from the tip to the elbow, then the landing line under the text.
    pub fn polylines(&self) -> Vec<Vec<Point>> {
        let land = Point::new(
            self.elbow.x + self.text_width_in() + self.height_in * 0.5,
            self.elbow.y,
        );
        vec![vec![self.tip, self.elbow, land]]
    }

    /// The arrowhead triangle at the tip, if the leader has one.
    pub fn arrowhead(&self) -> Option<[Point; 3]> {
        if !self.arrow {
            return None;
        }
        let dir = self.elbow.sub(self.tip);
        if dir.length() < 1e-9 {
            return None;
        }
        let u = dir.normalized();
        let len = (self.height_in * 0.9).max(0.08);
        let n = u.perp();
        let base = self.tip.add(u.scale(len));
        Some([
            self.tip,
            base.add(n.scale(len * 0.3)),
            base.sub(n.scale(len * 0.3)),
        ])
    }

    /// Bounds `[x0, y0, x1, y1]` of the leader and its text.
    pub fn bounds(&self) -> [f64; 4] {
        let tp = self.text_pos();
        let lines = self.text.lines().count().max(1) as f64;
        let w = self.text_width_in() + self.height_in * 0.5;
        let mut r = norm(self.tip, self.elbow);
        r[0] = r[0].min(self.elbow.x);
        r[2] = r[2].max(self.elbow.x + w).max(tp.x + w);
        r[3] = r[3].max(tp.y + self.height_in * lines);
        r
    }
}

/// The scalloped outline of a revision cloud around `rect`: a closed polyline
/// of small outward arcs about `bump_in` wide. Corners fall between bumps.
pub fn cloud_outline(rect: (Point, Point), bump_in: f64) -> Vec<Point> {
    let r = norm(rect.0, rect.1);
    let corners = [
        Point::new(r[0], r[1]),
        Point::new(r[2], r[1]),
        Point::new(r[2], r[3]),
        Point::new(r[0], r[3]),
    ];
    let mut out = Vec::new();
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        let len = a.dist(b);
        if len < 1e-9 {
            continue;
        }
        let n = ((len / bump_in.max(0.05)).round() as usize).max(1);
        let u = b.sub(a).normalized();
        // The rectangle runs counter-clockwise, so the outside is on the right.
        let out_n = Point::new(u.y, -u.x);
        let step = len / n as f64;
        for k in 0..n {
            let s = a.add(u.scale(step * k as f64));
            // A semicircle from s to s + step*u bulging outward.
            let mid = s.add(u.scale(step * 0.5));
            let steps = 6;
            for j in 0..steps {
                let t = PI * j as f64 / steps as f64;
                let p = mid
                    .add(u.scale(-(step * 0.5) * t.cos()))
                    .add(out_n.scale((step * 0.5) * t.sin()));
                out.push(p);
            }
        }
    }
    out
}

impl RevisionCloud {
    /// `[x0, y0, x1, y1]` of the rectangle the cloud surrounds.
    pub fn rect_bounds(&self) -> [f64; 4] {
        norm(self.rect.0, self.rect.1)
    }

    /// The scalloped outline (closed).
    pub fn outline(&self) -> Vec<Point> {
        cloud_outline(self.rect, CLOUD_BUMP_IN)
    }

    /// The revision tag: a triangle at the cloud's upper-left corner, and the
    /// point its mark is centred on. `None` without a revision mark.
    pub fn tag(&self) -> Option<([Point; 3], Point)> {
        if self.revision.trim().is_empty() {
            return None;
        }
        let r = self.rect_bounds();
        let s = 0.28;
        let base_y = r[3] + CLOUD_BUMP_IN * 0.5;
        let apex = Point::new(r[0] + s * 0.5, base_y + s * 0.9);
        let tri = [Point::new(r[0], base_y), Point::new(r[0] + s, base_y), apex];
        Some((tri, Point::new(r[0] + s * 0.5, base_y + s * 0.28)))
    }

    /// Bounds including the bumps and the tag.
    pub fn bounds(&self) -> [f64; 4] {
        let mut r = self.rect_bounds();
        let bump = CLOUD_BUMP_IN * 0.5;
        r = [r[0] - bump, r[1] - bump, r[2] + bump, r[3] + bump];
        if let Some((tri, _)) = self.tag() {
            for p in tri {
                r[3] = r[3].max(p.y);
            }
        }
        r
    }
}

fn dist_to_polyline(p: Point, pts: &[Point], closed: bool) -> f64 {
    let n = pts.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    (0..segs)
        .map(|i| plan_core::geometry::dist_to_segment(p, pts[i], pts[(i + 1) % n]))
        .fold(f64::INFINITY, f64::min)
}

/// How far `(x, y)` is from page CAD `o` (0 inside text).
fn cad_distance(o: &CadObject, x: f64, y: f64) -> f64 {
    let p = Point::new(x, y);
    match &o.item {
        CadItem::Line { a, b } => plan_core::geometry::dist_to_segment(p, *a, *b),
        CadItem::Polyline { points, closed } => dist_to_polyline(p, points, *closed),
        CadItem::Circle { center, radius } => (p.sub(*center).length() - radius).abs(),
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(TAU);
            let a = p.sub(*center).angle();
            let rel = (a - start_angle).rem_euclid(TAU);
            if rel <= sweep {
                (p.sub(*center).length() - radius).abs()
            } else {
                let at =
                    |t: f64| Point::new(center.x + radius * t.cos(), center.y + radius * t.sin());
                p.dist(at(*start_angle)).min(p.dist(at(*end_angle)))
            }
        }
        CadItem::Text { .. } => {
            let (lo, hi) = o.bounds();
            let dx = (lo.x - x).max(0.0).max(x - hi.x);
            let dy = (lo.y - y).max(0.0).max(y - hi.y);
            dx.hypot(dy)
        }
    }
}

impl LayoutPage {
    /// Adds a leader and returns its id. `elbow` is where the line meets the text.
    pub fn add_leader(&mut self, tip: Point, elbow: Point, text: &str, height_in: f64) -> Id {
        let id = self.next_cad_id();
        self.leaders.push(PageLeader {
            id,
            tip,
            elbow,
            text: text.to_string(),
            height_in,
            arrow: true,
        });
        id
    }

    /// Adds a revision cloud around the rectangle `a`-`b` and returns its id.
    pub fn add_cloud(&mut self, a: Point, b: Point, revision: &str) -> Id {
        let id = self.next_cad_id();
        self.clouds.push(RevisionCloud {
            id,
            rect: (a, b),
            revision: revision.to_string(),
        });
        id
    }

    /// An arc on the page: the counter-clockwise sweep from `start` to `end` (radians).
    pub fn add_arc(&mut self, center: Point, radius: f64, start: f64, end: f64) -> Id {
        self.add_cad(CadItem::Arc {
            center,
            radius,
            start_angle: start,
            end_angle: end,
        })
    }

    /// A circle on the page.
    pub fn add_circle(&mut self, center: Point, radius: f64) -> Id {
        self.add_cad(CadItem::Circle { center, radius })
    }

    /// What `id` names on this page.
    pub fn annotation_kind(&self, id: Id) -> Option<AnnotationKind> {
        if self.cad.iter().any(|o| o.id == id) {
            Some(AnnotationKind::Cad)
        } else if self.leaders.iter().any(|l| l.id == id) {
            Some(AnnotationKind::Leader)
        } else if self.clouds.iter().any(|c| c.id == id) {
            Some(AnnotationKind::Cloud)
        } else {
            None
        }
    }

    /// `[x0, y0, x1, y1]` of annotation `id`.
    pub fn annotation_bounds(&self, id: Id) -> Option<[f64; 4]> {
        match self.annotation_kind(id)? {
            AnnotationKind::Cad => {
                let (lo, hi) = self.cad.iter().find(|o| o.id == id)?.bounds();
                Some([lo.x, lo.y, hi.x, hi.y])
            }
            AnnotationKind::Leader => self.leaders.iter().find(|l| l.id == id).map(|l| l.bounds()),
            AnnotationKind::Cloud => self.clouds.iter().find(|c| c.id == id).map(|c| c.bounds()),
        }
    }

    /// The annotation nearest `(x, y)` within `tol` paper inches. Leaders
    /// and clouds win over CAD when they are as near.
    pub fn annotation_at(&self, x: f64, y: f64, tol: f64) -> Option<Id> {
        let p = Point::new(x, y);
        let mut best: Option<(Id, f64)> = None;
        let mut offer = |id: Id, d: f64| {
            if d <= tol && best.is_none_or(|(_, b)| d <= b) {
                best = Some((id, d));
            }
        };
        for o in &self.cad {
            offer(o.id, cad_distance(o, x, y));
        }
        for l in &self.leaders {
            let line = dist_to_polyline(p, &[l.tip, l.elbow], false);
            let b = l.bounds();
            let tp = l.text_pos();
            let tb = [tp.x, tp.y, b[2], b[3]];
            let dx = (tb[0] - x).max(0.0).max(x - tb[2]);
            let dy = (tb[1] - y).max(0.0).max(y - tb[3]);
            offer(l.id, line.min(dx.hypot(dy)));
        }
        for c in &self.clouds {
            offer(c.id, dist_to_polyline(p, &c.outline(), true));
        }
        best.map(|(id, _)| id)
    }

    /// Moves annotation `id` by `(dx, dy)` paper inches.
    pub fn move_annotation(&mut self, id: Id, dx: f64, dy: f64) -> bool {
        let d = Point::new(dx, dy);
        match self.annotation_kind(id) {
            Some(AnnotationKind::Cad) => {
                if let Some(o) = self.cad.iter_mut().find(|o| o.id == id) {
                    map_item(&mut o.item, &|p| p.add(d), 1.0, 1.0);
                }
                true
            }
            Some(AnnotationKind::Leader) => {
                if let Some(l) = self.leaders.iter_mut().find(|l| l.id == id) {
                    l.tip = l.tip.add(d);
                    l.elbow = l.elbow.add(d);
                }
                true
            }
            Some(AnnotationKind::Cloud) => {
                if let Some(c) = self.clouds.iter_mut().find(|c| c.id == id) {
                    c.rect = (c.rect.0.add(d), c.rect.1.add(d));
                }
                true
            }
            None => false,
        }
    }

    /// Resizes annotation `id` so its bounds become `r` (see
    /// [`annotation_bounds`](Self::annotation_bounds)): points scale with the
    /// bounds, text grows with the height, circles and arcs by the mean of the
    /// two scales. A cloud's rectangle never gets smaller than [`MIN_CLOUD_IN`].
    pub fn resize_annotation(&mut self, id: Id, r: [f64; 4]) -> bool {
        let Some(old) = self.annotation_bounds(id) else {
            return false;
        };
        let (ow, oh) = (old[2] - old[0], old[3] - old[1]);
        let sx = if ow > 1e-9 { (r[2] - r[0]) / ow } else { 1.0 };
        let sy = if oh > 1e-9 { (r[3] - r[1]) / oh } else { 1.0 };
        let map = move |p: Point| {
            Point::new(
                if ow > 1e-9 {
                    r[0] + (p.x - old[0]) * sx
                } else {
                    p.x + (r[0] - old[0])
                },
                if oh > 1e-9 {
                    r[1] + (p.y - old[1]) * sy
                } else {
                    p.y + (r[1] - old[1])
                },
            )
        };
        match self.annotation_kind(id) {
            Some(AnnotationKind::Cad) => {
                if let Some(o) = self.cad.iter_mut().find(|o| o.id == id) {
                    map_item(&mut o.item, &map, sx, sy);
                }
                true
            }
            Some(AnnotationKind::Leader) => {
                if let Some(l) = self.leaders.iter_mut().find(|l| l.id == id) {
                    l.tip = map(l.tip);
                    l.elbow = map(l.elbow);
                    l.height_in = (l.height_in * sy.abs()).clamp(0.03, 2.0);
                }
                true
            }
            Some(AnnotationKind::Cloud) => {
                if let Some(c) = self.clouds.iter_mut().find(|c| c.id == id) {
                    // The cloud's bounds hold bumps and the tag around its rectangle:
                    // keep that margin and resize the rectangle inside it.
                    let m = c.rect_bounds();
                    let b = c.bounds();
                    let (m0, m1) = (
                        Point::new(m[0] - b[0], m[1] - b[1]),
                        Point::new(b[2] - m[2], b[3] - m[3]),
                    );
                    let mut nr = [r[0] + m0.x, r[1] + m0.y, r[2] - m1.x, r[3] - m1.y];
                    nr[2] = nr[2].max(nr[0] + MIN_CLOUD_IN);
                    nr[3] = nr[3].max(nr[1] + MIN_CLOUD_IN);
                    c.rect = (Point::new(nr[0], nr[1]), Point::new(nr[2], nr[3]));
                }
                true
            }
            None => false,
        }
    }

    /// Removes annotation `id`.
    pub fn remove_annotation(&mut self, id: Id) -> bool {
        let n = self.cad.len() + self.leaders.len() + self.clouds.len();
        self.cad.retain(|o| o.id != id);
        self.leaders.retain(|l| l.id != id);
        self.clouds.retain(|c| c.id != id);
        n != self.cad.len() + self.leaders.len() + self.clouds.len()
    }
}

/// Applies `map` to every point of `item`; `sx`/`sy` are the scales it applies
/// (text height follows `sy`, radii the mean).
fn map_item(item: &mut CadItem, map: &dyn Fn(Point) -> Point, sx: f64, sy: f64) {
    match item {
        CadItem::Line { a, b } => {
            *a = map(*a);
            *b = map(*b);
        }
        CadItem::Polyline { points, .. } => {
            for p in points {
                *p = map(*p);
            }
        }
        CadItem::Circle { center, radius } => {
            *center = map(*center);
            *radius = (*radius * (sx.abs() + sy.abs()) * 0.5).max(0.01);
        }
        CadItem::Arc { center, radius, .. } => {
            *center = map(*center);
            *radius = (*radius * (sx.abs() + sy.abs()) * 0.5).max(0.01);
        }
        CadItem::Text { pos, height, .. } => {
            *pos = map(*pos);
            *height = (*height * sy.abs()).clamp(0.02, 4.0);
        }
    }
}

/// The arc through a centre, a start point and an end point: radius and
/// start angle from the start point, end angle from the direction of the end
/// point (counter-clockwise). `None` when the start point is the centre.
pub fn arc_from_points(center: Point, start: Point, end: Point) -> Option<CadItem> {
    let radius = start.dist(center);
    if radius < 1e-9 || end.dist(center) < 1e-9 {
        return None;
    }
    let a0 = start.sub(center).angle();
    let a1 = end.sub(center).angle();
    Some(CadItem::Arc {
        center,
        radius,
        start_angle: a0,
        end_angle: if (a1 - a0).rem_euclid(TAU) < 1e-9 {
            a0 + TAU - 1e-6
        } else {
            a1
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_docs::SheetSize;

    fn page() -> LayoutPage {
        let mut l = crate::Layout::new("t", SheetSize::ArchC);
        l.add_page(1, "p");
        l.pages.remove(0)
    }

    #[test]
    fn ids_are_shared_between_cad_leaders_and_clouds() {
        let mut p = page();
        let a = p.add_line(Point::new(1.0, 1.0), Point::new(3.0, 1.0));
        let b = p.add_leader(Point::new(5.0, 5.0), Point::new(6.0, 6.0), "NOTE", 0.125);
        let c = p.add_cloud(Point::new(1.0, 2.0), Point::new(3.0, 4.0), "1");
        let d = p.add_circle(Point::new(8.0, 8.0), 0.5);
        let ids = [a, b, c, d];
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 4, "{ids:?}");
        assert_eq!(p.annotation_kind(b), Some(AnnotationKind::Leader));
        assert_eq!(p.annotation_kind(c), Some(AnnotationKind::Cloud));
        assert_eq!(p.annotation_kind(a), Some(AnnotationKind::Cad));
        assert_eq!(p.annotation_kind(999), None);
    }

    #[test]
    fn moving_translates_every_kind() {
        let mut p = page();
        let l = p.add_line(Point::new(1.0, 1.0), Point::new(3.0, 1.0));
        let c = p.add_circle(Point::new(8.0, 8.0), 0.5);
        let ld = p.add_leader(Point::new(5.0, 5.0), Point::new(6.0, 6.0), "NOTE", 0.125);
        let cl = p.add_cloud(Point::new(1.0, 2.0), Point::new(3.0, 4.0), "");
        for id in [l, c, ld, cl] {
            assert!(p.move_annotation(id, 2.0, -1.0));
        }
        assert_eq!(
            p.cad[0].item,
            CadItem::Line {
                a: Point::new(3.0, 0.0),
                b: Point::new(5.0, 0.0)
            }
        );
        assert_eq!(
            p.cad[1].item,
            CadItem::Circle {
                center: Point::new(10.0, 7.0),
                radius: 0.5
            }
        );
        assert_eq!(p.leaders[0].tip, Point::new(7.0, 4.0));
        assert_eq!(p.leaders[0].elbow, Point::new(8.0, 5.0));
        assert_eq!(p.clouds[0].rect.0, Point::new(3.0, 1.0));
        assert!(!p.move_annotation(77, 1.0, 1.0));
    }

    #[test]
    fn resizing_scales_lines_circles_text_and_leaders_with_the_bounds() {
        let mut p = page();
        let l = p.add_line(Point::new(1.0, 1.0), Point::new(3.0, 2.0));
        assert!(p.resize_annotation(l, [1.0, 1.0, 5.0, 3.0]));
        assert_eq!(
            p.cad[0].item,
            CadItem::Line {
                a: Point::new(1.0, 1.0),
                b: Point::new(5.0, 3.0)
            }
        );
        let c = p.add_circle(Point::new(10.0, 10.0), 1.0);
        // Bounds are 9..11; doubling them doubles the radius.
        assert!(p.resize_annotation(c, [8.0, 8.0, 12.0, 12.0]));
        let CadItem::Circle { radius, center } = p.cad[1].item.clone() else {
            panic!()
        };
        assert!((radius - 2.0).abs() < 1e-9 && (center.x - 10.0).abs() < 1e-9);
        let t = p.add_text(Point::new(2.0, 8.0), "ABCD", 0.2);
        let b = p.annotation_bounds(t).unwrap();
        assert!(p.resize_annotation(t, [b[0], b[1], b[2], b[1] + (b[3] - b[1]) * 2.0]));
        let CadItem::Text { height, .. } = p.cad[2].item else {
            panic!()
        };
        assert!((height - 0.4).abs() < 1e-9);
        let ld = p.add_leader(Point::new(5.0, 5.0), Point::new(6.0, 6.0), "N", 0.1);
        let b = p.annotation_bounds(ld).unwrap();
        assert!(p.resize_annotation(ld, [b[0], b[1], b[0] + 2.0 * (b[2] - b[0]), b[3]]));
        assert!(p.leaders[0].tip.x <= 5.0 + 1e-9);
        let cl = p.add_cloud(Point::new(1.0, 5.0), Point::new(2.0, 6.0), "");
        let b = p.annotation_bounds(cl).unwrap();
        assert!(p.resize_annotation(cl, [b[0], b[1], b[2] + 1.0, b[3] + 1.0]));
        let r = p.clouds[0].rect_bounds();
        assert!(
            (r[2] - 3.0).abs() < 1e-9 && (r[3] - 7.0).abs() < 1e-9,
            "{r:?}"
        );
    }

    #[test]
    fn hit_testing_finds_the_nearest_annotation() {
        let mut p = page();
        let l = p.add_line(Point::new(1.0, 1.0), Point::new(5.0, 1.0));
        let c = p.add_circle(Point::new(10.0, 10.0), 1.0);
        let ld = p.add_leader(
            Point::new(3.0, 8.0),
            Point::new(4.0, 9.0),
            "SEE PLAN",
            0.125,
        );
        let cl = p.add_cloud(Point::new(12.0, 2.0), Point::new(15.0, 5.0), "2");
        assert_eq!(p.annotation_at(3.0, 1.02, 0.1), Some(l));
        assert_eq!(p.annotation_at(11.0, 10.0, 0.1), Some(c));
        assert_eq!(p.annotation_at(3.5, 8.5, 0.1), Some(ld));
        assert_eq!(p.annotation_at(13.5, 1.9, 0.3), Some(cl));
        assert_eq!(p.annotation_at(20.0, 20.0, 0.1), None);
        assert!(p.remove_annotation(cl));
        assert!(!p.remove_annotation(cl));
    }

    #[test]
    fn a_leader_has_a_line_a_landing_and_an_arrowhead_at_the_tip() {
        let l = PageLeader {
            id: 1,
            tip: Point::new(1.0, 1.0),
            elbow: Point::new(2.0, 2.0),
            text: "KEYNOTE".into(),
            height_in: 0.125,
            arrow: true,
        };
        let lines = l.polylines();
        assert_eq!(lines[0][0], l.tip);
        assert_eq!(lines[0][1], l.elbow);
        assert!(lines[0][2].x > l.elbow.x && (lines[0][2].y - l.elbow.y).abs() < 1e-12);
        let a = l.arrowhead().unwrap();
        assert_eq!(a[0], l.tip);
        let plain = PageLeader {
            arrow: false,
            ..l.clone()
        };
        assert!(plain.arrowhead().is_none());
        assert!(l.text_pos().y > l.elbow.y);
    }

    #[test]
    fn a_cloud_is_a_scalloped_loop_outside_its_rectangle() {
        let rect = (Point::new(0.0, 0.0), Point::new(3.0, 2.0));
        let o = cloud_outline(rect, 0.3);
        assert!(o.len() > 40);
        let (mut lo, mut hi) = (Point::new(9.0, 9.0), Point::new(-9.0, -9.0));
        for p in &o {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        // The bumps stick out of the rectangle by about half a bump.
        assert!(
            lo.x < -0.1 && lo.y < -0.1 && hi.x > 3.1 && hi.y > 2.1,
            "{lo:?} {hi:?}"
        );
        assert!(lo.x > -0.3 && hi.x < 3.3);
        let c = RevisionCloud {
            id: 1,
            rect,
            revision: "1".into(),
        };
        assert!(c.tag().is_some());
        assert!(RevisionCloud {
            revision: " ".into(),
            ..c.clone()
        }
        .tag()
        .is_none());
        assert!(c.bounds()[3] > 2.0 + CLOUD_BUMP_IN * 0.5);
    }

    #[test]
    fn arcs_come_from_a_centre_a_start_and_an_end() {
        let a = arc_from_points(Point::ZERO, Point::new(2.0, 0.0), Point::new(0.0, 5.0)).unwrap();
        let CadItem::Arc {
            radius,
            start_angle,
            end_angle,
            ..
        } = a
        else {
            panic!()
        };
        assert!((radius - 2.0).abs() < 1e-12 && start_angle.abs() < 1e-12);
        assert!((end_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(arc_from_points(Point::ZERO, Point::ZERO, Point::new(1.0, 0.0)).is_none());
        let mut p = page();
        let id = p.add_arc(Point::ZERO, 1.0, 0.0, 1.0);
        assert!(p.annotation_bounds(id).is_some());
    }
}
