//! Scenario 88 (round 16, brief 29): Framing Member Defaults, Framing Types,
//! Automatic and Manual Framing Defaults, and Structural Member Reporting
//! (manual pp. 880-912; CB-638 follow-ups, master-gaps 70 and 78). A built
//! house changes shape when a Framing Type is edited, a Default Framing
//! Member is applied to a hand-drawn member without touching its size, and
//! the reporting dialog's totals agree with the framing takeoff.

use super::{draw_shell, Sim};
use crate::dialogs::{framing_defaults, member_reporting};
use crate::editor::framing_view::{self, Record};
use crate::toolbar::{Action, FramingCommand};
use plan_core::geometry::Point;
use plan_framing::catalog::Role;
use plan_framing::{
    typed_takeoff, ManualMemberKind, Member, MemberKind, ReportMethod, ReportingDefault,
    SectionShape,
};

fn house() -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    sim.action(Action::Framing(FramingCommand::Build));
    sim
}

fn members(sim: &Sim) -> Vec<Member> {
    framing_view::members_for(&sim.app.cx.project, 0, true)
}

fn joists(sim: &Sim) -> Vec<Member> {
    members(sim)
        .into_iter()
        .filter(|m| m.kind == MemberKind::Joist)
        .collect()
}

fn steps(sim: &Sim) -> usize {
    sim.app.cx.action_history().0.len()
}

#[test]
fn editing_a_framing_type_to_i_joist_reshapes_the_floor_members_and_the_takeoff() {
    let mut sim = house();
    let before = members(&sim);
    assert!(!joists(&sim).is_empty(), "the floor was framed");
    assert!(joists(&sim).iter().all(|m| m.shape == SectionShape::Box));

    let mut cat = framing_defaults::catalog_of(&sim.app.cx);
    let name = cat.type_for_role(Role::of_member(MemberKind::Joist)).name;
    let plain = typed_takeoff(&before, &cat);
    assert!(
        !plain.cuts.iter().any(|c| c.member.contains(&name)),
        "{:?}",
        plain.cuts
    );
    // Edit the type: Wood, shape I-Joist, name shown in labels.
    let mut edited = cat.type_named(&name).unwrap().clone();
    edited.shape = plan_framing::catalog::FramingShape::IJoist;
    edited.include_name_in_labels = true;
    cat.edit_type(&name, edited).unwrap();
    let steps0 = steps(&sim);
    assert!(framing_defaults::set_catalog(
        &mut sim.app.cx,
        &cat,
        "Framing Types"
    ));
    assert_eq!(steps(&sim), steps0 + 1, "one edit is one undo step");

    assert!(
        joists(&sim).iter().all(|m| m.shape == SectionShape::IJoist),
        "every automatic joist follows the new shape"
    );
    let typed = typed_takeoff(&members(&sim), &cat);
    assert!(
        typed.cuts.iter().any(|c| c.member.contains(&name)),
        "the framing schedule names the type: {:?}",
        typed.cuts
    );
    // Lumber sizes and counts are untouched by a shape change.
    assert_eq!(members(&sim).len(), before.len());
    sim.undo();
    assert!(joists(&sim).iter().all(|m| m.shape == SectionShape::Box));
}

#[test]
fn a_default_member_applies_type_and_role_and_keeps_the_size() {
    let mut sim = house();
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
    let Some(Record::Manual(before)) = framing_view::find(sim.app.cx.floor(), id) else {
        panic!("the joist is stored");
    };
    let steps0 = steps(&sim);
    assert_eq!(
        framing_defaults::apply_default_to_selection(&mut sim.app.cx, "Joists - I Joists"),
        1
    );
    assert_eq!(steps(&sim), steps0 + 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Apply Framing Default Properties"));
    let Some(Record::Manual(after)) = framing_view::find(sim.app.cx.floor(), id) else {
        panic!("still stored");
    };
    assert_eq!(after.member_def, "Joists - I Joists");
    assert_ne!(after.framing_type, before.framing_type);
    assert_eq!(after.role, Some(Role::FloorJoist));
    assert_eq!(after.lumber, before.lumber, "a default has no size");
    assert_eq!((after.start, after.end), (before.start, before.end));
    sim.undo();
    let Some(Record::Manual(back)) = framing_view::find(sim.app.cx.floor(), id) else {
        panic!("still stored");
    };
    assert_eq!(back, before);
}

#[test]
fn the_reporting_dialog_totals_equal_the_takeoff_under_each_method() {
    let mut sim = house();
    let all = members(&sim);
    assert!(!all.is_empty());
    let takeoff = plan_framing::takeoff(&all);
    let pieces: u32 = takeoff.lines.iter().map(|(_, n)| *n).sum();
    let feet: f64 = takeoff.linear_feet_by_size.iter().map(|(_, f)| *f).sum();

    let mut cat = framing_defaults::catalog_of(&sim.app.cx);
    let buy = ReportingDefault::buy_list("Buy");
    let cut = buy.converted("Cuts", ReportMethod::CutList);
    let lin = buy.converted("Linear", ReportMethod::LinearLength);
    for d in [buy, cut, lin] {
        cat.reporting.defaults.push(d);
    }

    // Cut List: one piece per member, footage equal to the takeoff.
    assert!(cat.reporting.set_active("Cuts"));
    framing_defaults::set_catalog(&mut sim.app.cx, &cat, "Structural Member Reporting");
    let r = member_reporting::active_report(&sim.app.cx);
    assert!(
        (r.pieces() - f64::from(pieces)).abs() < 0.5,
        "cut pieces {} vs takeoff {pieces}",
        r.pieces()
    );
    assert!(
        (r.linear_feet() - feet).abs() < 0.5,
        "cut feet {} vs takeoff {feet}",
        r.linear_feet()
    );

    // Linear Length: the same footage.
    assert!(cat.reporting.set_active("Linear"));
    framing_defaults::set_catalog(&mut sim.app.cx, &cat, "Structural Member Reporting");
    let r = member_reporting::active_report(&sim.app.cx);
    assert!((r.linear_feet() - feet).abs() < 0.5, "{}", r.linear_feet());

    // Buy List: boards, never more than the cut pieces, never less footage.
    assert!(cat.reporting.set_active("Buy"));
    framing_defaults::set_catalog(&mut sim.app.cx, &cat, "Structural Member Reporting");
    let r = member_reporting::active_report(&sim.app.cx);
    assert!(r.pieces() <= f64::from(pieces) + 0.5, "{}", r.pieces());
    assert!(r.linear_feet() + 0.5 >= feet, "{} vs {feet}", r.linear_feet());
}

#[test]
fn the_framing_dialogs_open_from_their_commands() {
    let mut sim = house();
    for id in [
        framing_defaults::MEMBERS,
        framing_defaults::AUTOMATIC,
        framing_defaults::MANUAL,
    ] {
        sim.action(Action::Custom(id));
        sim.dialog_frame(false);
        assert!(framing_defaults::is_open(), "{id} opened its dialog");
        sim.dialog_frame_key(Some(eframe::egui::Key::Escape));
        sim.dialog_frame(false);
        assert!(!framing_defaults::is_open(), "{id} closed on Cancel");
    }
    // Cancel changed nothing: no undo step was added by opening and closing.
    let steps0 = steps(&sim);
    sim.action(Action::Custom(framing_defaults::TYPES));
    sim.dialog_frame(false);
    sim.action(Action::Custom(framing_defaults::REPORTING));
    sim.dialog_frame(false);
    assert_eq!(steps(&sim), steps0);
}
