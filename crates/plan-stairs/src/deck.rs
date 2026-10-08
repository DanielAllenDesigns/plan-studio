//! Decks: decking boards clipped to a polygon, the framing below, posts to
//! grade, railings on selected edges and the plan symbol.
//!
//! All lengths are inches; grade is elevation 0 and `elevation` is the top of
//! the boards. Scene space is X right, Y up, Z = -plan y (see `plan-3d`).

use crate::railing::{bar, railing_segments, run_meshes, RailingGeometry, RailingParams};
use crate::Stroke;
use plan_3d::{Material, Mesh};
use plan_core::Point;

/// Decking thickness (5/4 board).
const BOARD_THICKNESS: f64 = 1.0;
/// Joist and rim joist thickness (2x).
const JOIST_THICKNESS: f64 = 1.5;
/// Joist depth (2x8).
const JOIST_DEPTH: f64 = 7.25;
/// Drop beam width (doubled 2x).
const BEAM_WIDTH: f64 = 3.0;
/// Drop beam depth (2x10).
const BEAM_DEPTH: f64 = 9.25;
/// Post size (6x6).
const POST_SIZE: f64 = 5.5;
/// Posts closer than this across the deck belong to the same beam line.
const BEAM_ROW_TOLERANCE: f64 = 6.0;

/// A deck: its outline, height and framing.
#[derive(Debug, Clone, PartialEq)]
pub struct Deck {
    /// Outline in plan (either winding; may be concave).
    pub polygon: Vec<Point>,
    /// Elevation of the top of the boards above grade.
    pub elevation: f64,
    /// Board face width.
    pub board_width: f64,
    /// Gap between boards.
    pub board_gap: f64,
    /// Direction the boards run, degrees counter-clockwise from plan +X.
    pub board_direction_deg: f64,
    /// Joist spacing, on centre.
    pub joist_spacing: f64,
    /// Carry the joists on a drop beam through each row of posts.
    pub beam: bool,
    /// Post centres in plan.
    pub posts: Vec<Point>,
}

impl Deck {
    /// A deck with 5-1/2" boards at 1/8" gaps, 16" joists and no posts.
    pub fn new(polygon: Vec<Point>, elevation: f64) -> Self {
        Self {
            polygon,
            elevation,
            board_width: 5.5,
            board_gap: 0.125,
            board_direction_deg: 0.0,
            joist_spacing: 16.0,
            beam: false,
            posts: Vec::new(),
        }
    }

    /// Unit vectors `(along the boards, across the boards)`.
    fn axes(&self) -> (Point, Point) {
        let a = self.board_direction_deg.to_radians();
        let u = Point::new(a.cos(), a.sin());
        (u, u.perp())
    }

    /// Outline of every decking board in plan, clipped to the polygon.
    ///
    /// The last board in each run is ripped to the polygon edge. Boards end on
    /// the polygon's edges, so a slanted edge gives a slanted board end.
    pub fn boards(&self) -> Vec<Vec<Point>> {
        let (u, v) = self.axes();
        let mut out = Vec::new();
        let Some((lo, hi)) = span(&self.polygon, v) else {
            return out;
        };
        let (width, pitch) = (
            self.board_width.max(0.1),
            self.board_width.max(0.1) + self.board_gap.max(0.0),
        );
        let to_plan = |a: f64, b: f64| u * a + v * b;
        let mut k = 0.0;
        loop {
            let c0 = lo + k * pitch;
            if c0 >= hi - 1e-6 {
                break;
            }
            k += 1.0;
            let c1 = (c0 + width).min(hi);
            if c1 - c0 < 0.01 {
                continue;
            }
            let eps = ((c1 - c0) / 4.0).min(1e-4);
            let at0 = intervals(&self.polygon, u, v, c0 + eps);
            let at1 = intervals(&self.polygon, u, v, c1 - eps);
            if at0.len() == at1.len() {
                for (p, q) in at0.iter().zip(&at1) {
                    out.push(vec![
                        to_plan(p.0, c0),
                        to_plan(p.1, c0),
                        to_plan(q.1, c1),
                        to_plan(q.0, c1),
                    ]);
                }
            } else {
                for p in intervals(&self.polygon, u, v, (c0 + c1) / 2.0) {
                    out.push(vec![
                        to_plan(p.0, c0),
                        to_plan(p.1, c0),
                        to_plan(p.1, c1),
                        to_plan(p.0, c1),
                    ]);
                }
            }
        }
        out
    }

    /// Depth of the joists: full 2x8 unless the deck is lower than that.
    fn joist_depth(&self) -> f64 {
        JOIST_DEPTH.min((self.elevation - BOARD_THICKNESS).max(0.0))
    }

    /// Posts grouped into rows across the deck (rows of two or more carry a beam):
    /// `(across coordinate, min along, max along, member indices)`.
    fn beam_rows(&self) -> Vec<(f64, f64, f64, Vec<usize>)> {
        let (u, v) = self.axes();
        let mut order: Vec<usize> = (0..self.posts.len()).collect();
        order.sort_by(|&i, &j| self.posts[i].dot(v).total_cmp(&self.posts[j].dot(v)));
        let mut rows: Vec<(f64, f64, f64, Vec<usize>)> = Vec::new();
        for i in order {
            let (a, b) = (self.posts[i].dot(u), self.posts[i].dot(v));
            match rows.last_mut() {
                Some(r) if (b - r.0).abs() <= BEAM_ROW_TOLERANCE => {
                    r.1 = r.1.min(a);
                    r.2 = r.2.max(a);
                    r.3.push(i);
                }
                _ => rows.push((b, a, a, vec![i])),
            }
        }
        rows.retain(|r| r.3.len() >= 2);
        rows
    }

    /// 3D meshes: boards, rim joists, joists, optional drop beams and posts.
    pub fn deck_meshes(&self) -> Vec<Mesh> {
        let mut out = Vec::new();
        let top = self.elevation;
        let sc = |p: Point, y: f64| [p.x, y, -p.y];

        // Decking: each board is a thin convex slab.
        for b in self.boards() {
            let profile: Vec<[f64; 3]> = b.iter().map(|&p| sc(p, top - BOARD_THICKNESS)).collect();
            out.push(crate::model3d::solid(
                &profile,
                [0.0, BOARD_THICKNESS, 0.0],
                Material::Floor,
                None,
            ));
        }

        let depth = self.joist_depth();
        let joist_top = top - BOARD_THICKNESS;
        let joist_bottom = joist_top - depth;
        let (u, v) = self.axes();
        if depth > 0.01 {
            let mid = joist_top - depth / 2.0;
            let size = (JOIST_THICKNESS, depth);
            // Rim joists on every edge.
            let n = self.polygon.len();
            for i in 0..n {
                let (a, b) = (self.polygon[i], self.polygon[(i + 1) % n]);
                let lat = (b - a).perp();
                out.extend(bar(
                    sc(a, mid),
                    sc(b, mid),
                    sc(lat, 0.0),
                    size,
                    Material::Floor,
                    None,
                ));
            }
            // Joists across the boards, clipped to the polygon.
            if let Some((lo, hi)) = span(&self.polygon, u) {
                let spacing = self.joist_spacing.max(JOIST_THICKNESS * 2.0);
                let mut positions = Vec::new();
                let mut a = lo + JOIST_THICKNESS / 2.0;
                while a < hi - JOIST_THICKNESS / 2.0 - 1e-6 {
                    positions.push(a);
                    a += spacing;
                }
                positions.push(hi - JOIST_THICKNESS / 2.0);
                for a in positions {
                    for (b0, b1) in intervals(&self.polygon, v, u, a) {
                        let (p, q) = (u * a + v * b0, u * a + v * b1);
                        out.extend(bar(
                            sc(p, mid),
                            sc(q, mid),
                            sc(u, 0.0),
                            size,
                            Material::Floor,
                            None,
                        ));
                    }
                }
            }
        }

        // Drop beams through post rows, then posts down to grade.
        let rows = if self.beam && depth > 0.01 {
            self.beam_rows()
        } else {
            Vec::new()
        };
        let mut post_top = vec![joist_bottom; self.posts.len()];
        let beam_bottom = (joist_bottom - BEAM_DEPTH).max(0.0);
        for (b, a0, a1, members) in &rows {
            let mid = (joist_bottom + beam_bottom) / 2.0;
            let (p, q) = (u * *a0 + v * *b, u * *a1 + v * *b);
            out.extend(bar(
                sc(p, mid),
                sc(q, mid),
                sc(v, 0.0),
                (BEAM_WIDTH, joist_bottom - beam_bottom),
                Material::Floor,
                None,
            ));
            for &i in members {
                post_top[i] = beam_bottom;
            }
        }
        for (p, &t) in self.posts.iter().zip(&post_top) {
            if t > 0.01 {
                out.extend(bar(
                    sc(*p, 0.0),
                    sc(*p, t),
                    sc(u, 0.0),
                    (POST_SIZE, POST_SIZE),
                    Material::Floor,
                    None,
                ));
            }
        }
        out
    }

    /// Meshes of railings on the given polygon edges, standing on the deck.
    pub fn edge_railing_meshes(&self, open_edges: &[usize], params: &RailingParams) -> Vec<Mesh> {
        let runs = edge_runs(&self.polygon, open_edges);
        run_meshes(&runs, self.elevation, params, None)
    }

    /// Plan symbol: the outline and one line along each board.
    pub fn plan_symbol_deck(&self) -> Vec<Stroke> {
        let mut out = Vec::new();
        if self.polygon.len() >= 3 {
            out.push(Stroke::Polyline(self.polygon.clone(), true));
        }
        for b in self.boards() {
            out.push(Stroke::Line(b[0], b[1]));
        }
        out
    }
}

/// `(start, end)` of each distinct, valid edge index.
fn edge_runs(polygon: &[Point], open_edges: &[usize]) -> Vec<(Point, Point)> {
    let n = polygon.len();
    let mut seen = Vec::new();
    let mut runs = Vec::new();
    for &i in open_edges {
        if i < n && !seen.contains(&i) {
            seen.push(i);
            runs.push((polygon[i], polygon[(i + 1) % n]));
        }
    }
    runs
}

/// Railing layout (newels, balusters, rails) for each selected polygon edge.
///
/// Edge `i` runs from `polygon[i]` to `polygon[(i + 1) % n]`; out-of-range and
/// repeated indices are ignored. Adjacent edges each place a newel at their
/// shared corner; [`Deck::edge_railing_meshes`] merges them.
pub fn deck_edge_railing(
    polygon: &[Point],
    open_edges: &[usize],
    params: &RailingParams,
) -> Vec<RailingGeometry> {
    edge_runs(polygon, open_edges)
        .into_iter()
        .map(|(a, b)| railing_segments(a, b, params))
        .collect()
}

/// Min and max of the polygon's projection on `axis`.
fn span(polygon: &[Point], axis: Point) -> Option<(f64, f64)> {
    let mut it = polygon.iter().map(|p| p.dot(axis));
    let first = it.next()?;
    Some(it.fold((first, first), |(lo, hi), x| (lo.min(x), hi.max(x))))
}

/// Inside intervals, as `(from, to)` along `along`, of the polygon cut by the
/// line where the `across` coordinate equals `at` (even-odd rule).
fn intervals(polygon: &[Point], along: Point, across: Point, at: f64) -> Vec<(f64, f64)> {
    let n = polygon.len();
    let mut xs = Vec::new();
    for i in 0..n {
        let (p, q) = (polygon[i], polygon[(i + 1) % n]);
        let (bp, bq) = (p.dot(across), q.dot(across));
        if (bp <= at) != (bq <= at) {
            let t = (at - bp) / (bq - bp);
            xs.push(p.dot(along) + (q.dot(along) - p.dot(along)) * t);
        }
    }
    xs.sort_by(f64::total_cmp);
    xs.as_chunks::<2>().0.iter().map(|c| (c[0], c[1])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RailStyle, RailingParams};
    use plan_core::geometry::polygon_area;

    fn rect(w: f64, h: f64) -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, h),
            Point::new(0.0, h),
        ]
    }

    fn assert_unit_normals(meshes: &[Mesh]) {
        assert!(!meshes.is_empty());
        for m in meshes {
            assert_eq!(m.indices.len() % 3, 0);
            assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
            for v in &m.vertices {
                let n = v.normal;
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                assert!((l - 1.0).abs() < 1e-4, "normal length {l}");
            }
        }
    }

    fn boards_area(d: &Deck) -> f64 {
        d.boards().iter().map(|b| polygon_area(b).abs()).sum()
    }

    #[test]
    fn twelve_by_ten_deck_boards_cover_the_area() {
        let deck = Deck::new(rect(144.0, 120.0), 36.0);
        let boards = deck.boards();
        // 120" across at a 5.625" pitch: 21 full boards and a ripped one.
        assert_eq!(boards.len(), 22);
        let area = 144.0 * 120.0;
        let boards_only = boards_area(&deck);
        assert!(boards_only <= area);
        assert!((area - boards_only) / area < 0.03, "{boards_only}");
        // Boards plus their gaps account for the whole deck to within 2%.
        let gaps = 21.0 * deck.board_gap * 144.0;
        assert!(((boards_only + gaps) - area).abs() / area < 0.02);
        for b in &boards {
            assert!(b
                .iter()
                .all(|p| p.x > -1e-6 && p.x < 144.0 + 1e-6 && p.y > -1e-6 && p.y < 120.0 + 1e-6));
        }
    }

    #[test]
    fn boards_clip_to_rotated_and_concave_outlines() {
        let mut deck = Deck::new(rect(144.0, 120.0), 36.0);
        deck.board_direction_deg = 90.0;
        let area = 144.0 * 120.0;
        assert!((area - boards_area(&deck)) / area < 0.03);

        // L-shaped deck: 144x120 minus a 48x48 notch.
        let l = vec![
            Point::new(0.0, 0.0),
            Point::new(144.0, 0.0),
            Point::new(144.0, 72.0),
            Point::new(96.0, 72.0),
            Point::new(96.0, 120.0),
            Point::new(0.0, 120.0),
        ];
        let deck = Deck::new(l.clone(), 36.0);
        let area = polygon_area(&l).abs();
        let cover = boards_area(&deck);
        assert!(cover <= area + 1e-6);
        assert!((area - cover) / area < 0.03, "{cover} of {area}");

        // 45 degree boards on a rectangle stay inside it.
        let mut deck = Deck::new(rect(144.0, 120.0), 36.0);
        deck.board_direction_deg = 45.0;
        let area = 144.0 * 120.0;
        let cover = boards_area(&deck);
        assert!(
            cover <= area + 1e-6 && (area - cover) / area < 0.04,
            "{cover}"
        );
    }

    #[test]
    fn deck_meshes_have_unit_normals_and_reach_grade() {
        let mut deck = Deck::new(rect(144.0, 120.0), 36.0);
        deck.beam = true;
        deck.posts = vec![
            Point::new(24.0, 100.0),
            Point::new(72.0, 100.0),
            Point::new(120.0, 100.0),
        ];
        let meshes = deck.deck_meshes();
        assert_unit_normals(&meshes);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for m in &meshes {
            let (l, h) = m.bounds().expect("non-empty");
            lo = lo.min(l[1]);
            hi = hi.max(h[1]);
        }
        assert!(lo.abs() < 1e-4, "posts reach grade: {lo}");
        assert!((hi - 36.0).abs() < 1e-4, "top of boards: {hi}");
        // Boards (22) + 4 rim + joists + beam + 3 posts.
        assert!(meshes.len() > 22 + 4 + 3);
    }

    #[test]
    fn beam_shortens_posts_and_low_decks_skip_framing() {
        let mut deck = Deck::new(rect(144.0, 120.0), 36.0);
        deck.posts = vec![Point::new(24.0, 100.0), Point::new(120.0, 100.0)];
        let without = deck.deck_meshes().len();
        deck.beam = true;
        let with = deck.deck_meshes().len();
        assert_eq!(with, without + 1);

        let low = Deck::new(rect(144.0, 120.0), 1.0);
        let meshes = low.deck_meshes();
        assert_eq!(meshes.len(), low.boards().len());
        assert_unit_normals(&meshes);
    }

    #[test]
    fn edge_railing_covers_selected_edges_only() {
        let poly = rect(144.0, 120.0);
        let p = RailingParams::default();
        let geoms = deck_edge_railing(&poly, &[0, 1, 1, 9], &p);
        assert_eq!(geoms.len(), 2);
        // 144" edge: 2 spans; 120" edge: 2 spans.
        assert_eq!(geoms[0].newels.len(), 3);
        assert_eq!(geoms[1].newels.len(), 3);

        let mut deck = Deck::new(poly, 36.0);
        deck.posts.clear();
        for style in [RailStyle::default(), RailStyle::Glass] {
            let params = RailingParams { style, ..p };
            let meshes = deck.edge_railing_meshes(&[0, 1], &params);
            assert_unit_normals(&meshes);
            let lo = meshes
                .iter()
                .filter_map(|m| m.bounds())
                .map(|b| b.0[1])
                .fold(f32::MAX, f32::min);
            assert!((lo - 36.0).abs() < 1e-4, "railing stands on the deck: {lo}");
        }
        // Shared corner newel is placed once: 5 newels, not 6.
        let params = RailingParams {
            style: RailStyle::Glass,
            newel: crate::NewelParams {
                cap: false,
                ..p.newel
            },
            ..p
        };
        let n_two = deck.edge_railing_meshes(&[0, 1], &params).len();
        let n_one = deck.edge_railing_meshes(&[0], &params).len();
        let n_other = deck.edge_railing_meshes(&[1], &params).len();
        assert_eq!(n_two + 1, n_one + n_other);
    }

    #[test]
    fn plan_symbol_has_outline_and_board_lines() {
        let deck = Deck::new(rect(144.0, 120.0), 36.0);
        let strokes = deck.plan_symbol_deck();
        assert!(matches!(&strokes[0], Stroke::Polyline(pts, true) if pts.len() == 4));
        assert_eq!(strokes.len(), 1 + deck.boards().len());
    }
}
