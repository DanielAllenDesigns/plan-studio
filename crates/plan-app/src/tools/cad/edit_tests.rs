//! Tests of the CAD edit tools, boxes, blocks, hatching and the commands.

use super::edit;
use super::*;
use crate::dialogs::cad::blocks::ManagerAction;
use crate::plan_defaults;
use crate::tools::ToolSet;
use plan_core::cad::{CadBlockInfo, CadObject};

fn new_cx() -> EditorContext {
    EditorContext::new(plan_defaults::embedded())
}

fn tool(mode: CadMode) -> CadTool {
    let mut t = CadTool::default();
    t.set_mode(mode);
    t
}

fn click(t: &mut CadTool, cx: &mut EditorContext, x: f64, y: f64) {
    let p = PointerEvent::at(cx, Point::new(x, y));
    t.pointer_move(cx, p);
    t.pointer_down(cx, p.with_down(true));
    t.pointer_up(cx, p);
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: Point, b: Point) -> bool {
    a.dist(b) < 1e-6
}

fn line(cx: &mut EditorContext, a: Point, b: Point) -> Id {
    cx.project.add_cad(0, CAD_LAYER, CadItem::Line { a, b })
}

fn item(cx: &EditorContext, id: Id) -> CadItem {
    cad_by_id(cx.floor(), id).expect("object").item.clone()
}

/// Drawn objects, hidden data records left out.
fn drawn(cx: &EditorContext) -> Vec<&CadObject> {
    cx.floor().cad.iter().collect()
}

fn run_frame(t: &mut CadTool, cx: &mut EditorContext) {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| t.frame(cx, ctx));
}

#[test]
fn fillet_joins_two_lines_with_a_tangent_arc() {
    let mut cx = new_cx();
    let a = line(&mut cx, pt(120.0, 0.0), pt(24.0, 0.0));
    let b = line(&mut cx, pt(0.0, 120.0), pt(0.0, 36.0));
    let mut t = tool(CadMode::Fillet);
    t.edit.radius = 12.0;
    click(&mut t, &mut cx, 72.0, 0.0);
    assert!(t.edit.pick.is_some(), "the first line waits for the second");
    click(&mut t, &mut cx, 0.0, 72.0);
    assert!(near(
        match item(&cx, a) {
            CadItem::Line { b, .. } => b,
            _ => unreachable!(),
        },
        pt(12.0, 0.0)
    ));
    let CadItem::Line { a: bs, b: be } = item(&cx, b) else {
        panic!("line")
    };
    assert!(near(bs, pt(0.0, 120.0)) && near(be, pt(0.0, 12.0)));
    let arcs: Vec<_> = drawn(&cx)
        .into_iter()
        .filter(|c| matches!(c.item, CadItem::Arc { .. }))
        .collect();
    assert_eq!(arcs.len(), 1);
    let CadItem::Arc { center, radius, .. } = arcs[0].item else {
        unreachable!()
    };
    assert!(near(center, pt(12.0, 12.0)) && (radius - 12.0).abs() < 1e-9);
    assert_eq!(cx.undo().as_deref(), Some("Fillet"));
    assert!(drawn(&cx)
        .iter()
        .all(|c| matches!(c.item, CadItem::Line { .. })));
}

#[test]
fn chamfer_cuts_a_polyline_corner_and_a_square_corner_of_two_lines() {
    let mut cx = new_cx();
    let sq = cx.project.add_cad(
        0,
        CAD_LAYER,
        CadItem::Polyline {
            points: vec![
                pt(0.0, 0.0),
                pt(120.0, 0.0),
                pt(120.0, 120.0),
                pt(0.0, 120.0),
            ],
            closed: true,
        },
    );
    let mut t = tool(CadMode::Chamfer);
    t.edit.chamfer = (12.0, 24.0);
    click(&mut t, &mut cx, 120.0, 0.0);
    let CadItem::Polyline { points, closed } = item(&cx, sq) else {
        panic!("polyline")
    };
    assert!(closed && points.len() == 5);
    assert!(near(points[1], pt(108.0, 0.0)) && near(points[2], pt(120.0, 24.0)));
    // Fillet a polyline corner too: more points, none left on the corner.
    let mut f = tool(CadMode::Fillet);
    f.edit.radius = 24.0;
    click(&mut f, &mut cx, 0.0, 120.0);
    let CadItem::Polyline { points, .. } = item(&cx, sq) else {
        panic!("polyline")
    };
    assert!(points.len() > 7);
    assert!(points.iter().all(|p| p.dist(pt(0.0, 120.0)) > 1.0));
    // Clicking away from every corner does nothing.
    let before = item(&cx, sq);
    click(&mut f, &mut cx, 60.0, 0.0);
    assert_eq!(item(&cx, sq), before);
}

#[test]
fn offset_copies_a_line_to_the_clicked_side() {
    let mut cx = new_cx();
    let a = line(&mut cx, pt(0.0, 0.0), pt(120.0, 0.0));
    let mut t = tool(CadMode::Offset);
    // Through the second click.
    click(&mut t, &mut cx, 60.0, 0.0);
    click(&mut t, &mut cx, 60.0, 36.0);
    let mut lines = drawn(&cx)
        .into_iter()
        .filter_map(|c| match c.item {
            CadItem::Line { a, b } => Some((c.id, a, b)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    lines.retain(|l| l.0 != a);
    assert!(near(lines[0].1, pt(0.0, 36.0)) && near(lines[0].2, pt(120.0, 36.0)));
    // A fixed distance goes to the side that was clicked.
    t.edit.offset = 12.0;
    click(&mut t, &mut cx, 60.0, 0.0);
    click(&mut t, &mut cx, 60.0, -80.0);
    let low = drawn(&cx).into_iter().any(|c| {
        matches!(c.item, CadItem::Line { a, b } if near(a, pt(0.0, -12.0)) && near(b, pt(120.0, -12.0)))
    });
    assert!(low);
    // A circle offsets its radius; a closed polyline offsets inward.
    let c = cx.project.add_cad(
        0,
        CAD_LAYER,
        CadItem::Circle {
            center: pt(400.0, 400.0),
            radius: 48.0,
        },
    );
    let _ = c;
    assert_eq!(
        offset_item(
            &CadItem::Circle {
                center: pt(400.0, 400.0),
                radius: 48.0
            },
            pt(400.0, 400.0),
            12.0
        ),
        Some(CadItem::Circle {
            center: pt(400.0, 400.0),
            radius: 36.0
        })
    );
    let sq = CadItem::Polyline {
        points: vec![
            pt(0.0, 0.0),
            pt(100.0, 0.0),
            pt(100.0, 100.0),
            pt(0.0, 100.0),
        ],
        closed: true,
    };
    let Some(CadItem::Polyline { points, .. }) = offset_item(&sq, pt(50.0, 50.0), 10.0) else {
        panic!("offset polyline")
    };
    assert!(near(points[0], pt(10.0, 10.0)));
}

#[test]
fn trim_extend_and_break_edit_lines_against_their_neighbours() {
    let mut cx = new_cx();
    let a = line(&mut cx, pt(0.0, 0.0), pt(120.0, 0.0));
    line(&mut cx, pt(36.0, -12.0), pt(36.0, 12.0));
    line(&mut cx, pt(84.0, -12.0), pt(84.0, 12.0));
    let mut t = tool(CadMode::Trim);
    click(&mut t, &mut cx, 60.0, 0.0);
    let CadItem::Line { a: s, b: e } = item(&cx, a) else {
        panic!("line")
    };
    assert!(near(s, pt(0.0, 0.0)) && near(e, pt(36.0, 0.0)));
    assert!(drawn(&cx).iter().any(|c| {
        matches!(c.item, CadItem::Line { a, b } if near(a, pt(84.0, 0.0)) && near(b, pt(120.0, 0.0)))
    }));
    assert_eq!(cx.undo().as_deref(), Some("Trim Line"));

    // Extend the end of a short line to the next object.
    let mut cx = new_cx();
    let short = line(&mut cx, pt(0.0, 0.0), pt(48.0, 0.0));
    line(&mut cx, pt(120.0, -24.0), pt(120.0, 24.0));
    let mut t = tool(CadMode::Extend);
    click(&mut t, &mut cx, 44.0, 0.0);
    let CadItem::Line { b, .. } = item(&cx, short) else {
        panic!("line")
    };
    assert!(near(b, pt(120.0, 0.0)));

    // Break a line in two at the click.
    let mut cx = new_cx();
    let l = line(&mut cx, pt(0.0, 0.0), pt(120.0, 0.0));
    let mut t = tool(CadMode::BreakLine);
    click(&mut t, &mut cx, 48.0, 0.0);
    assert_eq!(drawn(&cx).len(), 2);
    let CadItem::Line { b, .. } = item(&cx, l) else {
        panic!("line")
    };
    assert!(near(b, pt(48.0, 0.0)));
}

#[test]
fn reverse_parallel_and_perpendicular_turn_lines() {
    let mut cx = new_cx();
    let a = line(&mut cx, pt(0.0, 0.0), pt(120.0, 0.0));
    let mut t = tool(CadMode::ReverseDirection);
    click(&mut t, &mut cx, 60.0, 0.0);
    assert_eq!(
        item(&cx, a),
        CadItem::Line {
            a: pt(120.0, 0.0),
            b: pt(0.0, 0.0)
        }
    );

    let mut cx = new_cx();
    let turn = line(&mut cx, pt(0.0, 0.0), pt(36.0, 48.0));
    let reference = line(&mut cx, pt(0.0, 240.0), pt(240.0, 240.0));
    let mut t = tool(CadMode::MakeParallel);
    click(&mut t, &mut cx, 30.0, 40.0);
    click(&mut t, &mut cx, 120.0, 240.0);
    let CadItem::Line { a, b } = item(&cx, turn) else {
        panic!("line")
    };
    assert!(near(a, pt(0.0, 0.0)) && near(b, pt(60.0, 0.0)), "{b:?}");
    let _ = reference;
    let mut t = tool(CadMode::MakePerpendicular);
    click(&mut t, &mut cx, 60.0, 0.0);
    click(&mut t, &mut cx, 120.0, 240.0);
    let CadItem::Line { b, .. } = item(&cx, turn) else {
        panic!("line")
    };
    assert!(near(b, pt(0.0, 60.0)) || near(b, pt(0.0, -60.0)), "{b:?}");
}

#[test]
fn commands_convert_the_selection() {
    let mut cx = new_cx();
    let ids: Vec<Id> = [
        (pt(0.0, 0.0), pt(120.0, 0.0)),
        (pt(120.0, 0.0), pt(120.0, 120.0)),
        (pt(120.0, 120.0), pt(0.0, 120.0)),
        (pt(0.0, 120.0), pt(0.0, 0.0)),
    ]
    .into_iter()
    .map(|(a, b)| line(&mut cx, a, b))
    .collect();
    cx.selection.items = ids.iter().map(|i| ObjectRef::Cad(*i)).collect();
    let mut t = tool(CadMode::ConvertToPolyline);
    assert!(t
        .run_command(&mut cx, CadMode::ConvertToPolyline)
        .commit
        .is_some());
    let objs = drawn(&cx);
    assert_eq!(objs.len(), 1);
    let CadItem::Polyline { points, closed } = &objs[0].item else {
        panic!("polyline")
    };
    assert!(*closed && points.len() == 4);

    // To a spline: smoother, so more points.
    let id = objs[0].id;
    cx.selection.set(ObjectRef::Cad(id));
    t.run_command(&mut cx, CadMode::ConvertToSpline);
    let CadItem::Polyline { points, .. } = item(&cx, id) else {
        panic!("polyline")
    };
    assert_eq!(points.len(), 4 * SPLINE_SEGMENTS_PER_SPAN);

    // Back to lines: one per segment.
    t.run_command(&mut cx, CadMode::PolylineToLines);
    let lines = drawn(&cx);
    assert_eq!(lines.len(), 4 * SPLINE_SEGMENTS_PER_SPAN);
    assert!(lines.iter().all(|c| matches!(c.item, CadItem::Line { .. })));
    // Nothing selected that fits says so.
    cx.selection.clear();
    let r = t.run_command(&mut cx, CadMode::PolylineToLines);
    assert!(r.commit.is_none() && cx.status.contains("Select"));
}

#[test]
fn hatch_fills_a_closed_shape_with_lines_and_replaces_the_old_hatch() {
    let mut cx = new_cx();
    let sq = cx.project.add_cad(
        0,
        CAD_LAYER,
        CadItem::Polyline {
            points: vec![
                pt(0.0, 0.0),
                pt(120.0, 0.0),
                pt(120.0, 120.0),
                pt(0.0, 120.0),
            ],
            closed: true,
        },
    );
    let mut t = tool(CadMode::Hatch);
    t.edit.hatch = 1;
    t.edit.hatch_spacing = 12.0;
    click(&mut t, &mut cx, 60.0, 60.0);
    let hatched = drawn(&cx).len();
    assert!(hatched > 8, "diagonal lines fill the square: {hatched}");
    let a = cx.floor().cad_attrs(sq).unwrap();
    let fill = a.fill.unwrap();
    assert_eq!(fill.pattern, "Diagonal Lines");
    assert_eq!(fill.lines.len(), hatched - 1);
    // Every hatch line sits inside the square, in one group with the outline.
    for c in drawn(&cx).into_iter().filter(|c| c.id != sq) {
        let (lo, hi) = c.item.bounds();
        assert!(lo.x >= -1e-6 && lo.y >= -1e-6 && hi.x <= 120.0 + 1e-6 && hi.y <= 120.0 + 1e-6);
    }
    let g = cx.floor().group_of(plan_core::ObjectRef::Cad(sq)).unwrap();
    assert_eq!(g.members.len(), hatched);
    // Hatching again with a cross hatch replaces, it does not stack.
    t.edit.hatch = 2;
    click(&mut t, &mut cx, 60.0, 60.0);
    let again = drawn(&cx).len();
    assert!(
        again > hatched && again < hatched * 3,
        "{again} vs {hatched}"
    );
    assert_eq!(
        cx.floor().cad_attrs(sq).unwrap().fill.unwrap().pattern,
        "Cross Hatch"
    );
    // Solid keeps the outline alone.
    t.edit.hatch = 0;
    click(&mut t, &mut cx, 60.0, 60.0);
    assert_eq!(drawn(&cx).len(), 1);
    assert_eq!(cx.floor().cad_attrs(sq).unwrap().fill.unwrap().pattern, "");
    // A click outside any closed shape explains itself.
    click(&mut t, &mut cx, 500.0, 500.0);
    assert!(cx.status.contains("inside"));
    // Undo takes the last hatch step back.
    assert_eq!(cx.undo().as_deref(), Some("Hatch"));
    assert!(drawn(&cx).len() > 1);
}

#[test]
fn boxes_are_three_click_oriented_boxes() {
    let mut cx = new_cx();
    let mut t = tool(CadMode::Box);
    click(&mut t, &mut cx, 0.0, 0.0);
    click(&mut t, &mut cx, 120.0, 0.0);
    assert!(drawn(&cx).is_empty(), "the depth click is still to come");
    click(&mut t, &mut cx, 60.0, 48.0);
    let CadItem::Polyline { points, closed } = &drawn(&cx)[0].item else {
        panic!("a box polyline")
    };
    assert!(*closed && near(points[2], pt(120.0, 48.0)) && near(points[3], pt(0.0, 48.0)));

    for (mode, objects) in [
        (CadMode::CrossBox, 3),
        (CadMode::BlockingBox, 2),
        (CadMode::Insulation, 2),
    ] {
        let mut cx = new_cx();
        let mut t = tool(mode);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 120.0, 0.0);
        click(&mut t, &mut cx, 60.0, 48.0);
        assert_eq!(drawn(&cx).len(), objects, "{mode:?}");
        assert_eq!(cx.floor().groups.len(), 1, "{mode:?} selects as one");
    }
}

#[test]
fn spline_bezier_has_the_expected_point_count_and_passes_through_the_points() {
    let mut cx = new_cx();
    let mut t = tool(CadMode::Spline);
    t.edit.spline = SplineKind::Bezier;
    for (x, y) in [(0.0, 0.0), (60.0, 36.0), (120.0, 0.0), (180.0, 36.0)] {
        click(&mut t, &mut cx, x, y);
    }
    // Angle snaps may nudge the clicks: compare with what was clicked.
    let clicked = t.pending().to_vec();
    assert_eq!(clicked.len(), 4);
    t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
    let CadItem::Polyline { points, closed } = &drawn(&cx)[0].item else {
        panic!("spline polyline")
    };
    assert!(!closed);
    assert_eq!(points.len(), 3 * SPLINE_SEGMENTS_PER_SPAN + 1);
    for (k, q) in clicked.iter().enumerate() {
        assert!(near(points[k * SPLINE_SEGMENTS_PER_SPAN], *q), "point {k}");
    }
    // The fit spline of the same points has the same count.
    let mut cx = new_cx();
    let mut t = tool(CadMode::Spline);
    for (x, y) in [(0.0, 0.0), (60.0, 36.0), (120.0, 0.0), (180.0, 36.0)] {
        click(&mut t, &mut cx, x, y);
    }
    t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
    let CadItem::Polyline { points, .. } = &drawn(&cx)[0].item else {
        panic!("spline polyline")
    };
    assert_eq!(points.len(), 3 * SPLINE_SEGMENTS_PER_SPAN + 1);
}

fn block_of_two(cx: &mut EditorContext) -> (Id, Id, Id) {
    let a = line(cx, pt(0.0, 0.0), pt(120.0, 0.0));
    let b = line(cx, pt(0.0, 48.0), pt(120.0, 48.0));
    cx.selection.items = vec![ObjectRef::Cad(a), ObjectRef::Cad(b)];
    let mut t = tool(CadMode::MakeBlock);
    assert!(t.make_block(cx).commit.is_some());
    let g = cx.floor().cad_blocks()[0].group;
    (a, b, g)
}

#[test]
fn insertion_and_backoff_points_are_set_by_clicking_a_block() {
    let mut cx = new_cx();
    let (_, _, g) = block_of_two(&mut cx);
    assert_eq!(cx.floor().cad_block(g).unwrap().name, "CAD Block 1");
    let mid = cx.floor().cad_block(g).unwrap().insertion.unwrap();
    assert!(near(mid, pt(60.0, 24.0)));
    cx.selection.clear();
    let mut t = tool(CadMode::AddInsertionPoint);
    click(&mut t, &mut cx, 60.0, 0.0);
    assert_eq!(t.edit.block, Some(g));
    click(&mut t, &mut cx, 12.0, 12.0);
    assert!(near(
        cx.floor().cad_block(g).unwrap().insertion.unwrap(),
        pt(12.0, 12.0)
    ));
    assert!(t.edit.block.is_none());
    cx.selection.clear();
    let mut t = tool(CadMode::AddBackoffPoint);
    click(&mut t, &mut cx, 60.0, 48.0);
    click(&mut t, &mut cx, 120.0, 24.0);
    assert!(near(
        cx.floor().cad_block(g).unwrap().backoff.unwrap(),
        pt(120.0, 24.0)
    ));
    assert_eq!(cx.undo().as_deref(), Some("Add Arrow Backoff Point"));
    assert!(cx.floor().cad_block(g).unwrap().backoff.is_none());
    // No block under the click (and none selected) says so.
    cx.selection.clear();
    let mut t = tool(CadMode::AddInsertionPoint);
    click(&mut t, &mut cx, 600.0, 600.0);
    assert!(cx.status.contains("block"));
}

#[test]
fn block_management_renames_inserts_edits_explodes_and_deletes() {
    let mut cx = new_cx();
    let (a, _, g) = block_of_two(&mut cx);
    let mut t = tool(CadMode::BlockManagement);
    run_frame(&mut t, &mut cx);
    assert!(
        matches!(t.edit.dialog, Some(edit::BlockUi::Manager(_))),
        "the manager is open"
    );
    // The command was a dialog: the tool stays until it closes.
    assert!(cx.requests.is_empty());
    let take = |t: &mut CadTool| match t.edit.dialog.take() {
        Some(edit::BlockUi::Manager(m)) => m,
        _ => panic!("manager"),
    };

    let m = take(&mut t);
    t.manager_action(&mut cx, m, ManagerAction::Rename(g, "Bench".into()));
    assert_eq!(cx.floor().cad_block(g).unwrap().name, "Bench");

    let m = take(&mut t);
    t.manager_action(&mut cx, m, ManagerAction::Insert(g));
    assert_eq!(t.mode(), CadMode::InsertBlock);
    click(&mut t, &mut cx, 480.0, 480.0);
    let blocks = cx.floor().cad_blocks();
    assert_eq!(blocks.len(), 2);
    let copy = blocks.iter().find(|b| b.group != g).unwrap();
    assert_eq!(copy.name, "Bench");
    assert!(near(copy.insertion.unwrap(), pt(480.0, 480.0)));
    let moved = cx.floor().group_items(copy.group);
    assert_eq!(moved.len(), 2);
    assert!(near(moved[0].bounds().0, pt(420.0, 456.0)));

    // Edit opens the specification; OK stores the draft, Cancel does not.
    t.set_mode(CadMode::BlockManagement);
    t.edit.pending = false;
    t.run_command(&mut cx, CadMode::BlockManagement);
    let m = take(&mut t);
    t.manager_action(&mut cx, m, ManagerAction::Edit(g));
    let Some(edit::BlockUi::Edit(id, mut d)) = t.edit.dialog.take() else {
        panic!("block specification")
    };
    assert_eq!(id, g);
    let mut info: CadBlockInfo = d.draft().clone();
    info.name = "Long Bench".into();
    info.backoff = Some(pt(1.0, 1.0));
    *d.draft_mut() = info;
    t.edit_outcome(&mut cx, g, d, crate::dialogs::Outcome::Ok);
    assert_eq!(cx.floor().cad_block(g).unwrap().name, "Long Bench");
    assert_eq!(
        cx.requests,
        vec![EditorRequest::SetTool(ToolId::Select)],
        "closing the dialog returns to Select Objects"
    );
    assert_eq!(cx.undo().as_deref(), Some("Edit CAD Block"));
    assert_eq!(cx.floor().cad_block(g).unwrap().name, "Bench");

    // Explode, then delete the copy.
    t.run_command(&mut cx, CadMode::BlockManagement);
    let m = take(&mut t);
    t.manager_action(&mut cx, m, ManagerAction::Explode(g));
    assert!(cx.floor().cad_block(g).is_none());
    assert!(cx.floor().cad.iter().any(|o| o.id == a));
    let m = match t.edit.dialog.take() {
        Some(edit::BlockUi::Manager(m)) => m,
        _ => panic!("manager again"),
    };
    let copy_group = cx.floor().cad_blocks()[0].group;
    t.manager_action(&mut cx, m, ManagerAction::Delete(copy_group));
    assert!(cx.floor().cad_blocks().is_empty());
    assert_eq!(drawn(&cx).len(), 2, "only the exploded originals remain");
}

#[test]
fn edit_cad_block_needs_a_selected_block() {
    let mut cx = new_cx();
    let mut t = tool(CadMode::EditBlock);
    run_frame(&mut t, &mut cx);
    assert!(t.edit.dialog.is_none());
    assert!(cx.status.contains("Select a CAD block"));
    assert_eq!(cx.requests, vec![EditorRequest::SetTool(ToolId::Select)]);
    cx.requests.clear();
    let (a, _, _) = block_of_two(&mut cx);
    cx.selection.set(ObjectRef::Cad(a));
    let mut t = tool(CadMode::EditBlock);
    run_frame(&mut t, &mut cx);
    assert!(matches!(t.edit.dialog, Some(edit::BlockUi::Edit(..))));
    assert!(cx.requests.is_empty());
    // The dialog draws without panicking.
    run_frame(&mut t, &mut cx);
}

#[test]
fn temporary_points_live_on_their_own_layer_and_can_be_deleted() {
    let mut cx = new_cx();
    let mut t = tool(CadMode::PlacePoint);
    click(&mut t, &mut cx, 60.0, 60.0);
    click(&mut t, &mut cx, 120.0, 60.0);
    let mut m = tool(CadMode::PointMarker);
    click(&mut m, &mut cx, 180.0, 60.0);
    let on_temp = |cx: &EditorContext| {
        cx.floor()
            .cad
            .iter()
            .filter(|c| c.layer == TEMP_POINT_LAYER)
            .count()
    };
    assert_eq!(on_temp(&cx), 4, "two crosses of two lines each");
    assert!(cx.project.layers.get(TEMP_POINT_LAYER).is_some());
    let markers = drawn(&cx).len() - 4;
    assert_eq!(markers, 3, "the marker is a permanent cross and circle");

    let mut d = tool(CadMode::DeleteTempPoints);
    run_frame(&mut d, &mut cx);
    assert_eq!(on_temp(&cx), 0);
    assert_eq!(drawn(&cx).len(), 3, "the marker stays");
    assert!(cx.floor().groups.iter().all(|g| g.members.len() >= 2));
    assert_eq!(cx.requests, vec![EditorRequest::SetTool(ToolId::Select)]);
    assert_eq!(cx.undo().as_deref(), Some("Delete Temporary Points"));
    assert_eq!(on_temp(&cx), 4);
    // Nothing to delete says so.
    let mut cx = new_cx();
    let mut d = tool(CadMode::DeleteTempPoints);
    run_frame(&mut d, &mut cx);
    assert!(cx.status.contains("no temporary"));
}

#[test]
fn make_and_explode_blocks_run_from_the_frame_hook() {
    let mut cx = new_cx();
    let a = line(&mut cx, pt(0.0, 0.0), pt(10.0, 0.0));
    let b = line(&mut cx, pt(0.0, 10.0), pt(10.0, 10.0));
    cx.selection.items = vec![ObjectRef::Cad(a), ObjectRef::Cad(b)];
    let mut t = tool(CadMode::MakeBlock);
    run_frame(&mut t, &mut cx);
    assert_eq!(
        cx.floor().cad_blocks().len(),
        1,
        "picking the command runs it"
    );
    assert_eq!(cx.requests, vec![EditorRequest::SetTool(ToolId::Select)]);
    run_frame(&mut t, &mut cx);
    assert_eq!(cx.floor().cad_blocks().len(), 1, "once, not every frame");
    let mut t = tool(CadMode::ExplodeBlock);
    run_frame(&mut t, &mut cx);
    assert!(cx.floor().cad_blocks().is_empty());
    assert!(cx.floor().groups.is_empty());
    assert_eq!(cx.selection.len(), 2);
}

#[test]
fn cad_detail_from_view_copies_the_view_into_a_new_floor() {
    let mut cx = new_cx();
    cx.project.add_wall(
        0,
        pt(0.0, 0.0),
        pt(240.0, 0.0),
        6.0,
        96.0,
        plan_core::WallKind::Exterior,
    );
    line(&mut cx, pt(0.0, 60.0), pt(240.0, 60.0));
    cx.project.add_dimension(
        0,
        plan_core::Dimension::new(
            0,
            plan_core::DimensionKind::Manual,
            pt(0.0, 0.0),
            pt(240.0, 0.0),
            24.0,
        ),
    );
    let mut t = tool(CadMode::DetailFromView);
    run_frame(&mut t, &mut cx);
    assert_eq!(cx.project.floors.len(), 2);
    assert_eq!(cx.floor, 1);
    let f = cx.floor();
    assert_eq!(f.name, "CAD Detail");
    // The wall outline and the line, plus the dimension.
    assert_eq!(f.cad.len(), 2);
    assert!(f
        .cad
        .iter()
        .any(|c| matches!(c.item, CadItem::Polyline { closed: true, .. })));
    assert_eq!(f.dimensions.len(), 1);
    // The source floor is untouched.
    assert_eq!(cx.project.floors[0].cad.len(), 1);
    assert_eq!(cx.undo().as_deref(), Some("CAD Detail From View"));
    assert_eq!(cx.project.floors.len(), 1);
    // An empty view has nothing to copy.
    let mut cx = new_cx();
    let mut t = tool(CadMode::DetailFromView);
    run_frame(&mut t, &mut cx);
    assert_eq!(cx.project.floors.len(), 1);
    assert!(cx.status.contains("no lines"));
}

#[test]
fn enter_types_the_fillet_radius_and_the_strip_adjusts_settings() {
    let mut cx = new_cx();
    let mut t = tool(CadMode::Fillet);
    t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
    assert!(t.typed.is_some());
    // Replace the shown radius with 2'-0".
    for _ in 0..12 {
        t.key(&mut cx, KeyEvent::key(egui::Key::Backspace));
    }
    t.key(&mut cx, KeyEvent::text("2'"));
    t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
    assert!((t.edit.radius - 24.0).abs() < 1e-9, "{}", t.edit.radius);
    assert!(t.typed.is_none());
    // A bad value is refused and stays open.
    t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
    for _ in 0..12 {
        t.key(&mut cx, KeyEvent::key(egui::Key::Backspace));
    }
    t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
    assert!(t.typed.is_some() && cx.status.contains("valid"));
    t.key(&mut cx, KeyEvent::escape());
    // The strip buttons nudge the radius and cycle the hatch pattern.
    let r = t.edit.radius;
    assert!(t.setting_click(BTN_SET_PLUS));
    assert!(t.edit.radius > r);
    let mut h = tool(CadMode::Hatch);
    let first = h.edit.hatch;
    assert!(h.setting_click(BTN_HATCH_NEXT));
    assert_eq!(h.edit.hatch, first + 1);
    assert!(h
        .setting_buttons()
        .iter()
        .any(|b| b.label.starts_with("Pattern")));
    assert!(!h.setting_click(999));
}

#[test]
fn every_cad_text_and_dimension_flyout_entry_activates() {
    use crate::toolbar::{self, Action};
    let flyouts = [
        toolbar::dimensions(),
        toolbar::auto_dimensions(),
        toolbar::text_tools(),
        toolbar::points(),
        toolbar::lines(),
        toolbar::arcs(),
        toolbar::circles(),
        toolbar::boxes(),
        toolbar::cad_blocks(),
    ];
    let ctx = egui::Context::default();
    let mut total = 0;
    for f in &flyouts {
        for e in &f.entries {
            let Action::SetTool(id) = e.action else {
                panic!("{} / {} is not a tool: {:?}", f.group, e.name, e.action);
            };
            let mut cx = new_cx();
            let mut set = ToolSet::new();
            set.set_active(&mut cx, id);
            assert_eq!(set.active().name(), e.name, "{}", f.group);
            assert_ne!(cx.status, "Tool not implemented yet", "{}", e.name);
            // Command entries do their work on the first frame without
            // panicking even with nothing selected.
            for _ in 0..2 {
                let _ = ctx.run(egui::RawInput::default(), |ctx| set.frame(&mut cx, ctx));
            }
            total += 1;
        }
    }
    assert!(total >= 40, "{total} entries");
}
