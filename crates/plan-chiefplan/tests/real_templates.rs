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
