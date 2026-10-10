//! Round 17: curved merge, custom stringers, trim, rail transitions, brackets.
use super::*;
use plan_core::Point;

fn curved(origin: Point, rise: f64) -> Stair {
    let mut s = Stair {
        id: 1,
        origin,
        direction: 0.0,
        params: StairParams::default(),
        floor_elevation: 0.0,
        base: 0.0,
    };
    s.params.shape = StairShape::Curved { inner_radius: 40.0 };
    s.params.total_rise = rise;
    s.params.turn = Turn::Left;
    s
}

#[test]
fn two_curved_sections_on_one_circle_merge_into_one_curved_section() {
    let lower = curved(Point::new(0.0, 0.0), 45.0);
    let (centre, sweep) = (curve_center(&lower).unwrap(), curve_sweep(&lower).unwrap());
    // The upper section: the same circle, started one tread further round.
    let mut upper = lower.clone();
    upper.id = 2;
    upper.params.total_rise = 45.0;
    upper.base = 0.0;
    let walk = 40.0 + lower.params.width / 2.0;
    let turn_by = sweep + lower.params.tread_depth / walk;
    let (s, c) = turn_by.sin_cos();
    // Rotate the lower section about its centre to get the upper one.
    let rel = lower.origin - centre;
    upper.origin = centre + Point::new(rel.x * c - rel.y * s, rel.x * s + rel.y * c);
    upper.direction = lower.direction + turn_by;
    upper.base = lower.base + lower.params.total_rise;
    let merged = merge(&lower, &upper).expect("curved sections on one circle merge");
    let (sl, su, sm) = (
        solve(&lower.params),
        solve(&upper.params),
        solve(&merged.params),
    );
    assert_eq!(sm.risers, sl.risers + su.risers);
    assert!(matches!(merged.params.shape, StairShape::Curved { .. }));
    assert_eq!(merged.params.subsections.len(), 2);
}

#[test]
fn curved_sections_on_different_circles_or_with_a_straight_one_do_not_merge() {
    let lower = curved(Point::new(0.0, 0.0), 45.0);
    let mut far = curved(Point::new(500.0, 500.0), 45.0);
    far.base = 45.0;
    assert!(merge(&lower, &far).is_err());
    let mut straight = lower.clone();
    straight.params.shape = StairShape::Straight;
    assert_eq!(merge(&lower, &straight), Err(MergeError::Shape));
}

#[test]
fn custom_stringers_give_boards_with_a_computed_depth() {
    let mut p = StairParams::default();
    assert!(custom_stringer_boards(&p).is_empty());
    p.finish.custom[0].count = 1;
    p.finish.custom[0].offset = 1.0;
    p.finish.custom[1].count = 2;
    p.finish.custom[2].count = 1;
    p.finish.custom[2].height_above = 2.0;
    let b = custom_stringer_boards(&p);
    assert_eq!(b.len(), 4);
    assert!(
        (b[0].lateral - (1.0 + 0.75)).abs() < 1e-9,
        "left row stands in by its offset"
    );
    assert!(
        (b[3].lateral - (p.width - 0.75)).abs() < 1e-9,
        "right row from the right edge"
    );
    // 8 inches below the nosing line on a 7.5 / 10 stair is less than 8 deep.
    assert!(b[0].depth > 5.0 && b[0].depth < 8.0, "{}", b[0].depth);
    assert!(b[3].depth > b[0].depth, "height above adds depth");
}

#[test]
fn trim_against_wall_goes_on_the_unguarded_sides() {
    let mut p = StairParams {
        left_side: SideKind::None,
        right_side: SideKind::Railing,
        ..StairParams::default()
    };
    assert_eq!(trim_sides(&p), (false, false), "off by default");
    p.finish.trim.on = true;
    assert_eq!(trim_sides(&p), (true, false));
}

#[test]
fn rail_transitions_step_gooseneck_and_smooth() {
    let mut p = StairParams::default();
    let step = junction_step(&p);
    assert!(step > 6.0 && step < 8.0);
    let none = transition_profile(&p);
    assert!(
        none.windows(2).any(|w| (w[0].0 - w[1].0).abs() < 1e-9),
        "an abrupt drop"
    );
    for kind in [RailTransition::Gooseneck, RailTransition::Smooth] {
        p.finish.transition = kind;
        let t = transition_profile(&p);
        assert!(
            (t.first().unwrap().1 - step).abs() < 1e-9,
            "{kind:?} starts at the sloped rail"
        );
        assert!(t.last().unwrap().1.abs() < 1e-9, "{kind:?} ends level");
        assert!(
            t.windows(2)
                .all(|w| w[1].0 > w[0].0 && w[1].1 <= w[0].1 + 1e-9),
            "{kind:?} falls steadily"
        );
    }
    p.finish.transition = RailTransition::None;
    p.finish.smooth_landing_rails = true;
    assert_eq!(
        transition_profile(&p).len(),
        9,
        "Smooth Transitions alone smooths it"
    );
}

#[test]
fn brackets_sit_under_each_riser_on_exposed_sides_only() {
    let mut p = StairParams {
        left_side: SideKind::Railing,
        right_side: SideKind::None,
        ..StairParams::default()
    };
    assert_eq!(bracket_stations(&p), (vec![], vec![]), "off by default");
    p.finish.brackets = true;
    let (l, r) = bracket_stations(&p);
    assert_eq!(l.len() as u32, solve(&p).risers);
    assert!(r.is_empty(), "a side against a wall is not exposed");
    assert!(l.windows(2).all(|w| w[1].0 > w[0].0 && w[1].1 > w[0].1));
}

#[test]
fn old_plans_load_without_the_finish_block() {
    let mut v = serde_json::to_value(StairParams::default()).unwrap();
    v.as_object_mut().unwrap().remove("finish");
    let p: StairParams = serde_json::from_value(v).unwrap();
    assert_eq!(p.finish, Finish::default());
}
