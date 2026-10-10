//! Lesson 6, Decks and Porches (pp. 105-126).
use crate::scenarios::tutorials_support::*;

#[test]
fn the_cottage_has_no_deck_room_yet() {
    // The deck and porch are built from railing and half-wall flyouts; the
    // shell they attach to must stay one living room until then.
    let sim = cottage();
    assert_eq!(sim.app.cx.rooms.len(), 1);
}

#[test]
#[ignore = "T7-06: CB-627 (deck post footings stop at the terrain)"]
fn deck_footings_against_terrain() {
    assert_ignored_break("CB-627");
}

#[test]
#[ignore = "T7-06: CB-530 (Absolute Elevation -28 with a Reference Point)"]
fn terrain_perimeter_absolute_elevation() {
    assert_ignored_break("CB-530");
}

#[test]
#[ignore = "T7-06: W-145 (porch stem walls: Stem Wall Height 37 1/2, Floor Under off)"]
fn porch_stem_walls() {
    assert_ignored_break("W-145");
}

#[test]
#[ignore = "T7-06: CB-109 (one-click deck stairs to grade with a DN label)"]
fn deck_stairs_to_grade() {
    assert_ignored_break("CB-109");
}

#[test]
#[ignore = "T7-06: CB-170 (doorway cut in the deck railing)"]
fn doorway_in_the_deck_railing() {
    assert_ignored_break("CB-170");
}

#[test]
#[ignore = "T7-06: L-63 (Move Up in Schedule renumbers notes)"]
fn notes_renumbered_by_move_up_in_schedule() {
    assert_ignored_break("L-63");
}
