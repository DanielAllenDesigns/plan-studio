use super::*;
use crate::railing::landing_rail_paths;
use plan_3d::Material;
use plan_core::geometry::polygon_area;
use std::f64::consts::FRAC_PI_6;

fn params(rise: f64) -> StairParams {
    StairParams {
        total_rise: rise,
        ..StairParams::default()
    }
}

fn stair(params: StairParams) -> Stair {
    Stair::new(1, Point::new(100.0, 50.0), 0.0, params)
}

fn count(parts: &[(StairPart, plan_3d::Mesh)], part: StairPart) -> usize {
    parts.iter().filter(|(p, _)| *p == part).count()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn solve_standard_floor_to_floor() {
    let s = solve(&params(109.125));
    assert_eq!(s.risers, 15);
    assert!(close(s.riser_height, 7.275));
    assert_eq!(s.treads, 14);
    assert!(close(s.total_run, 140.0));
    assert!(s.code_ok, "{:?}", s.warnings);
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
}

#[test]
fn solve_seven_foot_rise_stays_within_code() {
    let s = solve(&params(84.0));
    assert_eq!(s.risers, 11);
    assert!(s.riser_height <= 7.75);
    assert!(s.code_ok);
}

#[test]
fn solve_clamps_risers_for_tall_target() {
    let p = StairParams {
        riser_height_target: 9.0,
        ..params(84.0)
    };
    let s = solve(&p);
    assert!(s.riser_height <= MAX_RISER, "{}", s.riser_height);
    assert!(s.code_ok);
}

#[test]
fn solve_flags_code_and_comfort_problems() {
    let p = StairParams {
        tread_depth: 8.0,
        width: 30.0,
        ..params(109.125)
    };
    let s = solve(&p);
    assert!(!s.code_ok);
    assert!(s.warnings.iter().any(|w| w.contains("tread depth")));
    assert!(s.warnings.iter().any(|w| w.contains("width")));
    assert!(s.warnings.iter().any(|w| w.contains("2R+T")));
}

#[test]
fn l_shaped_run_includes_landing() {
    let p = StairParams {
        shape: StairShape::LShaped {
            treads_before_landing: 6,
        },
        ..params(109.125)
    };
    let s = solve(&p);
    assert_eq!(s.risers, 15);
    assert_eq!(s.treads, 14);
    // 13 regular treads + a 36" landing.
    assert!(close(s.total_run, 13.0 * 10.0 + 36.0));
}

#[test]
fn ramp_solution_uses_slope() {
    let p = StairParams {
        shape: StairShape::Ramp { slope_1_in: 12.0 },
        ..params(30.0)
    };
    let s = solve(&p);
    assert!(close(s.total_run, 360.0));
    assert!(s.code_ok);
    let steep = StairParams {
        shape: StairShape::Ramp { slope_1_in: 8.0 },
        ..params(30.0)
    };
    assert!(!solve(&steep).code_ok);
}

#[test]
fn straight_plan_symbol_has_risers_and_arrow() {
    let st = stair(params(109.125));
    let strokes = plan_symbol(&st, None);
    let risers = strokes
        .iter()
        .filter(|s| matches!(s, Stroke::Line(a, b) if close(a.dist(*b), 36.0)))
        .count();
    assert_eq!(risers, 15); // treads + 1
    assert!(strokes
        .iter()
        .any(|s| matches!(s, Stroke::Arc { end_deg, .. } if close(*end_deg, 360.0))));
    assert!(strokes
        .iter()
        .any(|s| matches!(s, Stroke::Text { text, .. } if text == "UP")));
    // Outline and arrowhead are closed polylines; the shaft is open.
    let closed = strokes
        .iter()
        .filter(|s| matches!(s, Stroke::Polyline(_, true)))
        .count();
    assert_eq!(closed, 2);
    // Arrowhead tip is at the top of the run, on the centreline.
    let tip = strokes
        .iter()
        .find_map(|s| match s {
            Stroke::Polyline(p, true) if p.len() == 3 => Some(p[0]),
            _ => None,
        })
        .expect("arrowhead");
    assert!(close(tip.x, 240.0) && close(tip.y, 50.0 - 18.0));
}

#[test]
fn cut_stair_draws_break_line_and_fewer_risers() {
    let st = stair(params(109.125));
    let strokes = plan_symbol(&st, Some(2.0 / 3.0));
    let risers = strokes
        .iter()
        .filter(|s| matches!(s, Stroke::Line(a, b) if close(a.dist(*b), 36.0)))
        .count();
    assert!((9..15).contains(&risers), "{risers}");
    let zig = strokes
        .iter()
        .find_map(|s| match s {
            Stroke::Polyline(p, false) if p.len() == 6 => Some(p),
            _ => None,
        })
        .expect("break line");
    let cut_x = 100.0 + 140.0 * 2.0 / 3.0;
    assert!(zig.iter().all(|p| (p.x - cut_x).abs() <= ZIGZAG_TOL));
}

const ZIGZAG_TOL: f64 = 3.0 + 1e-9;

#[test]
fn straight_meshes_have_one_tread_per_step() {
    let st = stair(params(109.125));
    let parts = tagged_meshes(&st);
    assert_eq!(count(&parts, StairPart::Tread), 14);
    assert_eq!(count(&parts, StairPart::Riser), 15);
    assert_eq!(count(&parts, StairPart::Stringer), 2);
    assert_eq!(count(&parts, StairPart::Handrail), 0);
    for (part, mesh) in &parts {
        match part {
            StairPart::Tread => assert_eq!(mesh.material, Material::Floor),
            _ => assert_eq!(mesh.material, Material::WallInterior),
        }
        assert_eq!(mesh.object_id, Some(1));
        assert_eq!(mesh.vertices.len(), 24, "{part:?}");
        assert_eq!(mesh.triangle_count(), 12, "{part:?}");
    }
    let top_y = parts
        .iter()
        .filter(|(p, _)| *p == StairPart::Tread)
        .map(|(_, m)| f64::from(m.bounds().unwrap().1[1]))
        .fold(f64::MIN, f64::max);
    assert!((top_y - (109.125 - 7.275)).abs() < 1e-3, "{top_y}");
    assert_eq!(meshes(&st).len(), parts.len());
}

#[test]
fn open_risers_and_handrail_options() {
    let p = StairParams {
        open_risers: true,
        handrail: true,
        ..params(109.125)
    };
    let parts = tagged_meshes(&stair(p));
    assert_eq!(count(&parts, StairPart::Riser), 0);
    assert_eq!(count(&parts, StairPart::Handrail), 2);
}

#[test]
fn mesh_normals_agree_with_winding_and_frame() {
    let mut st = stair(params(109.125));
    st.direction = FRAC_PI_6;
    st.floor_elevation = 96.0;
    for (_, mesh) in tagged_meshes(&st) {
        for tri in mesh.indices.chunks(3) {
            let v: Vec<_> = tri.iter().map(|&i| mesh.vertices[i as usize]).collect();
            let e1: Vec<f32> = (0..3)
                .map(|k| v[1].position[k] - v[0].position[k])
                .collect();
            let e2: Vec<f32> = (0..3)
                .map(|k| v[2].position[k] - v[0].position[k])
                .collect();
            let c = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let dot: f32 = (0..3).map(|k| c[k] * v[0].normal[k]).sum();
            assert!(dot > 0.0);
        }
    }
    // Bottom tread surface starts at the floor elevation plus one riser.
    let lowest_tread_top = tagged_meshes(&st)
        .iter()
        .filter(|(p, _)| *p == StairPart::Tread)
        .map(|(_, m)| f64::from(m.bounds().unwrap().1[1]))
        .fold(f64::MAX, f64::min);
    assert!((lowest_tread_top - (96.0 + 7.275)).abs() < 1e-3);
}

#[test]
fn straight_footprint_is_rotated_rectangle() {
    let mut st = stair(params(109.125));
    st.direction = FRAC_PI_6;
    let fp = footprint(&st);
    assert_eq!(fp.len(), 4);
    assert!(close(polygon_area(&fp).abs(), 36.0 * 140.0));
    let mut sides: Vec<f64> = (0..4).map(|i| fp[i].dist(fp[(i + 1) % 4])).collect();
    sides.sort_by(f64::total_cmp);
    assert!(close(sides[0], 36.0) && close(sides[1], 36.0));
    assert!(close(sides[2], 140.0) && close(sides[3], 140.0));
    // The first corner is the origin; the run follows the direction.
    assert!(fp[0].dist(st.origin) < 1e-9);
    let run_end = Point::new(
        st.origin.x + 140.0 * FRAC_PI_6.cos(),
        st.origin.y + 140.0 * FRAC_PI_6.sin(),
    );
    assert!(fp.iter().any(|p| p.dist(run_end) < 1e-9));
}

#[test]
fn top_point_arrives_at_total_rise() {
    let mut st = stair(params(109.125));
    st.floor_elevation = 96.0;
    let (p, z) = top_point(&st);
    // 140" along +x, centre of the 36" width (right of travel is -y).
    assert!(p.dist(Point::new(240.0, 32.0)) < 1e-9, "{p:?}");
    assert!(close(z, 96.0 + 109.125));
}

#[test]
fn l_shaped_geometry_left_and_right() {
    for (turn, want_y) in [(Turn::Left, 50.0 + 70.0), (Turn::Right, 50.0 - 36.0 - 70.0)] {
        let p = StairParams {
            shape: StairShape::LShaped {
                treads_before_landing: 6,
            },
            turn,
            ..params(109.125)
        };
        let st = stair(p);
        let fp = footprint(&st);
        // Flight 1 (60x36) + landing (36x36) + flight 2 (70x36).
        assert!(
            close(polygon_area(&fp).abs(), 36.0 * (60.0 + 36.0 + 70.0)),
            "{turn:?}"
        );
        let (top, z) = top_point(&st);
        assert!(close(z, 109.125));
        assert!(
            close(top.y, want_y) && close(top.x, 178.0),
            "{turn:?} {top:?}"
        );
        let parts = tagged_meshes(&st);
        assert_eq!(count(&parts, StairPart::Tread), 13);
        assert_eq!(count(&parts, StairPart::Landing), 1);
        // Landing top is at the 7th riser.
        let landing = parts
            .iter()
            .find(|(p, _)| *p == StairPart::Landing)
            .unwrap();
        assert!((f64::from(landing.1.bounds().unwrap().1[1]) - 7.0 * 7.275).abs() < 1e-3);
        let strokes = plan_symbol(&st, None);
        let risers = strokes
            .iter()
            .filter(|s| matches!(s, Stroke::Line(a, b) if close(a.dist(*b), 36.0)))
            .count();
        assert_eq!(risers, 15 - 2 + 2);
    }
}

#[test]
fn u_shaped_footprint_and_arrival() {
    for turn in [Turn::Left, Turn::Right] {
        let p = StairParams {
            shape: StairShape::UShaped {
                treads_before_landing: 6,
            },
            turn,
            ..params(109.125)
        };
        let st = stair(p);
        let fp = footprint(&st);
        // Flight 1 (60x36) + landing (36x72) + flight 2 (70x36).
        let want = 36.0 * 60.0 + 36.0 * 72.0 + 36.0 * 70.0;
        assert!(close(polygon_area(&fp).abs(), want), "{turn:?}");
        let (top, _) = top_point(&st);
        // Flight 2 runs back toward the start: 60 - 70 = -10 along u.
        assert!(close(top.x, 100.0 - 10.0), "{turn:?} {top:?}");
        let arrow = plan_symbol(&st, None);
        assert!(arrow
            .iter()
            .any(|s| matches!(s, Stroke::Polyline(p, false) if p.len() == 6)));
    }
}

#[test]
fn winder_has_pie_treads() {
    let p = StairParams {
        shape: StairShape::Winder { winders: 3 },
        ..params(109.125)
    };
    let st = stair(p);
    let s = solve(&st.params);
    assert!(close(s.total_run, 11.0 * 10.0 + 36.0));
    let parts = tagged_meshes(&st);
    assert_eq!(count(&parts, StairPart::Landing), 3);
    assert_eq!(count(&parts, StairPart::Tread), 11);
    let (_, z) = top_point(&st);
    assert!(close(z, 109.125));
    // Wedges tile the 36" square exactly.
    let layout = layout::Layout::build(&st);
    let area: f64 = layout
        .slabs
        .iter()
        .map(|s| {
            let pts: Vec<Point> = s.poly.iter().map(|&(u, v)| Point::new(u, v)).collect();
            polygon_area(&pts).abs()
        })
        .sum();
    assert!(close(area, 36.0 * 36.0), "{area}");
}

#[test]
fn ramp_is_one_sloped_slab() {
    let p = StairParams {
        shape: StairShape::Ramp { slope_1_in: 12.0 },
        ..params(30.0)
    };
    let st = stair(p);
    let parts = tagged_meshes(&st);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].0, StairPart::Ramp);
    assert!(close(f64::from(parts[0].1.bounds().unwrap().1[1]), 30.0));
    assert!(close(polygon_area(&footprint(&st)).abs(), 36.0 * 360.0));
    let (_, z) = top_point(&st);
    assert!(close(z, 30.0));
    assert!(plan_symbol(&st, None)
        .iter()
        .all(|s| !matches!(s, Stroke::Line(..))));
}

#[test]
fn too_few_risers_falls_back_to_straight() {
    let p = StairParams {
        shape: StairShape::UShaped {
            treads_before_landing: 3,
        },
        ..params(7.0)
    };
    let s = solve(&p);
    assert_eq!(s.risers, 1);
    assert!(s.warnings.iter().any(|w| w.contains("straight")));
    assert_eq!(footprint(&stair(p)).len(), 4);
}

#[test]
fn serde_round_trip() {
    let mut st = stair(StairParams {
        shape: StairShape::UShaped {
            treads_before_landing: 5,
        },
        turn: Turn::Right,
        handrail: true,
        ..params(100.0)
    });
    st.floor_elevation = 12.5;
    let json = serde_json::to_string(&st).unwrap();
    let back: Stair = serde_json::from_str(&json).unwrap();
    assert_eq!(back, st);
    let partial: StairParams = serde_json::from_str(r#"{"width": 42.0}"#).unwrap();
    assert!(close(partial.width, 42.0) && close(partial.tread_depth, 10.0));
}

// ----- Round 8: curved stairs, landings, ramp landings, stringers, sides -----

fn curved(rise: f64, inner: f64, turn: Turn) -> Stair {
    stair(StairParams {
        shape: StairShape::Curved {
            inner_radius: inner,
        },
        turn,
        ..params(rise)
    })
}

#[test]
fn fifteen_risers_from_109_1_8_floor_to_floor_with_the_7_3_4_maximum() {
    let p = StairParams {
        riser_height_target: MAX_RISER,
        ..params(109.125)
    };
    let s = solve(&p);
    assert_eq!(s.risers, 15);
    assert_eq!(s.treads, 14);
    assert!(close(s.riser_height, 109.125 / 15.0));
    assert!(s.riser_height <= MAX_RISER);
    assert_eq!(min_risers(109.125, MAX_RISER), 15);
    // One riser fewer would break the maximum.
    assert!(s.risers != 14);
}

#[test]
fn a_curved_stair_fans_its_treads_around_the_centre() {
    let st = curved(60.0, 30.0, Turn::Left);
    let sol = solve(&st.params);
    assert_eq!((sol.risers, sol.treads), (8, 7));
    assert!(sol.code_ok, "{:?}", sol.warnings);
    let centre = curve_center(&st).unwrap();
    // Left turn: the centre is 30" to the left of the first riser's left corner.
    assert!(close(centre.x, 100.0) && close(centre.y, 80.0));
    // The walking line is 48" from the centre; a tread is 10" deep there.
    let step = 10.0 / 48.0;
    let sweep = curve_sweep(&st).unwrap();
    assert!(close(sweep, 7.0 * step));
    let (top, z) = top_point(&st);
    assert!(close(top.dist(centre), 48.0));
    assert!(close(z, 60.0));
    // Turning left bends toward +y.
    assert!(top.y > 50.0 && top.x > 100.0);
    // The footprint is the annular sector between radii 30 and 66.
    let sector = 0.5 * sweep * (66.0 * 66.0 - 30.0 * 30.0);
    let area = polygon_area(&footprint(&st)).abs();
    assert!((area - sector).abs() / sector < 0.01, "{area} vs {sector}");
    // Seven wedge treads; every one reaches from radius 30 to 66 around the centre.
    let parts = tagged_meshes(&st);
    assert_eq!(count(&parts, StairPart::Tread), 7);
    assert_eq!(count(&parts, StairPart::Riser), 8);
    for (part, m) in &parts {
        if *part != StairPart::Tread {
            continue;
        }
        let radii: Vec<f64> = m
            .vertices
            .iter()
            .map(|v| {
                let p = Point::new(f64::from(v.position[0]), -f64::from(v.position[2]));
                p.dist(centre)
            })
            .collect();
        let (lo, hi) = radii
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), r| (a.min(*r), b.max(*r)));
        assert!(
            (lo - 30.0).abs() < 1e-3 && (hi - 66.0).abs() < 1e-3,
            "{lo} {hi}"
        );
    }
    // Plan: a radial line per riser, 36" long.
    let plan = plan_symbol(&st, None);
    let radials: Vec<_> = plan
        .iter()
        .filter_map(|s| match s {
            Stroke::Line(a, b) => Some(a.dist(*b)),
            _ => None,
        })
        .collect();
    assert_eq!(radials.len(), 8);
    assert!(radials.iter().all(|l| close(*l, 36.0)));
    assert!(plan
        .iter()
        .any(|s| matches!(s, Stroke::Text { text, .. } if text == "UP")));
}

#[test]
fn a_curved_stair_turning_right_mirrors_the_left_one() {
    let l = curved(60.0, 12.0, Turn::Left);
    let r = curved(60.0, 12.0, Turn::Right);
    let (tl, _) = top_point(&l);
    let (tr, _) = top_point(&r);
    assert!(close(tl.x, tr.x));
    // The walking line starts at y = 50 - 18 = 32 facing +x: left bends up, right down.
    assert!(tl.y > 32.0 && tr.y < 32.0);
    assert!(close(tl.y - 32.0, 32.0 - tr.y));
    assert!(close(curve_center(&r).unwrap().y, 50.0 - 36.0 - 12.0));
    assert!(close(
        polygon_area(&footprint(&l)).abs().round(),
        polygon_area(&footprint(&r)).abs().round()
    ));
}

#[test]
fn a_tight_curve_is_flagged_at_the_inside_edge() {
    let st = curved(60.0, 2.0, Turn::Left);
    let sol = solve(&st.params);
    assert!(!sol.code_ok);
    assert!(sol.warnings.iter().any(|w| w.contains("inside")));
}

#[test]
fn a_ramp_over_30_inches_gets_landings_every_run() {
    let st = stair(StairParams {
        shape: StairShape::Ramp { slope_1_in: 12.0 },
        ..params(60.0)
    });
    assert_eq!(ramp_runs(60.0), 2);
    assert_eq!(ramp_runs(30.0), 1);
    assert_eq!(ramp_runs(30.5), 2);
    let sol = solve(&st.params);
    assert_eq!(sol.landings, 1);
    assert!(sol.code_ok, "{:?}", sol.warnings);
    // Two 30" runs of 360" and one 60" landing.
    assert!(close(sol.total_run, 780.0));
    let parts = tagged_meshes(&st);
    assert_eq!(count(&parts, StairPart::Ramp), 2);
    assert_eq!(count(&parts, StairPart::Landing), 1);
    let (_, hi) = parts
        .iter()
        .filter(|(p, _)| *p == StairPart::Landing)
        .map(|(_, m)| m.bounds().unwrap())
        .next()
        .unwrap();
    assert!(close(f64::from(hi[1]), 30.0), "landing top {}", hi[1]);
    assert!(close(polygon_area(&footprint(&st)).abs(), 36.0 * 780.0));
    assert!(close(top_point(&st).1, 60.0));
    // The plan draws the landing outline.
    assert!(plan_symbol(&st, None)
        .iter()
        .any(|s| matches!(s, Stroke::Polyline(p, true) if p.len() == 4)));
}

#[test]
fn a_landing_is_a_flat_slab_at_its_height() {
    let st = stair(StairParams {
        shape: StairShape::Landing { depth: 48.0 },
        ..params(54.0)
    });
    let sol = solve(&st.params);
    assert_eq!((sol.risers, sol.treads), (0, 0));
    assert!(close(sol.total_run, 48.0));
    assert!(close(polygon_area(&footprint(&st)).abs(), 36.0 * 48.0));
    let parts = tagged_meshes(&st);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].0, StairPart::Landing);
    let (lo, hi) = parts[0].1.bounds().unwrap();
    assert!(close(f64::from(hi[1]), 54.0) && close(f64::from(lo[1]), 50.5));
    // The plan is the outline with crossed diagonals.
    let plan = plan_symbol(&st, None);
    assert_eq!(plan.len(), 3);
}

#[test]
fn a_polygon_landing_follows_its_outline() {
    let outline = vec![
        Point::new(0.0, 0.0),
        Point::new(60.0, 0.0),
        Point::new(60.0, 30.0),
        Point::new(30.0, 30.0),
        Point::new(30.0, 60.0),
        Point::new(0.0, 60.0),
    ];
    let st = Stair::new(
        2,
        Point::new(0.0, 0.0),
        0.0,
        StairParams {
            shape: StairShape::Landing { depth: 60.0 },
            outline: outline.clone(),
            ..params(40.0)
        },
    );
    assert!(close(polygon_area(&footprint(&st)).abs(), 2700.0));
    let parts = tagged_meshes(&st);
    assert_eq!(parts.len(), 1);
    let (lo, hi) = parts[0].1.bounds().unwrap();
    assert!(close(f64::from(hi[0]) - f64::from(lo[0]), 60.0));
    assert!(close(f64::from(hi[1]), 40.0));
    let plan = plan_symbol(&st, None);
    assert!(matches!(&plan[0], Stroke::Polyline(p, true) if p.len() == 6));
    // The outline survives JSON.
    let back: Stair = serde_json::from_str(&serde_json::to_string(&st).unwrap()).unwrap();
    assert_eq!(back, st);
}

#[test]
fn stringer_styles_change_the_boards() {
    let build = |stringer| {
        tagged_meshes(&stair(StairParams {
            stringer,
            ..params(109.125)
        }))
    };
    let closed = build(StringerStyle::Closed);
    assert_eq!(count(&closed, StairPart::Stringer), 2);
    // Open: a throat strip and a notch triangle per step, on both sides.
    let open = build(StringerStyle::Open);
    assert_eq!(count(&open, StairPart::Stringer), 2 * (1 + 14));
    assert_eq!(count(&build(StringerStyle::None), StairPart::Stringer), 0);
    for (_, m) in &open {
        assert!(m
            .vertices
            .iter()
            .all(|v| v.position.iter().all(|c| c.is_finite())));
    }
}

#[test]
fn open_and_closed_risers_change_the_riser_boards() {
    let closed = tagged_meshes(&stair(params(109.125)));
    let open = tagged_meshes(&stair(StairParams {
        open_risers: true,
        ..params(109.125)
    }));
    assert_eq!(count(&closed, StairPart::Riser), 15);
    assert_eq!(count(&open, StairPart::Riser), 0);
    assert_eq!(
        count(&closed, StairPart::Tread),
        count(&open, StairPart::Tread)
    );
}

#[test]
fn railing_balusters_follow_the_spacing() {
    let with = |spacing: f64| StairParams {
        railing: RailingParams {
            style: RailStyle::Balusters { spacing, size: 1.5 },
            ..RailingParams::default()
        },
        ..params(110.125)
    };
    // 14 treads, 10" deep: ceil(10 / (spacing + 1.5)) balusters per tread.
    let wide = stair_railing_geometry(&stair(with(4.0)), RailSide::Left, &with(4.0).railing);
    assert_eq!(wide.balusters.len(), 14 * 2);
    let tight = stair_railing_geometry(&stair(with(2.0)), RailSide::Left, &with(2.0).railing);
    assert_eq!(tight.balusters.len(), 14 * 3);
    // No opening wider than the spacing along the run.
    let mut xs: Vec<f64> = tight.balusters.iter().map(|b| b.0.x).collect();
    xs.sort_by(f64::total_cmp);
    assert!(xs.windows(2).all(|w| w[1] - w[0] - 1.5 <= 2.0 + 1e-9));
}

#[test]
fn stair_sides_become_railings_walls_and_half_walls() {
    let build = |left, right| {
        tagged_meshes(&stair(StairParams {
            left_side: left,
            right_side: right,
            ..params(110.125)
        }))
    };
    // Railings are tagged Handrail; walls and half-walls add Stringer boards
    // to the two real stringers.
    let none = build(SideKind::None, SideKind::None);
    assert_eq!(count(&none, StairPart::Handrail), 0);
    assert_eq!(count(&none, StairPart::Stringer), 2);
    let rail = build(SideKind::Railing, SideKind::None);
    let one = count(&rail, StairPart::Handrail);
    assert!(one > 14, "newels, balusters and a rail: {one}");
    assert_eq!(count(&rail, StairPart::Stringer), 2);
    let both = build(SideKind::Railing, SideKind::Railing);
    assert_eq!(count(&both, StairPart::Handrail), 2 * one);
    let half = build(SideKind::HalfWall, SideKind::None);
    let wall = build(SideKind::Wall, SideKind::None);
    assert!(count(&half, StairPart::Stringer) > 2);
    assert_eq!(count(&half, StairPart::Handrail), 0);
    let top = |parts: &[(StairPart, plan_3d::Mesh)]| {
        parts
            .iter()
            .filter(|(p, _)| *p == StairPart::Stringer)
            .map(|(_, m)| f64::from(m.bounds().unwrap().1[1]))
            .fold(f64::MIN, f64::max)
    };
    // The wall rises to headroom height; the half-wall stops at the rail.
    assert!(
        top(&wall) > top(&half) + 30.0,
        "{} {}",
        top(&wall),
        top(&half)
    );
    // All normals stay unit length.
    for (_, m) in half.iter().chain(&wall).chain(&both) {
        for v in &m.vertices {
            let l: f32 = v.normal.iter().map(|c| c * c).sum();
            assert!((l - 1.0).abs() < 1e-3);
        }
    }
}

#[test]
fn stair_sides_are_drawn_in_plan() {
    let plain = plan_symbol(&stair(params(110.125)), None).len();
    let railed = plan_symbol(
        &stair(StairParams {
            left_side: SideKind::Railing,
            ..params(110.125)
        }),
        None,
    )
    .len();
    // Two rail lines and two newel squares.
    assert_eq!(railed, plain + 4);
    let walled = plan_symbol(
        &stair(StairParams {
            right_side: SideKind::HalfWall,
            ..params(110.125)
        }),
        Some(2.0 / 3.0),
    )
    .len();
    assert_eq!(
        walled,
        plan_symbol(&stair(params(110.125)), Some(2.0 / 3.0)).len() + 1
    );
}

#[test]
fn a_curved_railing_follows_the_arc() {
    let p = |spacing: f64| RailingParams {
        style: RailStyle::Balusters { spacing, size: 1.5 },
        ..RailingParams::default()
    };
    let st = curved(60.0, 12.0, Turn::Left);
    // Left edge = inside radius 12: a tread is 4" long there -> 1 baluster per tread.
    let inside = stair_railing_geometry(&st, RailSide::Left, &p(4.0));
    assert_eq!(inside.balusters.len(), 7);
    // Right edge = radius 48: 16" per tread -> ceil(16 / 5.5) = 3 per tread.
    let outside = stair_railing_geometry(&st, RailSide::Right, &p(4.0));
    assert_eq!(outside.balusters.len(), 21);
    let centre = curve_center(&st).unwrap();
    for (pt, _, _) in &outside.balusters {
        assert!(close(pt.dist(centre), 48.0));
    }
    // One rail run per tread, rising a riser each.
    assert_eq!(outside.rails.len(), 7);
    let rise = outside.rails[1].1 - outside.rails[0].1;
    assert!(close(rise, solve(&st.params).riser_height));
    // A railing on a landing is nothing.
    let landing = stair(StairParams {
        shape: StairShape::Landing { depth: 36.0 },
        ..params(40.0)
    });
    assert!(stair_railing_geometry(&landing, RailSide::Left, &p(4.0))
        .rails
        .is_empty());
}

#[test]
fn new_fields_round_trip_and_old_json_still_loads() {
    let st = stair(StairParams {
        shape: StairShape::Curved { inner_radius: 14.0 },
        stringer: StringerStyle::Open,
        left_side: SideKind::Railing,
        right_side: SideKind::HalfWall,
        ..params(100.0)
    });
    let back: Stair = serde_json::from_str(&serde_json::to_string(&st).unwrap()).unwrap();
    assert_eq!(back, st);
    let old = r#"{"id":3,"origin":{"x":1.0,"y":2.0},"direction":0.0,"params":{"total_rise":100.0,"width":36.0,"shape":"Straight"},"floor_elevation":0.0}"#;
    let s: Stair = serde_json::from_str(old).unwrap();
    assert_eq!(s.params.left_side, SideKind::None);
    assert_eq!(s.params.stringer, StringerStyle::Closed);
    assert!(close(s.params.slab_thickness, 3.5));
}

#[test]
fn every_new_mesh_has_outward_normals_and_finite_positions() {
    let variants = [
        stair(StairParams {
            shape: StairShape::Curved { inner_radius: 30.0 },
            left_side: SideKind::Railing,
            right_side: SideKind::HalfWall,
            ..params(100.0)
        }),
        Stair::new(
            4,
            Point::new(10.0, 20.0),
            FRAC_PI_6,
            StairParams {
                shape: StairShape::Curved { inner_radius: 30.0 },
                turn: Turn::Right,
                ..params(100.0)
            },
        ),
        stair(StairParams {
            stringer: StringerStyle::Open,
            left_side: SideKind::Wall,
            ..params(110.125)
        }),
        stair(StairParams {
            shape: StairShape::Ramp { slope_1_in: 12.0 },
            left_side: SideKind::Railing,
            ..params(66.0)
        }),
        stair(StairParams {
            shape: StairShape::Landing { depth: 40.0 },
            ..params(48.0)
        }),
    ];
    for st in &variants {
        for (_, mesh) in tagged_meshes(st) {
            for tri in mesh.indices.chunks(3) {
                let v: Vec<_> = tri.iter().map(|&i| mesh.vertices[i as usize]).collect();
                let e1: Vec<f32> = (0..3)
                    .map(|k| v[1].position[k] - v[0].position[k])
                    .collect();
                let e2: Vec<f32> = (0..3)
                    .map(|k| v[2].position[k] - v[0].position[k])
                    .collect();
                let c = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                let dot: f32 = (0..3).map(|k| c[k] * v[0].normal[k]).sum();
                assert!(dot > 0.0, "{:?}", st.params.shape);
                assert!(v.iter().all(|p| p.position.iter().all(|x| x.is_finite())));
            }
        }
    }
}

#[test]
fn an_l_stair_gets_its_landing_automatically() {
    let st = stair(StairParams {
        shape: StairShape::LShaped {
            treads_before_landing: 6,
        },
        ..params(109.125)
    });
    assert_eq!(solve(&st.params).landings, 1);
    let parts = tagged_meshes(&st);
    assert_eq!(count(&parts, StairPart::Landing), 1);
    // The landing sits at the height of the seventh riser.
    let (_, hi) = parts
        .iter()
        .find(|(p, _)| *p == StairPart::Landing)
        .map(|(_, m)| m.bounds().unwrap())
        .unwrap();
    assert!(close(f64::from(hi[1]), 7.0 * 109.125 / 15.0));
}

#[test]
fn the_ramp_slab_sits_on_its_base_in_each_run() {
    let st = stair(StairParams {
        shape: StairShape::Ramp { slope_1_in: 12.0 },
        ..params(60.0)
    });
    let ramps: Vec<_> = tagged_meshes(&st)
        .into_iter()
        .filter(|(p, _)| *p == StairPart::Ramp)
        .collect();
    assert_eq!(ramps.len(), 2);
    let tops: Vec<f64> = ramps
        .iter()
        .map(|(_, m)| f64::from(m.bounds().unwrap().1[1]))
        .collect();
    assert!(close(tops[0], 30.0) && close(tops[1], 60.0), "{tops:?}");
}

#[test]
fn a_section_on_a_landing_starts_at_its_base_height() {
    let mut st = stair(params(60.0));
    st.floor_elevation = 100.0;
    st.base = 60.0;
    assert!(close(st.bottom_elevation(), 160.0));
    let (_, z) = top_point(&st);
    assert!(close(z, 220.0));
    let lowest_tread_top = tagged_meshes(&st)
        .iter()
        .filter(|(p, _)| *p == StairPart::Tread)
        .map(|(_, m)| f64::from(m.bounds().unwrap().1[1]))
        .fold(f64::MAX, f64::min);
    assert!(close(lowest_tread_top, 160.0 + 60.0 / 8.0));
    // Old files without `base` start at the floor.
    let old =
        r#"{"id":3,"origin":{"x":1.0,"y":2.0},"direction":0.0,"params":{},"floor_elevation":5.0}"#;
    let s: Stair = serde_json::from_str(old).unwrap();
    assert!(close(s.bottom_elevation(), 5.0));
}

// ----- flared bottom tread, spiral stairs, railings on landings -----

fn lateral_extent(mesh: &plan_3d::Mesh) -> f64 {
    // Direction 0: the lateral axis is plan y, scene -z.
    let zs: Vec<f64> = mesh
        .vertices
        .iter()
        .map(|v| f64::from(v.position[2]))
        .collect();
    zs.iter().cloned().fold(f64::MIN, f64::max) - zs.iter().cloned().fold(f64::MAX, f64::min)
}

fn lowest_y(mesh: &plan_3d::Mesh) -> f64 {
    mesh.vertices
        .iter()
        .map(|v| f64::from(v.position[1]))
        .fold(f64::MAX, f64::min)
}

#[test]
fn a_flared_bottom_tread_reaches_past_the_stair_on_both_sides() {
    let plain = stair(params(109.125));
    let flared = stair(StairParams {
        flare: 6.0,
        ..params(109.125)
    });
    let treads = |s: &Stair| {
        let mut t: Vec<plan_3d::Mesh> = tagged_meshes(s)
            .into_iter()
            .filter(|(p, _)| *p == StairPart::Tread)
            .map(|(_, m)| m)
            .collect();
        t.sort_by(|a, b| lowest_y(a).total_cmp(&lowest_y(b)));
        t
    };
    let (p, f) = (treads(&plain), treads(&flared));
    assert_eq!(p.len(), f.len());
    assert!(close(lateral_extent(&p[0]), 36.0));
    assert!(
        close(lateral_extent(&f[0]), 48.0),
        "{}",
        lateral_extent(&f[0])
    );
    // Only the bottom tread flares, and it keeps its height.
    for (a, b) in p.iter().zip(&f).skip(1) {
        assert!(close(lateral_extent(a), lateral_extent(b)));
    }
    assert!(close(lowest_y(&p[0]), lowest_y(&f[0])));
    // The plan symbol draws the apron outline, a closed shape wider than the stair.
    let apron = plan_symbol(&flared, None)
        .into_iter()
        .filter_map(|s| match s {
            Stroke::Polyline(pts, true) if pts.len() > 8 => Some(pts),
            _ => None,
        })
        .next()
        .expect("an apron outline");
    let (lo, hi) = (
        apron.iter().map(|p| p.y).fold(f64::MAX, f64::min),
        apron.iter().map(|p| p.y).fold(f64::MIN, f64::max),
    );
    // Stair from y = 50 (left) to y = 14 (right): 6" past each.
    assert!(close(hi, 56.0) && close(lo, 8.0), "{lo} {hi}");
    assert!(plan_symbol(&plain, None)
        .iter()
        .all(|s| !matches!(s, Stroke::Polyline(p, true) if p.len() > 8)));
    // Winders, curves, ramps and landings ignore it.
    for shape in [
        StairShape::Curved { inner_radius: 30.0 },
        StairShape::Ramp { slope_1_in: 12.0 },
        StairShape::Landing { depth: 36.0 },
    ] {
        let a = stair(StairParams {
            shape,
            flare: 6.0,
            ..params(60.0)
        });
        let b = stair(StairParams {
            shape,
            ..params(60.0)
        });
        assert_eq!(plan_symbol(&a, None), plan_symbol(&b, None), "{shape:?}");
    }
}

fn spiral(radius: f64, rise: f64) -> Stair {
    let walk = (SPIRAL_POLE_RADIUS + radius) * 0.5;
    stair(StairParams {
        shape: StairShape::Curved {
            inner_radius: SPIRAL_POLE_RADIUS,
        },
        spiral: true,
        width: radius - SPIRAL_POLE_RADIUS,
        tread_depth: walk * 0.5,
        ..params(rise)
    })
}

#[test]
fn a_spiral_stair_follows_the_spiral_code_not_the_straight_one() {
    // 60" across: 28" clear width, 8 1/2" treads at the walking line.
    let st = spiral(30.0, 109.125);
    let sol = solve(&st.params);
    assert!(sol.code_ok, "{:?}", sol.warnings);
    assert!(sol.riser_height <= SPIRAL_MAX_RISER);
    // The same numbers on an ordinary curved stair break the 10" tread and
    // 36" width limits.
    let plain = StairParams {
        spiral: false,
        ..st.params.clone()
    };
    assert!(!solve(&plain).code_ok);
    // A 9" riser is fine on a spiral, never on a stair.
    let tall = StairParams {
        riser_height_target: 9.0,
        ..st.params.clone()
    };
    assert!(solve(&tall).code_ok, "{:?}", solve(&tall).warnings);
    assert!(solve(&tall).riser_height > MAX_RISER);
    // Too narrow, too shallow or too steep fails the spiral code.
    for bad in [
        StairParams {
            width: 24.0,
            ..st.params.clone()
        },
        StairParams {
            tread_depth: 6.0,
            ..st.params.clone()
        },
        StairParams {
            headroom_min: 76.0,
            ..st.params.clone()
        },
    ] {
        assert!(!solve(&bad).code_ok, "{bad:?}");
    }
}

#[test]
fn a_spiral_stair_turns_more_than_once_around_a_centre_pole() {
    let st = spiral(30.0, 109.125);
    let sweep = curve_sweep(&st).unwrap();
    assert!(sweep > std::f64::consts::TAU, "{sweep}");
    let centre = curve_center(&st).unwrap();
    // Treads are wedges around the centre: every tread corner is between
    // the pole and the outside radius.
    let parts = tagged_meshes(&st);
    let treads = count(&parts, StairPart::Tread);
    assert_eq!(treads as u32, solve(&st.params).treads);
    for (part, m) in &parts {
        if *part != StairPart::Tread {
            continue;
        }
        for v in &m.vertices {
            let p = Point::new(f64::from(v.position[0]), -f64::from(v.position[2]));
            let r = p.dist(centre);
            assert!(r > SPIRAL_POLE_RADIUS - 1e-3 && r < 30.0 + 1e-3, "{r}");
        }
    }
    // The pole stands from the floor above the top step; an ordinary curved
    // stair has none.
    let pole = parts
        .iter()
        .filter(|(p, _)| *p == StairPart::Stringer)
        .map(|(_, m)| m)
        .find(|m| {
            m.vertices.iter().all(|v| {
                Point::new(f64::from(v.position[0]), -f64::from(v.position[2])).dist(centre)
                    < SPIRAL_POLE_RADIUS + 1e-3
            })
        })
        .expect("a centre pole");
    let top = pole
        .vertices
        .iter()
        .map(|v| f64::from(v.position[1]))
        .fold(f64::MIN, f64::max);
    assert!(top > solve(&st.params).riser_height * f64::from(solve(&st.params).risers));
    let no_pole = StairParams {
        spiral: false,
        ..st.params.clone()
    };
    assert!(tagged_meshes(&stair(no_pole)).len() < parts.len());
    // The plan symbol has the pole as a circle at the centre.
    assert!(plan_symbol(&st, None).iter().any(|s| matches!(
        s,
        Stroke::Arc { center, radius, .. }
            if center.dist(centre) < 1e-6 && close(*radius, SPIRAL_POLE_RADIUS)
    )));
    // It round-trips.
    let back: Stair = serde_json::from_str(&serde_json::to_string(&st).unwrap()).unwrap();
    assert_eq!(back, st);
}

fn rail_landing(left: SideKind, right: SideKind) -> Stair {
    stair(StairParams {
        shape: StairShape::Landing { depth: 48.0 },
        left_side: left,
        right_side: right,
        ..params(48.0)
    })
}

#[test]
fn a_landing_railing_guards_the_open_sides_not_the_ends() {
    let st = rail_landing(SideKind::Railing, SideKind::None);
    // Direction 0: the left side is the edge at the origin's y.
    let left = landing_edges(&st, RailSide::Left);
    assert_eq!(left.len(), 1);
    assert!(close(left[0].0.y, 50.0) && close(left[0].1.y, 50.0));
    assert!(close(left[0].0.dist(left[0].1), 48.0));
    let right = landing_edges(&st, RailSide::Right);
    assert_eq!(right.len(), 1);
    assert!(close(right[0].0.y, 14.0));
    // Meshes: a railing on the left only; its newels stand on the landing top.
    let parts = tagged_meshes(&st);
    let rails: Vec<&plan_3d::Mesh> = parts
        .iter()
        .filter(|(p, _)| *p == StairPart::Handrail)
        .map(|(_, m)| m)
        .collect();
    assert!(!rails.is_empty());
    for m in &rails {
        for v in &m.vertices {
            let y = -f64::from(v.position[2]);
            assert!(y > 50.0 - 4.0 && y < 50.0 + 4.0, "rail strays to y={y}");
        }
    }
    let ys: Vec<f64> = rails
        .iter()
        .flat_map(|m| m.vertices.iter().map(|v| f64::from(v.position[1])))
        .collect();
    let (lo, hi) = (
        ys.iter().cloned().fold(f64::MAX, f64::min),
        ys.iter().cloned().fold(f64::MIN, f64::max),
    );
    assert!(close(lo, 48.0), "{lo}");
    assert!(hi >= 48.0 + GUARD_HEIGHT - 2.0, "{hi}");
    // Both sides doubles it; none leaves the landing bare.
    let both = rail_landing(SideKind::Railing, SideKind::Railing);
    assert!(tagged_meshes(&both).len() > parts.len());
    let bare = rail_landing(SideKind::None, SideKind::None);
    assert_eq!(count(&tagged_meshes(&bare), StairPart::Handrail), 0);
    // The plan symbol shows the railing as double lines with newel squares.
    let strokes = |s: &Stair| plan_symbol(s, None).len();
    assert!(strokes(&st) > strokes(&bare) + 2);
    // A half wall is a railing on a low wall: the same edges, more solid.
    let half = rail_landing(SideKind::HalfWall, SideKind::None);
    assert!(count(&tagged_meshes(&half), StairPart::Handrail) > 0);
    // Edges of anything but a landing are none.
    assert!(landing_edges(&stair(params(100.0)), RailSide::Left).is_empty());
}

#[test]
fn a_polygon_landing_railing_follows_the_sides_that_face_left_and_right() {
    let outline = vec![
        Point::new(100.0, 50.0),
        Point::new(100.0, 14.0),
        Point::new(148.0, 14.0),
        Point::new(148.0, 50.0),
    ];
    let st = stair(StairParams {
        shape: StairShape::Landing { depth: 48.0 },
        outline,
        left_side: SideKind::Railing,
        ..params(48.0)
    });
    let left = landing_edges(&st, RailSide::Left);
    assert_eq!(left.len(), 1);
    assert!(close(left[0].0.y, 50.0) && close(left[0].1.y, 50.0));
}

#[test]
fn the_rail_of_an_l_stair_carries_across_the_landing_in_plan() {
    let base = StairParams {
        shape: StairShape::LShaped {
            treads_before_landing: 6,
        },
        turn: Turn::Left,
        ..params(109.125)
    };
    let bare = stair(base.clone());
    let railed = stair(StairParams {
        left_side: SideKind::Railing,
        right_side: SideKind::Railing,
        ..base
    });
    // Turning left, the left rail is the inside one: the two flights'
    // rails meet at the corner. The right (outside) rail runs around it.
    assert!(landing_rail_paths(&railed, RailSide::Left).is_empty());
    let outside = landing_rail_paths(&railed, RailSide::Right);
    assert_eq!(outside.len(), 1);
    assert!(outside[0].len() >= 3, "{:?}", outside[0]);
    // Every landing rail line is drawn: more strokes than the two flights'
    // rails alone, and some line runs across the landing region.
    let layout_u = 6.0 * 10.0;
    let landing_lines = plan_symbol(&railed, None)
        .into_iter()
        .filter(|s| !plan_symbol(&bare, None).contains(s))
        .filter(|s| match s {
            Stroke::Line(a, b) => a.x.min(b.x) >= 100.0 + layout_u - 1e-6 && a.dist(*b) > 10.0,
            _ => false,
        })
        .count();
    assert!(landing_lines >= 4, "{landing_lines}");
}
