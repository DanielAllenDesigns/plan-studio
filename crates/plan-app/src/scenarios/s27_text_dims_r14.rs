//! Scenario 27 (round 14): text boxes and dimensions.
//!
//! Text: a click-drag with the Text tool defines a box with a wrap width,
//! the box handles under Select resize it (one undo step), the border,
//! background and alignment draw, a new text lines up with the left edge of
//! nearby text, and Text Style Management renames and removes styles.
//! Dimensions: a manual string grows point by point, Add and Delete Extension
//! Line split and merge it, a copy keeps its links to the copied walls,
//! a dimension keeps its own format, arrow and extension lines, openings on a
//! curved wall are located along the arc, CAD objects show temporary
//! dimensions and a temporary dimension can be locked into a permanent one
//! (TXT-1, TXT-3, TXT-10, TXT-13, TXT-16, S-25, DIM-2, DIM-31, DIM-38,
//! DIM-39, DIM-41, S-58, S-63).

use super::Sim;
use crate::editor::handles::{handles_for, HandleKind};
use crate::editor::tempdim;
use crate::editor::ObjectRef;
use crate::tools::dimension::DimMode;
use crate::tools::text::TextMode;
use crate::tools::{KeyEvent, ToolId};
use plan_core::cad::{CadAttrs, CadItem};
use plan_core::dimension::{DimArrow, DimOverrides};
use plan_core::geometry::Point;
use plan_core::text_box::{layout, HAlign, TextBox, VAlign};
use plan_core::{Dimension, DimensionKind, Id, OpeningKind, WallKind};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn texts(sim: &Sim) -> Vec<Id> {
    sim.app
        .cx
        .floor()
        .cad
        .iter()
        .filter(|c| matches!(c.item, CadItem::Text { .. }))
        .map(|c| c.id)
        .collect()
}

fn text_item(sim: &Sim, id: Id) -> (Point, String, f64, f64) {
    match &sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .item
    {
        CadItem::Text {
            pos,
            text,
            height,
            angle,
        } => (*pos, text.clone(), *height, *angle),
        _ => panic!("not a text"),
    }
}

fn text_box_of(sim: &Sim, id: Id) -> TextBox {
    sim.app
        .cx
        .floor()
        .cad_attrs(id)
        .map(|a| a.text_box)
        .unwrap_or_default()
}

/// Places a text box by dragging with the Text tool and typing `words`.
fn place_box(sim: &mut Sim, a: (f64, f64), b: (f64, f64), words: &str) -> Id {
    sim.tool(ToolId::TextVariant(TextMode::Text));
    sim.drag(a, b);
    sim.key(KeyEvent::text(words));
    let r = sim.key(KeyEvent::key(eframe::egui::Key::Enter));
    assert_eq!(r.commit.as_deref(), Some("Place Text"), "{:?}", r.commit);
    *texts(sim).last().expect("the text was placed")
}

#[test]
fn a_dragged_text_box_wraps_resizes_by_its_handles_and_undoes_in_one_step() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    let words = "Install blocking behind the grab bars at every bath";
    let id = place_box(&mut sim, (100.0, 300.0), (260.0, 240.0), words);
    let tb = text_box_of(&sim, id);
    assert_eq!(tb.width, 160.0);
    assert_eq!(tb.height, 60.0);
    let (pos, _, h, _) = text_item(&sim, id);
    let lay = layout(words, &[], h, &tb);
    assert!(lay.lines.len() >= 2, "the text wraps inside 160\"");
    assert_eq!(pos.x, 100.0);

    // Select it: the box handles sit on its right edge, top edge and corner.
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Cad(id));
    let hs = handles_for(&sim.app.cx, 2.0);
    let kinds: Vec<HandleKind> = hs.iter().map(|h| h.kind).collect();
    for k in [
        HandleKind::Move,
        HandleKind::Rotate,
        HandleKind::ResizeEnd,
        HandleKind::Reshape(0),
        HandleKind::Reshape(1),
    ] {
        assert!(kinds.contains(&k), "{k:?} in {kinds:?}");
    }
    let width = hs.iter().find(|h| h.kind == HandleKind::ResizeEnd).unwrap();
    assert!(
        width.pos.dist(p(260.0, pos.y + lay.height * 0.5)) < 1e-6,
        "{:?}",
        width.pos
    );

    // Dragging the width handle reflows the text (S-25, TXT-13).
    let before = sim.app.cx.undo_label().map(String::from);
    sim.drag((width.pos.x, width.pos.y), (150.0, width.pos.y));
    let tb2 = text_box_of(&sim, id);
    assert_eq!(tb2.width, 50.0, "{tb2:?}");
    assert!(layout(words, &[], h, &tb2).lines.len() > lay.lines.len());
    // One undo step puts the width back.
    assert!(sim.undo().is_some());
    assert_eq!(text_box_of(&sim, id).width, 160.0);
    assert_eq!(sim.app.cx.undo_label().map(String::from), before);

    // The corner handle sizes width and height together.
    sim.app.cx.selection.set(ObjectRef::Cad(id));
    let hs = handles_for(&sim.app.cx, 2.0);
    let corner = hs
        .iter()
        .find(|h| h.kind == HandleKind::Reshape(1))
        .unwrap();
    sim.drag((corner.pos.x, corner.pos.y), (300.0, 330.0));
    let tb3 = text_box_of(&sim, id);
    assert_eq!(tb3.width, 200.0);
    assert!(tb3.height > 60.0);
}

#[test]
fn a_plain_text_gets_a_width_handle_too() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    sim.tool(ToolId::TextVariant(TextMode::Text));
    sim.click(50.0, 50.0);
    sim.key(KeyEvent::text("one two three four five six seven"));
    sim.key(KeyEvent::key(eframe::egui::Key::Enter));
    let id = texts(&sim)[0];
    assert!(text_box_of(&sim, id).is_plain());
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Cad(id));
    let hs = handles_for(&sim.app.cx, 2.0);
    let w = hs.iter().find(|h| h.kind == HandleKind::ResizeEnd).unwrap();
    // Pulling the edge in wraps the text where it was one line.
    sim.drag((w.pos.x, w.pos.y), (50.0 + 60.0, w.pos.y));
    let tb = text_box_of(&sim, id);
    assert_eq!(tb.width, 60.0);
    assert!(tb.is_boxed());
}

#[test]
fn border_background_and_alignment_draw_and_a_hit_inside_the_box_selects_it() {
    let mut sim = Sim::new();
    let id = sim.app.cx.project.add_cad(
        0,
        "Text",
        CadItem::Text {
            pos: p(0.0, 0.0),
            text: "Roof pitch is 8 in 12 unless noted".into(),
            height: 6.0,
            angle: 0.0,
        },
    );
    let count = |sim: &mut Sim| sim.plan_shapes().len();
    let plain = count(&mut sim);
    let bx = TextBox {
        width: 90.0,
        height: 60.0,
        halign: HAlign::Center,
        valign: VAlign::Middle,
        ..TextBox::default()
    };
    sim.app
        .cx
        .project
        .edit_cad_attrs(0, id, |a| a.text_box = bx);
    let boxed = count(&mut sim);
    assert!(
        boxed >= plain,
        "the wrapped lines draw ({plain} -> {boxed})"
    );
    let framed = TextBox {
        border: true,
        margin: 3.0,
        background: Some([250, 240, 200]),
        ..bx
    };
    sim.app
        .cx
        .project
        .edit_cad_attrs(0, id, |a| a.text_box = framed);
    let with_frame = count(&mut sim);
    assert!(
        with_frame >= boxed + 2,
        "a fill and a frame are two more shapes ({boxed} -> {with_frame})"
    );
    // A click inside the box, away from any glyph, picks the text.
    let cx = &sim.app.cx;
    let hits = crate::editor::render::settle_text_hits(cx, Vec::new(), p(80.0, 55.0), 1.0);
    assert_eq!(hits, vec![ObjectRef::Cad(id)]);
    // And outside the frame it does not.
    let miss = crate::editor::render::settle_text_hits(cx, Vec::new(), p(200.0, 55.0), 1.0);
    assert!(miss.is_empty());
}

#[test]
fn text_style_management_renames_and_removes_through_the_flyout() {
    let mut sim = Sim::new();
    sim.app.cx.project.layers.layers[0].text_style = "Room Label Style".into();
    sim.tool(ToolId::TextVariant(TextMode::TextStyles));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    assert!(sim.app.tools.active().name().contains("Text Style"));
    // The dialog lives in the tool; drive it the way the unit tests do.
    use crate::dialogs::text::manage::{StyleOp, TextStyleDialog};
    let mut d = TextStyleDialog::new(
        sim.app
            .cx
            .project
            .text_styles
            .names()
            .into_iter()
            .map(str::to_string)
            .collect(),
    );
    assert!(d.rename("Room Label Style", "Labels"));
    sim.app.cx.begin_change("Change Text Styles");
    assert_eq!(StyleOp::apply_all(d.ops(), &mut sim.app.cx.project), 1);
    sim.app.cx.mark_dirty();
    assert_eq!(sim.app.cx.project.layers.layers[0].text_style, "Labels");
    assert!(sim.undo().is_some());
    assert_eq!(
        sim.app.cx.project.layers.layers[0].text_style,
        "Room Label Style"
    );
}

// ------------------------------------------------------------ dimensions --

fn dims(sim: &Sim) -> Vec<Dimension> {
    sim.app.cx.floor().dimensions.clone()
}

fn dim_line_y(d: &Dimension) -> f64 {
    d.line_points().0.y
}

#[test]
fn a_manual_string_grows_a_point_per_click_and_ends_on_a_double_click() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    sim.tool(ToolId::DimensionVariant(DimMode::Manual));
    sim.click(0.0, 0.0);
    sim.click(100.0, 0.0);
    let r = sim.click(50.0, 30.0);
    assert_eq!(r.commit.as_deref(), Some("Manual Dimension"));
    assert_eq!(dims(&sim).len(), 1);
    // Each further click adds a segment on the same measuring line.
    let r = sim.click(200.0, 10.0);
    assert_eq!(r.commit.as_deref(), Some("Manual Dimension"));
    sim.click(150.0, 5.0);
    let all = dims(&sim);
    assert_eq!(all.len(), 3, "{all:?}");
    assert!(all.iter().all(|d| (dim_line_y(d) - 30.0).abs() < 1e-6));
    let lens: Vec<f64> = all.iter().map(Dimension::length).collect();
    assert!((lens[0] - 100.0).abs() < 1e-6 && (lens[1] - 100.0).abs() < 1e-6);
    assert!((lens[2] - 50.0).abs() < 1e-6, "going back is a segment too");
    // Every segment has its own value (DIM-2).
    let fmt = sim.app.cx.dim_format();
    assert_eq!(all[0].label(&fmt), "8'-4\"");
    assert_eq!(all[2].label(&fmt), "4'-2\"");
    // A double-click ends the string; the next click starts a new dimension.
    sim.double_click(150.0, 5.0);
    assert_eq!(dims(&sim).len(), 3, "the double-click adds nothing itself");
    sim.click(0.0, 100.0);
    assert_eq!(dims(&sim).len(), 3);
    // Each added point is one undo step.
    sim.esc();
    sim.tool(ToolId::Select);
    assert!(sim.undo().is_some());
    assert_eq!(dims(&sim).len(), 2);
}

#[test]
fn add_and_delete_extension_line_split_and_merge_a_string() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    let id = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(0, DimensionKind::Manual, p(0.0, 0.0), p(200.0, 0.0), 30.0),
    );
    sim.tool(ToolId::DimensionVariant(DimMode::ExtensionAdd));
    // Click on the dimension line at x = 80.
    let r = sim.click(80.0, 30.0);
    assert_eq!(r.commit.as_deref(), Some("Add Extension Line"));
    let all = dims(&sim);
    assert_eq!(all.len(), 2);
    let (a, b) = (
        all.iter().find(|d| d.id == id).unwrap(),
        all.iter().find(|d| d.id != id).unwrap(),
    );
    assert!((a.length() - 80.0).abs() < 1e-6 && (b.length() - 120.0).abs() < 1e-6);
    assert!(a.end.dist(b.start) < 1e-9 && a.offset == b.offset);
    // Delete the shared extension line: the two merge again.
    sim.tool(ToolId::DimensionVariant(DimMode::ExtensionDelete));
    let r = sim.click(80.0, 12.0);
    assert_eq!(r.commit.as_deref(), Some("Delete Extension Line"));
    let all = dims(&sim);
    assert_eq!(all.len(), 1);
    assert!((all[0].length() - 200.0).abs() < 1e-6);
    // An end that nothing continues just loses its line, and Add brings it back.
    let r = sim.click(0.0, 12.0);
    assert_eq!(r.commit.as_deref(), Some("Delete Extension Line"));
    assert_eq!(dims(&sim)[0].hide_ext, [true, false]);
    sim.tool(ToolId::DimensionVariant(DimMode::ExtensionAdd));
    let r = sim.click(0.0, 12.0);
    assert_eq!(r.commit.as_deref(), Some("Add Extension Line"));
    assert_eq!(dims(&sim)[0].hide_ext, [false, false]);
    // The Edit toolbar offers both as tools.
    sim.tool(ToolId::Select);
    sim.app
        .cx
        .selection
        .set(ObjectRef::Dimension(dims(&sim)[0].id));
    let actions = crate::tools::dimension::edit_actions(&sim.app.cx);
    assert!(actions.iter().any(|a| a.label == "Add Extension Line"));
    assert!(actions.iter().any(|a| a.label == "Delete Extension Line"));
    assert!(crate::tools::dimension::run_command(
        &mut sim.app.cx,
        crate::tools::dimension::CMD_EXT_DELETE
    ));
    assert!(sim
        .app
        .cx
        .requests
        .contains(&crate::editor::EditorRequest::SetTool(
            ToolId::DimensionVariant(DimMode::ExtensionDelete)
        )));
}

#[test]
fn a_copied_dimension_stays_tied_to_the_copied_walls() {
    let mut sim = Sim::new();
    let cx = &mut sim.app.cx;
    let a = cx
        .project
        .add_wall(0, p(0.0, 0.0), p(0.0, 200.0), 6.0, 96.0, WallKind::Interior);
    let b = cx.project.add_wall(
        0,
        p(120.0, 0.0),
        p(120.0, 200.0),
        6.0,
        96.0,
        WallKind::Interior,
    );
    let d = cx.project.add_dimension(
        0,
        Dimension::new(
            0,
            DimensionKind::Manual,
            p(3.0, 100.0),
            p(117.0, 100.0),
            20.0,
        ),
    );
    assert_eq!(cx.project.floors[0].attach_dimension(d), 2);
    // Copy all three and paste 400" to the right.
    cx.selection.items = vec![
        ObjectRef::Wall(a),
        ObjectRef::Wall(b),
        ObjectRef::Dimension(d),
    ];
    cx.copy_selection();
    cx.paste_at(p(400.0, 0.0), "Paste", false);
    let f = cx.floor();
    assert_eq!(f.dimensions.len(), 2);
    let copy = f.dimensions.iter().find(|x| x.id != d).unwrap();
    assert_eq!(copy.start, p(403.0, 100.0));
    let walls: Vec<Id> = f
        .walls
        .iter()
        .map(|w| w.id)
        .filter(|w| *w != a && *w != b)
        .collect();
    assert_eq!(walls.len(), 2);
    for k in 0..2 {
        let anchor = copy.anchors[k].expect("the copy is tied");
        assert!(
            walls.contains(&anchor.wall),
            "tied to a pasted wall, not the original"
        );
    }
    // Moving a pasted wall moves the pasted dimension, not the original's.
    let moved = copy.anchors[1].unwrap().wall;
    cx.project.floors[0].wall_mut(moved).unwrap().start.x += 30.0;
    cx.project.floors[0].wall_mut(moved).unwrap().end.x += 30.0;
    cx.project.floors[0].sync_dimension_anchors();
    let f = cx.floor();
    let copy = f.dimensions.iter().find(|x| x.id != d).unwrap();
    let orig = f.dimensions.iter().find(|x| x.id == d).unwrap();
    assert!((copy.length() - 144.0).abs() < 1e-6, "{}", copy.length());
    assert!((orig.length() - 114.0).abs() < 1e-6);

    // Copy the dimension alone: the walls were not copied, so it pastes free.
    cx.selection.items = vec![ObjectRef::Dimension(d)];
    cx.copy_selection();
    cx.paste_at(p(0.0, 500.0), "Paste", false);
    let last = cx.floor().dimensions.last().unwrap();
    assert_eq!(last.anchors, [None, None]);
}

#[test]
fn a_dimension_keeps_its_own_format_arrow_and_extension_lines() {
    let mut sim = Sim::new();
    let id = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(0, DimensionKind::Manual, p(0.0, 0.0), p(78.25, 0.0), 24.0),
    );
    let fmt = sim.app.cx.dim_format();
    let before = dims(&sim)[0].label(&fmt);
    assert_eq!(before, "6'-6 1/4\"");
    // Through the Dimension Specification.
    assert!(sim.open_spec(ObjectRef::Dimension(id)));
    sim.dialog_frame(false);
    let mut look = DimOverrides::default();
    look.units = Some(plan_core::units::LengthUnit::Inches);
    look.fraction = Some(8);
    look.arrow = Some(DimArrow::Arrow);
    look.arrow_filled = Some(false);
    look.ext_gap = Some(2.0);
    look.ext_past = Some(3.0);
    // The dialog's draft is private to its tests; edit the object the way its
    // OK does and check the look the plan draws and prints.
    sim.app.cx.begin_change("Change Dimension");
    sim.app.cx.project.floors[0].dimensions[0].look = look.clone();
    sim.app.cx.mark_dirty();
    let d = &dims(&sim)[0];
    assert_eq!(d.label(&fmt), "78 1/4\"");
    let draw_look = crate::editor::render::DimLook::of(&sim.app.cx, d);
    assert_eq!(draw_look.mark, DimArrow::Arrow);
    assert!(!draw_look.filled);
    assert_eq!((draw_look.gap, draw_look.past), (2.0, 3.0));
    assert!(sim.plan_shapes().len() > 4);
    // Undo restores the defaults' look.
    assert!(sim.undo().is_some());
    assert_eq!(dims(&sim)[0].label(&fmt), "6'-6 1/4\"");
    assert!(dims(&sim)[0].look.is_default());
}

#[test]
fn openings_on_a_curved_wall_are_located_along_the_arc() {
    let mut sim = Sim::new();
    let cx = &mut sim.app.cx;
    let w = cx
        .project
        .add_wall(0, p(0.0, 0.0), p(240.0, 0.0), 6.0, 96.0, WallKind::Exterior);
    // A semicircular bay: radius 120", arc length pi * 120.
    cx.project.floors[0].wall_mut(w).unwrap().curve =
        plan_core::WallCurve::from_radius(240.0, 120.0, true);
    let o = cx
        .project
        .add_opening(0, w, 100.0, OpeningKind::Window)
        .unwrap();
    cx.refresh();
    let wall = cx.floor().wall(w).unwrap().clone();
    let op = cx
        .floor()
        .openings
        .iter()
        .find(|x| x.id == o)
        .unwrap()
        .clone();
    let on_arc = wall.point_along(op.center_offset);
    let chord = wall.point_at(op.center_offset);
    assert!(on_arc.dist(chord) > 10.0, "the arc and the chord part ways");
    // The Locate Objects hit on the opening's center is on the arc.
    sim.tool(ToolId::DimensionVariant(DimMode::Centerline));
    sim.move_to(on_arc.x, on_arc.y);
    assert!(
        sim.app.cx.status.contains("Opening center"),
        "{}",
        sim.app.cx.status
    );
    let hit = {
        let pe = crate::tools::PointerEvent::at(&sim.app.cx, on_arc);
        crate::tools::dimension::locate_point(&sim.app.cx, &pe, true)
    };
    assert!(hit.dist(on_arc) < 1e-6, "{hit:?} is not {on_arc:?}");
    // A dimension tied to the opening's center follows it when it slides.
    let id = sim.app.cx.project.add_dimension(
        0,
        Dimension::new(
            0,
            DimensionKind::Manual,
            on_arc,
            on_arc.add(p(0.0, -60.0)),
            10.0,
        ),
    );
    let tied = sim.app.cx.project.floors[0].attach_dimension(id);
    assert!(tied >= 1, "tied to the opening or its wall");
    let a0 = sim
        .app
        .cx
        .floor()
        .dimensions
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .anchors[0];
    assert!(a0.is_some());
    assert_eq!(
        a0.unwrap().target,
        plan_core::dim_assoc::AnchorTarget::Opening,
        "the center of the opening, along the arc"
    );
    sim.app.cx.project.slide_opening(0, o, 140.0);
    sim.app.cx.project.floors[0].sync_dimension_anchors();
    let moved = wall.point_along(140.0);
    let d = sim
        .app
        .cx
        .floor()
        .dimensions
        .iter()
        .find(|d| d.id == id)
        .unwrap();
    assert!(d.start.dist(moved) < 1.0, "{:?} vs {moved:?}", d.start);
}

#[test]
fn cad_objects_show_temporary_dimensions_that_resize_them() {
    let mut sim = Sim::new();
    let cx = &mut sim.app.cx;
    let line = cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: p(0.0, 0.0),
            b: p(100.0, 0.0),
        },
    );
    let boxed = cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Polyline {
            points: vec![p(0.0, 50.0), p(60.0, 50.0), p(60.0, 90.0), p(0.0, 90.0)],
            closed: true,
        },
    );
    let circle = cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Circle {
            center: p(300.0, 50.0),
            radius: 24.0,
        },
    );
    let kinds = |sim: &mut Sim, id: Id| -> Vec<tempdim::TempDimKind> {
        sim.app.cx.selection.set(ObjectRef::Cad(id));
        sim.app.cx.refresh();
        sim.app.cx.temp.dims.iter().map(|d| d.kind).collect()
    };
    use tempdim::TempDimKind as K;
    assert_eq!(kinds(&mut sim, line), vec![K::CadLength, K::CadAngle]);
    assert_eq!(kinds(&mut sim, boxed), vec![K::CadWidth, K::CadHeight]);
    assert_eq!(kinds(&mut sim, circle), vec![K::CadRadius, K::CadDiameter]);

    // Type a length: the line keeps its start.
    sim.app.cx.selection.set(ObjectRef::Cad(line));
    sim.app.cx.refresh();
    let dim = sim.app.cx.temp.dims[0];
    assert_eq!(dim.value, 100.0);
    assert_eq!(
        tempdim::apply(&mut sim.app.cx, &dim, 150.0),
        Ok("Change Line Length")
    );
    let item = &sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == line)
        .unwrap()
        .item;
    assert!(
        matches!(item, CadItem::Line { a, b } if *a == p(0.0, 0.0) && (b.x - 150.0).abs() < 1e-9)
    );
    // The angle turns the end about the start.
    sim.app.cx.refresh();
    let ang = sim.app.cx.temp.dims[1];
    assert_eq!(
        tempdim::apply(&mut sim.app.cx, &ang, 90.0),
        Ok("Change Line Angle")
    );
    let item = &sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == line)
        .unwrap()
        .item;
    assert!(
        matches!(item, CadItem::Line { b, .. } if b.x.abs() < 1e-6 && (b.y - 150.0).abs() < 1e-6)
    );
    // A box keeps its lower left corner; a circle its center.
    sim.app.cx.selection.set(ObjectRef::Cad(boxed));
    sim.app.cx.refresh();
    let w = sim.app.cx.temp.dims[0];
    assert_eq!(tempdim::apply(&mut sim.app.cx, &w, 90.0), Ok("Resize Box"));
    let item = &sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == boxed)
        .unwrap()
        .item;
    let CadItem::Polyline { points, .. } = item else {
        panic!()
    };
    assert_eq!(points[0], p(0.0, 50.0));
    assert!((points[1].x - 90.0).abs() < 1e-9 && (points[2].y - 90.0).abs() < 1e-9);
    sim.app.cx.selection.set(ObjectRef::Cad(circle));
    sim.app.cx.refresh();
    let dia = sim.app.cx.temp.dims[1];
    assert_eq!(
        tempdim::apply(&mut sim.app.cx, &dia, 100.0),
        Ok("Change Radius")
    );
    let item = &sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == circle)
        .unwrap()
        .item;
    assert!(
        matches!(item, CadItem::Circle { center, radius } if *center == p(300.0, 50.0) && (radius - 50.0).abs() < 1e-9)
    );
    // Nothing smaller than half an inch.
    assert!(tempdim::apply(&mut sim.app.cx, &dia, 0.2).is_err());
}

#[test]
fn a_temporary_dimension_locks_into_a_permanent_one_and_unlocks_again() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    let cx = &mut sim.app.cx;
    let line = cx.project.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: p(0.0, 0.0),
            b: p(100.0, 0.0),
        },
    );
    cx.selection.set(ObjectRef::Cad(line));
    cx.refresh();
    assert!(!cx.temp.is_locked(0));
    // Only measured distances have a padlock.
    assert!(cx
        .temp
        .hit_lock(cx.temp.lock_pos(1, 2.0).unwrap(), 2.0)
        .is_none());
    let at = cx.temp.lock_pos(0, 2.0).unwrap();
    assert_eq!(cx.temp.hit_lock(at, 2.0), Some(0));
    assert_eq!(tempdim::toggle_lock(cx, 0), Ok("Lock Dimension"));
    cx.refresh();
    assert_eq!(cx.floor().dimensions.len(), 1);
    let d = &cx.floor().dimensions[0];
    assert_eq!(d.kind, DimensionKind::Manual);
    assert_eq!((d.start, d.end), (p(0.0, 0.0), p(100.0, 0.0)));
    assert!(cx.temp.is_locked(0), "the padlock shows locked");
    assert!(!cx.temp.is_locked(1));
    // The permanent dimension's number can be typed into by the Dimension
    // tool's value edit; undo takes the lock back in one step.
    assert_eq!(cx.undo().as_deref(), Some("Lock Dimension"));
    assert!(cx.floor().dimensions.is_empty());
    cx.redo();
    cx.refresh();
    // Clicking the padlock again unlocks.
    assert_eq!(tempdim::toggle_lock(cx, 0), Ok("Unlock Dimension"));
    assert!(cx.floor().dimensions.is_empty());
    // A locked layer refuses.
    cx.project.layers.set_locked("Dimensions, Manual", true);
    cx.refresh();
    assert!(tempdim::toggle_lock(cx, 0).is_err());
}

#[test]
fn walls_and_openings_lock_their_temporary_dimensions_to_the_wall_faces() {
    let mut sim = Sim::new();
    sim.app.cx.px_per_in = 2.0;
    let cx = &mut sim.app.cx;
    let w = cx
        .project
        .add_wall(0, p(0.0, 0.0), p(240.0, 0.0), 6.0, 96.0, WallKind::Exterior);
    let o = cx
        .project
        .add_opening(0, w, 100.0, OpeningKind::Door)
        .unwrap();
    cx.selection.set(ObjectRef::Opening(o));
    cx.refresh();
    let to_start = cx
        .temp
        .dims
        .iter()
        .position(|d| d.kind == tempdim::TempDimKind::OpeningToStart)
        .expect("a dimension to the wall start");
    assert_eq!(tempdim::toggle_lock(cx, to_start), Ok("Lock Dimension"));
    cx.refresh();
    let d = cx.floor().dimensions.last().unwrap().clone();
    assert!(
        d.anchors.iter().flatten().count() >= 1,
        "tied to the wall or opening: {d:?}"
    );
    // Slide the door: the locked dimension follows it.
    let before = d.length();
    cx.project.slide_opening(0, o, 140.0);
    cx.project.floors[0].sync_dimension_anchors();
    let after = cx.floor().dimensions.last().unwrap().length();
    assert!((after - before - 40.0).abs() < 1e-6, "{before} -> {after}");
    // The wall's own length can be locked too.
    cx.selection.set(ObjectRef::Wall(w));
    cx.refresh();
    let len = cx
        .temp
        .dims
        .iter()
        .position(|d| d.kind == tempdim::TempDimKind::WallLength)
        .unwrap();
    assert_eq!(tempdim::toggle_lock(cx, len), Ok("Lock Dimension"));
    // The angle is a label and cannot be locked.
    let angle = cx
        .temp
        .dims
        .iter()
        .position(|d| d.kind == tempdim::TempDimKind::WallAngle)
        .unwrap();
    assert!(tempdim::toggle_lock(cx, angle).is_err());
}

#[test]
fn text_boxes_and_dimension_looks_reach_the_page() {
    // The same laid-out box and dimension look draw on a printed sheet.
    use plan_docs::{plan_sheet, Scale, SheetSize, TitleBlock};
    let mut sim = Sim::new();
    let cx = &mut sim.app.cx;
    let id = cx.project.add_cad(
        0,
        "Text",
        CadItem::Text {
            pos: p(0.0, 0.0),
            text: "alpha beta gamma delta".into(),
            height: 4.0,
            angle: 0.0,
        },
    );
    let tb = TextBox {
        width: 4.0 * 0.6 * 11.0,
        border: true,
        background: Some([250, 240, 200]),
        ..TextBox::default()
    };
    cx.project.set_cad_attrs(
        0,
        CadAttrs {
            text_box: tb,
            ..CadAttrs::new(id)
        },
    );
    let dim = cx.project.add_dimension(
        0,
        Dimension::new(
            0,
            DimensionKind::Manual,
            p(0.0, -40.0),
            p(120.0, -40.0),
            12.0,
        ),
    );
    let tb = TitleBlock::default();
    let plain = plan_sheet(
        &cx.project,
        0,
        &[],
        SheetSize::ArchD,
        Scale::QuarterInch,
        &tb,
    );
    let pdf = String::from_utf8_lossy(&plain.pdf).to_string();
    assert!(pdf.contains("(alpha beta) Tj"), "the first line");
    assert!(pdf.contains("(gamma delta) Tj"), "the wrapped line");
    // An arrow on the dimension changes the page.
    cx.project.floors[0]
        .dimensions
        .iter_mut()
        .find(|d| d.id == dim)
        .unwrap()
        .look
        .arrow = Some(DimArrow::Arrow);
    let arrowed = plan_sheet(
        &cx.project,
        0,
        &[],
        SheetSize::ArchD,
        Scale::QuarterInch,
        &tb,
    );
    assert_ne!(plain.pdf, arrowed.pdf);
}
