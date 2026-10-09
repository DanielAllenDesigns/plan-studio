//! Performance benchmark and cache-invalidation tests on the large sample
//! (`samples/large-house.psplan`, see `docs/performance.md`).
//!
//! ```text
//! cargo test -p plan-app --release perf_bench -- --ignored --nocapture
//! ```
//!
//! The benchmark is `#[ignore]`d (timings are not assertions); the other
//! tests in this file run in the normal gate.

use crate::editor::{render, schedule_view, Camera, EditorContext};
use crate::scenarios::Sim;
use eframe::egui::{self, Shape};
use plan_core::geometry::Point;
use plan_core::{OpeningKind, Project};
use std::time::Instant;

/// The large sample, loaded the way File > Open does.
fn large_project() -> Project {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/large-house.psplan");
    let text = std::fs::read_to_string(&path).expect("samples/large-house.psplan");
    Project::from_json(&text).expect("large sample loads")
}

fn large_sim() -> Sim {
    let mut sim = Sim::new();
    let p = large_project();
    sim.app.cx.set_project(p);
    sim.app.cx.refresh();
    sim
}

/// What one headless draw produced, for before/after comparisons.
#[derive(Debug, Default, Clone)]
pub struct DrawStats {
    pub shapes: usize,
    pub segments: usize,
    pub paths: usize,
    pub texts: usize,
    pub meshes: usize,
    pub other: usize,
    /// `ctx.tessellate` of the frame (what eframe does after `update`).
    pub tessellate_ms: f64,
    pub triangles: usize,
}

fn count(shape: &Shape, st: &mut DrawStats) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| count(s, st)),
        Shape::LineSegment { .. } => st.segments += 1,
        Shape::Path(_) => st.paths += 1,
        Shape::Text(_) => st.texts += 1,
        Shape::Mesh(_) => st.meshes += 1,
        Shape::Noop => {}
        _ => st.other += 1,
    }
    if !matches!(shape, Shape::Vec(_) | Shape::Noop) {
        st.shapes += 1;
    }
}

/// Draws the plan into a headless painter; returns the stats of the painted
/// shapes and the milliseconds `draw_plan` alone took.
pub fn headless_draw(cx: &EditorContext, ctx: &egui::Context, px_per_in: f64) -> (DrawStats, f64) {
    let mut cam = Camera::default_view();
    cam.px_per_in = px_per_in;
    cam.center = Point::new(840.0, 420.0);
    cam.rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
    let mut ms = 0.0;
    let out = ctx.run(
        egui::RawInput {
            screen_rect: Some(cam.rect),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let (_, painter) = ui.allocate_painter(cam.rect.size(), egui::Sense::hover());
                    let t = Instant::now();
                    render::draw_plan(cx, &painter, &cam);
                    ms = t.elapsed().as_secs_f64() * 1000.0;
                });
        },
    );
    let mut st = DrawStats::default();
    for clipped in &out.shapes {
        count(&clipped.shape, &mut st);
    }
    let t = Instant::now();
    let prims = ctx.tessellate(out.shapes, 1.0);
    st.tessellate_ms = t.elapsed().as_secs_f64() * 1000.0;
    st.triangles = prims
        .iter()
        .map(|p| match &p.primitive {
            egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
            _ => 0,
        })
        .sum();
    (st, ms)
}

/// One whole UI frame the way `PlanApp::update` runs it (menus, toolbars,
/// panels, canvas, dialogs), headless. Returns milliseconds.
pub fn full_frame(app: &mut crate::PlanApp, ctx: &egui::Context, size: egui::Vec2) -> f64 {
    let t = Instant::now();
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        },
        |ctx| {
            app.cx.refresh();
            if crate::editor::roof_view::auto_rebuild(&mut app.cx) {
                app.cx.refresh();
            }
            let mut actions = Vec::new();
            app.handle_keys(ctx, &mut actions);
            app.menu_bar(ctx, &mut actions);
            app.toolbar_rows(ctx, &mut actions);
            app.status_bar(ctx);
            app.properties_panel(ctx);
            app.view_bar(ctx, &mut actions);
            app.dock_panel(ctx);
            app.tools.frame(&mut app.cx, ctx);
            app.process_requests();
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| app.canvas(ctx, ui));
            app.edit_toolbar(ctx);
            app.dialogs(ctx);
        },
    );
    t.elapsed().as_secs_f64() * 1000.0
}

/// The plan and layout templates the decode cache names.
fn cached_template_paths() -> Option<(std::path::PathBuf, Option<std::path::PathBuf>)> {
    let text = std::fs::read_to_string(crate::templates::cache_path()?).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let get = |k: &str| {
        v.get(k)?
            .get("path")
            .and_then(|p| p.as_str())
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_file())
    };
    Some((get("plan")?, get("layout")))
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Median milliseconds of `n` calls after one warm-up call.
fn time_ms(n: usize, mut f: impl FnMut()) -> f64 {
    f();
    median(
        (0..n)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1000.0
            })
            .collect(),
    )
}

#[test]
#[ignore = "benchmark: run with --ignored --nocapture"]
fn perf_bench() {
    let mut rows: Vec<(String, f64)> = Vec::new();
    let mut row = |name: &str, ms: f64| {
        println!("| {name:<58} | {ms:>9.2} |");
        rows.push((name.to_string(), ms));
    };
    println!("| operation (large-house.psplan, median ms) | ms |");
    println!("|{:-<60}|{:-<11}|", "", "");

    // ---- startup
    row(
        "startup: plan_defaults::embedded()",
        time_ms(5, || {
            std::hint::black_box(crate::plan_defaults::embedded());
        }),
    );
    // The Chief plan template named by the decode cache (read-only use: the
    // cache is fresh, so nothing is written). The first-ever run instead
    // scans the home folder for the template; that is not measured here
    // because it would write `~/.plan-studio`.
    if let Some((plan, layout)) = cached_template_paths() {
        let settings = crate::templates::TemplateSettings {
            plan: Some(plan),
            layout,
            seed_from_chief: true,
        };
        row(
            "startup: seeded defaults (cached Chief template)",
            time_ms(3, || {
                std::hint::black_box(crate::templates::seeded_defaults(
                    &settings,
                    crate::plan_defaults::embedded(),
                ));
            }),
        );
    }
    row(
        "startup: PlanApp::new (embedded defaults)",
        time_ms(5, || {
            std::hint::black_box(Sim::new());
        }),
    );
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/large-house.psplan"),
    )
    .unwrap();
    row(
        "open: Project::from_json (1.4 MB)",
        time_ms(3, || {
            std::hint::black_box(Project::from_json(&text).unwrap());
        }),
    );

    let mut sim = large_sim();
    let (walls, openings): (usize, usize) = sim
        .app
        .cx
        .project
        .floors
        .iter()
        .map(|f| (f.walls.len(), f.openings.len()))
        .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    {
        let fl = &sim.app.cx.project.floors;
        let rooms: Vec<usize> = fl
            .iter()
            .map(|f| plan_core::detect_rooms(&f.walls, 0.5).len())
            .collect();
        let sum = |g: &dyn Fn(&plan_core::Floor) -> usize| fl.iter().map(g).sum::<usize>();
        println!(
            "(walls {walls}, openings {openings}, floors {}, rooms per floor {rooms:?}, cad attrs {}, regions {}, hatches {})",
            fl.len(),
            sum(&|f| f.cad_attrs.len()),
            sum(&|f| plan_core::details::DetailsLayer::load(f).regions.len()),
            sum(&|f| plan_core::details::DetailsLayer::load(f).hatches.len()),
        );
    }

    // ---- derived data
    {
        let cx = &mut sim.app.cx;
        row(
            "cx.refresh() clean (nothing changed)",
            time_ms(20, || cx.refresh()),
        );
        row(
            "cx.refresh() after a non-wall edit (memoized rooms)",
            time_ms(5, || {
                cx.mark_dirty();
                cx.refresh();
            }),
        );
        let mut flip = false;
        row(
            "cx.refresh() after a wall edit (rooms + outlines + framing)",
            time_ms(5, || {
                flip = !flip;
                cx.floor_mut().walls[0].thickness = if flip { 4.5 } else { 4.75 };
                cx.mark_dirty();
                cx.refresh();
            }),
        );
        let walls = cx.floor().walls.clone();
        row(
            "  detect_rooms (1 floor)",
            time_ms(5, || {
                std::hint::black_box(plan_core::detect_rooms(&walls, 0.5));
            }),
        );
        row(
            "  wall_outlines (1 floor)",
            time_ms(5, || {
                std::hint::black_box(plan_core::wall_outlines(
                    &walls,
                    crate::editor::ops::JOIN_TOL,
                ));
            }),
        );
        let types = cx.wall_types().to_vec();
        row(
            "  wall_layer_outlines (1 floor)",
            time_ms(5, || {
                std::hint::black_box(plan_core::wall_layer_outlines(
                    &walls,
                    &types,
                    crate::editor::ops::JOIN_TOL,
                ));
            }),
        );
        row(
            "  roof_view::auto_rebuild check (per frame, 3 floors)",
            time_ms(10, || {
                std::hint::black_box(crate::editor::roof_view::auto_rebuild(cx));
            }),
        );
    }

    // ---- scaling: one floor of N walls (a grid of 10' cells)
    for (cols, rows) in [(10usize, 10usize), (14, 14), (20, 20), (28, 28)] {
        let mut p = Project::new("grid");
        let t = plan_core::WallKind::Interior;
        for r in 0..=rows {
            for c in 0..cols {
                let (x, y) = (c as f64 * 120.0, r as f64 * 120.0);
                p.add_wall(0, Point::new(x, y), Point::new(x + 120.0, y), 4.5, 96.0, t);
            }
        }
        for c in 0..=cols {
            for r in 0..rows {
                let (x, y) = (c as f64 * 120.0, r as f64 * 120.0);
                p.add_wall(0, Point::new(x, y), Point::new(x, y + 120.0), 4.5, 96.0, t);
            }
        }
        let walls = p.floors[0].walls.clone();
        let n = walls.len();
        row(
            &format!("scaling {n} walls: detect_rooms"),
            time_ms(3, || {
                std::hint::black_box(plan_core::detect_rooms(&walls, 0.5));
            }),
        );
        row(
            &format!("scaling {n} walls: wall_outlines"),
            time_ms(3, || {
                std::hint::black_box(plan_core::wall_outlines(
                    &walls,
                    crate::editor::ops::JOIN_TOL,
                ));
            }),
        );
    }

    // ---- draw (floor 0: schedules, cabinets, CAD, regions, terrain)
    let ctx = egui::Context::default();
    for (label, px) in [
        ("fit (0.45 px/in)", 0.45),
        ("working (1.5 px/in)", 1.5),
        ("close-up (5 px/in)", 5.0),
    ] {
        for floor in [0usize, 2] {
            sim.app.cx.floor = floor;
            sim.app.cx.mark_dirty();
            sim.app.cx.refresh();
            let cx = &sim.app.cx;
            let ms = time_ms(8, || {
                std::hint::black_box(headless_draw(cx, &ctx, px));
            });
            // `time_ms` includes ctx.run + painter; report draw_plan alone too.
            let only = median((0..8).map(|_| headless_draw(cx, &ctx, px).1).collect());
            let (st, _) = headless_draw(cx, &ctx, px);
            row(
                &format!("draw_plan floor {floor} {label}: draw_plan only"),
                only,
            );
            row(
                &format!("draw_plan floor {floor} {label}: full ctx.run"),
                ms,
            );
            println!("    shapes {st:?}");
            // Where one frame goes (a single extra frame, stage by stage).
            render::SECTION_MS.with(|m| m.borrow_mut().clear());
            headless_draw(cx, &ctx, px);
            let mut parts: Vec<(&str, f64)> =
                render::SECTION_MS.with(|m| m.borrow().iter().map(|(k, v)| (*k, *v)).collect());
            parts.sort_by(|a, b| b.1.total_cmp(&a.1));
            let top: Vec<String> = parts
                .iter()
                .take(6)
                .map(|(k, v)| format!("{k} {v:.2}"))
                .collect();
            println!("    stages ms: {}", top.join(", "));
        }
    }
    sim.app.cx.floor = 0;
    sim.app.cx.refresh();

    // ---- whole UI frames (menus, toolbars, panels, canvas)
    {
        let ctx = egui::Context::default();
        let size = egui::vec2(1400.0, 900.0);
        let first = full_frame(&mut sim.app, &ctx, size);
        let ms = median(
            (0..10)
                .map(|_| full_frame(&mut sim.app, &ctx, size))
                .collect(),
        );
        println!("(first full frame {first:.1} ms: fonts, icons, layout)");
        row("full UI frame, idle, floor 0 (median of 10)", ms);
    }

    // ---- built framing on floor 0 (studs, plates, floor joists)
    {
        let cx = &mut sim.app.cx;
        let t = Instant::now();
        let summary = crate::editor::framing_view::build(cx, false);
        println!(
            "(Build Framing floor 0: {:.0} ms, {} members, {:?})",
            t.elapsed().as_secs_f64() * 1000.0,
            cx.floor().framing.len(),
            summary
        );
        cx.refresh();
        let mut flip = false;
        row(
            "framed floor: cx.refresh() after a wall edit",
            time_ms(5, || {
                flip = !flip;
                cx.floor_mut().walls[0].thickness = if flip { 4.5 } else { 4.75 };
                cx.mark_dirty();
                cx.refresh();
            }),
        );
        row(
            "framed floor: cx.refresh() after a non-wall edit",
            time_ms(5, || {
                cx.mark_dirty();
                cx.refresh();
            }),
        );
        let ctx = egui::Context::default();
        let cx = &sim.app.cx;
        row(
            "framed floor: draw_plan fit (0.45 px/in)",
            median((0..8).map(|_| headless_draw(cx, &ctx, 0.45).1).collect()),
        );
        row(
            "framed floor: draw_plan close-up (5 px/in)",
            median((0..8).map(|_| headless_draw(cx, &ctx, 5.0).1).collect()),
        );
        render::SECTION_MS.with(|m| m.borrow_mut().clear());
        headless_draw(cx, &ctx, 0.45);
        let framing_ms = render::SECTION_MS.with(|m| m.borrow().get("framing").copied());
        println!("    framing stage ms (one frame): {framing_ms:?}");
        // Back to the unframed plan for the rest of the run.
        sim.app.cx.floor_mut().framing.clear();
        sim.app.cx.mark_dirty();
        sim.app.cx.refresh();
    }

    // ---- pointer work with Select Objects (per mouse-move event)
    {
        sim.tool(crate::tools::ToolId::Select);
        let pts: Vec<(f64, f64)> = (0..60)
            .map(|i| {
                (
                    60.0 + (i % 12) as f64 * 130.0,
                    60.0 + (i / 12) as f64 * 150.0,
                )
            })
            .collect();
        let mut k = 0usize;
        row(
            "hover: one pointer move (Select Objects, 591 walls)",
            time_ms(60, || {
                let (x, y) = pts[k % pts.len()];
                k += 1;
                sim.move_to(x, y);
            }),
        );
    }

    // ---- layout window (construction set pages), headless
    {
        let layout = plan_layout::default_construction_set(&sim.app.cx.project, 3);
        crate::shell::layout_window::store(&mut sim.app.cx.project, &layout);
        sim.app.cx.mark_dirty();
        sim.app.cx.refresh();
        let ctx = egui::Context::default();
        let frame = |cx: &mut EditorContext| {
            let t = Instant::now();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ctx, |ui| {
                            crate::shell::layout_window::show_central(ctx, ui, cx)
                        });
                },
            );
            t.elapsed().as_secs_f64() * 1000.0
        };
        frame(&mut sim.app.cx); // loads the layout into the view
        crate::shell::layout_window::show_page(1);
        let first = frame(&mut sim.app.cx);
        println!(
            "(layout window first frame, builds the box drawings: {first:.1} ms; page {} of {})",
            crate::shell::layout_window::current_page_index(),
            crate::shell::layout_window::page_list(&sim.app.cx.project).len()
        );
        let cx = &mut sim.app.cx;
        row(
            "layout window: next frame (box drawings cached)",
            median((0..8).map(|_| frame(cx)).collect()),
        );
        // What the frame used to add: serializing the plan to hash it.
        let p = &cx.project;
        row(
            "  (old per-frame project_sig: JSON of floors, layers, terrain)",
            time_ms(3, || {
                let mut n = 0;
                n += serde_json::to_string(&p.floors).map_or(0, |s| s.len());
                n += serde_json::to_string(&p.layers).map_or(0, |s| s.len());
                n += serde_json::to_string(&p.layer_sets).map_or(0, |s| s.len());
                n += serde_json::to_string(&p.cameras).map_or(0, |s| s.len());
                n += serde_json::to_string(&p.text_styles).map_or(0, |s| s.len());
                n += serde_json::to_string(&p.terrain).map_or(0, |s| s.len());
                std::hint::black_box(n);
            }),
        );
    }

    // ---- schedules alone
    {
        let cx = &sim.app.cx;
        let mut cam = Camera::default_view();
        cam.px_per_in = 1.5;
        let ctx2 = egui::Context::default();
        let ms = time_ms(8, || {
            let _ = ctx2.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, painter) =
                        ui.allocate_painter(egui::vec2(800.0, 600.0), egui::Sense::hover());
                    schedule_view::draw_schedules(cx, &painter, &cam);
                });
            });
        });
        row("draw_schedules floor 0 (6 tables)", ms);
    }

    // ---- 3D and documents
    {
        let p = &sim.app.cx.project;
        row(
            "project_hash",
            time_ms(5, || {
                std::hint::black_box(crate::shell::view3d_panel::project_hash(p));
            }),
        );
        let scope = crate::shell::view3d_panel::ViewScope::default();
        row(
            "build_view_scene (all floors)",
            time_ms(3, || {
                std::hint::black_box(crate::shell::view3d_panel::build_view_scene(p, &scope));
            }),
        );
        row(
            "construction set: build the layout (default_construction_set)",
            time_ms(2, || {
                std::hint::black_box(plan_layout::default_construction_set(p, 3));
            }),
        );
        let layout = plan_layout::default_construction_set(p, 3);
        {
            // Where the PDF time goes: the drawing of each page's boxes.
            let rcx = plan_layout::LayoutRenderContext::new(p);
            for page in &layout.pages {
                let t = Instant::now();
                let lines: usize = page
                    .boxes
                    .iter()
                    .map(|b| plan_layout::render_box_lines(b, &rcx).len())
                    .sum();
                println!(
                    "    page {:<28} {:>3} boxes {:>7} lines {:>8.1} ms",
                    page.title,
                    page.boxes.len(),
                    lines,
                    t.elapsed().as_secs_f64() * 1000.0
                );
            }
        }
        let mut bytes = 0;
        row(
            "construction set: render_pdf",
            time_ms(2, || {
                let rcx = plan_layout::LayoutRenderContext::new(p);
                bytes = plan_layout::render_pdf(&layout, &rcx).len();
            }),
        );
        println!("(construction set: {bytes} bytes)");
    }
    println!("{} rows", rows.len());
}

// ---------------------------------------------------------------------------
// Cache tests (normal gate)
// ---------------------------------------------------------------------------

#[test]
fn the_large_sample_has_the_advertised_object_counts() {
    let p = large_project();
    let sum = |f: &dyn Fn(&plan_core::Floor) -> usize| p.floors.iter().map(f).sum::<usize>();
    assert_eq!(p.floors.len(), 3);
    assert!((300..=800).contains(&sum(&|f| f.walls.len())));
    assert!(sum(&|f| f.openings.len()) >= 140);
    assert_eq!(sum(&|f| f.cabinets.len()), 60);
    assert_eq!(sum(&|f| f.dimensions.len()), 40);
    assert_eq!(sum(&|f| f.cad.len()), 200);
    assert_eq!(
        sum(&|f| plan_core::schedules::ScheduleLayer::load(f).schedules.len()),
        10
    );
    // The typed slots parse with their owning crates.
    for f in &p.floors {
        f.cabinets_as::<plan_cabinets::Cabinet>()
            .expect("cabinets parse");
    }
    assert_eq!(
        crate::editor::roof_view::load(&p.floors[2]).planes.len(),
        20
    );
    let t = crate::editor::site_view::load_terrain(&p).expect("terrain parses");
    assert_eq!(t.terrain.elevation_lines.len(), 50);
    assert!(t.built);
}

#[test]
fn drawing_the_large_sample_paints_something_on_every_floor() {
    let mut sim = large_sim();
    let ctx = egui::Context::default();
    for floor in 0..3 {
        sim.app.cx.floor = floor;
        sim.app.cx.mark_dirty();
        sim.app.cx.refresh();
        let (st, _) = headless_draw(&sim.app.cx, &ctx, 0.45);
        assert!(st.shapes > 500, "floor {floor}: {st:?}");
    }
}

/// An interior wall of the large sample that has no opening yet.
fn bare_interior_wall(cx: &EditorContext) -> plan_core::Id {
    let f = cx.floor();
    f.walls
        .iter()
        .find(|w| w.kind == plan_core::WallKind::Interior && f.openings_on(w.id).next().is_none())
        .expect("a wall without openings")
        .id
}

/// Text shapes `draw_schedules` paints with the camera over the first table.
fn schedule_texts(cx: &EditorContext, ctx: &egui::Context) -> usize {
    let first = schedule_view::load(cx).schedules[0].clone();
    let mut cam = Camera::default_view();
    cam.px_per_in = 1.5;
    cam.center = Point::new(first.position.x + 150.0, first.position.y - 200.0);
    cam.rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
    let out = ctx.run(
        egui::RawInput {
            screen_rect: Some(cam.rect),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let (_, painter) = ui.allocate_painter(cam.rect.size(), egui::Sense::hover());
                    schedule_view::draw_schedules(cx, &painter, &cam);
                });
        },
    );
    let mut st = DrawStats::default();
    for c in &out.shapes {
        count(&c.shape, &mut st);
    }
    st.texts
}

#[test]
fn a_door_added_changes_the_schedule_that_is_drawn() {
    let mut sim = large_sim();
    let cx = &mut sim.app.cx;
    let door = schedule_view::load(cx)
        .schedules
        .iter()
        .find(|s| s.kind == plan_core::schedules::ScheduleKind::Door)
        .unwrap()
        .clone();
    let before = schedule_view::layout_of(cx, &door, 0);
    let labels_before = schedule_view::labels(cx).len();
    // Draw once so every cache is warm.
    let ctx = egui::Context::default();
    let texts_before = schedule_texts(cx, &ctx);
    assert!(texts_before > 50, "the table is on screen: {texts_before}");
    let (s0, _) = headless_draw(cx, &ctx, 1.5);
    let wall = bare_interior_wall(cx);
    cx.begin_change("Add Door");
    let fl = cx.floor;
    let placed = cx.project.add_opening(fl, wall, 60.0, OpeningKind::Door);
    assert!(placed.is_some(), "door placed");
    // No refresh and no mark_dirty: begin_change is the change signal.
    let after = schedule_view::layout_of(cx, &door, 0);
    assert_eq!(after.table.rows.len(), before.table.rows.len() + 1);
    cx.refresh();
    let (s1, _) = headless_draw(cx, &ctx, 1.5);
    assert!(s1.shapes > s0.shapes, "the door is drawn: {s0:?} -> {s1:?}");
    assert_eq!(schedule_view::labels(cx).len(), labels_before + 1);
    // The table drawn has the new row: one text per column more.
    assert_eq!(
        schedule_texts(cx, &ctx),
        texts_before + door.visible_columns().count()
    );
    // Undo takes the row back.
    cx.undo();
    assert_eq!(
        schedule_view::layout_of(cx, &door, 0).table.rows.len(),
        before.table.rows.len()
    );
    assert_eq!(schedule_view::labels(cx).len(), labels_before);
    assert_eq!(schedule_texts(cx, &ctx), texts_before);
}

#[test]
fn the_cached_layout_equals_a_fresh_build_after_every_kind_of_edit() {
    let mut sim = large_sim();
    let cx = &mut sim.app.cx;
    let fresh = |cx: &EditorContext| -> Vec<schedule_view::Layout> {
        schedule_view::load(cx)
            .schedules
            .iter()
            .map(|s| schedule_view::build_layout(cx, s, cx.floor))
            .collect()
    };
    let cached = |cx: &EditorContext| -> Vec<schedule_view::Layout> {
        schedule_view::load(cx)
            .schedules
            .iter()
            .map(|s| schedule_view::layout_of(cx, s, cx.floor))
            .collect()
    };
    assert_eq!(cached(cx), fresh(cx));
    // A window added, a room renamed, a schedule moved and a schedule edited.
    let wall = bare_interior_wall(cx);
    cx.begin_change("Add Window");
    let fl = cx.floor;
    cx.project.add_opening(fl, wall, 60.0, OpeningKind::Window);
    cx.refresh();
    assert_eq!(cached(cx), fresh(cx));
    let rooms = cx.rooms.clone();
    cx.begin_change("Rename Room");
    let anchor = rooms[0].centroid;
    cx.project
        .set_room_name(fl, anchor, "Library", "Great Room", &rooms);
    cx.refresh();
    assert_eq!(cached(cx), fresh(cx));
    let id = schedule_view::load(cx).schedules[0].id;
    assert!(schedule_view::move_to(cx, id, Point::new(-3000.0, -3000.0)));
    assert_eq!(cached(cx), fresh(cx));
    let mut def = schedule_view::find(cx, id).unwrap();
    def.title = "Renamed".into();
    assert!(schedule_view::replace(cx, 0, def));
    assert_eq!(cached(cx), fresh(cx));
    assert_eq!(
        schedule_view::layout_of(cx, &schedule_view::find(cx, id).unwrap(), 0)
            .table
            .title,
        "Renamed"
    );
    // A group move of the schedule goes through translate_ids.
    cx.begin_change("Move");
    schedule_view::translate_ids(cx, &[id], Point::new(100.0, 0.0));
    assert_eq!(
        schedule_view::find(cx, id).unwrap().position,
        Point::new(-2900.0, -3000.0)
    );
    assert_eq!(schedule_view::extents(cx)[0].1.x, -2900.0);
    assert_eq!(cached(cx), fresh(cx));
}

/// Every line segment of a frame, as `(a, b)` screen points rounded to 0.01.
fn frame_segments(cx: &EditorContext, ctx: &egui::Context, px: f64) -> Vec<[(i64, i64); 2]> {
    let mut cam = Camera::default_view();
    cam.px_per_in = px;
    cam.center = Point::new(120.0, 120.0);
    cam.rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
    let out = ctx.run(
        egui::RawInput {
            screen_rect: Some(cam.rect),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let (_, painter) = ui.allocate_painter(cam.rect.size(), egui::Sense::hover());
                    render::draw_plan(cx, &painter, &cam);
                });
        },
    );
    fn walk(s: &Shape, out: &mut Vec<[(i64, i64); 2]>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::LineSegment { points, .. } => {
                let q =
                    |p: egui::Pos2| ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64);
                out.push([q(points[0]), q(points[1])]);
            }
            _ => {}
        }
    }
    let mut segs = Vec::new();
    for c in &out.shapes {
        walk(&c.shape, &mut segs);
    }
    segs
}

#[test]
fn a_region_hatch_follows_the_region_when_it_moves() {
    use plan_core::details::{DetailsLayer, MaterialRegion};
    let mut cx = EditorContext::new(crate::plan_defaults::embedded());
    let id = cx.project.alloc_id();
    cx.begin_change("Place Region");
    let mut layer = DetailsLayer::default();
    layer.regions.push(MaterialRegion {
        id,
        outline: vec![
            Point::new(60.0, 60.0),
            Point::new(180.0, 60.0),
            Point::new(180.0, 180.0),
            Point::new(60.0, 180.0),
        ],
        material: "Ceramic Tile 12x12".into(),
        ..MaterialRegion::default()
    });
    layer.store(&mut cx.project.floors[0]);
    cx.refresh();
    cx.view_flags
        .remove(&crate::toolbar::ViewFlag::ReferenceGrid);
    let ctx = egui::Context::default();
    let px = 6.0;
    let before = frame_segments(&cx, &ctx, px);
    // Move the region by 24" east and 12" north.
    let d = Point::new(24.0, 12.0);
    cx.begin_change("Move Region");
    assert_eq!(
        crate::editor::details_view::translate_ids(&mut cx, &[id], d),
        1
    );
    cx.refresh();
    let after = frame_segments(&cx, &ctx, px);
    assert_eq!(before.len(), after.len(), "the same drawing, moved");
    let (dx, dy) = (
        (d.x * px * 100.0).round() as i64,
        -(d.y * px * 100.0).round() as i64,
    );
    let moved: Vec<[(i64, i64); 2]> = before
        .iter()
        .map(|[a, b]| [(a.0 + dx, a.1 + dy), (b.0 + dx, b.1 + dy)])
        .collect();
    // The pattern strokes and the outline moved; the origin marker and the
    // sheet do not, so most (not all) segments are the moved ones.
    let hits = after.iter().filter(|s| moved.contains(s)).count();
    assert!(
        hits > 10,
        "{hits} of {} segments moved with the region",
        after.len()
    );
    assert_ne!(before, after);
}

#[test]
fn rooms_are_kept_for_non_wall_edits_and_rebuilt_for_wall_edits() {
    let mut sim = large_sim();
    let cx = &mut sim.app.cx;
    let rooms = cx.rooms.clone();
    let outlines = cx.outlines.len();
    assert!(!rooms.is_empty());
    // A CAD line: no new detection, same rooms.
    cx.begin_change("Add Line");
    cx.project.add_cad(
        0,
        "CAD, Default",
        plan_core::CadItem::Line {
            a: Point::ZERO,
            b: Point::new(10.0, 10.0),
        },
    );
    let (uid, rev) = cx.cache_key();
    cx.refresh();
    assert!(
        cx.cache_key().1 > rev && cx.cache_key().0 == uid,
        "refresh signals a change"
    );
    assert_eq!(cx.rooms.len(), rooms.len());
    assert_eq!(cx.outlines.len(), outlines);
    // A thicker wall: the inner polygons of its rooms change.
    let before: f64 = cx.rooms.iter().map(|r| r.interior_area_sq_in).sum();
    cx.begin_change("Thicken");
    let w = &mut cx.floor_mut().walls[40];
    w.thickness += 4.0;
    cx.refresh();
    let after: f64 = cx.rooms.iter().map(|r| r.interior_area_sq_in).sum();
    assert!(after < before, "{after} < {before}");
    // Switching floors recomputes for the other floor's walls and framing.
    cx.floor = 1;
    cx.mark_dirty();
    cx.refresh();
    assert_eq!(cx.outlines.len(), cx.floor().walls.len());
}

#[test]
fn framing_is_parsed_again_only_when_its_values_change() {
    let mut sim = large_sim();
    let cx = &mut sim.app.cx;
    let summary = crate::editor::framing_view::build(cx, false);
    assert!(summary.walls > 100, "{summary:?}");
    cx.refresh();
    let n = cx.framing.len();
    assert!(n > 100);
    // A non-framing edit keeps the parsed members; the same values again too.
    cx.begin_change("Add Line");
    cx.project.add_cad(
        0,
        "CAD, Default",
        plan_core::CadItem::Circle {
            center: Point::ZERO,
            radius: 5.0,
        },
    );
    cx.refresh();
    assert_eq!(cx.framing.len(), n);
    // Deleting the framing empties the list.
    cx.begin_change("Delete Framing");
    cx.floor_mut().framing.clear();
    cx.refresh();
    assert!(cx.framing.is_empty());
    // Another floor has its own (here: no) framing, even with equal walls.
    cx.floor = 1;
    cx.mark_dirty();
    cx.refresh();
    assert!(cx.framing.is_empty());
}
