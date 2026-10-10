//! Lesson 12, Room Moldings (pp. 211-222). The audit marks the first step as a break, so
//! every step is an ignored replay (scenario-proposals.md, 'ignored').
use crate::scenarios::tutorials_support::*;

#[test]
#[ignore = "T7-12: R-117 (Floor Defaults molding table (replace base, add crown at 3 in))"]
fn step_r_117() {
    assert_ignored_break("R-117");
}

#[test]
#[ignore = "T7-12: R-106 (Exterior Room selected by Tab)"]
fn step_r_106() {
    assert_ignored_break("R-106");
}

#[test]
#[ignore = "T7-12: R-110 (Make Room Molding Polyline height 96 with the library profile)"]
fn step_r_110() {
    assert_ignored_break("R-110");
}

#[test]
#[ignore = "T7-12: S-170 (Remove / Add Molding to Selected Edge and Extension Snap)"]
fn step_s_170() {
    assert_ignored_break("S-170");
}
