//! Scenario 56: callouts, markers, notes and the Rich Text Edit Bar (round 15,
//! TXT-29..56, manual pp. 521 to 570). Each text tool creates its object with
//! its specification dialog (Chief's tabs) in one undo step; handles edit the
//! record; a callout linked to a camera follows Send to Layout; notes number
//! themselves and feed the Note Schedule; everything prints on a layout page.

use super::{draw_shell, Sim};
use crate::dialogs::layout::{PageChoice, Placement, SendSource, SendSpec};
use crate::editor::{handles, ObjectRef};
use crate::shell::layout_window::{self as lw, LayoutView};
use crate::tools::text::TextMode;
use crate::tools::ToolId;
use plan_core::callout::{
    handle, AnnotRef, Callout, CalloutShape, Marker, MarkerKind, ViewKind, ViewLink,
};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::Id;
use plan_docs::MasterList;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    super::s21_layout_print::isolate_home();
    lw::use_memory_master_list(MasterList::default());
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    sim
}

fn group(sim: &Sim, head: Id) -> Vec<ObjectRef> {
    crate::editor::selection::expand_groups(&sim.app.cx, &[ObjectRef::Cad(head)])
}

fn pos_of(sim: &Sim, id: u8) -> Point {
    handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in)
        .into_iter()
        .find(|x| x.kind == handles::HandleKind::Annot(id))
        .map(|x| x.pos)
        .expect("handle")
}

fn steps(sim: &Sim) -> usize {
    sim.app.cx.action_history().0.len()
}

fn head(sim: &Sim) -> Id {
    let f = sim.app.cx.floor();
    f.annots
        .callouts
        .first()
        .map(|c| c.items[0])
        .or_else(|| f.annots.markers.first().map(|c| c.items[0]))
        .or_else(|| f.annots.notes.first().map(|c| c.items[0]))
        .expect("an annotation")
}

fn texts(sim: &Sim, items: &[Id]) -> Vec<String> {
    items
        .iter()
        .filter_map(|i| {
            sim.app
                .cx
                .floor()
                .cad
                .iter()
                .find(|c| c.id == *i)
                .and_then(|c| match &c.item {
                    CadItem::Text { text, .. } => Some(text.clone()),
                    _ => None,
                })
        })
        .collect()
}

/// The dialog the host has open: its title and tabs.
fn open_dialog(sim: &mut Sim) -> (String, Vec<&'static str>) {
    for _ in 0..3 {
        sim.dialog_frame(false);
    }
    let d = sim
        .app
        .spec
        .annot_dialog_mut()
        .expect("a Callout, Marker or Note Specification is open");
    (d.title(), d.tab_names())
}

#[test]
fn each_tool_opens_its_specification_with_chiefs_tabs_and_places_in_one_undo_step() {
    let cases: [(TextMode, &str, Vec<&str>); 3] = [
        (
            TextMode::Callout,
            "Callout Specification",
            vec![
                "Callout",
                "Attributes",
                "Line Style",
                "Section Arrow",
                "Main Text Style",
                "Link",
            ],
        ),
        (
            TextMode::Marker,
            "Marker Specification",
            vec!["Marker", "Line Style", "Text Style"],
        ),
        (
            TextMode::Note,
            "Note Specification",
            vec!["Note", "Line Style", "Text Style", "Object Information", "Schedule"],
        ),
    ];
    for (mode, title, tabs) in cases {
        let mut sim = house();
        sim.tool(ToolId::TextVariant(mode));
        let before = steps(&sim);
        sim.click(240.0, 180.0);
        let (t, tb) = open_dialog(&mut sim);
        assert_eq!((t.as_str(), tb), (title, tabs), "{mode:?}");
        assert!(sim.app.cx.floor().annots.is_empty(), "nothing before OK");
        sim.ok();
        let f = sim.app.cx.floor();
        assert_eq!(
            f.annots.callouts.len() + f.annots.markers.len() + f.annots.notes.len(),
            1,
            "{mode:?} placed"
        );
        assert_eq!(steps(&sim), before + 1, "{mode:?}: one undo step");
        assert!(!f.cad.is_empty());
        // The group is selected as one.
        assert_eq!(sim.app.cx.selection.len(), f.cad.len());
        // Undo and redo take it away and bring it back.
        sim.undo();
        assert!(sim.app.cx.floor().annots.is_empty() && sim.app.cx.floor().cad.is_empty());
        sim.redo();
        assert!(!sim.app.cx.floor().annots.is_empty());
        // The placed object opens the same dialog from Open Object, and
        // changing its text is one more step; Cancel changes nothing.
        let h = head(&sim);
        let before = steps(&sim);
        sim.open_spec(ObjectRef::Cad(h));
        let (t, _) = open_dialog(&mut sim);
        assert_eq!(t, title);
        sim.cancel();
        assert_eq!(steps(&sim), before);
        sim.open_spec(ObjectRef::Cad(h));
        open_dialog(&mut sim);
        sim.app
            .spec
            .annot_dialog_mut()
            .unwrap()
            .edit_label(|l| *l = "Z9".into());
        sim.ok();
        assert_eq!(steps(&sim), before + 1, "{mode:?}: the edit is one step");
        sim.undo();
        assert_eq!(steps(&sim), before);
    }
}

#[test]
fn ok_in_the_callout_dialog_places_a_callout_with_its_label_and_shape() {
    let mut sim = house();
    sim.tool(ToolId::TextVariant(TextMode::Callout));
    // The strip's shape button picks the next default shape.
    sim.app.cx.project.annot_defaults.callout.shape = CalloutShape::Hexagon;
    sim.click(100.0, 100.0);
    open_dialog(&mut sim);
    sim.app
        .spec
        .annot_dialog_mut()
        .unwrap()
        .edit_label(|l| *l = "A1".into());
    sim.ok();
    let c = &sim.app.cx.floor().annots.callouts[0];
    assert_eq!(c.shape, CalloutShape::Hexagon);
    assert_eq!(c.center, Point::new(100.0, 100.0));
    assert!(texts(&sim, &c.items).contains(&"A1".to_string()));
    // Six sides.
    assert!(sim.app.cx.floor().cad.iter().any(
        |o| matches!(&o.item, CadItem::Polyline { points, closed: true } if points.len() == 6)
    ));
}

#[test]
fn the_ten_shapes_all_place() {
    let mut sim = house();
    for (i, s) in CalloutShape::TEN.into_iter().enumerate() {
        sim.app.cx.begin_change("test");
        let c = Callout {
            shape: s,
            center: Point::new(40.0 * i as f64, 200.0),
            ..Callout::default()
        };
        sim.app.cx.project.add_callout(0, c);
    }
    sim.app.cx.refresh();
    assert_eq!(sim.app.cx.floor().annots.callouts.len(), 10);
    assert!(sim.app.cx.floor().cad.len() >= 20);
}

#[test]
fn handles_resize_rotate_and_add_arrows_in_one_undo_step_each() {
    let mut sim = house();
    sim.app.cx.begin_change("Place Callout");
    let h = sim.app.cx.project.add_callout(
        0,
        Callout {
            center: Point::new(240.0, 180.0),
            label: "7".into(),
            ..Callout::default()
        },
    );
    sim.app.cx.mark_dirty();
    sim.app.cx.selection.items = group(&sim, h);
    sim.tool(ToolId::Select);
    sim.app.cx.refresh();
    let hs = handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in);
    assert!(hs.len() >= 4, "{hs:?}");
    // Concentric resize.
    let before = steps(&sim);
    let a = pos_of(&sim, handle::RESIZE);
    sim.drag((a.x, a.y), (a.x + 20.0, a.y));
    assert_eq!(steps(&sim), before + 1);
    let c = &sim.app.cx.floor().annots.callouts[0];
    assert!(!c.auto_size && c.size > 20.0, "{}", c.size);
    assert_eq!(c.center, Point::new(240.0, 180.0), "the center stays");
    // Rotate.
    let before = steps(&sim);
    let r = pos_of(&sim, handle::ROTATE);
    sim.drag((r.x, r.y), (r.x + 60.0, r.y - 60.0));
    assert_eq!(steps(&sim), before + 1);
    assert!(sim.app.cx.floor().annots.callouts[0].shape_angle.abs() > 1.0);
    // Add Callout Arrow, then Add Text Line with Arrow.
    let before = steps(&sim);
    let ar = pos_of(&sim, handle::ADD_ARROW);
    sim.drag((ar.x, ar.y), (ar.x + 60.0, ar.y + 60.0));
    assert_eq!(sim.app.cx.floor().annots.callouts[0].arrows.angles.len(), 1);
    let ln = pos_of(&sim, handle::ADD_LINE);
    sim.drag((ln.x, ln.y), (ln.x - 80.0, ln.y - 80.0));
    assert_eq!(sim.app.cx.floor().annots.callouts[0].leaders.len(), 1);
    assert_eq!(steps(&sim), before + 2);
    // Everything is one group still, and moving it keeps the record.
    let c = &sim.app.cx.floor().annots.callouts[0];
    let members = group(&sim, c.items[0]);
    assert_eq!(members.len(), c.items.len());
}

#[test]
fn a_linked_callout_shows_the_camera_label_and_the_sheet_after_send_to_layout() {
    let mut sim = house();
    let cam = sim.app.cx.project.add_camera(plan_core::CameraObject::new(
        plan_core::CameraKind::CrossSection { back_clip: None },
        Point::new(240.0, 100.0),
        90.0,
        "Section A",
        0,
    ));
    sim.app.cx.begin_change("Place Callout");
    let mut c = Callout {
        center: Point::new(240.0, 100.0),
        label: "%referenced_view_callout_label%".into(),
        auto_below: true,
        link: Some(ViewLink {
            kind: ViewKind::Camera,
            id: cam,
            name: "Section A".into(),
        }),
        ..Callout::default()
    };
    c.section.on = true;
    c.section.above.text = "%linked_view_name%".into();
    sim.app.cx.project.add_callout(0, c);
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let items = sim.app.cx.floor().annots.callouts[0].items.clone();
    assert_eq!(texts(&sim, &items), vec!["1", "Section A"], "not sent yet");
    // Send to Layout (the real command): the sheet number appears by itself.
    let mut v = LayoutView::default();
    assert!(v.create(&mut sim.app.cx.project, None));
    let spec = SendSpec {
        source: SendSource::Camera {
            id: cam,
            name: "Section A".into(),
        },
        page: PageChoice::New,
        scale: None,
        placement: Placement::Centered,
    };
    v.send(&mut sim.app.cx.project, &spec, None).expect("sent");
    sim.app.cx.mark_dirty();
    sim.app.cx.refresh();
    let items = sim.app.cx.floor().annots.callouts[0].items.clone();
    let shown = texts(&sim, &items);
    let page = sim
        .app
        .cx
        .project
        .layout_pages()
        .into_iter()
        .find(|p| p.sources.iter().any(|(k, _)| k == "Camera"))
        .expect("the camera's page")
        .label();
    assert_eq!(shown, vec!["1".to_string(), page.clone(), "Section A".to_string()]);
    // And it prints on the plan's layout page (the callout draws in a box).
    v.send(
        &mut sim.app.cx.project,
        &SendSpec {
            source: SendSource::Plan {
                floor: 0,
                layer_set: "Default Set".into(),
            },
            page: PageChoice::Existing(1),
            scale: None,
            placement: Placement::FirstFree,
        },
        None,
    )
    .expect("plan sent");
    let pdf = lw::print_bytes(v.layout().unwrap(), &sim.app.cx.project, None);
    let text: String = pdf.iter().map(|&b| b as char).collect();
    assert!(
        text.contains(&format!("({page})")) || text.contains("Section A"),
        "the callout's text is on the printed page"
    );
}

#[test]
fn notes_number_per_type_feed_the_note_schedule_and_renumber_on_delete() {
    let mut sim = house();
    sim.tool(ToolId::TextVariant(TextMode::Note));
    for (x, body) in [(60.0, "Verify"), (120.0, "Match"), (180.0, "Patch")] {
        sim.click(x, 200.0);
        open_dialog(&mut sim);
        sim.app
            .spec
            .annot_dialog_mut()
            .unwrap()
            .edit_label(|l| *l = body.into());
        sim.ok();
    }
    let rows = sim.app.cx.project.note_rows();
    assert_eq!(
        rows.iter().map(|r| r.mark.as_str()).collect::<Vec<_>>(),
        vec!["Note 1", "Note 2", "Note 3"]
    );
    // Each note shows its number and a Caution symbol (no schedule yet).
    let first = sim.app.cx.floor().annots.notes[0].items.clone();
    assert_eq!(texts(&sim, &first), vec!["1", "!"]);
    // Create Note Schedule from Note(s): the symbol goes.
    sim.tool(ToolId::Select);
    sim.app.cx.selection.items = group(&sim, first[0]);
    let acts = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    assert!(acts
        .iter()
        .any(|a| a.label == "Create Note Schedule from Note(s)"));
    let before = steps(&sim);
    sim.app.cx.run_custom("annot.note_schedule");
    sim.app.cx.refresh();
    assert_eq!(steps(&sim), before + 1);
    let first = sim.app.cx.floor().annots.notes[0].items.clone();
    assert_eq!(texts(&sim, &first), vec!["1"], "the schedule exists now");
    // The schedule table lists the notes.
    let rows = plan_docs::schedule_kinds::entries(
        &sim.app.cx.project,
        plan_core::schedules::ScheduleKind::Note,
        None,
    );
    assert_eq!(rows.len(), 3);
    // Delete the middle one: the last renumbers.
    let middle = sim.app.cx.floor().annots.notes[1].items.clone();
    sim.app.cx.selection.items = group(&sim, middle[0]);
    sim.app.cx.delete_selection();
    sim.app.cx.refresh();
    let marks: Vec<String> = sim
        .app
        .cx
        .project
        .note_rows()
        .into_iter()
        .map(|r| r.mark)
        .collect();
    assert_eq!(marks, vec!["Note 1", "Note 2"]);
    let last = sim.app.cx.floor().annots.notes[1].items.clone();
    assert_eq!(texts(&sim, &last), vec!["2"]);
    sim.undo();
    assert_eq!(sim.app.cx.project.note_rows().len(), 3);
}

#[test]
fn markers_of_each_type_place_with_their_own_shapes() {
    let mut sim = house();
    for (i, k) in MarkerKind::ALL.into_iter().enumerate() {
        sim.app.cx.project.add_marker(
            0,
            Marker {
                kind: k,
                center: Point::new(60.0 * i as f64, 250.0),
                height_z: 96.0,
                ..Marker::default()
            },
        );
    }
    sim.app.cx.refresh();
    let m = &sim.app.cx.floor().annots.markers;
    assert_eq!(m.len(), 4);
    assert!(m[0].items.len() > m[2].items.len(), "a Level Line has a line and texts");
    // A marker has Move/Resize/Extend/Rotate-style handles.
    let h = m[0].items[0];
    sim.app.cx.selection.items = group(&sim, h);
    let hs = handles::handles_for(&sim.app.cx, sim.app.cx.px_per_in);
    assert_eq!(hs.len(), 3);
    assert!(matches!(sim.app.cx.floor().annot_of(h), Some(AnnotRef::Marker(0))));
}

#[test]
fn text_defaults_reach_new_text_and_the_defaults_dialogs_are_titled_with_the_saved_default() {
    use crate::dialogs::text::defaults::{open, DefaultsKind};
    let mut sim = house();
    sim.app.cx.project.annot_defaults.text.style = Some("Room Label Style".into());
    sim.tool(ToolId::TextVariant(TextMode::Text));
    sim.click(100.0, 100.0);
    sim.key(crate::tools::KeyEvent::text("Den"));
    sim.key(crate::tools::KeyEvent::key(eframe::egui::Key::Enter));
    let id = sim.app.cx.floor().cad.last().unwrap().id;
    assert_eq!(
        sim.app.cx.floor().cad_attrs(id).unwrap().text_style.as_deref(),
        Some("Room Label Style")
    );
    for k in DefaultsKind::ALL {
        let d = open(&sim.app.cx, k);
        assert_eq!(d.title(), format!("{} Defaults - Default", k.name()), "{k:?}");
        assert!(!d.tab_names().is_empty());
    }
}
