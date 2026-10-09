//! Scenario 89 (round 16, brief 30): Build Framing as a command with Build
//! Framing Once for one floor, Build Framing for Selected and Parent Objects,
//! Retain Wall Framing, Wall Details as documents, Framing Groups, Bearing
//! Walls and Beams, the Framing Reference, joins and breaks, and the truss
//! labels, Truss Detail and truss schedule (CB-256, CB-261, CB-638..CB-645,
//! W-134).

use super::{draw_shell, Sim};
use crate::dialogs::{defaults, framing as framing_dialog};
use crate::editor::framing_view::{self, cmd, FramingSettings, Record, Target};
use crate::editor::{EditActionKind, ObjectRef};
use crate::shell::docks::{browser_nodes, BrowserNode};
use crate::toolbar::{Action, FramingCommand};
use plan_core::geometry::Point;
use plan_core::{Id, OpeningKind};
use plan_framing::{
    BearingMode, FloorPick, GroupFlags, ManualMemberKind, Member, MemberKind, ReferenceMarker,
    Splice, TrussSpec, TrussType,
};

const W: f64 = 480.0;
const H: f64 = 360.0;

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    let first = sim.wall_ids()[0];
    sim.app
        .cx
        .project
        .add_opening(0, first, 200.0, OpeningKind::Door)
        .expect("a door fits");
    sim.app.cx.refresh();
    sim
}

fn two_floors() -> Sim {
    let mut sim = house();
    sim.app.cx.project.build_new_floor(true);
    sim.app.cx.refresh();
    sim
}

fn change(sim: &mut Sim, edit: impl FnOnce(&mut FramingSettings)) {
    let mut st = framing_view::settings(&sim.app.cx.project);
    edit(&mut st);
    framing_view::set_settings(sim.cx(), st);
}

fn build_dialog_ok(sim: &mut Sim, all_floors: bool) {
    framing_dialog::request_build(all_floors);
    sim.action(Action::Custom(defaults::FRAMING));
    sim.dialog_frame(false);
    sim.dialog_frame(false);
    sim.dialog_frame(true);
    sim.dialog_frame(false);
}

fn count(sim: &Sim, floor: usize, k: MemberKind) -> usize {
    framing_view::load(&sim.app.cx.project.floors[floor])
        .iter()
        .filter(|m| m.kind == k)
        .count()
}

fn steps(sim: &Sim) -> usize {
    sim.app.cx.action_history().0.len()
}

fn members_of(sim: &Sim, floor: usize, wall: Id) -> Vec<Member> {
    framing_view::load(&sim.app.cx.project.floors[floor])
        .into_iter()
        .filter(|m| m.wall_id == Some(wall))
        .collect()
}

fn labels(sim: &Sim) -> Vec<&'static str> {
    sim.app
        .cx
        .extra_edit_actions()
        .iter()
        .filter_map(|a| match a.kind {
            EditActionKind::Custom { label, id, .. } if id.starts_with("framing.") => Some(label),
            _ => None,
        })
        .collect()
}

// ----- item 1: the command and its semantics -----

#[test]
fn build_framing_once_builds_the_floor_framing_of_floor_two_only() {
    let mut sim = two_floors();
    assert!(sim.app.cx.project.floors.len() >= 2);
    change(&mut sim, |st| {
        st.build.build = GroupFlags {
            floor: true,
            ..GroupFlags::NONE
        };
        st.build.floor_pick = FloorPick::Floor(1);
    });
    let before = steps(&sim);
    build_dialog_ok(&mut sim, false);
    assert!(count(&sim, 1, MemberKind::Joist) > 5, "floor 2 has joists");
    assert_eq!(count(&sim, 0, MemberKind::Joist), 0, "floor 1 was not built");
    assert_eq!(count(&sim, 1, MemberKind::Stud), 0, "the walls were not asked for");
    assert_eq!(steps(&sim), before + 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Build Framing"));
    sim.undo();
    assert_eq!(count(&sim, 1, MemberKind::Joist), 0);
}

#[test]
fn the_build_framing_command_page_has_the_defaults_button_and_the_floor_choices() {
    let mut sim = two_floors();
    framing_dialog::request_build(false);
    let st = framing_view::settings(&sim.app.cx.project);
    assert_eq!(st.floor_names.len(), 2);
    let d = framing_dialog::FramingDefaultsDialog::new(&st);
    assert!(d.is_build() && !d.in_defaults());
    // Plan Studio keeps the menu's choice when no floor is picked.
    assert_eq!(st.build.floor_pick, FloorPick::Same);
    drop(d);
    sim.app.cx.refresh();
}

#[test]
fn build_framing_for_selected_rebuilds_a_wall_and_a_roof_plane_apart() {
    let mut sim = house();
    sim.action(Action::Framing(FramingCommand::Build));
    let wall = sim.wall_ids()[1];
    let studs0 = count(&sim, 0, MemberKind::Stud);
    let joists0 = count(&sim, 0, MemberKind::Joist);
    // Delete one wall stud by hand, then rebuild just that wall.
    {
        let cx = sim.cx();
        let mut all = framing_view::load(&cx.project.floors[0]);
        let i = all
            .iter()
            .position(|m| m.wall_id == Some(wall) && m.kind == MemberKind::Stud)
            .unwrap();
        all.remove(i);
        let mut values: Vec<serde_json::Value> = all
            .iter()
            .filter_map(|m| serde_json::to_value(m).ok())
            .collect();
        values.extend(
            cx.project.floors[0]
                .framing
                .iter()
                .filter(|v| serde_json::from_value::<plan_framing::Member>((*v).clone()).is_err())
                .cloned(),
        );
        cx.project.floors[0].framing = values;
    }
    assert_eq!(count(&sim, 0, MemberKind::Stud), studs0 - 1);
    sim.app.cx.selection.set(ObjectRef::Wall(wall));
    let l = labels(&sim);
    assert!(l.contains(&"Build Framing for Selected Object(s)"), "{l:?}");
    assert!(l.contains(&"Open Wall Detail"));
    let before = steps(&sim);
    sim.app.cx.run_custom(cmd::BUILD_SELECTED);
    assert_eq!(count(&sim, 0, MemberKind::Stud), studs0, "the wall is whole again");
    assert_eq!(count(&sim, 0, MemberKind::Joist), joists0, "the floor was left alone");
    assert_eq!(steps(&sim), before + 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Build Framing for Selected Object(s)"));
    sim.undo();
    assert_eq!(count(&sim, 0, MemberKind::Stud), studs0 - 1);
    // A retained wall refuses the button.
    framing_view::set_walls_retained(sim.cx(), &[wall], true);
    sim.app.cx.selection.set(ObjectRef::Wall(wall));
    let acts = sim.app.cx.extra_edit_actions();
    let b = acts
        .iter()
        .find(|a| a.label == "Build Framing for Selected Object(s)")
        .unwrap();
    assert!(!b.enabled);
    assert!(matches!(
        framing_view::build_targets(sim.cx(), &[Target::Wall(wall)], None),
        framing_view::Outcome::Refused(_)
    ));
}

#[test]
fn a_roof_plane_builds_its_own_framing() {
    let mut sim = house();
    sim.tool(crate::tools::ToolId::RoofVariant(crate::tools::roof::RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    sim.tool(crate::tools::ToolId::Select);
    sim.app.cx.selection.clear();
    sim.action(Action::Framing(FramingCommand::Build));
    let rafters = count(&sim, 0, MemberKind::Rafter);
    assert!(rafters > 20, "{rafters}");
    let studs = count(&sim, 0, MemberKind::Stud);
    let plane = crate::editor::roof_view::load(sim.app.cx.floor()).planes[0].id;
    sim.app.cx.selection.set(ObjectRef::RoofPlane(plane));
    assert!(labels(&sim).contains(&"Build Framing for Selected Object(s)"));
    sim.app.cx.run_custom(cmd::BUILD_SELECTED);
    // The whole roof is as it was: the plane's members were replaced by the same ones.
    assert_eq!(count(&sim, 0, MemberKind::Rafter), rafters);
    assert_eq!(count(&sim, 0, MemberKind::Stud), studs);
    // Retaining the plane stops the button.
    framing_view::set_planes_retained(sim.cx(), &[plane], true);
    assert!(matches!(
        framing_view::build_targets(sim.cx(), &[Target::RoofPlane(plane)], None),
        framing_view::Outcome::Refused(_)
    ));
}

#[test]
fn retain_wall_framing_protects_a_member_edited_in_the_wall_detail() {
    let mut sim = house();
    sim.action(Action::Framing(FramingCommand::Build));
    let wall = sim.wall_ids()[1];
    // The Wall Details exist once the framing is built, named from the wall labels.
    let details = framing_view::wall_details(&sim.app.cx.project);
    assert_eq!(details.len(), 4);
    let (di, _, w, label) = details.iter().find(|d| d.2 == wall).cloned().unwrap();
    assert_eq!(label, "W2");
    assert_eq!(sim.app.cx.project.floors[di].name, "W2");
    // Open it, select a stud's box and turn it flat to the outside.
    assert!(framing_view::open_wall_detail(sim.cx(), w));
    assert_eq!(sim.app.cx.floor, di);
    let map = framing_view::load_map(sim.app.cx.floor());
    let members = members_of(&sim, 0, wall);
    let all = framing_view::load(&sim.app.cx.project.floors[0]);
    let stud_index = all
        .iter()
        .position(|m| m.wall_id == Some(wall) && m.kind == MemberKind::Stud)
        .unwrap();
    let (cad, _) = map
        .members
        .iter()
        .find(|(_, i)| *i == stud_index)
        .copied()
        .expect("every member has its box");
    sim.app.cx.selection.set(ObjectRef::Cad(cad));
    let l = labels(&sim);
    for want in [
        "Build Framing for Parent Object(s)",
        "Find Wall",
        "Flat to Inside",
        "Flat to Outside",
    ] {
        assert!(l.contains(&want), "{want} in {l:?}");
    }
    let before = steps(&sim);
    sim.app.cx.run_custom(cmd::FLAT_OUTSIDE);
    assert_eq!(steps(&sim), before + 1, "one undo step");
    let flat = |sim: &Sim| {
        members_of(sim, 0, wall)
            .iter()
            .filter(|m| m.label.contains("flat to outside"))
            .count()
    };
    assert_eq!(flat(&sim), 1);
    assert_eq!(members_of(&sim, 0, wall).len(), members.len());
    // Back to the plan: with Retain on, a rebuild keeps the edit ...
    sim.app.cx.floor = 0;
    sim.app.cx.reset_view_state();
    framing_view::set_walls_retained(sim.cx(), &[wall], true);
    sim.action(Action::Framing(FramingCommand::Build));
    assert_eq!(flat(&sim), 1, "Retain Wall Framing kept the edit");
    // ... and with Retain off the rebuild makes it again.
    framing_view::set_walls_retained(sim.cx(), &[wall], false);
    sim.action(Action::Framing(FramingCommand::Build));
    assert_eq!(flat(&sim), 0, "the rebuild replaced the edited member");
}

#[test]
fn the_wall_detail_opens_from_the_edit_button_and_is_listed_in_the_project_browser() {
    let mut sim = house();
    let wall = sim.wall_ids()[0];
    sim.app.cx.selection.set(ObjectRef::Wall(wall));
    // Before the build there is no detail to open.
    let open = sim
        .app
        .cx
        .extra_edit_actions()
        .into_iter()
        .find(|a| a.label == "Open Wall Detail")
        .unwrap();
    assert!(!open.enabled);
    sim.action(Action::Framing(FramingCommand::Build));
    sim.app.cx.selection.set(ObjectRef::Wall(wall));
    sim.app.cx.run_custom(cmd::OPEN_WALL_DETAIL);
    assert!(framing_view::in_wall_detail(&sim.app.cx));
    assert_eq!(framing_view::detail_wall(sim.app.cx.floor()), Some(wall));
    let nodes = browser_nodes(&sim.app.cx);
    let rows = &nodes
        .iter()
        .find(|(n, _)| *n == BrowserNode::WallDetails)
        .unwrap()
        .1;
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().any(|r| r.label == "W1"));
    // The details are not listed again among the CAD details.
    let cad = &nodes
        .iter()
        .find(|(n, _)| *n == BrowserNode::CadDetails)
        .unwrap()
        .1;
    assert!(cad.is_empty());
    // The drawing carries the members as boxes, the dimensions and a title.
    let f = sim.app.cx.floor();
    assert!(f.cad.len() > 40);
    assert!(f
        .cad
        .iter()
        .any(|o| matches!(&o.item, plan_core::cad::CadItem::Text { text, .. } if text.contains("seen from the exterior"))));
    // Annotating survives a rebuild of the framing.
    let note = sim.app.cx.project.alloc_id();
    sim.app.cx.floor_mut().cad.push(plan_core::cad::CadObject {
        id: note,
        layer: plan_core::cad::DEFAULT_CAD_LAYER.into(),
        item: plan_core::cad::CadItem::Text {
            pos: Point::new(10.0, -40.0),
            text: "Note".into(),
            height: 3.0,
            angle: 0.0,
        },
    });
    sim.app.cx.floor = 0;
    sim.app.cx.reset_view_state();
    sim.action(Action::Framing(FramingCommand::Build));
    let di = framing_view::wall_detail_floor(&sim.app.cx.project, wall).unwrap();
    assert!(sim.app.cx.project.floors[di].cad.iter().any(|o| o.id == note));
    // Deleting a member from the detail takes it out of the wall's framing.
    framing_view::open_wall_detail(sim.cx(), wall);
    let map = framing_view::load_map(sim.app.cx.floor());
    let all = framing_view::load(&sim.app.cx.project.floors[0]);
    let n = all.iter().filter(|m| m.wall_id == Some(wall)).count();
    sim.app.cx.selection.set(ObjectRef::Cad(map.members[0].0));
    sim.app.cx.run_custom(cmd::WALL_MEMBER_DELETE);
    assert_eq!(members_of(&sim, 0, wall).len(), n - 1);
}

#[test]
fn a_framing_group_separates_platforms_and_the_question_is_asked() {
    let mut sim = house();
    // A partition splits the house in two rooms; only the exterior walls bear.
    sim.tool(crate::tools::ToolId::Wall {
        kind: plan_core::WallKind::Interior,
    });
    sim.drag((240.0, 0.0), (240.0, H));
    sim.app.cx.refresh();
    change(&mut sim, |st| {
        st.walls.bearing = BearingMode::ExteriorAndBearingLines;
        st.build.build = GroupFlags {
            floor: true,
            ..GroupFlags::NONE
        };
    });
    sim.action(Action::Framing(FramingCommand::Build));
    let one_platform = count(&sim, 0, MemberKind::Joist);
    assert!(one_platform > 10);
    // Joists cross the partition: no joist ends at x = 240 on both sides.
    let ends_at_partition = framing_view::load(sim.app.cx.floor())
        .iter()
        .filter(|m| m.kind == MemberKind::Joist)
        .filter(|m| {
            let a = m.transform.origin[0];
            let b = a + m.transform.axis_x[0] * m.length;
            (a - 240.0).abs() < 3.0 || (b - 240.0).abs() < 3.0
        })
        .count();
    assert_eq!(ends_at_partition, 0);
    // Selecting a room next to a room of its own group asks the question.
    let room = sim.app.cx.rooms.iter().position(|r| r.centroid.x < 240.0).unwrap();
    sim.app.cx.selection.set(ObjectRef::Room(room));
    let before = steps(&sim);
    let out = framing_view::build_selected(sim.cx(), None);
    assert!(matches!(out, framing_view::Outcome::AskGroup { .. }), "{out:?}");
    assert_eq!(steps(&sim), before, "asking changes nothing");
    // Yes: the room becomes a group of its own and the platforms are separate.
    let out = framing_view::build_selected(sim.cx(), Some(true));
    assert!(matches!(out, framing_view::Outcome::Built(n) if n > 0), "{out:?}");
    let names = &sim.app.cx.project.floors[0].room_names;
    assert!(names.iter().any(|n| n.options.framing_group == 1));
    sim.action(Action::Framing(FramingCommand::Build));
    // Joists now stop at the partition (the platforms are separate).
    let ends_at_partition = framing_view::load(sim.app.cx.floor())
        .iter()
        .filter(|m| m.kind == MemberKind::Joist)
        .filter(|m| {
            let a = m.transform.origin[0];
            let b = a + m.transform.axis_x[0] * m.length;
            (a - 240.0).abs() < 8.0 || (b - 240.0).abs() < 8.0
        })
        .count();
    assert!(ends_at_partition > 5, "{ends_at_partition}");
}

#[test]
fn a_bearing_wall_and_a_bearing_beam_lap_or_butt_the_joists_over_them() {
    let mut sim = house();
    sim.tool(crate::tools::ToolId::Wall {
        kind: plan_core::WallKind::Interior,
    });
    sim.drag((240.0, 0.0), (240.0, H));
    sim.app.cx.refresh();
    let partition = *sim.wall_ids().last().unwrap();
    change(&mut sim, |st| {
        st.walls.bearing = BearingMode::ExteriorAndBearingLines;
        st.build.build = GroupFlags {
            floor: true,
            ..GroupFlags::NONE
        };
        st.build.detail.splice = Splice::Butt;
    });
    // Not marked bearing: the joists run the whole way across.
    sim.action(Action::Framing(FramingCommand::Build));
    let plain = framing_view::manual_members(sim.app.cx.floor())
        .iter()
        .filter(|m| m.kind == ManualMemberKind::Joist)
        .count();
    // Mark it as a Bearing Wall: the joists break over it, butted.
    for w in &mut sim.app.cx.project.floors[0].walls {
        if w.id == partition {
            w.spec.structure.bearing_wall = true;
        }
    }
    sim.action(Action::Framing(FramingCommand::Build));
    let butt: Vec<_> = framing_view::manual_members(sim.app.cx.floor())
        .into_iter()
        .filter(|m| m.kind == ManualMemberKind::Joist)
        .collect();
    assert!(butt.len() > plain, "{} > {plain}", butt.len());
    let total_butt: f64 = butt.iter().map(|m| m.plan_length()).sum();
    // Lapped: the same joists but 8" longer in each line.
    change(&mut sim, |st| st.build.detail.splice = Splice::Lap);
    sim.action(Action::Framing(FramingCommand::Build));
    let lap: Vec<_> = framing_view::manual_members(sim.app.cx.floor())
        .into_iter()
        .filter(|m| m.kind == ManualMemberKind::Joist)
        .collect();
    let total_lap: f64 = lap.iter().map(|m| m.plan_length()).sum();
    assert_eq!(lap.len(), butt.len());
    assert!(total_lap > total_butt + 8.0 * (lap.len() as f64 / 2.0 - 1.0), "{total_lap} {total_butt}");
}

#[test]
fn the_framing_reference_marker_anchors_a_floor_and_move_to_framing_ref_snaps_to_it() {
    let mut sim = house();
    change(&mut sim, |st| st.build.build = GroupFlags { floor: true, ..GroupFlags::NONE });
    framing_view::add_record(sim.cx(), "Marker", |id| Record::Marker {
        id,
        marker: ReferenceMarker {
            point: Point::new(100.0, 100.0),
            angle: 0.0,
        },
    });
    sim.action(Action::Framing(FramingCommand::Build));
    // Joists run along y: their x positions sit on the marker's grid (x = 100 + 16k).
    let on_grid = |sim: &Sim| {
        framing_view::load(sim.app.cx.floor())
            .iter()
            .filter(|m| m.kind == MemberKind::Joist)
            .filter(|m| ((m.transform.origin[0] - 100.0).rem_euclid(16.0)).min(16.0 - (m.transform.origin[0] - 100.0).rem_euclid(16.0)) < 0.01)
            .count()
    };
    assert!(on_grid(&sim) > 20, "{}", on_grid(&sim));
    // Switching Use Framing Reference off for the floor puts it back to the plain layout.
    change(&mut sim, |st| st.build.set_reference(0, false));
    sim.action(Action::Framing(FramingCommand::Build));
    assert!(on_grid(&sim) < 5, "{}", on_grid(&sim));
    // Move to Framing Ref: a hand-drawn joist 5 1/2" off the grid moves onto it.
    let m = framing_view::new_member(
        sim.app.cx.floor(),
        ManualMemberKind::Joist,
        Point::new(105.5, 20.0),
        Point::new(105.5, 200.0),
    );
    let id = framing_view::add_record(sim.cx(), "Joist", |id| {
        Record::Manual(plan_framing::FramingMember { id, ..m })
    });
    framing_view::select(sim.cx(), vec![id]);
    assert!(labels(&sim).contains(&"Move to Framing Ref"));
    let before = steps(&sim);
    sim.app.cx.run_custom(cmd::MOVE_TO_REF);
    assert_eq!(steps(&sim), before + 1);
    let Some(Record::Manual(j)) = framing_view::find(sim.app.cx.floor(), id) else {
        panic!("gone");
    };
    assert!((j.start.x - 100.0).abs() < 1e-6 || (j.start.x - 100.0).rem_euclid(16.0).min(16.0 - (j.start.x - 100.0).rem_euclid(16.0)) < 1e-6, "{}", j.start.x);
}

#[test]
fn add_break_and_the_joins_change_manual_members() {
    let mut sim = house();
    let m = framing_view::new_member(
        sim.app.cx.floor(),
        ManualMemberKind::GeneralFraming,
        Point::new(20.0, 20.0),
        Point::new(220.0, 20.0),
    );
    let id = framing_view::add_record(sim.cx(), "Place", |id| {
        Record::Manual(plan_framing::FramingMember { id, ..m })
    });
    framing_view::select(sim.cx(), vec![id]);
    assert!(labels(&sim).contains(&"Add Break"));
    let before = steps(&sim);
    sim.app.cx.run_custom(cmd::ADD_BREAK);
    assert_eq!(steps(&sim), before + 1);
    let members = framing_view::manual_members(sim.app.cx.floor());
    assert_eq!(members.len(), 2);
    assert!((members[0].plan_length() - 100.0).abs() < 1e-6);
    // Two crossing members: Join and Lap Ends, then Mitre.
    let a = framing_view::new_member(
        sim.app.cx.floor(),
        ManualMemberKind::GeneralFraming,
        Point::new(300.0, 100.0),
        Point::new(395.0, 100.0),
    );
    let b = framing_view::new_member(
        sim.app.cx.floor(),
        ManualMemberKind::GeneralFraming,
        Point::new(400.0, 50.0),
        Point::new(400.0, 200.0),
    );
    let ia = framing_view::add_record(sim.cx(), "A", |id| {
        Record::Manual(plan_framing::FramingMember { id, ..a })
    });
    let ib = framing_view::add_record(sim.cx(), "B", |id| {
        Record::Manual(plan_framing::FramingMember { id, ..b })
    });
    framing_view::select(sim.cx(), vec![ia, ib]);
    let l = labels(&sim);
    assert!(l.contains(&"Join and Lap Ends") && l.contains(&"Join and Mitre Ends"));
    sim.app.cx.run_custom(cmd::JOIN_LAP);
    let Some(Record::Manual(a2)) = framing_view::find(sim.app.cx.floor(), ia) else {
        panic!()
    };
    assert!(a2.end.x < 400.0 && a2.end.x > 396.0);
    assert_eq!(a2.joint[1], plan_framing::JoinKind::Butt);
    sim.app.cx.run_custom(cmd::JOIN_MITRE);
    let Some(Record::Manual(a3)) = framing_view::find(sim.app.cx.floor(), ia) else {
        panic!()
    };
    assert!((a3.end.x - 400.0).abs() < 1e-6);
    assert_eq!(a3.joint[1], plan_framing::JoinKind::Mitre);
}

// ----- item 4: trusses -----

fn truss(sim: &mut Sim, y: f64, span: f64) -> Id {
    let mut m = framing_view::new_member(
        sim.app.cx.floor(),
        ManualMemberKind::RoofTruss,
        Point::new(0.0, y),
        Point::new(span, y),
    );
    m.truss = Some(TrussSpec::new(TrussType::Fink, span, 6.0));
    framing_view::add_record(sim.cx(), "Roof Truss", |id| {
        Record::Manual(plan_framing::FramingMember { id, ..m })
    })
}

#[test]
fn identical_trusses_share_a_label_the_truss_detail_draws_each_once_and_the_schedule_counts_them() {
    let mut sim = house();
    let a = truss(&mut sim, 40.0, 288.0);
    truss(&mut sim, 64.0, 288.0);
    truss(&mut sim, 88.0, 288.0);
    let d = truss(&mut sim, 112.0, 240.0);
    let labels_now = framing_view::truss_labels(&sim.app.cx.project);
    let label = |id: Id| labels_now.iter().find(|(i, _)| *i == id).map(|(_, l)| l.clone());
    assert_eq!(label(a).as_deref(), Some("TR-1"));
    assert_eq!(label(d).as_deref(), Some("TR-2"));
    assert_eq!(labels_now.iter().filter(|(_, l)| l == "TR-1").count(), 3);
    // The Truss Detail exists and has one diagram per configuration, with the quantity.
    let td = framing_view::truss_detail_floor(&sim.app.cx.project).expect("a Truss Detail");
    let f = &sim.app.cx.project.floors[td];
    let texts: Vec<String> = f
        .cad
        .iter()
        .filter_map(|o| match &o.item {
            plan_core::cad::CadItem::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"TR-1 (3)".to_string()), "{texts:?}");
    assert!(texts.contains(&"TR-2".to_string()));
    // Open Truss Detail from a selected truss, Find Trusses from the detail.
    framing_view::select(sim.cx(), vec![a]);
    assert!(labels(&sim).contains(&"Open Truss Detail"));
    sim.app.cx.run_custom(cmd::OPEN_TRUSS_DETAIL);
    assert!(framing_view::in_truss_detail(&sim.app.cx));
    assert!(labels(&sim).contains(&"Find Trusses"));
    sim.app.cx.run_custom(cmd::FIND_TRUSSES);
    assert_eq!(sim.app.cx.floor, 0);
    assert_eq!(framing_view::selected(&sim.app.cx).len(), 3);
    // The schedule rows.
    let rows = plan_framing::truss_schedule(&framing_view::all_manual_in_plan(&sim.app.cx.project), &[]);
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].label.as_str(), rows[0].quantity), ("TR-1", 3));
    // Deleting the trusses removes the Truss Detail with them.
    let selected = framing_view::selected(&sim.app.cx);
    framing_view::delete_records(sim.cx(), &selected);
    let ids: Vec<Id> = framing_view::manual_members(sim.app.cx.floor())
        .iter()
        .map(|m| m.id)
        .collect();
    framing_view::delete_records(sim.cx(), &ids);
    assert!(framing_view::truss_detail_floor(&sim.app.cx.project).is_none());
}

#[test]
fn the_trusses_of_a_gable_roof_share_one_label_and_a_locked_truss_keeps_its_shape() {
    let mut sim = house();
    sim.tool(crate::tools::ToolId::RoofVariant(crate::tools::roof::RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    sim.tool(crate::tools::ToolId::Select);
    let east = sim
        .app
        .cx
        .floor()
        .walls
        .iter()
        .max_by(|a, b| (a.start.x + a.end.x).total_cmp(&(b.start.x + b.end.x)))
        .unwrap()
        .id;
    sim.app.cx.selection.set(ObjectRef::Wall(east));
    sim.app.cx.run_custom("roof.wall.gable");
    change(&mut sim, |st| {
        st.roof.trusses = true;
        st.build.build = GroupFlags {
            roof: true,
            ..GroupFlags::NONE
        };
    });
    sim.action(Action::Framing(FramingCommand::Build));
    let members = framing_view::load(sim.app.cx.floor());
    let n = members
        .iter()
        .filter(|m| m.kind == MemberKind::TrussBottomChord)
        .count();
    assert!(n > 5);
    let configs = framing_view::truss_configs_of(&sim.app.cx.project);
    assert_eq!(configs.len(), 1);
    assert_eq!((configs[0].label.as_str(), configs[0].count), ("TR-1", n));
    // Moving a roof truss under a different roof plane gives it that pitch unless locked.
    let a = truss(&mut sim, 60.0, 200.0);
    let b = truss(&mut sim, 90.0, 200.0);
    for id in [a, b] {
        let Some(Record::Manual(mut m)) = framing_view::find(sim.app.cx.floor(), id) else {
            panic!()
        };
        m.truss.as_mut().unwrap().pitch = 3.0;
        if id == b {
            m.truss.as_mut().unwrap().locked = true;
        }
        framing_view::apply_edit(sim.cx(), m);
    }
    framing_view::move_records(sim.cx(), &[a, b], Point::new(0.0, 10.0));
    let pitch = |sim: &Sim, id: Id| match framing_view::find(sim.app.cx.floor(), id) {
        Some(Record::Manual(m)) => m.truss.unwrap().pitch,
        _ => panic!(),
    };
    assert!(pitch(&sim, a) != 3.0, "the roof truss took the roof's pitch");
    assert_eq!(pitch(&sim, b), 3.0, "the locked one kept its shape");
}
