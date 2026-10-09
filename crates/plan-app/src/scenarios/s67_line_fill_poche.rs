//! Scenario 67: line styles, fill styles, poché and custom patterns (manual
//! pp. 205-233; CAD-56, CAD-69..CAD-86, LAY-55, RF-100).
//!
//! Make a dash style and a text style in Line Style Management, put them on a
//! CAD line and on a property line (a layer's style), give a wall type's
//! layers a Fill Style, switch poché on in the plan and cut a cross section,
//! make a pattern from drawn CAD lines and fill a slab with it. Every action
//! is one undo step.

use super::Sim;
use crate::dialogs::{fill_style as fs, line_style as ls, pattern_editor as pe};
use crate::editor::ObjectRef;
use crate::toolbar::Action;
use crate::tools::cad::CadMode;
use crate::tools::ToolId;
use eframe::egui::{Color32, Shape};
use plan_core::cad::CadItem;
use plan_core::fill_styles::{FillStyle, FillTarget, PatternType, SystemPattern};
use plan_core::foundation::{FoundationLayer, Slab};
use plan_core::geometry::Point;
use plan_core::line_styles::{ComponentKind, LineComponent, LineStyleDef, LineTarget};
use plan_core::{Id, Layer, WallKind};

fn sim() -> Sim {
    let mut sim = Sim::new();
    sim.app.cx.defaults.grid.snap = 0.0;
    sim.app.cx.defaults.grid.angle_snap_deg = 0.0;
    sim
}

fn draw_line(sim: &mut Sim, a: (f64, f64), b: (f64, f64)) -> Id {
    sim.tool(ToolId::CadVariant(CadMode::Line));
    sim.drag(a, b);
    sim.app
        .cx
        .floor()
        .cad
        .iter()
        .map(|c| c.id)
        .max()
        .expect("a line was drawn")
}

/// Texts the plan draws.
fn texts(shapes: &[Shape]) -> Vec<String> {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect()
}

/// Shapes (fills and strokes) with exactly colour `c`.
fn with_color(shapes: &[Shape], c: Color32) -> usize {
    shapes
        .iter()
        .filter(|s| super::shape_colors(s).contains(&c))
        .count()
}

/// Typed exterior walls in a 20' x 12' box.
fn typed_shell(sim: &mut Sim) -> String {
    let cx = sim.cx();
    let ty = cx
        .defaults
        .wall_types
        .iter()
        .find(|t| t.kind == WallKind::Exterior && t.layers.iter().any(|l| l.is_main))
        .expect("an exterior wall type")
        .name
        .clone();
    let ring = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 144.0),
        Point::new(0.0, 144.0),
    ];
    for i in 0..4 {
        let id = cx
            .project
            .add_wall(0, ring[i], ring[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        cx.project.floors[0]
            .walls
            .iter_mut()
            .find(|w| w.id == id)
            .unwrap()
            .wall_type = Some(ty.clone());
    }
    cx.refresh();
    ty
}

#[test]
fn a_dash_style_and_a_text_style_go_on_a_line_and_a_property_line() {
    let mut sim = sim();
    // CAD > Lines > Line Style Management: a custom dash style and a text style.
    sim.action(Action::Custom(ls::MANAGEMENT));
    assert!(ls::dialog_open());
    ls::with_dialog(|d| {
        d.new_style();
        let s = d.spec_mut().unwrap();
        s.def.name = "Hidden Long".into();
        s.def.components = vec![LineComponent::dash(0.5, 0.1), LineComponent::dot(0.1)];
        d.accept_spec().unwrap();
        d.new_style();
        let s = d.spec_mut().unwrap();
        s.def.name = "Sewer".into();
        s.def.components = vec![
            LineComponent::dash(0.4, 0.08),
            LineComponent::text("SEWER", 0.09, 0.08),
        ];
        d.accept_spec().unwrap();
        assert!(d.lib.get("Hidden Long").is_some() && d.lib.get("Sewer").is_some());
    })
    .unwrap();
    assert!(ls::accept_dialog(&mut sim.app.cx));
    assert_eq!(sim.app.cx.undo_label(), Some("Line Style Management"));
    let lib = &sim.app.cx.project.styles.line_styles;
    assert_eq!(
        lib.get("Sewer").unwrap().components[1].kind,
        ComponentKind::Text
    );

    // Draw a CAD line and give it the text style.
    let line = draw_line(&mut sim, (0.0, 0.0), (240.0, 0.0));
    sim.tool(ToolId::Select);
    sim.app.cx.selection.items = vec![ObjectRef::Cad(line)];
    let sewer = sim.app.cx.project.styles.line_style("Sewer").unwrap();
    assert_eq!(ls::apply_to_selection(&mut sim.app.cx, Some(&sewer)), 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Line Style"));
    let shapes = sim.plan_shapes();
    let words = texts(&shapes);
    assert!(
        words.iter().filter(|w| w.as_str() == "SEWER").count() >= 2,
        "the word repeats along the line: {words:?}"
    );

    // A property line (a closed polyline on its own layer) takes the style
    // through the layer.
    let cx = sim.cx();
    cx.project
        .layers
        .add(Layer::new("Property Line", [180, 0, 0], 35));
    let lot = cx.project.add_cad(
        0,
        "Property Line",
        CadItem::Polyline {
            points: vec![
                Point::new(-60.0, -60.0),
                Point::new(300.0, -60.0),
                Point::new(300.0, 200.0),
                Point::new(-60.0, 200.0),
            ],
            closed: true,
        },
    );
    cx.refresh();
    let hidden = cx.project.styles.line_style("Hidden Long").unwrap();
    ls::apply_to_layer(cx, "Property Line", Some(&hidden));
    assert_eq!(cx.undo_label(), Some("Layer Line Style"));
    assert_eq!(
        cx.project
            .assigned_line_style(lot, "Property Line")
            .unwrap()
            .name,
        "Hidden Long"
    );
    // The line keeps its own style over the layer's.
    assert_eq!(
        cx.project
            .assigned_line_style(line, "CAD, Default")
            .unwrap()
            .name,
        "Sewer"
    );
    // The shared stroker follows the polygon round its corners, closed.
    let (pts, closed) = ls::item_path(
        &cx.project.floors[0]
            .cad
            .iter()
            .find(|c| c.id == lot)
            .unwrap()
            .item,
    )
    .unwrap();
    assert!(closed);
    let pieces = plan_core::line_styles::stroke_path(&hidden, &pts, closed, 48.0);
    let dots = pieces
        .iter()
        .filter(|p| matches!(p, plan_core::line_styles::Piece::Dot(_)))
        .count();
    assert!(dots > 20, "{dots}");
    sim.plan_shapes();

    // One undo step each.
    sim.undo();
    assert!(sim
        .app
        .cx
        .project
        .assigned_line_style(lot, "Property Line")
        .is_none());
    assert!(sim
        .app
        .cx
        .project
        .assigned_line_style(line, "CAD, Default")
        .is_some());
    sim.undo();
    assert!(sim
        .app
        .cx
        .project
        .assigned_line_style(line, "CAD, Default")
        .is_none());
}

#[test]
fn drawing_with_a_library_style_picked_in_the_browser_styles_each_new_line() {
    let mut sim = sim();
    let ex = sim.app.cx.project.styles.line_style("Existing").unwrap();
    assert_eq!(ex.components[1].text, "EX.");
    ls::set_drawing_style(&sim.app.cx, Some("Existing"));
    let a = draw_line(&mut sim, (0.0, 0.0), (120.0, 0.0));
    sim.dialog_frame(false);
    assert_eq!(
        sim.app.cx.project.styles.assigned_line(&LineTarget::Cad(a)),
        Some("Existing")
    );
    // The style was saved in the file when the first line took it.
    assert!(sim
        .app
        .cx
        .project
        .styles
        .line_styles
        .get("Existing")
        .is_some());
    let shapes = sim.plan_shapes();
    assert!(texts(&shapes).iter().any(|w| w == "EX."));
    // The drawing and its style undo together.
    sim.undo();
    assert!(sim
        .app
        .cx
        .project
        .styles
        .assigned_line(&LineTarget::Cad(a))
        .is_none());
    ls::set_drawing_style(&sim.app.cx, None);
    let b = draw_line(&mut sim, (0.0, 10.0), (120.0, 10.0));
    sim.dialog_frame(false);
    assert!(sim
        .app
        .cx
        .project
        .styles
        .assigned_line(&LineTarget::Cad(b))
        .is_none());
}

#[test]
fn a_wall_type_fill_shows_in_the_plan_and_poche_goes_on_the_cut_walls_in_plan_and_section() {
    let mut sim = sim();
    let ty = typed_shell(&mut sim);
    let poche = Color32::from_rgb(55, 55, 55);
    // Without poché or a fill the plan has no dark wall fill.
    assert_eq!(with_color(&sim.plan_shapes(), poche), 0);

    // A hatch on the main layer of the wall type (Layer Fill Style).
    let main = sim
        .app
        .cx
        .defaults
        .wall_types
        .iter()
        .find(|t| t.name == ty)
        .unwrap()
        .layers
        .iter()
        .position(|l| l.is_main)
        .unwrap();
    let ink = [10, 200, 30];
    let t = FillTarget::WallLayer {
        wall_type: ty.clone(),
        index: main,
    };
    sim.app.cx.begin_change("Layer Fill Style");
    sim.app
        .cx
        .project
        .styles
        .apply_fill(t.clone(), Some(FillStyle::hatch(45.0, 6.0, ink)));
    sim.app.cx.refresh();
    let hatch = with_color(
        &sim.plan_shapes(),
        Color32::from_rgb(ink[0], ink[1], ink[2]),
    );
    assert!(hatch > 10, "the layer hatch draws in the plan: {hatch}");

    // View > Poché: the layers without a fill style go dark; the hatched one
    // keeps its hatch. One undo step.
    sim.action(Action::Custom(fs::POCHE));
    assert_eq!(sim.app.cx.undo_label(), Some("Poch\u{e9}"));
    assert!(crate::editor::render::poche_on(&sim.app.cx));
    let on = with_color(&sim.plan_shapes(), poche);
    let layers = sim
        .app
        .cx
        .defaults
        .wall_types
        .iter()
        .find(|t| t.name == ty)
        .unwrap()
        .layers
        .len();
    assert_eq!(
        on,
        4 * (layers - 1),
        "{layers} layers, one with a fill style, 4 walls"
    );
    assert!(
        with_color(
            &sim.plan_shapes(),
            Color32::from_rgb(ink[0], ink[1], ink[2])
        ) >= hatch
    );

    // The switch is per view: another saved plan view starts off.
    let views = sim.app.cx.project.plan_views.clone();
    assert!(views.len() >= 1);
    sim.undo();
    assert!(!crate::editor::render::poche_on(&sim.app.cx));
    assert_eq!(with_color(&sim.plan_shapes(), poche), 0);
    sim.redo();
    assert!(crate::editor::render::poche_on(&sim.app.cx));

    // Glass walls and railings never get poché.
    let cx = sim.cx();
    let glass = cx.project.add_wall(
        0,
        Point::new(300.0, 0.0),
        Point::new(400.0, 0.0),
        4.0,
        96.0,
        WallKind::Exterior,
    );
    cx.project.floors[0]
        .walls
        .iter_mut()
        .find(|w| w.id == glass)
        .unwrap()
        .class = plan_core::WallClass::Glass;
    cx.refresh();
    assert_eq!(
        with_color(&sim.plan_shapes(), poche),
        on,
        "the glass wall stays light"
    );

    // A cross section through the box: the cut walls are the poché regions;
    // the wall type's fill hatches them and the view's switch drops them.
    let p = sim.app.cx.project.clone();
    let scene = crate::editor::framing_view::elevation_scene(&p);
    let opts = plan_elevation::Options {
        raster_px: 512,
        ..plan_elevation::Options::default()
    };
    let cut = plan_elevation::SectionCut {
        plane_normal: plan_elevation::ViewDir::Front,
        offset: -72.0,
    };
    let mut d = plan_elevation::section(&scene, cut, &opts);
    assert!(d.cut_regions().count() >= 2, "{}", d.cut_regions().count());
    let style = p.styles.fill_for(&t).unwrap().clone();
    let lines = plan_elevation::poche_hatch_lines(&d.regions, &style, &[]);
    assert!(lines.len() > 10, "the cut walls take the wall layer hatch");
    plan_elevation::without_poche(&mut d);
    assert_eq!(d.cut_regions().count(), 0);
}

#[test]
fn a_pattern_made_from_cad_lines_fills_a_slab_and_each_step_undoes() {
    let mut sim = sim();
    // Two CAD lines make a plus-shaped tile.
    let cx = sim.cx();
    let a = cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(100.0, 103.0),
            b: Point::new(106.0, 103.0),
        },
    );
    let b = cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(103.0, 100.0),
            b: Point::new(103.0, 106.0),
        },
    );
    sim.tool(ToolId::Select);
    sim.app.cx.selection.items = vec![ObjectRef::Cad(a), ObjectRef::Cad(b)];
    sim.action(Action::Custom(pe::CREATE));
    assert!(pe::dialog_open());
    pe::with_dialog(|d| {
        d.pattern.name = "Plus".into();
        assert_eq!(
            (d.pattern.groups[0].width, d.pattern.groups[0].height),
            (6.0, 6.0)
        );
    })
    .unwrap();
    assert!(pe::accept_dialog(&mut sim.app.cx));
    assert_eq!(sim.app.cx.undo_label(), Some("Create Pattern"));
    assert_eq!(sim.app.cx.project.styles.patterns.len(), 1);

    // A slab on the foundation layer of the floor.
    let slab_id = sim.app.cx.project.alloc_id();
    let mut layer = FoundationLayer::default();
    layer.slabs.push(Slab::new(
        slab_id,
        vec![
            Point::new(0.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(60.0, 36.0),
            Point::new(0.0, 36.0),
        ],
    ));
    sim.app.cx.project.floors[0].set_foundation(&layer).unwrap();
    sim.app.cx.refresh();
    sim.app.cx.selection.items = vec![ObjectRef::Foundation(slab_id)];
    // The Fill Style dialog: Library type "Plus", in its own colour.
    sim.action(Action::Custom(fs::APPLY));
    assert!(fs::dialog_open());
    let ink = [200, 20, 160];
    fs::with_dialog(|d| {
        d.style = FillStyle::library("Plus");
        d.style.color = plan_core::fill_styles::ColorSource::Single(ink);
        assert_eq!(d.targets, vec![FillTarget::Slab(slab_id)]);
    })
    .unwrap();
    assert!(fs::accept_dialog(&mut sim.app.cx));
    assert_eq!(sim.app.cx.undo_label(), Some("Fill Style"));
    let lines = with_color(
        &sim.plan_shapes(),
        Color32::from_rgb(ink[0], ink[1], ink[2]),
    );
    assert_eq!(
        lines,
        10 * 6 * 2,
        "a plus in every 6\" box of the 60 x 36 slab"
    );

    // The Eyedropper reads it back; Undo takes the fill, then the pattern.
    let got = fs::read_fill(&sim.app.cx, ObjectRef::Foundation(slab_id)).unwrap();
    assert_eq!(got.pattern, PatternType::Library("Plus".into()));
    sim.undo();
    assert!(sim
        .app
        .cx
        .project
        .styles
        .fill_for(&FillTarget::Slab(slab_id))
        .is_none());
    assert_eq!(
        with_color(
            &sim.plan_shapes(),
            Color32::from_rgb(ink[0], ink[1], ink[2])
        ),
        0
    );
    sim.undo();
    assert!(sim.app.cx.project.styles.patterns.is_empty());
}

#[test]
fn old_fills_stay_readable_and_a_named_fill_style_reaches_the_library() {
    let mut sim = sim();
    // A CAD fill from before the shared type (name, colour, spacing).
    let sq = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Polyline {
            points: vec![
                Point::ZERO,
                Point::new(48.0, 0.0),
                Point::new(48.0, 48.0),
                Point::new(0.0, 48.0),
            ],
            closed: true,
        },
    );
    sim.app.cx.project.edit_cad_attrs(0, sq, |a| {
        a.fill = Some(plan_core::cad::FillAttr {
            pattern: "Brick".into(),
            ..plan_core::cad::FillAttr::default()
        })
    });
    let old = fs::read_fill(&sim.app.cx, ObjectRef::Cad(sq)).unwrap();
    assert_eq!(old.pattern, PatternType::System(SystemPattern::Brick));
    // Fill Style Painter: paints the style on another shape in one step.
    let other = sim.app.cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Circle {
            center: Point::new(200.0, 0.0),
            radius: 20.0,
        },
    );
    sim.app.cx.begin_change("Fill Style Painter");
    assert!(fs::apply_fill(&mut sim.app.cx, ObjectRef::Cad(other), &old));
    assert_eq!(
        sim.app.cx.project.styles.fill_for(&FillTarget::Cad(other)),
        Some(&old)
    );
    sim.undo();
    assert!(sim
        .app
        .cx
        .project
        .styles
        .fill_for(&FillTarget::Cad(other))
        .is_none());
    // A named fill style goes to the file and the User Catalog.
    sim.action(Action::Custom(fs::NEW_NAMED));
    fs::with_dialog(|d| {
        d.name = "Gravel".into();
        d.style = FillStyle::system(SystemPattern::Concrete, 4.0, 4.0);
    })
    .unwrap();
    assert!(fs::accept_dialog(&mut sim.app.cx));
    assert_eq!(sim.app.cx.project.styles.user_fills[0].name, "Gravel");
    assert_eq!(sim.app.cx.undo_label(), Some("Fill Style Specification"));
    // The book round-trips through the plan file.
    let json = serde_json::to_string(&sim.app.cx.project).unwrap();
    let back: plan_core::Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.styles, sim.app.cx.project.styles);
    let mut plain = plan_core::Project::new("x");
    plain.styles = Default::default();
    assert!(
        !serde_json::to_string(&plain)
            .unwrap()
            .contains("line_assign"),
        "old files stay unchanged"
    );
    let _: LineStyleDef = LineStyleDef::default();
}
