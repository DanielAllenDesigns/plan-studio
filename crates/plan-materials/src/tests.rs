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

// ----- round 14: Material Specification -----

#[test]
fn a_class_sets_typical_values_and_holds_the_surface_in_its_range() {
    let mut d = MaterialDef::new("M", &["Custom"], [10, 20, 30]);
    assert_eq!(d.class, MaterialClass::General);
    assert_eq!(d.surface().roughness, 0.8);
    d.set_class(MaterialClass::Mirror);
    let s = d.surface();
    assert_eq!((s.metallic, s.transparency), (1.0, 0.0));
    assert!(s.roughness <= 0.05);
    // Sliders moved after the class was picked cannot break the class.
    d.roughness = 0.9;
    d.metallic = 0.0;
    d.transparency = 0.7;
    let s = d.surface();
    assert!(s.metallic == 1.0 && s.roughness <= 0.05 && s.transparency == 0.0);
    d.set_class(MaterialClass::Glass);
    assert!(d.surface().transparency >= 0.5 && d.rgba_f32()[3] <= 0.5);
    d.set_class(MaterialClass::Emissive);
    assert!(d.surface().emissive >= 0.3);
    d.set_class(MaterialClass::Plastic);
    d.metallic = 0.9;
    assert_eq!(d.surface().metallic, 0.0);
    d.set_class(MaterialClass::Metal);
    d.metallic = 0.1;
    assert!(d.surface().metallic >= 0.8);
    d.set_class(MaterialClass::Transparent);
    d.transparency = 0.0;
    assert!(d.surface().transparency >= 0.2);
    for c in MaterialClass::ALL {
        assert_eq!(MaterialClass::from_name(c.name()), Some(c));
        let t = c.typical();
        for v in [t.roughness, t.metallic, t.transparency, t.emissive] {
            assert!((0.0..=1.0).contains(&v));
        }
    }
    assert_eq!(MaterialClass::from_name("nope"), None);
}

#[test]
fn glass_and_metal_library_entries_carry_their_class() {
    let lib = core_library();
    assert_eq!(lib.find("Clear Glass").unwrap().class, MaterialClass::Glass);
    assert_eq!(lib.find("Chrome").unwrap().class, MaterialClass::Metal);
    assert!(lib.find("Chrome").unwrap().surface().metallic >= 0.8);
    assert_eq!(
        scene_material(lib.find("Chrome").unwrap()),
        plan_3d::Material::Metal
    );
    let mut mirror = MaterialDef::new("Mirror", &["Glass"], [200, 200, 210]);
    mirror.set_class(MaterialClass::Mirror);
    assert_eq!(scene_material(&mirror), plan_3d::Material::Metal);
}

#[test]
fn the_new_fields_round_trip_and_old_files_get_defaults() {
    let mut d = MaterialDef::new("Slate", &["Flooring"], [60, 60, 70]);
    d.pattern_scale = 2.0;
    d.pattern_angle = 30.0;
    d.texture_offset_in = (3.0, -2.0);
    d.texture_angle_deg = 45.0;
    d.blend_color = Some([9, 9, 9]);
    d.blend_amount = 0.25;
    d.manufacturer = "Acme".into();
    d.supplier = "Depot".into();
    d.price = 4.5;
    d.unit = PriceUnit::SqM;
    d.set_class(MaterialClass::Plastic);
    let mut lib = MaterialLibrary::default();
    lib.add(d.clone());
    let back = MaterialLibrary::from_json(&lib.to_json().unwrap()).unwrap();
    assert_eq!(back.find("Slate"), Some(&d));
    // A library written before this round has none of the fields.
    let mut v = serde_json::to_value(&lib).unwrap();
    let o = v["materials"][0].as_object_mut().unwrap();
    for k in [
        "class",
        "pattern_scale",
        "pattern_angle",
        "texture_offset_in",
        "texture_angle_deg",
        "blend_color",
        "blend_amount",
        "manufacturer",
        "supplier",
        "price",
        "unit",
    ] {
        o.remove(k);
    }
    let old: MaterialLibrary = serde_json::from_value(v).unwrap();
    let m = &old.materials[0];
    assert_eq!(m.class, MaterialClass::General);
    assert_eq!((m.pattern_scale, m.pattern_angle), (1.0, 0.0));
    assert_eq!(m.unit, PriceUnit::SqFt);
    assert_eq!(m.price, 0.0);
}

#[test]
fn prices_come_from_the_quote_or_the_older_cost() {
    let mut d = MaterialDef::new("P", &["Custom"], [0; 3]).with_cost(2.0, "C");
    assert_eq!(
        (d.quoted_price(), d.price_per_sq_ft()),
        ((2.0, PriceUnit::SqFt), 2.0)
    );
    d.price = 18.0;
    d.unit = PriceUnit::SqYd;
    assert_eq!(d.quoted_price(), (18.0, PriceUnit::SqYd));
    assert!((d.price_per_sq_ft() - 2.0).abs() < 1e-9);
    d.unit = PriceUnit::Each;
    assert_eq!(d.price_per_sq_ft(), 0.0);
}

#[test]
fn pattern_scale_and_angle_change_the_hatch() {
    let r = rect(96.0, 48.0);
    let base = pattern_strokes(&Pattern::brick(), r, 0.25);
    let mut d = MaterialDef::new("B", &["Masonry"], [150, 70, 50]).with_pattern(Pattern::brick());
    assert_eq!(
        d.hatch_strokes(r, 0.25),
        base,
        "scale 1, angle 0 is the plain pattern"
    );
    d.pattern_scale = 2.0;
    let big = d.hatch_strokes(r, 0.25);
    assert!(big.len() < base.len(), "bigger bricks, fewer joints");
    assert_eq!(
        Pattern::brick().scaled(2.0),
        Pattern::Brick {
            length: 16.0,
            height: 4.5
        }
    );
    // Line sets turn in place and stay inside the rectangle.
    let mut lines = MaterialDef::new("L", &["Custom"], [0; 3]).with_pattern(Pattern::Lines {
        angle_deg: 0.0,
        spacing: 6.0,
    });
    assert!(lines
        .hatch_strokes(r, 0.25)
        .iter()
        .all(|(a, b)| (a.y - b.y).abs() < 1e-9));
    lines.pattern_angle = 90.0;
    let up = lines.hatch_strokes(r, 0.25);
    assert!(!up.is_empty() && up.iter().all(|(a, b)| (a.x - b.x).abs() < 1e-6));
    assert!(up
        .iter()
        .all(|(a, b)| inside(*a, 96.0, 48.0) && inside(*b, 96.0, 48.0)));
    // A turned brick pattern is clipped by the caller; after clipping it
    // fills the polygon and differs from the unturned one.
    d.pattern_scale = 1.0;
    d.pattern_angle = 30.0;
    let poly = [
        Point::new(0.0, 0.0),
        Point::new(96.0, 0.0),
        Point::new(96.0, 48.0),
        Point::new(0.0, 48.0),
    ];
    let clipped = clip_strokes_to_polygon(&d.hatch_strokes(r, 0.25), &poly);
    assert!(!clipped.is_empty());
    assert!(clipped
        .iter()
        .all(|(a, b)| inside(*a, 96.0, 48.0) && inside(*b, 96.0, 48.0)));
    assert_ne!(clipped, clip_strokes_to_polygon(&base, &poly));
    // Nothing to scale in the stipple patterns; bad factors change nothing.
    assert_eq!(Pattern::Concrete.scaled(3.0), Pattern::Concrete);
    assert_eq!(Pattern::brick().scaled(-1.0), Pattern::brick());
}

#[test]
fn paint_modes_and_scopes_have_distinct_labels() {
    let modes: HashSet<&str> = PaintMode::ALL.iter().map(|m| m.label()).collect();
    assert_eq!(modes.len(), 6);
    assert!(modes.contains("Blend Colors") && modes.contains("Component"));
    assert!(PaintMode::Plan.is_wide() && !PaintMode::Object.is_wide());
    let scopes: HashSet<&str> = PaintScope::ALL.iter().map(|m| m.label()).collect();
    assert_eq!(scopes.len(), 3);
    assert_eq!(PaintMode::default(), PaintMode::Object);
}
