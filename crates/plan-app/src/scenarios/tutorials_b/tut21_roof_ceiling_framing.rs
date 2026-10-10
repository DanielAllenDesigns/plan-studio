//! Lesson 21, Roof and Ceiling Framing (pp. 364-381). Real: a built roof
//! gets rafters from Build Framing, one undo step. Open: post Raise/Lower
//! and Lock Total Height, the pivot lock, the nominal size label macro.
use crate::scenarios::tutorials_support::*;
use crate::toolbar::{Action, FramingCommand};
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;
use plan_framing::MemberKind;

#[test]
fn build_framing_gives_the_roof_rafters_in_one_step() {
    use super::support_b::count;
    let mut sim = cottage();
    sim.tool(ToolId::RoofVariant(RoofMode::Build));
    sim.click(240.0, 180.0);
    sim.ok();
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    let before = sim.app.cx.undo_depth();
    sim.action(Action::Framing(FramingCommand::Build));
    assert_eq!(sim.app.cx.undo_depth(), before + 1);
    assert!(count(&sim, 0, MemberKind::Rafter) > 20);
    sim.undo();
    assert_eq!(count(&sim, 0, MemberKind::Rafter), 0);
}

#[test]
#[ignore = "T7-21: CB-283 (post Raise/Lower and Lock Total Height after Paste Hold Position)"]
fn stacked_posts() {
    assert_ignored_break("CB-283");
}

#[test]
#[ignore = "T7-21: RF-115 (plane structure depth 11 1/4 without a pivot lock; rebuild keeps the plane's own depth)"]
fn plane_depth_follows_spec() {
    assert_ignored_break("RF-115");
}

#[test]
#[ignore = "T7-21: CB-644 (label valley rafters with the %nominal_size% macro)"]
fn nominal_size_label() {
    assert_ignored_break("CB-644");
}
