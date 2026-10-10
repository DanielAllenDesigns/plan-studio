//! Lesson 13, Interior Furnishings (pp. 222-236). Ignored at the first
//! block step (scenario-proposals.md); placing loose furniture is covered by
//! the library scenarios (s52).
use crate::scenarios::tutorials_support::*;

#[test]
#[ignore = "T7-13: CB-427 (Make Architectural Block of dresser + vase + frame)"]
fn make_architectural_block() {
    assert_ignored_break("CB-427");
}

#[test]
#[ignore = "T7-13: CB-436 (block moves as one, Explode, Add to Library)"]
fn block_explode_and_add_to_library() {
    assert_ignored_break("CB-436");
}

#[test]
#[ignore = "T7-13: S-118 (Find Object in Plan from the Furniture Schedule)"]
fn find_object_in_plan() {
    assert_ignored_break("S-118");
}

#[test]
#[ignore = "T7-13: S-195 (Replace From Library on identical tables)"]
fn replace_from_library() {
    assert_ignored_break("S-195");
}

#[test]
#[ignore = "T7-13: L-234 (Furniture Schedule with 3D Perspective, block listed as Dining Set)"]
fn furniture_schedule_with_blocks() {
    assert_ignored_break("L-234");
}
