//! Round 15 tests: sections (Disconnect Selected Subsection, Complete Break)
//! and downward stairs.

use super::*;
use plan_core::Point;

fn params(rise: f64) -> StairParams {
    StairParams {
        total_rise: rise,
        ..StairParams::default()
    }
}

fn stair(params: StairParams) -> Stair {
    Stair::new(7, Point::new(100.0, 50.0), 0.0, params)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn l_stair(turn: Turn) -> Stair {
    stair(StairParams {
        shape: StairShape::LShaped {
            treads_before_landing: 6,
        },
        turn,
        ..params(109.125)
    })
}

#[test]
fn an_l_stair_disconnects_into_a_flight_a_landing_and_a_flight() {
    for turn in [Turn::Left, Turn::Right] {
        let l = l_stair(turn);
        let parts = disconnect(&l).expect("an L stair has subsections");
        assert_eq!(parts.len(), 3);
        // The first piece keeps the stair's id, the rest are new.
        assert_eq!(parts[0].id, 7);
        assert_eq!((parts[1].id, parts[2].id), (0, 0));
        assert_eq!(parts[0].params.shape, StairShape::Straight);
        assert!(matches!(parts[1].params.shape, StairShape::Landing { .. }));
        assert_eq!(parts[2].params.shape, StairShape::Straight);
        // Same risers in total, the same riser height, and each piece stands
        // on the one below it.
        let sol = solve(&l.params);
        let risers: u32 = [&parts[0], &parts[2]]
            .iter()
            .map(|p| solve(&p.params).risers)
            .sum();
        assert_eq!(risers, sol.risers, "{turn:?}");
        for p in [&parts[0], &parts[2]] {
            assert!(close(solve(&p.params).riser_height, sol.riser_height));
        }
        let first_top = parts[0].base + parts[0].params.total_rise;
        assert!(close(parts[1].params.total_rise, first_top));
        assert!(close(parts[2].base, first_top));
        // The second flight arrives where the L stair does.
        let (want, z) = top_point(&l);
        let (got, z2) = top_point(&parts[2]);
        assert!(want.dist(got) < 1e-6, "{turn:?} {want:?} {got:?}");
        assert!(close(z, z2));
        // The first flight starts where the L does.
        assert!(parts[0].origin.dist(l.origin) < 1e-9);
    }
}

#[test]
fn a_u_stair_disconnects_and_a_split_landing_gives_two_landings() {
    let u = stair(StairParams {
        shape: StairShape::UShaped {
            treads_before_landing: 5,
        },
        split_landing: true,
        ..params(109.125)
    });
    let parts = disconnect(&u).unwrap();
    assert_eq!(parts.len(), 4);
    assert!(close(
        parts[2].params.total_rise - parts[1].params.total_rise,
        solve(&u.params).riser_height
    ));
    let (want, _) = top_point(&u);
    assert!(top_point(parts.last().unwrap()).0.dist(want) < 1e-6);
}

#[test]
fn only_l_and_u_stairs_disconnect() {
    assert!(disconnect(&stair(params(109.125))).is_none());
    let w = stair(StairParams {
        shape: StairShape::Winder { winders: 3 },
        ..params(109.125)
    });
    assert!(disconnect(&w).is_none());
}

#[test]
fn a_complete_break_makes_a_flight_a_landing_and_a_flight() {
    let s = stair(params(109.125));
    let sol = solve(&s.params);
    let parts = complete_break(&s, 6, 36.0).expect("a straight flight breaks");
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].id, 7);
    let (lo, hi) = (solve(&parts[0].params), solve(&parts[2].params));
    assert_eq!(lo.risers, 6);
    assert_eq!(lo.risers + hi.risers, sol.risers);
    assert!(close(lo.riser_height, sol.riser_height));
    assert!(close(hi.riser_height, sol.riser_height));
    // The landing is level with the top of the lower flight and the upper
    // flight starts on it, 36" further along.
    let top = parts[0].params.total_rise;
    assert!(close(parts[1].params.total_rise, top));
    assert!(close(parts[2].base, top));
    let (arrive, z) = top_point(&parts[2]);
    assert!(close(z, 109.125));
    let (end_lower, _) = top_point(&parts[0]);
    let ahead = arrive.x - end_lower.x;
    assert!(ahead > 36.0, "{ahead}");
    // Too few risers on either side refuses.
    assert!(complete_break(&s, 1, 36.0).is_none());
    assert!(complete_break(&s, sol.risers - 1, 36.0).is_none());
    // An L stair does not break.
    assert!(complete_break(&l_stair(Turn::Left), 3, 36.0).is_none());
}

#[test]
fn a_downward_stair_labels_dn_and_starts_its_arrow_at_the_top() {
    let up = stair(params(60.0));
    let down = Stair {
        params: StairParams {
            down: true,
            ..up.params.clone()
        },
        ..up.clone()
    };
    let text = |s: &Stair| {
        plan_symbol(s, None)
            .into_iter()
            .find_map(|k| match k {
                Stroke::Text { text, pos, .. } => Some((text, pos)),
                _ => None,
            })
            .unwrap()
    };
    let (t_up, p_up) = text(&up);
    let (t_dn, p_dn) = text(&down);
    assert_eq!(t_up, "UP");
    assert_eq!(t_dn, "DN");
    // UP stands near the bottom of the run, DN near the top.
    assert!(p_dn.x > p_up.x + 20.0, "{p_up:?} {p_dn:?}");
    // The arrowhead points back down the stair: its tip is the lowest end of
    // the centre line.
    let tip = |s: &Stair| {
        let strokes = plan_symbol(s, None);
        let line = strokes
            .iter()
            .filter_map(|k| match k {
                Stroke::Polyline(p, false) if p.len() == 2 => Some(p.clone()),
                _ => None,
            })
            .next()
            .expect("the centre line");
        (line[0], line[1])
    };
    let (a, b) = tip(&down);
    assert!(a.x > b.x, "{a:?} {b:?}");
    let (a, b) = tip(&up);
    assert!(a.x < b.x, "{a:?} {b:?}");
    // The steps are the same.
    assert_eq!(solve(&up.params), solve(&down.params));
}
