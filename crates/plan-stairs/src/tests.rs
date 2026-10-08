use super::*;
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
