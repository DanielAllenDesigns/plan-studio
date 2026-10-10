//! Round 16 tests: Best Fit, tread modes, lock end, the specification
//! table, merging sections and the landing rules.

use super::*;
use plan_core::Point;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn flight(origin: Point, base: f64, rise: f64) -> Stair {
    let mut s = Stair::new(
        1,
        origin,
        0.0,
        StairParams {
            total_rise: rise,
            riser_height_target: 7.5,
            tread_depth: 10.0,
            ..StairParams::default()
        },
    );
    s.base = base;
    s
}

// ----- Best Fit -----

#[test]
fn best_fit_picks_the_riser_closest_to_six_and_three_quarters() {
    // 108 divides into 16 risers of exactly 6 3/4.
    let b = best_fit(108.0);
    assert_eq!(b.risers, 16);
    assert!(close(b.riser_height, 6.75));
    // 109 1/8: 16 risers (6.82) beat 17 (6.42).
    assert_eq!(best_fit(109.125).risers, 16);
    // 100: 15 risers (6.67) beat 14 (7.14).
    let b = best_fit(100.0);
    assert_eq!(b.risers, 15);
    assert!(close(b.riser_height * 15.0, 100.0));
    // A short rise still gets one riser; the count never reaches zero.
    assert_eq!(best_fit(3.0).risers, 1);
    assert_eq!(best_fit(0.0).risers, 1);
    // The result always reaches the rise exactly.
    for rise in [30.0, 57.5, 96.0, 118.25, 144.0] {
        let b = best_fit(rise);
        assert!(close(b.riser_height * f64::from(b.risers), rise), "{rise}");
        // No other count is nearer 6 3/4.
        for n in 1..40u32 {
            let d = (rise / f64::from(n) - 6.75).abs();
            assert!((b.riser_height - 6.75).abs() <= d + 1e-9, "{rise} vs {n}");
        }
    }
}

#[test]
fn a_stair_is_steeper_or_shallower_than_its_best_fit() {
    assert_eq!(fit_status(16, 108.0), FitStatus::Best);
    assert_eq!(fit_status(14, 108.0), FitStatus::Steeper);
    assert_eq!(fit_status(18, 108.0), FitStatus::Shallower);
    let i = info(108.0, 14, 10.0, true);
    assert!(i.can_make_best_fit);
    assert!(i.reach.contains("steeper"));
    assert!(i.best_fit.contains("16 total risers"), "{}", i.best_fit);
    assert!(close(
        i.rise_angle,
        (108.0f64 / 14.0 / 10.0).atan().to_degrees()
    ));
    // Make Best Fit is off at the Best Fit and with manual heights.
    assert!(!info(108.0, 16, 10.0, true).can_make_best_fit);
    let manual = info(108.0, 14, 10.0, false);
    assert!(!manual.can_make_best_fit);
    assert_eq!(manual.reach, "Start and end heights are set manually");
}

// ----- tread modes and Lock End -----

#[test]
fn tread_modes_map_onto_the_two_lock_flags() {
    for m in [
        TreadMode::Automatic,
        TreadMode::LockDepth,
        TreadMode::LockCount,
    ] {
        let (d, c) = m.locks().unwrap();
        assert_eq!(TreadMode::from_locks(d, c), m);
    }
    assert_eq!(TreadMode::NoChange.locks(), None);
    assert_eq!(TreadMode::from_locks(true, true), TreadMode::NoChange);
    assert_eq!(TreadMode::ALL.len(), 4);
    assert_eq!(TreadMode::LockCount.name(), "Lock Number of Treads");
}

#[test]
fn lock_end_decides_which_end_of_the_section_moves() {
    assert_eq!(LockEnd::from_click(0.1), LockEnd::Top);
    assert_eq!(LockEnd::from_click(0.9), LockEnd::Bottom);
    // A section 20" longer: Lock Top pushes its bottom (and what is below)
    // back 20", Lock Bottom pushes its top (and what is above) forward 20".
    assert!(close(LockEnd::Top.bottom_shift(20.0), -20.0));
    assert!(close(LockEnd::Top.top_shift(20.0), 0.0));
    assert!(close(LockEnd::Bottom.bottom_shift(20.0), 0.0));
    assert!(close(LockEnd::Bottom.top_shift(20.0), 20.0));
}

// ----- the table -----

#[test]
fn the_table_numbers_sections_and_subsections_and_stops_at_ten_rows() {
    let a = flight(Point::new(0.0, 0.0), 0.0, 52.5);
    let b = flight(Point::new(70.0, 0.0), 52.5, 52.5);
    let rows = spec_rows(&[&a, &b]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].number, "1");
    assert_eq!(rows[1].number, "2");
    assert_eq!(rows[0].treads, 6);
    assert!(close(rows[0].length, 60.0));
    assert!(close(rows[0].width, 36.0));
    assert!(close(rows[0].riser_height, 7.5));
    assert!(close(rows[0].top_height, 52.5));
    assert!(close(rows[1].bottom_height, 52.5) && close(rows[1].top_height, 105.0));

    let merged = merge(&a, &b).unwrap();
    let rows = spec_rows(&[&merged]);
    let numbers: Vec<&str> = rows.iter().map(|r| r.number.as_str()).collect();
    assert_eq!(numbers, ["1-1", "1-2"]);
    assert_eq!(
        rows[0].treads + rows[1].treads,
        solve(&merged.params).treads
    );
    assert!(close(rows[1].top_height, 105.0));

    // Landings are not sections.
    let landing = Stair::new(
        2,
        Point::new(60.0, 0.0),
        0.0,
        StairParams {
            shape: StairShape::Landing { depth: 36.0 },
            ..StairParams::default()
        },
    );
    assert_eq!(spec_rows(&[&a, &landing, &b]).len(), 2);

    let many: Vec<Stair> = (0..12).map(|_| a.clone()).collect();
    let refs: Vec<&Stair> = many.iter().collect();
    assert_eq!(spec_rows(&refs).len(), SPEC_ROWS);
}

#[test]
fn an_l_stair_is_two_sections_in_the_table() {
    let l = Stair::new(
        1,
        Point::new(0.0, 0.0),
        0.0,
        StairParams {
            shape: StairShape::LShaped {
                treads_before_landing: 6,
            },
            total_rise: 109.125,
            ..StairParams::default()
        },
    );
    let rows = spec_rows(&[&l]);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[0].number.as_str(), rows[1].number.as_str()),
        ("1", "2")
    );
    assert_eq!(rows[0].treads + rows[1].treads + 1, solve(&l.params).treads);
}

// ----- merging -----

#[test]
fn two_parallel_flights_merge_into_one_section_of_two_subsections() {
    let a = flight(Point::new(0.0, 0.0), 0.0, 52.5);
    let b = flight(Point::new(70.0, 0.0), 52.5, 52.5);
    let m = merge(&a, &b).unwrap();
    let s = solve(&m.params);
    assert_eq!(m.id, a.id);
    assert_eq!(s.risers, 14);
    assert!(close(s.riser_height, 7.5));
    assert_eq!(m.params.subsections, vec![6, 7]);
    // The junction tread is the 10" gap: the depth stays 10".
    assert!(close(m.params.tread_depth, 10.0));
    assert!(close(m.params.total_rise, 105.0));
    // It climbs to where the upper flight did.
    let (top, z) = top_point(&m);
    let (want, zz) = top_point(&b);
    assert!(close(z, zz));
    assert!(top.dist(want) < 1e-6, "{top:?} {want:?}");
}

#[test]
fn merging_refuses_what_chief_refuses() {
    let a = flight(Point::new(0.0, 0.0), 0.0, 52.5);
    let good = flight(Point::new(70.0, 0.0), 52.5, 52.5);
    assert!(merge(&a, &good).is_ok());
    // Not parallel.
    let mut turned = good.clone();
    turned.direction = std::f64::consts::FRAC_PI_2;
    assert_eq!(merge(&a, &turned), Err(MergeError::NotParallel));
    // Opposite direction.
    let mut back = good.clone();
    back.direction = std::f64::consts::PI;
    assert_eq!(merge(&a, &back), Err(MergeError::NotParallel));
    // Up with down.
    let mut down = good.clone();
    down.params.down = true;
    assert_eq!(merge(&a, &down), Err(MergeError::Direction));
    // The ends do not meet (sideways, or far away).
    let side = flight(Point::new(70.0, 40.0), 52.5, 52.5);
    assert_eq!(merge(&a, &side), Err(MergeError::Ends));
    let far = flight(Point::new(400.0, 0.0), 52.5, 52.5);
    assert_eq!(merge(&a, &far), Err(MergeError::Ends));
    // The top of the upper cannot merge with the bottom of the lower.
    assert_eq!(merge(&good, &a), Err(MergeError::Ends));
    // Heights that do not follow on.
    let high = flight(Point::new(70.0, 0.0), 60.0, 52.5);
    assert_eq!(merge(&a, &high), Err(MergeError::Heights));
    // One section, one width.
    let mut wide = good.clone();
    wide.params.width = 48.0;
    assert_eq!(merge(&a, &wide), Err(MergeError::Width));
    // A ramp never merges with a stair; curved sections are not merged.
    let mut ramp = good.clone();
    ramp.params.shape = StairShape::Ramp { slope_1_in: 12.0 };
    assert_eq!(merge(&a, &ramp), Err(MergeError::Ramp));
    let mut curved = good;
    curved.params.shape = StairShape::Curved { inner_radius: 40.0 };
    assert_eq!(merge(&a, &curved), Err(MergeError::Shape));
}

// ----- landings -----

#[test]
fn a_landing_is_one_riser_thick_unless_it_stands_free() {
    assert!(close(auto_thickness(7.5, 0.75, false), 8.25));
    assert!(close(auto_thickness(7.5, 0.75, true), 6.75));
    assert!(close(auto_thickness(6.0, 0.0, false), 6.0));
}

#[test]
fn a_landing_takes_the_top_of_the_section_that_arrives() {
    assert!(close(auto_height(&[52.5], 0.0), 52.5));
    assert!(close(auto_height(&[52.5, 54.0], 0.0), 54.0));
    assert!(close(auto_height(&[], 41.0), 41.0));
    assert!(close(adjacent_height(52.5, 7.5), 60.0));
}

fn square(x: f64, y: f64, s: f64) -> Vec<Point> {
    vec![
        Point::new(x, y),
        Point::new(x + s, y),
        Point::new(x + s, y + s),
        Point::new(x, y + s),
    ]
}

#[test]
fn landings_within_an_inch_recognise_each_other() {
    let a = square(0.0, 0.0, 36.0);
    // Sharing an edge, or half an inch away.
    assert!(are_adjacent(&a, &square(36.0, 0.0, 36.0)));
    assert!(are_adjacent(&a, &square(36.5, 0.0, 36.0)));
    // Three inches away is two separate landings.
    assert!(!are_adjacent(&a, &square(39.0, 0.0, 36.0)));
    // Touching only at a corner does not count.
    assert!(!are_adjacent(&a, &square(36.0, 36.0, 36.0)));
    // The edges are reported: a's right edge (1) against b's left edge (3).
    let e = adjacent_edges(&a, &square(36.0, 0.0, 36.0), ADJACENT_TOLERANCE);
    assert_eq!(e, vec![(1, 3)]);
}

#[test]
fn the_short_edge_of_a_sharp_landing_is_at_least_six_inches() {
    let right_angle = std::f64::consts::FRAC_PI_2;
    assert!(close(short_edge(2.0, 1.0), MIN_SHORT_EDGE));
    assert!(close(short_edge(9.0, 1.0), 9.0));
    // At 90 degrees or more the edge is left alone.
    assert!(close(short_edge(2.0, right_angle), 2.0));
}
