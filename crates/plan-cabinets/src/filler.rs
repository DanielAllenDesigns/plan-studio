//! Fillers: strips that close the gap between a cabinet and a wall (or
//! another cabinet). [`fit_between`] sizes a filler to exactly the gap it
//! sits in.

use plan_core::geometry::Point;

use crate::cabinet::Cabinet;
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
}
