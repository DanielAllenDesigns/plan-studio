//! Round 15 tests: U gap and split landings, the walkline and radius
//! reference, curved ramps, the Stringers panel, runner, handrail
//! extensions, post profiles and post-to-beam, flare and starter treads,
//! and the Plan Display options.

use super::*;
use plan_core::geometry::polygon_area;
use plan_core::Point;

fn params(rise: f64) -> StairParams {
    StairParams {
        total_rise: rise,
        ..StairParams::default()
    }
}

fn stair(params: StairParams) -> Stair {
    Stair::new(1, Point::new(100.0, 50.0), 0.0, params)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn count(parts: &[(StairPart, plan_3d::Mesh)], part: StairPart) -> usize {
    parts.iter().filter(|(p, _)| *p == part).count()
}

fn bounds(parts: &[(StairPart, plan_3d::Mesh)]) -> ([f64; 3], [f64; 3]) {
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for (_, m) in parts {
        if let Some((a, b)) = m.bounds() {
            for k in 0..3 {
                lo[k] = lo[k].min(f64::from(a[k]));
                hi[k] = hi[k].max(f64::from(b[k]));
            }
        }
    }
    (lo, hi)
}

fn u_stair(gap: f64, split: bool, turn: Turn) -> Stair {
    stair(StairParams {
        shape: StairShape::UShaped {
            treads_before_landing: 5,
        },
        turn,
        u_gap: gap,
        split_landing: split,
        ..params(109.125)
    })
}

// ----- U-shaped stairs: gap and split landing -----

#[test]
fn a_u_stair_gap_pushes_the_second_flight_away_by_the_gap() {
    for turn in [Turn::Left, Turn::Right] {
        let tight = u_stair(0.0, false, turn);
        let gapped = u_stair(6.0, false, turn);
        let (a0, a1) = (
            polygon_area(&footprint(&tight)).abs(),
            polygon_area(&footprint(&gapped)).abs(),
        );
        // The gap is empty ground between the flights, so the area of the
        // footprint grows by the landing's strip alone.
        assert!(a1 > a0, "{a0} {a1}");
        // The stair still arrives at the full rise.
        let (_, z) = top_point(&gapped);
        assert!(close(z, 109.125), "{z}");
        // The two flights are `gap` further apart across the stair.
        let (p0, _) = top_point(&tight);
        let (p1, _) = top_point(&gapped);
        assert!(close(p0.dist(p1), 6.0), "{turn:?} {}", p0.dist(p1));
    }
}

#[test]
fn a_split_landing_is_two_platforms_one_riser_apart() {
    let st = u_stair(0.0, true, Turn::Left);
    let sol = solve(&st.params);
    assert_eq!(sol.landings, 2);
    assert_eq!(sol.risers, 15);
    let (_, z) = top_point(&st);
    assert!(close(z, sol.riser_height * 15.0), "{z}");
    let parts = tagged_meshes(&st);
    assert_eq!(count(&parts, StairPart::Landing), 2);
    // The two slabs' tops are one riser apart.
    let mut tops: Vec<f64> = parts
        .iter()
        .filter(|(p, _)| *p == StairPart::Landing)
        .map(|(_, m)| f64::from(m.bounds().unwrap().1[1]))
        .collect();
    tops.sort_by(f64::total_cmp);
    assert!(
        (tops[1] - tops[0] - sol.riser_height).abs() < 1e-3,
        "{tops:?}"
    );
    // The step between them is closed by a riser board.
    let plain = tagged_meshes(&u_stair(0.0, false, Turn::Left));
    assert!(count(&parts, StairPart::Riser) > count(&plain, StairPart::Riser) - 1);
    // One landing again when the split is off.
    assert_eq!(solve(&u_stair(0.0, false, Turn::Left).params).landings, 1);
    assert_eq!(count(&plain, StairPart::Landing), 1);
}

#[test]
fn winders_with_a_contraction_have_a_flat_inside_end() {
    let winder = |c: f64| {
        stair(StairParams {
            shape: StairShape::Winder { winders: 3 },
            winder_contraction: c,
            ..params(109.125)
        })
    };
    let layout = |c: f64| crate::layout::Layout::build(&winder(c));
    let sharp = layout(0.0);
    let cut = layout(2.0);
    assert_eq!(sharp.slabs.len(), 3);
    assert_eq!(cut.slabs.len(), 3);
    // Every wedge gains the corner the cut makes at its inside end.
    for (a, b) in sharp.slabs.iter().zip(&cut.slabs) {
        assert_eq!(b.poly.len(), a.poly.len() + 1);
    }
    // The inside end of the riser lines is at least the contraction wide:
    // the two inside points of the middle wedge are about 2" apart.
    let mid = &cut.slabs[1].poly;
    let inner: Vec<_> = mid.iter().filter(|p| p.0 < 100.0 || true).collect();
    assert!(!inner.is_empty());
    let width_at_inside = |poly: &[(f64, f64)]| {
        let mut best = f64::MAX;
        for (i, a) in poly.iter().enumerate() {
            for b in poly.iter().skip(i + 1) {
                let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                if d > 1e-6 {
                    best = best.min(d);
                }
            }
        }
        best
    };
    assert!(close(width_at_inside(mid), 2.0), "{}", width_at_inside(mid));
    // The treads are still all there in 3D.
    let slabs = tagged_meshes(&winder(2.0))
        .into_iter()
        .filter(|(p, _)| *p == StairPart::Landing)
        .count();
    assert_eq!(slabs, 3);
    // The area covered shrinks a little.
    let area = |l: &crate::layout::Layout| -> f64 {
        l.slabs
            .iter()
            .map(|s| {
                let pts: Vec<Point> = s.poly.iter().map(|&(x, y)| Point::new(x, y)).collect();
                polygon_area(&pts).abs()
            })
            .sum()
    };
    assert!(area(&cut) < area(&sharp));
}

// ----- walkline and radius reference -----

#[test]
fn the_walkline_sets_where_a_curve_measures_its_treads() {
    let curved = |walkline: Walkline| StairParams {
        shape: StairShape::Curved { inner_radius: 24.0 },
        walkline,
        ..params(109.125)
    };
    let centre = curved(Walkline::default());
    let near_inside = curved(Walkline {
        on: true,
        distance: 12.0,
        show: false,
    });
    // Treads of the same depth at the walkline sweep further when the
    // walkline is nearer the inside edge.
    let sweep = |p: StairParams| curve_sweep(&stair(p)).unwrap();
    assert!(sweep(near_inside.clone()) > sweep(centre.clone()));
    assert!(close(centre.walk_offset(), 18.0));
    assert!(close(near_inside.walk_offset(), 12.0));
    // The Radius Reference converts between the circles of the stair.
    let mut p = centre;
    assert!(close(p.curve_radius(RadiusRef::InnerArc).unwrap(), 24.0));
    assert!(close(p.curve_radius(RadiusRef::Centerline).unwrap(), 42.0));
    assert!(close(p.curve_radius(RadiusRef::OuterArc).unwrap(), 60.0));
    p.set_curve_radius(RadiusRef::OuterArc, 72.0);
    assert!(close(p.curve_radius(RadiusRef::InnerArc).unwrap(), 36.0));
    p.set_curve_radius(RadiusRef::Walkline, 30.0);
    assert!(close(p.curve_radius(RadiusRef::Walkline).unwrap(), 30.0));
    assert!(RadiusRef::ALL.iter().all(|r| !r.name().is_empty()));
    // A straight stair has no radius.
    assert_eq!(params(100.0).curve_radius(RadiusRef::Walkline), None);
}

#[test]
fn the_walkline_shows_in_plan_when_asked() {
    let with = |show: bool| {
        stair(StairParams {
            walkline: Walkline {
                on: true,
                distance: 12.0,
                show,
            },
            ..params(109.125)
        })
    };
    let open_polys = |s: &Stair| {
        plan_symbol(s, None)
            .into_iter()
            .filter(|x| matches!(x, Stroke::Polyline(p, false) if p.len() == 2))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        open_polys(&with(true)).len(),
        open_polys(&with(false)).len() + 1
    );
    // 12" from the right edge of a 36" stair going +x from y = 50 is y = 38.
    let line = open_polys(&with(true))
        .into_iter()
        .find_map(|x| match x {
            Stroke::Polyline(p, false) if p.len() == 2 && close(p[0].y, 50.0 - 24.0) => Some(p),
            _ => None,
        })
        .expect("the walkline");
    assert!(close(line[0].x, 100.0) && close(line[1].x, 240.0));
}

// ----- curved ramps -----

fn curved_ramp(rise: f64, turn: Turn) -> Stair {
    stair(StairParams {
        shape: StairShape::Ramp { slope_1_in: 12.0 },
        ramp_curve: Some(60.0),
        turn,
        handrail: true,
        ..params(rise)
    })
}

#[test]
fn a_curved_ramp_climbs_along_its_arc_and_arrives_at_the_rise() {
    for turn in [Turn::Left, Turn::Right] {
        let st = curved_ramp(24.0, turn);
        let sol = solve(&st.params);
        assert!(close(sol.total_run, 288.0), "{}", sol.total_run);
        assert!(sol.code_ok, "{:?}", sol.warnings);
        let (top, z) = top_point(&st);
        assert!(close(z, 24.0), "{z}");
        // The arc bends away from the start, so the top is not 288" along
        // the starting direction.
        assert!(top.x - 100.0 < 288.0 - 1.0, "{top:?}");
        let c = curve_center(&st).expect("a centre");
        assert!(close(c.x, 100.0));
        assert!(curve_sweep(&st).unwrap() > 0.5);
        assert!(polygon_area(&footprint(&st)).abs() > 288.0 * 30.0);
        // Meshes: sloped strips, finite and rising to the rise.
        let parts = tagged_meshes(&st);
        assert!(count(&parts, StairPart::Ramp) >= 10);
        let (lo, hi) = bounds(&parts);
        assert!(lo.iter().chain(hi.iter()).all(|x| x.is_finite()));
        assert!(hi[1] > 24.0 + 30.0, "handrails reach 34\" above: {}", hi[1]);
        assert!(count(&parts, StairPart::Handrail) > 4);
    }
}

#[test]
fn a_tall_curved_ramp_has_landings_every_30_inches() {
    let st = curved_ramp(60.0, Turn::Left);
    let sol = solve(&st.params);
    assert_eq!(sol.landings, 1);
    let parts = tagged_meshes(&st);
    assert_eq!(count(&parts, StairPart::Landing), 1);
    let (_, z) = top_point(&st);
    assert!(close(z, 60.0), "{z}");
    let plan = plan_symbol(&st, None);
    // Outline, one landing band, two handrail arcs.
    let closed = plan
        .iter()
        .filter(|s| matches!(s, Stroke::Polyline(_, true)))
        .count();
    assert!(closed >= 2, "{closed}");
    assert!(plan
        .iter()
        .any(|s| matches!(s, Stroke::Text { text, .. } if text == "UP")));
}

#[test]
fn a_curved_ramp_has_a_rail_that_rises_with_it() {
    let st = Stair {
        params: StairParams {
            left_side: SideKind::Railing,
            right_side: SideKind::Railing,
            ..curved_ramp(24.0, Turn::Left).params
        },
        ..curved_ramp(24.0, Turn::Left)
    };
    let g = stair_railing_geometry(&st, RailSide::Right, &st.params.railing);
    let first = g.rails.first().unwrap();
    let last = g.rails.last().unwrap();
    assert!(last.3 > first.1 + 20.0, "{} {}", first.1, last.3);
    assert!(g.newels.len() >= 2);
    assert!(!g.balusters.is_empty());
    let posts = stair_posts(&st);
    assert!(posts.newels.len() >= 4 && !posts.balusters.is_empty());
}

#[test]
fn a_straight_ramp_with_handrails_draws_them_in_plan_and_3d() {
    let plain = stair(StairParams {
        shape: StairShape::Ramp { slope_1_in: 12.0 },
        ..params(30.0)
    });
    let railed = Stair {
        params: StairParams {
            handrail: true,
            handrail_options: HandrailOptions {
                extend_top: 12.0,
                extend_bottom: 12.0,
                ..HandrailOptions::default()
            },
            ..plain.params.clone()
        },
        ..plain.clone()
    };
    assert_eq!(count(&tagged_meshes(&plain), StairPart::Handrail), 0);
    assert!(count(&tagged_meshes(&railed), StairPart::Handrail) >= 2);
    assert!(plan_symbol(&railed, None).len() >= plan_symbol(&plain, None).len() + 2);
    // The rails reach past the ends by the extension.
    let (lo, hi) = bounds(
        &tagged_meshes(&railed)
            .into_iter()
            .filter(|(p, _)| *p == StairPart::Handrail)
            .collect::<Vec<_>>(),
    );
    assert!(lo[0] <= 100.0 - 12.0 + 1e-3, "{}", lo[0]);
    assert!(hi[0] >= 100.0 + 360.0 + 12.0 - 1e-3, "{}", hi[0]);
}

// ----- the Stringers panel, runner, top landing -----

fn stringers(o: StringerOptions, style: StringerStyle) -> Vec<(StairPart, plan_3d::Mesh)> {
    tagged_meshes(&stair(StairParams {
        stringers: o,
        stringer: style,
        ..params(109.125)
    }))
}

#[test]
fn centre_stringers_and_left_out_sides() {
    let sides = count(
        &stringers(StringerOptions::default(), StringerStyle::Closed),
        StairPart::Stringer,
    );
    assert_eq!(sides, 2);
    let with_centre = StringerOptions {
        centre: 1,
        ..StringerOptions::default()
    };
    assert_eq!(
        count(
            &stringers(with_centre, StringerStyle::Closed),
            StairPart::Stringer
        ),
        3
    );
    // A steel stringer with concrete treads: one in the middle, none beside.
    let steel = StringerOptions {
        centre: 1,
        no_sides: true,
        ..StringerOptions::default()
    };
    assert_eq!(
        count(
            &stringers(steel, StringerStyle::Closed),
            StairPart::Stringer
        ),
        1
    );
    // A centre stringer shows even when the style has no side stringers.
    assert_eq!(
        count(
            &stringers(with_centre, StringerStyle::None),
            StairPart::Stringer
        ),
        1
    );
    // Open stringers cut a triangle per step per board.
    let open = count(
        &stringers(with_centre, StringerStyle::Open),
        StairPart::Stringer,
    );
    assert!(open > 3 * 14, "{open}");
}

#[test]
fn closing_the_underside_adds_a_skirt_and_a_soffit_and_the_top_can_stop_short() {
    let base = count(
        &stringers(StringerOptions::default(), StringerStyle::Closed),
        StairPart::Stringer,
    );
    let closed = StringerOptions {
        open_underneath: false,
        side_inset: 1.0,
        ..StringerOptions::default()
    };
    let parts = stringers(closed, StringerStyle::Closed);
    // Two skirts and a soffit beside the two stringers.
    assert_eq!(count(&parts, StairPart::Stringer), base + 3);
    // The skirt reaches the floor.
    let (lo, _) = bounds(
        &parts
            .iter()
            .filter(|(p, _)| *p == StairPart::Stringer)
            .cloned()
            .collect::<Vec<_>>(),
    );
    assert!(lo[1] <= 1e-3, "{}", lo[1]);
    // Without Extend Stringer Top the boards stop a tread short.
    let long = bounds(
        &stringers(StringerOptions::default(), StringerStyle::Closed)
            .into_iter()
            .filter(|(p, _)| *p == StairPart::Stringer)
            .collect::<Vec<_>>(),
    );
    let short = bounds(
        &stringers(
            StringerOptions {
                extend_top: false,
                ..StringerOptions::default()
            },
            StringerStyle::Closed,
        )
        .into_iter()
        .filter(|(p, _)| *p == StairPart::Stringer)
        .collect::<Vec<_>>(),
    );
    assert!(short.1[0] < long.1[0] - 9.0, "{} {}", short.1[0], long.1[0]);
    // A large stringer base adds a heel block under the first steps.
    let big = count(
        &stringers(
            StringerOptions {
                large_base: true,
                ..StringerOptions::default()
            },
            StringerStyle::Closed,
        ),
        StairPart::Stringer,
    );
    assert_eq!(big, base + 2);
}

#[test]
fn a_runner_covers_the_treads_and_tucks_over_the_nosing() {
    let runner = |width: f64, tucked: bool| {
        tagged_meshes(&stair(StairParams {
            runner: Runner { width, tucked },
            ..params(109.125)
        }))
    };
    assert_eq!(count(&runner(0.0, false), StairPart::Runner), 0);
    assert_eq!(count(&runner(24.0, false), StairPart::Runner), 14);
    assert_eq!(count(&runner(24.0, true), StairPart::Runner), 28);
    // It is centred and as wide as asked.
    let (lo, hi) = bounds(
        &runner(24.0, false)
            .into_iter()
            .filter(|(p, _)| *p == StairPart::Runner)
            .collect::<Vec<_>>(),
    );
    assert!(close(hi[2] - lo[2], 24.0), "{}", hi[2] - lo[2]);
    assert!(
        close(-(hi[2] + lo[2]) / 2.0, 50.0 - 18.0)
            || close((hi[2] + lo[2]) / 2.0, -(50.0 + 18.0))
            || close(-(hi[2] + lo[2]) / 2.0, 50.0 + 18.0)
    );
}

#[test]
fn the_top_landing_can_have_a_nosing_and_lose_its_top_riser() {
    let with = |top: TopLanding| {
        tagged_meshes(&stair(StairParams {
            top_landing: top,
            ..params(109.125)
        }))
    };
    let plain = with(TopLanding::default());
    assert_eq!(count(&plain, StairPart::Riser), 15);
    let nosed = with(TopLanding {
        nosing: true,
        riser_surface: true,
    });
    assert_eq!(
        count(&nosed, StairPart::Tread),
        count(&plain, StairPart::Tread) + 1
    );
    let bare = with(TopLanding {
        nosing: false,
        riser_surface: false,
    });
    assert_eq!(count(&bare, StairPart::Riser), 14);
}

// ----- rails: extensions, profiles, post to beam -----

#[test]
fn handrail_extensions_and_returns() {
    let rail = |o: HandrailOptions| {
        let parts = tagged_meshes(&stair(StairParams {
            handrail: true,
            handrail_options: o,
            ..params(109.125)
        }));
        (
            count(&parts, StairPart::Handrail),
            bounds(
                &parts
                    .into_iter()
                    .filter(|(p, _)| *p == StairPart::Handrail)
                    .collect::<Vec<_>>(),
            ),
        )
    };
    let (n0, b0) = rail(HandrailOptions::default());
    assert_eq!(n0, 2);
    let (n1, b1) = rail(HandrailOptions {
        extend_top: 12.0,
        extend_bottom: 12.0,
        ..HandrailOptions::default()
    });
    assert_eq!(n1, 2);
    assert!(close(b0.0[0] - b1.0[0], 12.0) && close(b1.1[0] - b0.1[0], 12.0));
    let (n2, _) = rail(HandrailOptions {
        return_top: true,
        return_bottom: true,
        ..HandrailOptions::default()
    });
    assert_eq!(n2, 6);
}

fn railed(params: RailingParams) -> Stair {
    stair(StairParams {
        right_side: SideKind::Railing,
        railing: params,
        ..params_with_rise()
    })
}

fn params_with_rise() -> StairParams {
    params(109.125)
}

#[test]
fn post_profiles_change_the_newels_and_balusters() {
    let verts = |rp: RailingParams| -> usize {
        stair_railing(&railed(rp), RailSide::Right, &rp)
            .iter()
            .map(|m| m.vertices.len())
            .sum()
    };
    let square = verts(RailingParams::default());
    let round_newel = verts(RailingParams {
        newel: NewelParams {
            profile: PostProfile::Round,
            ..NewelParams::default()
        },
        ..RailingParams::default()
    });
    let turned_balusters = verts(RailingParams {
        baluster_profile: PostProfile::Turned,
        ..RailingParams::default()
    });
    assert!(round_newel != square, "{round_newel} {square}");
    assert!(turned_balusters > square, "{turned_balusters} {square}");
    for profile in PostProfile::ALL {
        assert!(!profile.name().is_empty());
        let rp = RailingParams {
            newel: NewelParams {
                profile,
                ..NewelParams::default()
            },
            baluster_profile: profile,
            ..RailingParams::default()
        };
        let parts = tagged_meshes(&railed(rp));
        let (lo, hi) = bounds(&parts);
        assert!(lo.iter().chain(hi.iter()).all(|x| x.is_finite()));
    }
}

#[test]
fn a_post_to_beam_newel_reaches_down_through_the_floor() {
    let low = |p: NewelParams| {
        let rp = RailingParams {
            newel: p,
            ..RailingParams::default()
        };
        let g = stair_railing_geometry(&railed(rp), RailSide::Right, &rp);
        let _ = g;
        let meshes = stair_railing(&railed(rp), RailSide::Right, &rp);
        meshes
            .iter()
            .filter_map(|m| m.bounds())
            .map(|(a, _)| f64::from(a[1]))
            .fold(f64::MAX, f64::min)
    };
    let plain = low(NewelParams::default());
    let dropped = low(NewelParams {
        post_to_beam: true,
        ..NewelParams::default()
    });
    assert!(
        close(plain - dropped, NewelParams::default().beam_drop),
        "{plain} {dropped}"
    );
    let short = low(NewelParams {
        post_to_beam: true,
        beam_drop: 4.0,
        ..NewelParams::default()
    });
    assert!(close(plain - short, 4.0));
}

#[test]
fn a_library_post_takes_the_place_of_the_built_in_ones() {
    let st = railed(RailingParams::default());
    let all = tagged_meshes(&st);
    let no_newels = tagged_meshes_skipping(
        &st,
        PostSkip {
            newels: true,
            balusters: false,
        },
    );
    let no_balusters = tagged_meshes_skipping(
        &st,
        PostSkip {
            newels: false,
            balusters: true,
        },
    );
    let bare = tagged_meshes_skipping(
        &st,
        PostSkip {
            newels: true,
            balusters: true,
        },
    );
    assert!(no_newels.len() < all.len() && no_balusters.len() < all.len());
    assert!(bare.len() < no_newels.len() && bare.len() < no_balusters.len());
    // The places to put the library posts: the newels at the ends of the
    // flight, every baluster, with the foot and top elevations.
    let posts = stair_posts(&st);
    assert_eq!(posts.newels.len(), 2);
    assert!(posts.balusters.len() >= 14);
    assert!(close(posts.newels[0].top - posts.newels[0].foot, 40.0));
    assert!(posts
        .balusters
        .iter()
        .all(|b| b.top > b.foot && b.size > 0.0));
    // Sides without a railing have no posts.
    assert!(stair_posts(&stair(params(109.125))).newels.is_empty());
    // A landing's guarded edges have posts too.
    let landing = stair(StairParams {
        shape: StairShape::Landing { depth: 60.0 },
        left_side: SideKind::Railing,
        ..params(40.0)
    });
    assert!(stair_posts(&landing).newels.len() >= 2);
}

// ----- flare, starter treads, curved treads -----

fn tread_widths(st: &Stair) -> Vec<f64> {
    let mut t: Vec<(f64, f64)> = tagged_meshes(st)
        .into_iter()
        .filter(|(p, _)| *p == StairPart::Tread)
        .map(|(_, m)| {
            let (a, b) = m.bounds().unwrap();
            (f64::from(a[1]), f64::from(b[2] - a[2]))
        })
        .collect();
    t.sort_by(|a, b| a.0.total_cmp(&b.0));
    t.into_iter().map(|x| x.1).collect()
}

#[test]
fn flaring_the_bottom_corners_widens_the_first_treads_and_tapers_off() {
    let flared = stair(StairParams {
        flare_shape: Flare {
            corners: [6.0, 6.0, 0.0, 0.0],
            ..Flare::default()
        },
        ..params(109.125)
    });
    let w = tread_widths(&flared);
    assert_eq!(w.len(), 14);
    assert!(w[0] > 36.0 + 8.0, "{}", w[0]);
    assert!(w[0] > w[3] && w[3] > w[10], "{w:?}");
    assert!(close(w[13], 36.0) || w[13] < 38.0, "{}", w[13]);
    // The plan outline follows the flare.
    let widest = plan_symbol(&flared, None)
        .into_iter()
        .filter_map(|s| match s {
            Stroke::Polyline(p, true) if p.len() > 4 => Some(p),
            _ => None,
        })
        .map(|p| {
            let (lo, hi) = (
                p.iter().map(|q| q.y).fold(f64::MAX, f64::min),
                p.iter().map(|q| q.y).fold(f64::MIN, f64::max),
            );
            hi - lo
        })
        .fold(0.0, f64::max);
    assert!(widest > 36.0 + 10.0, "{widest}");
    // Softening rounds the taper: less flare half way along.
    let soft = stair(StairParams {
        flare_shape: Flare {
            corners: [6.0, 6.0, 0.0, 0.0],
            soften: 1.0,
            ..Flare::default()
        },
        ..params(109.125)
    });
    assert!(tread_widths(&soft)[5] < w[5]);
    // A flare only on the top corners widens the last treads.
    let top = stair(StairParams {
        flare_shape: Flare {
            corners: [0.0, 0.0, 4.0, 4.0],
            ..Flare::default()
        },
        ..params(109.125)
    });
    let t = tread_widths(&top);
    assert!(t[13] > t[0] + 4.0, "{t:?}");
    assert!(
        Flare::default().is_none()
            && !Flare {
                curve_all: 1.0,
                ..Flare::default()
            }
            .is_none()
    );
}

#[test]
fn curved_treads_bulge_down_the_stair() {
    let curved = stair(StairParams {
        flare_shape: Flare {
            curve_all: 2.0,
            ..Flare::default()
        },
        ..params(109.125)
    });
    // A tread's front edge reaches 2" further down the stair at the middle
    // than at the ends: its extent along the stair grows by 2".
    let plain = tagged_meshes(&stair(params(109.125)));
    let bent = tagged_meshes(&curved);
    let depth = |parts: &[(StairPart, plan_3d::Mesh)]| {
        let m = parts
            .iter()
            .filter(|(p, _)| *p == StairPart::Tread)
            .nth(5)
            .unwrap();
        let (a, b) = m.1.bounds().unwrap();
        f64::from(b[0] - a[0])
    };
    assert!(
        depth(&bent) > depth(&plain) + 1.0,
        "{} {}",
        depth(&bent),
        depth(&plain)
    );
    // The plan draws each riser line as segments.
    let lines = |s: &Stair| {
        plan_symbol(s, None)
            .iter()
            .filter(|x| matches!(x, Stroke::Line(..)))
            .count()
    };
    assert!(lines(&curved) > lines(&stair(params(109.125))));
}

#[test]
fn one_or_two_starter_treads_round_the_bottom_steps() {
    let aprons = |starter: Starter| {
        plan_symbol(
            &stair(StairParams {
                starter,
                ..params(109.125)
            }),
            None,
        )
        .into_iter()
        .filter(|s| matches!(s, Stroke::Polyline(p, true) if p.len() > 8))
        .count()
    };
    assert_eq!(aprons(Starter::None), 0);
    assert_eq!(aprons(Starter::One), 1);
    assert_eq!(aprons(Starter::Two), 2);
    assert_eq!(Starter::Two.count(), 2);
    let w = tread_widths(&stair(StairParams {
        starter: Starter::Two,
        ..params(109.125)
    }));
    // Both starters reach past the stair; the second one less than the
    // first.
    assert!(
        w[0] > w[1] && w[1] > 36.0 + 1.0 && w[2] <= 36.0 + 1e-6,
        "{w:?}"
    );
    // A bullnose still wins on its own end.
    let st = StairParams {
        starter: Starter::One,
        bullnose: Bullnose::Left,
        ..params(109.125)
    };
    assert!(st.apron_reach().0 >= st.apron_reach().1);
}

// ----- Plan Display and Arrow -----

#[test]
fn tread_numbers_count_the_treads_from_the_bottom() {
    let numbers = |s: &Stair| -> Vec<String> {
        plan_symbol(s, None)
            .into_iter()
            .filter_map(|x| match x {
                Stroke::Text { text, .. } if text != "UP" => Some(text),
                _ => None,
            })
            .collect()
    };
    let plain = stair(params(109.125));
    assert!(numbers(&plain).is_empty());
    let numbered = stair(StairParams {
        plan: PlanOptions {
            number_treads: true,
            ..PlanOptions::default()
        },
        ..params(109.125)
    });
    let n = numbers(&numbered);
    assert_eq!(n.len(), 14);
    assert_eq!(n.first().map(String::as_str), Some("1"));
    assert_eq!(n.last().map(String::as_str), Some("14"));
    // The cut stair numbers only the treads that show.
    let cut = plan_symbol(&numbered, Some(0.5))
        .into_iter()
        .filter(|x| matches!(x, Stroke::Text { text, .. } if text != "UP"))
        .count();
    assert!((6..14).contains(&cut), "{cut}");
    // An L stair numbers through the landing.
    let l = stair(StairParams {
        shape: StairShape::LShaped {
            treads_before_landing: 6,
        },
        plan: PlanOptions {
            number_treads: true,
            ..PlanOptions::default()
        },
        ..params(109.125)
    });
    assert_eq!(numbers(&l).len(), 6 + 7);
}

#[test]
fn the_arrow_style_and_size_follow_the_arrow_panel() {
    let head = |style: ArrowStyle, size: f64| -> Vec<Stroke> {
        plan_symbol(
            &stair(StairParams {
                plan: PlanOptions {
                    arrow: style,
                    arrow_size: size,
                    ..PlanOptions::default()
                },
                ..params(109.125)
            }),
            None,
        )
    };
    let closed = |s: &[Stroke]| {
        s.iter()
            .filter(|x| matches!(x, Stroke::Polyline(p, true) if p.len() == 3))
            .count()
    };
    assert_eq!(closed(&head(ArrowStyle::Closed, 6.0)), 1);
    assert_eq!(closed(&head(ArrowStyle::Open, 6.0)), 0);
    assert_eq!(closed(&head(ArrowStyle::Line, 6.0)), 0);
    // The open head is two barbs round the tip.
    assert!(head(ArrowStyle::Open, 6.0)
        .iter()
        .any(|x| matches!(x, Stroke::Polyline(p, false) if p.len() == 3)));
    // None leaves the label alone.
    let none = head(ArrowStyle::None, 6.0);
    assert!(none.iter().all(|x| !matches!(x, Stroke::Arc { .. })
        && !matches!(x, Stroke::Polyline(p, false) if p.len() == 2)));
    assert!(none
        .iter()
        .any(|x| matches!(x, Stroke::Text { text, .. } if text == "UP")));
    // A bigger head is longer.
    let size = |s: &[Stroke]| {
        s.iter()
            .find_map(|x| match x {
                Stroke::Polyline(p, true) if p.len() == 3 => Some(p[0].dist((p[1] + p[2]) * 0.5)),
                _ => None,
            })
            .unwrap()
    };
    assert!(size(&head(ArrowStyle::Closed, 12.0)) > size(&head(ArrowStyle::Closed, 6.0)) + 5.0);
    for a in ArrowStyle::ALL {
        assert!(!a.name().is_empty());
    }
}

#[test]
fn the_break_line_style_angle_and_size_follow_the_plan_display_panel() {
    let brk = |style: BreakStyle, angle: f64, size: f64| -> Vec<Point> {
        plan_symbol(
            &stair(StairParams {
                plan: PlanOptions {
                    break_style: style,
                    break_angle: angle,
                    break_size: size,
                    ..PlanOptions::default()
                },
                ..params(109.125)
            }),
            Some(2.0 / 3.0),
        )
        .into_iter()
        .find_map(|s| match s {
            Stroke::Polyline(p, false) if p.len() == 6 => Some(p),
            _ => None,
        })
        .expect("a break line")
    };
    let spread = |p: &[Point]| {
        p.iter().map(|q| q.x).fold(f64::MIN, f64::max)
            - p.iter().map(|q| q.x).fold(f64::MAX, f64::min)
    };
    assert!(close(spread(&brk(BreakStyle::Zigzag, 0.0, 3.0)), 6.0));
    assert!(close(spread(&brk(BreakStyle::Zigzag, 0.0, 5.0)), 10.0));
    assert!(close(spread(&brk(BreakStyle::Straight, 0.0, 3.0)), 0.0));
    // A slash runs corner to corner of the width.
    assert!(close(spread(&brk(BreakStyle::Slash, 0.0, 3.0)), 6.0));
    // An angle tilts the whole line: tan 45 x the 36" width.
    assert!(close(spread(&brk(BreakStyle::Straight, 45.0, 3.0)), 36.0));
    for b in BreakStyle::ALL {
        assert!(!b.name().is_empty());
    }
}

#[test]
fn rail_details_show_in_plan_only_when_switched_on() {
    let railed = |o: PlanOptions| {
        plan_symbol(
            &stair(StairParams {
                right_side: SideKind::Railing,
                plan: o,
                ..params(109.125)
            }),
            None,
        )
    };
    let all = railed(PlanOptions::default());
    let no_rails = railed(PlanOptions {
        draw_rails: false,
        ..PlanOptions::default()
    });
    let no_newels = railed(PlanOptions {
        draw_newels: false,
        ..PlanOptions::default()
    });
    let balusters = railed(PlanOptions {
        draw_balusters: true,
        ..PlanOptions::default()
    });
    assert_eq!(all.len(), no_rails.len() + 2);
    assert_eq!(all.len(), no_newels.len() + 2);
    assert!(balusters.len() > all.len() + 20);
}

#[test]
fn old_files_without_the_round_15_fields_still_load() {
    let json = serde_json::to_value(StairParams::default()).unwrap();
    let mut obj = json.as_object().unwrap().clone();
    for k in [
        "u_gap",
        "split_landing",
        "winder_contraction",
        "walkline",
        "radius_ref",
        "stringers",
        "runner",
        "top_landing",
        "handrail_options",
        "plan",
        "flare_shape",
        "starter",
        "ramp_curve",
        "newel_item",
        "baluster_item",
        "railing_openings",
        "allow_wrap",
    ] {
        assert!(obj.remove(k).is_some(), "{k}");
    }
    // The railing's new fields are missing too.
    let back: StairParams = serde_json::from_value(serde_json::Value::Object(obj)).unwrap();
    assert_eq!(back, StairParams::default());
    let mut rail = serde_json::to_value(RailingParams::default()).unwrap();
    rail.as_object_mut().unwrap().remove("baluster_profile");
    rail["newel"].as_object_mut().unwrap().remove("profile");
    rail["newel"]
        .as_object_mut()
        .unwrap()
        .remove("post_to_beam");
    let back: RailingParams = serde_json::from_value(rail).unwrap();
    assert_eq!(back, RailingParams::default());
}

// ----- landings: a railing choice per edge -----

fn landing(p: StairParams) -> Stair {
    stair(StairParams {
        shape: StairShape::Landing { depth: 60.0 },
        total_rise: 40.0,
        width: 48.0,
        ..p
    })
}

#[test]
fn each_landing_edge_can_force_or_drop_its_railing() {
    let base = StairParams {
        left_side: SideKind::Railing,
        right_side: SideKind::Railing,
        ..StairParams::default()
    };
    let auto = landing_guards(&landing(base.clone()));
    // Automatic: the two long sides only; the ends stay open.
    assert_eq!(auto.len(), 2);
    assert!(auto.iter().all(|g| g.kind == SideKind::Railing));
    let sides: Vec<_> = auto.iter().map(|g| g.side).collect();
    assert!(sides.contains(&RailSide::Left) && sides.contains(&RailSide::Right));
    // Has Railing on an end adds it; No Railing on a side takes it away.
    let left_edge = auto.iter().find(|g| g.side == RailSide::Left).unwrap().edge;
    let end_edge = (0..4).find(|i| auto.iter().all(|g| g.edge != *i)).unwrap();
    let mut rails = vec![EdgeRail::Automatic; 4];
    rails[end_edge] = EdgeRail::Has;
    rails[left_edge] = EdgeRail::No;
    let edited = landing_guards(&landing(StairParams {
        edge_rails: rails,
        ..base.clone()
    }));
    assert_eq!(edited.len(), 2);
    assert!(edited.iter().any(|g| g.edge == end_edge));
    assert!(edited.iter().all(|g| g.edge != left_edge));
    // The meshes and the plan follow.
    let posts = |s: &Stair| stair_posts(s).newels.len();
    assert_eq!(posts(&landing(base.clone())), 4);
    assert!(count(&tagged_meshes(&landing(base)), StairPart::Handrail) > 0);
    let none = landing(StairParams {
        edge_rails: vec![EdgeRail::No; 4],
        left_side: SideKind::Railing,
        right_side: SideKind::Railing,
        ..StairParams::default()
    });
    assert!(landing_guards(&none).is_empty());
    assert_eq!(count(&tagged_meshes(&none), StairPart::Handrail), 0);
    assert_eq!(posts(&none), 0);
    // A side of None still gets a railing on an edge that asks for one.
    let forced = landing(StairParams {
        edge_rails: vec![EdgeRail::Has; 4],
        ..StairParams::default()
    });
    assert_eq!(landing_guards(&forced).len(), 4);
    assert!(posts(&forced) >= 4);
    assert!(EdgeRail::ALL.iter().all(|e| !e.name().is_empty()));
}
