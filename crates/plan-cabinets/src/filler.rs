//! Fillers: strips that close the gap between a cabinet and a wall (or
//! another cabinet). [`fit_between`] sizes a filler to exactly the gap it
//! sits in.

use plan_core::geometry::{point_in_polygon, Point};

use crate::cabinet::{Cabinet, CabinetKind};
use crate::geom;

/// Largest gap a filler will close, inches (Chief's fillers are 1 to 6").
pub const MAX_FILLER_GAP: f64 = 12.0;
/// Inset of the search strip from the cabinet's back and front, inches, so
/// the wall the back sits on and the open front do not count as blockers.
const STRIP_INSET: f64 = 0.05;

/// The rectangle a wall occupies in plan: its centreline from `start` to
/// `end`, `thickness` wide.
pub fn wall_polygon(start: Point, end: Point, thickness: f64) -> Vec<Point> {
    let dir = end.sub(start).normalized();
    let n = dir.perp().scale(thickness / 2.0);
    vec![start.sub(n), end.sub(n), end.add(n), start.add(n)]
}

fn vertical_overlap(a: &Cabinet, b: &Cabinet) -> bool {
    a.elevation.max(b.elevation) < (a.elevation + a.height).min(b.elevation + b.height) - 0.5
}

/// The free stretch of `cab`'s own depth strip around its centre, as
/// `(left, right)` offsets along its width axis from its position; `None` on
/// a side that nothing blocks (a wall, or another cabinet at the same height,
/// blocks it). `None` for the whole when the centre lies inside an obstacle.
/// The resize handles snap to these edges.
pub fn run_bounds(
    cab: &Cabinet,
    others: &[Cabinet],
    walls: &[Vec<Point>],
) -> Option<(Option<f64>, Option<f64>)> {
    let u = Point::new(cab.angle.cos(), cab.angle.sin());
    let v = u.perp();
    let mut obstacles: Vec<Vec<Point>> = walls.to_vec();
    obstacles.extend(
        others
            .iter()
            .filter(|o| o.id != cab.id && !o.kind.is_custom() && vertical_overlap(o, cab))
            .map(Cabinet::footprint),
    );
    geom::free_span(
        &obstacles,
        cab.position,
        u,
        v,
        (STRIP_INSET, cab.depth - STRIP_INSET),
        cab.width / 2.0,
    )
    .ok()
}

/// [`run_bounds`] when both sides are blocked: the gap the cabinet sits in.
fn gap_bounds(cab: &Cabinet, others: &[Cabinet], walls: &[Vec<Point>]) -> Option<(f64, f64)> {
    let (left, right) = run_bounds(cab, others, walls)?;
    Some((left?, right?))
}

/// Resizes `filler` to fill the gap it sits in along its width axis.
///
/// The gap is the free stretch of the filler's own depth strip around its
/// centre, bounded on each side by a wall (`walls` are plan polygons, see
/// [`wall_polygon`]) or by another cabinet at the same height. When both
/// sides are bounded and the gap is between 0 and [`MAX_FILLER_GAP`] the
/// filler's position and width are set to match it exactly and the width is
/// returned; otherwise nothing changes and `None` comes back.
pub fn fit_between(filler: &mut Cabinet, others: &[Cabinet], walls: &[Vec<Point>]) -> Option<f64> {
    let (l, r) = gap_bounds(filler, others, walls)?;
    let gap = r - l;
    if gap <= 1e-6 || gap > MAX_FILLER_GAP {
        return None;
    }
    let u = Point::new(filler.angle.cos(), filler.angle.sin());
    filler.position = filler.position.add(u.scale(l));
    filler.width = gap;
    Some(gap)
}

/// Fit to gap (CB-5): a cabinet dragged between a wall and a cabinet (or
/// between two cabinets) whose gap is within `tolerance` inches of its own
/// width takes the gap's width and position exactly. Returns the new width, or
/// `None` (changing nothing) when the gap is open on a side or differs from
/// the width by more than `tolerance`.
pub fn fit_to_gap(
    cab: &mut Cabinet,
    others: &[Cabinet],
    walls: &[Vec<Point>],
    tolerance: f64,
) -> Option<f64> {
    let (l, r) = gap_bounds(cab, others, walls)?;
    let gap = r - l;
    if gap <= 1e-6 || (gap - cab.width).abs() > tolerance + 1e-9 {
        return None;
    }
    let u = Point::new(cab.angle.cos(), cab.angle.sin());
    cab.position = cab.position.add(u.scale(l));
    cab.width = gap;
    Some(gap)
}

// ----- automatic fillers (CB-480, CB-481, CB-482) -----

/// Largest gap an automatic filler closes, and how close cabinets must be to
/// merge, snap and align: 3 in (reference manual pp. 649 to 651).
pub const AUTO_FILLER_REACH: f64 = 3.0;
/// Fillers and gaps are rounded to the nearest 1/16 in.
const SIXTEENTH: f64 = 1.0 / 16.0;

/// The three families of cabinets that run side by side. Fillers and merging
/// only join cabinets of the same family and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunClass {
    Base,
    Wall,
    FullHeight,
}

impl RunClass {
    /// The filler kind of the family.
    pub fn filler_kind(self) -> CabinetKind {
        match self {
            RunClass::Base => CabinetKind::BaseFiller,
            RunClass::Wall => CabinetKind::WallFiller,
            RunClass::FullHeight => CabinetKind::FullHeightFiller,
        }
    }
}

/// The family of `c`; `None` for soffits, shelves, partitions and the
/// free-form tools, which never run.
pub fn run_class(c: &Cabinet) -> Option<RunClass> {
    match c.kind {
        CabinetKind::Base
        | CabinetKind::BaseFiller
        | CabinetKind::CornerBase
        | CabinetKind::BlindBase => Some(RunClass::Base),
        CabinetKind::Wall
        | CabinetKind::WallFiller
        | CabinetKind::CornerWall
        | CabinetKind::BlindWall => Some(RunClass::Wall),
        CabinetKind::FullHeight | CabinetKind::FullHeightFiller => Some(RunClass::FullHeight),
        _ => None,
    }
}

/// Do the two cabinets have the same bottom and the same top (to a quarter
/// inch)?
pub fn same_height(a: &Cabinet, b: &Cabinet) -> bool {
    (a.elevation - b.elevation).abs() < 0.26
        && ((a.elevation + a.height) - (b.elevation + b.height)).abs() < 0.26
}

/// Can `a` and `b` be run mates: same family, same height?
pub fn run_mates(a: &Cabinet, b: &Cabinet) -> bool {
    run_class(a).is_some() && run_class(a) == run_class(b) && same_height(a, b)
}

/// A plain box cabinet (not a corner unit): fillers attach to its two ends.
fn is_box(c: &Cabinet) -> bool {
    run_class(c).is_some() && !c.kind.is_corner() && c.custom.is_none()
}

/// Whether the plan angles `a` and `b` are the same, in radians.
fn parallel(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(std::f64::consts::TAU);
    d < 1e-4 || std::f64::consts::TAU - d < 1e-4
}

/// Whether the plan angles are equal or half a turn apart (a line, not a
/// direction).
fn collinear_angle(a: f64, b: f64) -> bool {
    parallel(a, b) || parallel(a, b + std::f64::consts::PI)
}

/// What Create Automatic Fillers and its angled variant ask for (General
/// Cabinet Defaults).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FillerOptions {
    /// Create Automatic Fillers.
    pub enabled: bool,
    /// Create Automatic Fillers for Angled Connections.
    pub angled: bool,
    /// The widest gap a filler closes, inches.
    pub reach: f64,
}

impl Default for FillerOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            angled: true,
            reach: AUTO_FILLER_REACH,
        }
    }
}

/// Rounds a width to the nearest 1/16 in.
fn to_sixteenth(v: f64) -> f64 {
    (v / SIXTEENTH).round() * SIXTEENTH
}

/// The distances from the ends of `cab` to the nearest obstacle on its left
/// and right in its own depth strip: `(left, right)`; `None` where nothing
/// blocks. Obstacles that overlap the cabinet give negative distances.
fn end_gaps(cab: &Cabinet, obstacles: &[Vec<Point>]) -> (Option<f64>, Option<f64>) {
    let u = Point::new(cab.angle.cos(), cab.angle.sin());
    match geom::free_span(
        obstacles,
        cab.position,
        u,
        u.perp(),
        (STRIP_INSET, cab.depth - STRIP_INSET),
        cab.width / 2.0,
    ) {
        Ok((l, r)) => (l.map(|l| -l), r.map(|r| r - cab.width)),
        Err(_) => (None, None),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Left,
    Right,
}

/// The nearest run mate on `end` of `a` (parallel, same angle) and the gap
/// to it.
fn nearest_mate<'a>(a: &Cabinet, mates: &[&'a Cabinet], end: End) -> Option<(f64, &'a Cabinet)> {
    let mut best: Option<(f64, &Cabinet)> = None;
    for m in mates {
        let (l, r) = end_gaps(a, &[m.footprint()]);
        let g = if end == End::Left { l } else { r };
        if let Some(g) = g.filter(|g| *g > -1e-6) {
            if best.is_none_or(|b| g < b.0) {
                best = Some((g, m));
            }
        }
    }
    best
}

/// A filler for `class` taking its height, top, toe kick, backsplash and
/// moldings from `like`, placed at `s` along `like`'s width axis (from its
/// back-left corner) and `t` along its depth axis, `width` wide and `depth`
/// deep.
fn make_filler(like: &Cabinet, class: RunClass, s: f64, t: f64, width: f64, depth: f64) -> Cabinet {
    let u = Point::new(like.angle.cos(), like.angle.sin());
    let mut f = Cabinet::filler(class.filler_kind(), width);
    f.angle = like.angle;
    f.position = like.position + u * s + u.perp() * t;
    f.depth = depth;
    f.height = like.height;
    f.elevation = like.elevation;
    f.countertop = like.countertop;
    f.toe_kick = like.toe_kick;
    f.backsplash = like.backsplash;
    f.moldings = like.moldings.clone();
    f.materials = like.materials.clone();
    f.door_style = like.door_style.clone();
    f.overlay = like.overlay;
    f.auto_filler = true;
    f.in_schedule = false;
    f
}

/// Is the filler rectangle clear of every obstacle (an obstacle's corner
/// inside it, or its centre inside an obstacle, is not)?
fn rect_is_free(f: &Cabinet, obstacles: &[Vec<Point>]) -> bool {
    let local = [
        Point::new(0.1, 0.1),
        Point::new(f.width - 0.1, 0.1),
        Point::new(f.width - 0.1, f.depth - 0.1),
        Point::new(0.1, f.depth - 0.1),
        Point::new(f.width / 2.0, f.depth / 2.0),
    ];
    let probes: Vec<Point> = local.iter().map(|p| f.to_plan(*p)).collect();
    obstacles.iter().all(|o| {
        !probes.iter().any(|p| point_in_polygon(*p, o))
            && !o.iter().any(|p| {
                let l = to_local(f, *p);
                l.x > 0.1 && l.x < f.width - 0.1 && l.y > 0.1 && l.y < f.depth - 0.1
            })
    })
}

fn to_local(c: &Cabinet, p: Point) -> Point {
    let d = p.sub(c.position);
    let (s, co) = c.angle.sin_cos();
    Point::new(d.x * co + d.y * s, -d.x * s + d.y * co)
}

/// The fillers the program makes by itself (Create Automatic Fillers,
/// reference manual p. 649), for the cabinets `cabs` and the `walls`
/// (plan polygons, see [`wall_polygon`]):
///
/// * between two cabinets of one family and height that stand side by side
///   with 1/16 in to `opts.reach` (3 in) between them: a filler of exactly
///   that width, as deep as the two have in common, with the toe kick,
///   moldings, backsplash and countertop of the first cabinet;
/// * between a cabinet side and a wall within the same reach (the filler is
///   as deep as the cabinet; the countertop of the pair then runs to the
///   wall);
/// * with `opts.angled`, in the angle where two cabinets of one family meet
///   at a front corner within the reach: a filler next to the one whose end
///   lies along the other's front plane.
///
/// Extended stiles of a framed cabinet count towards the gap (they act as
/// fillers and take precedence). Corner units take no fillers on their own
/// arms. The result never overlaps another cabinet or wall, carries
/// `auto_filler`, is left out of schedules, and has id 0. Existing automatic
/// fillers in `cabs` are ignored.
pub fn auto_fillers(cabs: &[Cabinet], walls: &[Vec<Point>], opts: &FillerOptions) -> Vec<Cabinet> {
    if !opts.enabled {
        return Vec::new();
    }
    let users: Vec<&Cabinet> = cabs
        .iter()
        .filter(|c| !c.auto_filler && !c.kind.is_custom() && c.custom.is_none())
        .collect();
    let mut out: Vec<Cabinet> = Vec::new();
    let reach = opts.reach.max(SIXTEENTH);

    for a in users.iter().copied().filter(|c| is_box(c)) {
        let Some(class) = run_class(a) else { continue };
        let u = Point::new(a.angle.cos(), a.angle.sin());
        let v = u.perp();
        let (a_t0, a_t1) = (a.position.dot(v), a.position.dot(v) + a.depth);
        let mates: Vec<&Cabinet> = users
            .iter()
            .copied()
            .filter(|o| o.id != a.id && run_mates(a, o) && parallel(o.angle, a.angle))
            .collect();
        let blockers: Vec<Vec<Point>> = users
            .iter()
            .filter(|o| {
                o.id != a.id
                    && o.elevation.max(a.elevation)
                        < (o.elevation + o.height).min(a.elevation + a.height) - 0.5
                    && !mates.iter().any(|m| m.id == o.id)
            })
            .map(|o| o.footprint())
            .collect();
        let (bl, br) = end_gaps(a, &blockers);
        let (wl, wr) = end_gaps(a, walls);
        for end in [End::Left, End::Right] {
            let (wall, other) = if end == End::Left { (wl, bl) } else { (wr, br) };
            let mate = nearest_mate(a, &mates, end);
            let cands = [
                wall.map(|g| (g, 0)),
                mate.map(|m| (m.0, 1)),
                other.map(|g| (g, 2)),
            ];
            let Some((gap, what)) = cands
                .into_iter()
                .flatten()
                .filter(|(g, _)| *g > -1e-6)
                .min_by(|x, y| x.0.total_cmp(&y.0))
            else {
                continue;
            };
            if what == 2 || gap > reach + 1e-6 {
                continue;
            }
            let (ext_a, ext_b, t0, t1) = if what == 1 {
                let m = mate.map(|m| m.1).unwrap_or(a);
                // A mate's own right end handles the pair: do the right end
                // here, and the left only for a mate that cannot (a corner
                // unit takes no fillers of its own).
                if end == End::Left && is_box(m) {
                    continue;
                }
                let m_t0 = m.position.dot(v);
                let ext_b = if end == End::Right {
                    m.stile_ext_left
                } else {
                    m.stile_ext_right
                };
                let ext_a = if end == End::Right {
                    a.stile_ext_right
                } else {
                    a.stile_ext_left
                };
                (ext_a, ext_b, a_t0.max(m_t0), a_t1.min(m_t0 + m.depth))
            } else {
                let ext_a = if end == End::Right {
                    a.stile_ext_right
                } else {
                    a.stile_ext_left
                };
                (ext_a, 0.0, a_t0, a_t1)
            };
            let width = to_sixteenth(gap - ext_a - ext_b);
            let depth = t1 - t0;
            if width < SIXTEENTH - 1e-9 || depth < 1.0 {
                continue;
            }
            let s = if end == End::Right {
                a.width + ext_a
            } else {
                -gap + ext_b
            };
            out.push(make_filler(a, class, s, t0 - a_t0, width, depth));
        }
    }

    if opts.angled {
        angled_fillers(&users, reach, &mut out);
    }

    // Drop duplicates and anything that would overlap a cabinet or a wall.
    let mut obstacles: Vec<Vec<Point>> = walls.to_vec();
    obstacles.extend(users.iter().map(|c| c.footprint()));
    let mut kept: Vec<Cabinet> = Vec::new();
    for f in out {
        let c = f.to_plan(Point::new(f.width / 2.0, f.depth / 2.0));
        if kept.iter().any(|k| {
            k.kind == f.kind
                && point_in_polygon(c, &k.footprint())
                && (k.angle - f.angle).abs() < 1e-6
        }) {
            continue;
        }
        let touching_only: Vec<Vec<Point>> = obstacles.clone();
        if rect_is_free(&f, &touching_only) {
            kept.push(f);
        }
    }
    kept
}

/// The fillers of the angled connections (see [`auto_fillers`]).
fn angled_fillers(users: &[&Cabinet], reach: f64, out: &mut Vec<Cabinet>) {
    for a in users.iter().copied().filter(|c| is_box(c)) {
        let Some(class) = run_class(a) else { continue };
        let u = Point::new(a.angle.cos(), a.angle.sin());
        let v = u.perp();
        let a_front = |x: f64| a.to_plan(Point::new(x, a.depth));
        let a_centre = a.to_plan(Point::new(a.width / 2.0, a.depth / 2.0));
        for b in users.iter().copied() {
            if b.id == a.id || !run_mates(a, b) || !is_box(b) || collinear_angle(a.angle, b.angle) {
                continue;
            }
            let bv = Point::new(-b.angle.sin(), b.angle.cos());
            let b_centre = b.to_plan(Point::new(b.width / 2.0, b.depth / 2.0));
            // The fronts must face each other.
            if b_centre.sub(a_centre).dot(v) <= 0.0 || a_centre.sub(b_centre).dot(bv) <= 0.0 {
                continue;
            }
            let b_fronts = [
                b.to_plan(Point::new(0.0, b.depth)),
                b.to_plan(Point::new(b.width, b.depth)),
            ];
            for (end, corner, dir) in [
                (End::Left, a_front(0.0), u * -1.0),
                (End::Right, a_front(a.width), u),
            ] {
                for bc in b_fronts {
                    let d = bc.sub(corner);
                    let (along, across) = (d.dot(dir), d.dot(v));
                    if along < SIXTEENTH || along > reach + 1e-6 || across.abs() > 0.6 {
                        continue;
                    }
                    let width = to_sixteenth(along);
                    let s = if end == End::Right { a.width } else { -along };
                    out.push(make_filler(a, class, s, 0.0, width, a.depth));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cabinet::CabinetKind;

    fn at(mut c: Cabinet, x: f64, id: u64) -> Cabinet {
        c.position = Point::new(x, 0.0);
        c.id = id;
        c
    }

    #[test]
    fn filler_width_equals_the_gap_between_cabinet_and_wall() {
        // Base 0..24, side wall face at x = 28: a 4" gap.
        let base = at(Cabinet::base(24.0), 0.0, 1);
        let side_wall = wall_polygon(Point::new(31.0, -10.0), Point::new(31.0, 60.0), 6.0);
        let mut f = at(Cabinet::filler(CabinetKind::BaseFiller, 3.0), 25.0, 2);
        let w = fit_between(&mut f, &[base], &[side_wall]).unwrap();
        assert!((w - 4.0).abs() < 1e-9, "{w}");
        assert!((f.width - 4.0).abs() < 1e-9);
        assert!((f.position.x - 24.0).abs() < 1e-9);
        // Same for a gap between two cabinets, at any angle.
        let a = at(Cabinet::base(24.0), 0.0, 1);
        let b = at(Cabinet::base(24.0), 29.5, 3);
        let mut g = at(Cabinet::filler(CabinetKind::BaseFiller, 3.0), 25.0, 4);
        assert!((fit_between(&mut g, &[a, b], &[]).unwrap() - 5.5).abs() < 1e-9);
    }

    #[test]
    fn rotated_run_and_unbounded_gaps() {
        let mut base = Cabinet::base(24.0);
        base.id = 1;
        base.angle = std::f64::consts::FRAC_PI_2;
        base.position = Point::new(0.0, 0.0);
        // Rotated: width runs along +y; wall across the run at y = 30.
        let wall = wall_polygon(Point::new(-40.0, 33.0), Point::new(40.0, 33.0), 6.0);
        let mut f = Cabinet::filler(CabinetKind::BaseFiller, 3.0);
        f.angle = std::f64::consts::FRAC_PI_2;
        f.position = Point::new(0.0, 25.0);
        let w = fit_between(&mut f, &[base], &[wall]).unwrap();
        assert!((w - 6.0).abs() < 1e-9, "{w}");
        // Nothing on one side: no fit and no change.
        let mut lone = Cabinet::filler(CabinetKind::BaseFiller, 3.0);
        lone.position = Point::new(100.0, 0.0);
        let before = lone.clone();
        assert!(fit_between(&mut lone, &[], &[]).is_none());
        assert_eq!(lone, before);
        // A gap wider than the limit is refused.
        let a = at(Cabinet::base(24.0), 0.0, 1);
        let b = at(Cabinet::base(24.0), 60.0, 2);
        let mut wide = at(Cabinet::filler(CabinetKind::BaseFiller, 3.0), 40.0, 3);
        assert!(fit_between(&mut wide, &[a, b], &[]).is_none());
    }

    #[test]
    fn wall_cabinets_do_not_block_base_fillers() {
        let base = at(Cabinet::base(24.0), 0.0, 1);
        let wall_cab = at(Cabinet::wall(30.0), 28.0, 2);
        let side = wall_polygon(Point::new(31.0, -10.0), Point::new(31.0, 60.0), 6.0);
        let mut f = at(Cabinet::filler(CabinetKind::BaseFiller, 3.0), 25.0, 3);
        assert!((fit_between(&mut f, &[base, wall_cab], &[side]).unwrap() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn fit_to_gap_snaps_a_nearly_fitting_cabinet_and_ignores_the_rest() {
        // A wall face at x = 0 and a base cabinet starting at x = 31: a
        // 31" gap. A 30" cabinet 0.5" off the wall snaps to the gap.
        let wall = wall_polygon(Point::new(-3.0, -10.0), Point::new(-3.0, 60.0), 6.0);
        let next = at(Cabinet::base(24.0), 31.0, 1);
        let mut c = at(Cabinet::base(30.0), 0.5, 2);
        let w = fit_to_gap(
            &mut c,
            std::slice::from_ref(&next),
            std::slice::from_ref(&wall),
            2.0,
        )
        .unwrap();
        assert!((w - 31.0).abs() < 1e-9 && (c.width - 31.0).abs() < 1e-9);
        assert!(c.position.x.abs() < 1e-9);
        // A 24" cabinet is 7" short of the gap: left alone.
        let mut small = at(Cabinet::base(24.0), 1.0, 3);
        let before = small.clone();
        assert!(fit_to_gap(
            &mut small,
            std::slice::from_ref(&next),
            std::slice::from_ref(&wall),
            2.0,
        )
        .is_none());
        assert_eq!(small, before);
        // No wall or cabinet on the left: an open side never fits.
        let mut open = at(Cabinet::base(30.0), 0.5, 4);
        assert!(fit_to_gap(&mut open, &[next], &[], 2.0).is_none());
    }

    // ----- automatic fillers -----

    fn user(w: f64, x: f64, id: u64) -> Cabinet {
        at(Cabinet::base(w), x, id)
    }

    fn auto(cabs: &[Cabinet], walls: &[Vec<Point>]) -> Vec<Cabinet> {
        auto_fillers(cabs, walls, &FillerOptions::default())
    }

    #[test]
    fn gaps_of_two_and_three_inches_get_a_filler_and_six_does_not() {
        for (gap, want) in [(2.0, Some(2.0)), (3.0, Some(3.0)), (6.0, None), (0.0, None)] {
            let cabs = [user(24.0, 0.0, 1), user(24.0, 24.0 + gap, 2)];
            let f = auto(&cabs, &[]);
            match want {
                Some(w) => {
                    assert_eq!(f.len(), 1, "gap {gap}");
                    assert!((f[0].width - w).abs() < 1e-9, "gap {gap}: {}", f[0].width);
                    assert!((f[0].position.x - 24.0).abs() < 1e-9);
                    assert!(f[0].auto_filler && !f[0].in_schedule);
                    assert_eq!(f[0].kind, CabinetKind::BaseFiller);
                    assert_eq!(f[0].display_label(), "");
                }
                None => assert!(f.is_empty(), "gap {gap}"),
            }
        }
    }

    #[test]
    fn the_filler_copies_the_height_toe_kick_and_top_of_its_neighbour() {
        let mut a = user(24.0, 0.0, 1);
        a.toe_kick = Some(crate::cabinet::ToeKick {
            height: 5.0,
            depth: 2.0,
        });
        let b = user(24.0, 26.0, 2);
        let f = &auto(&[a.clone(), b], &[])[0];
        assert_eq!(f.toe_kick, a.toe_kick);
        assert_eq!(f.countertop, a.countertop);
        assert_eq!(
            (f.height, f.elevation, f.depth),
            (a.height, a.elevation, 24.0)
        );
    }

    #[test]
    fn a_cabinet_near_a_wall_gets_a_filler_to_the_wall_on_either_side() {
        // A wall face at x = 27 on the right, one at x = -2.5 on the left.
        let right = wall_polygon(Point::new(30.0, -10.0), Point::new(30.0, 60.0), 6.0);
        let left = wall_polygon(Point::new(-5.5, -10.0), Point::new(-5.5, 60.0), 6.0);
        let a = user(24.0, 0.0, 1);
        let f = auto(std::slice::from_ref(&a), &[right.clone(), left.clone()]);
        assert_eq!(f.len(), 2);
        let mut xs: Vec<(i64, i64)> = f
            .iter()
            .map(|f| {
                (
                    (f.position.x * 100.0).round() as i64,
                    (f.width * 100.0).round() as i64,
                )
            })
            .collect();
        xs.sort_unstable();
        assert_eq!(xs, vec![(-250, 250), (2400, 300)]);
        // 4 in of wall gap is more than the reach.
        let far = wall_polygon(Point::new(31.0, -10.0), Point::new(31.0, 60.0), 6.0);
        assert!(auto(&[a], &[far]).is_empty());
    }

    #[test]
    fn a_wall_cabinet_next_to_a_wall_cabinet_gets_a_wall_filler_and_bases_ignore_it() {
        let mut w1 = Cabinet::wall(24.0);
        w1.id = 1;
        let mut w2 = Cabinet::wall(24.0);
        w2.id = 2;
        w2.position = Point::new(26.0, 0.0);
        let f = auto(&[w1.clone(), w2.clone()], &[]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CabinetKind::WallFiller);
        assert_eq!(
            (f[0].elevation, f[0].height, f[0].depth),
            (54.0, 30.0, 12.0)
        );
        // A base cabinet between them in height does not block or join them.
        let base = user(24.0, 0.0, 3);
        assert_eq!(auto(&[w1, w2, base], &[]).len(), 1);
    }

    #[test]
    fn different_heights_and_families_get_no_filler() {
        let a = user(24.0, 0.0, 1);
        let mut tall = Cabinet::full_height(24.0);
        tall.id = 2;
        tall.position = Point::new(26.0, 0.0);
        assert!(auto(&[a.clone(), tall], &[]).is_empty());
        let mut vanity = user(24.0, 26.0, 3);
        vanity.height = 34.5;
        assert!(auto(&[a, vanity], &[]).is_empty());
    }

    #[test]
    fn full_height_cabinets_get_a_full_height_filler() {
        let mut a = Cabinet::full_height(24.0);
        a.id = 1;
        let mut b = Cabinet::full_height(24.0);
        b.id = 2;
        b.position = Point::new(27.0, 0.0);
        let f = auto(&[a, b], &[]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CabinetKind::FullHeightFiller);
        assert_eq!(f[0].height, 84.0);
    }

    #[test]
    fn extended_stiles_take_up_part_of_the_gap() {
        let mut a = user(24.0, 0.0, 1);
        a.stile_ext_right = 1.0;
        let b = user(24.0, 27.0, 2);
        let f = auto(&[a.clone(), b.clone()], &[]);
        assert_eq!(f.len(), 1);
        assert!((f[0].width - 2.0).abs() < 1e-9 && (f[0].position.x - 25.0).abs() < 1e-9);
        // Both stiles taking the whole gap leaves nothing to fill.
        a.stile_ext_right = 1.5;
        let mut b2 = b;
        b2.stile_ext_left = 1.5;
        assert!(auto(&[a, b2], &[]).is_empty());
    }

    #[test]
    fn the_switches_turn_fillers_off() {
        let cabs = [user(24.0, 0.0, 1), user(24.0, 26.0, 2)];
        let off = FillerOptions {
            enabled: false,
            ..FillerOptions::default()
        };
        assert!(auto_fillers(&cabs, &[], &off).is_empty());
    }

    #[test]
    fn existing_automatic_fillers_are_not_blockers_or_neighbours() {
        let mut old = Cabinet::filler(CabinetKind::BaseFiller, 2.0);
        old.id = 3;
        old.auto_filler = true;
        old.position = Point::new(24.0, 0.0);
        let cabs = [user(24.0, 0.0, 1), user(24.0, 26.0, 2), old];
        let f = auto(&cabs, &[]);
        assert_eq!(f.len(), 1);
        assert!((f[0].width - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_rotated_run_gets_its_filler_along_its_own_axis() {
        let turn = |mut c: Cabinet, y: f64, id: u64| {
            c.angle = std::f64::consts::FRAC_PI_2;
            c.position = Point::new(0.0, y);
            c.id = id;
            c
        };
        let cabs = [
            turn(Cabinet::base(24.0), 0.0, 1),
            turn(Cabinet::base(24.0), 27.0, 2),
        ];
        let f = auto(&cabs, &[]);
        assert_eq!(f.len(), 1);
        assert!((f[0].position.y - 24.0).abs() < 1e-9 && (f[0].width - 3.0).abs() < 1e-9);
        assert!((f[0].angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn a_filler_goes_in_the_angle_where_two_runs_meet_at_a_front_corner() {
        // North run starting 3 in beyond the west run's front plane.
        let a = user(24.0, 27.0, 1);
        let mut b = Cabinet::base(24.0);
        b.angle = -std::f64::consts::FRAC_PI_2;
        b.position = Point::new(0.0, 48.0);
        b.id = 2;
        let f = auto(&[a.clone(), b.clone()], &[]);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!((f[0].width - 3.0).abs() < 1e-9 && (f[0].position.x - 24.0).abs() < 1e-9);
        assert_eq!(f[0].depth, 24.0);
        // Switch the angled variant off.
        let off = FillerOptions {
            angled: false,
            ..FillerOptions::default()
        };
        assert!(auto_fillers(&[a.clone(), b.clone()], &[], &off).is_empty());
        // Corners further apart than the reach: nothing.
        let far = user(24.0, 29.0, 1);
        assert!(auto(&[far, b], &[]).is_empty());
    }

    #[test]
    fn fit_bounds_helpers_agree_on_run_class() {
        assert_eq!(run_class(&Cabinet::base(24.0)), Some(RunClass::Base));
        assert_eq!(run_class(&Cabinet::wall(24.0)), Some(RunClass::Wall));
        assert_eq!(run_class(&Cabinet::new(CabinetKind::Shelf, 24.0)), None);
        assert!(run_mates(&Cabinet::base(24.0), &Cabinet::base(30.0)));
        assert!(!run_mates(&Cabinet::base(24.0), &Cabinet::wall(24.0)));
    }
}
