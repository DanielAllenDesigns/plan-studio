//! Lesson 19, Floor Framing (pp. 331-349). Real: Build Framing for floor 2
//! is one undo step and leaves floor 1 alone. Open: Raise/Lower beams and
//! the Trim Object(s) Sticky Mode.
use super::support_b::*;
use crate::scenarios::tutorials_support::*;
use plan_framing::{FloorPick, GroupFlags, MemberKind};

#[test]
fn building_floor_framing_for_floor_two_is_one_undo_step() {
    let mut sim = two_floor_cottage();
    change(&mut sim, |st| {
        st.build.build = GroupFlags {
            floor: true,
            ..GroupFlags::NONE
        };
        st.build.floor_pick = FloorPick::Floor(1);
    });
    let before = sim.app.cx.undo_depth();
    build_dialog_ok(&mut sim, false);
    assert_eq!(sim.app.cx.undo_depth(), before + 1);
    assert!(count(&sim, 1, MemberKind::Joist) > 5, "floor 2 has joists");
    assert_eq!(count(&sim, 0, MemberKind::Joist), 0, "floor 1 is untouched");
    sim.undo();
    assert_eq!(count(&sim, 1, MemberKind::Joist), 0);
}

#[test]
#[ignore = "T7-19: CB-283 (beam under the joists: Raise/Lower -11 7/8 in)"]
fn beam_raise_lower() {
    assert_ignored_break("CB-283");
}

#[test]
#[ignore = "T7-19: S-137 (Trim Object(s) with Sticky Mode on joist ends)"]
fn trim_objects_sticky() {
    assert_ignored_break("S-137");
}

#[test]
#[ignore = "T7-19: CB-638 (Bearing Wall flag turns the floor 2 joists toward the wall, ceiling joists unchanged)"]
fn bearing_wall_joist_direction() {
    assert_ignored_break("CB-638");
}
