//! Lesson 10, Custom Ceilings (pp. 177-190). The audit marks the first step as a break, so
//! every step is an ignored replay (scenario-proposals.md, 'ignored').
use crate::scenarios::tutorials_support::*;

#[test]
#[ignore = "T7-10: R-122 (Ceiling Finish Definition with layers (drywall + paint, hat channel at 24 OC))"]
fn step_r_122() {
    assert_ignored_break("R-122");
}

#[test]
#[ignore = "T7-10: R-146 (cathedral deck ceiling by clearing Flat Ceiling Over This Room)"]
fn step_r_146() {
    assert_ignored_break("R-146");
}

#[test]
#[ignore = "T7-10: CB-210 (hat-channel framing type and member)"]
fn step_cb_210() {
    assert_ignored_break("CB-210");
}

#[test]
#[ignore = "T7-10: R-111 (coffered soffits and the soffit ring)"]
fn step_r_111() {
    assert_ignored_break("R-111");
}
