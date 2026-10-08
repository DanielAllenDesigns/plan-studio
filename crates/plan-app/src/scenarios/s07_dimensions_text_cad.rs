//! Scenario 7: automatic exterior dimensions, a manual dimension, text, CAD
//! line and box, their specification dialogs and the Color restyle
//! (DIM-11, DIM-24, DIM-31, TXT-1, TXT-11, CAD-1, CAD-12, View > Color).

use super::{draw_shell, shape_colors, Sim};
use crate::editor::{EditorRequest, ObjectRef};
use crate::toolbar::{Action, ViewFlag};
use crate::tools::cad::CadMode;
use crate::tools::dimension::DimMode;
use crate::tools::text::TextMode;
use crate::tools::{KeyEvent, ToolId};
use eframe::egui::Key;
use plan_core::cad::CadItem;
use plan_core::{DimensionKind, WallKind};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim
}

#[test]
fn auto_exterior_dimensions_dimension_the_whole_shell_in_one_undo_step() {
    let mut sim = house();
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    assert_eq!(sim.app.tools.active().name(), "Auto Exterior Dimensions");
    let r = sim.click(240.0, 180.0);
    assert!(r.commit.is_some(), "{r:?} / {}", sim.app.cx.status);
    let dims = sim.app.cx.floor().dimensions.clone();
    assert!(dims.len() >= 2, "{} dimensions", dims.len());
    assert!(dims.iter().all(|d| d.kind == DimensionKind::AutoExterior));
    // DIM-24: an overall string across each side of the box, measured over
    // the outside faces of the walls.
    let t = sim.app.cx.wall_thickness(WallKind::Exterior);
    let lens: Vec<f64> = dims.iter().map(|d| d.length()).collect();
    let over = |len: f64| lens.iter().any(|l| (l - len).abs() < 8.0);
    assert!(
        over(W + 1.0 + t),
        "an overall width of about {}: {lens:?}",
        W + 1.0 + t
    );
    assert!(
        over(H + 1.0 + t),
        "an overall depth of about {}: {lens:?}",
        H + 1.0 + t
    );
    // The dimension text is architectural feet and inches.
    let text = sim.app.cx.fmt_dim(dims[0].length());
    assert!(text.contains('\'') && text.contains('"'), "{text}");
    // One undo step.
    let label = sim.app.cx.undo_label().map(String::from).unwrap();
    assert_eq!(sim.undo().as_deref(), Some(label.as_str()));
    assert!(sim.app.cx.floor().dimensions.is_empty());
    sim.redo();
    assert_eq!(sim.app.cx.floor().dimensions.len(), dims.len());
}

#[test]
fn a_manual_dimension_measures_between_wall_faces_and_opens_its_specification() {
    let mut sim = house();
    sim.tool(ToolId::DimensionVariant(DimMode::Manual));
    sim.click(200.0, 4.0);
    sim.click(200.0, H - 3.0);
    assert!(
        sim.app.cx.floor().dimensions.is_empty(),
        "the line is not placed yet"
    );
    let r = sim.click(260.0, 180.0);
    assert_eq!(r.commit.as_deref(), Some("Manual Dimension"));
    let d = sim.app.cx.floor().dimensions[0].clone();
    assert_eq!(d.kind, DimensionKind::Manual);
    // Face to face: the clear span between the south and north walls.
    // DIM-4: points locate to the main-layer faces, so it is a little more
    // than the clear span between the finished faces and less than the
    // centerline distance.
    let t = sim.app.cx.wall_thickness(WallKind::Exterior);
    let finished = H + 1.0 - t;
    assert!(
        d.length() > finished - 0.5 && d.length() < H + 1.0,
        "{} between {finished} and {}",
        d.length(),
        H + 1.0
    );
    assert_eq!(
        sim.app.cx.selection.single(),
        Some(ObjectRef::Dimension(d.id))
    );

    // DIM-31: double-click asks for the Dimension Specification.
    sim.requests.clear();
    sim.double_click(260.0, 180.0);
    assert!(
        sim.requests
            .contains(&EditorRequest::OpenSpec(ObjectRef::Dimension(d.id))),
        "{:?}",
        sim.requests
    );
    assert!(sim.app.spec.is_open());
    // OK with no edit leaves the dimension and the undo stack alone.
    let steps = sim.app.cx.undo_label().map(String::from);
    sim.ok();
    assert!(!sim.app.spec.is_open());
    assert_eq!(sim.app.cx.undo_label().map(String::from), steps);
    assert!(sim.app.cx.floor().dimensions.len() == 1);
    // And it goes with Delete / undo.
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(ObjectRef::Dimension(d.id));
    sim.key(KeyEvent::key(Key::Delete));
    assert!(sim.app.cx.floor().dimensions.is_empty());
    sim.undo();
    assert_eq!(sim.app.cx.floor().dimensions.len(), 1);
}

#[test]
fn every_dimension_mode_activates_and_names_itself() {
    let mut sim = house();
    for mode in DimMode::ALL {
        sim.tool(ToolId::DimensionVariant(mode));
        assert_eq!(sim.app.tools.active().name(), mode.name());
        assert!(!sim.app.tools.active().hint().is_empty(), "{mode:?}");
        sim.esc();
        sim.app.tools.active_id();
    }
}

#[test]
fn text_is_typed_on_a_click_and_its_specification_opens() {
    let mut sim = house();
    sim.tool(ToolId::TextVariant(TextMode::Text));
    assert_eq!(sim.app.tools.active().name(), "Text");
    sim.click(100.0, 100.0);
    assert!(
        sim.app.cx.temp.editing.is_some(),
        "typing asks the shell for characters"
    );
    sim.key(KeyEvent::text("Living Room"));
    let r = sim.key(KeyEvent::key(Key::Enter));
    assert_eq!(r.commit.as_deref(), Some("Place Text"));
    let c = sim.app.cx.floor().cad[0].clone();
    // TXT-11: text lives on the Text layer.
    assert_eq!(c.layer, "Text");
    let CadItem::Text { text, height, .. } = &c.item else {
        panic!("not text: {:?}", c.item)
    };
    assert_eq!(text, "Living Room");
    assert_eq!(*height, sim.app.cx.defaults.text.height);

    // Select Objects: double-click the text opens the Text Specification.
    sim.tool(ToolId::Select);
    sim.requests.clear();
    sim.double_click(104.0, 100.0 + height / 2.0);
    assert!(
        sim.requests.iter().any(|r| matches!(
            r,
            EditorRequest::OpenSpec(ObjectRef::Cad(_) | ObjectRef::Text(_))
        )),
        "{:?}",
        sim.requests
    );
    assert!(sim.app.spec.is_open());
    sim.cancel();
    assert!(!sim.app.spec.is_open());
    // Undo removes it.
    sim.undo();
    assert!(sim.app.cx.floor().cad.is_empty());
}

#[test]
fn a_cad_line_and_a_box_land_on_the_cad_layer_and_open_their_dialogs() {
    let mut sim = house();
    sim.tool(ToolId::CadVariant(CadMode::Line));
    assert_eq!(sim.app.tools.active().name(), "Draw Line");
    let r = sim.drag((50.0, 50.0), (200.0, 50.0));
    assert!(r.commit.is_some(), "{r:?}");
    sim.esc();
    sim.tool(ToolId::CadVariant(CadMode::RectPolyline));
    assert_eq!(sim.app.tools.active().name(), "Rectangular Polyline");
    sim.click(250.0, 100.0);
    sim.click(350.0, 200.0);
    let cad = sim.app.cx.floor().cad.clone();
    assert_eq!(cad.len(), 2, "{cad:?}");
    assert!(cad.iter().all(|c| c.layer == "CAD, Default"));
    assert!(matches!(cad[0].item, CadItem::Line { .. }));
    let CadItem::Polyline { points, closed } = &cad[1].item else {
        panic!("not a polyline: {:?}", cad[1].item)
    };
    assert_eq!(points.len(), 4);
    assert!(*closed || points.first() == points.last());
    let xs: Vec<f64> = points.iter().map(|p| p.x).collect();
    let ys: Vec<f64> = points.iter().map(|p| p.y).collect();
    let span = |v: &[f64]| {
        v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min)
    };
    assert!((span(&xs) - 100.0).abs() < 1.0 && (span(&ys) - 100.0).abs() < 1.0);

    // Each object's specification dialog opens and OK without changes is a no-op.
    for (i, o) in cad.iter().enumerate() {
        assert!(
            sim.app.spec.open(&mut sim.app.cx, ObjectRef::Cad(o.id)),
            "object {i}"
        );
        let steps = sim.app.cx.undo_label().map(String::from);
        sim.ok();
        assert!(!sim.app.spec.is_open());
        assert_eq!(sim.app.cx.undo_label().map(String::from), steps);
    }
    // Select Objects double-click on the box edge requests its dialog.
    sim.tool(ToolId::Select);
    sim.requests.clear();
    sim.double_click(300.0, 100.0);
    assert!(
        sim.requests
            .contains(&EditorRequest::OpenSpec(ObjectRef::Cad(cad[1].id))),
        "{:?}",
        sim.requests
    );
}

/// The Dimension, Text and CAD specification dialogs: change a value in the
/// draft (the `draft_mut` test accessors stand in for typing), press OK, and
/// get exactly one undo step each that restores the old value.
#[test]
fn editing_the_dimension_text_and_cad_dialogs_is_one_undo_step_each() {
    let mut sim = house();

    // A manual dimension, a text and a line.
    sim.tool(ToolId::DimensionVariant(DimMode::Manual));
    sim.click(200.0, 4.0);
    sim.click(200.0, H - 3.0);
    sim.click(260.0, 180.0);
    sim.tool(ToolId::TextVariant(TextMode::Text));
    sim.click(100.0, 100.0);
    sim.key(KeyEvent::text("Living Room"));
    sim.key(KeyEvent::key(Key::Enter));
    sim.tool(ToolId::CadVariant(CadMode::Line));
    sim.drag((50.0, 50.0), (200.0, 50.0));
    sim.esc();
    let dim = sim.app.cx.floor().dimensions[0].clone();
    let text = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| matches!(c.item, CadItem::Text { .. }))
        .unwrap()
        .clone();
    let line = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .find(|c| matches!(c.item, CadItem::Line { .. }))
        .unwrap()
        .clone();
    let cad = |sim: &Sim, id| {
        sim.app
            .cx
            .floor()
            .cad
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .clone()
    };

    // Dimension: offset 36".
    assert!(sim
        .app
        .spec
        .open(&mut sim.app.cx, ObjectRef::Dimension(dim.id)));
    sim.app.spec.dimension_draft_mut().unwrap().offset = 36.0;
    let before = sim.app.cx.floor().dimensions[0].clone();
    assert_eq!(before.offset, dim.offset, "nothing changes until OK");
    sim.ok();
    assert!(!sim.app.spec.is_open());
    assert_eq!(sim.app.cx.floor().dimensions[0].offset, 36.0);
    assert_eq!(sim.app.cx.undo_label(), Some("Change Dimension"));
    assert_eq!(sim.undo().as_deref(), Some("Change Dimension"));
    assert_eq!(sim.app.cx.floor().dimensions[0].offset, dim.offset);

    // Text: new wording.
    assert!(sim.app.spec.open(&mut sim.app.cx, ObjectRef::Cad(text.id)));
    if let CadItem::Text { text: t, .. } = &mut sim.app.spec.text_draft_mut().unwrap().item {
        *t = "Great Room".into();
    }
    sim.ok();
    assert!(!sim.app.spec.is_open());
    assert!(matches!(
        &cad(&sim, text.id).item,
        CadItem::Text { text: t, .. } if t == "Great Room"
    ));
    assert_eq!(sim.app.cx.undo_label(), Some("Change Text"));
    assert_eq!(sim.undo().as_deref(), Some("Change Text"));
    assert!(matches!(
        &cad(&sim, text.id).item,
        CadItem::Text { text: t, .. } if t == "Living Room"
    ));

    // CAD line: move its end point.
    assert!(sim.app.spec.open(&mut sim.app.cx, ObjectRef::Cad(line.id)));
    if let CadItem::Line { b, .. } = &mut sim.app.spec.cad_draft_mut().unwrap().item {
        b.y += 40.0;
    }
    sim.ok();
    assert!(!sim.app.spec.is_open());
    assert_ne!(cad(&sim, line.id).item, line.item);
    assert_eq!(sim.app.cx.undo_label(), Some("Change CAD Object"));
    assert_eq!(sim.undo().as_deref(), Some("Change CAD Object"));
    assert_eq!(cad(&sim, line.id).item, line.item);

    // Cancel leaves the plan and the undo stack alone.
    let steps = sim.app.cx.undo_label().map(String::from);
    assert!(sim
        .app
        .spec
        .open(&mut sim.app.cx, ObjectRef::Dimension(dim.id)));
    sim.app.spec.dimension_draft_mut().unwrap().offset = 99.0;
    sim.cancel();
    assert_eq!(sim.app.cx.floor().dimensions[0].offset, dim.offset);
    assert_eq!(sim.app.cx.undo_label().map(String::from), steps);
}

#[test]
fn every_cad_mode_and_text_mode_activates_with_a_hint() {
    let mut sim = house();
    for m in CadMode::ALL {
        sim.tool(ToolId::CadVariant(m));
        assert_eq!(sim.app.tools.active().name(), m.name());
        assert!(!sim.app.tools.active().hint().is_empty(), "{m:?}");
    }
    for m in TextMode::ALL {
        sim.tool(ToolId::TextVariant(m));
        assert_eq!(sim.app.tools.active().name(), m.name());
        assert!(!sim.app.tools.active().hint().is_empty(), "{m:?}");
    }
}

#[test]
fn restyle_color_off_turns_the_whole_plan_gray_and_back() {
    let mut sim = house();
    sim.tool(ToolId::DimensionVariant(DimMode::AutoExterior));
    sim.click(240.0, 180.0);
    sim.tool(ToolId::Door);
    sim.click(120.0, 0.0);
    sim.tool(ToolId::CadVariant(CadMode::Line));
    sim.drag((50.0, 50.0), (200.0, 50.0));
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    sim.app.cx.view_flags.insert(ViewFlag::Color);
    let colored = sim.plan_shapes();
    assert!(colored.len() > 20, "{} shapes", colored.len());
    let is_gray = |c: eframe::egui::Color32| c.r() == c.g() && c.g() == c.b();
    let any_color = colored.iter().flat_map(shape_colors).any(|c| !is_gray(c));
    assert!(any_color, "with Color on the plan has colored strokes");

    sim.action(Action::ToggleFlag(ViewFlag::Color));
    assert!(!sim.app.cx.view_flags.contains(&ViewFlag::Color));
    let gray = sim.plan_shapes();
    assert!(gray.len() > 20);
    let offenders: Vec<String> = gray
        .iter()
        .enumerate()
        .filter(|(_, sh)| !matches!(sh, eframe::egui::Shape::Rect(r) if r.rect.width() >= 1399.0))
        .filter(|(_, sh)| shape_colors(sh).iter().any(|c| !is_gray(*c)))
        .map(|(i, sh)| format!("#{i} {:.200}", format!("{sh:?}")))
        .collect();
    assert!(
        offenders.is_empty(),
        "{} colored shapes with Color off: {offenders:#?}",
        offenders.len()
    );
    sim.action(Action::ToggleFlag(ViewFlag::Color));
    assert!(sim.app.cx.view_flags.contains(&ViewFlag::Color));
}
