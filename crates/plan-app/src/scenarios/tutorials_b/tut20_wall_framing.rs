//! Lesson 20, Wall Framing (pp. 349-364). Real: Build Framing for the walls
//! of floor 1 is one undo step. Open: layer spacing from the wall type,
//! stagger blocking, the secondary dimension format.
use super::support_b::*;
use crate::scenarios::tutorials_support::*;
use plan_framing::{FloorPick, GroupFlags, MemberKind};

#[test]
fn building_wall_framing_adds_studs_in_one_undo_step() {
    let mut sim = cottage();
    change(&mut sim, |st| {
        st.build.build = GroupFlags {
            wall: true,
            ..GroupFlags::NONE
        };
        st.build.floor_pick = FloorPick::Floor(0);
    });
    let before = sim.app.cx.undo_depth();
    build_dialog_ok(&mut sim, false);
    assert_eq!(sim.app.cx.undo_depth(), before + 1);
    assert!(count(&sim, 0, MemberKind::Stud) > 10);
    assert_eq!(count(&sim, 0, MemberKind::Joist), 0, "walls only");
    sim.undo();
    assert_eq!(count(&sim, 0, MemberKind::Stud), 0);
}

#[test]
#[ignore = "T7-20: W-138 (stud spacing 24 OC from the wall type layer; basement walls rebuild at 24)"]
fn spacing_from_wall_type_layer() {
    assert_ignored_break("W-138");
}

#[test]
#[ignore = "T7-20: CB-645 (Stagger Blocking option in the wall build)"]
fn stagger_blocking() {
    assert_ignored_break("CB-645");
}

#[test]
#[ignore = "T7-20: DIM-46 (secondary dimension format in the Wall Detail)"]
fn secondary_dimension_format() {
    assert_ignored_break("DIM-46");
}

#[test]
#[ignore = "T7-20: DS-32 (Default Set for Wall Details saved with the detail dimensions)"]
fn wall_detail_default_set() {
    assert_ignored_break("DS-32");
}
