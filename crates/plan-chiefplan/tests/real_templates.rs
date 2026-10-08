//! Tests against Daniel's real Chief templates. They read the files in place
//! (nothing is copied) and are `#[ignore]`d because CI has no Chief install.
//! Run with `cargo test -p plan-chiefplan -- --ignored --nocapture`.

use plan_chiefplan::{
    build_inventory, build_inventory_with_values, calibrate, find_template_files, scan,
    templates_dir, Category, Confidence, TemplateKind, DEFAULT_LAYOUT_TEMPLATE,
    DEFAULT_PLAN_TEMPLATE,
};

fn template(name: &str) -> std::path::PathBuf {
    templates_dir().expect("HOME").join(name)
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn default_plan_template_names() {
    let inv = build_inventory(template(DEFAULT_PLAN_TEMPLATE)).unwrap();
    println!(
        "plan: {} strings ({} distinct), {} layer sets, {} layers, {} text styles, {} dimension sets, {} wall types, {} plan views, {} resources",
        inv.total_strings,
        inv.distinct_strings,
        inv.layer_sets.len(),
        inv.layers.len(),
        inv.text_styles.len(),
        inv.dimension_defaults.len(),
        inv.wall_types.len(),
        inv.plan_views.len(),
        inv.resources.len()
    );
    assert_eq!(inv.kind, Some(TemplateKind::Plan));
    assert!(
        inv.layer_sets.len() >= 20,
        "{} layer sets",
        inv.layer_sets.len()
    );
    for (cat, name) in [
        (Category::WallType, "Siding-6"),
        (Category::WallType, "Stucco-6"),
        (
            Category::DimensionDefaults,
            "1/4\" Scale Dimension Defaults",
        ),
        (Category::PlanView, "Floor Plan View Dimensioned"),
    ] {
        assert!(inv.contains(cat, name), "missing {name} in {cat:?}");
    }
    assert!(inv.thumbnail_bytes > 1000);
    assert!(inv.contains(Category::SheetSize, "ARCH C (18\" x 24\")"));
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn default_layout_template_names() {
    let inv = build_inventory(template(DEFAULT_LAYOUT_TEMPLATE)).unwrap();
    println!(
        "layout: {} strings, {} layer sets, {} sheet sizes, {} layout pages, {} macros",
        inv.total_strings,
        inv.layer_sets.len(),
        inv.sheet_sizes.len(),
        inv.layout_pages.len(),
        inv.title_block_macros.len()
    );
    assert_eq!(inv.kind, Some(TemplateKind::Layout));
    // The layout stores its sheet as ANSI B / US Letter entries; the
    // `ARCH C (18" x 24")` name lives in the plan template, not here.
    assert!(inv.contains(Category::SheetSize, "US Letter"));
    assert!(inv.contains(Category::SheetSize, "ANSI B (11\" x 17\")"));
    assert!(!inv.contains(Category::SheetSize, "ARCH C (18\" x 24\")"));
    assert!(inv.contains(Category::LayoutPage, "Page Template"));
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn every_template_parses_and_kind_matches_extension() {
    let dir = templates_dir().expect("HOME");
    let files = find_template_files(&dir);
    assert!(files.len() >= 20, "{} files", files.len());
    for f in files {
        let s = scan(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert_eq!(
            s.content_kind,
            s.extension_kind,
            "content vs extension for {}",
            f.display()
        );
        assert!(s.thumbnail_png.is_some(), "no thumbnail in {}", f.display());
        assert!(s.strings.len() > 500, "{}", f.display());
    }
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn layer_values_decode_with_expected_colours() {
    let (inv, rep) = build_inventory_with_values(template(DEFAULT_PLAN_TEMPLATE)).unwrap();
    println!(
        "values: {} layer sets decoded ({} named), {}/{} structural records",
        rep.layer_sets.len(),
        rep.layer_sets.iter().filter(|s| s.named).count(),
        rep.structural_records,
        rep.candidate_records
    );
    assert!(rep.layer_sets.len() >= 40);
    let set = &rep.layer_sets[0];
    println!("first set {:?}: {} layers", set.name, set.layers.len());
    assert!(set.layers.len() >= 300);
    let find = |n: &str| set.layers.iter().find(|l| l.name == n).unwrap();
    assert_eq!(find("Revision Clouds, General").color, [185, 0, 0]);
    assert_eq!(find("Dimensions, Electrical").color, [0, 0, 128]);
    assert!(inv.contains(Category::Layer, "Walls, Default Fill Color"));
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn calibration_across_all_templates() {
    let dir = templates_dir().expect("HOME");
    let reports: Vec<_> = find_template_files(&dir)
        .iter()
        .map(|p| build_inventory_with_values(p).unwrap().1)
        .collect();
    let cal = calibrate(&reports);
    println!(
        "calibration: {} files, {} sets, {}/{} structural records, {} wall stacks",
        cal.files,
        cal.layer_sets,
        cal.structural_records,
        cal.candidate_records,
        cal.wall_stacks_found
    );
    for f in &cal.fields {
        println!(
            "  {:18} {:?} same={:?} other={:?}",
            f.field, f.confidence, f.same_set_agreement, f.other_set_agreement
        );
    }
    let color = cal
        .fields
        .iter()
        .find(|f| f.field.starts_with("color"))
        .unwrap();
    assert!(color.confidence >= Confidence::Medium);
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn seed_from_default_templates() {
    use plan_chiefplan::{seed_defaults, TemplateSeed};
    use plan_core::PlanDefaults;

    let plan = build_inventory(template(DEFAULT_PLAN_TEMPLATE)).unwrap();
    let base = PlanDefaults::default();
    let (n_types, n_layers) = (base.wall_types.len(), base.layers.layers.len());
    let seed = seed_defaults(&plan, base);
    println!(
        "seed: +{} wall types ({} approximate), +{} layers, {} layer sets ({} with visible-layer lists), {} text styles, {} dimension sets, {} plan views",
        seed.added_wall_types.len(),
        seed.approximate_wall_types.len(),
        seed.added_layers.len(),
        seed.layer_sets.len(),
        seed.layer_sets.iter().filter(|s| s.visible_layers.is_some()).count(),
        seed.text_styles.len(),
        seed.dimension_sets.len(),
        seed.plan_views.len()
    );
    assert!(seed.defaults.wall_types.len() > n_types + 50);
    assert!(seed.defaults.layers.layers.len() > n_layers + 100);
    // `Walls,  Normal` must not duplicate the existing `Walls, Normal`.
    assert!(seed.defaults.layers.get("Walls,  Normal").is_none());
    assert!(seed
        .layer_sets
        .iter()
        .any(|s| s.name == "Floor Plan Dimensioned Layer Set"));
    let back: TemplateSeed = serde_json::from_str(&seed.to_json().unwrap()).unwrap();
    assert_eq!(back, seed);

    let layout = build_inventory(template(DEFAULT_LAYOUT_TEMPLATE)).unwrap();
    let lseed = seed_defaults(&layout, PlanDefaults::default());
    println!(
        "layout seed: sheet {:?}, {} pages, {} macros",
        lseed.layout.sheet_size,
        lseed.layout.pages.len(),
        lseed.layout.macros.len()
    );
    assert_eq!(
        lseed.layout.sheet_size, None,
        "18x24 is not listed by name in the layout"
    );
    assert!(lseed.layout.pages.contains(&"Page Template".to_string()));
}

// ----- Phase C: typed objects ------------------------------------------------

/// A template in Daniel's Templates folder, or `None` (the test then skips).
fn real(name: &str) -> Option<std::path::PathBuf> {
    let p = templates_dir()?.join(name);
    if p.exists() {
        Some(p)
    } else {
        eprintln!("skipping: {} not found", p.display());
        None
    }
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn phase_c_wall_types_on_daniels_plan() {
    let Some(path) = real(DEFAULT_PLAN_TEMPLATE) else {
        return;
    };
    let s = plan_chiefplan::summarize(path).unwrap();
    println!(
        "{} wall types, {} materials ({} named)",
        s.wall_types.len(),
        s.materials.len(),
        s.materials.iter().filter(|m| !m.name.is_empty()).count()
    );
    assert_eq!(s.wall_types.len(), 103);
    assert_eq!(s.materials.len(), 550);
    let stucco = s.wall_type("Stucco-6").unwrap();
    let layers: Vec<(&str, f64, bool)> = stucco
        .layers
        .iter()
        .map(|(m, t, main)| (m.as_str(), *t, *main))
        .collect();
    assert_eq!(
        layers,
        vec![
            ("Sand Finish - Eggshell", 1.125, false),
            ("Housewrap", 0.01, false),
            ("OSB-Hrz", 0.5, false),
            ("Fir Stud 16\" OC, Teal", 5.5, true),
            ("Drywall", 0.5, false),
        ]
    );
    // Chief shows 7 5/8"; the stored layers add up to 7.635".
    assert!((stucco.total_thickness_in - 7.635).abs() < 1e-9);
    assert!((s.wall_type("Interior-4").unwrap().total_thickness_in - 4.5).abs() < 1e-9);
    assert!((s.wall_type("Siding-6").unwrap().total_thickness_in - 7.01).abs() < 1e-9);
    assert!((s.wall_type("Frame-3 1/2").unwrap().total_thickness_in - 3.5).abs() < 1e-9);
    assert!((s.wall_type("Footing-16").unwrap().total_thickness_in - 16.0).abs() < 1e-9);
    // The stud layer is the main layer of the framed types.
    for n in [
        "Stucco-6",
        "Siding-6",
        "Brick-6",
        "Interior-4",
        "Interior-6",
    ] {
        let w = s.wall_type(n).unwrap_or_else(|| panic!("missing {n}"));
        let mains: Vec<&str> = w
            .layers
            .iter()
            .filter(|l| l.2)
            .map(|l| l.0.as_str())
            .collect();
        assert_eq!(mains.len(), 1, "{n}: {mains:?}");
        assert!(mains[0].starts_with("Fir Stud 16\" OC"), "{n}: {mains:?}");
    }
    // Every layer found its material by id.
    for w in &s.wall_types {
        for l in &w.layers {
            assert!(!l.0.starts_with("material #"), "{}: {}", w.name, l.0);
        }
    }
    assert_eq!(s.default_materials[0].material, "Sand Finish - Eggshell");
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn phase_c_text_styles_and_dimension_defaults() {
    let Some(path) = real(DEFAULT_PLAN_TEMPLATE) else {
        return;
    };
    let s = plan_chiefplan::summarize(path).unwrap();
    assert_eq!(s.text_styles.len(), 15);
    let q = s.text_style("1/4\" Text Style").unwrap();
    assert_eq!(
        (q.font.as_str(), q.font_style.as_str(), q.height_in),
        ("Avenir", "Book", 4.5)
    );
    assert_eq!(s.text_style("1\" Text Style").unwrap().height_in, 1.125);
    assert_eq!(s.text_style("1/8\" Text Style").unwrap().height_in, 9.0);
    let label = s.text_style("Room Label Style").unwrap();
    assert!(label.bold && label.height_in == 8.0);

    assert_eq!(s.dimension_defaults.len(), 14);
    let d = s.dimension_set("1/4\" Scale Dimension Defaults").unwrap();
    assert_eq!(d.text_style.as_deref(), Some("1/4\" Text Style"));
    // The same numbers as Daniel's UI capture of the 1/4" set.
    assert_eq!(d.arrow_size_in, Some(2.25));
    assert_eq!(d.extension_fixed_gap_in, Some(3.0));
    assert_eq!(d.extension_length_away_in, Some(3.0));
    assert_eq!(d.first_line_offset_in, Some(32.0));
    assert_eq!(d.line_separation_in, Some(18.0));
    assert_eq!(d.smallest_fraction, Some(8));
    assert_eq!(d.fraction_text_size_pct, Some(60));
    assert_eq!(d.exterior_reach_in, Some(240.0));
    // The other scale sets scale with the drawing scale.
    let half = s.dimension_set("1/2\" Scale Dimension Defaults").unwrap();
    assert_eq!(
        (half.arrow_size_in, half.line_separation_in),
        (Some(1.125), Some(12.0))
    );
    // Every set points at a style or at none; the resolved ones exist.
    for d in &s.dimension_defaults {
        if let Some(n) = &d.text_style {
            assert!(s.text_style(n).is_some(), "{}", d.name);
        }
    }
    assert_eq!(s.rich_text_defaults.len(), 15);
    let r = &s.rich_text_defaults[0];
    assert_eq!((r.font.as_str(), r.color), ("Avenir", [0, 64, 128]));
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn phase_c_heights_and_paper() {
    let Some(path) = real(DEFAULT_PLAN_TEMPLATE) else {
        return;
    };
    let s = plan_chiefplan::summarize(path).unwrap();
    assert_eq!(s.default_heights.room_type_height_in, Some(109.125));
    assert_eq!(
        s.default_heights.room_types_with_height,
        s.default_heights.room_types_agreeing
    );
    assert!(s.default_heights.room_types_agreeing >= 50);
    let paper = s
        .paper_sizes
        .iter()
        .find(|p| p.name.starts_with("ARCH C"))
        .unwrap();
    assert_eq!((paper.width_in, paper.height_in), (18.0, 24.0));
    assert!((paper.printable_width_in.unwrap() - 23.8333).abs() < 1e-3);
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn phase_c_layout_template() {
    let Some(path) = real(DEFAULT_LAYOUT_TEMPLATE) else {
        return;
    };
    let s = plan_chiefplan::summarize(path).unwrap();
    assert_eq!(s.kind, Some(TemplateKind::Layout));
    assert_eq!(s.wall_types.len(), 24);
    assert_eq!(s.text_styles.len(), 7);
    assert!(s.text_style("Layout Text Style").is_some());
    let l = s.layout.as_ref().unwrap();
    assert!(l.page_template_found);
    assert_eq!(l.page_count, Some(0));
    assert_eq!(l.sheet_from_file_name, Some((18.0, 24.0)));
    assert!(l.embedded_jpeg_bytes.unwrap_or(0) > 500_000);
    assert!(l.title_block_macros.iter().any(|(m, _)| m == "%room.name%"));
    assert!(l.project_info_fields.iter().any(|(m, _)| m == "Designer"));
    let b = s
        .paper_sizes
        .iter()
        .find(|p| p.name.starts_with("ANSI B"))
        .unwrap();
    assert_eq!((b.width_in, b.height_in), (11.0, 17.0));
    // Nothing client-specific leaks into the summary JSON.
    let json = serde_json::to_string(&s).unwrap();
    assert!(
        !json.contains("/Users/") && !json.contains(".plan\""),
        "client path in summary"
    );
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn phase_c_bridge_seeds_decoded_values() {
    let Some(path) = real(DEFAULT_PLAN_TEMPLATE) else {
        return;
    };
    let inv = build_inventory(path).unwrap();
    let seed = plan_chiefplan::seed_defaults(&inv, plan_core::defaults::PlanDefaults::default());
    println!(
        "decoded wall types {}, approximate {}",
        seed.decoded_wall_types.len(),
        seed.approximate_wall_types.len()
    );
    assert!(seed.decoded_wall_types.len() >= 80);
    // Only names the decode did not define stay guesses.
    assert!(
        seed.approximate_wall_types.len() <= 10,
        "{:?}",
        seed.approximate_wall_types
    );
    let brick = seed.defaults.wall_type("Brick-4, No Brick Ledge");
    println!("layout-only type present: {}", brick.is_some());
    let s = seed.text_style_defs.get("1/8\" Text Style").unwrap();
    assert_eq!((s.font.as_str(), s.height_in), ("Avenir", 9.0));
    assert_eq!(seed.default_height_in, Some(109.125));
    let quarter = seed
        .dimension_set_defs
        .iter()
        .find(|d| d.name == "1/4\" Scale")
        .unwrap();
    let base = plan_core::defaults::PlanDefaults::default().dimensions;
    assert_eq!(quarter.auto.arrow_size, base.arrow_size);
    assert_eq!(quarter.auto.auto_exterior_offset, base.auto_exterior_offset);
    assert_eq!(seed.decoded_dimension_sets.len(), 14);
}

#[test]
#[ignore = "needs Daniel's Chief templates"]
fn phase_c_every_template_summarizes() {
    let Some(dir) = templates_dir().filter(|d| d.exists()) else {
        eprintln!("skipping: no templates folder");
        return;
    };
    let files = find_template_files(&dir);
    assert!(files.len() >= 20, "{} files", files.len());
    for f in files {
        let s = plan_chiefplan::summarize(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert!(s.object_count > 100, "{}", f.display());
        assert!(
            s.wall_types.len() >= 20,
            "{}: {}",
            f.display(),
            s.wall_types.len()
        );
        assert!(!s.text_styles.is_empty(), "{}", f.display());
        for w in &s.wall_types {
            assert!(w.total_thickness_in >= 0.0 && w.total_thickness_in < 200.0);
        }
    }
}
