//! Pushing: a cabinet dragged into a run of cabinets moves the ones in its
//! way along the run (Chief's Bumping/Pushing setting), instead of stopping
//! against them.

use plan_core::geometry::Point;
use plan_core::Id;

use crate::cabinet::Cabinet;
use crate::geom;

/// Slack when comparing positions along the run, inches.
const EPS: f64 = 1e-6;
/// A neighbour must share at least this much depth with the moved cabinet
/// to be in its run, inches.
const MIN_SHARED_DEPTH: f64 = 0.5;

fn same_run(a: &Cabinet, b: &Cabinet) -> bool {
    let d = (a.angle - b.angle).rem_euclid(std::f64::consts::TAU);
    d < 1e-4 || std::f64::consts::TAU - d < 1e-4
}

fn heights_overlap(a: &Cabinet, b: &Cabinet) -> bool {
    a.elevation.max(b.elevation) < (a.elevation + a.height).min(b.elevation + b.height) - 0.5
}

/// Moves the cabinets of `others` that `moved` overlaps along its run, so the
/// run stays butted together, and those they in turn run into. Returns the
/// pushed cabinets at their new positions (every other cabinet stays where
/// it was; `moved` itself is not changed), or `None` when something cannot
/// give way: a wall, a cabinet of another run, or a custom top is in the way
/// of the last pushed cabinet. Cabinets that `moved` merely touches are not
/// moved, and nothing overlapping at a different depth is pushed.
pub fn push_run(moved: &Cabinet, others: &[Cabinet], walls: &[Vec<Point>]) -> Option<Vec<Cabinet>> {
    let u = Point::new(moved.angle.cos(), moved.angle.sin());
    let v = u.perp();
    let (m0, m1) = (moved.position.dot(u), moved.position.dot(u) + moved.width);
    let (mt0, mt1) = (moved.position.dot(v), moved.position.dot(v) + moved.depth);
    let in_run = |o: &Cabinet| {
        o.id != moved.id
            && !o.kind.is_custom()
            && same_run(o, moved)
            && heights_overlap(o, moved)
            && {
                let t0 = o.position.dot(v);
                (mt1.min(t0 + o.depth) - mt0.max(t0)) > MIN_SHARED_DEPTH
            }
    };
    let run: Vec<&Cabinet> = others.iter().filter(|o| in_run(o)).collect();
    let s0 = |o: &Cabinet| o.position.dot(u);
    let centre = (m0 + m1) / 2.0;
    let mut pushed: Vec<Cabinet> = Vec::new();

    // Right of the moved cabinet, nearest first; then left.
    for dir in [1.0_f64, -1.0] {
        let mut chain: Vec<&Cabinet> = run
            .iter()
            .copied()
            .filter(|o| {
                let mid = s0(o) + o.width / 2.0;
                if dir > 0.0 {
                    mid > centre
                } else {
                    mid <= centre
                }
            })
            .collect();
        chain.sort_by(|a, b| (dir * s0(a)).total_cmp(&(dir * s0(b))));
        // The edge that the next cabinet must clear.
        let mut edge = if dir > 0.0 { m1 } else { m0 };
        for o in chain {
            let (a0, a1) = (s0(o), s0(o) + o.width);
            let shift = if dir > 0.0 {
                if a0 >= edge - EPS {
                    continue;
                }
                edge - a0
            } else {
                if a1 <= edge + EPS {
                    continue;
                }
                edge - a1
            };
            let mut n = o.clone();
            n.position = o.position + u * shift;
            edge = if dir > 0.0 { a1 + shift } else { a0 + shift };
            pushed.push(n);
        }
    }
    if pushed.is_empty() {
        return Some(pushed);
    }

    // Everything that is not part of the run is an obstacle; so are the walls.
    let pushed_ids: Vec<Id> = pushed.iter().map(|p| p.id).collect();
    let mut obstacles: Vec<Vec<Point>> = walls.to_vec();
    obstacles.extend(
        others
            .iter()
            .filter(|o| {
                o.id != moved.id
                    && !o.kind.is_custom()
                    && heights_overlap(o, moved)
                    && !pushed_ids.contains(&o.id)
                    && !in_run(o)
            })
            .map(Cabinet::footprint),
    );
    for p in &pushed {
        let orig = others.iter().find(|o| o.id == p.id)?;
        let shift = (p.position - orig.position).dot(u);
        let strip = (0.05, orig.depth - 0.05);
        let Ok((left, right)) =
            geom::free_span(&obstacles, orig.position, u, v, strip, orig.width / 2.0)
        else {
            return None;
        };
        let ok_right = right.is_none_or(|r| orig.width + shift.max(0.0) <= r + EPS);
        let ok_left = left.is_none_or(|l| shift.min(0.0) >= l - EPS);
        if !ok_right || !ok_left {
            return None;
        }
    }
    Some(pushed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filler::wall_polygon;

    fn at(w: f64, x: f64, id: Id) -> Cabinet {
        let mut c = Cabinet::base(w);
        c.id = id;
        c.position = Point::new(x, 0.0);
        c
    }

    #[test]
    fn a_cabinet_dropped_on_a_run_pushes_the_ones_in_its_way() {
        // Run: 0..24, 24..48, 48..72. Drop a 30" cabinet at 20..50.
        let run = [at(24.0, 0.0, 1), at(24.0, 24.0, 2), at(24.0, 48.0, 3)];
        let moved = at(30.0, 20.0, 9);
        let pushed = push_run(&moved, &run, &[]).unwrap();
        let by = |id| pushed.iter().find(|p| p.id == id).map(|p| p.position.x);
        // Cabinet 1 straddles the left edge: pushed left to end at 20.
        assert_eq!(by(1), Some(-4.0));
        // Cabinet 2 is right of the centre (35): pushed right to start at 50.
        assert_eq!(by(2), Some(50.0));
        // Cabinet 3 was in 2's way (50..74 vs 48..72): chained to 74.
        assert_eq!(by(3), Some(74.0));
    }

    #[test]
    fn touching_cabinets_stay_and_a_far_run_is_left_alone() {
        let run = [at(24.0, 0.0, 1), at(24.0, 48.0, 2)];
        let moved = at(24.0, 24.0, 9);
        let pushed = push_run(&moved, &run, &[]).unwrap();
        assert!(pushed.is_empty(), "{pushed:?}");
    }

    #[test]
    fn a_wall_that_stops_the_chain_refuses_the_push() {
        // Wall face at x = 52 (a wall 6" thick centred at 55, running along y).
        let wall = wall_polygon(Point::new(55.0, -40.0), Point::new(55.0, 60.0), 6.0);
        let run = [at(24.0, 24.0, 2)];
        let moved = at(30.0, 10.0, 9);
        // Cabinet 2 would have to move to 40..64: through the wall.
        assert!(push_run(&moved, &run, std::slice::from_ref(&wall)).is_none());
        // With room it works.
        let free = wall_polygon(Point::new(95.0, -40.0), Point::new(95.0, 60.0), 6.0);
        assert!(push_run(&moved, &run, &[free]).is_some());
    }

    #[test]
    fn other_runs_and_other_heights_are_not_pushed() {
        let mut wall_cab = at(24.0, 24.0, 2);
        wall_cab.elevation = 54.0;
        wall_cab.height = 30.0;
        let mut turned = at(24.0, 24.0, 3);
        turned.angle = std::f64::consts::FRAC_PI_2;
        let moved = at(30.0, 20.0, 9);
        let pushed = push_run(&moved, &[wall_cab, turned], &[]);
        // The turned one is an obstacle but sits where the moved one is; the
        // run itself pushes nothing.
        assert!(pushed.is_some_and(|p| p.is_empty()));
    }

    #[test]
    fn a_cabinet_at_another_depth_is_not_in_the_run() {
        let mut behind = at(24.0, 24.0, 2);
        behind.position = Point::new(24.0, 40.0);
        let moved = at(30.0, 20.0, 9);
        let pushed = push_run(&moved, &[behind], &[]).unwrap();
        assert!(pushed.is_empty());
    }
}
