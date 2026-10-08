//! CAD to Walls: recognise pairs of parallel lines as walls.
//!
//! Chief's *CAD to Walls* turns the two face lines of a drawn wall into one
//! wall object. [`cad_to_walls`] does the same: it pairs near-parallel
//! segments whose separation is a plausible wall thickness, takes the
//! centerline over their overlapping extent, then closes corners and
//! T-junctions between the proposed walls.

use plan_core::{Id, Point, Project, WallKind};

/// Tuning for [`cad_to_walls`]. Lengths are inches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CadToWallsOptions {
    /// Closest two lines may be to count as a wall.
    pub min_thickness: f64,
    /// Farthest two lines may be to count as a wall.
    pub max_thickness: f64,
    /// Shortest overlap of two lines that still makes a wall.
    pub min_length: f64,
    /// How far wall ends may be moved to meet another wall.
    pub snap_tolerance: f64,
    /// Walls at least this thick are [`WallKind::Exterior`].
    pub exterior_threshold: f64,
}

impl Default for CadToWallsOptions {
    fn default() -> Self {
        Self {
            min_thickness: 3.0,
            max_thickness: 14.0,
            min_length: 12.0,
            snap_tolerance: 1.0,
            exterior_threshold: 5.5,
        }
    }
}

/// A wall suggested from CAD lines (centerline plus measured thickness).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallProposal {
    pub start: Point,
    pub end: Point,
    pub thickness: f64,
    pub kind: WallKind,
}

/// Result of [`cad_to_walls`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CadToWallsResult {
    pub walls: Vec<WallProposal>,
    /// Line pieces that were not part of any wall (lone lines, and leftover
    /// stretches of at least `min_length`).
    pub unmatched: Vec<(Point, Point)>,
}

/// Maximum angle between two lines that still count as parallel.
const PARALLEL_TOL_DEG: f64 = 1.0;
/// Walls closer to parallel than this (sine of the angle) never form corners.
const MIN_CORNER_SINE: f64 = 0.087; // about 5 degrees
/// Slack so exact boundary values (3.0, 14.0, 12.0) are accepted.
const EPS: f64 = 1e-6;

/// A segment normalised to point "rightwards" (angle in `[0, pi]`), with the
/// stretches of it already claimed by a wall.
struct Seg {
    orig: (Point, Point),
    a: Point,
    dir: Point,
    len: f64,
    angle: f64,
    /// Claimed stretches as distances from `a` along `dir`.
    used: Vec<(f64, f64)>,
}

impl Seg {
    fn new(p: Point, q: Point) -> Seg {
        let mut dir = q.sub(p).normalized();
        let (mut a, mut b) = (p, q);
        if dir.y < -1e-9 || (dir.y.abs() <= 1e-9 && dir.x < 0.0) {
            std::mem::swap(&mut a, &mut b);
            dir = dir.scale(-1.0);
        }
        Seg {
            orig: (p, q),
            a,
            dir,
            len: a.dist(b),
            angle: dir.angle().clamp(0.0, std::f64::consts::PI),
            used: Vec::new(),
        }
    }

    fn at(&self, t: f64) -> Point {
        self.a.add(self.dir.scale(t))
    }

    /// Distance along this segment of the projection of `p`.
    fn param(&self, p: Point) -> f64 {
        p.sub(self.a).dot(self.dir)
    }

    /// Point on this segment's infinite line nearest to `p`.
    fn foot(&self, p: Point) -> Point {
        self.at(self.param(p))
    }
}

/// Two segments that could form a wall.
struct Candidate {
    /// Reference (longer) and partner segment indices.
    r: usize,
    p: usize,
    sep: f64,
    overlap: f64,
}

/// Find walls in a set of line segments. See the module docs for the method.
///
/// Pairing is greedy by separation: each segment pairs with its closest
/// parallel neighbour first, and a stretch of a segment is only used once, so
/// a long face line can serve several shorter ones (for example on both sides
/// of a door opening).
pub fn cad_to_walls(lines: &[(Point, Point)], opts: &CadToWallsOptions) -> CadToWallsResult {
    let mut segs: Vec<Seg> = lines
        .iter()
        .filter(|(a, b)| a.dist(*b) > EPS)
        .map(|&(a, b)| Seg::new(a, b))
        .collect();

    let mut candidates = candidates(&segs, opts);
    candidates.sort_by(|x, y| {
        x.sep
            .total_cmp(&y.sep)
            .then(y.overlap.total_cmp(&x.overlap))
            .then((x.r, x.p).cmp(&(y.r, y.p)))
    });

    let mut walls = Vec::new();
    for c in &candidates {
        pair_into_walls(&mut segs, c, opts, &mut walls);
    }

    let unmatched = leftovers(&segs, opts.min_length);
    let mut walls: Vec<WallProposal> = walls
        .into_iter()
        .map(|(start, end, thickness)| WallProposal {
            start,
            end,
            thickness,
            kind: if thickness >= opts.exterior_threshold - EPS {
                WallKind::Exterior
            } else {
                WallKind::Interior
            },
        })
        .collect();
    close_corners(&mut walls, opts.snap_tolerance);
    CadToWallsResult { walls, unmatched }
}

/// Add proposals to `floor` of `project` as walls of the given `height`;
/// returns the new wall ids in proposal order.
pub fn apply_walls(
    project: &mut Project,
    floor: usize,
    proposals: &[WallProposal],
    height: f64,
) -> Vec<Id> {
    proposals
        .iter()
        .map(|w| project.add_wall(floor, w.start, w.end, w.thickness, height, w.kind))
        .collect()
}

/// Every near-parallel pair whose separation and overlap look like a wall.
fn candidates(segs: &[Seg], opts: &CadToWallsOptions) -> Vec<Candidate> {
    let tol = PARALLEL_TOL_DEG.to_radians();
    let mut order: Vec<usize> = (0..segs.len()).collect();
    order.sort_by(|&x, &y| segs[x].angle.total_cmp(&segs[y].angle));

    let mut out = Vec::new();
    let mut consider = |i: usize, j: usize| {
        let (r, p) = if segs[i].len >= segs[j].len {
            (i, j)
        } else {
            (j, i)
        };
        let (sr, sp) = (&segs[r], &segs[p]);
        let Some((lo, hi)) = overlap_on(sr, sp) else {
            return;
        };
        let sep = sp.foot(sr.at((lo + hi) * 0.5)).dist(sr.at((lo + hi) * 0.5));
        if hi - lo >= opts.min_length - EPS
            && sep >= opts.min_thickness - EPS
            && sep <= opts.max_thickness + EPS
        {
            out.push(Candidate {
                r,
                p,
                sep,
                overlap: hi - lo,
            });
        }
    };
    // Sliding window over the angle-sorted segments keeps this near-linear.
    for (k, &i) in order.iter().enumerate() {
        for &j in &order[k + 1..] {
            if segs[j].angle - segs[i].angle > tol {
                break;
            }
            consider(i, j);
        }
        // Angles near 0 and near pi are also parallel.
        if segs[i].angle < tol {
            for &j in order.iter().rev() {
                if segs[i].angle + std::f64::consts::PI - segs[j].angle > tol {
                    break;
                }
                if j != i {
                    consider(i, j);
                }
            }
        }
    }
    out
}

/// Overlap of `p` with `r` as a range of distances along `r`.
fn overlap_on(r: &Seg, p: &Seg) -> Option<(f64, f64)> {
    let (t0, t1) = (r.param(p.a), r.param(p.at(p.len)));
    let lo = t0.min(t1).max(0.0);
    let hi = t0.max(t1).min(r.len);
    (hi > lo).then_some((lo, hi))
}

/// Subtract `used` ranges from `[lo, hi]`, returning what is left.
fn free_ranges(lo: f64, hi: f64, used: &mut [(f64, f64)]) -> Vec<(f64, f64)> {
    used.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut cursor = lo;
    for &(u0, u1) in used.iter() {
        if u0 > cursor {
            out.push((cursor, u0.min(hi)));
        }
        cursor = cursor.max(u1);
        if cursor >= hi {
            break;
        }
    }
    if cursor < hi {
        out.push((cursor, hi));
    }
    out.retain(|(a, b)| b > a);
    out
}

/// Turn the unclaimed overlap of one candidate into walls and claim it.
fn pair_into_walls(
    segs: &mut [Seg],
    c: &Candidate,
    opts: &CadToWallsOptions,
    walls: &mut Vec<(Point, Point, f64)>,
) {
    let Some((lo, hi)) = overlap_on(&segs[c.r], &segs[c.p]) else {
        return;
    };
    // Everything already claimed on either segment, in the reference's frame.
    let mut used = segs[c.r].used.clone();
    used.extend(segs[c.p].used.iter().map(|&(u0, u1)| {
        let (a, b) = (
            segs[c.r].param(segs[c.p].at(u0)),
            segs[c.r].param(segs[c.p].at(u1)),
        );
        (a.min(b), a.max(b))
    }));

    for (flo, fhi) in free_ranges(lo, hi, &mut used) {
        if fhi - flo < opts.min_length - EPS {
            continue;
        }
        let (r, p) = (&segs[c.r], &segs[c.p]);
        let (p0, p1) = (r.at(flo), r.at(fhi));
        let (q0, q1) = (p.foot(p0), p.foot(p1));
        let thickness = (p0.dist(q0) + p1.dist(q1)) * 0.5;
        if thickness < opts.min_thickness - EPS || thickness > opts.max_thickness + EPS {
            continue;
        }
        walls.push((
            Point::lerp(p0, q0, 0.5),
            Point::lerp(p1, q1, 0.5),
            thickness,
        ));
        let (u0, u1) = (p.param(q0), p.param(q1));
        segs[c.r].used.push((flo, fhi));
        segs[c.p].used.push((u0.min(u1), u0.max(u1)));
    }
}

/// Lone segments, plus unclaimed stretches of partly used ones that are
/// long enough to be a wall on their own.
fn leftovers(segs: &[Seg], min_length: f64) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    for s in segs {
        if s.used.is_empty() {
            out.push(s.orig);
            continue;
        }
        let mut used = s.used.clone();
        for (a, b) in free_ranges(0.0, s.len, &mut used) {
            if b - a >= min_length - EPS {
                out.push((s.at(a), s.at(b)));
            }
        }
    }
    out
}

/// Which end of a proposal.
#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Start,
    End,
}

/// Move wall ends so corners and T-junctions close.
///
/// A wall end moves to the intersection of its centerline with another
/// wall's centerline when it stops short of (or slightly overshoots) that
/// wall's face, and that intersection lies on the other wall. Remaining ends
/// within `tol` of each other are merged at their midpoint.
fn close_corners(walls: &mut [WallProposal], tol: f64) {
    let moves = corner_moves(walls, tol);
    for (idx, end, target) in moves {
        match end {
            End::Start => walls[idx].start = target,
            End::End => walls[idx].end = target,
        }
    }
    merge_close_ends(walls, tol);
}

fn corner_moves(walls: &[WallProposal], tol: f64) -> Vec<(usize, End, Point)> {
    let mut moves = Vec::new();
    for (ia, a) in walls.iter().enumerate() {
        let da = a.end.sub(a.start).normalized();
        for (end, origin, out) in [(End::Start, a.start, da.scale(-1.0)), (End::End, a.end, da)] {
            // (|distance to move|, intersection) of the best other wall.
            let mut best: Option<(f64, Point)> = None;
            for (ib, b) in walls.iter().enumerate() {
                if ia == ib {
                    continue;
                }
                let db = b.end.sub(b.start).normalized();
                let denom = out.cross(db);
                if denom.abs() < MIN_CORNER_SINE {
                    continue;
                }
                let qp = b.start.sub(origin);
                let t = qp.cross(db) / denom; // along `out` from the end
                let s = qp.cross(out) / denom; // along `db` from b.start
                                               // Half of each wall's footprint, measured along the other wall.
                let reach_a = b.thickness * 0.5 / denom.abs() + tol;
                let reach_b = a.thickness * 0.5 / denom.abs() + tol;
                let on_b = s >= -reach_b && s <= b.start.dist(b.end) + reach_b;
                if t.abs() <= reach_a && on_b && best.is_none_or(|(d, _)| t.abs() < d) {
                    best = Some((t.abs(), origin.add(out.scale(t))));
                }
            }
            if let Some((_, target)) = best {
                moves.push((ia, end, target));
            }
        }
    }
    moves
}

/// Merge wall ends of different walls that lie within `tol` of each other.
fn merge_close_ends(walls: &mut [WallProposal], tol: f64) {
    let ends = |w: &WallProposal| [(End::Start, w.start), (End::End, w.end)];
    for i in 0..walls.len() {
        for j in i + 1..walls.len() {
            for (ei, pi) in ends(&walls[i]) {
                for (ej, pj) in ends(&walls[j]) {
                    let d = pi.dist(pj);
                    if d > 1e-9 && d <= tol {
                        let mid = Point::lerp(pi, pj, 0.5);
                        for (k, e) in [(i, ei), (j, ej)] {
                            match e {
                                End::Start => walls[k].start = mid,
                                End::End => walls[k].end = mid,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// Face lines of a rectangular box whose wall centerlines span `w` x `h`.
    fn box_faces(w: f64, h: f64, thickness: f64) -> Vec<(Point, Point)> {
        let t = thickness * 0.5;
        let rect = |o: f64| {
            let (x0, y0, x1, y1) = (-o, -o, w + o, h + o);
            [
                (pt(x0, y0), pt(x1, y0)),
                (pt(x1, y0), pt(x1, y1)),
                (pt(x1, y1), pt(x0, y1)),
                (pt(x0, y1), pt(x0, y0)),
            ]
        };
        let mut lines = rect(t).to_vec();
        lines.extend(rect(-t));
        lines
    }

    fn assert_closed_box(res: &CadToWallsResult) {
        assert_eq!(res.walls.len(), 4);
        for w in &res.walls {
            assert!((w.thickness - 6.0).abs() < 1e-9);
            assert_eq!(w.kind, WallKind::Exterior);
            for e in [w.start, w.end] {
                let neighbours = res
                    .walls
                    .iter()
                    .filter(|o| !std::ptr::eq(*o, w))
                    .filter(|o| o.start.dist(e) < 0.01 || o.end.dist(e) < 0.01)
                    .count();
                assert_eq!(neighbours, 1, "end {e:?} should meet exactly one wall");
            }
        }
    }

    #[test]
    fn twenty_by_ten_box_gives_four_closed_exterior_walls() {
        let res = cad_to_walls(&box_faces(240.0, 120.0, 6.0), &CadToWallsOptions::default());
        assert_closed_box(&res);
        assert!(res.unmatched.is_empty());
        // Corners sit on the centerline rectangle.
        let corners = [
            pt(0.0, 0.0),
            pt(240.0, 0.0),
            pt(240.0, 120.0),
            pt(0.0, 120.0),
        ];
        for c in corners {
            assert!(res
                .walls
                .iter()
                .any(|w| w.start.dist(c) < 0.01 || w.end.dist(c) < 0.01));
        }
    }

    #[test]
    fn box_faces_with_different_line_order_and_direction_still_work() {
        let mut lines = box_faces(240.0, 120.0, 6.0);
        lines.reverse();
        for l in &mut lines {
            *l = (l.1, l.0);
        }
        assert_closed_box(&cad_to_walls(&lines, &CadToWallsOptions::default()));
    }

    #[test]
    fn lone_line_is_unmatched() {
        let mut lines = box_faces(240.0, 120.0, 6.0);
        let lone = (pt(500.0, 500.0), pt(700.0, 500.0));
        lines.push(lone);
        let res = cad_to_walls(&lines, &CadToWallsOptions::default());
        assert_eq!(res.walls.len(), 4);
        assert_eq!(res.unmatched, vec![lone]);

        let only = cad_to_walls(&[lone], &CadToWallsOptions::default());
        assert!(only.walls.is_empty());
        assert_eq!(only.unmatched, vec![lone]);
    }

    #[test]
    fn interior_wall_meets_exterior_in_a_t() {
        let mut lines = box_faces(240.0, 120.0, 6.0);
        // 4.5" interior wall from the bottom wall up toward the top wall,
        // stopping at the faces (3" from each centerline).
        let (x, t) = (120.0, 2.25);
        lines.push((pt(x - t, 3.0), pt(x - t, 117.0)));
        lines.push((pt(x + t, 3.0), pt(x + t, 117.0)));
        let res = cad_to_walls(&lines, &CadToWallsOptions::default());
        assert_eq!(res.walls.len(), 5);
        let interior: Vec<_> = res
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Interior)
            .collect();
        assert_eq!(interior.len(), 1);
        let w = interior[0];
        assert!((w.thickness - 4.5).abs() < 1e-9);
        let (lo, hi) = if w.start.y < w.end.y {
            (w.start, w.end)
        } else {
            (w.end, w.start)
        };
        // Extended to the centerlines of the walls it abuts.
        assert!(lo.dist(pt(x, 0.0)) < 0.01 && hi.dist(pt(x, 120.0)) < 0.01);
    }

    #[test]
    fn long_face_line_serves_both_sides_of_a_gap() {
        // One long outer face, inner face broken by a 36" opening.
        let lines = vec![
            (pt(0.0, 0.0), pt(240.0, 0.0)),
            (pt(0.0, 5.0), pt(100.0, 5.0)),
            (pt(136.0, 5.0), pt(240.0, 5.0)),
        ];
        let res = cad_to_walls(&lines, &CadToWallsOptions::default());
        assert_eq!(res.walls.len(), 2);
        // The outer line's stretch across the gap is long enough to be reported.
        assert_eq!(res.unmatched.len(), 1);
        let (a, b) = res.unmatched[0];
        assert!((a.x - 100.0).abs() < 1e-9 && (b.x - 136.0).abs() < 1e-9);
    }

    #[test]
    fn lines_outside_thickness_range_do_not_pair() {
        let opts = CadToWallsOptions::default();
        let too_thin = [
            (pt(0.0, 0.0), pt(100.0, 0.0)),
            (pt(0.0, 1.0), pt(100.0, 1.0)),
        ];
        let too_thick = [
            (pt(0.0, 0.0), pt(100.0, 0.0)),
            (pt(0.0, 30.0), pt(100.0, 30.0)),
        ];
        let too_short = [(pt(0.0, 0.0), pt(8.0, 0.0)), (pt(0.0, 6.0), pt(8.0, 6.0))];
        for lines in [too_thin, too_thick, too_short] {
            let res = cad_to_walls(&lines, &opts);
            assert!(res.walls.is_empty());
            assert_eq!(res.unmatched.len(), 2);
        }
    }

    #[test]
    fn apply_walls_adds_to_the_project() {
        let res = cad_to_walls(&box_faces(240.0, 120.0, 6.0), &CadToWallsOptions::default());
        let mut p = Project::new("w");
        let ids = apply_walls(&mut p, 0, &res.walls, 96.0);
        assert_eq!(ids.len(), 4);
        assert_eq!(p.floors[0].walls.len(), 4);
        assert!(p.floors[0].walls.iter().all(|w| w.height == 96.0));
    }
}
