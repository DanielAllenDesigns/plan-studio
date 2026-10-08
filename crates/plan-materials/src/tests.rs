use std::collections::HashSet;

use plan_core::Point;

use super::*;

fn rect(w: f64, h: f64) -> (Point, Point) {
    (Point::new(0.0, 0.0), Point::new(w, h))
}

fn inside(p: Point, w: f64, h: f64) -> bool {
    let e = 1e-9;
    p.x >= -e && p.x <= w + e && p.y >= -e && p.y <= h + e
}

#[test]
fn core_library_has_unique_names() {
    let lib = core_library();
    let names: HashSet<&str> = lib.materials.iter().map(|m| m.name.as_str()).collect();
    assert!(lib.materials.len() >= 44);
    assert_eq!(names.len(), lib.materials.len(), "duplicate material names");
    for n in [
        "Sand Finish – Eggshell",
        "Drywall",
        "Fir Framing",
        "OSB-Hrz",
        "Housewrap",
        "Lincoln Door",
        "Water",
    ] {
        assert!(lib.find(n).is_some(), "missing {n}");
    }
}

#[test]
fn find_search_and_categories() {
    let lib = core_library();
    assert_eq!(
        lib.find("drywall").map(|m| m.name.as_str()),
        Some("Drywall")
    );
    assert!(lib.find("Nope").is_none());
    let brick = lib.search("brick");
    assert!(
        brick.iter().any(|m| m.name == "Brick – Red")
            && brick.iter().any(|m| m.name == "Brick – Tan")
    );
    assert_eq!(lib.search("").len(), lib.materials.len());
    let cats = lib.by_category();
    assert!(cats["Flooring"].iter().any(|m| m.name == "Oak Flooring"));
    assert_eq!(
        cats.values().map(Vec::len).sum::<usize>(),
        lib.materials.len()
    );
}

#[test]
fn library_json_round_trip() {
    let lib = core_library();
    let back = MaterialLibrary::from_json(&lib.to_json().unwrap()).unwrap();
    assert_eq!(lib, back);
}

#[test]
fn brick_courses_are_staggered_and_inside() {
    let strokes = pattern_strokes(&Pattern::brick(), rect(48.0, 24.0), 0.5);
    assert!(!strokes.is_empty() && strokes.len() <= MAX_STROKES);
    assert!(strokes
        .iter()
        .all(|&(a, b)| inside(a, 48.0, 24.0) && inside(b, 48.0, 24.0)));
    // Head joints: vertical segments, grouped per course.
    let joints = |course: usize| -> Vec<f64> {
        let (lo, hi) = (course as f64 * 2.25, (course + 1) as f64 * 2.25);
        let mut xs: Vec<f64> = strokes
            .iter()
            .filter(|(a, b)| {
                (a.x - b.x).abs() < 1e-9 && (a.y - lo).abs() < 1e-9 && (b.y - hi).abs() < 1e-9
            })
            .map(|(a, _)| a.x)
            .collect();
        xs.sort_by(f64::total_cmp);
        xs
    };
    let (even, odd) = (joints(0), joints(1));
    assert_eq!(even, vec![8.0, 16.0, 24.0, 32.0, 40.0]);
    assert_eq!(odd, vec![4.0, 12.0, 20.0, 28.0, 36.0, 44.0]);
    assert_eq!(joints(2), even);
}

#[test]
fn every_pattern_stays_inside_and_bounded() {
    let patterns = [
        Pattern::Lines {
            angle_deg: 45.0,
            spacing: 4.0,
        },
        Pattern::CrossHatch {
            angle_deg: 30.0,
            spacing: 3.0,
        },
        Pattern::brick(),
        Pattern::block(),
        Pattern::shingle(),
        Pattern::lap_siding(),
        Pattern::board_and_batten(),
        Pattern::Tile { w: 12.0, h: 12.0 },
        Pattern::Herringbone {
            length: 6.0,
            width: 2.0,
        },
        Pattern::Insulation,
        Pattern::Concrete,
        Pattern::Earth,
        Pattern::Grass,
    ];
    let r = (Point::new(100.0, -50.0), Point::new(220.0, 40.0));
    for p in &patterns {
        let s = pattern_strokes(p, r, 0.25);
        assert!(!s.is_empty(), "{p:?} produced nothing");
        assert!(s.len() <= MAX_STROKES);
        for &(a, b) in &s {
            for q in [a, b] {
                assert!(
                    q.x >= 100.0 - 1e-9
                        && q.x <= 220.0 + 1e-9
                        && q.y >= -50.0 - 1e-9
                        && q.y <= 40.0 + 1e-9,
                    "{p:?}"
                );
            }
        }
    }
    assert!(pattern_strokes(&Pattern::None, r, 0.25).is_empty());
    assert!(pattern_strokes(
        &Pattern::brick(),
        (Point::ZERO, Point::new(0.0, 10.0)),
        0.25
    )
    .is_empty());
}

#[test]
fn huge_regions_are_capped() {
    let big = (Point::ZERO, Point::new(1.0e6, 1.0e6));
    for p in [
        Pattern::Lines {
            angle_deg: 0.0,
            spacing: 1.0,
        },
        Pattern::Tile { w: 1.0, h: 1.0 },
        Pattern::Concrete,
        Pattern::Grass,
    ] {
        let n = pattern_strokes(&p, big, 0.0).len();
        assert!(n > 0 && n <= MAX_STROKES, "{p:?}: {n}");
    }
}

#[test]
fn small_scales_thin_hatches() {
    let p = Pattern::Lines {
        angle_deg: 0.0,
        spacing: 2.0,
    };
    let dense = pattern_strokes(&p, rect(100.0, 100.0), 1.0).len();
    let thin = pattern_strokes(&p, rect(100.0, 100.0), 0.0625).len();
    assert!(thin < dense);
}

#[test]
fn polygon_clip_keeps_inside_parts() {
    let tri = [
        Point::new(0.0, 0.0),
        Point::new(48.0, 0.0),
        Point::new(0.0, 48.0),
    ];
    let strokes = [
        (Point::new(-10.0, 10.0), Point::new(60.0, 10.0)),
        (Point::new(-10.0, 60.0), Point::new(60.0, 60.0)),
    ];
    let out = clip_strokes_to_polygon(&strokes, &tri);
    assert_eq!(out.len(), 1);
    let (a, b) = out[0];
    assert!((a.x - 0.0).abs() < 1e-9 && (b.x - 38.0).abs() < 1e-9 && (a.y - 10.0).abs() < 1e-9);
    // Hatching a rect then clipping never leaves the triangle.
    let hatch = pattern_strokes(
        &Pattern::Lines {
            angle_deg: 45.0,
            spacing: 3.0,
        },
        rect(48.0, 48.0),
        0.25,
    );
    let clipped = clip_strokes_to_polygon(&hatch, &tri);
    assert!(!clipped.is_empty());
    for (p, q) in clipped {
        assert!(
            p.x + p.y <= 48.0 + 1e-6 && q.x + q.y <= 48.0 + 1e-6 && p.x >= -1e-9 && p.y >= -1e-9
        );
    }
}

#[test]
fn procedural_textures_tile() {
    let lib = core_library();
    let n = 128u32;
    let mut checked = 0;
    for m in lib
        .materials
        .iter()
        .filter(|m| matches!(m.texture, Texture::Procedural(_)))
    {
        let px = render_texture(m, n);
        assert_eq!(px.len(), (n * n * 4) as usize, "{}", m.name);
        let at = |x: u32, y: u32, c: usize| f64::from(px[((y * n + x) * 4) as usize + c]);
        // Mean RGB step between two columns (or rows) of pixels.
        let col_diff = |x0: u32, x1: u32| -> f64 {
            (0..n)
                .flat_map(|y| (0..3).map(move |c| (y, c)))
                .map(|(y, c)| (at(x0, y, c) - at(x1, y, c)).abs())
                .sum::<f64>()
                / f64::from(n * 3)
        };
        let row_diff = |y0: u32, y1: u32| -> f64 {
            (0..n)
                .flat_map(|x| (0..3).map(move |c| (x, c)))
                .map(|(x, c)| (at(x, y0, c) - at(x, y1, c)).abs())
                .sum::<f64>()
                / f64::from(n * 3)
        };
        // The wrap-around step must look like any interior step.
        let h_base = (1..n).map(|x| col_diff(x - 1, x)).sum::<f64>() / f64::from(n - 1);
        let v_base = (1..n).map(|y| row_diff(y - 1, y)).sum::<f64>() / f64::from(n - 1);
        let (h, v) = (col_diff(0, n - 1), row_diff(0, n - 1));
        assert!(
            h <= h_base * 1.5 + 6.0,
            "{} horizontal seam {h} vs {h_base}",
            m.name
        );
        assert!(
            v <= v_base * 1.5 + 6.0,
            "{} vertical seam {v} vs {v_base}",
            m.name
        );
        assert!(h < 25.0 && v < 25.0, "{} seam too visible: {h} {v}", m.name);
        checked += 1;
    }
    assert!(checked >= 20);
}

#[test]
fn textures_are_deterministic_and_default_size() {
    let lib = core_library();
    let brick = lib.find("Brick – Red").unwrap();
    assert_eq!(render_texture(brick, 32), render_texture(brick, 32));
    assert_ne!(
        render_texture(brick, 32),
        render_texture(lib.find("Brick – Tan").unwrap(), 32)
    );
    assert_eq!(
        render_texture(brick, DEFAULT_TEXTURE_SIZE).len(),
        256 * 256 * 4
    );
    assert!(render_texture(brick, 0).is_empty());
    // Solid fill uses the base colour; glass alpha reflects transparency.
    let drywall = lib.find("Drywall").unwrap();
    assert_eq!(&render_texture(drywall, 2)[..4], &[232, 230, 224, 255]);
    let glass = render_texture(lib.find("Clear Glass").unwrap(), 4);
    assert_eq!(glass[3], 38);
}

#[test]
fn assignments_reference_library_materials() {
    let lib = core_library();
    let wall = default_assignments_for("Wall");
    assert_eq!(wall[0].component, "Exterior Wall Surface");
    assert_eq!(wall[0].material, "Sand Finish – Eggshell");
    assert_eq!(wall.len(), 3);
    for kind in ["Wall", "Door", "Window", "Room", "Cabinet", "Roof"] {
        let a = default_assignments_for(kind);
        assert!(a.len() >= 3, "{kind}");
        for x in a {
            assert!(
                lib.find(&x.material).is_some(),
                "{kind}/{}: {}",
                x.component,
                x.material
            );
        }
    }
    assert_eq!(default_assignments_for("window").len(), 5);
    assert!(default_assignments_for("spaceship").is_empty());
}

#[test]
fn technique_presets() {
    let ld = settings(RenderingTechnique::LineDrawing);
    assert!(ld.edge_lines);
    assert_eq!(ld.fill, FillMode::SolidColor([255, 255, 255]));
    let vv = settings(RenderingTechnique::VectorView);
    assert!(
        vv.edge_lines
            && vv.shading == ShadingModel::Flat
            && vv.fill == FillMode::Material
            && vv.edge_color == [0, 0, 0]
    );
    let gh = settings(RenderingTechnique::GlassHouse);
    assert!(gh.edge_lines && matches!(gh.fill, FillMode::Transparent(a) if a > 0.0 && a < 1.0));
    let clay = settings(RenderingTechnique::Clay);
    assert!(
        !clay.edge_lines
            && clay.shading == ShadingModel::Lit
            && matches!(clay.fill, FillMode::SolidColor(_))
    );
    assert!(settings(RenderingTechnique::Duotone).shadow_color.is_some());
    assert_eq!(RenderingTechnique::ALL.len(), 9);
    let json = serde_json::to_string(&settings(RenderingTechnique::Watercolor)).unwrap();
    let back: TechniqueSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(back, settings(RenderingTechnique::Watercolor));
}

#[test]
fn sun_position() {
    let noon = SunSettings::from_date_time_location((6, 21), 12.0, 34.0);
    assert!(
        (noon.altitude_deg - 79.4).abs() < 1.0,
        "{}",
        noon.altitude_deg
    );
    assert!(
        (noon.azimuth_deg - 180.0).abs() < 1.0,
        "{}",
        noon.azimuth_deg
    );
    assert!(noon.intensity > 0.9);
    let morning = SunSettings::from_date_time_location((6, 21), 8.0, 34.0);
    let evening = SunSettings::from_date_time_location((6, 21), 16.0, 34.0);
    assert!(morning.azimuth_deg < 180.0 && evening.azimuth_deg > 180.0);
    let winter = SunSettings::from_date_time_location((12, 21), 12.0, 34.0);
    assert!(
        (winter.altitude_deg - 32.6).abs() < 1.0,
        "{}",
        winter.altitude_deg
    );
    let night = SunSettings::from_date_time_location((6, 21), 0.0, 34.0);
    assert!(night.altitude_deg < 0.0 && night.intensity == 0.0);
    let d = noon.direction_to_sun();
    assert!((d.iter().map(|v| v * v).sum::<f64>() - 1.0).abs() < 1e-9 && d[1] > 0.9);
    assert!(d[2] > 0.0, "sun is south at noon, i.e. +Z");
}

#[test]
fn room_light_hangs_below_ceiling() {
    let l = default_room_light(Point::new(120.0, 96.0), 96.0);
    assert_eq!(l.kind, LightKind::Point);
    assert_eq!(l.position, [120.0, 96.0, 84.0]);
    assert!(l.on);
}
